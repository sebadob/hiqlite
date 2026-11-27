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
use futures_util::stream::{self, poll_fn};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use openraft::errors::RPCError;
use openraft::errors::Timeout;
use openraft::errors::Unreachable;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::task::Context;
use std::task::Poll;
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
use openraft::type_config::alias::{LogIdOf, SnapshotOf, VoteOf};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::{OptionalSend, RaftTypeConfig};

/// Instrumentation of the pipelined append transport, compiled in only with the `stream-stats`
/// feature. Enabled at run time with `HQL_RAFT_STREAM_STATS=1`, which makes every poll of a session
/// record what it did, so a stall is measured instead of guessed at.
#[cfg(all(feature = "stream-stats", any(feature = "cache", feature = "sqlite")))]
pub(crate) static STREAM_STATS: std::sync::LazyLock<Option<StreamStats>> =
    std::sync::LazyLock::new(|| {
        if std::env::var("HQL_RAFT_STREAM_STATS").ok().as_deref() == Some("1") {
            Some(StreamStats::default())
        } else {
            None
        }
    });

/// One counter per decision point of a pipelined session, and per shape of an answer.
#[cfg(all(feature = "stream-stats", any(feature = "cache", feature = "sqlite")))]
#[derive(Debug, Default)]
pub(crate) struct StreamStats {
    /// Requests the generator took out of openraft's stream.
    generated: AtomicU64,
    /// Times the generator was polled and had nothing to hand over.
    generator_pending: AtomicU64,
    /// Times the fill loop stopped because the window was full.
    window_full: AtomicU64,
    /// Times the fill loop stopped because the channel had nothing.
    channel_empty: AtomicU64,
    /// Requests handed to the connection.
    sent: AtomicU64,
    /// Times the front of the window had no answer yet.
    front_pending: AtomicU64,
    /// Answers yielded to openraft.
    answered: AtomicU64,
    /// Answers that acknowledged the whole request.
    ack_full: AtomicU64,
    /// Answers that acknowledged only part of a request.
    ack_partial: AtomicU64,
    /// Answers that reported a log conflict.
    ack_conflict: AtomicU64,
    /// Answers naming a request other than the one at the front of the window.
    out_of_order: AtomicU64,
    /// Entry-less requests the session did not send, because a request already on the socket
    /// carries the same commit.
    dropped_empty: AtomicU64,
}

