#![allow(unused)]

use crate::app_state::{AppState, RaftType};
use crate::network::NetworkStreaming;
use crate::{CacheVariants, Error, NodeConfig, NodeId, RaftConfig, init};
use hiqlite_wal::LogSync;
use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::storage::RaftLogStorage;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cmp::PartialEq;
use std::fmt::Debug;
use std::io;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::time;
use tracing::info;

#[cfg(feature = "sqlite")]
use crate::reader::ReaderRequest;
#[cfg(feature = "cache")]
use crate::{
    app_state::StateRaftCache,
    store::state_machine::memory::{TypeConfigCache, state_machine::StateMachineMemory},
};
#[cfg(feature = "sqlite")]
use crate::{
    app_state::StateRaftDB,
    store::state_machine::sqlite::{
        TypeConfigSqlite, state_machine::StateMachineSqlite, writer::WriterRequest,
    },
};

pub mod logs;
pub mod state_machine;

#[cfg(all(test, feature = "cache"))]
mod tests_openraft_suite;

/// Error type of the storage traits: openraft 0.10 defines them in terms of `io::Error`.
pub type StorageResult<T> = Result<T, io::Error>;

/// The log index this node's state machine must apply before the node is allowed to serve
/// requests, read straight off the log store while no peer is involved.
///
/// A single node is its own quorum: every entry its log holds is committed, so the log tail is
/// the target.
///
/// A cluster node must not gate on the log tail. It acknowledges appends before they are
/// flushed, and openraft commits on a quorum of those acks, so the tail can hold entries that
/// were never committed. Waiting for the tail is `Raft::wait_for_recovery()`, a call that needs
/// the very peers a startup gate has to survive without. The persisted `committed` pointer is
/// the peer-free target here, and the store clamps it to the log tail, so it is never ahead of
/// what this node can replay.
///
/// Which part of this is load-bearing depends on when the node opens its port. A node that binds
/// its router before the replay finishes - every node but the first - is gated by the check on
/// `/ready`. The first node is not: its startup path already waits for the replay, so `/ready` is
/// unreachable until the target is met, and the check there is a backstop, not the gate.
async fn startup_replay_target<T, C>(
    log_store: &mut T,
    single_node: bool,
) -> StorageResult<Option<u64>>
where
    T: RaftLogStorage<C>,
    C: RaftTypeConfig,
{
    if single_node {
        return Ok(log_store
            .get_log_state()
            .await?
            .last_log_id
            .map(|id| id.index));
    }
    Ok(log_store.read_committed().await?.map(|id| id.index))
}

#[cfg(feature = "sqlite")]
pub(crate) async fn start_raft_db(
    node_config: &NodeConfig,
    raft_config: Arc<RaftConfig>,
    do_reset_metadata: bool,
) -> Result<StateRaftDB, Error> {
    // We always want to start stopped and set to `false` as soon as we found out,
    // that we are not pristine node and need cleanup.
    let is_raft_stopped = Arc::new(AtomicBool::new(true));
    // The runtime mirror of openraft's `enable_elect`, seeded from our config value.
    let enable_elect = Arc::new(AtomicBool::new(raft_config.enable_elect));

    let mut log_store = hiqlite_wal::LogStore::<TypeConfigSqlite>::start(
        logs::logs_dir_db(&node_config.data_dir),
        node_config.wal_sync,
        node_config.wal_size,
    )
    .await?;
    let startup_replay_target =
        startup_replay_target(&mut log_store, node_config.nodes.len() == 1).await?;
    let state_machine_store = StateMachineSqlite::new(
        &node_config.data_dir,
        &node_config.filename_db,
        node_config.node_id,
        node_config.log_statements,
        node_config.prepared_statement_cache_capacity,
        node_config.read_pool_size,
        node_config.saturated_pool_threshold,
        node_config.slow_query_threshold,
        node_config.log_slow_queries,
        #[cfg(feature = "s3")]
        node_config.s3_config.clone(),
        do_reset_metadata,
        #[cfg(feature = "backup")]
        node_config.backup_keep_for_local,
        #[cfg(feature = "backup")]
        &node_config.backup_file_prefix,
    )
    .await?;

    let is_startup_finished = Arc::new(AtomicBool::new(false));
    let sql_writer = state_machine_store.write_tx.clone();
    let read_actors = state_machine_store.read_actors.clone();
    #[cfg(feature = "backup")]
    let backup_status = state_machine_store.backup_status.clone();
    let path_snapshots = StateMachineSqlite::path_snapshots(&node_config.data_dir);

    let network = NetworkStreaming {
        node_id: node_config.node_id,
        tls_config: node_config.tls_raft.as_ref().map(|tls| tls.client_config()),
        secret_raft: node_config.secret_raft.as_bytes().to_vec(),
        raft_type: RaftType::Sqlite,
        heartbeat_interval: node_config.raft_config.heartbeat_interval,
        is_raft_stopped: is_raft_stopped.clone(),
        is_startup_finished: is_startup_finished.clone(),
    };

    let shutdown_handle = log_store.shutdown_handle();

    let raft = openraft::Raft::new(
        node_config.node_id,
        raft_config.clone(),
        network,
        log_store,
        state_machine_store,
    )
    .await?;

    init::init_pristine_node_1_db(
        &raft,
        node_config.node_id,
        &node_config.nodes,
        &node_config.secret_api,
        node_config.tls_api.is_some(),
        node_config
            .tls_api
            .as_ref()
            .is_some_and(|c| c.danger_tls_no_verify()),
    )
    .await?;

    Ok(StateRaftDB {
        raft,
        shutdown_handle,
        sql_writer,
        read_actors,
        path_snapshots,
        log_statements: node_config.log_statements,
        is_raft_stopped,
        is_startup_finished,
        enable_elect,
        startup_replay_target,
        startup_replay_done: AtomicBool::new(false),
        #[cfg(feature = "backup")]
        backup_status,
    })
}

