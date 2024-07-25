use crate::store::logs::rocksdb::ActionWrite;
use crate::store::state_machine::memory::TypeConfigKV;
use crate::store::state_machine::sqlite::state_machine::SqlitePool;
use crate::store::state_machine::sqlite::writer::WriterRequest;
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::{NodeId, RaftConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Representation of an application state. This struct can be shared around to share
// instances of raft, store and more.
#[allow(dead_code)]
#[derive(Clone)] // TODO remove Clone to not accidentally do it -> always wrap in Arc
pub struct AppState {
    pub id: NodeId,
    pub addr_api: String,
    pub addr_raft: String,
    // TODO probably feature gate the 2 raft's properly (and stuff that belongs to them)
    pub raft_db: StateRaftDB,
    // pub raft: openraft::Raft<TypeConfigSqlite>,
    pub raft_cache: StateRaftCache,
    // Helper to avoid race conditions with multiple self-managed membership requests
    pub raft_lock: Arc<Mutex<()>>,
    // pub logs_writer: flume::Sender<ActionWrite>,
    // pub sql_writer: flume::Sender<WriterRequest>,
    // pub read_pool: Arc<SqlitePool>,
    pub config: Arc<RaftConfig>,
    // pub log_statements: bool,
    pub secret_raft: String,
    pub secret_api: String,
    // TODO this should become dynamic at some point to make dynamic cluster changes possible in the future
    #[allow(clippy::type_complexity)]
    pub client_buffers: HashMap<NodeId, (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>)>,
}

#[derive(Clone)]
pub struct StateRaftDB {
    pub raft: openraft::Raft<TypeConfigSqlite>,
    pub logs_writer: flume::Sender<ActionWrite>,
    pub sql_writer: flume::Sender<WriterRequest>,
    pub read_pool: Arc<SqlitePool>,
    pub log_statements: bool,
}

#[derive(Clone)]
pub struct StateRaftCache {
    pub raft: openraft::Raft<TypeConfigKV>,
    pub logs_writer: flume::Sender<ActionWrite>,
    // TODO how to implement reader here? probably expose the inner map?
    // pub read_pool: Arc<SqlitePool>,
}
