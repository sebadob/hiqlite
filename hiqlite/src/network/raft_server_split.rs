use crate::network::handshake::HandshakeSecret;
use crate::network::{AppStateExt, Error};
use axum::response::IntoResponse;
use fastwebsockets::{upgrade, FragmentCollectorRead, Frame, OpCode, Payload};
use openraft::error::{InstallSnapshotError, RaftError};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use tokio::task;
use tracing::{error, warn};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

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
    fn from(value: &[u8]) -> Self {
        bincode::deserialize(value).unwrap()
    }
}

impl From<Vec<u8>> for RaftStreamRequest {
    fn from(value: Vec<u8>) -> Self {
        bincode::deserialize(&value).unwrap()
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
    fn from(value: Vec<u8>) -> Self {
        bincode::deserialize(&value).unwrap()
    }
}

pub async fn stream(
    state: AppStateExt,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    let (response, socket) = ws.upgrade()?;

    tokio::task::spawn(async move {
        if let Err(err) = handle_socket(state, socket).await {
            warn!("WebSocket stream closed: {}", err);
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

    let (tx_write, rx_write) = flume::unbounded::<WsWriteMsg>();
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
                // let _ = tx_write.send_async(WsWriteMsg::Break).await;
                break;
            }
            OpCode::Binary => {
                let bytes = frame.payload.deref();
                match bincode::deserialize::<RaftStreamRequest>(bytes) {
                    Ok(req) => req,
                    Err(err) => {
                        error!("Error deserializing RaftStreamRequest: {:?}", err);
                        // let _ = tx_write.send_async(WsWriteMsg::Break).await;
                        break;
                    }
                }
            }
            _ => {
                warn!("Non binary payload received - exiting");
                // let _ = tx_write.send_async(WsWriteMsg::Break).await;
                break;
            }
        };

        let tx_write = tx_write.clone();
        let state = state.clone();
        task::spawn(async move {
            let bytes = match req {
                #[cfg(feature = "sqlite")]
                RaftStreamRequest::AppendDB((request_id, req)) => {
                    let res = state.raft_db.raft.append_entries(req).await;
                    let resp = RaftStreamResponse {
                        request_id,
                        payload: RaftStreamResponsePayload::AppendDB(res),
                    };
                    bincode::serialize(&resp).unwrap()
                }
                #[cfg(feature = "sqlite")]
                RaftStreamRequest::VoteDB((request_id, req)) => {
                    let res = state.raft_db.raft.vote(req).await;
                    let resp = RaftStreamResponse {
                        request_id,
                        payload: RaftStreamResponsePayload::VoteDB(res),
                    };
                    bincode::serialize(&resp).unwrap()
                }
                #[cfg(feature = "sqlite")]
                RaftStreamRequest::SnapshotDB((request_id, req)) => {
                    let res = state.raft_db.raft.install_snapshot(req).await;
                    let resp = RaftStreamResponse {
                        request_id,
                        payload: RaftStreamResponsePayload::SnapshotDB(res),
                    };
                    bincode::serialize(&resp).unwrap()
                }

                #[cfg(feature = "cache")]
                RaftStreamRequest::AppendCache((request_id, req)) => {
                    let res = state.raft_cache.raft.append_entries(req).await;
                    let resp = RaftStreamResponse {
                        request_id,
                        payload: RaftStreamResponsePayload::AppendCache(res),
                    };
                    bincode::serialize(&resp).unwrap()
                }
                #[cfg(feature = "cache")]
                RaftStreamRequest::VoteCache((request_id, req)) => {
                    let res = state.raft_cache.raft.vote(req).await;
                    let resp = RaftStreamResponse {
                        request_id,
                        payload: RaftStreamResponsePayload::VoteCache(res),
                    };
                    bincode::serialize(&resp).unwrap()
                }
                #[cfg(feature = "cache")]
                RaftStreamRequest::SnapshotCache((request_id, req)) => {
                    let res = state.raft_cache.raft.install_snapshot(req).await;
                    let resp = RaftStreamResponse {
                        request_id,
                        payload: RaftStreamResponsePayload::SnapshotCache(res),
                    };
                    bincode::serialize(&resp).unwrap()
                }
            };

            if let Err(err) = tx_write.send_async(WsWriteMsg::Payload(bytes)).await {
                error!(
                    "Error forwarding raft response to WebSocket writer: {}",
                    err
                );
            }
        });
    }

    // let res = select! {
    //     res = handle_write => res,
    //     res = handle_read => res,
    // };

    // try to close the writer if it should still be running
    let _ = tx_write.send_async(WsWriteMsg::Break).await;

    // handle_read.abort();

    Ok(())
}
