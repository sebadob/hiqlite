// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(doc, feature(doc_cfg))]

// The derive macros emit the absolute path `::hiqlite::...`. That resolves for downstream users,
// but not from inside this crate. Aliasing self makes the generated code (and in-crate tests of it)
// resolve the same way.
extern crate self as hiqlite;

// The base features `sqlite`, `cache` and `external-state-machine` each work standalone or in
// combination; everything else is an add-on on top of one of them.
#[cfg(not(any(feature = "__cluster", feature = "external-state-machine")))]
compile_error!("a base feature is required: `sqlite`, `cache` or `external-state-machine`");

#[cfg(all(feature = "cast_ints", feature = "cast_ints_unchecked"))]
compile_error!("features `cast_ints` and `cast_ints_unchecked` are mutually exclusive");

// These add-ons only have code inside the Raft runtime modules, so they need `sqlite` or `cache` -
// with `external-state-machine` alone their dependencies would be pulled unused.
#[cfg(all(feature = "auto-heal", not(any(feature = "sqlite", feature = "cache"))))]
compile_error!("feature `auto-heal` needs one of the base features `sqlite` or `cache`");

#[cfg(all(
    feature = "shutdown-handle",
    not(any(feature = "sqlite", feature = "cache"))
))]
compile_error!("feature `shutdown-handle` needs one of the base features `sqlite` or `cache`");

#[cfg(all(feature = "toml", not(any(feature = "sqlite", feature = "cache"))))]
compile_error!("feature `toml` needs one of the base features `sqlite` or `cache`");

#[cfg(all(
    feature = "webpki-roots",
    not(any(feature = "sqlite", feature = "cache"))
))]
compile_error!("feature `webpki-roots` needs one of the base features `sqlite` or `cache`");

#[cfg(all(
    feature = "jemalloc",
    not(target_env = "msvc"),
    not(feature = "__profiling")
))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use hiqlite_wal::LogSync;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use openraft::SnapshotPolicy;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use openraft::raft::ReadPolicy;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use std::fmt::{Debug, Display};

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::state_machine::Response;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use crate::{client::Client, error::Error};
#[cfg(feature = "backup")]
pub use backup::{BackupSource, DEFAULT_BACKUP_FILE_PREFIX};
#[cfg(feature = "config-embed")]
pub use config::REFERENCE_CONFIG;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use config::{EncKeysArg, NodeConfig, RaftConfig, RateLimitConfig};
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use network::management::{ElectionState, MembersState, NodeElection, NodeMembers};
#[cfg(feature = "sqlite")]
pub use query::cust_types::VecText;

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

/// Re-export of the exact `rusqlite` version Hiqlite is built with.
///
/// Use this instead of adding a separate `rusqlite` dependency to avoid
/// version conflicts, e.g. when implementing a `DeterministicSqliteOperation`
/// against the `rusqlite::Transaction` type.
#[cfg(any(feature = "sqlite", feature = "external-state-machine"))]
pub use rusqlite;

/// SQLite state-machine machinery for applications that already own consensus.
///
/// This module does not start a Hiqlite Raft group or network service.
#[cfg(feature = "external-state-machine")]
pub mod external_state_machine;

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

#[cfg(feature = "backup")]
mod backup;
#[cfg(feature = "dashboard")]
mod dashboard;
#[cfg(feature = "macros")]
pub mod macros;
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

#[cfg(any(feature = "sqlite", feature = "cache"))]
mod http_client;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub mod tls;

#[cfg(any(feature = "sqlite", feature = "cache"))]
mod v0_17_cold_restart_guard;

#[cfg(any(feature = "sqlite", feature = "cache"))]
type NodeId = u64;

#[allow(dead_code)]
pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(any(feature = "sqlite", feature = "cache"))]
pub trait CacheVariants {
    /// Returns the Enum Variants index, strictly matching the output of `hiqlite_cache_variants()`.
    fn hiqlite_cache_index(&self) -> usize;

