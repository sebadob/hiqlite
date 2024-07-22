use crate::store::logs::rocksdb::ActionWrite;
use crate::store::state_machine::sqlite::state_machine::SqlitePool;
use crate::store::state_machine::sqlite::writer::WriterRequest;
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::NodeId;
use openraft::Config;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Representation of an application state. This struct can be shared around to share
// instances of raft, store and more.
#[allow(dead_code)]
#[derive(Clone)]
pub struct AppState {
    pub id: NodeId,
    pub addr_api: String,
    pub addr_raft: String,
    pub raft: openraft::Raft<TypeConfigSqlite>,
    // Helper to avoid race conditions with multiple self-managed membership requests
    pub raft_lock: Arc<Mutex<()>>,
    pub logs_writer: flume::Sender<ActionWrite>,
    pub sql_writer: flume::Sender<WriterRequest>,
    pub read_pool: Arc<SqlitePool>,
    pub config: Arc<Config>,
    pub log_statements: bool,
    pub secret_raft: String,
    pub secret_api: String,
    // TODO this should become dynamic at some point to make dynamic cluster changes possible in the future
    #[allow(clippy::type_complexity)]
    pub client_buffers: HashMap<NodeId, (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>)>,
}
