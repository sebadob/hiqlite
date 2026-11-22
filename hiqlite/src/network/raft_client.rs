use crate::Node;
use crate::NodeId;
use crate::app_state::RaftType;
use crate::helpers::{deserialize_serde, serialize_serde};
use crate::network::raft_server::{
    RaftStreamRequest, RaftStreamResponse, RaftStreamResponsePayload, SnapshotWireRequest,
};
use crate::network::{WireSnapshotMeta, WireVote, web_socket_connect};
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, WebSocketWrite};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use openraft::errors::RPCError;
use openraft::errors::Unreachable;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::{select, task, time};
use tracing::{debug, error, info};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigCache;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[cfg(any(feature = "cache", feature = "sqlite"))]
use crate::Error;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::OptionalSend;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::RaftTypeConfig;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::errors::ReplicationClosed;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::errors::StreamingError;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::network::{RPCOption, RaftNetworkFactory, RaftNetworkV2};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::raft::{
    AppendEntriesRequest, AppendResult, SnapshotResponse, TransferLeaderRequest,
    TransferLeaderResponse, VoteRequest, VoteResponse,
};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::type_config::alias::{SnapshotOf, VoteOf};

#[cfg(any(feature = "cache", feature = "sqlite"))]
const SNAPSHOT_CHUNK_SIZE_FALLBACK: usize = 3 * 1024 * 1024;

/// The wire's snapshot message builder, so the chunk helper stays generic over both groups.
type SnapshotRequestFn = fn(
    (
        oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        SnapshotWireRequest,
    ),
) -> RaftRequest;

pub struct NetworkStreaming {
    pub node_id: NodeId,
    pub tls_config: Option<Arc<rustls::ClientConfig>>,
    pub secret_raft: Vec<u8>,
    pub raft_type: RaftType,
    pub heartbeat_interval: u64,
    pub is_raft_stopped: Arc<AtomicBool>,
    pub is_startup_finished: Arc<AtomicBool>,
    // pub sender: flume::Sender<RaftRequest>,
}

#[cfg(feature = "cache")]
impl RaftNetworkFactory<TypeConfigCache> for NetworkStreaming {
    type Network = NetworkConnectionStreaming;

    #[tracing::instrument(level = "debug", skip_all)]
    async fn new_client(&mut self, _target: NodeId, node: &Node) -> Self::Network {
        debug!("Building new Raft Cache client with target {}", node);

        let (sender, rx) = flume::bounded(1);

        let task = tokio::task::spawn(Box::pin(Self::ws_handler(
            self.node_id,
            self.raft_type.clone(),
            node.clone(),
            self.tls_config.clone(),
            self.secret_raft.clone(),
            rx,
            self.heartbeat_interval,
            self.is_raft_stopped.clone(),
            self.is_startup_finished.clone(),
        )));

        NetworkConnectionStreaming {
            node: node.clone(),
            sender,
            task: Some(task),
        }
    }
}

#[cfg(feature = "sqlite")]
impl RaftNetworkFactory<TypeConfigSqlite> for NetworkStreaming {
    type Network = NetworkConnectionStreaming;

    #[tracing::instrument(level = "debug", skip_all)]
    async fn new_client(&mut self, _target: NodeId, node: &Node) -> Self::Network {
        debug!("Building new Raft DB client with target {}", node);

        let (sender, rx) = flume::bounded(1);

        let task = tokio::task::spawn(Box::pin(Self::ws_handler(
            self.node_id,
            self.raft_type.clone(),
            node.clone(),
            self.tls_config.clone(),
            self.secret_raft.clone(),
            rx,
            self.heartbeat_interval,
            self.is_raft_stopped.clone(),
            self.is_startup_finished.clone(),
        )));

        NetworkConnectionStreaming {
            node: node.clone(),
            sender,
            task: Some(task),
        }
    }
}

