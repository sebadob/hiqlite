use rusqlite::Connection;
use std::any::Any;
use std::borrow::Cow;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(feature = "external-state-machine")]
use tokio::sync::oneshot;
use tracing::{trace, warn};

/// Requests a read actor answers on the connection its own thread owns.
///
/// The request carries the query as closures, not as data: the row mappers are generic and a
/// channel has one item type, so the query is built by its caller and run where the row it maps is
/// borrowed.  The price is that a request cannot be logged, serialised, or replayed at the actor.
pub enum ReaderRequest {
    /// One query, answered on the reply channel its closure captured.
    Query(QueryRequest),
    /// A query whose rows go to a consumer one at a time.  The actor runs it until its consumer is
    /// gone and answers the scan verdict its closure captured.
    #[cfg(feature = "sqlite")]
    Stream(QueryRequest),
    /// Ends the actor: it answers its ack and drops the connection it owns.
    #[cfg(feature = "external-state-machine")]
    Shutdown(oneshot::Sender<()>),
}

impl ReaderRequest {
    /// Log the saturation of the pool that just took this request, when it waited longer than the
    /// node's `saturated_pool_threshold` for its connection.
    ///
    /// A caller cannot see that wait: the send resolves as soon as the queue has a slot, and the
    /// request then sits in the queue until a connection takes it.  An actor sees it at its own
    /// dequeue, which is the moment its query got its connection, and it is the only one that can
    /// say how long the query waited for it.
    ///
    /// The two shapes of a long wait are not the same problem, so they are not logged as one.
    /// `queued` is the backlog behind this request and `last_busy` is the time this actor spent
    /// inside the query before it: a long `last_busy` with an empty queue is one query holding a
    /// connection, a deep queue with a short `last_busy` is a pool that has more reads than
    /// connections.  Both numbers are the actor's own, and `queued` is read from the channel only
    /// once this request has actually taken the warning, so a pool that stays saturated never
    /// touches the channel for its log.
    fn warn_if_saturated(
        &self,
        pool: &Saturation,
        rx: &flume::Receiver<ReaderRequest>,
        dequeued: Instant,
        last_busy: Duration,
    ) {
        let enqueued = match self {
            Self::Query(req) => req.enqueued,
            #[cfg(feature = "sqlite")]
            Self::Stream(req) => req.enqueued,
            #[cfg(feature = "external-state-machine")]
            Self::Shutdown(_) => return,
        };
        let waited = dequeued
            .checked_duration_since(enqueued)
            .unwrap_or_default();
        if waited < pool.saturated {
            return;
        }
        // The rate limit runs on milliseconds since the pool started, so a saturated pool warns
        // once per interval and its queue stays out of the log.
        let Some(now) = pool.now_ms() else { return };
        if !take_warning(now, &pool.warned) {
            return;
        }
        let queued = rx.len();
        let size = pool.size;
        if last_busy >= pool.saturated {
            warn!(
                "a read query held one of {size} read connections for {last_busy:?}: the next \
                 query waited {waited:?} for it, {queued} queued behind it"
            );
        } else {
            warn!(
                "the read pool is saturated: a query waited {waited:?} for one of {size} read \
                 connections, {queued} queued behind it, the last query on this connection ran \
                 {last_busy:?}"
            );
        }
    }

    /// Answer a request that no connection will ever take, and acknowledge a shutdown that no
    /// actor is left to take.
    ///
    /// A queued request carries its reply senders inside its own closures.  Left in the queue of a
    /// pool that is closing, they stay alive, its reply channel stays connected, and its caller
    /// waits on an answer that nothing can send any more.  Only a sender can break that, so
    /// whoever takes a request off a closing pool has to answer it.
    fn fail_as_closed(self) {
        match self {
            Self::Query(req) => (req.fail)(ActorFailure::Closed),
            #[cfg(feature = "sqlite")]
            Self::Stream(req) => (req.fail)(ActorFailure::Closed),
            #[cfg(feature = "external-state-machine")]
            Self::Shutdown(ack) => {
                let _ = ack.send(());
            }
        }
    }
}

