use crate::store::state_machine::memory::state_machine::{CacheRequest, CacheResponse};
use crate::Node;
use std::io::Cursor;

mod cache_ttl_handler;
pub mod kv_handler;
pub mod state_machine;

openraft::declare_raft_types!(
    pub TypeConfigKV:
        D = CacheRequest,
        R = CacheResponse,
        Node = Node,
        SnapshotData = Cursor<Vec<u8>>,
);