#[cfg(all(feature = "stream-stats", any(feature = "cache", feature = "sqlite")))]
impl StreamStats {
    fn add(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// The shape of one append answer. A partial acknowledgement is the event that invalidates
    /// every request pipelined behind the one it answers, so it has to be countable.
    fn record_ack<C>(&self, res: &AppendEntriesResponse<C>)
    where
        C: RaftTypeConfig,
    {
        match res {
            AppendEntriesResponse::Success => Self::add(&self.ack_full),
            AppendEntriesResponse::PartialSuccess(_) => Self::add(&self.ack_partial),
            AppendEntriesResponse::Conflict => Self::add(&self.ack_conflict),
            AppendEntriesResponse::HigherVote(_) => {}
        }
    }

    /// The counters of a run, for a benchmark to print next to its throughput.
    pub(crate) fn report() -> String {
        let Some(stats) = STREAM_STATS.as_ref() else {
            return "Raft stream stats are off (HQL_RAFT_STREAM_STATS=1 enables them)".to_string();
        };
        let g = stats.generated.load(Ordering::Relaxed);
        let de = stats.dropped_empty.load(Ordering::Relaxed);
        let gp = stats.generator_pending.load(Ordering::Relaxed);
        let wf = stats.window_full.load(Ordering::Relaxed);
        let ce = stats.channel_empty.load(Ordering::Relaxed);
        let sent = stats.sent.load(Ordering::Relaxed);
        let fp = stats.front_pending.load(Ordering::Relaxed);
        let a = stats.answered.load(Ordering::Relaxed);
        let full = stats.ack_full.load(Ordering::Relaxed);
        let partial = stats.ack_partial.load(Ordering::Relaxed);
        let conflict = stats.ack_conflict.load(Ordering::Relaxed);
        let ooo = stats.out_of_order.load(Ordering::Relaxed);
        format!(
            "Raft stream stats: generated {g}, generator pending polls {gp}, window full {wf}, \
channel empty {ce}, sent {sent}, dropped empty {de}, front pending polls {fp}, answered {a}, \
full acks {full}, partial acks {partial}, conflicts {conflict}, out of order {ooo}"
        )
    }
}

#[cfg(any(feature = "cache", feature = "sqlite"))]
const SNAPSHOT_CHUNK_SIZE_FALLBACK: usize = 3 * 1024 * 1024;

/// `AppendEntries` RPCs a single replication stream keeps on the socket at the same time.
///
/// 1 is one request, its answer, then the next. Above 1 the leader sends the next request while the
/// previous one is still being answered, so the round trip stops being paid per batch. The follower
/// applies appends one at a time, in the order they arrive, so the depth is purely the leader's
/// window: it is how many requests are on the socket and how many answers are on their way back.
///
/// A window above 1 only pays on a link whose round trip is the cost. A follower that acknowledges
/// part of a request invalidates every request the leader already pipelined behind it, and the
/// leader re-replicates them, which on a link with no distance costs several times what the
/// overlap can save. Raise it on a link with distance and measure it there.
///
/// The depth used when `HQL_RAFT_STREAM_DEPTH` is unset, unparsable, or out of range.
#[cfg(any(feature = "cache", feature = "sqlite"))]
const DEFAULT_RAFT_STREAM_DEPTH: usize = 8;

/// The highest depth the transport accepts, which bounds how many appends a single connection
/// keeps unacknowledged at once.
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

/// The way back to a caller that is waiting for one request's answer, handed over with the request.
type Ack = oneshot::Sender<Result<RaftStreamResponsePayload, Error>>;

/// The items a pipelined answer stream yields: an answer, or the end of the stream.
type AppendAnswers<C> = Option<Result<AppendResult<C>, RPCError<C>>>;

/// One append inside a pipeline window: the log id range its answer has to be reported against, and
/// the receiver that answer arrives on.
struct PendingAppend<C>
where
    C: RaftTypeConfig<NodeId = NodeId>,
{
    prev_log_id: Option<LogIdOf<C>>,
    last_log_id: Option<LogIdOf<C>>,
    rx: oneshot::Receiver<Result<RaftStreamResponsePayload, Error>>,
}

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

/// The state of one pipelined `stream_append` session: the requests openraft has handed over, the
/// ones that are on the socket, and their answers in the order the requests went out.
///
/// The queue is the whole reorder. Its front is always the request openraft is waiting for, and
/// the receiver it carries is the answer slot of exactly that request, so a session needs no
/// registry of what it has sent.
struct AppendPipeline<S, W>
where
    W: AppendWire,
{
    /// openraft's request stream. A window of one takes requests straight out of it, one per poll,
    /// which is the shape openraft's own sequential transport has: no queue in between, nothing
    /// handed over a channel. A wider window moves the stream into the generator below, because
    /// generation has to run beside the answers, and that needs a place to put what it generates.
    input: Option<Pin<Box<S>>>,
    /// The generation of a window wider than one, as a stream so it can be polled on every poll of
    /// the session. It owns the request stream, which is what makes generating and answering safe
    /// to poll in the same poll: a pending generation is never dropped, and a finished one is never
    /// polled again.
    generator: Option<Pin<Box<dyn Stream<Item = ()> + Send>>>,
    /// The requests generation has produced and the session has not sent yet. Its capacity is the
    /// window, so a leader that generates faster than the follower answers stops generating rather
    /// than queueing without end.
    rx_req: Option<flume::Receiver<AppendEntriesRequest<W::Config>>>,
    /// The requests taken off the channel on this poll. Living in the state means a poll allocates
    /// nothing, which matters because a poll is more frequent than a request.
    drained: Vec<AppendEntriesRequest<W::Config>>,
    /// Set when the input has handed over everything it will, which the closed channel says.
    input_exhausted: bool,
    /// Set when the generator has finished, because a finished stream must never be polled again.
    generator_done: bool,
    sender: flume::Sender<RaftRequest>,
    ids: (NodeId, NodeId),
    /// How long the front of the window may wait for its answer.
    ttl: Duration,
    /// The deadline of the current front, reset when the next request becomes the front.
    timer: Pin<Box<time::Sleep>>,
    depth: usize,
    /// The requests on the socket in send order.
    pending: VecDeque<PendingAppend<W::Config>>,
    /// Set once the session is over, by a failed answer or by a drained session, so the stream ends
    /// instead of being polled on.
    done: bool,
}

/// The shape of one append answer, for the counters only.
#[cfg(all(feature = "stream-stats", any(feature = "cache", feature = "sqlite")))]
fn record_ack_of_payload(payload: &RaftStreamResponsePayload) {
    if let Some(stats) = STREAM_STATS.as_ref() {
        match payload {
            RaftStreamResponsePayload::AppendDB(Ok(res)) => stats.record_ack(res),
            #[cfg(feature = "cache")]
            RaftStreamResponsePayload::AppendCache(Ok(res)) => stats.record_ack(res),
            _ => {}
        }
    }
}

/// One counter of the pipelined transport. With the `stream-stats` feature compiled out, this is
/// nothing at all, so a normal build pays no load, no branch and no atomic for it.
#[cfg(all(feature = "stream-stats", any(feature = "cache", feature = "sqlite")))]
macro_rules! stream_stats {
    // A statement: a call on a function of this module.
    ($name:ident($($arg:expr),* $(,)?)) => {
        $name($($arg),*);
    };
    // A counter: a closure picking one field out of `StreamStats`.
    ($counter:expr) => {
        if let Some(stats) = STREAM_STATS.as_ref() {
            let counter: for<'a> fn(&'a StreamStats) -> &'a AtomicU64 = $counter;
            StreamStats::add(counter(stats));
        }
    };
}

