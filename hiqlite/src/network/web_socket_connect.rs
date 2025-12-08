use crate::app_state::RaftType;
use crate::network::handshake::HandshakeSecret;
use crate::{Error, NodeId, tls};
use axum::http::Request;
use axum::http::header::{CONNECTION, UPGRADE};
use bytes::Bytes;
use fastwebsockets::{Frame, WebSocket};
use http_body_util::Empty;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tracing::{debug, error, info};

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

pub async fn try_connect(
    node_id: NodeId,
    addr: &str,
    raft_type: &RaftType,
    tls_config: Option<Arc<rustls::ClientConfig>>,
    secret: &[u8],
) -> Result<WebSocket<TokioIo<Upgraded>>, Error> {
    tokio::time::timeout(
        Duration::from_secs(5),
        try_connect_stream(node_id, addr, raft_type, tls_config, secret),
    )
    .await
    .map_err(|_| {
        Error::Connect("Could not open WebSocket stream after timeout of 5 seconds".to_string())
    })?
}

async fn try_connect_stream(
    node_id: NodeId,
    addr: &str,
    raft_type: &RaftType,
    tls_config: Option<Arc<rustls::ClientConfig>>,
    secret: &[u8],
) -> Result<WebSocket<TokioIo<Upgraded>>, Error> {
    let scheme = if tls_config.is_some() {
        "https"
    } else {
        "http"
    };
    let uri = format!("{}://{}/stream/{}", scheme, addr, raft_type.as_str());
    info!("Trying to connect to: {uri}");

    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .body(Empty::<Bytes>::new())
        .map_err(|err| {
            error!("Error connecting to {uri}: {err:?}");
            Error::Connect(err.to_string())
        })?;

    debug!("Opening TcpStream to: {addr}");
    let stream = TcpStream::connect(addr)
        .await
        .map_err(|err| Error::Connect(err.to_string()))?;

    let (mut ws, _) = if let Some(config) = tls_config {
        let (addr, _) = addr.split_once(':').unwrap_or((addr, ""));
        let tls_stream = tls::into_tls_stream(addr, stream, config).await?;

        fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await
    } else {
        fastwebsockets::handshake::client(&SpawnExecutor, req, stream).await
    }
    .map_err(|err| {
        error!("Error opening WebSocket stream: {err:?}");
        Error::Connect(err.to_string())
    })?;
    ws.set_auto_close(true);
    debug!("WebSocket connection established");

    if let Err(err) = HandshakeSecret::client(&mut ws, secret, node_id).await {
        error!("Error opening WebSocket stream to {addr}: {err:?}");
        let _ = ws
            .write_frame(Frame::close(1000, b"Invalid Handshake"))
            .await;
        Err(Error::Connect(format!(
            "Error during API WebSocket handshake to {addr}: {err}"
        )))
    } else {
        info!("WebSocket stream connected to: {addr}");
        Ok(ws)
    }
}
