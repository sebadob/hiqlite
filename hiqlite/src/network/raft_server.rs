use crate::helpers::{deserialize_serde, serialize_serde};
use crate::network::handshake::HandshakeSecret;
use crate::network::{AppStateExt, Error};
use axum::response::IntoResponse;
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, upgrade};
use openraft::errors::{Fatal, RaftError, StreamingError, Unreachable};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tokio::task;
use tracing::{debug, error, warn};

use crate::network::{WireSnapshotMeta, WireVote};
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigCache;
#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, SnapshotResponse, TransferLeaderRequest,
    TransferLeaderResponse, VoteRequest, VoteResponse,
};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::type_config::alias::SnapshotOf;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Serialize, Deserialize)]
pub enum RaftStreamRequest {
    #[cfg(feature = "sqlite")]
    AppendDB((usize, AppendEntriesRequest<TypeConfigSqlite>)),
    #[cfg(feature = "sqlite")]
    VoteDB((usize, VoteRequest<TypeConfigSqlite>)),
    #[cfg(feature = "sqlite")]
    SnapshotDB(SnapshotWireRequest),
    #[cfg(feature = "sqlite")]
    TransferLeaderDB((usize, TransferLeaderRequest<TypeConfigSqlite>)),

    #[cfg(feature = "cache")]
    AppendCache((usize, AppendEntriesRequest<TypeConfigCache>)),
    #[cfg(feature = "cache")]
    VoteCache((usize, VoteRequest<TypeConfigCache>)),
    #[cfg(feature = "cache")]
    SnapshotCache(SnapshotWireRequest),
    #[cfg(feature = "cache")]
    TransferLeaderCache((usize, TransferLeaderRequest<TypeConfigCache>)),
}

/// One chunk of a streamed snapshot. openraft hands the network the snapshot data instead
/// of a payload, so the chunking stays ours and `vote` + `meta` travel with every chunk.
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotWireRequest {
    pub request_id: usize,
    pub vote: WireVote,
    pub meta: WireSnapshotMeta,
    pub data: Vec<u8>,
    pub first: bool,
    pub last: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RaftStreamResponse {
    pub request_id: usize,
    pub payload: RaftStreamResponsePayload,
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Serialize, Deserialize)]
pub enum RaftStreamResponsePayload {
    #[cfg(feature = "sqlite")]
    AppendDB(Result<AppendEntriesResponse<TypeConfigSqlite>, RaftError<TypeConfigSqlite>>),
    #[cfg(feature = "sqlite")]
    VoteDB(Result<VoteResponse<TypeConfigSqlite>, RaftError<TypeConfigSqlite>>),
    #[cfg(feature = "sqlite")]
    SnapshotDB(Result<SnapshotResponse<TypeConfigSqlite>, StreamingError<TypeConfigSqlite>>),
    #[cfg(feature = "sqlite")]
    TransferLeaderDB(Result<TransferLeaderResponse<TypeConfigSqlite>, RaftError<TypeConfigSqlite>>),

    #[cfg(feature = "cache")]
    AppendCache(Result<AppendEntriesResponse<TypeConfigCache>, RaftError<TypeConfigCache>>),
    #[cfg(feature = "cache")]
    VoteCache(Result<VoteResponse<TypeConfigCache>, RaftError<TypeConfigCache>>),
    #[cfg(feature = "cache")]
    SnapshotCache(Result<SnapshotResponse<TypeConfigCache>, StreamingError<TypeConfigCache>>),
    #[cfg(feature = "cache")]
    TransferLeaderCache(
        Result<TransferLeaderResponse<TypeConfigCache>, RaftError<TypeConfigCache>>,
    ),
}

#[derive(Debug)]
pub(crate) enum WsWriteMsg {
    Payload(Vec<u8>),
    /// A frame the WebSocket library generated and obligates us to send back to the peer:
    /// a Pong in response to a Ping, or the echo of a Close frame. `payload` is the raw
    /// payload of the corresponding inbound frame, `opcode` tells us which one it was.
    ObligatedSend {
        opcode: OpCode,
        payload: Vec<u8>,
    },
    Break,
}

