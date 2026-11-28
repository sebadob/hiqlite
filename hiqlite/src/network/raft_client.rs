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
use futures_util::stream;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use openraft::errors::RPCError;
use openraft::errors::Unreachable;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::{select, task, time};
use tracing::{debug, error, info, warn};

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
use openraft::network::{RPCOption, RaftNetworkFactory, RaftNetworkV2};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, AppendResult, SnapshotResponse,
    TransferLeaderRequest, TransferLeaderResponse, VoteRequest, VoteResponse,
};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::type_config::alias::LogIdOf;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::type_config::alias::{SnapshotOf, VoteOf};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::{OptionalSend, RaftTypeConfig};

#[cfg(any(feature = "cache", feature = "sqlite"))]
const SNAPSHOT_CHUNK_SIZE_FALLBACK: usize = 4 * 1024 * 1024;

/// The answer of one group's `AppendEntries` RPC, as it travels back over the wire.
type AppendAnswer<C> = Result<AppendEntriesResponse<C>, RaftError<C>>;

/// The way back to a caller that is waiting for one request's answer, handed over with the request.
type Ack = oneshot::Sender<Result<RaftStreamResponsePayload, Error>>;

/// The boxed answer stream openraft's `stream_append` expects from a transport.
type AppendStream<'s, C> = BoxStream<'s, Result<AppendResult<C>, RPCError<C>>>;

/// What opening that stream answers: the stream itself, or the reason it never opened.
type OpenAppendStream<'s, C> = Result<AppendStream<'s, C>, RPCError<C>>;

/// One answer of a replication stream, and whether it acknowledges the whole request it answers.
type StreamAnswerOf<C> = (Result<AppendResult<C>, RPCError<C>>, bool);

/// The answer of one request of a replication stream, together with the log range the request
/// carried. The range travels with the answer because openraft's acknowledgement is the range's end,
/// and the stream pairs nothing: the connection hands the answers over in request order.
pub(crate) struct StreamAnswer<C: RaftTypeConfig> {
    payload: Result<RaftStreamResponsePayload, Error>,
    prev_log_id: Option<LogIdOf<C>>,
    last_log_id: Option<LogIdOf<C>>,
}

/// The answer channel of one replication stream, plus the log range of the request it belongs to.
pub(crate) struct StreamAck<C: RaftTypeConfig> {
    sender: flume::Sender<StreamAnswer<C>>,
    prev_log_id: Option<LogIdOf<C>>,
    last_log_id: Option<LogIdOf<C>>,
}

impl<C: RaftTypeConfig> StreamAck<C> {
    /// Hand over the answer of the request this ack belongs to.
    #[inline]
    fn answer(self, payload: Result<RaftStreamResponsePayload, Error>) -> bool {
        self.sender
            .send(StreamAnswer {
                payload,
                prev_log_id: self.prev_log_id,
                last_log_id: self.last_log_id,
            })
            .is_ok()
    }
}

/// Where the answer of one request goes back to.
pub(crate) enum AckTarget {
    /// A single RPC caller waiting for exactly this answer.
    Call(Ack),
    /// The replication stream that sent this `AppendEntries` to the DB group.
    #[cfg(feature = "sqlite")]
    AppendDb(StreamAck<TypeConfigSqlite>),
    /// The replication stream that sent this `AppendEntries` to the cache group.
    #[cfg(feature = "cache")]
    AppendCache(StreamAck<TypeConfigCache>),
}

impl std::fmt::Debug for AckTarget {
    /// The target is a channel, not data, and it travels inside `RaftRequest`, which the send path
    /// logs - naming the target is all a log line can use there.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call(_) => f.write_str("ack caller"),
            #[cfg(feature = "sqlite")]
            Self::AppendDb(_) => f.write_str("ack append stream"),
            #[cfg(feature = "cache")]
            Self::AppendCache(_) => f.write_str("ack append stream"),
        }
    }
}

impl AckTarget {
    /// Hand one answer to its owner. `false` when that owner is gone, which is the normal end of a
    /// replication stream: openraft stops consuming answers the moment its session ends.
    #[inline]
    fn deliver(self, payload: RaftStreamResponsePayload) -> bool {
        match self {
            AckTarget::Call(ack) => ack.send(Ok(payload)).is_ok(),
            #[cfg(feature = "sqlite")]
            AckTarget::AppendDb(ack) => ack.answer(Ok(payload)),
            #[cfg(feature = "cache")]
            AckTarget::AppendCache(ack) => ack.answer(Ok(payload)),
        }
    }

