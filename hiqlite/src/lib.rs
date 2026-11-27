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

// Exactly one rustls crypto backend. Both would give rustls two providers, and TLS compiled in with
// neither leaves the Raft / API stack with a TLS layer that cannot connect. `fips` is aws-lc-rs only.
#[cfg(all(feature = "ring", feature = "aws_lc_rs"))]
compile_error!("features `ring` and `aws_lc_rs` are mutually exclusive, pick exactly one");

#[cfg(all(feature = "fips", feature = "ring"))]
compile_error!("feature `fips` needs `aws_lc_rs`, not `ring`");

#[cfg(all(
    feature = "__cluster",
    not(feature = "ring"),
    not(feature = "aws_lc_rs")
))]
compile_error!("TLS needs exactly one of `ring` or `aws_lc_rs`");

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
use std::num::NonZeroUsize;
use std::thread;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::state_machine::Response;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use crate::{client::Client, error::Error};
#[cfg(feature = "backup")]
pub use backup::{BackupSource, DEFAULT_BACKUP_FILE_PREFIX};
#[cfg(feature = "config-embed")]
pub use config::REFERENCE_CONFIG;
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use config::{
    DEFAULT_SHUTDOWN_DELAY, EncKeysArg, MIN_CLUSTER_SHUTDOWN_DELAY, NodeConfig, RaftConfig,
    RateLimitConfig, SHUTDOWN_TEARDOWN_BUDGET,
};
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub use network::management::{ElectionState, MembersState, NodeElection, NodeMembers};
#[cfg(feature = "sqlite")]
pub use query::cust_types::VecText;

#[cfg(feature = "sqlite")]
pub use crate::query::rows::{
    ColIndex, FieldMaps, FieldNames, FromRowIndexed, Row, RowOwned, SqlShape, TryFromRowIndexed,
    register_hoisted, register_hoisted_try, sql_shape,
};
#[cfg(feature = "sqlite")]
pub use crate::query::stream::{JsonLinesExt, QueryStream};
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
#[cfg(any(feature = "sqlite", feature = "external-state-machine"))]
pub use rusqlite;

/// Re-export of the exact `serde_rusqlite` version Hiqlite is built with.
#[cfg(feature = "sqlite")]
pub use serde_rusqlite;

/// Re-export of the `StreamExt` combinators of the exact `futures-util` version Hiqlite is built
/// with.
///
/// A [`QueryStream`] is a `futures_util::stream::Stream`, so `.map()`, `.take()`, `.filter_map()`
/// and the rest of the combinators work on it, and importing this trait is all it takes to reach
/// them - no `futures-util` dependency of your own.  A plain row loop needs neither: the inherent
/// [`QueryStream::next`] is shadowing `StreamExt::next`, and it answers with a `Result`.
#[cfg(feature = "sqlite")]
pub use futures_util::StreamExt;

// No doc comment here on purpose: an outer doc merges with the `//!` block of the module, and
// rustdoc then resolves the intra-doc links of that block at crate root, where the module's items
// are not in scope. All of this module's docs live in `external_state_machine.rs`.
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

/// The threads that own the read connections of both SQLite state machines: one thread per
/// connection, all of them serving one channel of `reader::ReaderRequest`.
#[cfg(any(feature = "sqlite", feature = "external-state-machine"))]
mod reader;

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

/// The highest `read_pool_size` either state machine accepts.  Each read connection is one OS
/// thread with its own SQLite handle, about 170 KB of RSS plus the stack of the thread that owns it.
pub(crate) const MAX_READ_POOL_SIZE: usize = 1024;

/// The smallest default `read_pool_size`: the fixed pool size a node ran with before the count
/// followed the core count.  It is a floor for the default only - an operator may set
/// `read_pool_size` as low as 1.
const READ_POOL_SIZE_FLOOR: usize = 4;

/// The default count of read actors: one read connection per core.
///
/// `available_parallelism` respects a cgroup CPU limit, so a CPU-limited container gets an actor
/// count that matches the cores it may actually run on.  The floor is the old fixed pool size of
/// 4: a 1-core or 2-core pod keeps the number of read connections it always had, because two
/// parked reads on two connections block every other local read on that node.
pub(crate) fn default_read_pool_size() -> usize {
    thread::available_parallelism()
        .map_or(READ_POOL_SIZE_FLOOR, NonZeroUsize::get)
        .clamp(READ_POOL_SIZE_FLOOR, MAX_READ_POOL_SIZE)
}

#[cfg(any(feature = "sqlite", feature = "cache"))]
#[doc(hidden)]
pub use phf;

/// The caches of a node, usually an enum with `#[derive(CacheVariants)]`.
///
/// The cluster identifies a cache by its variant name: the raft log and the snapshots carry the
/// name, and each node maps it to its own index. Reordering the variants is therefore safe. A
/// renamed variant is a new, empty cache, and the entries of the old name stay in the cluster until
/// they expire or a `clear_cache_all()`.
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub trait CacheVariants {
    /// Returns the Enum Variants index, strictly matching the output of `hiqlite_cache_variants()`.
    fn hiqlite_cache_index(&self) -> usize;

    /// Returns the Enum Variants as `(idx, name)` in strictly ascending order, starting at `0`.
    fn hiqlite_cache_variants() -> &'static [(usize, &'static str)];

    /// Returns the map from each variant name to its index in `hiqlite_cache_variants()`.
    fn hiqlite_cache_map() -> &'static phf::Map<&'static str, usize>;

    /// Returns the variant name, which identifies the cache in the cluster.
    #[must_use]
    fn hiqlite_cache_name(&self) -> &'static str {
        Self::hiqlite_cache_variants()[self.hiqlite_cache_index()].1
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

        fn hiqlite_cache_map() -> &'static phf::Map<&'static str, usize> {
            static MAP: phf::Map<&'static str, usize> = phf::Map::new();
            &MAP
        }
    }
}

/// The main entry function to start a Raft / Hiqlite node.
///
/// # Errors
///
/// Anything that keeps the node from booting: [`Error::Config`] for a node config and Raft
/// config that does not validate and for a `node_id` no node line carries, [`Error::WAL`]
/// and [`Error::Error`] for the log directory, its lock file and the snapshots,
/// [`Error::InitializeErrorDb`] from `openraft` while the cluster bootstraps, and the
/// [`Error`] of the backup restore when one is asked for.
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
///
/// # Errors
///
/// Anything that keeps the node from booting: [`Error::Config`] for a node config and Raft
/// config that does not validate and for a `node_id` no node line carries, [`Error::WAL`]
/// and [`Error::Error`] for the log directory, its lock file and the snapshots,
/// [`Error::InitializeErrorDb`] from `openraft` while the cluster bootstraps, and the
/// [`Error`] of the backup restore when one is asked for.
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
///
/// # Errors
///
/// Anything that keeps the node from booting: [`Error::Config`] for a node config and Raft
/// config that does not validate and for a `node_id` no node line carries, [`Error::WAL`]
/// and [`Error::Error`] for the log directory, its lock file and the snapshots,
/// [`Error::InitializeErrorDb`] from `openraft` while the cluster bootstraps, and the
/// [`Error`] of the backup restore when one is asked for.
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
///
/// # Errors
///
/// Anything that keeps the node from booting: [`Error::Config`] for a node config and Raft
/// config that does not validate and for a `node_id` no node line carries, [`Error::WAL`]
/// and [`Error::Error`] for the log directory, its lock file and the snapshots,
/// [`Error::InitializeErrorDb`] from `openraft` while the cluster bootstraps, and the
/// [`Error`] of the backup restore when one is asked for.
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
