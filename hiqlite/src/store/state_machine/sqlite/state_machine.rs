#![allow(clippy::upper_case_acronyms)]

use crate::helpers::{
    atomic_file_switch, deserialize_serde, set_path_access, validate_db_backup_snapshot,
};
use crate::migration::Migration;
use crate::query::rows::RowsOwnedChecked;
use crate::reader::{self, ReaderRequest};
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
use futures_util::{Stream, StreamExt};
use openraft::storage::{EntryResponder, RaftStateMachine, Snapshot};
use openraft::type_config::alias::{
    EntryOf, LogIdOf, SnapshotMetaOf, SnapshotOf, StoredMembershipOf,
};
use openraft::{EntryPayload, OptionalSend};
use rusqlite::functions::FunctionFlags;
use rusqlite::{OpenFlags, OptionalExtension, ToSql};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::clone::Clone;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::{fs, task, time};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

type Entry = EntryOf<TypeConfigSqlite>;
type SnapshotData = tokio::fs::File;

pub type Params = Vec<Param>;

/// Total wall-clock budget for retrying the read connections at startup. Each attempt already
/// waits up to 30 s (`busy_timeout`) for exclusive lock holders, so this covers a full contention
/// cycle plus margin; beyond that the failure is unrecoverable and we panic instead of hanging
/// forever.
const READ_CONNS_CONNECT_BUDGET: Duration = Duration::from_secs(60);

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