/// The query a read actor runs on its own connection, with `size_hint` as the row capacity its
/// caller expects and the statement it runs as text.  The connection is a call-time borrow, so the
/// actor hands over its own, and so does the statement: it travels in the request, not in the
/// closure, because the actor is the one that can tell a slow query from a busy pool.
type Query = Box<dyn for<'a> FnOnce(&'a Connection, usize, &'a str) + Send>;

/// The shortest gap between two saturation warnings of one read pool: a pool that stays saturated
/// logs its state, not its queue.
const SATURATED_WARN_INTERVAL: Duration = Duration::from_secs(10);

/// The saturation watch of one read pool: the actor count it warns about, the two thresholds it
/// warns at, whether it names a slow statement, and the monotonic stamps of the last warning of
/// each kind it emitted.
struct Saturation {
    start: Instant,
    size: usize,
    saturated: Duration,
    slow_query: Duration,
    log_slow: bool,
    warned: AtomicU64,
    warned_slow: AtomicU64,
}

impl Saturation {
    /// The watch of a pool of `size` read actors, at the node's two thresholds.
    fn new(size: usize, saturated: Duration, slow_query: Duration, log_slow: bool) -> Self {
        Self {
            start: Instant::now(),
            size,
            saturated,
            slow_query,
            log_slow,
            warned: AtomicU64::new(0),
            warned_slow: AtomicU64::new(0),
        }
    }

    /// The milliseconds since the pool started, which is the monotonic clock both rate limits run
    /// on.  A value that does not fit an `u64` is a pool older than 584 million years, and it
    /// stops warning.
    fn now_ms(&self) -> Option<u64> {
        u64::try_from(self.start.elapsed().as_millis()).ok()
    }

    /// The warning of a query that ran longer than the node's `slow_query_threshold` on one of
    /// this pool's connections, naming the statement that did it - when the node logs statements.
    ///
    /// The statement is the whole of what is logged: parameters never reach the log, and they are
    /// not in the request to begin with.  A statement typed into the dashboard is different, because
    /// its values are inline in its text, and that is what the node's `log_slow_queries` is for.
    fn warn_if_slow(&self, busy: Duration, sql: &str) {
        if !self.log_slow || busy < self.slow_query {
            return;
        }
        let Some(now) = self.now_ms() else { return };
        if !take_warning(now, &self.warned_slow) {
            return;
        }
        warn!(
            "slow query on one of {size} read connections, ran {busy:?}: {sql}",
            size = self.size,
        );
    }
}

/// One query on a read actor's own connection.
pub struct QueryRequest {
    /// The rows the caller expects, as the capacity its result is built with: `ROWS_HINT` when the
    /// result size is not known upfront, `1` for the `*_one` variants, `0` for a stream and for a
    /// query that grows its result freely.
    pub size_hint: usize,
    /// The statement the actor runs, as text.  It travels in the request, not in the closure, so
    /// the actor can name it: a slow query is only actionable with the statement that caused it.
    /// Parameters are never part of a request, and never reach a log.
    pub sql: Cow<'static, str>,
    /// Runs the query on the actor's connection, with `size_hint` as its capacity and its statement
    /// as a borrow, and answers its own reply channel.
    pub work: Query,
    /// Answers the reply channel with the `Err` of a failure: the `Err` a panic leaves behind, and
    /// the `Err` of a request no connection will ever take.  Both reach the caller, and the actor
    /// keeps serving.
    pub fail: Box<dyn FnOnce(ActorFailure) + Send>,
    /// When the caller handed the query to the pool.  The actor reads it back at its own dequeue,
    /// which is the moment the query got a connection: the queue absorbs the wait, so the send
    /// resolves long before that, and this is the only place the number exists.
    pub enqueued: Instant,
}

impl QueryRequest {
    /// Run the query on the connection of the actor that took it, and hand back its statement.
    ///
    /// The statement comes back because the actor is the one that measures how long the query ran,
    /// and the one that names it when that was too long - and the request is moved into this call,
    /// so this is where the text is recovered without a copy.
    ///
    /// A panic is caught here, not passed on: the row mappers panic on data that can only be
    /// wrong, exactly like they do inside a blocking task, and the answer their caller gets today
    /// is the `Err` of that task's `JoinError`.  The actor survives with its connection, its
    /// statement cache and its pages intact, and the unwinding of the query's own `Drop`s returns
    /// its statement to that cache in a state the next query can run on.
    fn run(self, conn: &Connection) -> Cow<'static, str> {
        let QueryRequest {
            size_hint,
            sql,
            work,
            fail: on_fail,
            // The stamp is the pool's observability, spent before `run` is reached.
            ..
        } = self;
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| work(conn, size_hint, &sql))) {
            let text = panic_text(&payload);
            trace!("read pool panic: {text}");
            on_fail(ActorFailure::Panicked(text));
        }
        sql
    }
}

