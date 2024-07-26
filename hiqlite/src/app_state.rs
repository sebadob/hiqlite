use crate::store::logs;
use crate::store::state_machine::sqlite::state_machine::SqlitePool;
use crate::store::state_machine::sqlite::writer::WriterRequest;
use crate::NodeId;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::state_machine::StateMachineMemory;
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

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
    pub kv_store: Arc<StateMachineMemory>,
}