    /// Fail the request because the connection it was sent on is gone. A replication stream gets
    /// the failure as an answer, which is how its stream learns that the connection ended.
    fn fail(self, err: Error) {
        match self {
            AckTarget::Call(ack) => {
                let _ = ack.send(Err(err));
            }
            #[cfg(feature = "sqlite")]
            AckTarget::AppendDb(ack) => {
                let _ = ack.answer(Err(err));
            }
            #[cfg(feature = "cache")]
            AckTarget::AppendCache(ack) => {
                let _ = ack.answer(Err(err));
            }
        }
    }
}

/// Every request the connection has sent and that is not answered yet, keyed by the id the server
/// echoes back in its answer.
///
/// The server answers a connection in the order it received its requests, so answers normally
/// arrive in send order. Keeping the ids makes that an assumption and not a requirement: an answer
/// naming a later request waits until the request the connection expects next is answered, and
/// everything that queued up behind it is delivered with it.
/// Slots the reorder buffer starts with.
///
/// The buffer only ever holds the requests that are in flight on one connection, and openraft keeps
/// at most `max_payload_entries` of them per replication stream. Half of its default covers a normal
/// connection, and the map grows from there when a stream runs deeper.
const INFLIGHT_CAPACITY: usize = 16;

/// The reorder buffer of one connection.
///
/// The socket answers a connection in the order it received its requests, and openraft wants the
/// answers of a replication stream in that same order. A hash map on the request id is keyed by
/// exactly what an answer names, and ids are integers, so the hash is a multiply and a shift.
struct InFlight {
    /// The id of the oldest request that is not delivered yet.
    expected: usize,
    /// Sent, not answered.
    targets: rustc_hash::FxHashMap<usize, AckTarget>,
    /// Answered, not delivered.
    ready: rustc_hash::FxHashMap<usize, RaftStreamResponsePayload>,
}

impl InFlight {
    /// A reorder map with the buffer's starting capacity, keyed by the request id.
    fn map<V>() -> rustc_hash::FxHashMap<usize, V> {
        rustc_hash::FxHashMap::with_capacity_and_hasher(
            INFLIGHT_CAPACITY,
            rustc_hash::FxBuildHasher,
        )
    }

    fn new() -> Self {
        Self {
            expected: 0,
            targets: Self::map(),
            ready: Self::map(),
        }
    }

    /// Register a request that goes onto the socket. The id is the connection's own send counter, so
    /// a queue that has just emptied starts expecting this one.
    #[inline]
    fn push(&mut self, id: usize, target: AckTarget) {
        if self.targets.is_empty() {
            self.expected = id;
        }
        self.targets.insert(id, target);
    }

    /// Deliver one answer, together with everything that had queued up behind it. `false` for an id
    /// the connection never sent: that answer belongs to no caller, and delivering it would
    /// acknowledge a request nobody is waiting for.
    #[inline]
    fn answer(&mut self, id: usize, payload: RaftStreamResponsePayload) -> bool {
        if !self.targets.contains_key(&id) {
            return false;
        }
        self.ready.insert(id, payload);

        while let Some(payload) = self.ready.remove(&self.expected) {
            let target = self
                .targets
                .remove(&self.expected)
                .expect("an answer in the reorder buffer always has the target it was sent with");
            self.expected += 1;
            if !target.deliver(payload) {
                error!("sending back stream response from raft server");
            }
        }

        true
    }

    /// Requests sent and not answered yet.
    fn len(&self) -> usize {
        self.targets.len()
    }

    fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Fail everything the connection sent on a socket that is gone. A response for those requests
    /// can never arrive on the next connection: the server answers every request on the connection
    /// it received it on.
    fn fail_all(&mut self, err: &str) {
        for (_, target) in std::mem::take(&mut self.targets) {
            target.fail(Error::Connect(err.into()));
        }
        self.ready.clear();
    }
}