/// Requests a read actor keeps queued behind its `recv` before its senders have to wait.
const QUEUE_PER_ACTOR: usize = 4;

/// A SQLite read pool that can close even while clients still hold an `AppState`.
#[cfg(feature = "sqlite")]
#[derive(Debug)]
pub(crate) struct ReadPool {
    sender: std::sync::RwLock<Option<flume::Sender<ReaderRequest>>>,
    closed: flume::Receiver<()>,
}

#[cfg(feature = "sqlite")]
impl ReadPool {
    pub(crate) fn new(sender: flume::Sender<ReaderRequest>, closed: flume::Receiver<()>) -> Self {
        Self {
            sender: std::sync::RwLock::new(Some(sender)),
            closed,
        }
    }

    pub(crate) async fn send(
        &self,
        request: ReaderRequest,
    ) -> Result<(), flume::SendError<ReaderRequest>> {
        let sender = self.sender.read().unwrap().clone();
        match sender {
            Some(sender) => sender.send_async(request).await,
            None => Err(flume::SendError(request)),
        }
    }

    pub(crate) async fn shutdown(&self) {
        drop(self.sender.write().unwrap().take());
        // Existing sends can finish. Disconnection comes after the last actor drops its SQLite
        // connection, so backup restore can safely replace the database on Windows.
        let _ = self.closed.recv_async().await;
    }
}

/// One thread per connection, all of them competing consumers of one channel: a request goes to
/// whichever actor is free, a connection is never acquired, handed over, or shared, and its
/// prepared statement cache, its page cache and its read-only handle stay warm on that thread for
/// the whole life of the node.
///
/// Dropping the last `Sender` disconnects the actors, so every actor leaves its loop and
/// drops its connection - the last one closing the last read handle of the database.  A `flume`
/// receiver takes the messages already in the queue before it reports the disconnect, so a pool
/// that is closed this way serves everything it accepted. The returned completion receiver
/// disconnects once all actors close their connections. An actor that leaves on its
/// [`ReaderRequest::Shutdown`] drains what is behind it and answers those requests with the pool
/// being closed, so a request never outlives the connections it was waiting for.
///
/// A stream is busy work for as long as it lives: its actor is held from its first row to the drop
/// of the stream, the pauses of its consumer included, so `slow_query_threshold` measures a stream
/// exactly like a query that ran the same wall-clock time.  That is the intent - a stream you do not
/// consume blocks its connection for exactly that long, and it is named in the log for it.
///
/// `log_slow_queries` is the node's switch for naming a slow statement: the saturation warnings are
/// emitted either way, and only the line carrying the statement text is silenced.
pub fn spawn_read_actors(
    conns: Vec<Connection>,
    saturated_pool_threshold: Duration,
    slow_query_threshold: Duration,
    log_slow_queries: bool,
) -> (flume::Sender<ReaderRequest>, flume::Receiver<()>) {
    let (tx, rx) = flume::bounded::<ReaderRequest>(conns.len() * QUEUE_PER_ACTOR);
    let (completion, closed) = flume::bounded::<()>(0);
    let pool = Arc::new(Saturation::new(
        conns.len(),
        saturated_pool_threshold,
        slow_query_threshold,
        log_slow_queries,
    ));

    for conn in conns {
        let rx = rx.clone();
        let pool = pool.clone();
        let completion = completion.clone();
        thread::spawn(move || {
            // Drop the SQLite handle before signalling completion, including during unwinding.
            let _completion = completion;
            let connection = conn;
            // The time this actor spent inside the query it last ran, which is the number that
            // names a slow query, and the pair of stamps it comes from is the same pair the queue
            // wait comes from.
            let mut last_busy = Duration::ZERO;
            while let Ok(req) = rx.recv() {
                let dequeued = Instant::now();
                req.warn_if_saturated(&pool, &rx, dequeued, last_busy);
                let sql = match req {
                    ReaderRequest::Query(req) => req.run(&connection),
                    #[cfg(feature = "sqlite")]
                    ReaderRequest::Stream(req) => req.run(&connection),
                    #[cfg(feature = "external-state-machine")]
                    ReaderRequest::Shutdown(ack) => {
                        let _ = ack.send(());
                        break;
                    }
                };
                last_busy = dequeued.elapsed();
                pool.warn_if_slow(last_busy, &sql);
            }

            // The actor is gone, and everything it still takes off the queue has no connection
            // left to run it.  Draining is what answers those callers: `flume` hands a receiver the
            // messages already in the queue before it answers `Disconnected`, so the only way a
            // request is stranded here is an actor leaving on its shutdown with more behind it -
            // which is a snapshot restore closing the pool.  On a disconnect the queue is empty by
            // the time this runs, so the drain answers nothing and changes nothing.
            while let Ok(req) = rx.try_recv() {
                req.fail_as_closed();
            }
        });
    }

    (tx, closed)
}

