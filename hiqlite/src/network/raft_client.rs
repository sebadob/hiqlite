use crate::Node;
use crate::NodeId;
use crate::app_state::RaftType;
use crate::helpers::{deserialize_serde, serialize_serde};
use crate::network::raft_server::{
    RaftStreamRequest, RaftStreamResponse, RaftStreamResponsePayload, SnapshotWireRequest,
};
use crate::network::{WireSnapshotMeta, WireVote, web_socket_connect};
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, WebSocketWrite};
use futures_util::Stream;
use futures_util::StreamExt;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use openraft::errors::RPCError;
use openraft::errors::Timeout;
use openraft::errors::Unreachable;
use rustc_hash::FxHashMap;
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
use openraft::base::{BoxFuture, BoxStream};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::entry::RaftEntry;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::errors::RaftError;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::errors::ReplicationClosed;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::errors::StreamingError;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::network::RPCTypes;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::network::{RPCOption, RaftNetworkFactory, RaftNetworkV2};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, AppendResult, SnapshotResponse,
    TransferLeaderRequest, TransferLeaderResponse, VoteRequest, VoteResponse,
};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::type_config::alias::{SnapshotOf, VoteOf};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::{OptionalSend, RaftTypeConfig};

#[cfg(any(feature = "cache", feature = "sqlite"))]
const SNAPSHOT_CHUNK_SIZE_FALLBACK: usize = 3 * 1024 * 1024;

/// `AppendEntries` RPCs a single replication stream keeps on the socket at the same time.
///
/// 1 is openraft's own sequential behaviour: one request, its answer, then the next. Anything
/// above 1 pipelines, meaning the leader sends the next request while the previous one is still
/// being answered, so the round trip stops being paid per batch. The follower answers appends
/// through `Raft::stream_append`, whose own pipeline buffer is 64, so any depth up to that is
/// safe on both ends.
///
/// The depth used when `HQL_RAFT_STREAM_DEPTH` is unset, unparsable, or out of range.
#[cfg(any(feature = "cache", feature = "sqlite"))]
const DEFAULT_RAFT_STREAM_DEPTH: usize = 32;

/// openraft's own pipeline buffer size on the follower side, the practical ceiling for a depth.
#[cfg(any(feature = "cache", feature = "sqlite"))]
const MAX_RAFT_STREAM_DEPTH: usize = 64;

/// TEMPORARY, for the depth sweep only: the value is read once from `HQL_RAFT_STREAM_DEPTH`,
/// clamped into the safe range, so a benchmark run does not rebuild the crate per value. Revert
/// this to a plain `const` before merging.
#[cfg(any(feature = "cache", feature = "sqlite"))]
pub(crate) static RAFT_STREAM_DEPTH: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| {
    let env = "HQL_RAFT_STREAM_DEPTH";
    let depth = std::env::var(env)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_RAFT_STREAM_DEPTH);
    if depth != DEFAULT_RAFT_STREAM_DEPTH {
        info!("Raft stream depth set to {depth} by {env}");
    }
    depth.clamp(1, MAX_RAFT_STREAM_DEPTH)
});

/// Multiplier on openraft's `soft_ttl()` for the answer deadline of one pipelined append.
///
/// openraft hands a replication stream its heartbeat interval as the soft TTL, and a follower
/// that has a snapshot install in front of its queue needs far more than one heartbeat to answer
/// the requests queued behind it.
#[cfg(any(feature = "cache", feature = "sqlite"))]
const RAFT_STREAM_TTL_FACTOR: u32 = 8;

/// Lower bound for that deadline, matching the default `install_snapshot_timeout` a snapshot in
/// front of the queue can take.
#[cfg(any(feature = "cache", feature = "sqlite"))]
const RAFT_STREAM_TTL_FLOOR: Duration = Duration::from_secs(10);

/// The answer of one group's `AppendEntries` RPC, as it travels back over the wire.
type AppendAnswer<C> = Result<AppendEntriesResponse<C>, RaftError<C>>;

/// The boxed answer stream openraft's `stream_append` expects from a transport.
type AppendStream<'s, C> = BoxStream<'s, Result<AppendResult<C>, RPCError<C>>>;

/// What opening that stream answers: the stream itself, or the reason it never opened.
type OpenAppendStream<'s, C> = Result<AppendStream<'s, C>, RPCError<C>>;

