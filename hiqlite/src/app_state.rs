use crate::NodeId;
use chrono::Utc;
use serde::Deserialize;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;

#[cfg(any(feature = "backup", feature = "dashboard"))]
use crate::client::stream::ClientStreamReq;
#[cfg(feature = "dashboard")]
use crate::dashboard::DashboardState;
#[cfg(feature = "sqlite")]
use crate::reader::ReadPool;
#[cfg(feature = "s3")]
use crate::s3::S3Config;
#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::LockRequest;
#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::NotifyRequest;
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::{
    TypeConfigCache, kv_handler::CacheRequestHandler, state_machine::StateMachineMemory,
};
#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::{
    TypeConfigSqlite, state_machine::StateMachineSqlite, writer::WriterRequest,
};
#[cfg(any(feature = "backup", feature = "dashboard"))]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RaftType {
    #[cfg(feature = "sqlite")]
    Sqlite,
    #[cfg(feature = "cache")]
    Cache,
    Unknown,
}

impl RaftType {
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "sqlite")]
            RaftType::Sqlite => "sqlite",
            #[cfg(feature = "cache")]
            RaftType::Cache => "cache",
            RaftType::Unknown => "unknown",
        }
    }
}

// Representation of an application state. This struct can be shared around to share
// instances of raft, store and more.
pub(crate) struct AppState {
    pub app_start: chrono::DateTime<Utc>,
    pub is_shutting_down: AtomicBool,
    pub servers_closed: flume::Receiver<()>,
    #[cfg(feature = "backup")]
    pub backups_dir: String,
    /// The prefix of every backup file name - see `NodeConfig`'s `backup_file_prefix`.
    #[cfg(feature = "backup")]
    pub backup_file_prefix: String,
    pub id: NodeId,
    #[cfg(feature = "cache")]
    pub nodes: Vec<crate::Node>,
    pub addr_api: String,
    #[cfg(feature = "sqlite")]
    pub raft_db: StateRaftDB,
    #[cfg(feature = "cache")]
    pub raft_cache: StateRaftCache,
    pub raft_lock: Arc<Mutex<()>>,
    #[cfg(feature = "s3")]
    pub s3_config: Option<Arc<S3Config>>,
    pub secret_raft: String,
    pub secret_api: String,
    pub secret_metrics: Option<String>,
    #[cfg(feature = "dashboard")]
    pub dashboard: DashboardState,
    #[cfg(any(feature = "backup", feature = "dashboard"))]
    pub client_request_id: AtomicUsize,
    #[cfg(any(feature = "backup", feature = "dashboard"))]
    pub tx_client_stream: flume::Sender<ClientStreamReq>,
    pub health_check_delay: Duration,
    /// See `NodeConfig::shutdown_delay`.
    pub shutdown_delay: Duration,
    pub learner_only: bool,
    /// The node ids that are configured to never start an election on their own. The leader uses
    /// this list to pick a handover target that is actually able to become one.
    pub election_disable: Vec<NodeId>,
}

