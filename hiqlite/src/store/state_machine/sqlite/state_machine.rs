#![allow(clippy::upper_case_acronyms)]

use crate::helpers::{
    atomic_file_switch, deserialize_serde, set_path_access, validate_db_backup_snapshot,
};
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
use bincode_next::{Decode, Encode};
use fs4::FileExt;
use openraft::storage::{RaftStateMachine, SnapshotSignature};
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
use std::time::{Duration, Instant};
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

/// Total wall-clock budget for retrying read-pool connections at startup. Each attempt already
/// waits up to 30 s (busy_timeout) for exclusive lock holders, so this covers a full contention
/// cycle plus margin; beyond that the failure is unrecoverable and we panic instead of hanging
/// forever.
const READ_POOL_CONNECT_BUDGET: Duration = Duration::from_secs(60);

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
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum QueryWrite {
    Execute(Query),
    ExecuteReturning(Query),
    Transaction(Vec<Query>),
    Batch(#[bincode(with_serde)] Cow<'static, str>),
    Migration(Vec<Migration>),
    #[allow(dead_code)] // only constructed with the `backup` feature
    Backup((NodeId, i64)),
    RTT,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Query {
    #[bincode(with_serde)]
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
        #[cfg(feature = "backup")] local_backup_keep_for: Duration,
    ) -> Result<StateMachineSqlite, Box<StorageError<NodeId>>> {
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

        let lock_file = Self::check_set_lock_file(&path_lock_file, &path_db, &mut db_exists).await;

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
            local_backup_keep_for,
            lock_file,
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

    /// Checks the state-machine lock file and returns it already holding the `fs4` advisory
    /// lock, so the caller can hand it to the writer thread (which releases it on shutdown).
    ///
    /// The lock file doubles as a crash marker: we remove it on graceful shutdown, so its mere
    /// presence means the previous run did not shut down cleanly. On top of that we take an `fs4`
    /// advisory (flock) lock, which the OS releases automatically as soon as the last fd closes
    /// or the process dies. A *held* lock therefore can only come from a live process - unlike
    /// file existence, which also survives a crash. If the file is both present and held, another
    /// process is still using this state machine, so we refuse to start on top of it instead of
    /// silently deleting and rebuilding the DB.
    async fn check_set_lock_file(
        path_lock_file: &str,
        path_db: &str,
        db_exists: &mut bool,
    ) -> std::fs::File {
        // The lock file doubles as a crash marker: we remove it on graceful shutdown, so its mere
        // presence means the previous run did not shut down cleanly.
        let existed = std::fs::metadata(path_lock_file).is_ok();

        // Open (creating if needed) and try to take the advisory lock non-blocking. If another
        // live process already holds it, `try_lock` reports `WouldBlock`.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path_lock_file)
            .unwrap_or_else(|err| panic!("Cannot open lock file {path_lock_file}: {err}"));

        match FileExt::try_lock(&file) {
            Ok(()) => {}
            Err(fs4::TryLockError::WouldBlock) => panic!(
                "State machine lock file {path_lock_file} is held by another live process - \
                 refusing to start on top of it"
            ),
            Err(fs4::TryLockError::Error(err)) => {
                panic!("Error locking state machine lock file {path_lock_file}: {err}")
            }
        }

        if existed {
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
        }

        file
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
            // Bounded retry: each attempt already waits up to 30 s (busy_timeout) for exclusive
            // lock holders. After the total budget the failure is unrecoverable at startup, so
            // we panic instead of retrying forever.
            let deadline = Instant::now() + READ_POOL_CONNECT_BUDGET;
            let mut last_warn = None;
            let mut conn = Self::connect(
                path.to_string(),
                filename_db.to_string(),
                true,
                prepared_statement_cache_capacity,
            )
            .await;
            while conn.is_err() {
                let now = Instant::now();
                if now >= deadline {
                    panic!(
                        "Read-pool connection to '{path_full}' still failing after {:?} of retries \
                         (last error: {:?}). Unrecoverable at startup - panicking instead of \
                         retrying forever.",
                        READ_POOL_CONNECT_BUDGET,
                        conn.as_ref().err()
                    );
                }
                if last_warn.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(5)) {
                    warn!(
                        "Read-pool connection to '{path_full}' failing: {:?}; retrying for up to \
                         {:?} in total",
                        conn.as_ref().err(),
                        READ_POOL_CONNECT_BUDGET
                    );
                    last_warn = Some(now);
                }
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

    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
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

    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
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
                        deserialize_serde(&meta_bytes).expect("Metadata to deserialize ok");
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

        // clean up possibly existing old data
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

        validate_db_backup_snapshot(src.clone())
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write_snapshot(
                    Some(SnapshotSignature {
                        last_log_id: meta.last_log_id,
                        last_membership_log_id: *meta.last_membership.log_id(),
                        snapshot_id: meta.snapshot_id.clone(),
                    }),
                    &err,
                ),
            })?;

        atomic_file_switch(src, &dest)
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
        // `read_current_snapshot` only ever tries to read the snapshot with the highest ID, which
        // is against the trait contract by definition. However, we only ever store a single
        // snapshot anyway, so there is no need to even look for other ones. All snapshots expect
        // the latest one are being cleaned up pretty much directly.
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

/// Real production payload types must encode to *identical* bytes under bincode 2 and
/// bincode-next, and each crate must be able to decode the other's output. This is what
/// lets us swap crates without migrating existing databases on the SQLite/Raft-log path.
#[cfg(test)]
mod bincode_compat_real_types {
    use super::{Query, QueryWrite};
    use crate::migration::Migration;
    use crate::store::state_machine::sqlite::param::Param;
    use std::borrow::Cow;

    /// Asserts the 1:1 + cross-decode contract for `v` under both configs.
    ///
    /// Equivalence is checked by byte-stable re-encoding rather than `PartialEq`: bincode
    /// encoding is injective, so if decoding `bytes` yields `d` and `encode(d) == bytes`,
    /// then `d == v`. That works for these types (which do not derive `PartialEq`) and also
    /// proves the decode->re-encode round trip is stable in both directions.
    fn assert_1to1<T>(v: &T, label: &str)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    {
        // legacy (fixint): the exact config `helpers::serialize` and the Raft log store
        // persist with today.
        let b2 = bincode::serde::encode_to_vec(v, bincode::config::legacy()).unwrap();
        let bn = bincode_next::serde::encode_to_vec(v, bincode_next::config::legacy()).unwrap();
        assert_eq!(b2, bn, "{label}: legacy bytes differ");

        let (d, n) =
            bincode_next::serde::decode_from_slice::<T, _>(&b2, bincode_next::config::legacy())
                .unwrap();
        assert_eq!(
            n,
            b2.len(),
            "{label}: legacy consumed-length mismatch (bincode-next)"
        );
        let re = bincode_next::serde::encode_to_vec(&d, bincode_next::config::legacy()).unwrap();
        assert_eq!(
            re, b2,
            "{label}: bincode-next decode->re-encode not byte-stable (legacy)"
        );

        let (d, n) =
            bincode::serde::decode_from_slice::<T, _>(&bn, bincode::config::legacy()).unwrap();
        assert_eq!(
            n,
            bn.len(),
            "{label}: legacy consumed-length mismatch (bincode2)"
        );
        let re = bincode::serde::encode_to_vec(&d, bincode::config::legacy()).unwrap();
        assert_eq!(
            re, bn,
            "{label}: bincode2 decode->re-encode not byte-stable (legacy)"
        );

        // standard (varint): the default wire config.
        let b2s = bincode::serde::encode_to_vec(v, bincode::config::standard()).unwrap();
        let bns = bincode_next::serde::encode_to_vec(v, bincode_next::config::standard()).unwrap();
        assert_eq!(b2s, bns, "{label}: standard bytes differ");

        let (d, n) =
            bincode_next::serde::decode_from_slice::<T, _>(&b2s, bincode_next::config::standard())
                .unwrap();
        assert_eq!(
            n,
            b2s.len(),
            "{label}: standard consumed-length mismatch (bincode-next)"
        );
        let re = bincode_next::serde::encode_to_vec(&d, bincode_next::config::standard()).unwrap();
        assert_eq!(
            re, b2s,
            "{label}: bincode-next decode->re-encode not byte-stable (standard)"
        );

        let (d, n) =
            bincode::serde::decode_from_slice::<T, _>(&bns, bincode::config::standard()).unwrap();
        assert_eq!(
            n,
            bns.len(),
            "{label}: standard consumed-length mismatch (bincode2)"
        );
        let re = bincode::serde::encode_to_vec(&d, bincode::config::standard()).unwrap();
        assert_eq!(
            re, bns,
            "{label}: bincode2 decode->re-encode not byte-stable (standard)"
        );
    }

    fn sample_query() -> Query {
        Query {
            sql: Cow::Owned("INSERT INTO t (a, b, c) VALUES (?1, ?2, ?3)".to_string()),
            params: vec![
                Param::Null,
                Param::Integer(i64::MAX),
                Param::Real(-1.5f64),
                Param::Text("text param".into()),
                Param::Blob(vec![0u8, 1, 2, 3]),
            ],
        }
    }

    #[test]
    fn query_write_variants_are_byte_identical() {
        let q = sample_query();
        assert_1to1(&QueryWrite::Execute(q.clone()), "Execute");
        assert_1to1(&QueryWrite::ExecuteReturning(q.clone()), "ExecuteReturning");
        assert_1to1(
            &QueryWrite::Transaction(vec![q.clone(), q.clone()]),
            "Transaction",
        );
        assert_1to1(
            &QueryWrite::Batch(Cow::Owned("BEGIN; SELECT 1; COMMIT".to_string())),
            "Batch",
        );
        let migration = Migration {
            id: 1,
            name: "create_t".into(),
            hash: "ab".into(),
            content: b"CREATE TABLE t (a INTEGER)".to_vec(),
        };
        assert_1to1(&QueryWrite::Migration(vec![migration]), "Migration");
        assert_1to1(&QueryWrite::Backup((1u64, 0i64)), "Backup");
        assert_1to1(&QueryWrite::RTT, "RTT");
    }

    #[test]
    fn param_variants_are_byte_identical() {
        let cases: Vec<(&str, Param)> = vec![
            ("Null", Param::Null),
            ("Integer", Param::Integer(i64::MIN)),
            ("Real", Param::Real(f64::MAX)),
            ("Text", Param::Text("text".into())),
            ("Blob", Param::Blob(vec![0u8; 16])),
            (
                "StmtOutputIndexed",
                Param::StmtOutputIndexed(2usize, 3usize),
            ),
            (
                "StmtOutputNamed",
                Param::StmtOutputNamed(1usize, Cow::Owned("col".to_string())),
            ),
        ];
        for (label, p) in cases {
            assert_1to1(&p, label);
        }
    }

    #[test]
    fn query_is_byte_identical() {
        assert_1to1(&sample_query(), "Query");
    }
}

/// Native SIMD path interop with legacy bincode2 serde data.
///
/// This is the load-bearing guarantee for moving the persisted SQLite/Raft-log path to
/// bincode-next's fast SIMD derive: bytes written by *today's* production codec
/// (`bincode = "2"`, serde adapter) must decode correctly with the new SIMD decoder, and
/// (for mixed-version clusters / rollback) bytes written by the new SIMD encoder must
/// still decode with the old bincode2 serde decoder.
///
/// `NParam`/`NQuery`/`NMigration`/`NQueryWrite` are structural mirrors of the real
/// types, using bincode-next's native `Encode`/`Decode` derive (the SIMD path). They must
/// match the real types field-for-field and variant-for-variant.
#[cfg(test)]
mod bincode_compat_native_simd {
    use super::{Query, QueryWrite};
    use crate::migration::Migration;
    use crate::store::state_machine::sqlite::param::Param;
    use std::borrow::Cow;

    use bincode_next::{Decode, Encode};

    #[derive(Encode, Decode, PartialEq, Debug, Clone)]
    enum NParam {
        Null,
        Integer(i64),
        Real(f64),
        Text(String),
        Blob(Vec<u8>),
        StmtOutputIndexed(usize, usize),
        StmtOutputNamed(usize, String),
    }

    #[derive(Encode, Decode, PartialEq, Debug, Clone)]
    struct NQuery {
        sql: String,
        params: Vec<NParam>,
    }

    #[derive(Encode, Decode, PartialEq, Debug, Clone)]
    struct NMigration {
        id: u32,
        name: String,
        hash: String,
        content: Vec<u8>,
    }

    #[derive(Encode, Decode, PartialEq, Debug, Clone)]
    enum NQueryWrite {
        Execute(NQuery),
        ExecuteReturning(NQuery),
        Transaction(Vec<NQuery>),
        Batch(String),
        Migration(Vec<NMigration>),
        Backup((u64, i64)),
        RTT,
    }

    /// Proves bidirectional interop for one logical value under both configs.
    ///
    /// Direction 1 (the migration guarantee): encode `real` with today's production codec
    /// (`bincode::serde`) and decode those exact bytes with the new SIMD path; the result
    /// must equal `native`.
    ///
    /// Direction 2 (rollback / mixed cluster): encode `native` with the new SIMD path and
    /// decode those bytes with the old `bincode::serde`; the decoded value must re-encode to
    /// the same bytes as `real` (bincode is injective, so equal bytes == equal value). This
    /// works without `PartialEq` on the real types.
    fn check_interop<Real, Native>(real: &Real, native: &Native, label: &str)
    where
        Real: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
        Native: Encode + Decode<()> + PartialEq + std::fmt::Debug,
    {
        // ---- legacy (fixint): the exact config hiqlite persists today ----
        let b2 = bincode::serde::encode_to_vec(real, bincode::config::legacy()).unwrap();

        // Direction 1: old bincode2 serde bytes -> new SIMD decode.
        let (d1, n1) =
            bincode_next::decode_from_slice::<Native, _>(&b2, bincode_next::config::legacy())
                .unwrap();
        assert_eq!(
            n1,
            b2.len(),
            "{label}: legacy consumed-length mismatch (SIMD)"
        );
        assert_eq!(
            &d1, native,
            "{label}: SIMD decode of old bincode2 bytes != expected (legacy)"
        );

        // Direction 2: new SIMD bytes -> old bincode2 serde decode.
        let bn = bincode_next::encode_to_vec(native, bincode_next::config::legacy()).unwrap();
        let (d2, n2) =
            bincode::serde::decode_from_slice::<Real, _>(&bn, bincode::config::legacy()).unwrap();
        assert_eq!(
            n2,
            bn.len(),
            "{label}: legacy consumed-length mismatch (bincode2 of SIMD bytes)"
        );
        assert_eq!(
            bincode::serde::encode_to_vec(&d2, bincode::config::legacy()).unwrap(),
            bincode::serde::encode_to_vec(real, bincode::config::legacy()).unwrap(),
            "{label}: bincode2 decode of SIMD bytes != expected (legacy)"
        );

        // ---- standard (varint): the default wire config ----
        let b2s = bincode::serde::encode_to_vec(real, bincode::config::standard()).unwrap();
        let (d1s, n1s) =
            bincode_next::decode_from_slice::<Native, _>(&b2s, bincode_next::config::standard())
                .unwrap();
        assert_eq!(
            n1s,
            b2s.len(),
            "{label}: standard consumed-length mismatch (SIMD)"
        );
        assert_eq!(
            &d1s, native,
            "{label}: SIMD decode of old bincode2 bytes != expected (standard)"
        );

        let bns = bincode_next::encode_to_vec(native, bincode_next::config::standard()).unwrap();
        let (d2s, n2s) =
            bincode::serde::decode_from_slice::<Real, _>(&bns, bincode::config::standard())
                .unwrap();
        assert_eq!(
            n2s,
            bns.len(),
            "{label}: standard consumed-length mismatch (bincode2 of SIMD bytes)"
        );
        assert_eq!(
            bincode::serde::encode_to_vec(&d2s, bincode::config::standard()).unwrap(),
            bincode::serde::encode_to_vec(real, bincode::config::standard()).unwrap(),
            "{label}: bincode2 decode of SIMD bytes != expected (standard)"
        );
    }

    fn sample_params_real() -> Vec<Param> {
        vec![
            Param::Null,
            Param::Integer(i64::MAX),
            Param::Real(-1.5f64),
            Param::Text("text param".into()),
            Param::Blob(vec![0u8, 1, 2, 3]),
            Param::StmtOutputIndexed(2usize, 3usize),
            Param::StmtOutputNamed(1usize, Cow::Owned("col".to_string())),
        ]
    }

    fn sample_params_native() -> Vec<NParam> {
        vec![
            NParam::Null,
            NParam::Integer(i64::MAX),
            NParam::Real(-1.5f64),
            NParam::Text("text param".to_string()),
            NParam::Blob(vec![0u8, 1, 2, 3]),
            NParam::StmtOutputIndexed(2usize, 3usize),
            NParam::StmtOutputNamed(1usize, "col".to_string()),
        ]
    }

    fn sample_query_real() -> Query {
        Query {
            sql: Cow::Owned("INSERT INTO t (a, b, c) VALUES (?1, ?2, ?3)".to_string()),
            params: sample_params_real(),
        }
    }

    fn sample_query_native() -> NQuery {
        NQuery {
            sql: "INSERT INTO t (a, b, c) VALUES (?1, ?2, ?3)".to_string(),
            params: sample_params_native(),
        }
    }

    fn sample_migration_real() -> Migration {
        Migration {
            id: 1,
            name: "create_t".into(),
            hash: "ab".into(),
            content: b"CREATE TABLE t (a INTEGER)".to_vec(),
        }
    }

    fn sample_migration_native() -> NMigration {
        NMigration {
            id: 1,
            name: "create_t".to_string(),
            hash: "ab".to_string(),
            content: b"CREATE TABLE t (a INTEGER)".to_vec(),
        }
    }

    #[test]
    fn param_variants_interop() {
        let cases: Vec<(&str, Param, NParam)> = vec![
            ("Null", Param::Null, NParam::Null),
            (
                "Integer",
                Param::Integer(i64::MIN),
                NParam::Integer(i64::MIN),
            ),
            ("Real", Param::Real(f64::MAX), NParam::Real(f64::MAX)),
            (
                "Text",
                Param::Text("text".into()),
                NParam::Text("text".to_string()),
            ),
            (
                "Blob",
                Param::Blob(vec![0u8; 16]),
                NParam::Blob(vec![0u8; 16]),
            ),
            (
                "StmtOutputIndexed",
                Param::StmtOutputIndexed(2usize, 3usize),
                NParam::StmtOutputIndexed(2usize, 3usize),
            ),
            (
                "StmtOutputNamed",
                Param::StmtOutputNamed(1usize, Cow::Owned("col".to_string())),
                NParam::StmtOutputNamed(1usize, "col".to_string()),
            ),
        ];
        for (label, r, n) in cases {
            check_interop(&r, &n, label);
        }
    }

    #[test]
    fn query_interop() {
        check_interop(&sample_query_real(), &sample_query_native(), "Query");
    }

    #[test]
    fn migration_interop() {
        check_interop(
            &sample_migration_real(),
            &sample_migration_native(),
            "Migration",
        );
    }

    #[test]
    fn query_write_variants_interop() {
        let q = sample_query_real();
        let nq = sample_query_native();
        let m = sample_migration_real();
        let nm = sample_migration_native();

        check_interop::<QueryWrite, NQueryWrite>(
            &QueryWrite::Execute(q.clone()),
            &NQueryWrite::Execute(nq.clone()),
            "QW.Execute",
        );
        check_interop::<QueryWrite, NQueryWrite>(
            &QueryWrite::ExecuteReturning(q.clone()),
            &NQueryWrite::ExecuteReturning(nq.clone()),
            "QW.ExecuteReturning",
        );
        check_interop::<QueryWrite, NQueryWrite>(
            &QueryWrite::Transaction(vec![q.clone(), q.clone()]),
            &NQueryWrite::Transaction(vec![nq.clone(), nq.clone()]),
            "QW.Transaction",
        );
        check_interop::<QueryWrite, NQueryWrite>(
            &QueryWrite::Batch(Cow::Owned("BEGIN; SELECT 1; COMMIT".to_string())),
            &NQueryWrite::Batch("BEGIN; SELECT 1; COMMIT".to_string()),
            "QW.Batch",
        );
        check_interop::<QueryWrite, NQueryWrite>(
            &QueryWrite::Migration(vec![m.clone()]),
            &NQueryWrite::Migration(vec![nm.clone()]),
            "QW.Migration",
        );
        check_interop::<QueryWrite, NQueryWrite>(
            &QueryWrite::Backup((1u64, 0i64)),
            &NQueryWrite::Backup((1u64, 0i64)),
            "QW.Backup",
        );
        check_interop::<QueryWrite, NQueryWrite>(&QueryWrite::RTT, &NQueryWrite::RTT, "QW.RTT");
    }
}

/// Golden-reference tests for the *real* openraft wire payload.
///
/// `helpers::serialize` / `helpers::deserialize` are the single choke point for every byte that
/// crosses the raft wire (`network/raft_client.rs` sends `RaftStreamRequest` through them). The
/// envelope type here is exactly what production serializes: [`RaftStreamRequest`] and its
/// openraft message payloads.
///
/// Two independent guarantees are pinned per variant:
///
/// 1. **Golden bytes** — the base64 of the production-encoded wire bytes is a string constant
///    below. Production encodes with the `bincode_next` serde adapter (`legacy()`); the test also
///    re-encodes with the legacy `bincode` crate to pin the exact byte stream. Because bincode is
///    untyped (raw bytes, no schema), any openraft struct-layout change across versions (field
///    order, added/removed field, type change) silently changes this byte stream; comparing against
///    the pinned constant catches it.
/// 2. **Codec 1:1** — the `bincode_next` serde adapter must emit *byte-identical* output to the
///    production codec on this exact wire type (and cross-decode it), proving the drop-in swap is
///    safe for the real payload, not just hand-built mirrors.
#[cfg(test)]
mod raft_wire_goldens {
    use super::{Query, QueryWrite, TypeConfigSqlite};
    use crate::Node;
    use crate::migration::Migration;
    use crate::network::raft_server::RaftStreamRequest;
    use crate::store::state_machine::sqlite::param::Param;
    use std::borrow::Cow;
    use std::collections::BTreeMap;

    use openraft::raft::{AppendEntriesRequest, InstallSnapshotRequest, VoteRequest};
    use openraft::{
        CommittedLeaderId, Entry, EntryPayload, LeaderId, LogId, Membership, SnapshotMeta, Vote,
    };

    // Pinned golden references (see module docs). Regenerate with:
    //   cargo test -p hiqlite --offline --lib raft_wire_goldens::dump_wire_goldens -- --nocapture
    const GOLDEN_APPEND_DB: &str = "AAAAAAcAAAAAAAAABQAAAAAAAAABAAAAAAAAAAEBBAAAAAAAAAAAAAAAAAAAAAoAAAAAAAAACQAAAAAAAAAFAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAAAAABQAAAAAAAAABAAAAAAAAAAEAAAAAAAAAAQAAAAAAAAArAAAAAAAAAElOU0VSVCBJTlRPIHQgKGEsIGIsIGMpIFZBTFVFUyAoPzEsID8yLCA/MykHAAAAAAAAAAAAAAABAAAA/////////38CAAAAAAAAAAAA+L8DAAAACgAAAAAAAAB0ZXh0IHBhcmFtBAAAAAQAAAAAAAAAAAECAwUAAAACAAAAAAAAAAMAAAAAAAAABgAAAAEAAAAAAAAAAwAAAAAAAABjb2wFAAAAAAAAAAEAAAAAAAAAAgAAAAAAAAABAAAAAQAAACsAAAAAAAAASU5TRVJUIElOVE8gdCAoYSwgYiwgYykgVkFMVUVTICg/MSwgPzIsID8zKQcAAAAAAAAAAAAAAAEAAAD/////////fwIAAAAAAAAAAAD4vwMAAAAKAAAAAAAAAHRleHQgcGFyYW0EAAAABAAAAAAAAAAAAQIDBQAAAAIAAAAAAAAAAwAAAAAAAAAGAAAAAQAAAAAAAAADAAAAAAAAAGNvbAUAAAAAAAAAAQAAAAAAAAADAAAAAAAAAAEAAAACAAAAAgAAAAAAAAArAAAAAAAAAElOU0VSVCBJTlRPIHQgKGEsIGIsIGMpIFZBTFVFUyAoPzEsID8yLCA/MykHAAAAAAAAAAAAAAABAAAA/////////38CAAAAAAAAAAAA+L8DAAAACgAAAAAAAAB0ZXh0IHBhcmFtBAAAAAQAAAAAAAAAAAECAwUAAAACAAAAAAAAAAMAAAAAAAAABgAAAAEAAAAAAAAAAwAAAAAAAABjb2wrAAAAAAAAAElOU0VSVCBJTlRPIHQgKGEsIGIsIGMpIFZBTFVFUyAoPzEsID8yLCA/MykHAAAAAAAAAAAAAAABAAAA/////////38CAAAAAAAAAAAA+L8DAAAACgAAAAAAAAB0ZXh0IHBhcmFtBAAAAAQAAAAAAAAAAAECAwUAAAACAAAAAAAAAAMAAAAAAAAABgAAAAEAAAAAAAAAAwAAAAAAAABjb2wFAAAAAAAAAAEAAAAAAAAABAAAAAAAAAABAAAAAwAAABcAAAAAAAAAQkVHSU47IFNFTEVDVCAxOyBDT01NSVQFAAAAAAAAAAEAAAAAAAAABQAAAAAAAAABAAAABAAAAAEAAAAAAAAAAQAAAAgAAAAAAAAAY3JlYXRlX3QCAAAAAAAAAGFiGgAAAAAAAABDUkVBVEUgVEFCTEUgdCAoYSBJTlRFR0VSKQUAAAAAAAAAAQAAAAAAAAAGAAAAAAAAAAEAAAAFAAAAAQAAAAAAAAAAAAAAAAAAAAUAAAAAAAAAAQAAAAAAAAAHAAAAAAAAAAEAAAAGAAAABQAAAAAAAAABAAAAAAAAAGMAAAAAAAAAAgAAAAEAAAAAAAAAAgAAAAAAAAABAAAAAAAAAAIAAAAAAAAAAgAAAAAAAAABAAAAAAAAAAEAAAAAAAAADgAAAAAAAAAxMjcuMC4wLjE6MzA4MQ4AAAAAAAAAMTI3LjAuMC4xOjMwODACAAAAAAAAAAIAAAAAAAAADgAAAAAAAAAxMjcuMC4wLjE6NDA4MQ4AAAAAAAAAMTI3LjAuMC4xOjQwODABBQAAAAAAAAABAAAAAAAAAAkAAAAAAAAA";
    const GOLDEN_VOTE_DB: &str =
        "AQAAAAgAAAAAAAAAAwAAAAAAAAACAAAAAAAAAAABAgAAAAAAAAABAAAAAAAAAAUAAAAAAAAA";
    const GOLDEN_SNAPSHOT_DB: &str = "AgAAAAkAAAAAAAAABgAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGwAAAAAAAABzbmFwc2hvdC1wYXlsb2FkLTAxMjM0NTY3ODkB";

    /// Minimal RFC 4648 standard-alphabet base64 encoder (with padding). No external dep.
    fn b64_encode(data: &[u8]) -> String {
        const ALPHA: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            out.push(ALPHA[(b0 >> 2) as usize] as char);
            out.push(ALPHA[((b0 & 0b11) << 4 | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHA[(((b1 & 0b1111) << 2) | (b2 >> 6)) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHA[(b2 & 0b111111) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    #[test]
    fn b64_encoder_matches_rfc4648_vectors() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(b"f"), "Zg==");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(b64_encode(b"foo"), "Zm9v");
        assert_eq!(b64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(b64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
    }

    // ---- deterministic sample data (fixed values only; no randomness, no maps-of-maps) ----

    fn sample_params() -> Vec<Param> {
        vec![
            Param::Null,
            Param::Integer(i64::MAX),
            Param::Real(-1.5f64),
            Param::Text("text param".into()),
            Param::Blob(vec![0u8, 1, 2, 3]),
            Param::StmtOutputIndexed(2usize, 3usize),
            Param::StmtOutputNamed(1usize, Cow::Owned("col".to_string())),
        ]
    }

    fn sample_query() -> Query {
        Query {
            sql: Cow::Owned("INSERT INTO t (a, b, c) VALUES (?1, ?2, ?3)".to_string()),
            params: sample_params(),
        }
    }

    fn sample_migration() -> Migration {
        Migration {
            id: 1,
            name: "create_t".into(),
            hash: "ab".into(),
            content: b"CREATE TABLE t (a INTEGER)".to_vec(),
        }
    }

    /// The exact wire envelope for `AppendDB`, with entries carrying *every* [`QueryWrite`] variant
    /// plus the Blank and Membership [`EntryPayload`] variants, so one golden pins the whole tree.
    fn build_append_db() -> RaftStreamRequest {
        let q = sample_query();
        let m = sample_migration();
        let data_variants = vec![
            QueryWrite::Execute(q.clone()),
            QueryWrite::ExecuteReturning(q.clone()),
            QueryWrite::Transaction(vec![q.clone(), q.clone()]),
            QueryWrite::Batch(Cow::Owned("BEGIN; SELECT 1; COMMIT".to_string())),
            QueryWrite::Migration(vec![m.clone()]),
            QueryWrite::Backup((1u64, 0i64)),
            QueryWrite::RTT,
        ];

        let mut entries: Vec<Entry<TypeConfigSqlite>> = vec![];
        // EntryPayload::Blank
        entries.push(Entry {
            log_id: LogId::new(CommittedLeaderId::new(5u64, 1u64), 0u64),
            payload: EntryPayload::Blank,
        });
        // EntryPayload::Normal carrying every QueryWrite variant.
        for (i, qw) in data_variants.into_iter().enumerate() {
            entries.push(Entry {
                log_id: LogId::new(CommittedLeaderId::new(5u64, 1u64), i as u64 + 1),
                payload: EntryPayload::Normal(qw),
            });
        }
        // EntryPayload::Membership (exercises the Membership/Node serialization layout).
        let mut nodes = BTreeMap::new();
        nodes.insert(
            1u64,
            Node {
                id: 1,
                addr_raft: "127.0.0.1:3081".into(),
                addr_api: "127.0.0.1:3080".into(),
            },
        );
        nodes.insert(
            2u64,
            Node {
                id: 2,
                addr_raft: "127.0.0.1:4081".into(),
                addr_api: "127.0.0.1:4080".into(),
            },
        );
        let membership = Membership::<u64, Node>::from(nodes);
        entries.push(Entry {
            log_id: LogId::new(CommittedLeaderId::new(5u64, 1u64), 99u64),
            payload: EntryPayload::Membership(membership),
        });

        RaftStreamRequest::AppendDB((
            7usize,
            AppendEntriesRequest {
                vote: Vote {
                    leader_id: LeaderId::new(5u64, 1u64),
                    committed: true,
                },
                prev_log_id: Some(LogId::new(CommittedLeaderId::new(4u64, 0u64), 10u64)),
                entries,
                leader_commit: Some(LogId::new(CommittedLeaderId::new(5u64, 1u64), 9u64)),
            },
        ))
    }

    fn build_vote_db() -> RaftStreamRequest {
        RaftStreamRequest::VoteDB((
            8usize,
            VoteRequest {
                vote: Vote {
                    leader_id: LeaderId::new(3u64, 2u64),
                    committed: false,
                },
                last_log_id: Some(LogId::new(CommittedLeaderId::new(2u64, 1u64), 5u64)),
            },
        ))
    }

    fn build_snapshot_db() -> RaftStreamRequest {
        RaftStreamRequest::SnapshotDB((
            9usize,
            InstallSnapshotRequest {
                vote: Vote {
                    leader_id: LeaderId::new(6u64, 1u64),
                    committed: false,
                },
                meta: SnapshotMeta::default(),
                offset: 0u64,
                data: b"snapshot-payload-0123456789".to_vec(),
                done: true,
            },
        ))
    }

    /// Shared assertions for one wire variant (see module docs).
    fn assert_golden<T>(payload: &T, golden: &str, label: &str)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    {
        // The exact bytes on the wire today (helpers::serialize == bincode2 serde, legacy()).
        let b2 = bincode::serde::encode_to_vec(payload, bincode::config::legacy()).unwrap();

        // 1. Golden pin: catches any openraft struct-layout change across versions.
        assert_eq!(
            b64_encode(&b2),
            golden,
            "{label}: wire bytes differ from the pinned golden — did the openraft layout change?"
        );

        // 2. Codec 1:1 on the real wire type: bincode-next serde must emit identical bytes.
        let bn =
            bincode_next::serde::encode_to_vec(payload, bincode_next::config::legacy()).unwrap();
        assert_eq!(
            b2, bn,
            "{label}: bincode-next serde wire bytes != bincode2 serde wire bytes"
        );

        // 3. Cross-decode: read the production bytes with bincode-next serde; re-encoding must
        //    round-trip to the same bytes (bincode is injective).
        let (dec, n) =
            bincode_next::serde::decode_from_slice::<T, _>(&b2, bincode_next::config::legacy())
                .unwrap();
        assert_eq!(
            n,
            b2.len(),
            "{label}: bincode-next serde consumed-length mismatch"
        );
        assert_eq!(
            bincode::serde::encode_to_vec(&dec, bincode::config::legacy()).unwrap(),
            b2,
            "{label}: cross-decoded value re-encodes differently"
        );
    }

    #[test]
    fn golden_append_db() {
        assert_golden(&build_append_db(), GOLDEN_APPEND_DB, "AppendDB");
    }

    #[test]
    fn golden_vote_db() {
        assert_golden(&build_vote_db(), GOLDEN_VOTE_DB, "VoteDB");
    }

    #[test]
    fn golden_snapshot_db() {
        assert_golden(&build_snapshot_db(), GOLDEN_SNAPSHOT_DB, "SnapshotDB");
    }

    /// Prints the base64 of each wire variant's production-encoded bytes so they can be pasted into
    /// the `GOLDEN_*` constants above. Run with `--nocapture`.
    #[test]
    fn dump_wire_goldens() {
        for (name, p) in [
            ("GOLDEN_APPEND_DB", build_append_db()),
            ("GOLDEN_VOTE_DB", build_vote_db()),
            ("GOLDEN_SNAPSHOT_DB", build_snapshot_db()),
        ] {
            let b2 = bincode::serde::encode_to_vec(&p, bincode::config::legacy()).unwrap();
            eprintln!(
                "\n=== {name} ({} bytes) ===\nconst {name}: &str = \"{}\";",
                b2.len(),
                b64_encode(&b2)
            );
        }
    }
}
