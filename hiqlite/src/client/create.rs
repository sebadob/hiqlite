use crate::app_state::AppState;
use crate::client::DbClient;
use crate::{tls, Client, NodeId};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

#[cfg(feature = "sqlite")]
use crate::client::stream::ClientStreamReq;

impl Client {
    /// Create a local client that skips network connections if not necessary
    pub(crate) fn new_local(
        state: Arc<AppState>,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        #[cfg(feature = "sqlite")] tx_client_db: flume::Sender<ClientStreamReq>,
        #[cfg(feature = "sqlite")] rx_client_db: flume::Receiver<ClientStreamReq>,
        tx_shutdown: watch::Sender<bool>,
    ) -> Self {
        let leader_id = state.id;
        let leader_addr = state.addr_api.clone();

        let node_id = state.id;
        let secret = state.secret_api.clone();

        #[cfg(feature = "cache")]
        let leader_cache = Arc::new(RwLock::new((leader_id, leader_addr.clone())));
        #[cfg(feature = "sqlite")]
        let leader_db = Arc::new(RwLock::new((leader_id, leader_addr)));

        #[cfg(feature = "cache")]
        let (tx_client_cache, rx_client_cache) = flume::unbounded();

        #[cfg(feature = "cache")]
        Self::open_stream(
            node_id,
            tls_config.clone(),
            secret.as_bytes().to_vec(),
            leader_cache.clone(),
            rx_client_cache,
        );

        #[cfg(feature = "sqlite")]
        Self::open_stream(
            node_id,
            tls_config.clone(),
            secret.as_bytes().to_vec(),
            leader_db.clone(),
            rx_client_db,
        );

        let db_client = DbClient {
            state: Some(state),
            #[cfg(feature = "cache")]
            leader_cache,
            #[cfg(feature = "sqlite")]
            leader_db,
            // TODO do we even still need this for a local client? -> all raft messages should use internal API ?
            client: reqwest::Client::builder()
                .http2_prior_knowledge()
                // TODO
                // .danger_accept_invalid_certs(tls_config.as_ref().map(|c| c.))
                .build()
                .unwrap(),
            #[cfg(feature = "cache")]
            tx_client_cache,
            #[cfg(feature = "sqlite")]
            tx_client_db,
            tls_config,
            api_secret: None,
            request_id: AtomicUsize::new(0),
            tx_shutdown: Some(tx_shutdown),
            #[cfg(feature = "listen_notify")]
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

        #[cfg(feature = "cache")]
        let leader_cache = Arc::new(RwLock::new((node_id, node_addr.clone())));
        #[cfg(feature = "sqlite")]
        let leader_db = Arc::new(RwLock::new((node_id, node_addr)));

        #[cfg(feature = "sqlite")]
        let (tx_client_db, rx_client_db) = flume::unbounded();
        #[cfg(feature = "cache")]
        let (tx_client_cache, rx_client_cache) = flume::unbounded();

        #[cfg(feature = "cache")]
        Self::open_stream(
            node_id,
            tls_config.clone(),
            api_secret.as_bytes().to_vec(),
            leader_cache.clone(),
            rx_client_cache,
        );

        #[cfg(feature = "sqlite")]
        Self::open_stream(
            node_id,
            tls_config.clone(),
            api_secret.as_bytes().to_vec(),
            leader_db.clone(),
            rx_client_db,
        );

        let db_client = DbClient {
            state: None,
            #[cfg(feature = "sqlite")]
            leader_db,
            #[cfg(feature = "cache")]
            leader_cache,
            client: reqwest::Client::builder()
                // .user_agent("Raft Client")
                .http2_prior_knowledge()
                // TODO
                // .danger_accept_invalid_certs()
                .build()
                .unwrap(),
            #[cfg(feature = "cache")]
            tx_client_cache,
            #[cfg(feature = "sqlite")]
            tx_client_db,
            tls_config,
            api_secret: Some(api_secret),
            request_id: AtomicUsize::new(0),
            tx_shutdown: None,
            #[cfg(feature = "listen_notify")]
            app_start: chrono::Utc::now().timestamp_micros(),
        };

        Self {
            inner: Arc::new(db_client),
        }
    }
}
