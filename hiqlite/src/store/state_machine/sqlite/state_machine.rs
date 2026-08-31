#![allow(clippy::upper_case_acronyms)]

use crate::helpers::{deserialize, set_path_access};
use crate::migration::Migration;
use crate::query::rows::RowOwned;
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::store::state_machine::sqlite::param::Param;
use crate::store::state_machine::sqlite::snapshot_builder::SQLiteSnapshotBuilder;
use crate::store::state_machine::sqlite::writer::WriterRequest::MetadataRead;
use crate::store::state_machine::sqlite::writer::{
    self, MetaPersistRequest, SqlBatch, SqlTransaction, WriterRequest,
};
use crate::store::{StorageResult, logs};
use crate::{Error, Node, NodeId};
use openraft::storage::RaftStateMachine;
use openraft::{
    EntryPayload, LogId, OptionalSend, Snapshot, SnapshotId, SnapshotMeta, StorageError,
    StorageIOError, StoredMembership,
};
use rusqlite::functions::FunctionFlags;
use rusqlite::{OpenFlags, OptionalExtension, ToSql};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::clone::Clone;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::{fs, task, time};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

type Entry = openraft::Entry<TypeConfigSqlite>;
type SnapshotData = tokio::fs::File;

// TODO uses a `Mutex<_>` inside. We could make this pool a lot
//  faster by building our own lock-free one.
pub type SqlitePool = deadpool::unmanaged::Pool<rusqlite::Connection>;

pub type Params = Vec<Param>;

/// Non-deterministic SQLite functions that are forbidden on raft write connections,
/// where every node must apply the identical statement. The dashboard pre-scans
/// manual queries against this list to return a proper error instead of the panic.
pub(crate) const FORBIDDEN_NON_DET_FNS: &[&str] = &[
    "date",
    "datetime",
    "julianday",
    "now",
    "random",
    "randomblob",
    "strftime",
    "time",
    "timediff",
    "unixepoch",
];

pub struct PathDb(pub String);
pub struct PathBackups(pub String);
pub struct PathSnapshots(pub String);
pub struct PathLockFile(pub String);

