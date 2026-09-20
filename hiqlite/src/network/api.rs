use crate::app_state::RaftType;
use crate::helpers::{deserialize, get_raft_metrics, serialize};
use crate::network::handshake::HandshakeSecret;
use crate::network::{AppStateExt, Error, validate_secret};
use crate::{APP_VERSION, Node};
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use chrono::Utc;
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, upgrade};
use openraft::{ServerState, StoredMembership};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::ops::{Deref, Sub};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::{task, time};
use tracing::{debug, error, info, warn};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::{
    kv_handler::CacheRequestHandler,
    state_machine::{CacheRequest, CacheResponse},
};

#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::{
    LockAwaitPayload, LockRequest, LockState,
};

#[cfg(feature = "sqlite")]
use crate::{
    migration::Migration,
    query::{query_consistent_local, query_owned_local, rows::RowOwned},
    store::state_machine::sqlite::state_machine::{Query, QueryWrite},
};

#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::NotifyRequest;

pub async fn health(state: AppStateExt) -> Result<(), Error> {
    #[cfg(all(not(feature = "sqlite"), not(feature = "cache")))]
    panic!("neither `sqlite` nor `cache` feature enabled");

    #[cfg(any(feature = "sqlite", feature = "cache"))]
    {
        if check_health(&state).await.is_err() {
            // after at least 3 seconds, we should have a new leader
            time::sleep(Duration::from_secs(3)).await;
            check_health(&state).await?;
        }
    }

    Ok(())
}

