use crate::app_state::AppState;
use crate::client_stream::ClientStreamReq;
use crate::NodeId;
use reqwest::Client;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

mod backup;
mod batch;
mod create;
mod execute;
mod helpers;
mod mgmt;
mod migrate;
mod query;
mod transaction;

/// Database client
#[derive(Clone)]
pub struct DbClient {
    pub(crate) state: Option<Arc<AppState>>,
    pub(crate) leader: Arc<RwLock<(NodeId, String)>>,
    pub(crate) client: Arc<Client>,
    pub(crate) tx_client: flume::Sender<ClientStreamReq>,
    pub(crate) tls_config: Option<Arc<rustls::ClientConfig>>,
    // Only remote clients will have `Some(_)` here -> makes the client cheaper to clone
    pub(crate) api_secret: Option<String>,
    pub(crate) request_id: Arc<AtomicUsize>,
    pub(crate) tx_shutdown: Option<watch::Sender<bool>>,
}
