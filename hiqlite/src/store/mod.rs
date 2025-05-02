#![allow(unused)]

use crate::app_state::{AppState, RaftType};
use crate::init::IsPristineNode1;
use crate::network::NetworkStreaming;
use crate::{init, Error, NodeConfig, NodeId, RaftConfig};
use openraft::storage::RaftLogStorage;
use openraft::{Raft, StorageError};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cmp::PartialEq;
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
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
    let log_store =
        logs::rocksdb::LogStoreRocksdb::new(&node_config.data_dir, node_config.sync_immediate)
            .await;
    let state_machine_store = StateMachineSqlite::new(
        &node_config.data_dir,
        &node_config.filename_db,
        node_config.node_id,
        node_config.log_statements,
        node_config.prepared_statement_cache_capacity,
        node_config.read_pool_size,
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
        raft_type: RaftType::Sqlite,
        heartbeat_interval: node_config.raft_config.heartbeat_interval,
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
        is_raft_stopped: AtomicBool::new(false),
    })
}

#[cfg(feature = "cache")]
pub(crate) async fn start_raft_cache<C>(
    node_config: NodeConfig,
    raft_config: Arc<RaftConfig>,
) -> Result<(IsPristineNode1, StateRaftCache), Error>
where
    C: Debug + IntoEnumIterator + crate::cache_idx::CacheIndex,
{
    let log_store = logs::memory::LogStoreMemory::new();
    let state_machine_store = Arc::new(StateMachineMemory::new::<C>().await?);

    let network = NetworkStreaming {
        node_id: node_config.node_id,
        tls_config: node_config.tls_raft.as_ref().map(|tls| tls.client_config()),
        secret_raft: node_config.secret_raft.as_bytes().to_vec(),
        raft_type: RaftType::Cache,
        heartbeat_interval: node_config.raft_config.heartbeat_interval,
    };

    let tx_caches = state_machine_store.tx_caches.clone();
    #[cfg(feature = "listen_notify")]
    let tx_notify = state_machine_store.tx_notify.clone();
    #[cfg(feature = "listen_notify_local")]
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

    let is_pristine = init::init_pristine_node_1_cache(
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

    Ok((
        is_pristine,
        StateRaftCache {
            raft,
            lock: Default::default(),
            tx_caches,
            #[cfg(feature = "listen_notify")]
            tx_notify,
            #[cfg(feature = "listen_notify_local")]
            rx_notify,
            #[cfg(feature = "dlock")]
            tx_dlock,
            is_raft_stopped: AtomicBool::new(false),
        },
    ))
}
