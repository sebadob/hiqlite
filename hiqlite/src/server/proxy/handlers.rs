use crate::Error;
use crate::app_state::RaftType;
use crate::helpers::serialize;
use crate::network::handshake::HandshakeSecret;
use crate::server::proxy::state::AppStateProxy;
use crate::server::proxy::stream;
use crate::store::state_machine::memory::notify_handler::NotifyRequest;
use axum::Json;
use axum::extract::Path;
use axum::http::header::ACCEPT;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use fastwebsockets::{upgrade, FragmentCollectorRead, Frame, OpCode, Payload};
use serde::Serialize;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error};

pub type AppStateExt = axum::extract::State<Arc<AppStateProxy>>;

static HEADER_NAME_SECRET: &str = "X-API-SECRET";

pub async fn ping() {}

pub async fn listen(
    state: AppStateExt,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    let (response, socket) = ws.upgrade()?;
    debug!("New /listen WebSocket connection");

    tokio::task::spawn(async move {
        if let Err(err) = handle_listen_socket(state, socket).await {
            error!("Error in /listen WebSocket connection: {}", err);
        }
    });

    Ok(response)
}

async fn handle_listen_socket(
    state: AppStateExt,
    socket: upgrade::UpgradeFut,
) -> Result<(), fastwebsockets::WebSocketError> {
    let mut ws = socket.await?;
    ws.set_auto_close(true);

    if let Err(err) = HandshakeSecret::server(&mut ws, state.secret_api.as_bytes()).await {
        error!("Error during /listen WebSocket handshake: {}", err);
        let _ = ws.write_frame(Frame::close(1000, b"Invalid Handshake")).await;
        return Ok(());
    }

    // Register this connection as a listener with the state machine.
    let (tx_notify, rx_notify) = flume::unbounded::<(i64, Vec<u8>)>();
    if let Err(err) = state
        .tx_notify
        .send_async(NotifyRequest::Listen(tx_notify))
        .await
    {
        error!("Failed to register /listen listener: {}", err);
        return Ok(());
    }

    let (rx, mut write) = ws.split(tokio::io::split);
    // IMPORTANT: the reader is NOT CANCEL SAFE - it runs in its own task.
    let mut read = FragmentCollectorRead::new(rx);

    // The client pings for keepalive; echo the obligated sends (Pong / Close) back to the socket.
    let (tx_obligated, rx_obligated) = flume::bounded::<(OpCode, Vec<u8>)>(1);
    let handle_read = tokio::task::spawn(async move {
        while let Ok(frame) = read
            .read_frame(&mut |frame| {
                let tx_obligated = tx_obligated.clone();
                async move {
                    if let Err(err) = tx_obligated
                        .send_async((frame.opcode, frame.payload.to_vec()))
                        .await
                    {
                        error!("/listen: failed to forward obligated frame: {}", err);
                    }
                    Ok::<(), Error>(())
                }
            })
            .await
        {
            if frame.opcode == OpCode::Close {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            ev = rx_notify.recv_async() => match ev {
                Ok((ts, data)) => {
                    let bytes = match serialize(&(ts, data)) {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!("/listen: failed to serialize notification: {}", err);
                            break;
                        }
                    };
                    if write.write_frame(Frame::binary(Payload::Owned(bytes))).await.is_err() {
                        debug!("/listen: write failed - closing");
                        break;
                    }
                }
                Err(_) => break,
            },
            ob = rx_obligated.recv_async() => match ob {
                Ok((opcode, payload)) => {
                    // The library produced this frame and expects us to send it verbatim: a Pong
                    // in response to the client's keepalive Ping, or the echo of a Close frame.
                    let frame = if opcode == OpCode::Pong {
                        Frame::pong(Payload::Owned(payload))
                    } else {
                        Frame::close_raw(Payload::Owned(payload))
                    };
                    if write.write_frame(frame).await.is_err() {
                        debug!("/listen: obligated frame write failed - closing");
                        break;
                    }
                }
                Err(_) => break,
            },
        }
    }

    handle_read.abort();
    let _ = write.write_frame(Frame::close(1000, b"Done")).await;
    debug!("/listen WebSocket connection exiting");

    Ok(())
}

pub async fn stream(
    state: AppStateExt,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    let permit = tokio::time::timeout(
        Duration::from_secs(10),
        state.active_streams_permits.clone().acquire_owned(),
    )
    .await
    .map_err(|_| Error::Timeout("Stream request timed out - max connections reached".to_string()))?
    .map_err(|_| Error::Request("Server is shutting down".to_string()))?;

    let (response, socket) = ws.upgrade()?;
    tokio::task::spawn(async move {
        let _permit = permit;
        if let Err(err) = stream::handle_socket(state.clone(), socket).await {
            // if let Err(err) = handle_socket_sequential(state, socket).await {
            error!("Error in websocket connection: {}", err);
        }
    });

    Ok(response)
}

pub(crate) async fn metrics(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let metrics = match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.client.metrics_db().await?,
        #[cfg(feature = "cache")]
        RaftType::Cache => state.client.metrics_cache().await?,
        RaftType::Unknown => {
            return Err(Error::new(
                "unknown raft type - neither `sqlite` nor `cache` feature enabled",
            ));
        }
    };

    fmt_ok(headers, &metrics)
}

#[inline(always)]
fn fmt_ok<S: Debug + Serialize>(headers: HeaderMap, payload: S) -> Result<Response, Error> {
    if let Some(accept) = headers.get(ACCEPT)
        && accept == HeaderValue::from_static("application/json")
    {
        return Ok(Json(payload).into_response());
    }
    Ok(serialize(&payload)?.into_response())
}

#[inline(always)]
fn validate_secret(state: &AppStateExt, headers: &HeaderMap) -> Result<(), Error> {
    match headers.get(HEADER_NAME_SECRET) {
        None => Err(Error::Token("API Secret missing".into())),
        Some(secret) => {
            if !constant_time_eq::constant_time_eq(state.secret_api.as_bytes(), secret.as_bytes()) {
                Err(Error::Token("Invalid API Secret".into()))
            } else {
                Ok(())
            }
        }
    }
}
