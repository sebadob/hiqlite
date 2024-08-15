use crate::server::proxy::state::AppStateProxy;
use crate::{Client, Error};
use axum::routing::get;
use axum::Router;
use config::Config;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

pub mod config;
mod handlers;
mod notify;
mod state;
mod stream;

pub async fn start_proxy(config: Config) -> Result<(), Error> {
    if config.tls_config.is_some() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("default CryptoProvider installation to succeed");
    }

    let tls_client_config = config.tls_config.as_ref().map(|c| c.client_config());

    let client = Client::remote(
        config.nodes,
        tls_client_config.is_some(),
        config
            .tls_config
            .as_ref()
            .map(|c| c.danger_tls_no_verify)
            .unwrap_or(false),
        config.secret_api.clone(),
        false,
    )
    .await?;

    let tx_notify = notify::spawn_listener(client.clone());

    let state = Arc::new(AppStateProxy {
        client,
        secret_api: config.secret_api,
        tx_notify,
        // dashboard_password: config.password_dashboard,
    });

    let router = Router::new()
        .nest(
            "/cluster",
            Router::new()
                // .route("/add_learner/:raft_type", post(management::add_learner))
                // .route("/become_member/:raft_type", post(management::become_member))
                // .route(
                //     "/membership/:raft_type",
                //     get(management::get_membership).post(management::post_membership),
                // )
                .route("/metrics/:raft_type", get(handlers::metrics)),
        )
        .route("/listen", get(handlers::listen))
        .route("/stream", get(handlers::stream))
        // .route("/health", get(api::health))
        .route("/ping", get(handlers::ping))
        .with_state(state.clone());

    let addr = format!("0.0.0.0:{}", config.listen_port);
    info!("listening on {}", addr);
    let addr = SocketAddr::from_str(&addr).expect("valid socket address");

    if let Some(config) = &config.tls_config {
        let tls_config = config.server_config().await;

        axum_server::bind_rustls(addr, tls_config)
            .serve(router.into_make_service())
            .await
            .unwrap();
    } else {
        axum_server::bind(addr)
            .serve(router.into_make_service())
            .await
            .unwrap();
    };

    Ok(())
}
