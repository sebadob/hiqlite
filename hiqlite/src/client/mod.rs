use crate::app_state::AppState;
use crate::NodeId;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use stream::ClientStreamReq;
use tokio::sync::{watch, RwLock};

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
#[cfg(feature = "listen_notify_local")]
mod listen_notify;
mod mgmt;
#[cfg(feature = "sqlite")]
mod migrate;
#[cfg(feature = "sqlite")]
mod query;
#[cfg(feature = "shutdown-handle")]
mod shutdown_handle;
pub mod stream;
#[cfg(feature = "sqlite")]
mod transaction;

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
    #[cfg(feature = "sqlite")]
    pub(crate) leader_db: Arc<RwLock<(NodeId, String)>>,
    pub(crate) nodes: Vec<String>,
    pub(crate) client: Option<reqwest::Client>,
    #[cfg(feature = "cache")]
    pub(crate) tx_client_cache: flume::Sender<ClientStreamReq>,
    #[cfg(feature = "sqlite")]
    pub(crate) tx_client_db: flume::Sender<ClientStreamReq>,
    pub(crate) tls_config: Option<Arc<rustls::ClientConfig>>,
    pub(crate) api_secret: Option<String>,
    pub(crate) request_id: AtomicUsize,
    pub(crate) tx_shutdown: Option<watch::Sender<bool>>,
    #[cfg(feature = "listen_notify_local")]
    pub(crate) app_start: i64,
    #[cfg(feature = "listen_notify_local")]
    pub(crate) rx_notify: Option<flume::Receiver<(i64, Vec<u8>)>>,
}