    /// Returns the Enum Variants as `(idx, name)` in strictly ascending order, starting at `0`.
    fn hiqlite_cache_variants() -> &'static [(usize, &'static str)];

    /// Returns the cache variants as a stable, line-oriented text form: one `<index> <name>`
    /// pair per line in ascending index order (e.g. `0 App\n1 AuthCodes\n`). This is the
    /// on-disk metadata format written by the cache state machine; parse each line with
    /// `split_once(' ')`.
    fn hiqlite_cache_variants_normalized() -> String {
        Self::hiqlite_cache_variants()
            .iter()
            .map(|&(idx, name)| format!("{idx} {name}\n"))
            .collect()
    }

    /// Returns `true` when every `(index, name)` pair recorded in `stored` (the normalized
    /// form of a previously persisted cache index) still exists unchanged in the current enum.
    /// Adding new variants at the end is allowed (enum expansion); any re-order,
    /// insert-in-between, removal, or rename makes it incompatible and returns `false`.
    ///
    /// A rename is recovered by starting every node once with `HQL_CACHE_META_RESET=true`, which
    /// re-initializes the persisted metadata from the current enum. A removal needs that plus a
    /// full cache clear on every node before anything reads a cache; the entries of the removed
    /// index are persisted, and every index behind it has shifted.
    fn hiqlite_cache_compatible_with(stored: &str) -> bool {
        let variants = Self::hiqlite_cache_variants();
        for line in stored.lines() {
            let (idx, name) = match line.split_once(' ') {
                Some((idx, name)) => (idx, name),
                None => return false,
            };
            let idx: usize = match idx.parse() {
                Ok(idx) => idx,
                Err(_) => return false,
            };
            match variants.get(idx).map(|v| v.1) {
                Some(n) if n == name => {}
                _ => return false,
            }
        }
        true
    }
}

/// A Raft / Hiqlite node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg(any(feature = "sqlite", feature = "cache"))]
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

#[cfg(any(feature = "sqlite", feature = "cache"))]
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
#[cfg(feature = "sqlite")]
pub async fn start_node(node_config: NodeConfig) -> Result<Client, Error> {
    start::start_node_inner::<empty::Empty>(
        Box::new(node_config),
        #[cfg(feature = "backup")]
        None,
    )
    .await
}

/// The main entry function to start a Raft / Hiqlite node.
/// With the `cache` feature enabled, you need to provide the generic enum which
/// will function as the Cache Index value to decide between multiple caches.
#[cfg(feature = "cache")]
pub async fn start_node_with_cache<C>(node_config: NodeConfig) -> Result<Client, Error>
where
    C: Debug + CacheVariants,
{
    start::start_node_inner::<C>(
        Box::new(node_config),
        #[cfg(feature = "backup")]
        None,
    )
    .await
}

/// Start a Raft / Hiqlite node and restore the given backup before the Raft boots.
///
/// This is the `HQL_BACKUP_RESTORE` env var as a parameter: the env var is not read at all, which
/// is what you need when a single process runs several Hiqlite instances with their own names and
/// their own backups. Set a distinct `backup_file_prefix` per instance, so their backups never collide
/// in a shared backups directory or bucket.
///
/// Give every node of the cluster the same source: node 1 applies the backup, all other nodes
/// delete their own state and join the restored cluster clean. A source that points at S3 needs
/// the `s3` feature. Restoring wipes what the node has today - it is a deliberate act, and it
/// happens before the node writes anything.
#[cfg(feature = "backup")]
pub async fn start_node_from_backup(
    node_config: NodeConfig,
    backup: BackupSource,
) -> Result<Client, Error> {
    start::start_node_inner::<empty::Empty>(Box::new(node_config), Some(backup)).await
}

/// Start a Raft / Hiqlite node with the `cache` feature and restore the given backup before the
/// Raft boots.
///
/// Same as [`start_node_from_backup`], with the cache variant enum of [`start_node_with_cache`].
/// The restore wipes the caches as well - they are rebuilt from the restored Raft log, and
/// cache-only data that was in them is gone.
#[cfg(all(feature = "backup", feature = "cache"))]
pub async fn start_node_with_cache_from_backup<C>(
    node_config: NodeConfig,
    backup: BackupSource,
) -> Result<Client, Error>
where
    C: Debug + CacheVariants,
{
    start::start_node_inner::<C>(Box::new(node_config), Some(backup)).await
}