#[cfg(any(feature = "sqlite", feature = "cache"))]
async fn check_health(state: &AppStateExt) -> Result<(), Error> {
    if Utc::now().sub(state.health_check_delay) < state.app_start {
        info!("Early health check within the HQL_HEALTH_CHECK_DELAY timeframe - returning true");
        return Ok(());
    }

    #[cfg(feature = "sqlite")]
    {
        let metrics = state.raft_db.raft.metrics().borrow().clone();
        metrics.running_state?;
        if metrics.current_leader.is_none() {
            return Err(Error::LeaderChange(
                "The leader voting process has not finished yet for Raft DB".into(),
            ));
        }
    }
    #[cfg(feature = "cache")]
    {
        let metrics = state.raft_cache.raft.metrics().borrow().clone();
        metrics.running_state?;
        if metrics.current_leader.is_none() {
            return Err(Error::LeaderChange(
                "The leader voting process has not finished yet Raft Cache".into(),
            ));
        }
    }

    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn ready(state: AppStateExt) -> Result<(), Error> {
    #[cfg(all(not(feature = "sqlite"), not(feature = "cache")))]
    panic!("neither `sqlite` nor `cache` feature enabled");

    if state.is_shutting_down.load(Ordering::Relaxed) {
        return Err(Error::Error("Node is shutting down".into()));
    }

    let secs_since_start = Utc::now().sub(state.app_start).num_seconds();

    #[cfg(feature = "sqlite")]
    {
        if !state.raft_db.is_startup_finished.load(Ordering::Relaxed) {
            warn!("Node is still starting up (sqlite)");
            return Err(Error::Error("Node is still starting up (sqlite)".into()));
        }

        // to avoid a chicken-and-egg problem, a pristine node 1 should always return ready
        let is_pristine_node_1 =
            state.id == 1 && !state.raft_db.raft.is_initialized().await? && secs_since_start > 10;

        if !is_pristine_node_1 {
            if state.raft_db.is_raft_stopped.load(Ordering::Relaxed) {
                warn!("sqlite raft is not running");
                return Err(Error::Error("sqlite raft is not running".into()));
            }

            let metrics = get_raft_metrics(&state, &RaftType::Sqlite).await;
            ensure_ready_member(
                state.id,
                state.learner_only,
                metrics.state,
                &metrics.membership_config,
                "sqlite",
            )?;

            if metrics.current_leader.is_none() && (state.id != 1 || secs_since_start < 10) {
                warn!("sqlite raft leader vote in progress - secs_since_start: {secs_since_start}");
                return Err(Error::Error("sqlite raft leader vote in progress".into()));
            }
        }
    }

    #[cfg(feature = "cache")]
    {
        if !state.raft_cache.is_startup_finished.load(Ordering::Relaxed) {
            warn!("Node is still starting up (cache)");
            return Err(Error::Error("Node is still starting up (cache)".into()));
        }

        let is_pristine_node_1 = state.id == 1
            && !state.raft_cache.raft.is_initialized().await?
            && secs_since_start > 10;

        if !is_pristine_node_1 {
            if state.raft_cache.is_raft_stopped.load(Ordering::Relaxed) {
                warn!("cache raft is not running");
                return Err(Error::Error("cache raft is not running".into()));
            }

            let metrics = get_raft_metrics(&state, &RaftType::Cache).await;
            ensure_ready_member(
                state.id,
                state.learner_only,
                metrics.state,
                &metrics.membership_config,
                "cache",
            )?;

            if metrics.current_leader.is_none() && (state.id != 1 || secs_since_start < 10) {
                warn!("cache raft leader vote in progress");
                return Err(Error::Error(
                    "cache raft leader vote in progress - secs_since_start: {secs_since_start}"
                        .into(),
                ));
            }
        }
    }

    Ok(())
}

fn ensure_ready_member(
    id: u64,
    learner_only: bool,
    state: ServerState,
    membership_config: &StoredMembership<u64, Node>,
    raft_label: &'static str,
) -> Result<(), Error> {
    if state == ServerState::Shutdown {
        warn!("not yet a ready member of the {raft_label} raft");
        return Err(Error::Error(
            format!("not yet a ready member of the {raft_label} raft").into(),
        ));
    }

    if state != ServerState::Learner && membership_config.voter_ids().any(|voter_id| voter_id == id)
    {
        return Ok(());
    }

    if learner_only
        && state == ServerState::Learner
        && membership_config
            .nodes()
            .any(|(member_id, _)| *member_id == id)
    {
        return Ok(());
    }

    warn!("not yet a ready member of the {raft_label} raft");
    Err(Error::Error(
        format!("not yet a ready member of the {raft_label} raft").into(),
    ))
}

pub async fn post_create_backup(state: AppStateExt, headers: HeaderMap) -> Result<(), Error> {
    validate_secret(&state, &headers)?;

    #[cfg(all(feature = "backup", feature = "sqlite"))]
    {
        let mut leader = 0;
        for _ in 0..5 {
            match state.raft_db.raft.current_leader().await {
                None => {
                    time::sleep(Duration::from_secs(1)).await;
                }
                Some(current) => {
                    leader = current;
                    break;
                }
            }
        }
        if leader == 0 {
            return Err(Error::LeaderChange("Leader election ongoing".into()));
        }

        let now = Utc::now().timestamp();
        if leader == state.id {
            state
                .raft_db
                .raft
                .client_write(QueryWrite::Backup((state.id, now)))
                .await?;
        } else {
            let (ack, rx) = tokio::sync::oneshot::channel();
            state
                .tx_client_stream
                .send_async(crate::client::stream::ClientStreamReq::Backup(
                    crate::client::stream::ClientBackupPayload {
                        request_id: state.new_request_id(),
                        node_id: leader,
                        ts: now,
                        ack,
                    },
                ))
                .await
                .map_err(|err| Error::Error(err.to_string().into()))?;
            rx.await
                .map_err(|err| Error::Error(err.to_string().into()))??;
        }
    }

    Ok(())
}

pub async fn ping() {}

pub async fn get_version() -> impl IntoResponse {
    APP_VERSION
}

#[cfg(test)]
mod tests {
    use super::ensure_ready_member;
    use crate::Node;
    use openraft::{Membership, ServerState, StoredMembership};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn learner_only_readiness_accepts_committed_learner_member() {
        let membership = membership_with_voters_and_learners([1, 2], [1, 2, 3]);

        assert!(ensure_ready_member(3, true, ServerState::Learner, &membership, "test").is_ok());
    }

    #[test]
    fn learner_only_readiness_rejects_non_member_learner() {
        let membership = membership_with_voters_and_learners([1, 2], [1, 2]);

        assert!(ensure_ready_member(3, true, ServerState::Learner, &membership, "test").is_err());
    }

    #[test]
    fn learner_only_readiness_rejects_learners_when_disabled() {
        let membership = membership_with_voters_and_learners([1, 2], [1, 2, 3]);

        assert!(ensure_ready_member(3, false, ServerState::Learner, &membership, "test").is_err());
    }

    #[test]
    fn member_readiness_rejects_learner_state_without_learner_only() {
        let membership = membership_with_voters_and_learners([1, 2, 3], [1, 2, 3]);

        assert!(ensure_ready_member(3, false, ServerState::Learner, &membership, "test").is_err());
    }

    #[test]
    fn member_readiness_accepts_voter_in_non_learner_state() {
        let membership = membership_with_voters_and_learners([1, 2, 3], [1, 2, 3]);

        assert!(ensure_ready_member(3, false, ServerState::Follower, &membership, "test").is_ok());
    }

    #[test]
    fn member_readiness_rejects_shutdown_voter() {
        let membership = membership_with_voters_and_learners([1, 2, 3], [1, 2, 3]);

        assert!(ensure_ready_member(3, false, ServerState::Shutdown, &membership, "test").is_err());
    }

    fn membership_with_voters_and_learners<const VOTERS: usize, const MEMBERS: usize>(
        voters: [u64; VOTERS],
        members: [u64; MEMBERS],
    ) -> StoredMembership<u64, Node> {
        let voters = BTreeSet::from(voters);
        let nodes = members
            .into_iter()
            .map(|id| {
                (
                    id,
                    Node {
                        id,
                        addr_raft: format!("localhost:{}", 8100 + id),
                        addr_api: format!("localhost:{}", 8200 + id),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        StoredMembership::new(None, Membership::new(vec![voters], nodes))
    }
}

#[cfg(feature = "listen_notify")]
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

#[cfg(feature = "listen_notify")]
async fn handle_listen_socket(
    state: AppStateExt,
    socket: upgrade::UpgradeFut,
) -> Result<(), fastwebsockets::WebSocketError> {
    let mut ws = socket.await?;
    ws.set_auto_close(true);

    if let Err(err) = HandshakeSecret::server(&mut ws, state.secret_api.as_bytes()).await {
        error!("Error during /listen WebSocket handshake: {}", err);
        let _ = ws
            .write_frame(Frame::close(1000, b"Invalid Handshake"))
            .await;
        return Ok(());
    }

    // Register this connection as a listener with the state machine.
    let (tx_notify, rx_notify) = flume::bounded::<(i64, Vec<u8>)>(1);
    if let Err(err) = state
        .raft_cache
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
    let handle_read = task::spawn(async move {
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

#[cfg(not(feature = "listen_notify"))]
pub async fn listen(state: AppStateExt, headers: HeaderMap) -> Result<(), Error> {
    validate_secret(&state, &headers)?;
    Err(Error::Config(
        "'listen_notify' feature is not active".into(),
    ))
}

/// This is the WebSocket stream a Raft client (Followers) connects to.
pub async fn stream(
    state: AppStateExt,
    Path(raft_type): Path<RaftType>,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    let (response, socket) = ws.upgrade()?;
    debug!("New Raft Stream for {:?}", raft_type);

    #[cfg(feature = "cache")]
    {
        if !state.raft_cache.is_startup_finished.load(Ordering::Relaxed) {
            warn!("Cache Raft still starting up - rejecting client streaming connection");
            return Err(Error::BadRequest("Raft is still starting up".into()));
        }
        if state.raft_cache.is_raft_stopped.load(Ordering::Relaxed) {
            warn!("Cache Raft has been stopped - rejecting client streaming connection");
            return Err(Error::BadRequest("Raft has been stopped".into()));
        }
    }
    #[cfg(feature = "sqlite")]
    {
        if !state.raft_db.is_startup_finished.load(Ordering::Relaxed) {
            warn!("Sqlite Raft still starting up - rejecting client streaming connection");
            return Err(Error::BadRequest("Raft is still starting up".into()));
        }
        if state.raft_db.is_raft_stopped.load(Ordering::Relaxed) {
            warn!("Sqlite Raft has been stopped - rejecting client streaming connection");
            return Err(Error::BadRequest("Raft has been stopped".into()));
        }
    }

    tokio::task::spawn(async move {
        if let Err(err) = handle_socket_concurrent(state, socket).await {
            error!("Error in websocket connection: {}", err);
        }
    });

    Ok(response)
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiStreamRequest {
    pub(crate) request_id: usize,
    pub(crate) payload: ApiStreamRequestPayload,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum ApiStreamRequestPayload {
    #[cfg(feature = "sqlite")]
    Execute(Query),
    #[cfg(feature = "sqlite")]
    ExecuteReturning(Query),
    #[cfg(feature = "sqlite")]
    Transaction(Vec<Query>),
    #[cfg(feature = "sqlite")]
    QueryConsistent(Query),
    #[cfg(feature = "sqlite")]
    Batch(std::borrow::Cow<'static, str>),
    #[cfg(feature = "sqlite")]
    Migrate(Vec<Migration>),

    #[cfg(feature = "backup")]
    Backup((crate::NodeId, i64)),

    #[cfg(feature = "cache")]
    KV(CacheRequest),

    // remote-only clients
    #[cfg(feature = "sqlite")]
    Query(Query),
    #[cfg(feature = "cache")]
    KVGet(CacheRequest),
    #[cfg(feature = "dlock")]
    LockAwait(CacheRequest),
    #[cfg(feature = "listen_notify")]
    Notify(CacheRequest),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiStreamResponse {
    pub(crate) request_id: usize,
    pub(crate) result: ApiStreamResponsePayload,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum ApiStreamResponsePayload {
    #[cfg(feature = "sqlite")]
    Execute(Result<usize, Error>),
    #[cfg(feature = "sqlite")]
    ExecuteReturning(Result<Vec<Result<RowOwned, Error>>, Error>),
    #[cfg(feature = "sqlite")]
    Transaction(Result<Vec<Result<usize, Error>>, Error>),
    #[cfg(feature = "sqlite")]
    Query(Result<Vec<RowOwned>, Error>),
    #[cfg(feature = "sqlite")]
    QueryConsistent(Result<Vec<RowOwned>, Error>),
    #[cfg(feature = "sqlite")]
    Batch(Result<Vec<Result<usize, Error>>, Error>),
    #[cfg(feature = "sqlite")]
    Migrate(Result<(), Error>),

    #[cfg(feature = "backup")]
    Backup(Result<(), Error>),

    #[cfg(feature = "cache")]
    KV(Result<CacheResponse, Error>),

    #[cfg(feature = "dlock")]
    Lock(Result<LockState, Error>),

    #[cfg(feature = "listen_notify")]
    Notify(Result<(), Error>),
}

#[derive(Debug)]
pub(crate) enum WsWriteMsg {
    Payload(ApiStreamResponse),
    /// A frame the WebSocket library generated and obligates us to send back to the peer:
    /// a Pong in response to a Ping, or the echo of a Close frame. `payload` is the raw
    /// payload of the corresponding inbound frame, `opcode` tells us which one it was.
    ObligatedSend {
        opcode: OpCode,
        payload: Vec<u8>,
    },
    Break,
}

async fn handle_socket_concurrent(
    state: AppStateExt,
    socket: upgrade::UpgradeFut,
) -> Result<(), fastwebsockets::WebSocketError> {
    let mut ws = socket.await?;
    ws.set_auto_close(true);

    let _client_id = match HandshakeSecret::server(&mut ws, state.secret_api.as_bytes()).await {
        Ok(id) => id,
        Err(err) => {
            error!("Error during WebSocket handshake: {}", err);
            ws.write_frame(Frame::close(1000, b"Invalid Handshake"))
                .await?;
            return Ok(());
        }
    };

    let (tx_write, rx_write) = flume::bounded::<WsWriteMsg>(1);

    // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
    let (rx, mut write) = ws.split(tokio::io::split);
    // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
    let mut read = FragmentCollectorRead::new(rx);

    let handle_write = task::spawn(async move {
        while let Ok(req) = rx_write.recv_async().await {
            match req {
                WsWriteMsg::Payload(resp) => {
                    let bytes = match serialize(&resp) {
                        Ok(bytes) => bytes,
                        Err(err) => {
                            error!(
                                "Error serializing response payload - closing connection: {}",
                                err
                            );
                            break;
                        }
                    };
                    let frame = Frame::binary(Payload::Borrowed(&bytes));
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
                    // we ignore any errors here since it may be possible that the reader
                    // has closed already - we just try a graceful connection close
                    debug!("handle_socket_concurrent -> server stream break message");
                    break;
                }
            }
        }

        let _ = write
            .write_frame(Frame::close(1000, b"Invalid Request"))
            .await;

        debug!("handle_socket_concurrent -> server stream exiting");
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
                let bytes = frame.payload.deref();
                match deserialize::<ApiStreamRequest>(bytes) {
                    Ok(req) => req,
                    Err(err) => {
                        error!("Error deserializing ApiStreamRequest: {:?}", err);
                        let _ = tx_write.send_async(WsWriteMsg::Break).await;
                        break;
                    }
                }
            }
            _ => {
                let _ = tx_write.send_async(WsWriteMsg::Break).await;
                break;
            }
        };

        let state = state.clone();
        let tx_write = tx_write.clone();
        task::spawn(async move {
            let request_id = req.request_id;

            let res = match req.payload {
                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Execute(sql) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Execute(sql))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Execute(res) => res.result,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Execute(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Execute(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::ExecuteReturning(sql) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::ExecuteReturning(sql))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::ExecuteReturning(res) => res.result,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::ExecuteReturning(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::ExecuteReturning(Err(Error::from(
                                err,
                            ))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Transaction(queries) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Transaction(queries))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Transaction(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Transaction(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Transaction(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::QueryConsistent(Query { sql, params }) => {
                    let res = query_consistent_local(
                        &state.raft_db.raft,
                        state.raft_db.log_statements,
                        state.raft_db.read_pool.clone(),
                        sql,
                        params,
                    )
                    .await;

                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::QueryConsistent(res),
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Batch(sql) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Batch(sql))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Batch(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Batch(res.result),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Batch(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Migrate(migrations) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Migration(migrations))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Migrate(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Migrate(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Migrate(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "backup")]
                ApiStreamRequestPayload::Backup((node_id, ts)) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Backup((node_id, ts)))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Backup(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Backup(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Backup(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Query(Query { sql, params }) => {
                    let res = query_owned_local(
                        state.raft_db.log_statements,
                        state.raft_db.read_pool.clone(),
                        sql,
                        params,
                    )
                    .await;

                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Query(res),
                    }
                }

                #[cfg(feature = "cache")]
                ApiStreamRequestPayload::KV(cache_req) => {
                    // Bounds-check the cache index before committing to the raft log. Embedded
                    // clients resolve indices locally, so an out-of-range index can only arrive
                    // via a hand-crafted wire request; rejecting it here keeps the state
                    // machine's `.get(idx).unwrap()` in `apply()` from panicking on it.
                    let cache_idx = match cache_req {
                        CacheRequest::Get { cache_idx, .. }
                        | CacheRequest::Put { cache_idx, .. }
                        | CacheRequest::GetRemove { cache_idx, .. }
                        | CacheRequest::Replace { cache_idx, .. }
                        | CacheRequest::Delete { cache_idx, .. }
                        | CacheRequest::Clear { cache_idx, .. }
                        | CacheRequest::ClearCounters { cache_idx, .. }
                        | CacheRequest::CounterGet { cache_idx, .. }
                        | CacheRequest::CounterSet { cache_idx, .. }
                        | CacheRequest::CounterAdd { cache_idx, .. }
                        | CacheRequest::CounterDel { cache_idx, .. } => Some(cache_idx),
                        CacheRequest::ClearAll
                        | CacheRequest::Notify(_)
                        | CacheRequest::Lock(_)
                        | CacheRequest::LockAwait(_)
                        | CacheRequest::LockRelease(_) => None,
                    };

                    if let Some(cache_idx) = cache_idx
                        && cache_idx >= state.raft_cache.tx_caches.len()
                    {
                        ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::KV(Err(Error::new(format!(
                                "cache index {cache_idx} out of range (0..{})",
                                state.raft_cache.tx_caches.len()
                            )))),
                        }
                    } else {
                        match state.raft_cache.raft.client_write(cache_req).await {
                            Ok(resp) => {
                                let resp: CacheResponse = resp.data;
                                ApiStreamResponse {
                                    request_id,
                                    result: ApiStreamResponsePayload::KV(Ok(resp)),
                                }
                            }
                            Err(err) => ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::KV(Err(Error::from(err))),
                            },
                        }
                    }
                }

                #[cfg(feature = "cache")]
                ApiStreamRequestPayload::KVGet(cache_req) => {
                    let (cache_idx, key) = match cache_req {
                        CacheRequest::Get { cache_idx, key } => (cache_idx, key),
                        _ => unreachable!(),
                    };

                    let (ack, rx) = tokio::sync::oneshot::channel();
                    state
                        .raft_cache
                        .tx_caches
                        .get(cache_idx)
                        .unwrap()
                        .send(CacheRequestHandler::Get { key, reply: ack })
                        .expect("kv handler to always be running");
                    let value = rx.await.expect("to always get an answer from kv handler");
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::KV(Ok(CacheResponse::Value(value))),
                    }
                }

                #[cfg(feature = "dlock")]
                ApiStreamRequestPayload::LockAwait(cache_req) => {
                    let (key, id) = match cache_req {
                        CacheRequest::LockAwait((key, id)) => (key, id),
                        _ => unreachable!(),
                    };

                    let (ack, rx) = tokio::sync::oneshot::channel();
                    state
                        .raft_cache
                        .tx_dlock
                        .send(LockRequest::Await(LockAwaitPayload { key, id, ack }))
                        .expect("kv handler to always be running");
                    let lock_state = rx
                        .await
                        .expect("to always get an answer from the kv handler");

                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Lock(Ok(lock_state)),
                    }
                }

                #[cfg(feature = "listen_notify")]
                ApiStreamRequestPayload::Notify(cache_req) => {
                    let (ts, data) = match cache_req {
                        CacheRequest::Notify((ts, data)) => (ts, data),
                        _ => unreachable!(),
                    };

                    match state
                        .raft_cache
                        .raft
                        .client_write(CacheRequest::Notify((ts, data)))
                        .await
                    {
                        Ok(_) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Notify(Ok(())),
                        },
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Notify(Err(Error::from(err))),
                        },
                    }
                }
            };

            if let Err(err) = tx_write.send_async(WsWriteMsg::Payload(res)).await {
                error!("Error sending payload to tx_write - exiting: {}", err);
            }
        });
    }

    // ignore the result in case the writer has already exited and drop the channel
    // on purpose to make sure a maybe still running writer catches it
    let _ = tx_write.send_async(WsWriteMsg::Break).await;
    drop(tx_write);

    // may panic in a race condition during shutdown
    let _ = handle_write.await;

    debug!("handle_socket_concurrent exiting");

    Ok(())
}
