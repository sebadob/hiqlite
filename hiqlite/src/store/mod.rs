#![allow(unused)]

use crate::app_state::AppState;
use crate::network::NetworkStreaming;
use crate::{init, Error, NodeConfig, NodeId, RaftConfig};
use num_traits::ToPrimitive;
use openraft::storage::RaftLogStorage;
use openraft::{Raft, StorageError};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cmp::PartialEq;
use std::fmt::Debug;
use std::sync::Arc;
use strum::IntoEnumIterator;

#[cfg(feature = "cache")]
use crate::{
    app_state::StateRaftCache,
    store::state_machine::memory::{state_machine::StateMachineMemory, TypeConfigKV},
};

#[cfg(feature = "sqlite")]
use crate::{
    app_state::StateRaftDB,
    store::state_machine::sqlite::{
        state_machine::{SqlitePool, StateMachineSqlite},
        writer::WriterRequest,
        TypeConfigSqlite,
    },
};

pub mod logs;
pub mod state_machine;

pub type StorageResult<T> = Result<T, StorageError<NodeId>>;

#[cfg(feature = "sqlite")]
pub(crate) async fn start_raft_db(
    node_config: NodeConfig,
    raft_config: Arc<RaftConfig>,
) -> Result<StateRaftDB, Error> {
    let log_store = logs::rocksdb::LogStoreRocksdb::new(&node_config.data_dir).await;
    let state_machine_store = StateMachineSqlite::new(
        &node_config.data_dir,
        &node_config.filename_db,
        node_config.node_id,
        node_config.log_statements,
        #[cfg(feature = "s3")]
        node_config.s3_config,
    )
    .await
    .unwrap();

    let logs_writer = log_store.tx_writer.clone();
    let sql_writer = state_machine_store.write_tx.clone();
    let read_pool = state_machine_store.read_pool.clone();

    // Create the network layer that will connect and communicate the raft instances and
    // will be used in conjunction with the store created above.
    let network = NetworkStreaming {
        node_id: node_config.node_id,
        tls_config: node_config.tls_raft.as_ref().map(|tls| tls.client_config()),
        secret_raft: node_config.secret_raft.as_bytes().to_vec(),
    };

    // Create a local raft instance.
    let raft = openraft::Raft::new(
        node_config.node_id,
        raft_config.clone(),
        network,
        log_store,
        state_machine_store,
    )
    .await
    .expect("Raft create failed");

    init::init_pristine_node_1_db(
        &raft,
        node_config.node_id,
        &node_config.nodes,
        &node_config.secret_api,
        node_config.tls_api.is_some(),
        node_config
            .tls_api
            .as_ref()
            .map(|c| c.danger_tls_no_verify)
            .unwrap_or(false),
    )
    .await?;

    Ok(StateRaftDB {
        raft,
        lock: Default::default(),
        logs_writer,
        sql_writer,
        read_pool,
        log_statements: node_config.log_statements,
    })
}

#[cfg(feature = "cache")]
pub(crate) async fn start_raft_cache<C>(
    node_config: NodeConfig,
    raft_config: Arc<RaftConfig>,
) -> Result<StateRaftCache, Error>
where
    C: Debug + Serialize + for<'a> Deserialize<'a> + IntoEnumIterator + ToPrimitive,
{
    let log_store = logs::memory::LogStoreMemory::new();
    let state_machine_store = Arc::new(StateMachineMemory::new::<C>().await.unwrap());

    let network = NetworkStreaming {
        node_id: node_config.node_id,
        tls_config: node_config.tls_raft.as_ref().map(|tls| tls.client_config()),
        secret_raft: node_config.secret_raft.as_bytes().to_vec(),
    };

    let tx_caches = state_machine_store.tx_caches.clone();
    #[cfg(feature = "listen_notify")]
    let tx_notify = state_machine_store.tx_notify.clone();
    #[cfg(feature = "listen_notify")]
    let rx_notify = state_machine_store.rx_notify.clone();

    #[cfg(feature = "dlock")]
    let tx_dlock = state_machine_store.tx_dlock.clone();

    let raft = openraft::Raft::new(
        node_config.node_id,
        raft_config.clone(),
        network,
        log_store,
        state_machine_store,
    )
    .await
    .expect("Raft create failed");

    init::init_pristine_node_1_cache(
        &raft,
        node_config.node_id,
        &node_config.nodes,
        &node_config.secret_api,
        node_config.tls_api.is_some(),
        node_config
            .tls_api
            .as_ref()
            .map(|c| c.danger_tls_no_verify)
            .unwrap_or(false),
    )
    .await?;

    Ok(StateRaftCache {
        raft,
        lock: Default::default(),
        tx_caches,
        #[cfg(feature = "listen_notify")]
        tx_notify,
        #[cfg(feature = "listen_notify")]
        rx_notify,
        #[cfg(feature = "dlock")]
        tx_dlock,
    })
}

// TODO get rid of it - duplication
#[cfg(feature = "sqlite")]
pub(crate) fn connect_sqlite(
    path: Option<&str>,
    read_only: bool,
) -> Result<rusqlite::Connection, Error> {
    use rusqlite::OpenFlags;

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
