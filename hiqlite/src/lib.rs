// Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../../README.md")]
#![forbid(unsafe_code)]

use crate::app_state::AppState;
use crate::network::raft_server;
use crate::network::NetworkStreaming;
use crate::network::{api, management};
use crate::store::new_storage;
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Display;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::{task, time};
use tracing::info;

pub use crate::client::DbClient;
pub use crate::error::Error;
pub use crate::store::state_machine::sqlite::state_machine::{Params, Response};
pub use config::{NodeConfig, RaftConfig};
pub use openraft::SnapshotPolicy;
pub use rusqlite::Row;
pub use store::state_machine::sqlite::param::Param;
pub use tls::ServerTlsConfig;

mod app_state;
mod client;
mod client_stream;
mod config;
mod error;
mod migration;
mod network;
mod store;
mod tls;

type NodeId = u64;

// use mac

// hiqlite_macros::embed_migrations!("../../migrations");

/// Create params for distributed SQL modifying queries.
/// TODO create multiple branches here to be able to catch the correct sizes
/// with upfront capacity assignment earlier.
#[macro_export]
macro_rules! params {
    ( $( $param:expr ),* ) => {
        {
            let mut params = Vec::new();
            $(
                params.push(Param::from($param));
            )*
            params
        }
    };
}

/// A Raft member node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Node {
    pub id: NodeId,
    pub addr_raft: String,
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

// #[cfg(feature = "sqlite")]
// openraft::declare_raft_types!(
//     pub TypeConfigSqliteSqlite:
//         D = Query,
//         R = Response,
//         Node = Node,
//         SnapshotData = tokio::fs::File,
// );

/// Starts a Raft node.
/// # Panics
/// If an incorrect `node_config` was given.
pub async fn start_node(node_config: NodeConfig, auto_init: bool) -> Result<DbClient, Error> {
    node_config.is_valid()?;

    if node_config.tls_api.is_some() || node_config.tls_raft.is_some() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("default CryptoProvider installation to succeed");
    }

    let raft_config = Arc::new(node_config.config.validate().unwrap());

    // let (log_store, state_machine_store) =
    //     new_storage(&node_config.data_dir, node_config.filename_db.as_deref()).await;
    let (log_store, state_machine_store) = new_storage(
        node_config.node_id,
        node_config.data_dir,
        node_config.filename_db,
    )
    .await;

    // let kv_store = state_machine_store.data.kvs.clone();
    let sql_writer = state_machine_store.write_tx.clone();
    let sql_reader = state_machine_store.read_pool.clone();

    // Create the network layer that will connect and communicate the raft instances and
    // will be used in conjunction with the store created above.
    let network = NetworkStreaming {
        // let network = Network {
        node_id: node_config.node_id,
        tls_config: node_config.tls_raft.as_ref().map(|tls| tls.client_config()),
        secret_raft: node_config.secret_raft.as_bytes().to_vec(),
    };

    // Create a local raft instance.
    let raft = openraft::Raft::new(
        node_config.node_id,
        raft_config.clone(),
        network,
        log_store,
        state_machine_store,
    )
    .await
    .expect("Raft create failed");

    let (api_addr, rpc_addr) = {
        let node = node_config
            .nodes
            .get(node_config.node_id as usize - 1)
            .expect("NodeConfig.node_id not found in NodeConfig.nodes");
        (node.addr_api.clone(), node.addr_raft.clone())
    };

    let mut client_buffers = HashMap::new();
    for node in &node_config.nodes {
        // if node.id != node_config.node_id {
        let (tx, rx) = flume::unbounded();
        client_buffers.insert(node.id, (tx, rx));
        // }
    }

    let state = Arc::new(AppState {
        id: node_config.node_id,
        addr_api: api_addr.clone(),
        addr_raft: rpc_addr.clone(),
        raft,
        read_pool: sql_reader,
        sql_writer,
        // kv_store,
        config: raft_config,
        secret_api: node_config.secret_api,
        secret_raft: node_config.secret_raft,
        client_buffers,
    });

    // let compression_middleware = ServiceBuilder::new().layer(CompressionLayer::new());

    let (tx_shutdown, rx_shutdown) = watch::channel(false);

    let router_internal = Router::new()
        .route("/stream", get(raft_server::stream))
        .route("/ping", get(api::ping))
        // .layer(compression_middleware.clone().into_inner())
        .with_state(state.clone());

    info!("rpc internal listening on {}", &rpc_addr);

    let tls_config = if let Some(config) = &node_config.tls_raft {
        Some(config.server_config().await)
    } else {
        None
    };
    let shutdown = shutdown_signal(rx_shutdown.clone());
    let _handle_internal = task::spawn(async move {
        if let Some(config) = tls_config {
            let addr = SocketAddr::from_str(&rpc_addr).expect("valid RPC socket address");
            // TODO find a way to do a graceful shutdown with `axum_server` or to handle TLS
            // properly with axum directly
            axum_server::bind_rustls(addr, config)
                .serve(router_internal.into_make_service())
                .await
                .unwrap();
        } else {
            let listener = TcpListener::bind(rpc_addr)
                .await
                .expect("valid RPC socket address");
            axum::serve(listener, router_internal.into_make_service())
                .with_graceful_shutdown(shutdown)
                .await
                .unwrap()
        }
    });

    let router_api = Router::new()
        .nest(
            "/cluster",
            Router::new()
                .route("/add-learner", post(management::add_learner))
                .route("/change-membership", post(management::change_membership))
                .route("/init", post(management::init))
                .route("/metrics", get(management::metrics)),
        )
        .route("/execute", post(api::execute))
        .route("/query", post(api::query))
        .route("/query/consistent", post(api::query))
        .route("/stream", get(api::stream))
        .route("/ping", get(api::ping))
        // .layer(compression_middleware.clone().into_inner())
        .with_state(state.clone());

    info!("api external listening on {}", &api_addr);
    let tls_config = if let Some(config) = &node_config.tls_api {
        Some(config.server_config().await)
    } else {
        None
    };
    let _handle_external = task::spawn(async move {
        if let Some(config) = tls_config {
            let addr = SocketAddr::from_str(&api_addr).expect("valid RPC socket address");
            // TODO find a way to do a graceful shutdown with `axum_server` or to handle TLS
            // properly with axum directly
            axum_server::bind_rustls(addr, config)
                .serve(router_api.into_make_service())
                .await
                .unwrap();
        } else {
            let listener = TcpListener::bind(api_addr)
                .await
                .expect("valid RPC socket address");
            axum::serve(listener, router_api.into_make_service())
                .with_graceful_shutdown(shutdown_signal(rx_shutdown))
                .await
                .unwrap()
        }
    });

    if auto_init {
        init_cluster(
            state.clone(),
            node_config.nodes,
            node_config.tls_raft.is_some(),
            node_config
                .tls_raft
                .map(|c| c.danger_tls_no_verify)
                .unwrap_or(false),
        )
        .await?;
    }

    let client = DbClient::new_local(
        state,
        node_config.tls_api.map(|c| c.client_config()),
        tx_shutdown,
    );

    Ok(client)
}

