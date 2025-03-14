use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::Node;
use crate::Response;

pub mod param;
pub mod reader;
pub mod snapshot_builder;
pub mod state_machine;
pub mod transaction_variable;
pub mod writer;

mod transaction_env;

openraft::declare_raft_types!(
    pub TypeConfigSqlite:
        D = QueryWrite,
        R = Response,
        Node = Node,
        SnapshotData = tokio::fs::File,
);
