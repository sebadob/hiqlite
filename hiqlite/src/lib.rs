// Copyright 2025 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(doc, feature(doc_auto_cfg))]

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::state_machine::Response;
pub use openraft::SnapshotPolicy;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use crate::{client::Client, error::Error};
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use config::{NodeConfig, RaftConfig};
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use tls::ServerTlsConfig;

#[cfg(feature = "cache")]
pub use num_derive::ToPrimitive;
#[cfg(feature = "cache")]
pub use strum::EnumIter;

#[cfg(feature = "dlock")]
pub use client::dlock::Lock;

#[cfg(feature = "sqlite")]
pub use crate::query::rows::Row;
#[cfg(feature = "sqlite")]
pub use crate::store::state_machine::sqlite::{param::Param, state_machine::Params};
#[cfg(feature = "sqlite")]
pub use migration::AppliedMigration;

// TODO remove after enough crash testing and making sure we can never get into a
// split brain situation
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod split_brain_check;

#[cfg(any(feature = "sqlite", feature = "cache"))]
mod app_state;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod client;
#[cfg(any(feature = "sqlite", feature = "cache"))]
mod config;
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

#[cfg(any(feature = "cache", feature = "sqlite"))]
pub(crate) static START_TS: std::sync::LazyLock<chrono::DateTime<chrono::Utc>> =
    std::sync::LazyLock::new(chrono::Utc::now);
#[cfg(any(feature = "cache", feature = "sqlite"))]
pub(crate) static HEALTH_CHECK_DELAY_SECS: std::sync::LazyLock<u16> =
    std::sync::LazyLock::new(|| {
        std::env::var("HQL_HEALTH_CHECK_DELAY_SECS")
            .unwrap_or_else(|_| String::from("30"))
            .parse::<u16>()
            .expect("Cannot parse HQL_HEALTH_CHECK_DELAY_SECS as u16")
    });

/// Helper macro to created Owned Params which can be serialized and sent
/// over the Raft network between nodes.
#[macro_export]
macro_rules! params {
    ( $( $param:expr ),* ) => {
        {
            #[allow(unused_mut)]
            let mut params = Vec::with_capacity(2);
            $(
                params.push(Param::from($param));
            )*
            params
        }
    };
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

/// The main entry function to start a Raft / Hiqlite node.
/// # Panics
/// If an incorrect `node_config` was given.
#[cfg(feature = "sqlite")]
pub async fn start_node(node_config: NodeConfig) -> Result<Client, Error> {
    #[derive(
        Debug, serde::Serialize, serde::Deserialize, strum::EnumIter, num_derive::ToPrimitive,
    )]
    enum Empty {}

    start::start_node_inner::<Empty>(node_config).await
}

/// The main entry function to start a Raft / Hiqlite node.
/// With the `cache` feature enabled, you need to provide the generic enum which
/// will function as the Cache Index value to decide between multiple caches.
/// # Panics
/// If an incorrect `node_config` was given.
#[cfg(feature = "cache")]
pub async fn start_node_with_cache<C>(node_config: NodeConfig) -> Result<Client, Error>
where
    C: Debug
        + Serialize
        + for<'a> Deserialize<'a>
        + strum::IntoEnumIterator
        + num_traits::ToPrimitive,
{
    start::start_node_inner::<C>(node_config).await
}
