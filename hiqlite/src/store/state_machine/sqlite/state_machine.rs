use crate::migration::Migration;
use crate::store::state_machine::sqlite::param::Param;
use crate::store::state_machine::sqlite::snapshot_builder::SQLiteSnapshotBuilder;
use crate::store::state_machine::sqlite::writer::{
    self, MetaPersistRequest, SqlBatch, SqlTransaction, WriterRequest,
};
use crate::store::state_machine::sqlite::{reader, TypeConfigSqlite};
use crate::store::{logs, StorageResult};
use crate::{Error, Node, NodeId};
use openraft::storage::RaftStateMachine;
use openraft::{
    EntryPayload, LogId, OptionalSend, Snapshot, SnapshotId, SnapshotMeta, StorageError,
    StorageIOError, StoredMembership,
};
use rusqlite::{OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio::{fs, task, time};
use tracing::{debug, error, warn};
use uuid::Uuid;

type Entry = openraft::Entry<TypeConfigSqlite>;
type SnapshotData = tokio::fs::File;

pub type SqlitePool = deadpool::unmanaged::Pool<rusqlite::Connection>;

pub type Params = Vec<Param>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryWrite {
    Execute(Query),
    Transaction(Vec<Query>),
    Batch(Cow<'static, str>),
    Migration(Vec<Migration>),
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
    Transaction(Result<Vec<Result<usize, Error>>, Error>),
    Batch(ResponseBatch),
    Migrate(Result<(), Error>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseExecute {
    pub result: Result<usize, Error>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBatch {
    pub result: Vec<Result<usize, Error>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSnapshot {
    pub meta: SnapshotMeta<NodeId, Node>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineData {
    pub last_applied_log_id: Option<LogId<NodeId>>,
    pub last_membership: StoredMembership<NodeId, Node>,
    pub last_snapshot_id: Option<String>,
    pub last_snapshot_path: Option<String>,
}

impl Default for StateMachineData {
    fn default() -> Self {
        Self {
            last_applied_log_id: None,
            last_membership: Default::default(),
            last_snapshot_id: None,
            last_snapshot_path: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateMachineSqlite {
    pub data: StateMachineData,

    this_node: NodeId,
    path_snapshots: String,
    path_lock_file: String,

    pub read_pool: Arc<SqlitePool>,
    pub(crate) write_tx: flume::Sender<WriterRequest>,
}

impl Drop for StateMachineSqlite {
    fn drop(&mut self) {
        // TODO send metadata persist action to writer
        let (ack, rx) = flume::unbounded();
        self.write_tx
            .send(WriterRequest::MetadataPersist(MetaPersistRequest {
                data: bincode::serialize(&self.data).unwrap(),
                ack,
            }))
            .unwrap();
        rx.recv().unwrap();

        Self::remove_lock_file(&self.path_lock_file);
        // self.handle.abort();
    }
}

impl StateMachineSqlite {
    pub(crate) async fn new(
        data_dir: Cow<'static, str>,
        filename_db: Cow<'static, str>,
        this_node: NodeId,
    ) -> Result<StateMachineSqlite, StorageError<NodeId>> {
        let path_base = format!("{}/state_machine", data_dir);

        let path_snapshots = format!("{}/snapshots", path_base);
        fs::create_dir_all(&path_snapshots)
            .await
            .expect("Cannot create snapshots path");

        let path_db = format!("{}/db", path_base);
        let path_lock_file = format!("{}/lock", path_base);
        let mut db_exists = fs::File::open(format!("{}/{}", path_db, filename_db))
            .await
            .is_ok();
        Self::check_set_lock_file(&path_lock_file, &path_db, &mut db_exists).await;

        if !db_exists {
            // may error if is has been re-created already in auto-rebuild from lockfile
            let _ = fs::create_dir_all(&path_db).await;
        }

        // Always start the writer first
        let conn = Self::connect(path_db.to_string(), filename_db.to_string(), false)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;
        let write_tx = writer::spawn_writer(conn);

        let read_pool = Self::connect_read_pool(path_db.as_ref(), filename_db.as_ref())
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;

        // only try to fetch the data from the DB if it actually existed beforehand
        let state_machine_data: StateMachineData = if db_exists {
            let (ack, rx) = oneshot::channel();
            write_tx
                .send_async(WriterRequest::MetadataRead(ack))
                .await
                .map_err(|err| StorageError::IO {
                    source: StorageIOError::read(&err),
                })?;
            let bytes = rx.await.expect("To always get Metadata from DB");

            bincode::deserialize(&bytes).unwrap()
        } else {
            let metadata = StateMachineData::default();
            let data = bincode::serialize(&metadata).unwrap();

            let (ack, rx) = flume::unbounded();
            write_tx
                .send_async(WriterRequest::MetadataPersist(MetaPersistRequest {
                    data,
                    ack,
                }))
                .await
                .map_err(|err| StorageError::IO {
                    source: StorageIOError::write(&err),
                })?;
            rx.recv_async().await.map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;

            metadata
        };

        let mut slf = Self {
            data: state_machine_data,
            this_node,
            path_snapshots,
            path_lock_file,
            read_pool: Arc::new(read_pool),
            write_tx,
        };

        // TODO only apply the latest snapshot if we do not have a DB yet?
        // TODO or just apply it all the time and therefore don't care about graceful shutdown for SQLite?
        if !db_exists {
            if let Some(snapshot) = slf.read_current_snapshot_from_disk().await? {
                slf.update_state_machine_(snapshot).await?;
            }
        }

        Ok(slf)
    }

    async fn check_set_lock_file(path_lock_file: &str, path_db: &str, db_exists: &mut bool) {
        let is_locked = fs::File::open(path_lock_file).await.is_ok();

        if is_locked {
            #[cfg(feature = "auto-heal")]
            {
                warn!(
                    "Lock file already exists: {}\n\
                    Node did not shut down gracefully - auto-rebuilding State Machine",
                    path_lock_file
                );

                // if we can't create the lock file, we will delete the current state machine
                // data so it can be rebuilt.
                // TODO is it enough to delete DB only, or do we need to do a full wipe?
                let _ = fs::remove_dir_all(path_db).await;

                // re-create the DB folder
                if let Err(err) = fs::create_dir_all(path_db).await {
                    panic!("Cannot re-create DB folder {}: {}", path_db, err);
                }

                *db_exists = false;

                // // try again but panic if it fails this time
                // if let Err(err) = fs::File::create(path_lock_file).await {
                //     panic!("Cannot create lock file {}: {}", path_lock_file, err);
                // }
            }

            #[cfg(not(feature = "auto-heal"))]
            panic!(
                "Lock file already exists: {}\n\
                Node did not shut down gracefully - needs manual interaction",
                path_lock_file
            );
        } else if let Err(err) = fs::File::create(path_lock_file).await {
            panic!("Error creating lock file {}: {}", path_lock_file, err);
        }

        // if let Err(err) = fs::File::create(path_lock_file).await {
        //     #[cfg(feature = "auto-rebuild")]
        //     {
        //         error!(
        //             "Lock file already exists: {}\n\
        //             Node did not shut down gracefully - auto-rebuilding State Machine",
        //             path_lock_file
        //         );
        //
        //         // if we can't create the lock file, we will delete the current state machine
        //         // data so it can be rebuilt.
        //         // TODO is it enough to delete DB only, or do we need to do a full wipe?
        //         let _ = fs::remove_dir_all(path_db).await;
        //
        //         // re-create the DB folder
        //         if let Err(err) = fs::create_dir_all(path_db).await {
        //             panic!("Cannot re-create DB folder {}: {}", path_db, err);
        //         }
        //
        //         // // try again but panic if it fails this time
        //         // if let Err(err) = fs::File::create(path_lock_file).await {
        //         //     panic!("Cannot create lock file {}: {}", path_lock_file, err);
        //         // }
        //     }
        //
        //     #[cfg(not(feature = "auto-rebuild"))]
        //     panic!(
        //         "Lock file already exists: {}\n\
        //         Node did not shut down gracefully - needs manual interaction",
        //         path_lock_file
        //     );
        // }
    }

    fn remove_lock_file(path: &str) {
        std::fs::remove_file(path).expect("The lock file to always exist when the node is running");
    }

    pub async fn connect(
        path: String,
        filename_db: String,
        read_only: bool,
    ) -> Result<rusqlite::Connection, Error> {
        task::spawn_blocking(move || {
            // let conn = if in_memory {
            //     // Self::apply_pragmas(&conn, read_only)?;
            //     let mut flags = OpenFlags::default();
            //     flags.set(OpenFlags::from_name("SQLITE_OPEN_URI").unwrap(), true);
            //     flags.set(OpenFlags::from_name("SQLITE_OPEN_MEMORY").unwrap(), true);
            //     flags.set(
            //         OpenFlags::from_name("SQLITE_OPEN_SHARED_CACHE").unwrap(),
            //         true,
            //     );
            //
            //     // TODO in memory does not work this way - will create a file with that name
            //     // TODO OpenFlag are fine here but do not exist for deadpool
            //     // let addr = path.unwrap_or(IN_MEMORY_ADDR);
            //     // let conn = rusqlite::Connection::open(path)?;
            //     // Self::apply_pragmas(&conn, false)?;
            //
            //     let path_full = format!("file:{}", filename_db);
            //     // let path_full = format!("file:{}?mode=memory&cache=shared", filename_db);
            //
            //     // TODO set db name to URI and try to open shared DB
            //     rusqlite::Connection::open_with_flags(path_full, flags)?
            //     // let conn = rusqlite::Connection::open(filename_db)?;
            // } else {
            let path_full = format!("{}/{}", path, filename_db);
            let conn = rusqlite::Connection::open(path_full)?;
            // };

            Self::apply_pragmas(&conn, read_only)?;

            Ok(conn)
        })
        .await?
    }

    async fn connect_read_pool(
        path: &str,
        filename_db: &str,
        // in_memory: bool,
    ) -> Result<SqlitePool, Error> {
        // ) -> Result<deadpool_sqlite::Pool, ApiError> {
        let path_full = format!("{}/{}", path, filename_db);
        // let config = deadpool_sqlite::Config::new(path_full);

        let amount = 4;
        let mut conns = Vec::with_capacity(amount);
        for _ in 0..amount {
            let mut conn = Self::connect(path.to_string(), filename_db.to_string(), true).await;
            while conn.is_err() {
                time::sleep(Duration::from_millis(10)).await;
                conn = Self::connect(path.to_string(), filename_db.to_string(), true).await;
            }
            conns.push(conn.unwrap());
        }

        let pool = deadpool::unmanaged::Pool::from(conns);

        // let pool = config
        //     .builder(deadpool_sqlite::Runtime::Tokio1)
        //     .unwrap()
        //     .post_create(deadpool_sqlite::Hook::async_fn(
        //         |conn: &mut deadpool_sync::SyncWrapper<rusqlite::Connection>, _| {
        //             Box::pin(async move {
        //                 conn.interact(|conn| {
        //                     Self::apply_pragmas(conn, true).map_err(|_| InteractError::Aborted)
        //                 })
        //                 .await
        //                 .map_err(|err| deadpool_sqlite::HookError::Message(err.to_string().into()))?
        //                 .map_err(|err| {
        //                     deadpool_sqlite::HookError::Message(err.to_string().into())
        //                 })?;
        //                 Ok(())
        //             })
        //         },
        //     ))
        //     .build()?;

        let conn = pool.get().await?;
        task::spawn_blocking(move || {
            let _ = conn.query_row("SELECT 1", (), |row| {
                let res: i64 = row.get(0)?;
                Ok(res)
            })?;
            Ok::<(), Error>(())
        })
        .await?;

        // conn.interact(|conn| {
        //     conn.query_row("SELECT 1", (), |row| {
        //         let res: i64 = row.get(0)?;
        //         Ok(res)
        //     })
        // })
        // .await??;

        Ok(pool)
    }

    fn apply_pragmas(conn: &rusqlite::Connection, read_only: bool) -> Result<(), rusqlite::Error> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "journal_size_limit", 16384)?;
        conn.pragma_update(None, "wal_autocheckpoint", 4_000)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // conn.pragma_update(None, "busy_timeout", "5000")?;
        conn.pragma_update(None, "temp_store", "memory")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;

        // TODO maybe add a 'paranoid' level to sync absolutely everything all the time
        // TODO req / s would go down to ~300 / s for a single thread though
        // conn.pragma_update(None, "synchronous", "FULL")?;

        // the default is 4096, but increasing makes sense if you write bigger rows
        // conn.pragma_update(None, "page_size", 4096).unwrap();

        // if set, it will try to keep the whole DB cached in memory, if it fits
        // not set currently for better comparison to sqlx
        // conn.pragma_update(None, "mmap_size", "30000000000")
        //     .unwrap();

        // only allow select statements
        if read_only {
            conn.pragma_update(None, "query_only", true)?;
        } else {
            // conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        }

        // TODO make this configurable
        conn.set_prepared_statement_cache_capacity(1024);

        Ok(())
    }

    async fn update_state_machine_(
        &mut self,
        snapshot: StoredSnapshot,
    ) -> Result<(), StorageError<NodeId>> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::SnapshotApply((snapshot.path, tx)))
            .await
            .expect("SQLite Writer rx to always be listening");

        self.data = rx.await.expect("Snapshot installation to succeed");

        Ok(())
    }

    async fn read_current_snapshot_from_disk(&self) -> StorageResult<Option<StoredSnapshot>> {
        let mut list = tokio::fs::read_dir(&self.path_snapshots)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;

        let mut snapshot_id: Option<Uuid> = None;
        while let Ok(Some(entry)) = list.next_entry().await {
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

            // we only expect sub-dirs in the snapshot dir
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
        // let path_db = format!("{}/data", path_snapshot);
        // let path_meta = format!("{}/meta", path_snapshot);

        // let meta_bytes = fs::read(path_meta).await.map_err(|err| StorageError::IO {
        //     source: StorageIOError::read(&err),
        // })?;
        // let meta = bincode::deserialize(&meta_bytes).unwrap();

        let db_path = self.path_snapshots.clone();
        let filename_db = id.to_string();

        // open a DB connection to read out the metadata
        let conn = Self::connect(db_path, filename_db, false)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;

        let metadata = task::spawn_blocking(move || {
            let mut stmt = conn
                .prepare("SELECT data FROM _metadata WHERE key = 'meta'")
                .map_err(|err| StorageError::IO {
                    source: StorageIOError::write(&err),
                })?;
            let metadata = stmt
                .query_row((), |row| {
                    // let (snapshot_id, last_membership) = stmt.query_map((), |row| {
                    // let id_bytes: Vec<u8> = row.get(0).unwrap();
                    // let id = Uuid::from_slice(&id_bytes).unwrap();
                    // let metadata_bytes: Vec<u8> = row.get(2).unwrap();
                    // let metadata: SnapshotMeta<NodeId, Node> =
                    //     bincode::deserialize(&metadata_bytes).unwrap();
                    // Ok((id, metadata))
                    let meta_bytes: Vec<u8> = row.get(0)?;
                    let metadata: StateMachineData =
                        bincode::deserialize(&meta_bytes).expect("Metadata to deserialize ok");
                    Ok(metadata)
                })
                .map_err(|err| StorageError::IO {
                    source: StorageIOError::write(&err),
                })?;

            Ok::<StateMachineData, StorageError<NodeId>>(metadata)
            // Ok::<std::option::Option<(Uuid, SnapshotMeta<u64, Node>)>, StorageError<NodeId>>(res)
        })
        .await
        .map_err(|err| StorageError::IO {
            source: StorageIOError::write(&err),
        })??;

        if let Some(path) = metadata.last_snapshot_path {
            let meta = SnapshotMeta {
                last_log_id: metadata.last_applied_log_id,
                last_membership: metadata.last_membership,
                snapshot_id: metadata.last_snapshot_id.unwrap(),
            };
            let snapshot = StoredSnapshot { meta, path };
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }

        // match res {
        //     None => Ok(None),
        //     Some((id, meta)) => {
        // let snapshot = StoredSnapshot {
        //     meta,
        //     path: format!("{}/{}", self.path_snapshots, metadata),
        // };
        // Ok(Some(snapshot))
        //     }
        // }
    }

    async fn get_current_snapshot_(&self) -> StorageResult<Option<StoredSnapshot>> {
        if let Some(snapshot_id) = self.data.last_snapshot_id.clone() {
            Ok(Some(StoredSnapshot {
                meta: SnapshotMeta {
                    last_log_id: self.data.last_applied_log_id,
                    last_membership: self.data.last_membership.clone(),
                    snapshot_id,
                },
                path: self
                    .data
                    .last_snapshot_path
                    .clone()
                    .expect("last_snapshot_path to always be Some when snapshot_id exists"),
            }))
        } else {
            Ok(None)
        }
    }

    // async fn persist_snapshot(&self, snapshot: StoredSnapshot) -> StorageResult<()> {
    //     // let path_new = format!("{}/{}", self.path_snapshots, snapshot.meta.snapshot_id);
    //     fs::create_dir_all(&self.path_snapshots).await.map_err(|err| StorageError::IO {
    //         source: StorageIOError::write(&err),
    //     })?;
    //
    //     // let path_meta = format!("{}/meta", path_new);;
    //     // let meta_bytes = bincode::serialize(&snapshot.meta).unwrap();
    //     // fs::write(path_meta, meta_bytes).await.map_err(|err| StorageError::IO { source: StorageIOError::read(&err) })?;
    //
    //     // TODO fix src path
    //     let src = snapshot.meta.snapshot_id;
    //     let tar = format!("{}/data", path_new);;
    //     fs::copy(&src, tar).await.map_err(|err| StorageError::IO { source: StorageIOError::write(&err) })?;
    //
    //     fs::remove_dir_all(src).await.map_err(|err| StorageError::IO { source: StorageIOError::write(&err) })?;
    //
    //     Ok(())
    // }

    // async fn spawn_write_handler(conn: rusqlite::Connection) -> flume::Sender<Query> {
    //     let (tx, rx) = flume::bounded::<Query>(2);
    //
    //     let _handle = std::thread::spawn(move || loop {
    //     // let _handle = task::spawn_blocking(move || loop {
    //         let query = match rx.recv() {
    //             Ok(q) => q,
    //             Err(err) => {
    //                 error!("SQLite write handler channel recv error: {:?}", err);
    //                 continue;
    //             }
    //         };
    //         debug!("Query in Write handler: {:?}", query);
    //
    //         match query {
    //             Query::Execute(q) => {
    //                 let res = {
    //                     let mut stmt = match conn.prepare_cached(q.sql.as_ref()) {
    //                         Ok(stmt) => stmt,
    //                         Err(err) => {
    //                             error!("Preparing cached query {}: {:?}", q.sql, err);
    //                             return Err(SqlError::ExecuteReturnedResults);
    //                         }
    //                     };
    //
    //                     let params_len = q.params.len();
    //                     for i in 0..params_len {
    //                         let value = q
    //                             .params
    //                             .get(i)
    //                             .expect("bounded params.get() should never panic");
    //                         if let Err(err) = stmt.raw_bind_parameter(i + 1, value) {
    //                             error!(
    //                                 "Error binding param {} to query {}: {:?}",
    //                                 value, q.sql, err
    //                             );
    //                             return Err(SqlError::InvalidQuery);
    //                         }
    //                     }
    //
    //                     stmt.raw_execute()
    //                 };
    //
    //                 q.tx.send(res).expect("oneshot tx to never be dropped");
    //             }
    //         }
    //     });
    //
    //     tx
    // }
}

impl RaftStateMachine<TypeConfigSqlite> for StateMachineSqlite {
    // type SnapshotBuilder = Self;
    type SnapshotBuilder = SQLiteSnapshotBuilder;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, Node>), StorageError<NodeId>> {
        Ok((
            self.data.last_applied_log_id,
            self.data.last_membership.clone(),
        ))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let entries = entries.into_iter();
        let mut replies = Vec::with_capacity(entries.size_hint().0);

        for entry in entries {
            // TODO probably should be moved after disk IO -> persist safely for crash resistance!
            let resp = match entry.payload {
                EntryPayload::Blank => Response::Empty,

                EntryPayload::Normal(QueryWrite::Execute(Query { sql, params })) => {
                    let (tx, rx) = oneshot::channel();
                    let query = writer::Query::Execute(writer::SqlExecute {
                        sql,
                        params,
                        last_applied_log_id: self.data.last_applied_log_id,
                        tx,
                    });

                    self.write_tx
                        .send_async(WriterRequest::Query(query))
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx
                        .await
                        .expect("to always get a response from sql writer")
                        .map_err(Error::from);
                    Response::Execute(ResponseExecute { result })
                }

                EntryPayload::Normal(QueryWrite::Transaction(queries)) => {
                    let (tx, rx) = oneshot::channel();
                    let req = WriterRequest::Query(writer::Query::Transaction(SqlTransaction {
                        queries,
                        last_applied_log_id: self.data.last_applied_log_id,
                        tx,
                    }));

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");

                    let resp = match result {
                        Ok(res) => {
                            let mapped = res
                                .into_iter()
                                .map(|res| res.map_err(Error::from))
                                .collect();
                            Ok(mapped)
                        }
                        Err(err) => Err(err),
                    };

                    Response::Transaction(resp)
                }

                EntryPayload::Normal(QueryWrite::Batch(sql)) => {
                    let (tx, rx) = oneshot::channel();
                    let req = WriterRequest::Query(writer::Query::Batch(SqlBatch {
                        sql,
                        last_applied_log_id: self.data.last_applied_log_id,
                        tx,
                    }));

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");
                    Response::Batch(ResponseBatch { result })
                }

                EntryPayload::Membership(mem) => {
                    self.data.last_membership = StoredMembership::new(Some(entry.log_id), mem);
                    Response::Empty
                }

                EntryPayload::Normal(QueryWrite::Migration(migrations)) => {
                    let (tx, rx) = oneshot::channel();
                    let req = WriterRequest::Migrate(writer::Migrate {
                        migrations,
                        last_applied_log_id: self.data.last_applied_log_id,
                        tx,
                    });

                    self.write_tx
                        .send_async(req)
                        .await
                        .expect("sql writer to always be listening");

                    let result = rx.await.expect("to always get a response from sql writer");
                    Response::Migrate(result)
                }

                EntryPayload::Membership(mem) => {
                    self.data.last_membership = StoredMembership::new(Some(entry.log_id), mem);
                    Response::Empty
                }
            };

            replies.push(resp);
            self.data.last_applied_log_id = Some(entry.log_id);
        }

        Ok(replies)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        SQLiteSnapshotBuilder {
            last_membership: self.data.last_membership.clone(),
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
        // tokio file handle
        // TODO at this step, it should always be the `/temp` file -> make 100% sure
        _snapshot: Box<SnapshotData>,
    ) -> Result<(), StorageError<NodeId>> {
        // TODO extract file path from snapshot data
        // let new_snapshot = StoredSnapshot {
        //     meta: meta.clone(),
        //     // path: ,
        // };

        // self.update_state_machine_(new_snapshot.clone()).await?;

        // self.persist_snapshot(new_snapshot, snapshot)?;

        // fs::create_dir_all(&self.path_snapshots)
        //     .await
        //     .map_err(|err| StorageError::IO {
        //         source: StorageIOError::write(&err),
        //     })?;

        // let path_meta = format!("{}/meta", path_new);;
        // let meta_bytes = bincode::serialize(&snapshot.meta).unwrap();
        // fs::write(path_meta, meta_bytes).await.map_err(|err| StorageError::IO { source: StorageIOError::read(&err) })?;

        // let (ack, rx) = oneshot::channel();
        // let src = format!("{}/temp", self.path_snapshots);
        // self.write_tx
        //     .send_async(WriterRequest::SnapshotApply(src, ack))
        //     .await
        //     .map_err(|err| StorageError::IO {
        //         source: StorageIOError::write(&err),
        //     })?;
        //
        // self.data = rx
        //     .await
        //     .expect("to always get a response from Snapshot Install");

        // // TODO fix src path
        let src = format!("{}/temp", self.path_snapshots);
        let tar = format!("{}/{}", self.path_snapshots, meta.snapshot_id);
        fs::copy(&src, &tar).await.map_err(|err| StorageError::IO {
            source: StorageIOError::write(&err),
        })?;

        fs::remove_file(src).await.map_err(|err| StorageError::IO {
            source: StorageIOError::write(&err),
        })?;

        self.update_state_machine_(StoredSnapshot {
            meta: meta.clone(),
            path: tar,
        })
        .await?;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigSqlite>>, StorageError<NodeId>> {
        let snap = self.get_current_snapshot_().await?;
        match snap {
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
