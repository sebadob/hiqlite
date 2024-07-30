// Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../../README.md")]
#![forbid(unsafe_code)]

use crate::app_state::{AppState, RaftType};
use crate::network::raft_server;
use crate::network::{api, management};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task;
use tracing::info;

pub use crate::db_client::DbClient;
pub use crate::error::Error;
pub use crate::query::rows::Row;
pub use crate::store::state_machine::sqlite::state_machine::{Params, Response};
pub use config::{NodeConfig, RaftConfig};
pub use migration::AppliedMigration;
pub use openraft::SnapshotPolicy;
pub use store::state_machine::sqlite::param::Param;
pub use tls::ServerTlsConfig;

mod app_state;
mod config;
mod db_client;
mod error;
mod helpers;
mod init;
mod migration;
mod network;
mod query;
mod store;
mod tls;

#[cfg(feature = "backup")]
mod backup;
#[cfg(feature = "dashboard")]
mod dashboard;
#[cfg(feature = "s3")]
pub mod s3;

type NodeId = u64;

/// Create params for distributed SQL modifying queries.
/// TODO create multiple branches here to be able to catch the correct sizes
/// with upfront capacity assignment earlier.
#[macro_export]
macro_rules! params {
    ( $( $param:expr ),* ) => {
        {
            #[allow(unused_mut)]
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

/// Starts a Raft node.
/// # Panics
/// If an incorrect `node_config` was given.
pub async fn start_node(node_config: NodeConfig) -> Result<DbClient, Error> {
    node_config.is_valid()?;

    if node_config.tls_api.is_some() || node_config.tls_raft.is_some() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("default CryptoProvider installation to succeed");
    }

    let tls_api_client_config = node_config.tls_api.clone().map(|c| c.client_config());
    let tls_raft = node_config.tls_raft.is_some();
    let tls_no_verify = node_config
        .tls_raft
        .as_ref()
        .map(|c| c.danger_tls_no_verify)
        .unwrap_or(false);

    #[cfg(feature = "s3")]
    s3::init_enc_keys(&node_config.enc_keys_from)?;

    #[cfg(all(feature = "backup", feature = "sqlite"))]
    let backup_applied = backup::restore_backup_start(&node_config).await?;

    let raft_config = Arc::new(node_config.raft_config.clone().validate().unwrap());

    #[cfg(feature = "sqlite")]
    let raft_db = store::start_raft_db(node_config.clone(), raft_config.clone()).await?;
    #[cfg(feature = "cache")]
    let raft_cache = store::start_raft_cache(node_config.clone(), raft_config).await?;

    let (api_addr, rpc_addr) = {
        let node = node_config
            .nodes
            .get(node_config.node_id as usize - 1)
            .expect("NodeConfig.node_id not found in NodeConfig.nodes");

        let api_addr = build_listen_addr(&node.addr_api, tls_api_client_config.is_some());
        let addr_raft = build_listen_addr(&node.addr_raft, tls_raft);

        (api_addr, addr_raft)
    };

    // TODO put behind Mutex to make it dynamic?
    let mut client_buffers = HashMap::new();
    for node in &node_config.nodes {
        let (tx, rx) = flume::unbounded();
        client_buffers.insert(node.id, (tx, rx));
    }

    let state = Arc::new(AppState {
        id: node_config.node_id,
        addr_api: api_addr.clone(),
        #[cfg(feature = "sqlite")]
        raft_db,
        #[cfg(feature = "cache")]
        raft_cache,
        secret_api: node_config.secret_api,
        secret_raft: node_config.secret_raft,
        client_buffers,
        #[cfg(feature = "dashboard")]
        dashboard: dashboard::DashboardState {
            password_dashboard: node_config.password_dashboard,
        },
    });

    #[cfg(all(feature = "backup", feature = "sqlite"))]
    if backup_applied {
        backup::restore_backup_finish(&state).await;
    }

    let (tx_shutdown, rx_shutdown) = watch::channel(false);

    let router_internal = Router::new()
        .route("/stream/db", get(raft_server::stream))
        .route("/stream/cache", get(raft_server::stream))
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

    let default_routes = Router::new()
        .nest(
            "/cluster",
            Router::new()
                .route("/add_learner/:raft_type", post(management::add_learner))
                .route("/become_member/:raft_type", post(management::become_member))
                .route(
                    "/membership/:raft_type",
                    get(management::get_membership).post(management::post_membership),
                )
                // .route("/init", post(management::init))
                .route("/metrics", get(management::metrics)),
        )
        // TODO
        // .route("/execute", post(api::execute))
        // TODO
        // .route("/query", post(api::query))
        // TODO
        // .route("/query/consistent", post(api::query))
        .route("/stream", get(api::stream))
        .route("/ping", get(api::ping));

    #[cfg(not(feature = "dashboard"))]
    let router_api = default_routes.with_state(state.clone());
    #[cfg(feature = "dashboard")]
    let router_api = default_routes
        .route("/", get(dashboard::handlers::redirect_to_index))
        .nest(
            "/dashboard",
            Router::new()
                .route("/", get(dashboard::handlers::redirect_to_index))
                .nest(
                    "/api",
                    Router::new()
                        .route("/query", post(dashboard::handlers::post_query))
                        .route(
                            "/session",
                            get(dashboard::handlers::get_session)
                                .post(dashboard::handlers::post_session),
                        )
                        .route("/tables", get(dashboard::handlers::get_tables)),
                )
                .layer(dashboard::middleware::middleware())
                .fallback(dashboard::static_files::handler),
        )
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

    #[cfg(feature = "sqlite")]
    let member_db = {
        let st = state.clone();
        let nodes = node_config.nodes.clone();

        task::spawn(async move {
            init::become_cluster_member(
                st,
                &RaftType::Sqlite,
                node_config.node_id,
                &nodes,
                tls_raft,
                tls_no_verify,
            )
            .await
        })
    };

    #[cfg(feature = "cache")]
    let member_cache = {
        let st = state.clone();
        let nodes = node_config.nodes.clone();
        task::spawn(async move {
            init::become_cluster_member(
                st,
                &RaftType::Cache,
                node_config.node_id,
                &nodes,
                tls_raft,
                tls_no_verify,
            )
            .await
        })
    };

    #[cfg(feature = "sqlite")]
    member_db.await??;
    #[cfg(feature = "cache")]
    member_cache.await??;

    let client = DbClient::new_local(state, tls_api_client_config, tx_shutdown);

    Ok(client)
}

fn build_listen_addr(addr: &str, tls: bool) -> String {
    let port = if let Some((_, port)) = addr.split_once(':') {
        port
    } else if tls {
        "443"
    } else {
        "80"
    };
    format!("0.0.0.0:{}", port)
}

async fn shutdown_signal(mut rx: watch::Receiver<bool>) {
    let _ = rx.changed().await;
}
