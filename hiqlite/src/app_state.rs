use crate::NodeId;
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use tokio::sync::{Mutex, MutexGuard};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::{kv_handler::CacheRequestHandler, TypeConfigKV};

#[cfg(feature = "dashboard")]
use crate::client::stream::ClientStreamReq;
#[cfg(feature = "dashboard")]
use crate::dashboard::DashboardState;
#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::LockRequest;
#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::NotifyRequest;
#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::{
    state_machine::SqlitePool, writer::WriterRequest, TypeConfigSqlite,
};
#[cfg(feature = "dashboard")]
use std::sync::atomic::{AtomicUsize, Ordering};

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
    pub id: NodeId,
    pub addr_api: String,
    #[cfg(feature = "sqlite")]
    pub raft_db: StateRaftDB,
    #[cfg(feature = "cache")]
    pub raft_cache: StateRaftCache,
    pub secret_raft: String,
    pub secret_api: String,
    #[cfg(feature = "sqlite")]
    pub client_buffers_db: Mutex<HashMap<NodeId, VecDeque<Vec<u8>>>>,
    #[cfg(feature = "cache")]
    pub client_buffers_cache: Mutex<HashMap<NodeId, VecDeque<Vec<u8>>>>,
    #[cfg(feature = "dashboard")]
    pub dashboard: DashboardState,
    #[cfg(feature = "dashboard")]
    pub client_request_id: AtomicUsize,
    #[cfg(feature = "dashboard")]
    pub tx_client_stream: flume::Sender<ClientStreamReq>,
}

impl AppState {
    pub async fn get_buf_lock(
        &self,
        raft_type: &RaftType,
    ) -> MutexGuard<HashMap<NodeId, VecDeque<Vec<u8>>>> {
        match raft_type {
            #[cfg(feature = "sqlite")]
            RaftType::Sqlite => self.client_buffers_db.lock().await,
            #[cfg(feature = "cache")]
            RaftType::Cache => self.client_buffers_cache.lock().await,
            RaftType::Unknown => unreachable!("Invalid RaftType"),
        }
    }
}

#[cfg(feature = "dashboard")]
impl AppState {
    #[inline(always)]
    pub fn new_request_id(&self) -> usize {
        self.client_request_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(feature = "sqlite")]
pub struct StateRaftDB {
    pub raft: openraft::Raft<TypeConfigSqlite>,
    pub lock: tokio::sync::Mutex<()>,
    pub logs_writer: flume::Sender<crate::store::logs::rocksdb::ActionWrite>,
    pub sql_writer: flume::Sender<WriterRequest>,
    pub read_pool: SqlitePool,
    pub log_statements: bool,
}

#[cfg(feature = "cache")]
pub struct StateRaftCache {
    pub raft: openraft::Raft<TypeConfigKV>,
    pub lock: tokio::sync::Mutex<()>,
    pub tx_caches: Vec<flume::Sender<CacheRequestHandler>>,
    #[cfg(feature = "listen_notify")]
    pub tx_notify: flume::Sender<NotifyRequest>,
    #[cfg(feature = "listen_notify_local")]
    pub rx_notify: flume::Receiver<(i64, Vec<u8>)>,
    #[cfg(feature = "dlock")]
    pub tx_dlock: flume::Sender<LockRequest>,
}