impl fmt::Display for QueryWrite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
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
    pub result: Result<RowsOwnedChecked, Error>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBatch {
    pub result: Result<Vec<Result<usize, Error>>, Error>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSnapshot {
    pub meta: SnapshotMetaOf<TypeConfigSqlite>,
    pub path: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StateMachineData {
    pub last_applied_log_id: Option<LogIdOf<TypeConfigSqlite>>,
    pub last_membership: StoredMembershipOf<TypeConfigSqlite>,
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

    pub(crate) read_actors: flume::Sender<ReaderRequest>,
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
        saturated_pool_threshold: Duration,
        slow_query_threshold: Duration,
        log_slow_queries: bool,
        #[cfg(feature = "s3")] s3_config: Option<Arc<crate::s3::S3Config>>,
        do_reset_metadata: bool,
        #[cfg(feature = "backup")] local_backup_keep_for: Duration,
        #[cfg(feature = "backup")] backup_file_prefix: &str,
    ) -> Result<StateMachineSqlite, io::Error> {
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
            path_db.clone(),
            filename_db.to_string(),
            prepared_statement_cache_capacity,
        )
        .await?;
        let write_tx = writer::spawn_writer(
            conn,
            this_node,
            path_lock_file.clone(),
            log_statements,
            do_reset_metadata,
            #[cfg(feature = "backup")]
            local_backup_keep_for,
            #[cfg(feature = "backup")]
            backup_file_prefix.to_string(),
            lock_file,
        );

        let conns = Self::connect_read_conns(
            path_db.as_ref(),
            filename_db,
            prepared_statement_cache_capacity,
            read_pool_size,
        )
        .await?;
        let read_actors = reader::spawn_read_actors(
            conns,
            saturated_pool_threshold,
            slow_query_threshold,
            log_slow_queries,
        );

        let mut slf = Self {
            // data: state_machine_data,
            this_node,
            path_snapshots,
            #[cfg(feature = "backup")]
            path_backups,
            path_lock_file,
            #[cfg(feature = "s3")]
            s3_config,
            read_actors,
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

    pub fn path_backups(data_dir: &str) -> String {
        format!("{}/backups", Self::path_base(data_dir))
    }

    pub fn path_db(data_dir: &str) -> String {
        format!("{}/db", Self::path_base(data_dir))
    }

    pub fn path_snapshots(data_dir: &str) -> String {
        format!("{}/snapshots", Self::path_base(data_dir))
    }

    pub fn path_lock_file(data_dir: &str) -> String {
        format!("{}/lock", Self::path_base(data_dir))
    }

    pub async fn build_folders(
        data_dir: &str,
        create: bool,
    ) -> (PathDb, PathBackups, PathSnapshots, PathLockFile) {
        let path_base = Self::path_base(data_dir);

        let path_db = Self::path_db(data_dir);
        let path_backups = Self::path_backups(data_dir);
        let path_snapshots = Self::path_snapshots(data_dir);
        let path_lock_file = Self::path_lock_file(data_dir);

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

    /// Checks the state-machine lock file and returns it already holding the advisory
    /// lock, so the caller can hand it to the writer thread (which releases it on shutdown).
    ///
    /// The lock file doubles as a crash marker: we remove it on graceful shutdown, so its mere
    /// presence means the previous run did not shut down cleanly. On top of that we take an
    /// advisory (flock) lock, which the OS releases automatically as soon as the last fd closes
    /// or the process dies. A *held* lock therefore can only come from a live process - unlike
    /// file existence, which also survives a crash. If the file is both present and held, another
    /// process is still using this state machine, so we refuse to start on top of it instead of
    /// silently deleting and rebuilding the DB.
    // openraft declares this trait method `async`, so the impl has to keep `async`; the lock file
    // check is pure std file work and has nothing to await.
    #[allow(clippy::unused_async, clippy::unused_async_trait_impl)]
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

        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => panic!(
                "State machine lock file {path_lock_file} is held by another live process - \
                 refusing to start on top of it"
            ),
            Err(std::fs::TryLockError::Error(err)) => {
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
                "Lock file already exists: {path_lock_file}\n\
                Node did not shut down gracefully - needs manual interaction"
            );
        }

        file
    }

    pub(crate) fn remove_lock_file(path: &str) {
        let _ = std::fs::remove_file(path);
    }

    /// The read-write opener: the writer's own connection, and the snapshot metadata read.
    ///
    /// It keeps `CREATE`, because the writer is the one that creates the file.  The read actors'
    /// handles come from [`Self::connect_read_conn`], which opens them read-only.
    ///
    /// `NO_MUTEX` drops the mutex SQLite puts around every call on a connection.  It is a promise
    /// about how the handle is used, not a setting: the caller must keep one thread touching a
    /// connection at a time.  The writer's handle is moved into the writer thread and never leaves
    /// it, and the snapshot metadata read is a handle of its own, used by the one task that opens
    /// it.
    ///
    /// `URI` is what `Connection::open` passes, and it stays: a `file:`-style path keeps parsing as
    /// a URI, where dropping the flag would silently turn it into a plain filename.
    pub async fn connect(
        path: String,
        filename_db: String,
        prepared_statement_cache_capacity: usize,
    ) -> Result<rusqlite::Connection, Error> {
        task::spawn_blocking(move || {
            let path_full = format!("{path}/{filename_db}");
            let conn = rusqlite::Connection::open_with_flags(
                path_full,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI,
            )?;

            Self::apply_pragmas(&conn, false, prepared_statement_cache_capacity)?;
            Self::overwrite_non_det_fns(&conn);

            Ok(conn)
        })
        .await?
    }

    /// One read connection, opened read-only and warmed up, ready to hand to an actor.
    ///
    /// The warm-up is part of the open, not a step after it: a connection only joins the pool once
    /// it has answered a query, so the first query of a caller does not pay for the first page
    /// reads of a cold file, and a connection that cannot answer one never reaches an actor.
    ///
    /// It reads the schema of the file, which is the first real page read a connection can make:
    /// `SELECT 1` answers from a constant and touches no file at all, so it warms nothing and
    /// proves nothing about the file.  The schema is the right thing to read - it is in every valid
    /// SQLite file, on a fresh one exactly like on an established one, and a file that is not a
    /// database fails on reading it.  `_metadata` would be the obvious table, and it is not usable
    /// here: the writer creates it in its own thread, which this open runs next to, so a fresh
    /// instance would race with the table it warms on.
    ///
    /// `NO_MUTEX` is the same promise as on the writer's handle: this connection is moved into the
    /// actor thread that owns it, and it is the only one that ever touches it.  `URI` is kept for
    /// the same reason it is kept there.
    async fn connect_read_conn(
        path: &str,
        filename_db: &str,
        prepared_statement_cache_capacity: usize,
    ) -> Result<rusqlite::Connection, Error> {
        let path = path.to_string();
        let filename_db = filename_db.to_string();
        task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open_with_flags(
                format!("{path}/{filename_db}"),
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI,
            )?;
            Self::apply_pragmas(&conn, true, prepared_statement_cache_capacity)?;
            // The warm-up belongs to the open: a connection only joins the pool once it has
            // answered a query.
            conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                row.get::<_, i64>(0)
            })?;
            Ok(conn)
        })
        .await?
    }

    async fn connect_read_conns(
        path: &str,
        filename_db: &str,
        prepared_statement_cache_capacity: usize,
        size: usize,
    ) -> Result<Vec<rusqlite::Connection>, Error> {
        let path_full = format!("{path}/{filename_db}");

        // Bounded retry over the whole set: each attempt already waits up to 30 s (busy_timeout)
        // for exclusive lock holders.  After the total budget the failure is unrecoverable at
        // startup, so we panic instead of retrying forever.
        let deadline = Instant::now() + READ_CONNS_CONNECT_BUDGET;
        let mut last_warn = None;

        let mut conns = Vec::with_capacity(size);
        for _ in 0..size {
            let conn = loop {
                match Self::connect_read_conn(path, filename_db, prepared_statement_cache_capacity)
                    .await
                {
                    Ok(conn) => break conn,
                    Err(err) => {
                        let now = Instant::now();
                        if now >= deadline {
                            panic!(
                                "Read connection to '{path_full}' still failing after \
                                 {READ_CONNS_CONNECT_BUDGET:?} of retries (last error: {err:?}). \
                                 Unrecoverable at startup - panicking instead of retrying forever."
                            );
                        }
                        if last_warn.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(5))
                        {
                            warn!(
                                "Read connection to '{path_full}' failing: {err:?}; retrying for \
                                 up to {READ_CONNS_CONNECT_BUDGET:?} in total"
                            );
                            last_warn = Some(now);
                        }
                        time::sleep(Duration::from_millis(10)).await;
                    }
                }
            };
            conns.push(conn);
        }

        Ok(conns)
    }

    /// Applies the pragmas of one connection, in the shape its role is allowed to write.
    ///
    /// The pragmas that change the database file itself - the journal mode, the page size, the
    /// vacuum mode and `PRAGMA optimize` - belong to the writer alone.  It opens first, it owns the
    /// file, and a read handle is opened read-only, so a read connection that tried to set them
    /// would fail with `SQLITE_READONLY` on any file that is not already in that shape, which is
    /// exactly what a database restored from a backup file is.  Everything a read connection does
    /// set is scoped to its own connection.
    fn apply_pragmas(
        conn: &rusqlite::Connection,
        read_only: bool,
        prepared_statement_cache_capacity: usize,
    ) -> Result<(), rusqlite::Error> {
        if !read_only {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "page_size", 4096)?;
            conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
            // `PRAGMA optimize` writes ANALYZE stats into the database file.
            conn.pragma_update(None, "optimize", "0x10002")?;
        }

        // synchronous set to OFF is not an issue in our case.
        // If the OS crashes before it could flush any buffers to disk, we will rebuild the DB
        // anyway from the logs store just to be 100% sure that all cluster members are in a
        // consistent state. Setting it to OFF here gives us an ~18% boost compared to NORMAL while
        // not having any disadvantage with the Raft setup.
        conn.pragma_update(None, "synchronous", "OFF")?;

        conn.pragma_update(None, "journal_size_limit", 16384)?;
        conn.pragma_update(None, "wal_autocheckpoint", 4_000)?;

        // setting in-memory temp_store actually slows down SELECTs a little bit
        // conn.pragma_update(None, "temp_store", "memory")?;

        conn.pragma_update(None, "foreign_keys", "ON")?;

        // backups/snapshot restores hold an exclusive lock; 30s busy timeout stops
        // concurrent reads failing with `SQLITE_BUSY` during those windows
        conn.busy_timeout(Duration::from_secs(30))?;

        // note:
        // in tests, `mmap_size` did not show any performance benefit with the settings above

        // only allow select statements
        if read_only {
            conn.pragma_update(None, "query_only", true)?;
        } else {
            // `locking_mode=EXCLUSIVE` on the writer is off on purpose, and it is not a tuning
            // knob: while any connection holds the database in exclusive locking mode, every
            // query against it answers `SQLITE_BUSY`, so the read actors stop answering.  Its
            // documented gains are a small drop in filesystem syscalls and WAL without the
            // shared-memory wal-index, which is for a process that is the only one touching the
            // file.  This node is the opposite: one writer thread and `read_pool_size` readers,
            // reading while it writes, which is the whole point of WAL here.  Measured: readers
            // on read-only handles fail their first query with "database is locked" the moment
            // the writer sets it.
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
    async fn update_state_machine_(&mut self, snapshot_path: String) -> Result<(), io::Error> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::SnapshotApply((snapshot_path, tx)))
            .await
            .expect("SQLite Writer rx to always be listening");

        rx.await.expect("Snapshot installation to succeed")?;

        Ok(())
    }

    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
    async fn read_current_snapshot(&mut self) -> StorageResult<Option<StoredSnapshot>> {
        let mut list = tokio::fs::read_dir(&self.path_snapshots).await?;

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
            let Ok(id) = Uuid::parse_str(name) else {
                debug!("Non-UUID in snapshots folder");
                continue;
            };

            let meta = entry.metadata().await?;
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
        let conn = Self::connect(db_path, filename_db, 2).await?;

        // let path_snapshot_clone = path_snapshot.clone();
        let path_dbg = path_snapshot.clone();
        let metadata = task::spawn_blocking(move || {
            let mut stmt = conn
                .prepare("SELECT data FROM _metadata WHERE key = 'meta'")
                .map_err(|err| {
                    Error::Sqlite(
                        format!("Error preparing metadata read stmt in read from snapshot: {err}")
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
                        format!("Error reading metadata from Snapshot '{path_dbg}': {err}").into(),
                    )
                })?;

            Ok::<StateMachineData, Error>(metadata)
        })
        .await?;

        let metadata = metadata?;

        // openraft 0.10 has no `snapshot_id` in `SnapshotMeta` anymore; the file name is the
        // snapshot's identity on disk. `last_snapshot_id` stays in our metadata, but a snapshot
        // installed from a peer keeps the sender's id there, so it is advisory only.
        let meta = SnapshotMetaOf::<TypeConfigSqlite> {
            last_log_id: metadata.last_applied_log_id,
            last_membership: metadata.last_membership,
        };
        let snapshot = StoredSnapshot {
            meta,
            path: path_snapshot,
        };

        Ok(Some(snapshot))
    }
}

