// The `openraft::declare_raft_types!` currently produces a known clippy error.
// This has been fixed already but not been released yet.
// This allow can be removed as soon as `openraft` is > 0.9.17
#![allow(unexpected_cfgs)]

use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::Node;
use crate::Response;

pub mod param;
pub mod reader;
pub mod snapshot_builder;
pub mod state_machine;
pub mod writer;
pub mod transaction_variable;

mod transaction_env;

openraft::declare_raft_types!(
    pub TypeConfigSqlite:
        D = QueryWrite,
        R = Response,
        Node = Node,
        SnapshotData = tokio::fs::File,
);