async fn shutdown_signal(mut rx: watch::Receiver<bool>) {
    let _ = rx.changed().await;
}

async fn init_cluster(
    state: Arc<AppState>,
    nodes: Vec<Node>,
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    // After the start, the cluster must be initialized.
    if state.id == 1 {
        // Do not try to initialize already initialized nodes
        if state.raft.is_initialized().await? {
            return Ok(());
        }

        // If it is not initialized, wait long enough to make sure this
        // node is not joined again to an already existing cluster after data loss.
        let heartbeat = state.raft.config().heartbeat_interval;
        // We will wait for 5 heartbeats to make sure no other cluster is running
        time::sleep(Duration::from_millis(heartbeat * 5)).await;

        // Make sure we are not initialized by now, otherwise go on
        if state.raft.is_initialized().await? {
            return Ok(());
        }

        wait_for_nodes_online(&state, &nodes, tls, tls_no_verify).await;

        let mut nodes_set = BTreeMap::new();
        for node in nodes {
            nodes_set.insert(node.id, node);
        }
        state.raft.initialize(nodes_set).await?;
    }

    Ok(())
}

async fn wait_for_nodes_online(state: &AppState, nodes: &[Node], tls: bool, tls_no_verify: bool) {
    let scheme = if tls { "https" } else { "http" };
    let remotes = nodes
        .iter()
        .filter_map(|node| {
            (node.id != state.id).then_some(format!("{}://{}/ping", scheme, node.addr_raft))
        })
        .collect::<Vec<String>>();
    let mut remotes_online = 0;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(tls_no_verify)
        .http2_prior_knowledge()
        .build()
        .unwrap();
    while remotes_online != remotes.len() {
        info!("Waiting for remote nodes {:?} to become reachable", remotes);

        remotes_online = 0;
        time::sleep(Duration::from_secs(1)).await;

        for node in &remotes {
            if client.get(node).send().await.is_ok() {
                remotes_online += 1;
            }
        }
    }

    info!("All remote nodes are reachable");
}
