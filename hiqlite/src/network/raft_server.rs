use crate::network::handshake::HandshakeSecret;
use crate::network::{AppStateExt, Error};
use crate::store::state_machine::memory::TypeConfigKV;
use axum::response::IntoResponse;
use fastwebsockets::{upgrade, Frame, OpCode, Payload};
use openraft::raft::VoteRequest;
use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse};
use openraft::raft::{InstallSnapshotRequest, InstallSnapshotResponse, VoteResponse};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[derive(Debug, Serialize, Deserialize)]
pub enum RaftStreamRequest {
    AppendDB(AppendEntriesRequest<TypeConfigSqlite>),
    VoteDB(VoteRequest<u64>),
    SnapshotDB(InstallSnapshotRequest<TypeConfigSqlite>),

    AppendCache(AppendEntriesRequest<TypeConfigKV>),
    VoteCache(VoteRequest<u64>),
    SnapshotCache(InstallSnapshotRequest<TypeConfigKV>),
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

impl RaftStreamRequest {
    pub fn as_payload(&self) -> fastwebsockets::Frame {
        Frame::binary(Payload::from(bincode::serialize(self).unwrap()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RaftStreamResponse {
    Append(AppendEntriesResponse<u64>),
    Vote(VoteResponse<u64>),
    Snapshot(InstallSnapshotResponse<u64>),
    Error(Error),
}

impl From<Vec<u8>> for RaftStreamResponse {
    fn from(value: Vec<u8>) -> Self {
        bincode::deserialize(&value).unwrap()
    }
}

impl RaftStreamResponse {
    fn as_payload(&self) -> fastwebsockets::Frame {
        Frame::binary(Payload::from(bincode::serialize(self).unwrap()))
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

    if let Err(err) = HandshakeSecret::server(&mut ws, state.secret_raft.as_bytes()).await {
        error!("Error during WebSocket handshake: {}", err);
        ws.write_frame(Frame::close(1000, b"Invalid Handshake"))
            .await?;
        return Ok(());
    }

    // TODO test if we can even bump the performance with a split here as well
    let mut ws = fastwebsockets::FragmentCollector::new(ws);

    loop {
        let frame = ws.read_frame().await?;
        match frame.opcode {
            OpCode::Close => break,
            OpCode::Binary => {
                let bytes = frame.payload.to_vec();
                match RaftStreamRequest::from(bytes) {
                    RaftStreamRequest::AppendDB(req) => {
                        match state.raft_db.raft.append_entries(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::Append(res).as_payload())
                                    .await?;
                            }
                            Err(err) => {
                                ws.write_frame(
                                    RaftStreamResponse::Error(Error::from(err)).as_payload(),
                                )
                                .await?;
                            }
                        }
                    }

                    RaftStreamRequest::VoteDB(req) => match state.raft_db.raft.vote(req).await {
                        Ok(res) => {
                            ws.write_frame(RaftStreamResponse::Vote(res).as_payload())
                                .await?;
                        }
                        Err(err) => {
                            ws.write_frame(
                                RaftStreamResponse::Error(Error::from(err)).as_payload(),
                            )
                            .await?;
                        }
                    },

                    RaftStreamRequest::SnapshotDB(req) => {
                        match state.raft_db.raft.install_snapshot(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::Snapshot(res).as_payload())
                                    .await?;
                            }
                            Err(err) => {
                                ws.write_frame(
                                    RaftStreamResponse::Error(Error::from(err)).as_payload(),
                                )
                                .await?;
                            }
                        }
                    }

                    RaftStreamRequest::AppendCache(req) => {
                        match state.raft_cache.raft.append_entries(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::Append(res).as_payload())
                                    .await?;
                            }
                            Err(err) => {
                                ws.write_frame(
                                    RaftStreamResponse::Error(Error::from(err)).as_payload(),
                                )
                                .await?;
                            }
                        }
                    }

                    RaftStreamRequest::VoteCache(req) => {
                        match state.raft_cache.raft.vote(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::Vote(res).as_payload())
                                    .await?;
                            }
                            Err(err) => {
                                ws.write_frame(
                                    RaftStreamResponse::Error(Error::from(err)).as_payload(),
                                )
                                .await?;
                            }
                        }
                    }

                    RaftStreamRequest::SnapshotCache(req) => {
                        match state.raft_cache.raft.install_snapshot(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::Snapshot(res).as_payload())
                                    .await?;
                            }
                            Err(err) => {
                                ws.write_frame(
                                    RaftStreamResponse::Error(Error::from(err)).as_payload(),
                                )
                                .await?;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}
