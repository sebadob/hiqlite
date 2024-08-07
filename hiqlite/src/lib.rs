// Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::state_machine::Response;

pub use openraft::SnapshotPolicy;

#[cfg(any(feature = "sqlite", feature = "cache"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "sqlite", feature = "cache"))))]
pub use crate::{client::Client, error::Error};
#[cfg_attr(docsrs, doc(cfg(any(feature = "sqlite", feature = "cache"))))]
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use config::{NodeConfig, RaftConfig};
#[cfg_attr(docsrs, doc(cfg(any(feature = "sqlite", feature = "cache"))))]
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use tls::ServerTlsConfig;

#[cfg(feature = "cache")]
#[cfg_attr(docsrs, doc(cfg(feature = "cache")))]
pub use num_derive::ToPrimitive;
#[cfg(feature = "cache")]
#[cfg_attr(docsrs, doc(cfg(feature = "cache")))]
pub use strum::EnumIter;

#[cfg(feature = "dlock")]
#[cfg_attr(docsrs, doc(cfg(feature = "dlock")))]
pub use client::dlock::Lock;

#[cfg(feature = "sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
pub use crate::query::rows::Row;
#[cfg(feature = "sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
pub use crate::store::state_machine::sqlite::{param::Param, state_machine::Params};
#[cfg(feature = "sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
pub use migration::AppliedMigration;

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

#[cfg(all(feature = "backup", any(feature = "cache", feature = "sqlite")))]
mod backup;
#[cfg(feature = "dashboard")]
mod dashboard;
#[cfg(feature = "sqlite")]
mod migration;
#[cfg(feature = "sqlite")]
mod query;

/// Exports and types to set up a connection to an S3 storage bucket.
/// Needs the feature `s3` enabled.
#[cfg(all(feature = "s3", any(feature = "cache", feature = "sqlite")))]
#[cfg_attr(docsrs, doc(cfg(feature = "s3")))]
pub mod s3;

type NodeId = u64;

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
#[cfg(all(feature = "sqlite", not(feature = "cache")))]
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
pub async fn start_node<C>(node_config: NodeConfig) -> Result<Client, Error>
where
    C: Debug
        + Serialize
        + for<'a> Deserialize<'a>
        + strum::IntoEnumIterator
        + num_traits::ToPrimitive,
{
    start::start_node_inner::<C>(node_config).await
}
