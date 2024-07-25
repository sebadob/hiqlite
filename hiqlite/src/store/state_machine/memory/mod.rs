use crate::store::state_machine::memory::memory::{CacheRequest, CacheResponse};
use crate::Node;
use std::io::Cursor;

pub mod memory;

openraft::declare_raft_types!(
    pub TypeConfigKV:
        D = CacheRequest,
        R = CacheResponse,
        Node = Node,
        SnapshotData = Cursor<Vec<u8>>,
);