/// The wire pair of one Raft group's `AppendEntries` RPC: how to wrap the request for the socket,
/// and how to take the answer back out of a response payload.
///
/// The two travel together so the request type of one group can never be paired with the answer
/// type of the other.
#[cfg(any(feature = "cache", feature = "sqlite"))]
pub(crate) trait AppendWire: Copy + 'static {
    type Config: RaftTypeConfig<NodeId = NodeId>;

    /// Wraps an `AppendEntries` request and the place its answer goes back to into the message for
    /// the socket.
    fn request(ack: AckTarget, req: AppendEntriesRequest<Self::Config>) -> RaftRequest;

    /// Wraps the answer channel of a replication stream and the log range of one of its requests.
    fn stream_ack(
        sender: flume::Sender<StreamAnswer<Self::Config>>,
        prev_log_id: Option<LogIdOf<Self::Config>>,
        last_log_id: Option<LogIdOf<Self::Config>>,
    ) -> AckTarget;

    /// Takes the `AppendEntries` answer out of a response payload, `None` for any other payload.
    fn response(payload: RaftStreamResponsePayload) -> Option<AppendAnswer<Self::Config>>;
}

#[cfg(feature = "sqlite")]
#[derive(Clone, Copy)]
pub(crate) struct AppendDb;

#[cfg(feature = "sqlite")]
impl AppendWire for AppendDb {
    type Config = TypeConfigSqlite;

    #[inline]
    fn request(ack: AckTarget, req: AppendEntriesRequest<Self::Config>) -> RaftRequest {
        RaftRequest::AppendDB((ack, req))
    }

    #[inline]
    fn stream_ack(
        sender: flume::Sender<StreamAnswer<Self::Config>>,
        prev_log_id: Option<LogIdOf<Self::Config>>,
        last_log_id: Option<LogIdOf<Self::Config>>,
    ) -> AckTarget {
        AckTarget::AppendDb(StreamAck {
            sender,
            prev_log_id,
            last_log_id,
        })
    }

    #[inline]
    fn response(payload: RaftStreamResponsePayload) -> Option<AppendAnswer<Self::Config>> {
        match payload {
            RaftStreamResponsePayload::AppendDB(res) => Some(res),
            _ => None,
        }
    }
}

#[cfg(feature = "cache")]
#[derive(Clone, Copy)]
pub(crate) struct AppendCache;

#[cfg(feature = "cache")]
impl AppendWire for AppendCache {
    type Config = TypeConfigCache;

    #[inline]
    fn request(ack: AckTarget, req: AppendEntriesRequest<Self::Config>) -> RaftRequest {
        RaftRequest::AppendCache((ack, req))
    }

    #[inline]
    fn stream_ack(
        sender: flume::Sender<StreamAnswer<Self::Config>>,
        prev_log_id: Option<LogIdOf<Self::Config>>,
        last_log_id: Option<LogIdOf<Self::Config>>,
    ) -> AckTarget {
        AckTarget::AppendCache(StreamAck {
            sender,
            prev_log_id,
            last_log_id,
        })
    }

    #[inline]
    fn response(payload: RaftStreamResponsePayload) -> Option<AppendAnswer<Self::Config>> {
        match payload {
            RaftStreamResponsePayload::AppendCache(res) => Some(res),
            _ => None,
        }
    }
}

/// The size a Raft request may reach before the transport warns about it, in bytes.
///
/// openraft bounds a replication request by the number of entries it packs, never by their size, so
/// a request is as big as the entries the leader had queued: a transaction is a single entry of
/// whatever size the application gives it. A request of this size travels the network as one
/// message, the follower collects and decodes it as one message, and everything else that connection
/// carries queues up behind it. Measured on a three-node cluster on one machine: 28 MB replicates
/// fine, 57 MB stalls its connection until the write times out, and the budget is a quarter of that
/// stable value because the limit depends on the machine and the network. Splitting a request is the
/// application's call, so this is the loud marker that one got away too big.
pub(crate) const WARN_REQUEST_BYTES: usize = 8 * 1024 * 1024;

/// Warns when a serialized Raft request is over `WARN_REQUEST_BYTES`. The writer has the bytes in
/// hand already, so this measures the request exactly as it goes onto the socket, at no cost.
#[inline]
fn warn_if_request_is_huge(bytes: usize) {
    if bytes > WARN_REQUEST_BYTES {
        warn!(
            r"
    Raft request is {} bytes, over the {} byte budget: it travels the network as a single
    message, the follower has to collect and decode it as a single message, and everything
    else on this connection queues up behind it. Split the payload in the application.
    This may work, but it can trigger Raft issues on slower networks.
            ",
            bytes, WARN_REQUEST_BYTES,
        );
    }
}

/// The wire's snapshot message builder, so the chunk helper stays generic over both groups.
type SnapshotRequestFn = fn(
    (
        oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        SnapshotWireRequest,
    ),
) -> RaftRequest;