/// The wire pair of one Raft group's `AppendEntries` RPC: how to wrap the request for the socket,
/// and how to take the answer back out of a response payload.
///
/// The two travel together so the request type of one group can never be paired with the answer
/// type of the other.
#[cfg(any(feature = "cache", feature = "sqlite"))]
trait AppendWire: Copy + 'static {
    type Config: RaftTypeConfig<NodeId = NodeId>;

    /// Wraps an `AppendEntries` request and its ack into the message for the socket.
    fn request(
        ack: oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        req: AppendEntriesRequest<Self::Config>,
    ) -> RaftRequest;

    /// Takes the `AppendEntries` answer out of a response payload, `None` for any other payload.
    fn response(payload: RaftStreamResponsePayload) -> Option<AppendAnswer<Self::Config>>;
}

#[cfg(feature = "sqlite")]
#[derive(Clone, Copy)]
struct AppendDb;

#[cfg(feature = "sqlite")]
impl AppendWire for AppendDb {
    type Config = TypeConfigSqlite;

    fn request(
        ack: oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        req: AppendEntriesRequest<Self::Config>,
    ) -> RaftRequest {
        RaftRequest::AppendDB((ack, req))
    }

    fn response(payload: RaftStreamResponsePayload) -> Option<AppendAnswer<Self::Config>> {
        match payload {
            RaftStreamResponsePayload::AppendDB(res) => Some(res),
            _ => None,
        }
    }
}

#[cfg(feature = "cache")]
#[derive(Clone, Copy)]
struct AppendCache;

#[cfg(feature = "cache")]
impl AppendWire for AppendCache {
    type Config = TypeConfigCache;

    fn request(
        ack: oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        req: AppendEntriesRequest<Self::Config>,
    ) -> RaftRequest {
        RaftRequest::AppendCache((ack, req))
    }

    fn response(payload: RaftStreamResponsePayload) -> Option<AppendAnswer<Self::Config>> {
        match payload {
            RaftStreamResponsePayload::AppendCache(res) => Some(res),
            _ => None,
        }
    }
}

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

        // Unbounded on purpose: `Drop` signals shutdown with a `try_send`, which must never fail,
        // and a replication stream only ever queues `RAFT_STREAM_DEPTH` appends plus a few unary
        // RPCs that each wait for their own answer.
        let (sender, rx) = flume::unbounded();

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
            local_id: self.node_id,
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

        // Unbounded for the same reason as the Cache group above.
        let (sender, rx) = flume::unbounded();

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
            local_id: self.node_id,
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
        // Every request the socket has sent and not answered yet, keyed by its id. A pipelined
        // replication stream holds up to `RAFT_STREAM_DEPTH` appends here at once, plus the unary
        // RPCs (vote, snapshot chunk, leadership transfer) that each wait for their own answer.
        // All of them are failed on the reconnect path, so the map is empty again before a new
        // connection starts.
        let mut in_flight: FxHashMap<
            usize,
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        > = FxHashMap::default();
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

                        // if there is a network error, no reason to try too hard to connect
                        time::sleep(Duration::from_millis(heartbeat_interval)).await;

                        // Nothing reaches the socket while we are not connected, so every queued
                        // request has to be failed here. A pipelined stream queues up to
                        // `RAFT_STREAM_DEPTH` appends at once, and the channel is unbounded, so
                        // this has to drain fully rather than take a fixed number of items.
                        while let Ok(req) = rx.try_recv() {
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

                        continue;
                    }
                }
            };
            debug_assert!(
                in_flight.is_empty(),
                "raft in flight buffer should always be empty when restoring a connection"
            );

            // Both channels carry a whole pipeline: `RAFT_STREAM_DEPTH` requests go out and their
            // answers come back in bursts, so a capacity of 1 would serialise them all over again.
            let (tx_write, rx_write) = flume::bounded(*RAFT_STREAM_DEPTH);
            let (tx_read, rx_read) = flume::bounded(*RAFT_STREAM_DEPTH);

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

            // Drop our copy of the reader's sender so the reader task's clone is the only one
            // left. The reader exiting is then visible as a disconnected channel.
            drop(tx_read);

            // Frames the previous connection's reader had already queued belong to requests that
            // path failed when it broke the connection. Drop them rather than matching them
            // against this connection's ids.
            while let Ok(req) = rx_read.try_recv() {
                debug!("Dropping a frame of the previous raft connection: {req:?}");
            }

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

            // A response for a request sent on the dead socket can never arrive on the next one:
            // the server answers every request on the connection it received it on.
            for (_, ack) in in_flight.drain() {
                let _ = ack.send(Err(Error::Connect("Raft WebSocket stream ended".into())));
            }
            // replace the map so the allocation a large drain leaves behind is released
            in_flight = FxHashMap::default();

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
    /// The id of the node this connection was built for, used in openraft's timeout errors.
    local_id: NodeId,
    node: Node,
    sender: flume::Sender<RaftRequest>,
    task: Option<JoinHandle<()>>,
}

