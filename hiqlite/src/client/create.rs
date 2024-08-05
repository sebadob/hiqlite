use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::client::DbClient;
use crate::{tls, Client, NodeId};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

impl Client {
    /// Create a local client that skips network connections if not necessary
    pub(crate) fn new_local(
        state: Arc<AppState>,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        tx_client: flume::Sender<ClientStreamReq>,
        rx_client: flume::Receiver<ClientStreamReq>,
        tx_shutdown: watch::Sender<bool>,
    ) -> Self {
        let leader_id = state.id;
        let leader_addr = state.addr_api.clone();

        let node_id = state.id;
        let secret = state.secret_api.clone();
        let leader = Arc::new(RwLock::new((leader_id, leader_addr)));
        Self::open_stream(
            node_id,
            tls_config.clone(),
            secret.as_bytes().to_vec(),
            leader.clone(),
            rx_client,
        );

        let db_client = DbClient {
            state: Some(state),
            leader,
            // TODO do we even still need this for a local client? -> all raft messages should use internal API ?
            client: reqwest::Client::builder()
                .http2_prior_knowledge()
                // TODO
                // .danger_accept_invalid_certs(tls_config.as_ref().map(|c| c.))
                .build()
                .unwrap(),
            tx_client,
            tls_config,
            api_secret: None,
            request_id: AtomicUsize::new(0),
            tx_shutdown: Some(tx_shutdown),
            #[cfg(feature = "cache")]
            app_start: chrono::Utc::now().timestamp_micros(),
        };

        Self {
            inner: Arc::new(db_client),
        }
    }

    /// Create a remote client
    ///
    /// Provide any node as address. As long as all nodes can be reached,
    /// leader changes will happen automatically.
    pub fn remote(
        node_id: NodeId,
        node_addr: String,
        tls: bool,
        tls_no_verify: bool,
        api_secret: String,
    ) -> Self {
        let tls_config = if tls {
            Some(tls::build_tls_config(tls_no_verify))
        } else {
            None
        };

        let leader = Arc::new(RwLock::new((node_id, node_addr)));
        let (tx_client, rx_client) = flume::unbounded();
        Self::open_stream(
            node_id,
            tls_config.clone(),
            api_secret.as_bytes().to_vec(),
            leader.clone(),
            rx_client,
        );

        let db_client = DbClient {
            state: None,
            leader,
            client: reqwest::Client::builder()
                // .user_agent("Raft Client")
                .http2_prior_knowledge()
                // TODO
                // .danger_accept_invalid_certs()
                .build()
                .unwrap(),
            tx_client,
            tls_config,
            api_secret: Some(api_secret),
            request_id: AtomicUsize::new(0),
            tx_shutdown: None,
            #[cfg(feature = "cache")]
            app_start: chrono::Utc::now().timestamp_micros(),
        };

        Self {
            inner: Arc::new(db_client),
        }
    }
}
