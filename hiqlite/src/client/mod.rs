use crate::app_state::AppState;
use crate::{Error, NodeId};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use stream::ClientStreamReq;
use tokio::sync::{RwLock, oneshot, watch};

#[cfg(feature = "backup")]
mod backup;
#[cfg(feature = "sqlite")]
mod batch;
#[cfg(feature = "cache")]
mod cache;
mod create;
#[cfg(feature = "dlock")]
pub mod dlock;
#[cfg(feature = "sqlite")]
mod execute;
mod helpers;
#[cfg(feature = "listen_notify")]
mod listen_notify;
mod mgmt;
#[cfg(feature = "sqlite")]
mod migrate;
#[cfg(feature = "sqlite")]
mod query;
mod rate_limit;
#[cfg(feature = "shutdown-handle")]
mod shutdown_handle;
pub mod stream;
#[cfg(feature = "sqlite")]
mod transaction;

pub(crate) type ShutdownResult = Result<(), String>;

/// The main database client.
///
/// It will handle all things you need to work with the Database / Cache / Event Bus /
/// Distributed Locks. It wraps all inner data inside an internal `Arc<_>`, which means it's very
/// cheap to clone directly.
#[derive(Clone)]
pub struct Client {
    pub(crate) inner: Arc<DbClient>,
}

pub(crate) struct DbClient {
    pub(crate) state: Option<Arc<AppState>>,
    #[cfg(feature = "cache")]
    pub(crate) leader_cache: Arc<RwLock<(NodeId, String)>>,
    /// Claims the leader lookup of the cache group, for the same reason as the DB group's.
    #[cfg(feature = "cache")]
    pub(crate) leader_lookup_cache: AtomicBool,
    #[cfg(feature = "sqlite")]
    pub(crate) leader_db: Arc<RwLock<(NodeId, String)>>,
    /// Claims the leader lookup of the DB group. A leader id changes rarely, so a burst of writes
    /// that all land in the same leadership gap must not each run a lookup: the writer that claims
    /// it runs the one lookup, and the others go straight back to the cached leader. Claiming is a
    /// single atomic swap, so nothing in the write path blocks.
    #[cfg(feature = "sqlite")]
    pub(crate) leader_lookup_db: AtomicBool,

    /// This is always empty for local clients, and carries the actual nodes for remote clients.
    pub(crate) nodes: Vec<String>,
    pub(crate) client: Option<reqwest::Client>,
    #[cfg(feature = "cache")]
    pub(crate) tx_client_cache: flume::Sender<ClientStreamReq>,
    #[cfg(feature = "sqlite")]
    pub(crate) tx_client_db: flume::Sender<ClientStreamReq>,
    pub(crate) tls_config: Option<Arc<rustls::ClientConfig>>,
    pub(crate) tls_no_verify: bool,
    pub(crate) api_secret: Option<String>,
    pub(crate) request_id: AtomicUsize,
    pub(crate) tx_shutdown: Option<watch::Sender<Option<ShutdownResult>>>,
    #[cfg(feature = "listen_notify")]
    pub(crate) app_start: i64,
    #[cfg(feature = "listen_notify")]
    pub(crate) rx_notify: Option<flume::Receiver<(i64, Vec<u8>)>>,
    #[cfg(feature = "cache")]
    pub(crate) rate_limit_cache: Option<AtomicU32>,
    #[cfg(feature = "cache")]
    pub(crate) rate_limit_cache_await:
        crossbeam::channel::Sender<oneshot::Sender<Result<(), Error>>>,
    #[cfg(feature = "sqlite")]
    pub(crate) rate_limit_db: Option<AtomicU32>,
    #[cfg(feature = "sqlite")]
    pub(crate) rate_limit_db_await: crossbeam::channel::Sender<oneshot::Sender<Result<(), Error>>>,
}