impl Drop for NetworkConnectionStreaming {
    fn drop(&mut self) {
        // The request channel is unbounded, so this never fails on a full queue.
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
        match AppendDb::response(
            self.send(RaftRequest::AppendDB((ack, req)), rx)
                .await
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?,
        ) {
            // An append request without entries only asks for an ack, which is the request's
            // `prev_log_id` - openraft asserts the ack to be inside the (empty) sent range.
            Some(res) => res
                .map(|res| res.into_append_result(prev_log_id, last_log_id.or(prev_log_id)))
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err))),
            None => Err(RPCError::Unreachable(Unreachable::from_string(
                "Raft append answer carries a different payload type",
            ))),
        }
    }

    /// Pipelined instead of openraft's sequential default - see `append_stream`.
    fn stream_append<S>(
        &mut self,
        input: S,
        option: RPCOption,
    ) -> BoxFuture<'_, OpenAppendStream<'_, TypeConfigSqlite>>
    where
        S: Stream<Item = AppendEntriesRequest<TypeConfigSqlite>> + OptionalSend + Unpin + 'static,
    {
        self.append_stream::<AppendDb, S>(input, &option)
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
        match AppendCache::response(
            self.send(RaftRequest::AppendCache((ack, rpc)), rx)
                .await
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?,
        ) {
            // An append request without entries only asks for an ack, which is the request's
            // `prev_log_id` - openraft asserts the ack to be inside the (empty) sent range.
            Some(res) => res
                .map(|res| res.into_append_result(prev_log_id, last_log_id.or(prev_log_id)))
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err))),
            None => Err(RPCError::Unreachable(Unreachable::from_string(
                "Raft append answer carries a different payload type",
            ))),
        }
    }

    /// Pipelined instead of openraft's sequential default - see `append_stream`.
    fn stream_append<S>(
        &mut self,
        input: S,
        option: RPCOption,
    ) -> BoxFuture<'_, OpenAppendStream<'_, TypeConfigCache>>
    where
        S: Stream<Item = AppendEntriesRequest<TypeConfigCache>> + OptionalSend + Unpin + 'static,
    {
        self.append_stream::<AppendCache, S>(input, &option)
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

    /// The pipelined transport behind `RaftNetworkV2::stream_append`.
    ///
    /// `buffered()` is the whole pipeline: it keeps up to `RAFT_STREAM_DEPTH` RPCs on the socket at
    /// the same time and yields their answers in request order, which is exactly what openraft
    /// asks of a pipelined transport.
    ///
    /// Termination follows openraft's own sequential default. A full acknowledgement is what lets
    /// the stream go on, and openraft stops at the first answer that is not one - a conflict, a
    /// higher vote, an RPC error - so nothing queued behind it is ever asked for. When the input
    /// stream ends, `buffered` drains what is still on the socket and the answer stream ends.
    ///
    /// The input stream is only ever polled by `map()` and `buffered()`, never by a `select!`,
    /// because openraft builds it with `stream::unfold`: dropping a pending `next()` of an `unfold`
    /// stream destroys its state, and the stream would end as if it had run out of entries.
    fn append_stream<'s, W, S>(
        &self,
        input: S,
        option: &RPCOption,
    ) -> BoxFuture<'s, OpenAppendStream<'s, W::Config>>
    where
        W: AppendWire,
        S: Stream<Item = AppendEntriesRequest<W::Config>> + OptionalSend + Unpin + 'static,
    {
        let sender = self.sender.clone();
        let ids = (self.local_id, self.node.id);
        let ttl = Self::response_ttl(option);

        let stream = input
            .map(move |req| {
                let sender = sender.clone();
                async move { Self::send_append::<W>(&sender, ids, req, ttl).await }
            })
            .buffered(*RAFT_STREAM_DEPTH);

        Box::pin(async move { Ok(Box::pin(stream) as BoxStream<'s, _>) })
    }

    /// Sends one `AppendEntries` RPC on the shared socket and waits for its answer. Several of these
    /// run at the same time, one per pipelined request, and the socket's reader routes each answer
    /// back through this call's own ack.
    async fn send_append<W>(
        sender: &flume::Sender<RaftRequest>,
        ids: (NodeId, NodeId),
        req: AppendEntriesRequest<W::Config>,
        ttl: Duration,
    ) -> Result<AppendResult<W::Config>, RPCError<W::Config>>
    where
        W: AppendWire,
    {
        let prev_log_id = req.prev_log_id.clone();
        // An append request without entries only asks for an ack, which is the request's
        // `prev_log_id` - openraft asserts the ack to be inside the (empty) sent range.
        let last_log_id = req
            .entries
            .last()
            .map(|entry| entry.log_id())
            .or(prev_log_id.clone());

        let (ack, rx) = oneshot::channel();
        sender
            .send_async(W::request(ack, req))
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?;

        match time::timeout(ttl, rx).await {
            Err(_elapsed) => Err(RPCError::Timeout(Timeout {
                action: RPCTypes::AppendEntries,
                id: ids.0,
                target: ids.1,
                timeout: ttl,
            })),
            // The ack is dropped by `ws_handler` when the connection ends, which is the same
            // "we do not know what happened to this request" case a connect failure is.
            Ok(Err(_)) => Err(RPCError::Unreachable(Unreachable::from_string(
                "Raft stream answer dropped",
            ))),
            Ok(Ok(Err(err))) => Err(RPCError::Unreachable(Unreachable::new(&err))),
            Ok(Ok(Ok(payload))) => W::response(payload)
                .ok_or_else(|| {
                    RPCError::Unreachable(Unreachable::from_string(
                        "Raft stream answer carries a different payload type",
                    ))
                })?
                .map(move |res| res.into_append_result(prev_log_id, last_log_id))
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err))),
        }
    }

    /// The answer deadline openraft asks for with `RPCOption`, kept inside the bounds a pipelined
    /// stream can actually respect.
    fn response_ttl(option: &RPCOption) -> Duration {
        option
            .soft_ttl()
            .saturating_mul(RAFT_STREAM_TTL_FACTOR)
            .max(RAFT_STREAM_TTL_FLOOR)
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::store::state_machine::sqlite::TypeConfigSqlite;
    use futures_util::stream;
    use openraft::Vote;
    use openraft::impls::leader_id_adv::LeaderId;
    use openraft::log_id::LogId;
    use openraft::type_config::alias::LogIdOf;
    use openraft::type_config::alias::VoteOf;

    /// The proof of the pipeline: every request of a stream has to reach the socket before any of
    /// them is answered, and answers that come back out of order have to be yielded in request
    /// order again.
    #[tokio::test]
    async fn append_stream_sends_the_whole_pipeline_and_answers_in_order() {
        let (tx, rx) = flume::unbounded();
        let conn = NetworkConnectionStreaming {
            local_id: 1,
            node: Node {
                id: 2,
                addr_api: "127.0.0.1:8102".to_string(),
                addr_raft: "127.0.0.1:8202".to_string(),
            },
            sender: tx,
            task: None,
        };

        let requests: Vec<_> = (1..=3u64)
            .map(|index| AppendEntriesRequest::<TypeConfigSqlite> {
                vote: VoteOf::<TypeConfigSqlite>::from(Vote::new_committed(1, 1)),
                prev_log_id: Some(LogIdOf::<TypeConfigSqlite>::from(LogId {
                    leader_id: LeaderId {
                        term: 1u64,
                        node_id: 1u64,
                    },
                    index,
                })),
                leader_commit: None,
                entries: vec![],
            })
            .collect();
        let expected: Vec<_> = requests.iter().map(|req| req.prev_log_id).collect();

        let mut answers = Box::pin(
            conn.append_stream::<AppendDb, _>(
                stream::iter(requests),
                &RPCOption::new(Duration::from_millis(500)),
            )
            .await
            .unwrap(),
        );

        // Nothing is answered yet, so the first poll must not resolve. By the time it gives up,
        // all three requests have to sit on the socket already.
        let first = time::timeout(Duration::from_millis(50), answers.next()).await;
        assert!(
            first.is_err(),
            "an answer arrived before the pipeline was sent"
        );

        let mut acks = Vec::new();
        for _ in 0..3 {
            match rx.try_recv().expect("a request never reached the socket") {
                RaftRequest::AppendDB((ack, _)) => acks.push(ack),
                other => panic!("unexpected request on the socket: {other:?}"),
            }
        }
        assert!(
            rx.try_recv().is_err(),
            "the stream sent more requests than its input had"
        );

        // Answer them out of order; the stream has to put them back.
        let third = acks.pop().unwrap();
        let second = acks.pop().unwrap();
        let first_ack = acks.pop().unwrap();
        for ack in [third, first_ack, second] {
            ack.send(Ok(RaftStreamResponsePayload::AppendDB(Ok(
                AppendEntriesResponse::Success,
            ))))
            .expect("the answer was not taken");
        }

        let mut got = Vec::new();
        for _ in 0..3 {
            let answer = time::timeout(Duration::from_millis(500), answers.next())
                .await
                .expect("the stream stalled")
                .expect("the stream ended too early")
                .expect("the stream yielded an RPC error");
            match answer {
                Ok(matching) => got.push(matching),
                Err(err) => panic!("the stream reported an append error: {err}"),
            }
        }
        assert_eq!(got, expected);
    }
}