#[derive(Debug)]
enum RaftRequest {
    #[cfg(feature = "sqlite")]
    AppendDB(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            AppendEntriesRequest<TypeConfigSqlite>,
        ),
    ),
    #[cfg(feature = "sqlite")]
    VoteDB(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            VoteRequest<TypeConfigSqlite>,
        ),
    ),
    #[cfg(feature = "sqlite")]
    SnapshotDB(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            SnapshotWireRequest,
        ),
    ),
    #[cfg(feature = "sqlite")]
    TransferLeaderDB(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            TransferLeaderRequest<TypeConfigSqlite>,
        ),
    ),

    #[cfg(feature = "cache")]
    AppendCache(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            AppendEntriesRequest<TypeConfigCache>,
        ),
    ),
    #[cfg(feature = "cache")]
    VoteCache(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            VoteRequest<TypeConfigCache>,
        ),
    ),
    #[cfg(feature = "cache")]
    SnapshotCache(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            SnapshotWireRequest,
        ),
    ),
    #[cfg(feature = "cache")]
    TransferLeaderCache(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            TransferLeaderRequest<TypeConfigCache>,
        ),
    ),

    StreamResponse(RaftStreamResponse),

    ReaderExit,
    Shutdown,
}

#[derive(Debug)]
enum WritePayload {
    Payload(Vec<u8>),
    Close,
}

/// A [`JoinHandle`] that aborts its task when dropped.
///
/// `ws_handler` owns the handles of the reader and writer tasks that hold the
/// two halves of the WebSocket, and aborts them on its own reconnect path. That
/// path never runs when `ws_handler` is itself aborted, and dropping a bare
/// `JoinHandle` only detaches its task instead of cancelling it, so the two
/// tasks would keep the socket open for good. Wrapping the handles propagates
/// the cancellation: aborting `ws_handler` drops its locals, and dropping this
/// guard aborts the task it holds.
struct AbortOnDrop(JoinHandle<()>);

