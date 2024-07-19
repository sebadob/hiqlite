#![allow(unused)]

use crate::{Error, NodeId};
use openraft::storage::RaftLogStorage;
use openraft::StorageError;
use rusqlite::OpenFlags;
use std::borrow::Cow;
use std::cmp::PartialEq;
// in-memory
// use crate::store::state_machine_memory::StateMachineStore;

// sqlite
use state_machine::sqlite::state_machine::StateMachineSqlite;

// pub mod state_machine_memory;
mod logs;
pub mod state_machine;

pub type StorageResult<T> = Result<T, StorageError<NodeId>>;

// REDB
// pub(crate) async fn new_storage(
//     db_path: Cow<'static, str>,
//     filename_db: Cow<'static, str>,
//     mode: NodeMode,
// ) -> (logs::LogStore, StateMachineStore) {
//     let log_store = logs::LogStore::new(db_path.as_ref()).await;
//
//     // let sm_store = state_machine_rocksdb::build_state_machine(db_path).await;
//     let memory_sqlite = mode != NodeMode::Disk;
//     let sm_store = StateMachineStore::new(db_path, filename_db, memory_sqlite)
//         .await
//         .unwrap();
//     (log_store, sm_store)
// }

// IN MEMORY
// TODO take optional in_memory value to be able to start multiple rafts in the end
// pub(crate) async fn new_storage(
//     db_path: &str,
//     filename_db: Option<&str>,
// ) -> (logs_memory::LogStore, StateMachineStore) {
//     // let log_store = LogStore::new(db_path).await;
//     let log_store = logs_memory::LogStore::new();
//
//     // let sm_store = state_machine_rocksdb::build_state_machine(db_path).await;
//     let sm_store = StateMachineStore::new(db_path, filename_db).await.unwrap();
//     (log_store, sm_store)
// }

// ROCKSDB
pub(crate) async fn new_storage(
    db_path: Cow<'static, str>,
    filename_db: Cow<'static, str>,
    // mode: NodeMode,
    // ) -> (T, StateMachineStore)
    // where
    //     T: RaftLogStorage<TypeConfigSqlite>,
    // {
) -> (logs::rocksdb::LogStoreRocksdb, StateMachineSqlite) {
    // let log_store = LogStore::new(db_path).await;

    // let log_store: T = match mode {
    //     NodeMode::Disk | NodeMode::Memory => logs_rocks::LogStore::new(&db_path).await,
    //     NodeMode::Ephemeral => logs_memory::LogStore::new(),
    // };

    // let memory_sqlite = mode != NodeMode::Disk;

    let log_store = logs::rocksdb::LogStoreRocksdb::new(&db_path).await;

    // let sm_store = state_machine_rocksdb::build_state_machine(db_path).await;
    let sm_store = StateMachineSqlite::new(db_path, filename_db).await.unwrap();
    (log_store, sm_store)
    // (log_store(db_path, mode).await, sm_store)
}

// async fn log_store<LS>(db_path: Cow<'static, str>, mode: NodeMode) -> LS
// where
//     LS: RaftLogStorage<TypeConfigSqlite>,
// {
//     match mode {
//         NodeMode::Disk | NodeMode::Memory => logs_rocks::LogStore::new(&db_path).await,
//         NodeMode::Ephemeral => logs_memory::LogStore::new(),
//     }
// }

// SQLITE
// pub(crate) async fn new_storage(
//     db_path: &str,
//     filename_db: Option<&str>,
// ) -> (logs_sqlite::LogStore, StateMachineStore) {
//     // let log_store = LogStore::new(db_path).await;
//     let log_store = logs_sqlite::LogStore::new(db_path, filename_db)
//         .await
//         .unwrap();
//
//     // let sm_store = state_machine_rocksdb::build_state_machine(db_path).await;
//     let sm_store = StateMachineStore::new(db_path, filename_db).await.unwrap();
//     (log_store, sm_store)
// }

#[cfg(feature = "sqlite")]
pub(crate) fn connect_sqlite(
    path: Option<&str>,
    read_only: bool,
) -> Result<rusqlite::Connection, Error> {
    let conn = if let Some(path) = path {
        let conn = rusqlite::Connection::open(path)?;
        apply_pragmas(&conn, read_only)?;
        conn
    } else {
        let mut flags = OpenFlags::default();
        flags.set(OpenFlags::from_name("SQLITE_OPEN_MEMORY").unwrap(), true);
        flags.set(
            OpenFlags::from_name("SQLITE_OPEN_SHARED_CACHE").unwrap(),
            true,
        );
        let conn = rusqlite::Connection::open_in_memory_with_flags(flags)?;
        apply_pragmas(&conn, read_only)?;
        conn
    };
    // TODO in memory does not work this way - will create a file with that name
    // TODO OpenFlag are fine here but do not exist for deadpool
    // let addr = path.unwrap_or(IN_MEMORY_ADDR);
    // let conn = rusqlite::Connection::open(path)?;
    // Self::apply_pragmas(&conn, false)?;

    Ok(conn)
}

#[cfg(feature = "sqlite")]
fn apply_pragmas(conn: &rusqlite::Connection, read_only: bool) -> Result<(), rusqlite::Error> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "journal_size_limit", 32768)?;
    // conn.pragma_update(None, "journal_size_limit", 16384)?;
    // conn.pragma_update(None, "wal_autocheckpoint", 100_000)?;
    conn.pragma_update(None, "wal_autocheckpoint", 10_000)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    // conn.pragma_update(None, "busy_timeout", "5000")?;
    conn.pragma_update(None, "temp_store", "memory")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;

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