#[cfg(any(feature = "backup", feature = "dashboard"))]
impl AppState {
    #[inline]
    pub fn new_request_id(&self) -> usize {
        self.client_request_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(feature = "sqlite")]
pub struct StateRaftDB {
    pub raft: openraft::Raft<TypeConfigSqlite, StateMachineSqlite>,
    pub shutdown_handle: hiqlite_wal::ShutdownHandle,
    pub sql_writer: flume::Sender<WriterRequest>,
    pub(crate) read_actors: Arc<ReadPool>,
    /// Where the network layer assembles an incoming snapshot, for the state machine to read.
    pub path_snapshots: String,
    pub log_statements: bool,
    pub is_raft_stopped: Arc<AtomicBool>,
    pub is_startup_finished: Arc<AtomicBool>,
    /// Mirrors openraft's runtime `enable_elect` for this group. openraft only has a setter
    /// (`Raft::runtime_config().elect()`), so Hiqlite tracks the value it applies here.
    pub enable_elect: Arc<AtomicBool>,
    /// The last log index this node has to re-apply before it is ready, taken from its own log
    /// store at startup. A single node targets its whole log tail, a cluster node the persisted
    /// `committed` pointer. `None` when the log store has no pointer to give, which is the case
    /// for a pristine node and for the in-memory cache store.
    pub startup_replay_target: Option<u64>,
    /// Set once the startup replay has finished, so it is never checked again afterward.
    pub startup_replay_done: AtomicBool,
    /// This node's backup outcomes since it started, for `GET /backup`.
    #[cfg(feature = "backup")]
    pub backup_status: Arc<crate::backup::BackupStatus>,
}

#[cfg(feature = "cache")]
pub struct StateRaftCache {
    pub raft: openraft::Raft<TypeConfigCache, Arc<StateMachineMemory>>,
    pub tx_caches: Vec<flume::Sender<CacheRequestHandler>>,
    #[cfg(feature = "listen_notify")]
    pub tx_notify: flume::Sender<NotifyRequest>,
    #[cfg(feature = "listen_notify")]
    pub rx_notify: flume::Receiver<(i64, Vec<u8>)>,
    #[cfg(feature = "dlock")]
    pub tx_dlock: flume::Sender<LockRequest>,
    pub is_raft_stopped: Arc<AtomicBool>,
    pub is_startup_finished: Arc<AtomicBool>,
    /// Mirrors openraft's runtime `enable_elect` for this group. openraft only has a setter
    /// (`Raft::runtime_config().elect()`), so Hiqlite tracks the value it applies here.
    pub enable_elect: Arc<AtomicBool>,
    pub shutdown_handle: Option<hiqlite_wal::ShutdownHandle>,
    /// Where the network layer assembles an incoming snapshot, for the state machine to read.
    pub path_snapshots: String,
    /// The last log index this node has to re-apply before it is ready, taken from its own log
    /// store at startup. A single node targets its whole log tail, a cluster node the persisted
    /// `committed` pointer. `None` when the log store has no pointer to give, which is the case
    /// for a pristine node and for the in-memory cache store.
    pub startup_replay_target: Option<u64>,
    /// Set once the startup replay has finished, so it is never checked again afterward.
    pub startup_replay_done: AtomicBool,
    #[cfg(feature = "cache")]
    pub cache_storage_disk: bool,
}

#[cfg(feature = "sqlite")]
impl StateRaftDB {
    /// See [`check_startup_replay`].
    pub(crate) fn check_replay(&self) -> Result<(), crate::Error> {
        use openraft::type_config::async_runtime::watch::WatchReceiver;

        if self
            .startup_replay_done
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(());
        }
        let last_applied = self
            .raft
            .metrics()
            .borrow_watched()
            .last_applied
            .map(|id| id.index);
        check_startup_replay(self.startup_replay_target, last_applied, "sqlite")?;
        self.startup_replay_done
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(feature = "cache")]
impl StateRaftCache {
    /// See [`check_startup_replay`].
    pub(crate) fn check_replay(&self) -> Result<(), crate::Error> {
        use openraft::type_config::async_runtime::watch::WatchReceiver;

        if self
            .startup_replay_done
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(());
        }
        let last_applied = self
            .raft
            .metrics()
            .borrow_watched()
            .last_applied
            .map(|id| id.index);
        check_startup_replay(self.startup_replay_target, last_applied, "cache")?;
        self.startup_replay_done
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

/// Returns an error while the node is still re-applying the log it found at startup.
///
/// After a restart the state machine is behind the node's own log, and after an unclean stop it
/// may even be empty. Reporting ready in that window lets an application read stale or missing
/// data. The target comes from the local log store only, so the check never needs a peer: a
/// single node takes its log tail, because it commits on its own, and a cluster node takes the
/// persisted `committed` pointer, which is the part of its log that is guaranteed to replay.
/// Entries committed after that pointer are not this gate's job - the leader sends them once the
/// node joins, and `wait_for_cluster_sync` waits for them. The target is fixed at startup, so
/// writes accepted afterward never move it, and once it is reached the check is latched.
#[cfg(any(feature = "sqlite", feature = "cache"))]
fn check_startup_replay(
    target: Option<u64>,
    last_applied: Option<u64>,
    raft_name: &str,
) -> Result<(), crate::Error> {
    if let Some(target) = target
        && last_applied.is_none_or(|applied| applied < target)
    {
        return Err(crate::Error::Error(
            format!(
                "{raft_name}: still re-applying its log after startup (applied {last_applied:?} \
                 of {target})"
            )
            .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(target: Option<u64>, last_applied: Option<u64>) -> bool {
        check_startup_replay(target, last_applied, "test").is_ok()
    }

    /// The gate must stay open when the log store has no pointer to give - a pristine node and
    /// the in-memory cache store - and it must close on every shape of "not replayed yet",
    /// including a state machine that was wiped and has applied nothing at all.
    #[test]
    fn startup_replay_gate() {
        // no target: nothing to wait for, the node is ready
        assert!(ready(None, None));
        assert!(ready(None, Some(7)));

        // a cluster node whose state machine was wiped has applied nothing, and its target is the
        // persisted `committed` pointer - this is the case a cluster never gated on before
        assert!(!ready(Some(100), None));

        // mid-replay
        assert!(!ready(Some(100), Some(0)));
        assert!(!ready(Some(100), Some(99)));

        // the target is reached
        assert!(ready(Some(100), Some(100)));

        // the leader already sent more than the startup target: the gate is behind the state
        // machine, not in front of it
        assert!(ready(Some(100), Some(140)));
    }
}