// The variant order is part of the raft log format and must stay stable and
// feature-independent (see `CacheRequest` for details).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryWrite {
    Execute(Query),
    ExecuteReturning(Query),
    Transaction(Vec<Query>),
    Batch(Cow<'static, str>),
    Migration(Vec<Migration>),
    #[allow(dead_code)] // only constructed with the `backup` feature
    Backup((NodeId, i64)),
    RTT,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub sql: Cow<'static, str>,
    pub params: Params,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Empty,
    Execute(ResponseExecute),
    ExecuteReturning(ResponseExecuteReturning),
    Transaction(Result<Vec<Result<usize, Error>>, Error>),
    Batch(ResponseBatch),
    Migrate(Result<(), Error>),
    Backup(Result<(), Error>),
    RTT,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseExecute {
    pub result: Result<usize, Error>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseExecuteReturning {
    pub result: Result<Vec<Result<RowOwned, Error>>, Error>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBatch {
    pub result: Result<Vec<Result<usize, Error>>, Error>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSnapshot {
    pub meta: SnapshotMeta<NodeId, Node>,
    pub path: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StateMachineData {
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub last_membership: StoredMembership<NodeId, Node>,
    pub last_snapshot_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StateMachineSqlite {
    // pub data: StateMachineData,
    this_node: NodeId,
    path_snapshots: String,
    #[cfg(feature = "backup")]
    path_backups: String,
    path_lock_file: String,

    #[cfg(feature = "s3")]
    s3_config: Option<Arc<crate::s3::S3Config>>,

    pub(crate) read_pool: SqlitePool,
    pub(crate) write_tx: flume::Sender<WriterRequest>,
}

impl StateMachineSqlite {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        data_dir: &str,
        filename_db: &str,
        this_node: NodeId,
        log_statements: bool,
        prepared_statement_cache_capacity: usize,
        read_pool_size: usize,
        #[cfg(feature = "s3")] s3_config: Option<Arc<crate::s3::S3Config>>,
        do_reset_metadata: bool,
        #[cfg(feature = "backup")] local_backup_keep_days: u16,
    ) -> Result<StateMachineSqlite, StorageError<NodeId>> {
        // IMPORTANT: Do NOT change the order of the db exists check!
        // DB recovery will fail otherwise!
        let mut db_exists = Self::db_exists(data_dir, filename_db).await;
        debug!("db_exists in state_machine::new(): {db_exists}");

        let (
            PathDb(path_db),
            PathBackups(path_backups),
            PathSnapshots(path_snapshots),
            PathLockFile(path_lock_file),
        ) = Self::build_folders(data_dir, true).await;

        Self::check_set_lock_file(&path_lock_file, &path_db, &mut db_exists).await;

        // Always start the writer first! -> creates mandatory tables
        let conn = Self::connect(
            path_db.to_string(),
            filename_db.to_string(),
            false,
            prepared_statement_cache_capacity,
        )
        .await
        .map_err(|err| StorageError::IO {
            source: StorageIOError::write(&err),
        })?;
        let write_tx = writer::spawn_writer(
            conn,
            this_node,
            path_lock_file.clone(),
            log_statements,
            do_reset_metadata,
            #[cfg(feature = "backup")]
            local_backup_keep_days,
        );

        let read_pool = Self::connect_read_pool(
            path_db.as_ref(),
            filename_db,
            prepared_statement_cache_capacity,
            read_pool_size,
        )
        .await
        .map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;

        let mut slf = Self {
            // data: state_machine_data,
            this_node,
            path_snapshots,
            #[cfg(feature = "backup")]
            path_backups,
            path_lock_file,
            #[cfg(feature = "s3")]
            s3_config,
            read_pool,
            write_tx,
        };

        if !db_exists && let Some(snapshot) = slf.read_current_snapshot().await? {
            slf.update_state_machine_(snapshot.path).await?;
        }

        Ok(slf)
    }

    async fn db_exists(data_dir: &str, filename_db: &str) -> bool {
        let path_db = Self::path_db(data_dir);
        let path_db_full = format!("{path_db}/{filename_db}");
        fs::File::open(&path_db_full).await.is_ok()
    }

    pub fn path_base(data_dir: &str) -> String {
        format!("{data_dir}/state_machine")
    }

    fn path_db(data_dir: &str) -> String {
        format!("{}/db", Self::path_base(data_dir))
    }

    pub async fn build_folders(
        data_dir: &str,
        create: bool,
    ) -> (PathDb, PathBackups, PathSnapshots, PathLockFile) {
        let path_base = Self::path_base(data_dir);

        let path_db = Self::path_db(data_dir);
        let path_backups = format!("{path_base}/backups");
        let path_snapshots = format!("{path_base}/snapshots");
        let path_lock_file = format!("{path_base}/lock");

        if create {
            // this may error if we did already re-create it in a lock file recovery before
            let _ = fs::create_dir_all(&path_db).await;
            set_path_access(&path_base, 0o700)
                .await
                .expect("Cannot set access rights for path_base");
            set_path_access(&path_db, 0o700)
                .await
                .expect("Cannot set access rights for path_db");

            fs::create_dir_all(&path_backups)
                .await
                .expect("create state machine folder backups");
            set_path_access(&path_backups, 0o700)
                .await
                .expect("Cannot set access rights for path_backups");

            fs::create_dir_all(&path_snapshots)
                .await
                .expect("create state machine folder snapshots");
            set_path_access(&path_snapshots, 0o700)
                .await
                .expect("Cannot set access rights for path_snapshots");
        }

        (
            PathDb(path_db),
            PathBackups(path_backups),
            PathSnapshots(path_snapshots),
            PathLockFile(path_lock_file),
        )
    }

    async fn check_set_lock_file(path_lock_file: &str, path_db: &str, db_exists: &mut bool) {
        let is_locked = fs::File::open(path_lock_file).await.is_ok();

        if is_locked {
            #[cfg(feature = "auto-heal")]
            {
                warn!(
                    "Lock file already exists: {path_lock_file}\n\
                    Node did not shut down gracefully - auto-rebuilding State Machine"
                );

                // if we can't create the lock file, we will delete the current state machine
                // data so it can be rebuilt.
                // TODO is it enough to delete DB only, or do we need to do a full wipe?
                let _ = fs::remove_dir_all(path_db).await;

                // re-create the DB folder
                if let Err(err) = fs::create_dir_all(path_db).await {
                    panic!("Cannot re-create DB folder {path_db}: {err}");
                }

                *db_exists = false;
            }

            #[cfg(not(feature = "auto-heal"))]
            panic!(
                "Lock file already exists: {}\n\
                Node did not shut down gracefully - needs manual interaction",
                path_lock_file
            );
        } else if let Err(err) = fs::File::create(path_lock_file).await {
            panic!("Error creating lock file {path_lock_file}: {err}");
        }
    }

    pub(crate) fn remove_lock_file(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    pub async fn connect(
        path: String,
        filename_db: String,
        read_only: bool,
        prepared_statement_cache_capacity: usize,
    ) -> Result<rusqlite::Connection, Error> {
        task::spawn_blocking(move || {
            let path_full = format!("{path}/{filename_db}");
            let conn = rusqlite::Connection::open(path_full)?;

            Self::apply_pragmas(&conn, read_only, prepared_statement_cache_capacity)?;
            if !read_only {
                Self::overwrite_non_det_fns(&conn);
            }

            Ok(conn)
        })
        .await?
    }

    async fn connect_read_pool(
        path: &str,
        filename_db: &str,
        prepared_statement_cache_capacity: usize,
        pool_size: usize,
    ) -> Result<SqlitePool, Error> {
        let path_full = format!("{path}/{filename_db}");

        let mut conns = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            let mut conn = Self::connect(
                path.to_string(),
                filename_db.to_string(),
                true,
                prepared_statement_cache_capacity,
            )
            .await;
            while conn.is_err() {
                time::sleep(Duration::from_millis(10)).await;
                conn = Self::connect(
                    path.to_string(),
                    filename_db.to_string(),
                    true,
                    prepared_statement_cache_capacity,
                )
                .await;
            }
            conns.push(conn?);
        }

        let pool = deadpool::unmanaged::Pool::from(conns);
        let conn = pool.get().await?;
        task::spawn_blocking(move || {
            let _ = conn.query_row("SELECT 1", (), |row| {
                let res: i64 = row.get(0)?;
                Ok(res)
            })?;
            Ok::<(), Error>(())
        })
        .await?;

        Ok(pool)
    }

    fn apply_pragmas(
        conn: &rusqlite::Connection,
        read_only: bool,
        prepared_statement_cache_capacity: usize,
    ) -> Result<(), rusqlite::Error> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // synchronous set to OFF is not an issue in our case.
        // If the OS crashes before it could flush any buffers to disk, we will rebuild the DB
        // anyway from the logs store just to be 100% sure that all cluster members are in a
        // consistent state. Setting it to OFF here gives us an ~18% boost compared to NORMAL while
        // not having any disadvantage with the Raft setup.
        conn.pragma_update(None, "synchronous", "OFF")?;

        conn.pragma_update(None, "page_size", 4096)?;
        conn.pragma_update(None, "journal_size_limit", 16384)?;
        conn.pragma_update(None, "wal_autocheckpoint", 4_000)?;

        // setting in-memory temp_store actually slows down SELECTs a little bit
        // conn.pragma_update(None, "temp_store", "memory")?;

        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
        conn.pragma_update(None, "optimize", "0x10002")?;

        // backups/snapshot restores hold an exclusive lock; 30s busy timeout stops
        // concurrent reads failing with `SQLITE_BUSY` during those windows
        conn.busy_timeout(Duration::from_secs(30))?;

        // note:
        // in tests, `mmap_size` did not show any performance benefit with the settings above

        // only allow select statements
        if read_only {
            conn.pragma_update(None, "query_only", true)?;
        } else {
            // conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        }

        // TODO make configurable
        conn.set_prepared_statement_cache_capacity(prepared_statement_cache_capacity);

        Ok(())
    }

    fn overwrite_non_det_fns(conn: &rusqlite::Connection) {
        // Overwrite the non-deterministic functions with a panicking guard: using one on
        // the write path would diverge the cluster, so it fails loudly.
        // No query-string scan: the guard only runs when a statement actually calls the
        // function, so queries that never use them pay nothing.
        for &name in FORBIDDEN_NON_DET_FNS {
            conn.create_scalar_function(
                name,
                -1,
                FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
                move |_| -> rusqlite::Result<String> {
                    panic!(
                        "forbidden usage of `{name}()` - non-deterministic functions must never be \
                        used for writing connections in a Raft cluster"
                    )
                },
            )
            .expect("Cannot register forbidden function");
        }
    }

    async fn update_state_machine_(
        &mut self,
        snapshot_path: String,
    ) -> Result<(), StorageError<NodeId>> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::SnapshotApply((snapshot_path, tx)))
            .await
            .expect("SQLite Writer rx to always be listening");

        rx.await
            .expect("Snapshot installation to succeed")
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;

        Ok(())
    }

    async fn read_current_snapshot(&mut self) -> StorageResult<Option<StoredSnapshot>> {
        let mut list = tokio::fs::read_dir(&self.path_snapshots)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;

        let mut snapshot_id: Option<Uuid> = None;
        loop {
            let entry = match list.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(err) => {
                    warn!("Error reading directory entries: {err:?}");
                    break;
                }
            };
            let file_name = entry.file_name();
            let name = file_name.to_str().unwrap_or("UNKNOWN");
            let id = match Uuid::parse_str(name) {
                Ok(uuid) => uuid,
                Err(_) => {
                    debug!("Non-UUID in snapshots folder");
                    continue;
                }
            };

            let meta = entry.metadata().await.map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;
            if meta.is_dir() {
                warn!("Invalid folder in snapshots dir: {}", name);
                continue;
            }

            if let Some(curr) = &snapshot_id {
                if &id > curr {
                    snapshot_id = Some(id);
                }
            } else {
                snapshot_id = Some(id);
            }
        }

        if snapshot_id.is_none() {
            return Ok(None);
        }

        let id = snapshot_id.unwrap();
        let path_snapshot = format!("{}/{}", self.path_snapshots, id);
        let db_path = self.path_snapshots.clone();
        let filename_db = id.to_string();

        // open a DB connection to read out the metadata
        let conn = Self::connect(db_path, filename_db, false, 2)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;

        // let path_snapshot_clone = path_snapshot.clone();
        let path_dbg = path_snapshot.clone();
        let metadata = task::spawn_blocking(move || {
            let mut stmt = conn
                .prepare("SELECT data FROM _metadata WHERE key = 'meta'")
                .map_err(|err| {
                    Error::Sqlite(
                        format!(
                            "Error preparing metadata read stmt in read from snapshot: {}",
                            err
                        )
                        .into(),
                    )
                })?;
            let mut metadata = stmt
                .query_row((), |row| {
                    let meta_bytes: Vec<u8> = row.get(0)?;
                    let metadata: StateMachineData =
                        deserialize(&meta_bytes).expect("Metadata to deserialize ok");
                    Ok(metadata)
                })
                .map_err(|err| {
                    Error::Sqlite(
                        format!(
                            "Error reading metadata from Snapshot '{}': {}",
                            path_dbg, err
                        )
                        .into(),
                    )
                })?;

            Ok::<StateMachineData, Error>(metadata)
        })
        .await
        .map_err(|err| StorageError::IO {
            source: StorageIOError::write(&err),
        })?;

        let metadata = metadata.map_err(|err| StorageError::IO {
            source: StorageIOError::write(&err),
        })?;
        let snapshot_id = id.to_string();
        assert_eq!(
            Some(snapshot_id.as_str()),
            metadata.last_snapshot_id.as_deref()
        );

        let meta = SnapshotMeta {
            last_log_id: metadata.last_applied_log_id,
            last_membership: metadata.last_membership,
            snapshot_id,
        };
        let snapshot = StoredSnapshot {
            meta,
            path: path_snapshot,
        };

        Ok(Some(snapshot))
    }
}

impl RaftStateMachine<TypeConfigSqlite> for StateMachineSqlite {
    type SnapshotBuilder = SQLiteSnapshotBuilder;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, Node>), StorageError<NodeId>> {
        let (ack, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::MetadataRead(ack))
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;
        let data = rx.await.expect("To always get Metadata from DB");

        debug!("applied_state: {:?}", data);

        Ok((data.last_applied_log_id, data.last_membership))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let entries = entries.into_iter();

        let (bound_lower, bound_upper) = entries.size_hint();
        let entries_len = bound_upper
            .expect("We always expect an upper bound to entries in apply()")
            - bound_lower
            + 1;
        let mut replies = Vec::with_capacity(entries_len);

        for entry in entries {
            let last_applied_log_id = Some(entry.log_id);

            // TODO if we always collect 1 in-flight req in a temp var to always have 1 req prepared
            // before we await the rx before, we could probably improve the throughput here a bit
            // in exchange for a more complicated logic -> test!

            let resp = match entry.payload {
                // TODO we probably need to update the log id in writer in case of ::Empty?
                EntryPayload::Blank => Response::Empty,

                EntryPayload::Normal(QueryWrite::Execute(Query { sql, params })) => {
                    let (tx, rx) = oneshot::channel();
                    let query = writer::Query::Execute(writer::SqlExecute {
                        sql,
                        params,
                        last_applied_log_id,
                        tx,
                    });

                    self.write_tx
                        .send_async(WriterRequest::Query(query))
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");
                    Response::Execute(ResponseExecute { result })
                }

                EntryPayload::Normal(QueryWrite::ExecuteReturning(Query { sql, params })) => {
                    let (tx, rx) = oneshot::channel();
                    let query = writer::Query::ExecuteReturning(writer::SqlExecuteReturning {
                        sql,
                        params,
                        last_applied_log_id,
                        tx,
                    });

                    self.write_tx
                        .send_async(WriterRequest::Query(query))
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");
                    Response::ExecuteReturning(ResponseExecuteReturning { result })
                }

                EntryPayload::Normal(QueryWrite::Transaction(queries)) => {
                    let (tx, rx) = oneshot::channel();
                    let req = WriterRequest::Query(writer::Query::Transaction(SqlTransaction {
                        queries,
                        last_applied_log_id,
                        tx,
                    }));

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");
                    Response::Transaction(result)
                }

                EntryPayload::Normal(QueryWrite::Batch(sql)) => {
                    let (tx, rx) = oneshot::channel();
                    let req = WriterRequest::Query(writer::Query::Batch(SqlBatch {
                        sql,
                        last_applied_log_id,
                        tx,
                    }));

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");
                    Response::Batch(ResponseBatch { result })
                }

                EntryPayload::Normal(QueryWrite::Backup((node_id, ts))) => {
                    #[cfg(feature = "backup")]
                    {
                        let (ack, rx) = oneshot::channel();
                        let req = WriterRequest::Backup(writer::BackupRequest {
                            node_id,
                            target_folder: self.path_backups.clone(),
                            ts,
                            #[cfg(feature = "s3")]
                            s3_config: self.s3_config.clone(),
                            last_applied_log_id,
                            ack,
                        });

                        self.write_tx
                            .send_async(req)
                            .await
                            .expect("sql writer to always be listening");

                        let result = rx.await.expect("to always get a response from sql writer");
                        Response::Backup(result)
                    }
                    #[cfg(not(feature = "backup"))]
                    unreachable!("Backup requires the `backup` feature")
                }

                EntryPayload::Normal(QueryWrite::Migration(migrations)) => {
                    let (tx, rx) = oneshot::channel();
                    let req = WriterRequest::Migrate(writer::Migrate {
                        migrations,
                        last_applied_log_id,
                        tx,
                    });

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");
                    Response::Migrate(result)
                }

                EntryPayload::Normal(QueryWrite::RTT) => {
                    let (ack, rx) = oneshot::channel();
                    let req = WriterRequest::RTT(writer::RTTRequest {
                        last_applied_log_id,
                        ack,
                    });

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    rx.await.expect("to always get a response from sql writer");
                    Response::RTT
                }

                EntryPayload::Membership(mem) => {
                    let (ack, rx) = oneshot::channel();
                    let req = WriterRequest::MetadataMembership(writer::MetaMembershipRequest {
                        last_membership: StoredMembership::new(Some(entry.log_id), mem),
                        last_applied_log_id,
                        ack,
                    });

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    rx.await.expect("to always get a response from sql writer");

                    Response::Empty
                }
            };

            replies.push(resp);
        }

        Ok(replies)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        // TODO clean up possibly existing restore files inside snapshot builder upon success

        SQLiteSnapshotBuilder {
            #[cfg(feature = "backup")]
            path_backups: self.path_backups.clone(),
            path_snapshots: self.path_snapshots.clone(),
            write_tx: self.write_tx.clone(),
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn begin_receiving_snapshot(&mut self) -> Result<Box<fs::File>, StorageError<NodeId>> {
        let path = format!("{}/temp", self.path_snapshots);

        // clean up possible existing old data
        let _ = fs::remove_file(&path).await;

        match fs::File::create(path).await {
            Ok(file) => Ok(Box::new(file)),
            Err(err) => Err(StorageError::IO {
                source: StorageIOError::write(&err),
            }),
        }
    }

    #[tracing::instrument(level = "trace", skip(self, _snapshot))]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, Node>,
        _snapshot: Box<SnapshotData>,
    ) -> Result<(), StorageError<NodeId>> {
        let src = format!("{}/temp", self.path_snapshots);
        let dest = format!("{}/{}", self.path_snapshots, meta.snapshot_id);
        // atomic move: a crash can never leave a partially copied snapshot at the final path
        fs::rename(&src, &dest)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;

        self.update_state_machine_(dest).await?;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigSqlite>>, StorageError<NodeId>> {
        match self.read_current_snapshot().await? {
            None => Ok(None),
            Some(snap) => {
                let file = fs::File::open(&snap.path)
                    .await
                    .map_err(|err| StorageError::IO {
                        source: StorageIOError::read(&err),
                    })?;

                Ok(Some(Snapshot {
                    meta: snap.meta,
                    snapshot: Box::new(file),
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_functions_panic_on_purpose_and_fail_the_statement() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        StateMachineSqlite::overwrite_non_det_fns(&conn);

        // The registered closures `panic!` on purpose; rusqlite turns that into a
        // statement error at the FFI boundary, so the query fails.
        let err = conn
            .query_row("SELECT now()", (), |row| row.get::<_, String>(0))
            .unwrap_err();
        assert!(err.to_string().contains("unwinding panic"));

        // the connection stays usable afterwards
        let one: i64 = conn.query_row("SELECT 1", (), |row| row.get(0)).unwrap();
        assert_eq!(one, 1);

        // every registered function panics, not just `now()` - "unwinding panic" proves
        // the call reached our closure, not a missing function
        for name in FORBIDDEN_NON_DET_FNS {
            let err = conn
                .query_row(&format!("SELECT {name}()"), (), |row| {
                    row.get::<_, String>(0)
                })
                .unwrap_err();
            assert!(
                err.to_string().contains("unwinding panic"),
                "{name} did not panic: {err}"
            );
        }
    }
}

#[cfg(test)]
mod serialized_enum_order {
    use super::*;

    /// The serialized variant index is part of the raft log format: a reorder
    /// would silently corrupt logs written by older builds with a different
    /// feature set. Pin the current order so a reorder fails this test instead.
    #[test]
    fn query_write_variant_order_is_stable() {
        let idx = |req: &QueryWrite| crate::helpers::serialize(req).unwrap()[0];
        let query = || Query {
            sql: Cow::Owned(String::new()),
            params: vec![],
        };

        assert_eq!(idx(&QueryWrite::Execute(query())), 0);
        assert_eq!(idx(&QueryWrite::ExecuteReturning(query())), 1);
        assert_eq!(idx(&QueryWrite::Transaction(vec![])), 2);
        assert_eq!(idx(&QueryWrite::Batch(Cow::Owned(String::new()))), 3);
        assert_eq!(idx(&QueryWrite::Migration(vec![])), 4);
        assert_eq!(idx(&QueryWrite::Backup((0, 0))), 5);
        assert_eq!(idx(&QueryWrite::RTT), 6);
    }
}
