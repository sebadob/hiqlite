use crate::app_state::AppState;
use crate::network::raft_server;
use crate::network::{api, management};
use crate::{Client, Error, NodeConfig, init, split_brain_check, store};
use axum::Router;
use axum::routing::{get, post};
use chrono::Utc;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task;
use tracing::{debug, info};

#[cfg(feature = "backup")]
use crate::backup;
#[cfg(feature = "dashboard")]
use crate::dashboard;

#[allow(clippy::extra_unused_type_parameters)]
pub async fn start_node_inner<C>(node_config: NodeConfig) -> Result<Client, Error>
where
    C: Debug + strum::IntoEnumIterator + crate::cache_idx::CacheIndex,
{
    node_config.is_valid()?;

    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        debug!("Error installing default rustls crypto provider, may have been installed already");
    }

    let tls_api_client_config = node_config.tls_api.clone().map(|c| c.client_config());
    let tls_raft = node_config.tls_raft.is_some();
    let tls_no_verify = node_config
        .tls_raft
        .as_ref()
        .map(|c| c.danger_tls_no_verify)
        .unwrap_or(false);

    #[cfg(any(feature = "s3", feature = "dashboard"))]
    node_config.init_enc_keys();

    #[cfg(feature = "dashboard")]
    dashboard::init()?;

    #[cfg(all(feature = "backup", feature = "sqlite"))]
    let backup_applied = backup::restore_backup_start(&node_config).await?;

    let raft_config = Arc::new(node_config.raft_config.clone().validate().unwrap());

    let _do_reset_metadata = init::check_execute_reset(&node_config.data_dir).await?;
    #[cfg(feature = "sqlite")]
    let raft_db =
        store::start_raft_db(&node_config, raft_config.clone(), _do_reset_metadata).await?;
    #[cfg(feature = "cache")]
    let raft_cache = store::start_raft_cache::<C>(&node_config, raft_config.clone()).await?;

    let (api_addr, rpc_addr) = {
        let node = node_config
            .nodes
            .get(node_config.node_id as usize - 1)
            .expect("NodeConfig.node_id not found in NodeConfig.nodes");

        let api_addr = build_listen_addr(
            &node_config.listen_addr_api,
            &node.addr_api,
            tls_api_client_config.is_some(),
        );
        let addr_raft = build_listen_addr(&node_config.listen_addr_raft, &node.addr_raft, tls_raft);

        (api_addr, addr_raft)
    };

    #[cfg(feature = "sqlite")]
    let (tx_client_stream, rx_client_stream) = flume::bounded(1);

    let state = Arc::new(AppState {
        app_start: Utc::now(),
        is_shutting_down: AtomicBool::new(false),
        #[cfg(feature = "backup")]
        backups_dir: format!("{}/state_machine/backups", node_config.data_dir),
        id: node_config.node_id,
        #[cfg(feature = "cache")]
        nodes: node_config.nodes.clone(),
        addr_api: api_addr.clone(),
        #[cfg(feature = "sqlite")]
        raft_db,
        #[cfg(feature = "cache")]
        raft_cache,
        raft_lock: Arc::new(Mutex::new(())),
        secret_api: node_config.secret_api,
        secret_raft: node_config.secret_raft,
        #[cfg(feature = "dashboard")]
        dashboard: dashboard::DashboardState {
            password_dashboard: node_config.password_dashboard,
        },
        #[cfg(feature = "dashboard")]
        client_request_id: std::sync::atomic::AtomicUsize::new(0),
        #[cfg(feature = "dashboard")]
        tx_client_stream: tx_client_stream.clone(),
        shutdown_delay_millis: node_config.shutdown_delay_millis,
        health_check_delay_secs: node_config.health_check_delay_secs,
        #[cfg(feature = "s3")]
        s3_config: node_config.s3_config.clone(),
    });

    #[cfg(any(feature = "sqlite", feature = "cache"))]
    split_brain_check::spawn(
        state.clone(),
        node_config.nodes.clone(),
        node_config.tls_api.is_some(),
    );

    #[cfg(all(feature = "backup", feature = "sqlite"))]
    if backup_applied {
        backup::restore_backup_finish(&state).await;
    }

    let (tx_shutdown, rx_shutdown) = tokio::sync::watch::channel(false);

    let router_internal = Router::new()
        // .route("/stream", get(raft_server_split::stream))
        .route("/stream/sqlite", get(raft_server::stream_sqlite))
        .route("/stream/cache", get(raft_server::stream_cache))
        .route("/health", get(api::health))
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
            //  properly with axum directly
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
                .route("/add_learner/{raft_type}", post(management::add_learner))
                .route(
                    "/become_member/{raft_type}",
                    post(management::become_member),
                )
                .route(
                    "/membership/{raft_type}",
                    get(management::get_membership)
                        .post(management::post_membership)
                        .delete(management::leave_cluster),
                )
                .route("/metrics/{raft_type}", get(management::metrics)),
        )
        .route("/listen", get(api::listen))
        .route("/stream/{raft_type}", get(api::stream))
        .route("/health", get(api::health))
        .route("/ready", get(api::ready))
        .route("/ping", get(api::ping));

    #[cfg(not(feature = "dashboard"))]
    let router_api = default_routes.with_state(state.clone());
    #[cfg(feature = "dashboard")]
    let router_api = if state.dashboard.password_dashboard.is_some() {
        default_routes
            .route("/", get(dashboard::handlers::redirect_to_index))
            .nest(
                "/dashboard",
                Router::new()
                    .route("/", get(dashboard::handlers::redirect_to_index))
                    .nest(
                        "/api",
                        Router::new()
                            .route("/metrics", get(dashboard::handlers::get_metrics))
                            .route("/pow", get(dashboard::handlers::get_pow))
                            .route("/query", post(dashboard::handlers::post_query))
                            .route(
                                "/session",
                                get(dashboard::handlers::get_session)
                                    .post(dashboard::handlers::post_session),
                            )
                            .route("/tables", get(dashboard::handlers::get_tables))
                            .route(
                                "/tables/{filter}",
                                get(dashboard::handlers::get_tables_filtered),
                            ),
                    )
                    .layer(dashboard::middleware::middleware())
                    .fallback(dashboard::static_files::handler),
            )
            .with_state(state.clone())
    } else {
        default_routes.with_state(state.clone())
    };

    info!("api external listening on {api_addr}");
    let tls_config = if let Some(config) = &node_config.tls_api {
        Some(config.server_config().await)
    } else {
        None
    };
    let _handle_external = task::spawn(async move {
        if let Some(config) = tls_config {
            let addr = SocketAddr::from_str(&api_addr).expect("valid RPC socket address");
            // TODO find a way to do a graceful shutdown with `axum_server` or to handle TLS
            //  properly with axum directly
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
                &crate::app_state::RaftType::Sqlite,
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
                &crate::app_state::RaftType::Cache,
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

    let client = Client::new_local(
        state,
        tls_api_client_config,
        #[cfg(feature = "cache")]
        tls_no_verify,
        #[cfg(feature = "sqlite")]
        tx_client_stream,
        #[cfg(feature = "sqlite")]
        rx_client_stream,
        tx_shutdown,
    )
    .await;

    #[cfg(all(feature = "backup", feature = "s3"))]
    if let Some(s3_config) = node_config.s3_config {
        backup::start_cron(client.clone(), s3_config, node_config.backup_config);
    }

    Ok(client)
}

/// The port will be split off from the `node_addr`
fn build_listen_addr(listen_addr: &str, node_addr: &str, tls: bool) -> String {
    let port = if let Some((_, port)) = node_addr.split_once(':') {
        port
    } else if tls {
        "443"
    } else {
        "80"
    };
    format!("{listen_addr}:{port}")
}

async fn shutdown_signal(mut rx: tokio::sync::watch::Receiver<bool>) {
    let _ = rx.changed().await;
}
