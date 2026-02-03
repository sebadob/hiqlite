// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(doc, feature(doc_auto_cfg))]

#[cfg(all(feature = "cast_ints", feature = "cast_ints_unchecked"))]
compile_error!("features `cast_ints` and `cast_ints_unchecked` are mutually exclusive!");

#[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub use hiqlite_wal::LogSync;
pub use openraft::SnapshotPolicy;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::state_machine::Response;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use crate::{client::Client, error::Error};
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use config::{NodeConfig, RaftConfig};
#[cfg(feature = "sqlite")]
pub use query::cust_types::VecText;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use tls::ServerTlsConfig;

#[cfg(feature = "sqlite")]
pub use crate::query::rows::Row;
#[cfg(feature = "sqlite")]
pub use crate::store::state_machine::sqlite::{
    param::Param,
    state_machine::Params,
    transaction_variable::{StmtColumn, StmtIndex},
};
#[cfg(feature = "dlock")]
pub use client::dlock::Lock;
#[cfg(feature = "sqlite")]
pub use migration::AppliedMigration;

#[cfg(any(feature = "sqlite", feature = "cache"))]
mod app_state;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod client;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod config;
#[cfg(all(any(feature = "sqlite", feature = "cache"), feature = "toml"))]
mod config_toml;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod error;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod helpers;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod init;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod network;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod start;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod store;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod tls;

#[cfg(feature = "backup")]
mod backup;
#[cfg(feature = "dashboard")]
mod dashboard;
#[cfg(feature = "sqlite")]
mod migration;
#[cfg(feature = "sqlite")]
mod query;
mod split_brain_check;

/// Exports and types to set up a connection to an S3 storage bucket.
/// Needs the feature `s3` enabled.
#[cfg(feature = "s3")]
pub mod s3;

/// Contains everything to start the server binary.
/// Changes inside this module are not considered breaking changes.
/// They should only be used internally to compile the standalone binary.
#[cfg(feature = "server")]
pub mod server;

type NodeId = u64;

pub trait CacheVariants {
    /// Returns the Enum Variants index, strictly matching the output of `hiqlite_cache_variants()`.
    fn hiqlite_cache_index(&self) -> usize;

    /// Returns the Enum Variants as `(idx, name)` in strictly ascending order, starting at `0`.
    fn hiqlite_cache_variants() -> &'static [(usize, &'static str)];
}

/// A Raft / Hiqlite node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Node {
    /// Each Raft config must include one Node with `id == 1`.
    /// Node `1` will care about init and setup if the Raft does not exit yet or
    /// if other Nodes need to join.
    pub id: NodeId,
    /// The Raft internal address. This is separated from the API address and runs on
    /// a different server and port to make it possible to boost security and split
    /// network bandwidth. The internal Raft API should never be exposed to the public.
    pub addr_raft: String,
    /// The public API address. To this address, the `Client`s will connect to talk
    /// to other Raft Leader nodes over the network if necessary.
    pub addr_api: String,
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Node {{ id: {}, rpc_addr: {}, api_addr: {} }}",
            self.id, self.addr_raft, self.addr_api
        )
    }
}

#[cfg(feature = "sqlite")]
mod empty {
    use crate::CacheVariants;

    #[derive(Debug)]
    pub enum Empty {}

    impl CacheVariants for Empty {
        fn hiqlite_cache_index(&self) -> usize {
            unreachable!()
        }

        fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
            &[]
        }
    }
}

/// The main entry function to start a Raft / Hiqlite node.
/// # Panics
/// If an incorrect `node_config` was given.
#[cfg(feature = "sqlite")]
pub async fn start_node(node_config: NodeConfig) -> Result<Client, Error> {
    start::start_node_inner::<empty::Empty>(node_config).await
}

/// The main entry function to start a Raft / Hiqlite node.
/// With the `cache` feature enabled, you need to provide the generic enum which
/// will function as the Cache Index value to decide between multiple caches.
/// # Panics
/// If an incorrect `node_config` was given.
#[cfg(feature = "cache")]
pub async fn start_node_with_cache<C>(node_config: NodeConfig) -> Result<Client, Error>
where
    C: Debug + CacheVariants,
{
    start::start_node_inner::<C>(node_config).await
}