/// The shape of one append answer, for the counters only.
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
            node: node.clone(),
            sender,
            task: Some(task),
        }
    }
}

#[derive(Debug)]
pub(crate) enum RaftRequest {
    #[cfg(feature = "sqlite")]
    AppendDB((AckTarget, AppendEntriesRequest<TypeConfigSqlite>)),
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
    AppendCache((AckTarget, AppendEntriesRequest<TypeConfigCache>)),
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
        // Every request this socket has sent and not answered yet, keyed by the id the server echoes
        // back in its answer. Everything in it is failed on the reconnect path, so it is empty again
        // before a new connection starts.
        let mut in_flight = InFlight::new();
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
                        // request has to be failed here. A pipelined stream queues its whole payload
                        // at once, and the channel is unbounded, so this has to drain fully rather
                        // than take a fixed number of items.
                        while let Ok(req) = rx.try_recv() {
                            let target = match req {
                                #[cfg(feature = "sqlite")]
                                RaftRequest::AppendDB((target, _)) => Some(target),
                                #[cfg(feature = "sqlite")]
                                RaftRequest::VoteDB((ack, _)) => Some(AckTarget::Call(ack)),
                                #[cfg(feature = "sqlite")]
                                RaftRequest::SnapshotDB((ack, _)) => Some(AckTarget::Call(ack)),
                                #[cfg(feature = "sqlite")]
                                RaftRequest::TransferLeaderDB((ack, _)) => {
                                    Some(AckTarget::Call(ack))
                                }
                                #[cfg(feature = "cache")]
                                RaftRequest::AppendCache((target, _)) => Some(target),
                                #[cfg(feature = "cache")]
                                RaftRequest::VoteCache((ack, _)) => Some(AckTarget::Call(ack)),
                                #[cfg(feature = "cache")]
                                RaftRequest::SnapshotCache((ack, _)) => Some(AckTarget::Call(ack)),
                                #[cfg(feature = "cache")]
                                RaftRequest::TransferLeaderCache((ack, _)) => {
                                    Some(AckTarget::Call(ack))
                                }
                                RaftRequest::StreamResponse(_) => None,
                                RaftRequest::ReaderExit => {
                                    continue;
                                }
                                RaftRequest::Shutdown => {
                                    break 'outer;
                                }
                            };

                            if let Some(target) = target {
                                target.fail(Error::Connect(err.to_string()));
                            }
                        }