pub async fn stream_cache(
    state: AppStateExt,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Incoming WebSocket stream for Cache");

    #[cfg(feature = "cache")]
    {
        if !state.raft_cache.is_startup_finished.load(Ordering::Relaxed) {
            warn!("Cache Raft still starting up - rejecting streaming connection");
            return Err(Error::BadRequest("Raft is still starting up".into()));
        }
        if state.raft_cache.is_raft_stopped.load(Ordering::Relaxed) {
            warn!("Cache Raft has been stopped - rejecting streaming connection");
            return Err(Error::BadRequest("Raft has been stopped".into()));
        }
    }

    tracing::info!("WebSocket cache stream request accepted");

    let (response, socket) = ws.upgrade()?;
    tokio::task::spawn(Box::pin(async move {
        if let Err(err) = handle_socket(state, socket).await {
            debug!("Cache WebSocket stream closed: {}", err);
        }
    }));

    Ok(response)
}

pub async fn stream_sqlite(
    state: AppStateExt,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Incoming WebSocket stream for SQLite");

    #[cfg(feature = "sqlite")]
    {
        if !state.raft_db.is_startup_finished.load(Ordering::Relaxed) {
            warn!("Sqlite Raft still starting up - rejecting streaming connection");
            return Err(Error::BadRequest("Raft is still starting up".into()));
        }
        if state.raft_db.is_raft_stopped.load(Ordering::Relaxed) {
            warn!("Sqlite Raft has been stopped - rejecting streaming connection");
            return Err(Error::BadRequest("Raft has been stopped".into()));
        }
    }

    tracing::info!("WebSocket sqlite stream request accepted");

    let (response, socket) = ws.upgrade()?;
    tokio::task::spawn(Box::pin(async move {
        if let Err(err) = handle_socket(state, socket).await {
            debug!("SQLite WebSocket stream closed: {}", err);
        }
    }));

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

    let (tx_write, rx_write) = flume::bounded::<WsWriteMsg>(1);
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
                WsWriteMsg::ObligatedSend { opcode, payload } => {
                    // The library generated this frame and expects us to send it verbatim.
                    let frame = if opcode == OpCode::Pong {
                        Frame::pong(Payload::Owned(payload))
                    } else {
                        // The only other obligated send the library produces is the Close echo.
                        Frame::close_raw(Payload::Owned(payload))
                    };
                    if let Err(err) = write.write_frame(frame).await {
                        error!("Error writing obligated WebSocket frame: {}", err);
                        break;
                    }
                }
                WsWriteMsg::Break => {
                    debug!("handle_socket -> server stream break message");
                    break;
                }
            }
        }

        debug!("handle_socket -> Raft server WebSocket writer exiting");
        let _ = write.write_frame(Frame::close(1000, b"go away")).await;
    });

    while let Ok(frame) = read
        .read_frame(&mut |frame| {
            // The library generates the obligated frame (Pong for Ping, Close echo) and
            // expects us to send it back on the socket - forward it to the writer task.
            let tx_write = tx_write.clone();
            async move {
                if let Err(err) = tx_write
                    .send_async(WsWriteMsg::ObligatedSend {
                        opcode: frame.opcode,
                        payload: frame.payload.to_vec(),
                    })
                    .await
                {
                    error!(
                        "Error forwarding obligated WebSocket frame to writer (OpCode {:?}): {}",
                        frame.opcode, err
                    );
                }
                Ok::<(), Error>(())
            }
        })
        .await
    {
        let req = match frame.opcode {
            OpCode::Close => {
                debug!("received Close frame in server stream");
                break;
            }
            OpCode::Binary => {
                let bytes = &*frame.payload;
                match deserialize_serde::<RaftStreamRequest>(bytes) {
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
                let res = state
                    .raft_db
                    .raft
                    .append_entries(req)
                    .await
                    .map(AppendEntriesResponse::from);
                if let Err(RaftError::Fatal(Fatal::Stopped)) = &res {
                    debug!("Raft DB stopped - exiting");
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
            RaftStreamRequest::SnapshotDB(req) => {
                let request_id = req.request_id;
                let res = snapshot_chunk_db(&state, req).await;
                (request_id, RaftStreamResponsePayload::SnapshotDB(res))
            }
            #[cfg(feature = "sqlite")]
            RaftStreamRequest::TransferLeaderDB((request_id, req)) => {
                let res = state
                    .raft_db
                    .raft
                    .handle_transfer_leader(req)
                    .await
                    .map_err(RaftError::Fatal);
                (request_id, RaftStreamResponsePayload::TransferLeaderDB(res))
            }

            #[cfg(feature = "cache")]
            RaftStreamRequest::AppendCache((request_id, req)) => {
                let res = state
                    .raft_cache
                    .raft
                    .append_entries(req)
                    .await
                    .map(AppendEntriesResponse::from);
                if let Err(RaftError::Fatal(Fatal::Stopped)) = &res {
                    debug!("Raft Cache stopped - exiting");
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
            RaftStreamRequest::SnapshotCache(req) => {
                let request_id = req.request_id;
                let res = snapshot_chunk_cache(&state, req).await;
                (request_id, RaftStreamResponsePayload::SnapshotCache(res))
            }
            #[cfg(feature = "cache")]
            RaftStreamRequest::TransferLeaderCache((request_id, req)) => {
                let res = state
                    .raft_cache
                    .raft
                    .handle_transfer_leader(req)
                    .await
                    .map_err(RaftError::Fatal);
                (
                    request_id,
                    RaftStreamResponsePayload::TransferLeaderCache(res),
                )
            }
        };

        if let Err(err) = tx_write
            .send_async(WsWriteMsg::Payload(serialize_serde(&RaftStreamResponse {
                request_id,
                payload,
            })))
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

    debug!("handle_socket exiting");

    Ok(())
}

/// openraft hands the network the snapshot data instead of a payload, so the chunk assembly is
/// ours: every chunk lands in the receive file the state machine reads, and only the last one is
/// handed to the Raft. Earlier chunks get the sender's vote echoed back as an ack.
#[cfg(feature = "sqlite")]
async fn snapshot_chunk_db(
    state: &AppStateExt,
    req: SnapshotWireRequest,
) -> Result<SnapshotResponse<TypeConfigSqlite>, StreamingError<TypeConfigSqlite>> {
    let receiver = receiver_file(&state.raft_db.path_snapshots, "temp", &req)
        .await
        .map_err(|err| StreamingError::Unreachable(Unreachable::new(&err)))?;

    if !req.last {
        return Ok(SnapshotResponse::new(req.vote));
    }

    let snapshot = SnapshotOf::<TypeConfigSqlite, tokio::fs::File> {
        meta: req.meta,
        snapshot: receiver,
    };

    state
        .raft_db
        .raft
        .install_full_snapshot(req.vote, snapshot)
        .await
        .map_err(streaming_from_fatal)
}

#[cfg(feature = "cache")]
async fn snapshot_chunk_cache(
    state: &AppStateExt,
    req: SnapshotWireRequest,
) -> Result<SnapshotResponse<TypeConfigCache>, StreamingError<TypeConfigCache>> {
    let receiver = receiver_file(&state.raft_cache.path_snapshots, "temp~", &req)
        .await
        .map_err(|err| StreamingError::Unreachable(Unreachable::new(&err)))?;

    if !req.last {
        return Ok(SnapshotResponse::new(req.vote));
    }

    // only the `in-memory-snapshots` build wants the data in the handle itself
    #[cfg(not(feature = "in-memory-snapshots"))]
    let data = receiver;
    #[cfg(feature = "in-memory-snapshots")]
    let data = {
        let path = format!("{}/temp~", state.raft_cache.path_snapshots);
        drop(receiver);
        std::io::Cursor::new(
            tokio::fs::read(&path)
                .await
                .map_err(|err| StreamingError::Unreachable(Unreachable::new(&err)))?,
        )
    };

    let snapshot = SnapshotOf::<TypeConfigCache, _> {
        meta: req.meta,
        snapshot: data,
    };

    state
        .raft_cache
        .raft
        .install_full_snapshot(req.vote, snapshot)
        .await
        .map_err(streaming_from_fatal)
}

/// Write one chunk to the snapshot's receive file. `first` truncates on purpose, so a leftover
/// file from a crashed run is never appended to.
async fn receiver_file(
    dir: &str,
    name: &str,
    req: &SnapshotWireRequest,
) -> Result<tokio::fs::File, std::io::Error> {
    use tokio::io::AsyncWriteExt;

    let path = format!("{dir}/{name}");

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!req.first)
        .truncate(req.first)
        .open(&path)
        .await?;

    if !req.data.is_empty() {
        file.write_all(&req.data).await?;
    }

    // the state machine reads this file by path, so everything must be on disk already
    file.flush().await?;

    Ok(file)
}

fn streaming_from_fatal<C>(fatal: Fatal<C>) -> StreamingError<C>
where
    C: openraft::RaftTypeConfig,
{
    match fatal {
        Fatal::StorageError(err) => StreamingError::StorageError(err),
        // A stopped or panicked Raft has no streaming-level meaning, so the sender sees a closed
        // replication stream and retries, which is what the old InstallSnapshotError did.
        other => StreamingError::Closed(openraft::errors::ReplicationClosed::new(other)),
    }
}