impl RaftStateMachine<TypeConfigSqlite> for StateMachineSqlite {
    type SnapshotData = tokio::fs::File;
    type SnapshotBuilder = SQLiteSnapshotBuilder;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogIdOf<TypeConfigSqlite>>,
            StoredMembershipOf<TypeConfigSqlite>,
        ),
        io::Error,
    > {
        let (ack, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::MetadataRead(ack))
            .await
            .map_err(|err| io::Error::other(format!("sql writer channel closed: {err}")))?;
        let data = rx.await.expect("To always get Metadata from DB");

        debug!("applied_state: {:?}", data);

        Ok((data.last_applied_log_id, data.last_membership))
    }

    async fn apply<Strm>(&mut self, mut entries: Strm) -> Result<(), io::Error>
    where
        Strm: Stream<Item = Result<EntryResponder<TypeConfigSqlite>, io::Error>>
            + Unpin
            + OptionalSend,
    {
        while let Some(item) = entries.next().await {
            let (entry, responder) = item?;

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
                        last_membership: StoredMembershipOf::<TypeConfigSqlite>::new(
                            Some(entry.log_id),
                            mem,
                        ),
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

            if let Some(responder) = responder {
                responder.send(resp);
            }
        }

        Ok(())
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

    // openraft 0.10 has no `begin_receiving_snapshot()` on the state machine anymore: the receiver
    // of a snapshot is created by the network, and the data arrives through `install_snapshot()`.
    #[tracing::instrument(level = "trace", skip(self, _snapshot))]
    async fn install_snapshot(
        &mut self,
        _meta: &SnapshotMetaOf<TypeConfigSqlite>,
        _snapshot: Self::SnapshotData,
    ) -> Result<(), io::Error> {
        let src = format!("{}/temp", self.path_snapshots);
        // openraft 0.10 gives us no snapshot id to name the file with, so we pick our own.
        let dest = format!("{}/{}", self.path_snapshots, Uuid::now_v7());

        validate_db_backup_snapshot(src.clone()).await?;

        // Restore from the received file before publishing it. If the restore fails, no new
        // snapshot exists under a valid ID, so a restart cannot select a snapshot this node's
        // database was never restored from. Publishing first would leave exactly that behind.
        self.update_state_machine_(src.clone()).await?;

        atomic_file_switch(src, &dest).await?;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<SnapshotOf<TypeConfigSqlite, Self::SnapshotData>>, io::Error> {
        // `read_current_snapshot` only ever tries to read the snapshot with the highest ID, which
        // is against the trait contract by definition. However, we only ever store a single
        // snapshot anyway, so there is no need to even look for other ones. All snapshots expect
        // the latest one are being cleaned up pretty much directly.
        match self.read_current_snapshot().await? {
            None => Ok(None),
            Some(snap) => {
                let file = fs::File::open(&snap.path).await?;

                Ok(Some(Snapshot {
                    meta: snap.meta,
                    snapshot: file,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The read actors' handles are opened read-only, so SQLite itself is what keeps them from
    /// writing - not only the `query_only` pragma.  `PRAGMA optimize` is the one pragma of the
    /// shared set that writes into the database, and a read-only handle refuses it, which is why
    /// `apply_pragmas` skips it for a read connection.
    #[test]
    fn a_read_connection_is_read_only_at_the_file_layer() {
        let dir =
            std::env::temp_dir().join(format!("hiqlite_read_only_conn_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("read_only.db");

        let writer = rusqlite::Connection::open(&db).unwrap();
        StateMachineSqlite::apply_pragmas(&writer, false, 8).unwrap();
        writer.execute("CREATE TABLE t (a INTEGER)", ()).unwrap();
        writer.execute("INSERT INTO t VALUES (1)", ()).unwrap();

        let reader =
            rusqlite::Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        StateMachineSqlite::apply_pragmas(&reader, true, 8).unwrap();

        let n: i64 = reader
            .query_row("SELECT count(a) FROM t", (), |row| row.get(0))
            .unwrap();
        assert_eq!(n, 1, "a read-only handle must read the writer's data");

        let err = reader.execute("INSERT INTO t VALUES (2)", ()).unwrap_err();
        let text = err.to_string().to_lowercase();
        assert!(
            text.contains("readonly") || text.contains("read only"),
            "a read-only handle must refuse a write: {err}"
        );

        assert!(
            reader.pragma_update(None, "optimize", "0x10002").is_err(),
            "PRAGMA optimize writes ANALYZE stats, so a read-only handle must refuse it"
        );

        drop(reader);
        drop(writer);

        // The shape a restore from a backup file leaves behind: a database that is not in WAL.
        // A read connection opens it and reads it without ever trying to change the file - the
        // journal mode is the writer's to set, and a read-only handle refuses it.
        let restored = dir.join("restored.db");
        let writer = rusqlite::Connection::open(&restored).unwrap();
        writer
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
        writer.execute("CREATE TABLE t (a INTEGER)", ()).unwrap();
        writer.execute("INSERT INTO t VALUES (7)", ()).unwrap();
        drop(writer);

        let reader =
            rusqlite::Connection::open_with_flags(&restored, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        StateMachineSqlite::apply_pragmas(&reader, true, 8).unwrap();
        let a: i64 = reader
            .query_row("SELECT a FROM t", (), |row| row.get(0))
            .unwrap();
        assert_eq!(
            a, 7,
            "a read connection must read a restored database that is not in WAL"
        );

        drop(reader);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
///
/// Only the append golden is a storage guarantee and must never move - a diff there means existing
/// data dirs can't be kept and a data migration is needed. The vote and snapshot goldens pin the
/// current shape after the accepted wire break of 0.10 (`VoteRequest` gained
/// `leadership_transfer_from`), which is what stops 0.9 and 0.10 nodes from talking to each other.
#[cfg(test)]
mod raft_wire_goldens {
    use super::{Query, QueryWrite, TypeConfigSqlite};
    use crate::Node;
    use crate::migration::Migration;
    use crate::network::raft_server::{RaftStreamRequest, SnapshotWireRequest};
    use crate::network::{TypeConfigWire, WireSnapshotMeta, WireVote};
    use crate::store::state_machine::sqlite::param::Param;
    use std::borrow::Cow;
    use std::collections::BTreeMap;

    use openraft::impls::leader_id_adv::LeaderId;
    use openraft::raft::{AppendEntriesRequest, VoteRequest};
    use openraft::type_config::alias::{
        EntryOf, EntryPayloadOf, LogIdOf, MembershipOf, SnapshotMetaOf, StoredMembershipOf, VoteOf,
    };

    // Pinned golden references (see module docs). Regenerate with:
    //   cargo test -p hiqlite --offline --lib raft_wire_goldens::dump_wire_goldens -- --nocapture
    const GOLDEN_APPEND_DB: &str = "AAAAAAcAAAAAAAAABQAAAAAAAAABAAAAAAAAAAEBBAAAAAAAAAAAAAAAAAAAAAoAAAAAAAAACQAAAAAAAAAFAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAAAAABQAAAAAAAAABAAAAAAAAAAEAAAAAAAAAAQAAAAAAAAArAAAAAAAAAElOU0VSVCBJTlRPIHQgKGEsIGIsIGMpIFZBTFVFUyAoPzEsID8yLCA/MykHAAAAAAAAAAAAAAABAAAA/////////38CAAAAAAAAAAAA+L8DAAAACgAAAAAAAAB0ZXh0IHBhcmFtBAAAAAQAAAAAAAAAAAECAwUAAAACAAAAAAAAAAMAAAAAAAAABgAAAAEAAAAAAAAAAwAAAAAAAABjb2wFAAAAAAAAAAEAAAAAAAAAAgAAAAAAAAABAAAAAQAAACsAAAAAAAAASU5TRVJUIElOVE8gdCAoYSwgYiwgYykgVkFMVUVTICg/MSwgPzIsID8zKQcAAAAAAAAAAAAAAAEAAAD/////////fwIAAAAAAAAAAAD4vwMAAAAKAAAAAAAAAHRleHQgcGFyYW0EAAAABAAAAAAAAAAAAQIDBQAAAAIAAAAAAAAAAwAAAAAAAAAGAAAAAQAAAAAAAAADAAAAAAAAAGNvbAUAAAAAAAAAAQAAAAAAAAADAAAAAAAAAAEAAAACAAAAAgAAAAAAAAArAAAAAAAAAElOU0VSVCBJTlRPIHQgKGEsIGIsIGMpIFZBTFVFUyAoPzEsID8yLCA/MykHAAAAAAAAAAAAAAABAAAA/////////38CAAAAAAAAAAAA+L8DAAAACgAAAAAAAAB0ZXh0IHBhcmFtBAAAAAQAAAAAAAAAAAECAwUAAAACAAAAAAAAAAMAAAAAAAAABgAAAAEAAAAAAAAAAwAAAAAAAABjb2wrAAAAAAAAAElOU0VSVCBJTlRPIHQgKGEsIGIsIGMpIFZBTFVFUyAoPzEsID8yLCA/MykHAAAAAAAAAAAAAAABAAAA/////////38CAAAAAAAAAAAA+L8DAAAACgAAAAAAAAB0ZXh0IHBhcmFtBAAAAAQAAAAAAAAAAAECAwUAAAACAAAAAAAAAAMAAAAAAAAABgAAAAEAAAAAAAAAAwAAAAAAAABjb2wFAAAAAAAAAAEAAAAAAAAABAAAAAAAAAABAAAAAwAAABcAAAAAAAAAQkVHSU47IFNFTEVDVCAxOyBDT01NSVQFAAAAAAAAAAEAAAAAAAAABQAAAAAAAAABAAAABAAAAAEAAAAAAAAAAQAAAAgAAAAAAAAAY3JlYXRlX3QCAAAAAAAAAGFiGgAAAAAAAABDUkVBVEUgVEFCTEUgdCAoYSBJTlRFR0VSKQUAAAAAAAAAAQAAAAAAAAAGAAAAAAAAAAEAAAAFAAAAAQAAAAAAAAAAAAAAAAAAAAUAAAAAAAAAAQAAAAAAAAAHAAAAAAAAAAEAAAAGAAAABQAAAAAAAAABAAAAAAAAAGMAAAAAAAAAAgAAAAEAAAAAAAAAAgAAAAAAAAABAAAAAAAAAAIAAAAAAAAAAgAAAAAAAAABAAAAAAAAAAEAAAAAAAAADgAAAAAAAAAxMjcuMC4wLjE6MzA4MQ4AAAAAAAAAMTI3LjAuMC4xOjMwODACAAAAAAAAAAIAAAAAAAAADgAAAAAAAAAxMjcuMC4wLjE6NDA4MQ4AAAAAAAAAMTI3LjAuMC4xOjQwODABBQAAAAAAAAABAAAAAAAAAAkAAAAAAAAA";
    const GOLDEN_VOTE_DB: &str =
        "AQAAAAgAAAAAAAAAAwAAAAAAAAACAAAAAAAAAAABAgAAAAAAAAABAAAAAAAAAAUAAAAAAAAAAA==";
    const GOLDEN_SNAPSHOT_DB: &str = "AgAAAAkAAAAAAAAABgAAAAAAAAABAAAAAAAAAAABBgAAAAAAAAABAAAAAAAAACoAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGwAAAAAAAABzbmFwc2hvdC1wYXlsb2FkLTAxMjM0NTY3ODkBAQ==";

    /// Minimal RFC 4648 standard-alphabet base64 encoder (with padding). No external dep.
    fn b64_encode(data: &[u8]) -> String {
        const ALPHA: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
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
                out.push(ALPHA[(b2 & 0b11_1111) as usize] as char);
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

        let mut entries: Vec<EntryOf<TypeConfigSqlite>> = vec![];
        // EntryPayload::Blank
        entries.push(EntryOf::<TypeConfigSqlite> {
            log_id: LogIdOf::<TypeConfigSqlite> {
                leader_id: LeaderId {
                    term: 5u64,
                    node_id: 1u64,
                },
                index: 0u64,
            },
            payload: EntryPayloadOf::<TypeConfigSqlite>::Blank,
        });
        // EntryPayload::Normal carrying every QueryWrite variant.
        for (i, qw) in data_variants.into_iter().enumerate() {
            entries.push(EntryOf::<TypeConfigSqlite> {
                log_id: LogIdOf::<TypeConfigSqlite> {
                    leader_id: LeaderId {
                        term: 5u64,
                        node_id: 1u64,
                    },
                    index: i as u64 + 1,
                },
                payload: EntryPayloadOf::<TypeConfigSqlite>::Normal(qw),
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
        let membership = MembershipOf::<TypeConfigSqlite>::new(
            vec![std::collections::BTreeSet::from_iter([1u64, 2u64])],
            nodes,
        )
        .unwrap();
        entries.push(EntryOf::<TypeConfigSqlite> {
            log_id: LogIdOf::<TypeConfigSqlite> {
                leader_id: LeaderId {
                    term: 5u64,
                    node_id: 1u64,
                },
                index: 99u64,
            },
            payload: EntryPayloadOf::<TypeConfigSqlite>::Membership(membership),
        });

        RaftStreamRequest::AppendDB((
            7usize,
            AppendEntriesRequest::<TypeConfigSqlite> {
                vote: VoteOf::<TypeConfigSqlite> {
                    leader_id: LeaderId {
                        term: 5u64,
                        node_id: 1u64,
                    },
                    committed: true,
                },
                prev_log_id: Some(LogIdOf::<TypeConfigSqlite> {
                    leader_id: LeaderId {
                        term: 4u64,
                        node_id: 0u64,
                    },
                    index: 10u64,
                }),
                entries,
                leader_commit: Some(LogIdOf::<TypeConfigSqlite> {
                    leader_id: LeaderId {
                        term: 5u64,
                        node_id: 1u64,
                    },
                    index: 9u64,
                }),
            },
        ))
    }

    fn build_vote_db() -> RaftStreamRequest {
        RaftStreamRequest::VoteDB((
            8usize,
            VoteRequest::<TypeConfigSqlite> {
                vote: VoteOf::<TypeConfigSqlite> {
                    leader_id: LeaderId {
                        term: 3u64,
                        node_id: 2u64,
                    },
                    committed: false,
                },
                last_log_id: Some(LogIdOf::<TypeConfigSqlite> {
                    leader_id: LeaderId {
                        term: 2u64,
                        node_id: 1u64,
                    },
                    index: 5u64,
                }),
                leadership_transfer_from: None,
            },
        ))
    }

    fn build_snapshot_db() -> RaftStreamRequest {
        RaftStreamRequest::SnapshotDB(SnapshotWireRequest {
            request_id: 9usize,
            vote: WireVote {
                leader_id: LeaderId {
                    term: 6u64,
                    node_id: 1u64,
                },
                committed: false,
            },
            meta: WireSnapshotMeta {
                last_log_id: Some(LogIdOf::<TypeConfigWire> {
                    leader_id: LeaderId {
                        term: 6u64,
                        node_id: 1u64,
                    },
                    index: 42u64,
                }),
                last_membership: StoredMembershipOf::<TypeConfigWire>::new(
                    None,
                    MembershipOf::<TypeConfigWire>::new(
                        vec![std::collections::BTreeSet::new()],
                        BTreeMap::<u64, Node>::new(),
                    )
                    .unwrap(),
                ),
            },
            data: b"snapshot-payload-0123456789".to_vec(),
            first: true,
            last: true,
        })
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