                        continue;
                    }
                }
            };
            debug_assert!(
                in_flight.is_empty(),
                "raft in flight buffer should always be empty when restoring a connection: {}",
                in_flight.len()
            );

            // The relay loop below is the single point both directions run through, and a pipelined
            // stream hands it a whole payload at once. A capacity of 1 would make the relay wait for
            // the socket writer before it routes the next request, and an answer arriving behind
            // that wait would stall the stream that is waiting for it, so both carry the full
            // pipeline. Their size is bounded by openraft's own replication buffers, not by here.
            let (tx_write, rx_write) = flume::unbounded();
            let (tx_read, rx_read) = flume::unbounded();

            // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
            let (read, write) = socket.split(tokio::io::split);
            // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
            let read = FragmentCollectorRead::new(read);

            // Guarded: `ws_handler` may be aborted before it reaches the
            // aborts below, and these must then cancel and not detach.
            let handle_read =
                AbortOnDrop(task::spawn(Box::pin(Self::stream_reader(read, tx_read))));
            let handle_write =
                AbortOnDrop(task::spawn(Box::pin(Self::stream_writer(write, rx_write))));

            // Frames the previous connection's reader had already queued belong to requests that
            // path failed when it broke the connection. Drop them rather than matching them
            // against this connection's ids.
            // TODO this should not be necessary? We just created this channel, and the socket was closed anyway.
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
                    RaftRequest::AppendDB((target, req)) => {
                        Some((target, RaftStreamRequest::AppendDB((request_id, req))))
                    }
                    #[cfg(feature = "sqlite")]
                    RaftRequest::VoteDB((ack, req)) => Some((
                        AckTarget::Call(ack),
                        RaftStreamRequest::VoteDB((request_id, req)),
                    )),
                    #[cfg(feature = "sqlite")]
                    RaftRequest::SnapshotDB((ack, mut req)) => {
                        req.request_id = request_id;
                        Some((AckTarget::Call(ack), RaftStreamRequest::SnapshotDB(req)))
                    }
                    #[cfg(feature = "sqlite")]
                    RaftRequest::TransferLeaderDB((ack, req)) => Some((
                        AckTarget::Call(ack),
                        RaftStreamRequest::TransferLeaderDB((request_id, req)),
                    )),

                    #[cfg(feature = "cache")]
                    RaftRequest::AppendCache((target, req)) => {
                        Some((target, RaftStreamRequest::AppendCache((request_id, req))))
                    }
                    #[cfg(feature = "cache")]
                    RaftRequest::VoteCache((ack, req)) => Some((
                        AckTarget::Call(ack),
                        RaftStreamRequest::VoteCache((request_id, req)),
                    )),
                    #[cfg(feature = "cache")]
                    RaftRequest::SnapshotCache((ack, mut req)) => {
                        req.request_id = request_id;
                        Some((AckTarget::Call(ack), RaftStreamRequest::SnapshotCache(req)))
                    }
                    #[cfg(feature = "cache")]
                    RaftRequest::TransferLeaderCache((ack, req)) => Some((
                        AckTarget::Call(ack),
                        RaftStreamRequest::TransferLeaderCache((request_id, req)),
                    )),

                    RaftRequest::StreamResponse(resp) => {
                        // The server echoes the id of the request it answered. Delivering in send
                        // order is the reorder buffer's job, and an id the connection never sent
                        // acknowledges a request nobody is waiting for.
                        if !in_flight.answer(resp.request_id, resp.payload) {
                            error!(
                                "client ack for RaftStreamResponse missing: {}",
                                resp.request_id
                            );
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

                if let Some((target, payload)) = stream_req {
                    let bytes = serialize_serde(&payload);
                    warn_if_request_is_huge(bytes.len());

                    if let Err(err) = tx_write.send_async(WritePayload::Payload(bytes)).await {
                        target.fail(Error::Connect(format!(
                            "Error sending Write Request to WebSocket writer: {err}"
                        )));
                        break;
                    }

                    in_flight.push(request_id, target);
                    request_id += 1;
                }
            }

            let _ = tx_write.send_async(WritePayload::Close).await;

            // A response for a request sent on the dead socket can never arrive on the next one:
            // the server answers every request on the connection it received it on.
            in_flight.fail_all("Raft WebSocket stream ended");

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

    /// One `AppendEntries` round trip on the connection: the request onto the socket, then its
    /// answer. `append_entries` is exactly this.
    async fn append_entries_wire<W>(
        &mut self,
        req: AppendEntriesRequest<W::Config>,
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
            .or_else(|| prev_log_id.clone());

        let (ack, rx) = oneshot::channel();
        let payload = self
            .send(W::request(AckTarget::Call(ack), req), rx)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?;

        match W::response(payload) {
            Some(res) => res
                .map(|res| res.into_append_result(prev_log_id, last_log_id))
                .map_err(|err| RPCError::Unreachable(Unreachable::new(&err))),
            None => Err(RPCError::Unreachable(Unreachable::from_string(
                "Raft append answer carries a different payload type",
            ))),
        }
    }

    /// openraft's request stream, pipelined onto the connection.
    ///
    /// Every request openraft produces goes onto the socket the next time the answer stream is
    /// polled, and the answers of the session come back on their own channel in request order, each
    /// carrying the log range of the request it answers. Nothing here is bounded: openraft feeds the
    /// request stream from its replication buffers and rate limits the writes that fill them, and the
    /// socket keeps the order of everything that travels over it.
    fn append_stream<'s, S, W>(&self, input: S) -> BoxFuture<'s, OpenAppendStream<'s, W::Config>>
    where
        S: Stream<Item = AppendEntriesRequest<W::Config>> + OptionalSend + Unpin + 'static,
        W: AppendWire,
    {
        let (tx, rx) = flume::unbounded();
        let state = AppendSession {
            input: Some(input),
            sender: self.sender.clone(),
            answers: SessionAnswers(rx),
            open: Some(tx),
            done: false,
        };

        Box::pin(async move {
            Ok(Box::pin(stream::unfold(state, |mut state| async move {
                if state.done {
                    return None;
                }

                loop {
                    select! {
                        biased;

                        req = AppendSession::next_request(&mut state.input), if state.input.is_some() => {
                            if let Some(req) = req {
                                let prev_log_id = req.prev_log_id.clone();
                                // An append request without entries only asks for an ack, which is
                                // the request's `prev_log_id` - openraft asserts the ack to be inside
                                // the (empty) sent range.
                                let last_log_id = req
                                    .entries
                                    .last()
                                    .map(|entry| entry.log_id())
                                    .or_else(|| prev_log_id.clone());
                                let sender = state
                                    .open
                                    .clone()
                                    .expect("the answers channel is open while the input is");
                                let ack = W::stream_ack(sender, prev_log_id, last_log_id);

                                if let Err(err) = state.sender.send(W::request(ack, req)) {
                                    state.done = true;
                                    return Some((
                                        Err(RPCError::Unreachable(Unreachable::new(&err))),
                                        state,
                                    ));
                                }
                            } else {
                                // openraft has nothing left to send. Dropping the sender half closes
                                // the answers channel with the last answer, and the stream ends with
                                // it; a session that stayed open there would sit in front of
                                // everything that has to follow on the connection.
                                state.input = None;
                                state.open = None;
                            }
                        }

                        answer = state.answers.next() => {
                            match answer {
                                // The answers channel closed: every answer of the session is
                                // delivered and openraft has nothing left to send.
                                Err(_) if state.input.is_none() => return None,
                                Err(err) => {
                                    state.done = true;
                                    return Some((
                                        Err(RPCError::Unreachable(Unreachable::new(&err))),
                                        state,
                                    ));
                                }
                                Ok(answer) => {
                                    let (res, fully_acked) = Self::stream_answer::<W>(answer);
                                    state.done = !fully_acked;
                                    return Some((res, state));
                                }
                            }
                        }
                    }
                }
            })) as AppendStream<'s, W::Config>)
        })
    }

    /// One answer of a replication stream as openraft wants it, and whether it acknowledges the
    /// whole request it answers.
    fn stream_answer<W>(answer: StreamAnswer<W::Config>) -> StreamAnswerOf<W::Config>
    where
        W: AppendWire,
    {
        let StreamAnswer {
            payload,
            prev_log_id,
            last_log_id,
        } = answer;

        let res = match payload {
            Err(err) => Err(RPCError::Unreachable(Unreachable::new(&err))),
            Ok(payload) => match W::response(payload) {
                Some(Ok(res)) => Ok(res.into_append_result(prev_log_id, last_log_id.clone())),
                Some(Err(err)) => Err(RPCError::Unreachable(Unreachable::new(&err))),
                None => Err(RPCError::Unreachable(Unreachable::from_string(
                    "Raft append answer carries a different payload type",
                ))),
            },
        };

        // Only an acknowledgement of the whole request lets the stream go on. Everything behind the
        // answer that is not one is dropped: the leader rebuilds this replication from the point the
        // answer names.
        let fully_acked = matches!(&res, Ok(Ok(acked)) if *acked == last_log_id);
        (res, fully_acked)
    }
}

/// The answers of one replication session, in request order.
struct SessionAnswers<C: RaftTypeConfig>(flume::Receiver<StreamAnswer<C>>);

impl<C: RaftTypeConfig> SessionAnswers<C> {
    /// The next answer of the session. The borrow is of this field alone, which is what lets the
    /// answer stream poll for the next request and for the next answer in the same `select!`.
    async fn next(&mut self) -> Result<StreamAnswer<C>, flume::RecvError> {
        self.0.recv_async().await
    }
}

/// The state of one replication session's answer stream - see
/// `NetworkConnectionStreaming::append_stream`.
///
/// Only an acknowledgement of the whole request lets the stream go on. Everything behind the answer
/// that is not one is dropped: the leader rebuilds this replication from the point that answer
/// names.
struct AppendSession<S, C>
where
    C: RaftTypeConfig,
{
    /// openraft's request stream, `None` once it has answered `None`.
    input: Option<S>,
    /// The connection's request channel.
    sender: flume::Sender<RaftRequest>,
    /// The answers of this session, in request order.
    answers: SessionAnswers<C>,
    /// The sender half of that channel, kept only to drop it once the input is exhausted.
    open: Option<flume::Sender<StreamAnswer<C>>>,
    /// Set by the answer that ends the session.
    done: bool,
}

impl<S, C> AppendSession<S, C>
where
    C: RaftTypeConfig,
    S: Stream<Item = AppendEntriesRequest<C>> + Unpin,
{
    /// The next request openraft has produced, `None` once its stream is exhausted. The borrow is of
    /// the input field alone, which is what lets the answer stream poll for the next request and for
    /// the next answer in the same `select!`.
    async fn next_request(input: &mut Option<S>) -> Option<AppendEntriesRequest<C>> {
        match input.as_mut() {
            None => None,
            Some(input) => input.next().await,
        }
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
        self.append_entries_wire::<AppendDb>(req).await
    }

    /// Pipelined instead of openraft's sequential default - see `append_stream`.
    fn stream_append<S>(
        &mut self,
        input: S,
        _option: RPCOption,
    ) -> BoxFuture<'_, OpenAppendStream<'_, TypeConfigSqlite>>
    where
        S: Stream<Item = AppendEntriesRequest<TypeConfigSqlite>> + OptionalSend + Unpin + 'static,
    {
        self.append_stream::<S, AppendDb>(input)
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
        self.append_entries_wire::<AppendCache>(rpc).await
    }

    /// Pipelined instead of openraft's sequential default - see `append_stream`.
    fn stream_append<S>(
        &mut self,
        input: S,
        _option: RPCOption,
    ) -> BoxFuture<'_, OpenAppendStream<'_, TypeConfigCache>>
    where
        S: Stream<Item = AppendEntriesRequest<TypeConfigCache>> + OptionalSend + Unpin + 'static,
    {
        self.append_stream::<S, AppendCache>(input)
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

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use openraft::type_config::alias::EntryOf;

    use crate::store::state_machine::sqlite::state_machine::{Query, QueryWrite};

    fn log_id_of(index: u64) -> LogIdOf<TypeConfigSqlite> {
        LogIdOf::<TypeConfigSqlite>::from(LogId {
            leader_id: LeaderId {
                term: 1u64,
                node_id: 1u64,
            },
            index,
        })
    }

    /// One append request carrying a single entry, which is what a replication stream produces for
    /// a write.
    fn append_with_entry(index: u64) -> AppendEntriesRequest<TypeConfigSqlite> {
        AppendEntriesRequest::<TypeConfigSqlite> {
            vote: VoteOf::<TypeConfigSqlite>::from(Vote::new_committed(1, 1)),
            prev_log_id: Some(log_id_of(index)),
            leader_commit: None,
            entries: vec![EntryOf::<TypeConfigSqlite>::new_normal(
                log_id_of(index + 1),
                QueryWrite::Execute(Query {
                    sql: "INSERT INTO _bench (id, ts, name) VALUES ($1, $2, $3)".into(),
                    params: Vec::new(),
                }),
            )],
        }
    }
    use super::*;
    use crate::store::state_machine::sqlite::TypeConfigSqlite;
    use futures_util::StreamExt;
    use futures_util::stream;
    use openraft::Vote;
    use openraft::entry::RaftEntry;
    use openraft::impls::leader_id_adv::LeaderId;
    use openraft::log_id::LogId;
    use openraft::type_config::alias::LogIdOf;
    use openraft::type_config::alias::VoteOf;

    /// A replication stream is pipelined: every request openraft produces goes onto the socket
    /// while the answers of the ones before it are still outstanding. A stream that waited for an
    /// answer before it sent the next request would run at one round trip per append, which is what
    /// the throughput of a whole cluster is decided by.
    #[tokio::test]
    async fn append_stream_sends_every_request_before_it_answers_any() {
        let (tx, rx) = flume::unbounded();
        let conn = NetworkConnectionStreaming {
            node: Node::from("2 127.0.0.1:5100 127.0.0.1:5200"),
            sender: tx,
            task: None,
        };
        let requests: Vec<_> = (1..=3u64).map(append_with_entry).collect();
        // The progress an answer reports is the last entry of its request, which is what the
        // transport hands back to openraft as the acknowledged range.
        let expected: Vec<_> = requests
            .iter()
            .map(|req| {
                Ok(Ok(Some(
                    req.entries.last().expect("a fixture entry").log_id(),
                )))
            })
            .collect();

        let answers = tokio::spawn(async move {
            let mut answers = Box::pin(
                conn.append_stream::<_, AppendDb>(stream::iter(requests))
                    .await
                    .expect("the append stream opened"),
            );
            let mut got = Vec::new();
            while let Some(res) = answers.next().await {
                got.push(res);
            }
            got
        });

        // The socket side of the test: take the requests, keep the answers back until all of them
        // are on the socket.
        let mut acks = Vec::new();
        for _ in 0..3 {
            let req = time::timeout(Duration::from_millis(500), rx.recv_async())
                .await
                .expect("a request never reached the socket")
                .expect("the request channel is closed");
            let RaftRequest::AppendDB((AckTarget::AppendDb(ack), _)) = req else {
                panic!("unexpected request on the socket: {req:?}")
            };
            acks.push(ack);
        }
        assert!(
            rx.try_recv().is_err(),
            "the stream sent more requests than its input had"
        );

        for ack in acks {
            assert!(
                ack.answer(Ok(RaftStreamResponsePayload::AppendDB(Ok(
                    AppendEntriesResponse::Success,
                )))),
                "the answer was not taken"
            );
        }

        let got = time::timeout(Duration::from_millis(500), answers)
            .await
            .expect("the stream stalled")
            .expect("the answering task panicked");
        assert_eq!(got, expected);
    }

    /// An answer that is not the whole request ends the session: everything behind it is dropped
    /// and the leader rebuilds the replication from the point that answer names.
    #[tokio::test]
    async fn append_stream_stops_after_an_answer_that_is_not_the_whole_request() {
        let (tx, rx) = flume::unbounded();
        let conn = NetworkConnectionStreaming {
            node: Node::from("2 127.0.0.1:5100 127.0.0.1:5200"),
            sender: tx,
            task: None,
        };
        let requests: Vec<_> = (1..=3u64).map(append_with_entry).collect();

        // The stream only sends when it is polled, so it runs beside the test.
        let answers = tokio::spawn(async move {
            let mut answers = Box::pin(
                conn.append_stream::<_, AppendDb>(stream::iter(requests))
                    .await
                    .expect("the append stream opened"),
            );
            let mut got = Vec::new();
            while let Some(res) = answers.next().await {
                got.push(res);
            }
            got
        });

        let first = time::timeout(Duration::from_millis(500), rx.recv_async())
            .await
            .expect("a request never reached the socket")
            .expect("the request channel is closed");
        let RaftRequest::AppendDB((AckTarget::AppendDb(ack), req)) = first else {
            panic!("unexpected request on the socket: {first:?}")
        };
        // The follower took everything before the request's entries, which is inside its range but
        // not its end.
        let partial = req.prev_log_id.expect("a fixture prev log id");
        assert!(
            ack.answer(Ok(RaftStreamResponsePayload::AppendDB(Ok(
                AppendEntriesResponse::PartialSuccess(Some(partial)),
            )))),
            "the answer was not taken"
        );

        let got = time::timeout(Duration::from_millis(500), answers)
            .await
            .expect("the stream stalled")
            .expect("the answering task panicked");
        assert_eq!(
            got,
            vec![Ok(Ok(Some(partial)))],
            "the stream went on after an answer that was not the whole request"
        );
    }

    /// The reorder buffer of a connection delivers answers in request order, whatever order the
    /// server answered them in: an answer naming a later request waits until the request the
    /// connection expects next is answered, and everything queued behind it goes with it.
    #[tokio::test]
    async fn in_flight_delivers_answers_in_request_order() {
        let mut in_flight = InFlight::new();
        let (ack_1, mut rx_1) = oneshot::channel();
        let (ack_2, mut rx_2) = oneshot::channel();
        let (ack_3, mut rx_3) = oneshot::channel();
        in_flight.push(10, AckTarget::Call(ack_1));
        in_flight.push(11, AckTarget::Call(ack_2));
        in_flight.push(12, AckTarget::Call(ack_3));

        let vote = |term| {
            RaftStreamResponsePayload::VoteDB(Ok(VoteResponse::new(
                Vote::new_committed(term, 1),
                None,
                true,
            )))
        };

        // The server answered the two later requests first.
        assert!(in_flight.answer(11, vote(2)));
        assert!(
            rx_1.try_recv().is_err(),
            "the expected request was answered"
        );
        assert!(in_flight.answer(12, vote(3)));
        assert!(
            rx_2.try_recv().is_err(),
            "an answer was delivered before the one it waits behind"
        );

        assert!(in_flight.answer(10, vote(1)));
        assert!(rx_1.try_recv().is_ok());
        assert!(rx_2.try_recv().is_ok());
        assert!(rx_3.try_recv().is_ok());

        assert!(
            !in_flight.answer(99, vote(4)),
            "an answer to a request that was never sent was delivered"
        );
    }
}
