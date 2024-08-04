use crate::network::handshake::HandshakeSecret;
use crate::network::{AppStateExt, Error};
use axum::response::IntoResponse;
use fastwebsockets::{upgrade, Frame, OpCode, Payload};
use serde::{Deserialize, Serialize};
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
#[cfg(any(feature = "cache", feature = "sqlite"))]
use tracing::info;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Serialize, Deserialize)]
pub enum RaftStreamRequest {
    #[cfg(feature = "sqlite")]
    AppendDB(AppendEntriesRequest<TypeConfigSqlite>),
    #[cfg(feature = "sqlite")]
    VoteDB(VoteRequest<u64>),
    #[cfg(feature = "sqlite")]
    SnapshotDB(InstallSnapshotRequest<TypeConfigSqlite>),

    #[cfg(feature = "cache")]
    AppendCache(AppendEntriesRequest<TypeConfigKV>),
    #[cfg(feature = "cache")]
    VoteCache(VoteRequest<u64>),
    #[cfg(feature = "cache")]
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
    #[cfg(feature = "sqlite")]
    AppendDB(AppendEntriesResponse<u64>),
    #[cfg(feature = "sqlite")]
    VoteDB(VoteResponse<u64>),
    #[cfg(feature = "sqlite")]
    SnapshotDB(InstallSnapshotResponse<u64>),

    #[cfg(feature = "cache")]
    AppendCache(AppendEntriesResponse<u64>),
    #[cfg(feature = "cache")]
    VoteCache(VoteResponse<u64>),
    #[cfg(feature = "cache")]
    SnapshotCache(InstallSnapshotResponse<u64>),

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
                    #[cfg(feature = "sqlite")]
                    RaftStreamRequest::AppendDB(req) => {
                        match state.raft_db.raft.append_entries(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::AppendDB(res).as_payload())
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

                    #[cfg(feature = "sqlite")]
                    RaftStreamRequest::VoteDB(req) => match state.raft_db.raft.vote(req).await {
                        Ok(res) => {
                            ws.write_frame(RaftStreamResponse::VoteDB(res).as_payload())
                                .await?;
                        }
                        Err(err) => {
                            ws.write_frame(
                                RaftStreamResponse::Error(Error::from(err)).as_payload(),
                            )
                            .await?;
                        }
                    },

                    #[cfg(feature = "sqlite")]
                    RaftStreamRequest::SnapshotDB(req) => {
                        info!(
                            "\n\ninstall db snapshot req in raft server: {:?} / {:?}\n\n",
                            req.vote, req.meta
                        );

                        match state.raft_db.raft.install_snapshot(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::SnapshotDB(res).as_payload())
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

                    #[cfg(feature = "cache")]
                    RaftStreamRequest::AppendCache(req) => {
                        match state.raft_cache.raft.append_entries(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::AppendCache(res).as_payload())
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

                    #[cfg(feature = "cache")]
                    RaftStreamRequest::VoteCache(req) => {
                        match state.raft_cache.raft.vote(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::VoteCache(res).as_payload())
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

                    #[cfg(feature = "cache")]
                    RaftStreamRequest::SnapshotCache(req) => {
                        info!(
                            "\n\ninstall cache snapshot req in raft server: {:?} / {:?}\n\n",
                            req.vote, req.meta
                        );

                        match state.raft_cache.raft.install_snapshot(req).await {
                            Ok(res) => {
                                ws.write_frame(RaftStreamResponse::SnapshotCache(res).as_payload())
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