/// Why a read handed to the read pool never reached its answer.
pub(crate) enum ActorFailure {
    /// The pool is gone: every read connection of the node has been closed.
    Closed,
    /// A connection took the request and died before answering it.
    Died,
    /// The query panicked on its connection, with the text its `JoinError` used to carry.
    Panicked(String),
}

impl ActorFailure {
    /// The crate's `Error` for a read that never reached its answer.
    #[cfg(feature = "sqlite")]
    pub(crate) fn into_error(self) -> crate::Error {
        match self {
            Self::Closed => crate::Error::Sqlite("the read pool is closed".into()),
            Self::Died => crate::Error::Sqlite("the read pool died during the query".into()),
            Self::Panicked(text) => crate::Error::Error(text.into()),
        }
    }
}

/// A read that is inside the read pool's queue, with the channel its answer comes back on.
pub(crate) struct Queued<T, E>
where
    T: Send + 'static,
    E: Send + 'static,
{
    rx: flume::Receiver<Result<T, E>>,
    on_fail: Box<dyn Fn(ActorFailure) -> E + Send>,
}

impl<T, E> Queued<T, E>
where
    T: Send + 'static,
    E: Send + 'static,
{
    /// Build one query for a read actor, and the channel its answer comes back on.
    ///
    /// This is the one place a [`ReaderRequest`] is built: the reply channel, its clone for the
    /// failure half, and the three ways a read never reaches its answer, which `on_fail` turns into
    /// the caller's own `Error`.  The query is built here and run on the actor, because the row it
    /// maps is borrowed on that actor's own connection.  `sql` is the statement, which travels in
    /// the request so the actor can name it in a slow-query warning; parameters are never part of a
    /// request.
    ///
    /// The request and its answer come back apart, because a caller has to hand one over and keep
    /// the other: a read that must not land behind the shutdown of a snapshot restore sends the
    /// request while it holds the pool's lock, and waits for the answer after it hands the lock back.
    pub(crate) fn build<F>(
        sql: Cow<'static, str>,
        size_hint: usize,
        query: F,
        on_fail: impl Fn(ActorFailure) -> E + Clone + Send + 'static,
    ) -> (ReaderRequest, Self)
    where
        F: for<'a> FnOnce(&'a Connection, usize, &'a str) -> Result<T, E> + Send + 'static,
    {
        let (tx, rx) = flume::bounded::<Result<T, E>>(1);
        let tx_fail = tx.clone();
        let on_panic = on_fail.clone();

        (
            ReaderRequest::Query(QueryRequest {
                size_hint,
                sql,
                work: Box::new(move |conn, size_hint, sql| {
                    let _ = tx.send(query(conn, size_hint, sql));
                }),
                fail: Box::new(move |failure| {
                    let _ = tx_fail.send(Err(on_panic(failure)));
                }),
                enqueued: Instant::now(),
            }),
            Self {
                rx,
                on_fail: Box::new(on_fail),
            },
        )
    }

    /// Wait for the answer of a queued read.
    ///
    /// A disconnected reply is [`ActorFailure::Died`]: the connection that took the request is
    /// gone, and its answer with it.
    pub(crate) async fn answer(self) -> Result<T, E> {
        match self.rx.recv_async().await {
            Ok(res) => res,
            Err(_) => Err((self.on_fail)(ActorFailure::Died)),
        }
    }
}