#[cfg(not(all(feature = "stream-stats", any(feature = "cache", feature = "sqlite"))))]
macro_rules! stream_stats {
    ($($t:tt)*) => {
        const {}
    };
}

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
            stream_depth: *RAFT_STREAM_DEPTH,
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
            stream_depth: *RAFT_STREAM_DEPTH,
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
        // Every request this socket has sent and not answered yet, in send order. The server answers
        // a connection in the order it received its requests, so the front of this queue is always
        // the owner of the next answer, and the pipelined appends are simply its longest part.
        // Everything in it is failed on the reconnect path, so it is empty again before a new
        // connection starts.
        let mut in_flight: VecDeque<(usize, Ack)> = VecDeque::new();
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
                "raft in flight buffer should always be empty when restoring a connection: {}",
                in_flight.len()
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
            let handle_read =
                AbortOnDrop(task::spawn(Box::pin(Self::stream_reader(read, tx_read))));
            let handle_write =
                AbortOnDrop(task::spawn(Box::pin(Self::stream_writer(write, rx_write))));

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
                        // The answer order is the request order, so the front of the queue is the
                        // ack this response belongs to. An answer that names another request means
                        // that order is broken, and delivering it would hand one caller the answer
                        // of another: fail it and end the connection instead.
                        match in_flight.pop_front() {
                            None => {
                                error!(
                                    "client ack for RaftStreamResponse missing: {}",
                                    resp.request_id
                                );
                            }
                            Some((id, ack)) if id == resp.request_id => {
                                if ack.send(Ok(resp.payload)).is_err() {
                                    error!("sending back stream response from raft server");
                                }
                            }
                            Some((id, ack)) => {
                                stream_stats!(|s: &StreamStats| &s.out_of_order);
                                let _ = ack.send(Err(Error::Connect(format!(
                                    "Raft node {} answered out of request order: {} vs {}",
                                    node.id, resp.request_id, id,
                                ))));
                                error!(
                                    "Raft node {} answered out of request order: {} vs {}",
                                    node.id, resp.request_id, id
                                );
                                break;
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

                    in_flight.push_back((request_id, ack));
                    request_id += 1;
                }
            }

            let _ = tx_write.send_async(WritePayload::Close).await;

            // A response for a request sent on the dead socket can never arrive on the next one:
            // the server answers every request on the connection it received it on.
            for (_, ack) in in_flight.drain(..) {
                let _ = ack.send(Err(Error::Connect("Raft WebSocket stream ended".into())));
            }

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
    /// How many appends a `stream_append` session of this connection keeps in flight. Read from
    /// `RAFT_STREAM_DEPTH` when the factory builds a connection.
    stream_depth: usize,
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
    /// Up to `RAFT_STREAM_DEPTH` requests are on the socket at the same time and their answers are
    /// yielded in request order, which is the reorder openraft asks of a pipelined transport.
    ///
    /// Termination follows openraft's own sequential default. A full acknowledgement is what lets the
    /// stream go on, and openraft stops at the first answer that is not one - a conflict, a higher
    /// vote, an RPC error - so nothing queued behind it is ever asked for. When the input ends, the
    /// window drains and the answer stream ends.
    fn append_stream<'s, W, S>(
        &self,
        input: S,
        option: &RPCOption,
    ) -> BoxFuture<'s, OpenAppendStream<'s, W::Config>>
    where
        W: AppendWire,
        S: Stream<Item = AppendEntriesRequest<W::Config>> + OptionalSend + Unpin + 'static,
    {
        let ttl = Self::response_ttl(option);
        let depth = self.stream_depth;

        // A window of one has nothing to overlap: one request goes out, its answer comes back, the
        // next goes out. Taking requests straight out of openraft's stream is then the whole path,
        // with no queue in between, which is what openraft's own sequential transport costs.
        //
        // A wider window needs generation to run beside the answers. openraft produces a request by
        // reading its log, so its stream is ready only when there is something to send. A session
        // that waited for the next request would hold back the answers of everything it already
        // sent, and a session that polled it once per answer would never fill its window, because
        // one poll yields at most one request. Generation therefore moves into a stream of its own
        // that owns the request stream and feeds a channel bounded by the window. Owning the stream
        // is what makes it safe to poll in the same poll as the answers: a pending generation is
        // never dropped, and a finished one is never polled again.
        let (input, generator, rx_req) = if depth == 1 {
            (Some(Box::pin(input)), None, None)
        } else {
            let (tx_req, rx_req) = flume::bounded(depth);
            // The sender is moved into the loop below, so the channel is closed by the `drop` at its
            // end and by nothing else: a session dropped mid-stream closes it the same way.
            let generator: Pin<Box<dyn Stream<Item = ()> + Send>> = Box::pin(stream::unfold(
                (input, tx_req),
                move |(mut input, tx_req)| async move {
                    while let Some(req) = input.next().await {
                        if tx_req.send_async(req).await.is_err() {
                            break;
                        }
                    }
                    // Closing the channel is what tells the session that nothing more is coming.
                    drop(tx_req);
                    None
                },
            ));
            (None, Some(generator), Some(rx_req))
        };

        let mut pipeline = AppendPipeline {
            input,
            generator,
            rx_req,
            drained: Vec::with_capacity(depth),
            input_exhausted: false,
            generator_done: false,
            sender: self.sender.clone(),
            ids: (self.local_id, self.node.id),
            ttl,
            timer: Box::pin(time::sleep(ttl)),
            depth,
            pending: VecDeque::new(),
            done: false,
        };

        Box::pin(async move {
            let answers = poll_fn(move |cx| Self::poll_append::<_, W>(&mut pipeline, cx));
            Ok(Box::pin(answers) as BoxStream<'s, _>)
        })
    }

    /// One poll of a pipelined session: drive generation, send what it produced, and hand out the
    /// answer at the front of the window. Nothing here waits.
    ///
    /// Each request goes out with the receiver of its own answer and that receiver is the pairing, so
    /// a request is never registered anywhere else. The queue is filled in send order and drained from
    /// the front, which is the request order openraft wants, and the server answers a connection in
    /// wire order, so the front receiver always belongs to the front request.
    fn poll_append<S, W>(
        st: &mut AppendPipeline<S, W>,
        cx: &mut Context<'_>,
    ) -> Poll<AppendAnswers<W::Config>>
    where
        W: AppendWire,
        S: Stream<Item = AppendEntriesRequest<W::Config>> + Unpin,
    {
        if st.done {
            return Poll::Ready(None);
        }

        // Generation first, so a request it produced on this tick can go out on this tick. Its end
        // is not the end of the session: the window still has to drain, so the fill below drains to
        // `Disconnected` and only then closes the session.
        if !st.generator_done
            && let Some(generator) = st.generator.as_mut()
            && generator.as_mut().poll_next(cx).is_ready()
        {
            st.generator_done = true;
        }
        if st.generator.is_some() && !st.generator_done {
            stream_stats!(|s: &StreamStats| &s.generator_pending);
        }

        // Take everything that fits the window, in one go. Draining the whole channel per poll is
        // what lets the coalescing below see a run of requests together, which is how they arrive
        // when the leader is busy.
        let mut drained = std::mem::take(&mut st.drained);
        drained.clear();
        if let Some(input) = st.input.as_mut() {
            // The window of one: one request per poll, taken straight from openraft's stream.
            if st.pending.is_empty() && !st.input_exhausted {
                match input.as_mut().poll_next(cx) {
                    Poll::Ready(Some(req)) => {
                        stream_stats!(|s: &StreamStats| &s.generated);
                        drained.push(req);
                    }
                    Poll::Ready(None) => st.input_exhausted = true,
                    Poll::Pending => {}
                }
            }
        } else if let Some(rx_req) = st.rx_req.as_ref() {
            while !st.input_exhausted && st.pending.len() + drained.len() < st.depth {
                match rx_req.try_recv() {
                    Ok(req) => {
                        stream_stats!(|s: &StreamStats| &s.generated);
                        drained.push(req);
                    }
                    Err(flume::TryRecvError::Empty) => {
                        stream_stats!(|s: &StreamStats| &s.channel_empty);
                        break;
                    }
                    Err(flume::TryRecvError::Disconnected) => st.input_exhausted = true,
                }
            }
        }

        // A request with no entries carries only the leader's commit. Generation reads the commit
        // when it builds a request, so any request built after it carries a commit at least as new,
        // and a request with entries in this batch takes that commit to the follower for free. An
        // entry-less request with such a request behind it is dropped. The last entry-less request
        // of a run always goes out: nothing after it promises its commit, and a follower applies log
        // entries only up to the commit it has been told about.
        let last_with_entries = drained.iter().rposition(|req| !req.entries.is_empty());
        for (index, req) in drained.drain(..).enumerate() {
            if req.entries.is_empty() && last_with_entries.is_some_and(|last| last > index) {
                stream_stats!(|s: &StreamStats| &s.dropped_empty);
                continue;
            }

            let prev_log_id = req.prev_log_id.clone();
            // An append request without entries only asks for an ack, which is the request's
            // `prev_log_id` - openraft asserts the ack to be inside the (empty) sent range.
            let last_log_id = req
                .entries
                .last()
                .map(|entry| entry.log_id())
                .or(prev_log_id.clone());

            // The request channel is bounded by the window, so this send is what keeps the window
            // full: it never waits for an answer, and only the writer task can block.
            let (ack, rx) = oneshot::channel();
            if st.sender.send(W::request(ack, req)).is_err() {
                st.done = true;
                return Poll::Ready(Some(Err(RPCError::Unreachable(Unreachable::from_string(
                    "Raft stream sender closed",
                )))));
            }
            st.pending.push_back(PendingAppend {
                prev_log_id,
                last_log_id,
                rx,
            });
            stream_stats!(|s: &StreamStats| &s.sent);
        }
        st.drained = drained;

        if !st.input_exhausted && st.pending.len() >= st.depth {
            stream_stats!(|s: &StreamStats| &s.window_full);
        }

        let front_ready = {
            let Some(front) = st.pending.front_mut() else {
                // An empty window is the end of the session only once nothing more is coming. A
                // leader with an idle log has a generator that is ready for nothing, and the session
                // simply waits for its next request.
                if st.input_exhausted {
                    st.done = true;
                    return Poll::Ready(None);
                }
                return Poll::Pending;
            };
            Pin::new(&mut front.rx).poll(cx)
        };

        match front_ready {
            Poll::Pending => {
                stream_stats!(|s: &StreamStats| &s.front_pending);
                // Only the front of the window can say anything about the connection: it is the
                // oldest request on the socket, and an answer that never arrives there means nothing
                // queued behind it can be trusted either, so the session ends with it.
                if st.timer.as_mut().poll(cx).is_ready() {
                    st.done = true;
                    return Poll::Ready(Some(Err(RPCError::Timeout(Timeout {
                        action: RPCTypes::AppendEntries,
                        id: st.ids.0,
                        target: st.ids.1,
                        timeout: st.ttl,
                    }))));
                }
                Poll::Pending
            }
            // The ack is dropped by `ws_handler` when the connection ends, which is the same
            // "we do not know what happened to this request" case a connect failure is.
            Poll::Ready(Err(_)) => {
                st.done = true;
                Poll::Ready(Some(Err(RPCError::Unreachable(Unreachable::from_string(
                    "Raft stream answer dropped",
                )))))
            }
            Poll::Ready(Ok(Err(err))) => {
                st.done = true;
                Poll::Ready(Some(Err(RPCError::Unreachable(Unreachable::new(&err)))))
            }
            Poll::Ready(Ok(Ok(payload))) => {
                stream_stats!(|s: &StreamStats| &s.answered);
                stream_stats!(record_ack_of_payload(&payload));
                let Some(front) = st.pending.pop_front() else {
                    st.done = true;
                    return Poll::Ready(None);
                };

                let answer = match W::response(payload) {
                    None => Err(RPCError::Unreachable(Unreachable::from_string(
                        "Raft stream answer carries a different payload type",
                    ))),
                    Some(res) => res
                        .map(move |res| {
                            res.into_append_result(front.prev_log_id, front.last_log_id)
                        })
                        .map_err(|err| RPCError::Unreachable(Unreachable::new(&err))),
                };
                if answer.is_err() {
                    st.done = true;
                }

                // The request that becomes the front starts its own deadline here.
                st.timer
                    .as_mut()
                    .reset(tokio::time::Instant::now() + st.ttl);

                Poll::Ready(Some(answer))
            }
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
    /// a write. An entry-less request is a commit carrier, and the session drops those while the
    /// window is busy, so a fixture that wants its requests on the socket has to carry entries.
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
    use openraft::impls::leader_id_adv::LeaderId;
    use openraft::log_id::LogId;
    use openraft::type_config::alias::LogIdOf;
    use openraft::type_config::alias::VoteOf;

    /// A leader with an idle log has a request stream that is ready for nothing, and the answers of
    /// the requests that are already on the socket must be delivered through that. A session that
    /// waits for the next request holds its own acknowledgements back, and with them the commit
    /// progress of the leader it runs on.
    #[tokio::test]
    async fn append_stream_answers_while_the_request_stream_has_nothing_to_send() {
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
            stream_depth: 4,
        };

        let requests = vec![append_with_entry(1)];

        let mut answers = Box::pin(
            conn.append_stream::<AppendDb, _>(
                // One request, then a stream that is ready for nothing and never ends.
                Box::pin(stream::unfold(
                    Some(requests.into_iter()),
                    |requests| async move {
                        match requests {
                            Some(mut requests) => Some((requests.next()?, None)),
                            None => std::future::pending().await,
                        }
                    },
                )),
                &RPCOption::new(Duration::from_millis(500)),
            )
            .await
            .unwrap(),
        );

        let answering = tokio::spawn(async move {
            match time::timeout(Duration::from_millis(500), rx.recv_async()).await {
                Ok(Ok(RaftRequest::AppendDB((ack, _)))) => ack
                    .send(Ok(RaftStreamResponsePayload::AppendDB(Ok(
                        AppendEntriesResponse::Success,
                    ))))
                    .expect("the answer was not taken"),
                other => panic!("unexpected request on the socket: {other:?}"),
            }
        });

        let answer = time::timeout(Duration::from_millis(500), answers.next())
            .await
            .expect("the stream stalled behind a request stream with nothing to send")
            .expect("the stream ended too early")
            .expect("the stream yielded an RPC error");
        assert!(answer.is_ok());
        answering.await.expect("the answering task panicked");
    }

    /// A pipelined session must end after its input is exhausted and its window has drained, and it
    /// must not touch the input stream again once that stream has answered `None`.
    #[tokio::test]
    async fn append_stream_ends_after_the_input_and_the_window_are_drained() {
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
            stream_depth: 4,
        };

        // Fewer requests than the window holds, so the fill runs into the end of the input.
        let requests: Vec<_> = (1..=2u64).map(append_with_entry).collect();

        let mut answers = Box::pin(
            conn.append_stream::<AppendDb, _>(
                // openraft builds its request stream with `unfold`, which panics on a poll after it
                // ended, so the test input has to be one too.
                Box::pin(stream::unfold(
                    Some(requests.into_iter()),
                    |requests| async move {
                        let mut requests = requests?;
                        let req = requests.next()?;
                        Some((req, Some(requests)))
                    },
                )),
                &RPCOption::new(Duration::from_millis(500)),
            )
            .await
            .unwrap(),
        );

        // The pipeline only sends when it is polled, so the answering runs beside it.
        let answering = tokio::spawn(async move {
            let mut acks = Vec::new();
            for _ in 0..2 {
                match time::timeout(Duration::from_millis(500), rx.recv_async()).await {
                    Ok(Ok(RaftRequest::AppendDB((ack, _)))) => acks.push(ack),
                    other => panic!("unexpected request on the socket: {other:?}"),
                }
            }
            for ack in acks {
                ack.send(Ok(RaftStreamResponsePayload::AppendDB(Ok(
                    AppendEntriesResponse::Success,
                ))))
                .expect("the answer was not taken");
            }
        });

        for _ in 0..2 {
            let answer = time::timeout(Duration::from_millis(500), answers.next())
                .await
                .expect("the stream stalled")
                .expect("the stream ended too early")
                .expect("the stream yielded an RPC error");
            assert!(answer.is_ok());
        }

        assert!(
            time::timeout(Duration::from_millis(500), answers.next())
                .await
                .expect("the stream stalled")
                .is_none(),
            "the stream did not end after its input and its window were drained"
        );
        answering.await.expect("the answering task panicked");
    }

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
            stream_depth: 4,
        };

        let requests: Vec<_> = (1..=3u64).map(append_with_entry).collect();
        // The progress an answer reports is the last entry of its request, which is what the
        // transport hands back to openraft as the acknowledged range.
        let expected: Vec<_> = requests
            .iter()
            .map(|req| Some(req.entries.last().expect("a fixture entry").log_id()))
            .collect();

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
