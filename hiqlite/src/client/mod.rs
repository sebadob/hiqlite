use crate::app_state::AppState;
use crate::NodeId;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use stream::ClientStreamReq;
use tokio::sync::{watch, RwLock};

mod batch;
mod create;
mod execute;
mod helpers;
mod mgmt;
mod migrate;
mod query;
pub mod stream;
mod transaction;

#[cfg(feature = "backup")]
mod backup;
#[cfg(feature = "cache")]
mod cache;
#[cfg(feature = "dlock")]
pub(crate) mod dlock;
#[cfg(feature = "cache")]
mod listen_notify;
#[cfg(feature = "shutdown-handle")]
mod shutdown_handle;

/// Database client
#[derive(Clone)]
pub struct Client {
    pub(crate) inner: Arc<DbClient>,
}

pub(crate) struct DbClient {
    pub(crate) state: Option<Arc<AppState>>,
    // TODO maybe we can get rid of the Arc here with a mod of client stream
    pub(crate) leader: Arc<RwLock<(NodeId, String)>>,
    pub(crate) client: reqwest::Client,
    pub(crate) tx_client: flume::Sender<ClientStreamReq>,
    pub(crate) tls_config: Option<Arc<rustls::ClientConfig>>,
    // Only remote clients will have `Some(_)` here -> local ones have the state
    pub(crate) api_secret: Option<String>,
    pub(crate) request_id: AtomicUsize,
    pub(crate) tx_shutdown: Option<watch::Sender<bool>>,
    #[cfg(feature = "cache")]
    pub(crate) app_start: i64,
}
