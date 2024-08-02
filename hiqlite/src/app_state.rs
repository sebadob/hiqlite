use crate::store::logs;
use crate::store::state_machine::sqlite::state_machine::SqlitePool;
use crate::store::state_machine::sqlite::writer::WriterRequest;
use crate::NodeId;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[cfg(feature = "dashboard")]
use crate::client::stream::ClientStreamReq;
#[cfg(feature = "dashboard")]
use crate::dashboard::DashboardState;
#[cfg(feature = "dashboard")]
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RaftType {
    #[cfg(feature = "sqlite")]
    Sqlite,
    #[cfg(feature = "cache")]
    Cache,
}

impl RaftType {
    pub fn as_str(&self) -> &str {
        match self {
            #[cfg(feature = "sqlite")]
            RaftType::Sqlite => "sqlite",
            #[cfg(feature = "cache")]
            RaftType::Cache => "cache",
        }
    }
}

// Representation of an application state. This struct can be shared around to share
// instances of raft, store and more.
pub struct AppState {
    pub id: NodeId,
    pub addr_api: String,
    // pub addr_raft: String,
    pub raft_db: StateRaftDB,
    #[cfg(feature = "cache")]
    pub raft_cache: StateRaftCache,
    // Helper to avoid race conditions with multiple self-managed membership requests
    // pub raft_lock: Mutex<()>,
    pub secret_raft: String,
    pub secret_api: String,
    // TODO this should become dynamic at some point to make dynamic cluster changes possible in the future
    #[allow(clippy::type_complexity)]
    pub client_buffers: HashMap<NodeId, (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>)>,
    #[cfg(feature = "dashboard")]
    pub dashboard: DashboardState,
    #[cfg(feature = "dashboard")]
    pub client_request_id: AtomicUsize,
    #[cfg(feature = "dashboard")]
    pub tx_client_stream: flume::Sender<ClientStreamReq>,
}

#[cfg(feature = "dashboard")]
impl AppState {
    #[inline(always)]
    pub fn new_request_id(&self) -> usize {
        self.client_request_id.fetch_add(1, Ordering::Relaxed)
    }
}

pub struct StateRaftDB {
    pub raft: openraft::Raft<TypeConfigSqlite>,
    pub lock: Mutex<()>,
    pub logs_writer: flume::Sender<logs::rocksdb::ActionWrite>,
    pub sql_writer: flume::Sender<WriterRequest>,
    pub read_pool: Arc<SqlitePool>,
    pub log_statements: bool,
}

#[cfg(feature = "cache")]
pub struct StateRaftCache {
    pub raft: openraft::Raft<TypeConfigKV>,
    pub lock: Mutex<()>,
    pub tx_caches: Vec<flume::Sender<CacheRequestHandler>>,
    pub rx_notify: flume::Receiver<(i64, Vec<u8>)>,
}