impl AbortOnDrop {
    /// Abort eagerly. Idempotent, so the explicit aborts on the reconnect path
    /// and the later `Drop` do not conflict.
    #[inline]
    fn abort(&self) {
        self.0.abort();
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[allow(clippy::type_complexity)]
impl NetworkStreaming {
    #[allow(clippy::too_many_arguments)]
    async fn ws_handler(
        this_node: NodeId,
        raft_type: RaftType,
        node: Node,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        secret: Vec<u8>,
        rx: flume::Receiver<RaftRequest>,
        heartbeat_interval: u64,
        is_raft_stopped: Arc<AtomicBool>,
        is_startup_finished: Arc<AtomicBool>,
    ) {
        let mut request_id = 0usize;
        // TODO probably, a Vec<_> is faster here since we would never have too many in flight reqs
        // for raft internal replication and voting? -> check
        // maybe feature-gate an alternative impl, even though it might not make the biggest difference
        let mut in_flight: HashMap<
            usize,
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        > = HashMap::with_capacity(4);
        let mut shutdown = false;

        'outer: loop {
            if is_raft_stopped.load(Ordering::Relaxed) {
                if !is_startup_finished.load(Ordering::Relaxed) {
                    debug!("Raft is still starting up - skipping initial connection");
                    time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                debug!("Raft is stopped - exiting NetworkStreaming::ws_handler()");
                break;
            }

            info!("Trying to open WebSocket stream");
            let socket = {
                match web_socket_connect::try_connect(
                    this_node,
                    &node.addr_raft,
                    &format!("/stream/{}", raft_type.as_str()),
                    tls_config.clone(),
                    &secret,
                )
                .await
                {
                    Ok(socket) => {
                        info!("WebSocket connected successfully");
                        socket
                    }
                    Err(err) => {
                        error!("Socket connect error to node {}: {:?}", node.id, err);

                        for _ in 0..3 {
                            // if there is a network error, no reason to try too hard to connect
                            time::sleep(Duration::from_millis(heartbeat_interval)).await;

                            // make sure channel is always free
                            if let Ok(req) = rx.try_recv() {
                                let ack = match req {
                                    #[cfg(feature = "sqlite")]
                                    RaftRequest::AppendDB((ack, _)) => Some(ack),
                                    #[cfg(feature = "sqlite")]
                                    RaftRequest::VoteDB((ack, _)) => Some(ack),
                                    #[cfg(feature = "sqlite")]
                                    RaftRequest::SnapshotDB((ack, _)) => Some(ack),
                                    #[cfg(feature = "sqlite")]
                                    RaftRequest::TransferLeaderDB((ack, _)) => Some(ack),
                                    #[cfg(feature = "cache")]
                                    RaftRequest::AppendCache((ack, _)) => Some(ack),
                                    #[cfg(feature = "cache")]
                                    RaftRequest::VoteCache((ack, _)) => Some(ack),
                                    #[cfg(feature = "cache")]
                                    RaftRequest::SnapshotCache((ack, _)) => Some(ack),
                                    #[cfg(feature = "cache")]
                                    RaftRequest::TransferLeaderCache((ack, _)) => Some(ack),
                                    RaftRequest::StreamResponse(_) => None,
                                    RaftRequest::ReaderExit => {
                                        continue;
                                    }
                                    RaftRequest::Shutdown => {
                                        break 'outer;
                                    }
                                };

                                if let Some(ack) = ack {
                                    let _ = ack.send(Err(Error::Connect(err.to_string())));
                                }
                            }
                        }

                        continue;
                    }
                }
            };
            assert!(
                in_flight.is_empty(),
                "raft in flight buffer should always be empty when restoring a connection"
            );

            let (tx_write, rx_write) = flume::bounded(1);
            let (tx_read, rx_read) = flume::bounded(1);

            // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
            let (read, write) = socket.split(tokio::io::split);
            // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
            let read = FragmentCollectorRead::new(read);

            // Guarded: `ws_handler` may be aborted before it reaches the
            // aborts below, and these must then cancel and not detach.
            let handle_read = AbortOnDrop(task::spawn(Box::pin(Self::stream_reader(
                read,
                tx_read.clone(),
            ))));
            let handle_write =
                AbortOnDrop(task::spawn(Box::pin(Self::stream_writer(write, rx_write))));

            loop {
                let res = select! {
                    res = rx_read.recv_async() => res,
                    res = rx.recv_async() => res,
                };

                let req = match res {
                    Ok(r) => r,
                    Err(err) => {
                        error!("Client stream reader error: {}", err,);

                        if rx.is_disconnected() {
                            debug!("Raft tx dropped - exiting Stream Reader");
                            shutdown = true;
                        }
                        if rx_read.is_disconnected() {
                            debug!("Client Stream reader exited - initiating shutdown + reconnect");
                        }

                        break;
                    }
                };

                let stream_req = match req {
                    #[cfg(feature = "sqlite")]
                    RaftRequest::AppendDB((ack, req)) => {
                        Some((ack, RaftStreamRequest::AppendDB((request_id, req))))
                    }
                    #[cfg(feature = "sqlite")]
                    RaftRequest::VoteDB((ack, req)) => {
                        Some((ack, RaftStreamRequest::VoteDB((request_id, req))))
                    }
                    #[cfg(feature = "sqlite")]
                    RaftRequest::SnapshotDB((ack, mut req)) => {
                        req.request_id = request_id;
                        Some((ack, RaftStreamRequest::SnapshotDB(req)))
                    }
                    #[cfg(feature = "sqlite")]
                    RaftRequest::TransferLeaderDB((ack, req)) => {
                        Some((ack, RaftStreamRequest::TransferLeaderDB((request_id, req))))
                    }

                    #[cfg(feature = "cache")]
                    RaftRequest::AppendCache((ack, req)) => {
                        Some((ack, RaftStreamRequest::AppendCache((request_id, req))))
                    }
                    #[cfg(feature = "cache")]
                    RaftRequest::VoteCache((ack, req)) => {
                        Some((ack, RaftStreamRequest::VoteCache((request_id, req))))
                    }
                    #[cfg(feature = "cache")]
                    RaftRequest::SnapshotCache((ack, mut req)) => {
                        req.request_id = request_id;
                        Some((ack, RaftStreamRequest::SnapshotCache(req)))
                    }
                    #[cfg(feature = "cache")]
                    RaftRequest::TransferLeaderCache((ack, req)) => Some((
                        ack,
                        RaftStreamRequest::TransferLeaderCache((request_id, req)),
                    )),

                    RaftRequest::StreamResponse(resp) => {
                        match in_flight.remove(&resp.request_id) {
                            None => {
                                error!("client ack for RaftStreamResponse missing");
                            }
                            Some(ack) => {
                                if ack.send(Ok(resp.payload)).is_err() {
                                    error!("sending back stream response from raft server");
                                }
                            }
                        }
                        None
                    }

                    RaftRequest::ReaderExit => {
                        debug!(
                            "ReaderExit - Client Stream reader exited - initiating shutdown + reconnect"
                        );
                        break;
                    }
                    RaftRequest::Shutdown => {
                        debug!("RaftRequest::Shutdown");
                        shutdown = true;
                        break;
                    }
                };

                if let Some((ack, payload)) = stream_req {
                    let bytes = serialize_serde(&payload);

                    if let Err(err) = tx_write.send_async(WritePayload::Payload(bytes)).await {
                        let _ = ack.send(Err(Error::Connect(format!(
                            "Error sending Write Request to WebSocket writer: {err}"
                        ))));
                        break;
                    }

                    in_flight.insert(request_id, ack);
                    request_id += 1;
                }
            }

            let _ = tx_write.send_async(WritePayload::Close).await;

            for (_, ack) in in_flight.drain() {
                let _ = ack.send(Err(Error::Connect("Raft WebSocket stream ended".into())));
            }
            // reset to a reasonable size for the next start to keep memory usage under control
            in_flight = HashMap::with_capacity(4);

            // Give the writer enough time to possibly send out the close request.
            // Since we need to re-connect, there is no need to rush anyway.
            time::sleep(Duration::from_millis(250)).await;

            handle_write.abort();
            handle_read.abort();

            if shutdown {
                break;
            }
        }

        debug!("Raft Client shut down, tx closed, exiting WsHandler");
    }

    async fn stream_reader(
        mut read: FragmentCollectorRead<ReadHalf<TokioIo<Upgraded>>>,
        tx: flume::Sender<RaftRequest>,
    ) {
        while let Ok(frame) = read
            .read_frame(&mut |frame| async move {
                // TODO obligated sends should be auto ping / pong / close ? -> verify!
                debug!(
                    "Received obligated send in stream client: OpCode: {:?}: {:?}",
                    frame.opcode.clone(),
                    frame.payload
                );
                Ok::<(), Error>(())
            })
            .await
        {
            match frame.opcode {
                OpCode::Continuation | OpCode::Text | OpCode::Ping | OpCode::Pong => {}
                OpCode::Binary => {
                    let bytes = &*frame.payload;
                    let payload = match deserialize_serde::<RaftStreamResponse>(bytes) {
                        Ok(payload) => payload,
                        Err(err) => {
                            // corrupt frame from a cluster member: drop the connection instead
                            // of panicking the raft client task
                            error!("Error deserializing raft response from member: {err:?}");
                            break;
                        }
                    };
                    if let Err(err) = tx.send_async(RaftRequest::StreamResponse(payload)).await {
                        error!(
                            "Error sending Response to Raft client stream manager: {:?}",
                            err
                        );
                    }
                }
                OpCode::Close => break,
            }
        }

        let _ = tx.send_async(RaftRequest::ReaderExit).await;
        debug!("Exiting Client Stream Reader");
    }

    async fn stream_writer(
        mut write: WebSocketWrite<WriteHalf<TokioIo<Upgraded>>>,
        rx: flume::Receiver<WritePayload>,
    ) {
        while let Ok(payload) = rx.recv_async().await {
            match payload {
                WritePayload::Payload(bytes) => {
                    let frame = Frame::binary(Payload::from(bytes));
                    if let Err(err) = write.write_frame(frame).await {
                        error!("Client Stream error: {:?}", err);
                        break;
                    }
                }
                WritePayload::Close => {
                    debug!("Received Close request in Client Stream Writer");
                    let _ = write.write_frame(Frame::close(1000, b"go away")).await;
                    break;
                }
            }
        }

        debug!("Exiting Client Stream Writer");
    }
}

#[allow(clippy::type_complexity)]
pub struct NetworkConnectionStreaming {
    node: Node,
    sender: flume::Sender<RaftRequest>,
    task: Option<JoinHandle<()>>,
}

impl Drop for NetworkConnectionStreaming {
    fn drop(&mut self) {
        let _ = self.sender.try_send(RaftRequest::Shutdown);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl NetworkConnectionStreaming {
    #[inline]
    async fn send(
        &mut self,
        req: RaftRequest,
        rx: oneshot::Receiver<Result<RaftStreamResponsePayload, Error>>,
    ) -> Result<RaftStreamResponsePayload, Error> {
        tracing::debug!(
            req = debug(&req),
            "sending rpc request to {}",
            self.node.addr_raft
        );

        self.sender.send_async(req).await.map_err(|err| {
            error!(
                "NetworkConnectionStreaming::send to node {}: {}",
                self.node.id,
                err.to_string()
            );
            Error::Connect(err.to_string())
        })?;

        rx.await.map_err(|err| Error::Connect(err.to_string()))?
    }
}

#[cfg(feature = "sqlite")]
impl RaftNetworkV2<TypeConfigSqlite> for NetworkConnectionStreaming {
    type SnapshotData = tokio::fs::File;

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfigSqlite>,
        _option: RPCOption,
    ) -> Result<AppendResult<TypeConfigSqlite>, RPCError<TypeConfigSqlite>> {
        let prev_log_id = req.prev_log_id;
        let last_log_id = req.entries.last().map(|entry| entry.log_id);

        let (ack, rx) = oneshot::channel();
        match self
            .send(RaftRequest::AppendDB((ack, req)), rx)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?
        {
            RaftStreamResponsePayload::AppendDB(res) => res
                // An append request without entries only asks for an ack, which is the request's
                // `prev_log_id` - openraft asserts the ack to be inside the (empty) sent range.
                .map(|res| res.into_append_result(prev_log_id, last_log_id.or(prev_log_id)))
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err))),
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn vote(
        &mut self,
        req: VoteRequest<TypeConfigSqlite>,
        _option: RPCOption,
    ) -> Result<VoteResponse<TypeConfigSqlite>, RPCError<TypeConfigSqlite>> {
        let (ack, rx) = oneshot::channel();
        match self
            .send(RaftRequest::VoteDB((ack, req)), rx)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?
        {
            RaftStreamResponsePayload::VoteDB(res) => {
                res.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn transfer_leader(
        &mut self,
        req: TransferLeaderRequest<TypeConfigSqlite>,
        _option: RPCOption,
    ) -> Result<TransferLeaderResponse<TypeConfigSqlite>, RPCError<TypeConfigSqlite>> {
        let (ack, rx) = oneshot::channel();
        match self
            .send(RaftRequest::TransferLeaderDB((ack, req)), rx)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?
        {
            RaftStreamResponsePayload::TransferLeaderDB(res) => {
                res.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn full_snapshot(
        &mut self,
        vote: VoteOf<TypeConfigSqlite>,
        snapshot: SnapshotOf<TypeConfigSqlite, Self::SnapshotData>,
        cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        option: RPCOption,
    ) -> Result<SnapshotResponse<TypeConfigSqlite>, StreamingError<TypeConfigSqlite>> {
        tokio::pin!(cancel);
        let meta = snapshot.meta;
        let snapshot = snapshot.snapshot;
        let payload = self
            .chunk_stream(
                vote,
                meta,
                snapshot,
                &mut cancel,
                option,
                RaftRequest::SnapshotDB,
            )
            .await?;
        match payload {
            RaftStreamResponsePayload::SnapshotDB(res) => res,
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "cache")]
impl RaftNetworkV2<TypeConfigCache> for NetworkConnectionStreaming {
    #[cfg(not(feature = "in-memory-snapshots"))]
    type SnapshotData = tokio::fs::File;
    #[cfg(feature = "in-memory-snapshots")]
    type SnapshotData = std::io::Cursor<Vec<u8>>;

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfigCache>,
        _option: RPCOption,
    ) -> Result<AppendResult<TypeConfigCache>, RPCError<TypeConfigCache>> {
        let prev_log_id = rpc.prev_log_id;
        let last_log_id = rpc.entries.last().map(|entry| entry.log_id);

        let (ack, rx) = oneshot::channel();
        match self
            .send(RaftRequest::AppendCache((ack, rpc)), rx)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?
        {
            RaftStreamResponsePayload::AppendCache(res) => res
                // An append request without entries only asks for an ack, which is the request's
                // `prev_log_id` - openraft asserts the ack to be inside the (empty) sent range.
                .map(|res| res.into_append_result(prev_log_id, last_log_id.or(prev_log_id)))
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err))),
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn vote(
        &mut self,
        rpc: VoteRequest<TypeConfigCache>,
        _option: RPCOption,
    ) -> Result<VoteResponse<TypeConfigCache>, RPCError<TypeConfigCache>> {
        let (ack, rx) = oneshot::channel();
        match self
            .send(RaftRequest::VoteCache((ack, rpc)), rx)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?
        {
            RaftStreamResponsePayload::VoteCache(res) => {
                res.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn transfer_leader(
        &mut self,
        rpc: TransferLeaderRequest<TypeConfigCache>,
        _option: RPCOption,
    ) -> Result<TransferLeaderResponse<TypeConfigCache>, RPCError<TypeConfigCache>> {
        let (ack, rx) = oneshot::channel();
        match self
            .send(RaftRequest::TransferLeaderCache((ack, rpc)), rx)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?
        {
            RaftStreamResponsePayload::TransferLeaderCache(res) => {
                res.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn full_snapshot(
        &mut self,
        vote: VoteOf<TypeConfigCache>,
        snapshot: SnapshotOf<TypeConfigCache, Self::SnapshotData>,
        cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        option: RPCOption,
    ) -> Result<SnapshotResponse<TypeConfigCache>, StreamingError<TypeConfigCache>> {
        tokio::pin!(cancel);
        let meta = snapshot.meta;
        let snapshot = snapshot.snapshot;
        let payload = self
            .chunk_stream(
                vote,
                meta,
                snapshot,
                &mut cancel,
                option,
                RaftRequest::SnapshotCache,
            )
            .await?;
        match payload {
            RaftStreamResponsePayload::SnapshotCache(res) => res,
            _ => unreachable!(),
        }
    }
}

impl NetworkConnectionStreaming {
    #[cfg(any(feature = "cache", feature = "sqlite"))]
    /// openraft 0.10 hands the network a whole snapshot instead of a payload, so the chunking
    /// over the WebSocket is ours again: every chunk is one message with its own ack, and only
    /// the last one is answered with the real response.
    async fn chunk_stream<C, R>(
        &mut self,
        vote: WireVote,
        meta: WireSnapshotMeta,
        mut data: R,
        mut cancel: impl Future<Output = ReplicationClosed> + Unpin,
        option: RPCOption,
        snapshot_request: SnapshotRequestFn,
    ) -> Result<RaftStreamResponsePayload, StreamingError<C>>
    where
        C: RaftTypeConfig,
        R: tokio::io::AsyncRead + Unpin + OptionalSend,
    {
        use tokio::io::AsyncReadExt;

        let chunk_size = option
            .snapshot_chunk_size()
            .unwrap_or(SNAPSHOT_CHUNK_SIZE_FALLBACK);
        let mut buf = vec![0u8; chunk_size];
        let mut first = true;

        loop {
            let read = data
                .read(&mut buf)
                .await
                .map_err(|err| StreamingError::Unreachable(Unreachable::new(&err)))?;
            let last = read == 0;

            let (ack, rx) = oneshot::channel();
            let req = SnapshotWireRequest {
                request_id: 0,
                vote,
                meta: meta.clone(),
                data: buf[..read].to_vec(),
                first,
                last,
            };
            first = false;

            let payload = select! {
                payload = self.send(snapshot_request((ack, req)), rx) => {
                    payload.map_err(|err| StreamingError::Unreachable(Unreachable::new(&err)))?
                },
                closed = &mut cancel => return Err(StreamingError::Closed(closed)),
            };

            if last {
                return Ok(payload);
            }
        }
    }
}
