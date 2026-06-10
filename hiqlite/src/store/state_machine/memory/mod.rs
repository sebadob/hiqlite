use crate::store::state_machine::memory::state_machine::{CacheRequest, CacheResponse};
use crate::Node;
#[cfg(feature = "in-memory-snapshots")]
use std::io::Cursor;

mod cache_ttl_handler;
pub mod kv_handler;
pub mod state_machine;

#[cfg(feature = "dlock")]
pub mod dlock_handler;

#[cfg(feature = "listen_notify_local")]
pub mod notify_handler;

// By default Cache Snapshots are streamed directly from file, which is zero-copy and
// efficient even for large caches. With the opt-in `in-memory-snapshots` feature the
// snapshot is held in memory instead, so a pure cache-only node needs no `data_dir`.
#[cfg(not(feature = "in-memory-snapshots"))]
openraft::declare_raft_types!(
    pub TypeConfigKV:
        D = CacheRequest,
        R = CacheResponse,
        Node = Node,
        SnapshotData = tokio::fs::File,
);

#[cfg(feature = "in-memory-snapshots")]
openraft::declare_raft_types!(
    pub TypeConfigKV:
        D = CacheRequest,
        R = CacheResponse,
        Node = Node,
        SnapshotData = Cursor<Vec<u8>>,
);
