use crate::network::handshake::HandshakeSecret;
use crate::network::{serialize_network, AppStateExt, Error};
use axum::response::IntoResponse;
use fastwebsockets::{upgrade, FragmentCollectorRead, Frame, OpCode, Payload};
use openraft::error::{Fatal, InstallSnapshotError, RaftError};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::atomic::Ordering;
use tokio::task;
use tracing::{debug, error, warn};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

use crate::helpers::deserialize;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Serialize, Deserialize)]
pub enum RaftStreamRequest {
    #[cfg(feature = "sqlite")]
    AppendDB((usize, AppendEntriesRequest<TypeConfigSqlite>)),
    #[cfg(feature = "sqlite")]
    VoteDB((usize, VoteRequest<u64>)),
    #[cfg(feature = "sqlite")]
    SnapshotDB((usize, InstallSnapshotRequest<TypeConfigSqlite>)),

    #[cfg(feature = "cache")]
    AppendCache((usize, AppendEntriesRequest<TypeConfigKV>)),
    #[cfg(feature = "cache")]
    VoteCache((usize, VoteRequest<u64>)),
    #[cfg(feature = "cache")]
    SnapshotCache((usize, InstallSnapshotRequest<TypeConfigKV>)),
}

impl From<&[u8]> for RaftStreamRequest {
    #[inline]
    fn from(value: &[u8]) -> Self {
        deserialize(value).unwrap()
    }
}