/// Queue a read and wait for its answer. Dropping the caller leaves the actor free to finish.
#[cfg(feature = "sqlite")]
pub(crate) async fn query_on_actor<T, F, E>(
    read_actors: &ReadPool,
    sql: Cow<'static, str>,
    size_hint: usize,
    query: F,
    on_fail: impl Fn(ActorFailure) -> E + Clone + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: Send + Sync + 'static,
    F: for<'a> FnOnce(&'a Connection, usize, &'a str) -> Result<T, E> + Send + 'static,
{
    let (req, queued) = Queued::build(sql, size_hint, query, on_fail);

    read_actors
        .send(req)
        .await
        .map_err(|_| (queued.on_fail)(ActorFailure::Closed))?;

    queued.answer().await
}

/// Whether the warning of one kind may be emitted at `now_ms`, the milliseconds since the read
/// pool started: a pool that stays saturated warns once per interval, not once per query.
///
/// The stamp moves only on a warning that is actually emitted - a pool that stays saturated must
/// warn again when its interval is over, not be silenced by the queries that never warned.  The
/// `compare_exchange` makes the answer unique, so the actors of one pool cannot both take the
/// same warning.
fn take_warning(now_ms: u64, warned: &AtomicU64) -> bool {
    let mut prev = warned.load(Ordering::Relaxed);
    loop {
        let since = u128::from(now_ms.saturating_sub(prev));
        if prev != 0 && since < SATURATED_WARN_INTERVAL.as_millis() {
            return false;
        }
        match warned.compare_exchange(prev, now_ms, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(actual) => prev = actual,
        }
    }
}

/// The text of a panic payload, in the shape the `JoinError` of a blocking task used to carry it.
fn panic_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&'static str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "a read panicked without a panic message".to_string()
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::Error;

    /// The rows of the fixture table, as the `i64` its `id` column holds.
    const ROWS: i64 = 10;

    /// One actor on an in-memory DB with a `t(id)` of `ROWS` rows.
    fn actor() -> flume::Sender<ReaderRequest> {
        actors(1).0
    }

    /// `n` actors, each on its own in-memory connection holding the same `t(id)` fixture.
    fn actors(n: usize) -> (flume::Sender<ReaderRequest>, flume::Receiver<()>) {
        let mut conns = Vec::with_capacity(n);
        for _ in 0..n {
            let conn = Connection::open_in_memory().expect("in-memory connection");
            conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")
                .expect("fixture table");
            for id in 1..=ROWS {
                conn.execute("INSERT INTO t (id) VALUES (?1)", [id])
                    .expect("fixture row");
            }
            conns.push(conn);
        }
        spawn_read_actors(
            conns,
            Duration::from_millis(100),
            Duration::from_secs(1),
            true,
        )
    }

    /// A request that answers the row count of the fixture table.
    fn count_request(tx: flume::Sender<i64>) -> ReaderRequest {
        ReaderRequest::Query(QueryRequest {
            size_hint: 1,
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(move |conn, _size_hint, _sql| {
                let n = conn
                    .query_row("SELECT COUNT(*) FROM t", (), |row| row.get::<_, i64>(0))
                    .expect("fixture count");
                let _ = tx.send(n);
            }),
            fail: Box::new(|_failure| {}),
        })
    }

    /// A request with nothing to answer, to fill a queue.
    fn noop_request() -> ReaderRequest {
        ReaderRequest::Query(QueryRequest {
            size_hint: 0,
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(|_conn, _size_hint, _sql| {}),
            fail: Box::new(|_failure| {}),
        })
    }

    #[tokio::test]
    async fn a_request_runs_on_the_actors_connection() {
        let tx = actor();
        let (reply, rx) = flume::bounded(1);
        tx.send(count_request(reply))
            .expect("the actor is listening");

        assert_eq!(rx.recv_async().await.expect("the actor answered"), ROWS);
    }

    #[tokio::test]
    async fn the_size_hint_reaches_the_actor() {
        let tx = actor();
        let (reply, rx) = flume::bounded(1);
        tx.send(ReaderRequest::Query(QueryRequest {
            size_hint: 42,
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(move |_conn, size_hint, _sql| {
                let _ = reply.send(size_hint);
            }),
            fail: Box::new(|_failure| {}),
        }))
        .expect("the actor is listening");

        assert_eq!(rx.recv_async().await.expect("the actor answered"), 42);
    }

    #[tokio::test]
    async fn a_reply_to_a_dropped_caller_leaves_the_actor_serving() {
        let tx = actor();
        let (dropped, rx) = flume::bounded(1);
        drop(rx);
        tx.send(count_request(dropped))
            .expect("the actor is listening");

        // The actor finished a query whose answer nobody reads, and takes the next one.
        let (reply, rx) = flume::bounded(1);
        tx.send(count_request(reply))
            .expect("the actor is listening");

        assert_eq!(rx.recv_async().await.expect("the actor answers on"), ROWS);
    }

    #[test]
    fn the_queue_is_bounded_by_the_actor_count() {
        let (tx, _) = actors(3);
        assert_eq!(tx.capacity(), Some(3 * QUEUE_PER_ACTOR));
    }

    #[tokio::test]
    async fn shutdown_drains_reads_while_clients_keep_the_pool() {
        let (sender, closed) = actors(2);
        let pool = Arc::new(ReadPool::new(sender, closed));
        let client = pool.clone();
        let (reply, count) = flume::bounded(1);
        client
            .send(count_request(reply.clone()))
            .await
            .expect("the pool accepts a read");

        tokio::time::timeout(Duration::from_secs(10), pool.shutdown())
            .await
            .expect("all connections close");

        assert_eq!(count.try_recv(), Ok(ROWS));
        assert!(client.send(count_request(reply)).await.is_err());
    }

    /// The saturation rate limit: the first saturation warns, the interval silences the ones
    /// behind it, and the interval after the warning lets one through again.
    #[test]
    fn a_saturated_pool_warns_once_per_interval() {
        let pool = Saturation::new(4, Duration::from_millis(100), Duration::from_secs(1), true);
        assert!(
            take_warning(1_000, &pool.warned),
            "the first saturation warns"
        );
        assert!(
            !take_warning(1_500, &pool.warned),
            "the saturation behind it stays quiet"
        );
        assert!(
            !take_warning(9_999, &pool.warned),
            "the interval is not over"
        );
        assert!(
            take_warning(11_000, &pool.warned),
            "the interval after the warning warns again"
        );
        assert!(
            !take_warning(11_500, &pool.warned),
            "the queries that did not warn do not move the interval"
        );
    }

    /// A request that parks its actor inside a query until every sender of a release channel is
    /// dropped: the queue behind it stops draining, which is the shape a restore races.
    fn parking_request(park: flume::Sender<()>, keep: flume::Receiver<()>) -> ReaderRequest {
        ReaderRequest::Query(QueryRequest {
            size_hint: 0,
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(move |_conn, _size_hint, _sql| {
                let _ = park.send(());
                let _ = keep.recv();
            }),
            fail: Box::new(|_failure| {}),
        })
    }

    /// The queue of a busy actor is bounded by its actor count.
    #[tokio::test]
    async fn a_full_queue_blocks_its_sender() {
        let tx = actor();
        // Park the actor inside a query: it signals that it is in there and then waits on a
        // receiver nobody ever answers, so nothing is taken off the queue.
        let (park, parked) = flume::bounded(1);
        let (_keep, keep) = flume::bounded::<()>(1);
        tx.send(parking_request(park, keep))
            .expect("the actor is listening");
        parked
            .recv_async()
            .await
            .expect("the actor is inside its query");

        for _ in 0..QUEUE_PER_ACTOR {
            tx.try_send(noop_request()).expect("the queue has room");
        }
        assert!(
            tx.try_send(noop_request()).is_err(),
            "the queue of a busy actor is full"
        );
    }

    /// Ends every actor of a pool with its shutdown, after `park` has parked them.
    #[cfg(feature = "external-state-machine")]
    #[tokio::test]
    async fn shutdown_ends_every_actor() {
        let (tx, _) = actors(3);
        let mut acks = Vec::new();
        for _ in 0..3 {
            let (ack, rx) = oneshot::channel();
            tx.send(ReaderRequest::Shutdown(ack))
                .expect("the actors are listening");
            acks.push(rx);
        }
        for ack in acks {
            ack.await.expect("the actor acknowledged its shutdown");
        }
    }

    /// Park every actor of a pool inside a query and hand back the sender that releases them.
    ///
    /// A parked actor takes nothing off the queue, which is the state a snapshot restore races:
    /// the shutdowns and the reads that are already queued sit in front of each other.
    #[cfg(feature = "external-state-machine")]
    async fn park_actors(tx: &flume::Sender<ReaderRequest>, count: usize) -> flume::Sender<()> {
        let (park, parked) = flume::bounded::<()>(count);
        let (keep, keep_rx) = flume::bounded::<()>(count);
        for _ in 0..count {
            tx.send(parking_request(park.clone(), keep_rx.clone()))
                .expect("every actor is listening");
        }
        drop(park);
        for _ in 0..count {
            parked
                .recv_async()
                .await
                .expect("every actor is inside its query");
        }
        drop(parked);
        drop(keep_rx);
        keep
    }

    /// The fixture's row count as a read that is inside the pool's queue when this returns, so a
    /// test owns the exact moment the request enters it.
    #[cfg(feature = "external-state-machine")]
    async fn queue_count(tx: &flume::Sender<ReaderRequest>) -> Queued<i64, Error> {
        let (request, queued) = Queued::build(
            Cow::from("SELECT COUNT(*) FROM t"),
            1,
            |conn, _size_hint, _sql| {
                conn.query_row("SELECT COUNT(*) FROM t", (), |row| row.get::<_, i64>(0))
                    .map_err(|err| Error::Sqlite(err.to_string().into()))
            },
            ActorFailure::into_error,
        );
        tx.send_async(request)
            .await
            .expect("the pool accepts the query");
        queued
    }

    /// A read that is queued in front of the shutdowns of a closing pool still runs on its
    /// connection: the queue is served in order, and a shutdown is only the end of that service.
    ///
    /// One actor, so it is the only consumer, and the order it serves is the order of the queue.
    /// With more actors, the second one can take the read out of order, which is a legal answer
    /// and not something this test is about.
    ///
    /// The pool is kept open - the test's own sender stays alive - because a pool whose last
    /// sender is dropped answers every queued read with the closed pool, and that is a different
    /// promise.  The actor's `Shutdown` is the only thing that ends this one.
    #[cfg(feature = "external-state-machine")]
    #[tokio::test]
    async fn a_query_queued_before_the_shutdowns_still_runs() {
        let (tx, _) = actors(1);
        let keep = park_actors(&tx, 1).await;

        let queued = queue_count(&tx).await;
        let (ack, ack_rx) = oneshot::channel();
        tx.send(ReaderRequest::Shutdown(ack))
            .expect("the queue has room behind the parked read");

        drop(keep);
        ack_rx.await.expect("the actor acknowledged its shutdown");
        let answer = tokio::time::timeout(Duration::from_secs(10), queued.answer())
            .await
            .expect("the queued query reached an answer");
        assert_eq!(answer.expect("the query ran"), ROWS);
        drop(tx);
    }

    /// A read that lands behind the shutdowns of a closing pool gets an answer, and not a wait
    /// that never ends.  A queued request carries its reply senders inside its own closures, so
    /// left in the queue it keeps its own reply channel connected, and only an actor taking it off
    /// the queue can break that.
    ///
    /// One actor, so the order in the queue is the order it serves: the shutdown ends the loop and
    /// the read behind it can only come out of the drain.  A pool closed by dropping its sender
    /// needs no drain - `flume` hands a receiver the messages that are already in the queue before
    /// it answers `Disconnected` - so this is the one path that strands a request.
    #[cfg(feature = "external-state-machine")]
    #[tokio::test]
    async fn a_query_behind_the_shutdowns_gets_an_answer() {
        let (tx, _) = actors(1);
        let keep = park_actors(&tx, 1).await;

        let (ack, ack_rx) = oneshot::channel();
        tx.send(ReaderRequest::Shutdown(ack))
            .expect("the queue has room behind the parked read");
        // Behind the shutdown: no actor is left that would take it.
        let queued = queue_count(&tx).await;

        drop(keep);
        ack_rx.await.expect("the actor acknowledged its shutdown");
        let answer = tokio::time::timeout(Duration::from_secs(10), queued.answer())
            .await
            .expect("the query behind the shutdowns reached an answer");
        assert!(
            answer.is_err(),
            "the pool is closing, and the query says so: {answer:?}"
        );
        drop(tx);
    }

    #[tokio::test]
    async fn a_panicking_query_answers_its_caller_and_keeps_the_actor_alive() {
        let tx = actor();
        let (reply, rx) = flume::bounded(1);
        tx.send(ReaderRequest::Query(QueryRequest {
            size_hint: 1,
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(|_conn, _size_hint, _sql| {
                panic!("the row data does not match the mapped type");
            }),
            fail: Box::new(move |failure| {
                let _ = reply.send(Err::<i64, Error>(failure.into_error()));
            }),
        }))
        .expect("the actor is listening");

        let err = rx.recv_async().await.expect("the panic reached its caller");
        assert!(
            matches!(err, Err(Error::Error(_))),
            "a panic answers the variant the JoinError used to: {err:?}"
        );

        // The actor serves on, on the connection it owns.
        let (reply, rx) = flume::bounded(1);
        tx.send(count_request(reply))
            .expect("the actor is listening");

        assert_eq!(rx.recv_async().await.expect("the actor serves on"), ROWS);
    }

    /// The unwinding proof: a panic inside a row mapping unwinds the `RawRows`, the `Statement` and
    /// the `CachedStatement` of the query that panicked, and the cached statement comes back to its
    /// cache in a state the next query of the same SQL can run on.
    #[tokio::test]
    async fn a_panic_mid_cursor_leaves_the_cached_statement_usable() {
        let tx = actor();
        let (reply, rx) = flume::bounded(1);
        tx.send(ReaderRequest::Query(QueryRequest {
            size_hint: usize::try_from(ROWS).expect("row count fits"),
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(|conn, _size_hint, _sql| {
                let mut stmt = conn
                    .prepare_cached("SELECT id FROM t WHERE id >= ?1")
                    .expect("prepare cached statement");
                stmt.raw_bind_parameter(1, 1_i64).expect("bind param");
                let mut rows = stmt.raw_query();
                let mut seen = 0;
                while let Ok(Some(_row)) = rows.next() {
                    seen += 1;
                    if seen == 3 {
                        panic!("the row data does not match the mapped type");
                    }
                }
            }),
            fail: Box::new(move |failure| {
                let _ = reply.send(Err::<Vec<i64>, Error>(failure.into_error()));
            }),
        }))
        .expect("the actor is listening");

        let err = rx.recv_async().await.expect("the panic reached its caller");
        assert!(matches!(err, Err(Error::Error(_))), "{err:?}");

        let (reply, rx) = flume::bounded(1);
        let reply_panic = reply.clone();
        tx.send(ReaderRequest::Query(QueryRequest {
            size_hint: usize::try_from(ROWS).expect("row count fits"),
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(move |conn, size_hint, _sql| {
                let mut stmt = conn
                    .prepare_cached("SELECT id FROM t WHERE id >= ?1")
                    .expect("prepare cached statement");
                stmt.raw_bind_parameter(1, 1_i64).expect("bind param");
                let mut rows = stmt.raw_query();
                let mut ids = Vec::with_capacity(size_hint);
                while let Ok(Some(row)) = rows.next() {
                    ids.push(row.get::<_, i64>(0).expect("id"));
                }
                let _ = reply.send(Ok(ids));
            }),
            fail: Box::new(move |failure| {
                let _ = reply_panic.send(Err(failure.into_error()));
            }),
        }))
        .expect("the actor is listening");

        let ids = rx
            .recv_async()
            .await
            .expect("the actor answered")
            .expect("the same query runs clean after the panic");
        assert_eq!(
            i64::try_from(ids.len()).expect("row count fits"),
            ROWS,
            "the cached statement is usable after the unwind"
        );
    }
}
