use crate::app_state::AppState;
use crate::client::DbClient;
use crate::{tls, Client, Error};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

#[cfg(feature = "listen_notify")]
use crate::client::listen_notify::RemoteListener;

#[cfg(feature = "sqlite")]
use crate::client::stream::ClientStreamReq;

impl Client {
    /// Create a local client that skips network connections if not necessary
    pub(crate) async fn new_local(
        state: Arc<AppState>,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        #[cfg(feature = "sqlite")] tx_client_db: flume::Sender<ClientStreamReq>,
        #[cfg(feature = "sqlite")] rx_client_db: flume::Receiver<ClientStreamReq>,
        tx_shutdown: watch::Sender<bool>,
    ) -> Self {
        let leader_id = state.id;
        let leader_addr = state.addr_api.clone();

        let secret = state.secret_api.as_bytes().to_vec();

        #[cfg(feature = "cache")]
        let leader_cache = Arc::new(RwLock::new((leader_id, leader_addr.clone())));
        #[cfg(feature = "sqlite")]
        let leader_db = Arc::new(RwLock::new((leader_id, leader_addr)));

        #[cfg(feature = "cache")]
        let (tx_client_cache, rx_client_cache) = flume::unbounded();

        // #[cfg(feature = "cache")]
        // Self::open_stream(
        //     tls_config.clone(),
        //     secret.as_bytes().to_vec(),
        //     leader_cache.clone(),
        //     rx_client_cache,
        // );
        //
        // #[cfg(feature = "sqlite")]
        // Self::open_stream(
        //     tls_config.clone(),
        //     secret.as_bytes().to_vec(),
        //     leader_db.clone(),
        //     rx_client_db,
        // );

        let db_client = DbClient {
            state: Some(state),
            #[cfg(feature = "cache")]
            leader_cache,
            #[cfg(feature = "sqlite")]
            leader_db,
            // TODO do we even still need this for a local client? -> all raft messages should use internal API ?
            nodes: Vec::default(),
            client: None,
            // client: reqwest::Client::builder()
            //     .http2_prior_knowledge()
            //     // TODO
            //     // .danger_accept_invalid_certs(tls_config.as_ref().map(|c| c.))
            //     .build()
            //     .unwrap(),
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
            #[cfg(feature = "listen_notify")]
            rx_notify: None,
        };

        let slf = Self {
            inner: Arc::new(db_client),
        };

        slf.find_set_active_leader().await;

        #[cfg(feature = "cache")]
        slf.open_stream(
            secret.clone(),
            slf.inner.leader_cache.clone(),
            rx_client_cache,
        );
        #[cfg(feature = "sqlite")]
        slf.open_stream(secret, slf.inner.leader_db.clone(), rx_client_db);

        slf
    }

    /// Create a remote client
    ///
    /// Provide any node as address. As long as all nodes can be reached,
    /// leader changes will happen automatically.
    pub async fn remote(
        nodes: Vec<String>,
        tls: bool,
        tls_no_verify: bool,
        api_secret: String,
        with_proxy: bool,
    ) -> Result<Self, Error> {
        if nodes.is_empty() {
            return Err(Error::Config(
                "You must provide at least 1 node to connect to".into(),
            ));
        }

        let tls_config = if tls {
            Some(tls::build_tls_config(tls_no_verify))
        } else {
            None
        };

        // we just use this as a placeholder to be able to initialize the remote note
        let node_id = 0;
        let node_addr = nodes[0].clone();

        #[cfg(feature = "cache")]
        let leader_cache = Arc::new(RwLock::new((node_id, node_addr.clone())));
        #[cfg(feature = "sqlite")]
        let leader_db = Arc::new(RwLock::new((node_id, node_addr)));

        #[cfg(feature = "sqlite")]
        let (tx_client_db, rx_client_db) = flume::unbounded();
        #[cfg(feature = "cache")]
        let (tx_client_cache, rx_client_cache) = flume::unbounded();

        #[cfg(feature = "listen_notify")]
        let rx_notify = Some(RemoteListener::spawn(
            leader_cache.clone(),
            tls,
            api_secret.clone(),
        ));

        // #[cfg(feature = "cache")]
        // Self::open_stream(
        //     tls_config.clone(),
        //     api_secret.as_bytes().to_vec(),
        //     leader_cache.clone(),
        //     rx_client_cache,
        // );
        //
        // #[cfg(feature = "sqlite")]
        // Self::open_stream(
        //     tls_config.clone(),
        //     api_secret.as_bytes().to_vec(),
        //     leader_db.clone(),
        //     rx_client_db,
        // );

        let api_secret_bytes = api_secret.as_bytes().to_vec();

        let db_client = DbClient {
            state: None,
            #[cfg(feature = "sqlite")]
            leader_db,
            #[cfg(feature = "cache")]
            leader_cache,
            nodes,
            client: Some(
                reqwest::Client::builder()
                    // .user_agent("Raft Client")
                    .http2_prior_knowledge()
                    // TODO
                    // .danger_accept_invalid_certs()
                    .build()
                    .unwrap(),
            ),
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
            #[cfg(feature = "listen_notify")]
            rx_notify,
        };

        let slf = Self {
            inner: Arc::new(db_client),
        };

        // It should be enough to check for DB proxy here. When running, the forward to leader
        // errors should never be forwarded through the proxy.
        if !with_proxy {
            slf.find_set_active_leader().await;
        }

        #[cfg(feature = "cache")]
        slf.open_stream(
            api_secret_bytes.clone(),
            slf.inner.leader_cache.clone(),
            rx_client_cache,
        );
        #[cfg(feature = "sqlite")]
        slf.open_stream(api_secret_bytes, slf.inner.leader_db.clone(), rx_client_db);

        Ok(slf)
    }
}