impl From<Vec<u8>> for RaftStreamRequest {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        deserialize(&value).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RaftStreamResponse {
    pub request_id: usize,
    pub payload: RaftStreamResponsePayload,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum RaftStreamResponsePayload {
    #[cfg(feature = "sqlite")]
    AppendDB(Result<AppendEntriesResponse<u64>, RaftError<u64>>),
    #[cfg(feature = "sqlite")]
    VoteDB(Result<VoteResponse<u64>, RaftError<u64>>),
    #[cfg(feature = "sqlite")]
    SnapshotDB(Result<InstallSnapshotResponse<u64>, RaftError<u64, InstallSnapshotError>>),

    #[cfg(feature = "cache")]
    AppendCache(Result<AppendEntriesResponse<u64>, RaftError<u64>>),
    #[cfg(feature = "cache")]
    VoteCache(Result<VoteResponse<u64>, RaftError<u64>>),
    #[cfg(feature = "cache")]
    SnapshotCache(Result<InstallSnapshotResponse<u64>, RaftError<u64, InstallSnapshotError>>),
}

#[derive(Debug)]
pub(crate) enum WsWriteMsg {
    Payload(Vec<u8>),
    Break,
}

impl From<Vec<u8>> for RaftStreamResponse {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        deserialize(&value).unwrap()
    }
}

pub async fn stream_cache(
    state: AppStateExt,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    debug!("Incoming WebSocket stream for Cache");

    #[cfg(feature = "cache")]
    if state.raft_cache.is_raft_stopped.load(Ordering::Relaxed) {
        return Err(Error::BadRequest("Raft has been stopped".into()));
    }

    let (response, socket) = ws.upgrade()?;
    tokio::task::spawn(async move {
        if let Err(err) = handle_socket(state, socket).await {
            warn!("Cache WebSocket stream closed: {}", err);
        }
    });

    Ok(response)
}

pub async fn stream_sqlite(
    state: AppStateExt,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    debug!("Incoming WebSocket stream for SQLite");

    #[cfg(feature = "sqlite")]
    if state.raft_db.is_raft_stopped.load(Ordering::Relaxed) {
        return Err(Error::BadRequest("Raft has been stopped".into()));
    }

    let (response, socket) = ws.upgrade()?;
    tokio::task::spawn(async move {
        if let Err(err) = handle_socket(state, socket).await {
            warn!("SQLite WebSocket stream closed: {}", err);
        }
    });

    Ok(response)
}

async fn handle_socket(
    state: AppStateExt,
    socket: upgrade::UpgradeFut,
) -> Result<(), fastwebsockets::WebSocketError> {
    let mut ws = socket.await?;
    ws.set_auto_close(true);

    if let Err(err) = HandshakeSecret::server(&mut ws, state.secret_raft.as_bytes()).await {
        error!("Error during WebSocket handshake: {}", err);
        ws.write_frame(Frame::close(1000, b"Invalid Handshake"))
            .await?;
        return Ok(());
    }

    let (tx_write, rx_write) = flume::bounded::<WsWriteMsg>(2);
    let (rx, mut write) = ws.split(tokio::io::split);
    // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
    let mut read = FragmentCollectorRead::new(rx);

    task::spawn(async move {
        while let Ok(req) = rx_write.recv_async().await {
            match req {
                WsWriteMsg::Payload(bytes) => {
                    let frame = Frame::binary(Payload::Owned(bytes));
                    if let Err(err) = write.write_frame(frame).await {
                        error!("Error during WebSocket write: {}", err);
                        break;
                    }
                }
                WsWriteMsg::Break => {
                    warn!("server stream break message");
                    break;
                }
            }
        }

        warn!("Raft server WebSocket writer exiting");
        let _ = write.write_frame(Frame::close(1000, b"go away")).await;
    });

    while let Ok(frame) = read
        .read_frame(&mut |frame| async move {
            // TODO obligated sends should be auto ping / pong / close ? -> verify!
            warn!(
                "\n\nReceived obligated send in stream client: OpCode: {:?}: {:?}\n\n",
                frame.opcode.clone(),
                frame.payload
            );
            Ok::<(), Error>(())
        })
        .await
    {
        let req = match frame.opcode {
            OpCode::Close => {
                warn!("received Close frame in server stream");
                break;
            }
            OpCode::Binary => {
                let bytes = frame.payload.deref();
                match deserialize::<RaftStreamRequest>(bytes) {
                    Ok(req) => req,
                    Err(err) => {
                        error!("Error deserializing RaftStreamRequest: {:?}", err);
                        break;
                    }
                }
            }
            _ => {
                warn!("Non binary payload received - exiting");
                break;
            }
        };

        // Note: This was wrapped inside a `tokio::task` before just in case we would be able
        // to achieve higher throughput. After in depth testing, at least with openraft 0.9, it
        // has no benefit at all to do the extra work. Instead, it is actually a tiny performance
        // penalty if we spawn a task each time - requests are coming in in-order anyway.
        let (request_id, payload) = match req {
            #[cfg(feature = "sqlite")]
            RaftStreamRequest::AppendDB((request_id, req)) => {
                let res = state.raft_db.raft.append_entries(req).await;
                if let Err(RaftError::Fatal(Fatal::Stopped)) = &res {
                    warn!("Raft DB stopped - exiting");
                    state.raft_db.is_raft_stopped.store(true, Ordering::Relaxed);
                    break;
                }
                (request_id, RaftStreamResponsePayload::AppendDB(res))
            }
            #[cfg(feature = "sqlite")]
            RaftStreamRequest::VoteDB((request_id, req)) => {
                let res = state.raft_db.raft.vote(req).await;
                (request_id, RaftStreamResponsePayload::VoteDB(res))
            }
            #[cfg(feature = "sqlite")]
            RaftStreamRequest::SnapshotDB((request_id, req)) => {
                let res = state.raft_db.raft.install_snapshot(req).await;
                (request_id, RaftStreamResponsePayload::SnapshotDB(res))
            }

            #[cfg(feature = "cache")]
            RaftStreamRequest::AppendCache((request_id, req)) => {
                let res = state.raft_cache.raft.append_entries(req).await;
                if let Err(RaftError::Fatal(Fatal::Stopped)) = &res {
                    warn!("Raft Cache stopped - exiting");
                    state
                        .raft_cache
                        .is_raft_stopped
                        .store(true, Ordering::Relaxed);
                    break;
                }
                (request_id, RaftStreamResponsePayload::AppendCache(res))
            }
            #[cfg(feature = "cache")]
            RaftStreamRequest::VoteCache((request_id, req)) => {
                let res = state.raft_cache.raft.vote(req).await;
                (request_id, RaftStreamResponsePayload::VoteCache(res))
            }
            #[cfg(feature = "cache")]
            RaftStreamRequest::SnapshotCache((request_id, req)) => {
                let res = state.raft_cache.raft.install_snapshot(req).await;
                (request_id, RaftStreamResponsePayload::SnapshotCache(res))
            }
        };

        if let Err(err) = tx_write
            .send_async(WsWriteMsg::Payload(serialize_network(
                &RaftStreamResponse {
                    request_id,
                    payload,
                },
            )))
            .await
        {
            error!(
                "Error forwarding raft response to WebSocket writer: {}",
                err
            );
        }
    }

    // try to close the writer if it should still be running
    let _ = tx_write.send_async(WsWriteMsg::Break).await;

    Ok(())
}