#[cfg(feature = "cache")]
pub(crate) async fn start_raft_cache<C>(
    node_config: &NodeConfig,
    raft_config: Arc<RaftConfig>,
) -> Result<StateRaftCache, Error>
where
    C: Debug + CacheVariants,
{
    // TODO add a check here and only start the cache layer, if the given Idx Enum is NOT empty

    // We always want to start stopped and set to `false` as soon as we found out,
    // that we are not pristine node and need cleanup.
    let is_raft_stopped = Arc::new(AtomicBool::new(true));
    let is_startup_finished = Arc::new(AtomicBool::new(false));
    // The runtime mirror of openraft's `enable_elect`, seeded from our config value.
    let enable_elect = Arc::new(AtomicBool::new(raft_config.enable_elect));

    let state_machine_store = Arc::new(
        StateMachineMemory::new::<C>(&node_config.data_dir, !node_config.cache_storage_disk)
            .await?,
    );
    let network = NetworkStreaming {
        node_id: node_config.node_id,
        tls_config: node_config.tls_raft.as_ref().map(|tls| tls.client_config()),
        secret_raft: node_config.secret_raft.as_bytes().to_vec(),
        raft_type: RaftType::Cache,
        heartbeat_interval: node_config.raft_config.heartbeat_interval,
        is_startup_finished: is_startup_finished.clone(),
        is_raft_stopped: is_raft_stopped.clone(),
    };

    let tx_caches = state_machine_store.tx_caches.clone();
    let path_snapshots = format!(
        "{}/snapshots",
        StateMachineMemory::path_sm(&node_config.data_dir)
    );
    #[cfg(feature = "listen_notify")]
    let tx_notify = state_machine_store.tx_notify.clone();
    #[cfg(feature = "listen_notify")]
    let rx_notify = state_machine_store.rx_notify.clone();

    #[cfg(feature = "dlock")]
    let tx_dlock = state_machine_store.tx_dlock.clone();

    // The log-revert allowance from `NodeConfig::default_raft_config` applies to both groups, so
    // the shared `Arc` Raft config is used here as-is.

    let (raft, shutdown_handle, startup_replay_target) = if node_config.cache_storage_disk {
        let mut log_store = hiqlite_wal::LogStore::<TypeConfigCache>::start(
            logs::logs_dir_cache(&node_config.data_dir),
            node_config.wal_sync,
            node_config.wal_size,
        )
        .await?;
        let startup_replay_target =
            startup_replay_target(&mut log_store, node_config.nodes.len() == 1).await?;
        let shutdown_handle = log_store.shutdown_handle();

        let raft = openraft::Raft::new(
            node_config.node_id,
            raft_config.clone(),
            network,
            log_store,
            state_machine_store,
        )
        .await?;

        (raft, Some(shutdown_handle), startup_replay_target)
    } else {
        let raft = openraft::Raft::new(
            node_config.node_id,
            raft_config.clone(),
            network,
            logs::memory::LogStoreMemory::new(),
            state_machine_store,
        )
        .await?;

        // An in-memory log store starts empty, so there is nothing to replay.
        (raft, None, None)
    };

    init::init_pristine_node_1_cache(
        &raft,
        node_config.cache_storage_disk,
        node_config.node_id,
        &node_config.nodes,
        &node_config.secret_api,
        node_config.tls_api.is_some(),
        node_config
            .tls_api
            .as_ref()
            .is_some_and(|c| c.danger_tls_no_verify()),
    )
    .await?;

    Ok(StateRaftCache {
        raft,
        tx_caches,
        #[cfg(feature = "listen_notify")]
        tx_notify,
        #[cfg(feature = "listen_notify")]
        rx_notify,
        #[cfg(feature = "dlock")]
        tx_dlock,
        is_raft_stopped,
        is_startup_finished,
        enable_elect,
        shutdown_handle,
        path_snapshots,
        startup_replay_target,
        startup_replay_done: AtomicBool::new(false),
        cache_storage_disk: node_config.cache_storage_disk,
    })
}
