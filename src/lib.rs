// Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>

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
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::{task, time};
use tracing::info;

// #[cfg(feature = "kv")]
// pub use crate::store::state_machine_memory::{Request, Response};

pub use crate::client::DbClient;
pub use crate::error::Error;
pub use crate::store::state_machine::sqlite::state_machine::{Params, Response};
pub use config::{NodeConfig, RaftConfig};
pub use openraft::SnapshotPolicy;
pub use rusqlite::Row;
pub use store::state_machine::sqlite::param::Param;

mod app_state;
mod client;
mod client_stream;
mod config;
mod error;
mod network;
mod store;

type NodeId = u64;

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

    let raft_config = Arc::new(node_config.config.validate().unwrap());

    // let (log_store, state_machine_store) =
    //     new_storage(&node_config.data_dir, node_config.filename_db.as_deref()).await;
    let (log_store, state_machine_store) =
        new_storage(node_config.data_dir, node_config.filename_db).await;

    // let kv_store = state_machine_store.data.kvs.clone();
    let sql_writer = state_machine_store.write_tx.clone();
    let sql_reader = state_machine_store.read_pool.clone();

    // Create the network layer that will connect and communicate the raft instances and
    // will be used in conjunction with the store created above.
    let network = NetworkStreaming {
        // let network = Network {
        node_id: node_config.node_id,
        tls: node_config.tls,
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

    // if let Ok(path) = env::var("TLS_CERT") {
    //     let key = env::var("TLS_KEY").expect("TLS_KEY is missing");
    //     let config = RustlsConfig::from_pem_file(PathBuf::from(path), PathBuf::from(key))
    //         .await
    //         .unwrap();
    //
    //     TLS.set(true).unwrap();
    //
    //     info!("listening on https://{}", &http_addr);
    //     axum_server::bind_rustls(http_addr, config)
    //         .serve(app.into_make_service_with_connect_info::<SocketAddr>())
    //         .await
    //         .unwrap();
    // } else {
    //     TLS.set(false).unwrap();

    let (tx_shutdown, rx_shutdown) = watch::channel(false);

    let router_internal = Router::new()
        .route("/stream", get(raft_server::stream))
        // .layer(compression_middleware.clone().into_inner())
        .with_state(state.clone());

    info!("rpc internal listening on {}", &rpc_addr);
    let listener = TcpListener::bind(rpc_addr).await?;
    let shutdown = shutdown_signal(rx_shutdown.clone());
    let _handle_internal = task::spawn(async move {
        axum::serve(listener, router_internal.into_make_service())
            .with_graceful_shutdown(shutdown)
            .await
            .unwrap()
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
    let listener = TcpListener::bind(api_addr).await?;
    let _handle_external = task::spawn(async move {
        axum::serve(listener, router_api.into_make_service())
            .with_graceful_shutdown(shutdown_signal(rx_shutdown))
            .await
            .unwrap()
    });

    if auto_init {
        init_cluster(state.clone(), node_config.nodes, node_config.tls).await?;
    }

    let client = DbClient::new_local(state, node_config.tls, tx_shutdown);

    Ok(client)
}

async fn shutdown_signal(mut rx: watch::Receiver<bool>) {
    let _ = rx.changed().await;
}

async fn init_cluster(
    state: Arc<AppState>,
    nodes: Vec<Node>,
    tls: bool,
    // client: &DbClient,
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

        wait_for_nodes_online(&state, &nodes, tls).await;

        let mut nodes_set = BTreeMap::new();
        for node in nodes {
            nodes_set.insert(node.id, node);
        }
        state.raft.initialize(nodes_set).await?;

        // // The init will be done on node 1 only.
        // // This will bootstrap the cluster from a cold start.
        // match client.init().await {
        //     Ok(_) => {
        //         info!("New Raft Cluster has been initialized");
        //
        //         time::sleep(Duration::from_secs(1)).await;
        //
        //         let mut new_members = BTreeSet::new();
        //
        //         // After the initialization, we add the others as learners.
        //         for node in nodes {
        //             new_members.insert(node.id);
        //
        //             // Don't add this node as learner to itself
        //             if node.id != state.id {
        //                 match client
        //                     .add_learner(LearnerReq {
        //                         node_id: node.id,
        //                         addr_api: node.addr_api,
        //                         addr_raft: node.addr_raft,
        //                     })
        //                     .await
        //                 {
        //                     Ok(res) => {
        //                         info!("Node {} added as learner: {:?}", node.id, res)
        //                     }
        //                     Err(err) => {
        //                         error!("Error adding Node {} as learner: {:?}", node.id, err)
        //                     }
        //                 }
        //             }
        //         }
        //
        //         time::sleep(Duration::from_secs(1)).await;
        //
        //         match client.change_membership(&new_members).await {
        //             Ok(res) => {
        //                 info!("Membership changed: {:?}", res)
        //             }
        //             Err(err) => {
        //                 error!("Error changing membership: {:?}", err)
        //             }
        //         }
        //     }
        //     Err(err) => error!("{}", err),
        // }
    }

    Ok(())
}

async fn wait_for_nodes_online(state: &AppState, nodes: &[Node], tls: bool) {
    let scheme = if tls { "https" } else { "http" };
    let remotes = nodes
        .iter()
        .filter_map(|node| {
            (node.id != state.id).then_some(format!("{}://{}/ping", scheme, node.addr_api))
        })
        .collect::<Vec<String>>();
    let mut remotes_online = 0;

    let client = reqwest::Client::new();
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
