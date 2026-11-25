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
/// warns at, and the monotonic stamps of the last warning of each kind it emitted.
struct Saturation {
    start: Instant,
    size: usize,
    saturated: Duration,
    slow_query: Duration,
    warned: AtomicU64,
    warned_slow: AtomicU64,
}

impl Saturation {
    /// The watch of a pool of `size` read actors, at the node's two thresholds.
    fn new(size: usize, saturated: Duration, slow_query: Duration) -> Self {
        Self {
            start: Instant::now(),
            size,
            saturated,
            slow_query,
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
    /// this pool's connections, naming the statement that did it.
    ///
    /// The statement is the whole of what is logged: parameters never reach the log, and they are
    /// not in the request to begin with.
    fn warn_if_slow(&self, busy: Duration, sql: &str) {
        if busy < self.slow_query {
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
    /// Answers the reply channel with the `Err` a panic leaves behind, so a panicking query reaches
    /// its caller and the actor keeps serving.
    pub panic: Box<dyn FnOnce(String) + Send>,
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
            panic: on_panic,
            // The stamp is the pool's observability, spent before `run` is reached.
            ..
        } = self;
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| work(conn, size_hint, &sql))) {
            let text = panic_text(&payload);
            trace!("read pool panic: {text}");
            on_panic(text);
        }
        sql
    }
}

/// Requests a read actor keeps queued behind its `recv` before its senders have to wait.
const QUEUE_PER_ACTOR: usize = 4;

/// One thread per connection, all of them competing consumers of one channel: a request goes to
/// whichever actor is free, a connection is never acquired, handed over, or shared, and its
/// prepared statement cache, its page cache and its read-only handle stay warm on that thread for
/// the whole life of the node.
///
/// Dropping the returned `Sender` disconnects the receivers, so every actor leaves its loop and
/// drops its connection - the last one closing the last read handle of the database.
pub fn spawn_read_actors(
    conns: Vec<Connection>,
    saturated_pool_threshold: Duration,
    slow_query_threshold: Duration,
) -> flume::Sender<ReaderRequest> {
    let (tx, rx) = flume::bounded::<ReaderRequest>(conns.len() * QUEUE_PER_ACTOR);
    let pool = Arc::new(Saturation::new(
        conns.len(),
        saturated_pool_threshold,
        slow_query_threshold,
    ));

    for conn in conns {
        let rx = rx.clone();
        let pool = pool.clone();
        thread::spawn(move || {
            // The time this actor spent inside the query it last ran, which is the number that
            // names a slow query, and the pair of stamps it comes from is the same pair the queue
            // wait comes from.
            let mut last_busy = Duration::ZERO;
            while let Ok(req) = rx.recv() {
                let dequeued = Instant::now();
                req.warn_if_saturated(&pool, &rx, dequeued, last_busy);
                let sql = match req {
                    ReaderRequest::Query(req) => req.run(&conn),
                    #[cfg(feature = "sqlite")]
                    ReaderRequest::Stream(req) => req.run(&conn),
                    #[cfg(feature = "external-state-machine")]
                    ReaderRequest::Shutdown(ack) => {
                        let _ = ack.send(());
                        return;
                    }
                };
                last_busy = dequeued.elapsed();
                pool.warn_if_slow(last_busy, &sql);
            }
        });
    }

    tx
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

/// Hand one query to a read actor and wait for its answer.
///
/// This is the one place a [`ReaderRequest`] is built: the reply channel, its clone for the panic
/// half, and the three ways a read never reaches its answer, which `on_fail` turns into the
/// caller's own `Error`.  The query is built here and run on the actor, because the row it maps is
/// borrowed on that actor's own connection.  `sql` is the statement, which travels in the request
/// so the actor can name it in a slow-query warning; parameters are never part of a request.
///
/// A caller that is dropped mid-query leaves the actor finishing a result nobody reads, which is
/// the cheap answer for a point query - SQLite cannot be interrupted mid-cursor.
pub(crate) async fn query_on_actor<T, F, E>(
    read_actors: &flume::Sender<ReaderRequest>,
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
    let (tx, rx) = flume::bounded::<Result<T, E>>(1);
    let tx_panic = tx.clone();
    let on_panic = on_fail.clone();
    let enqueued = Instant::now();

    read_actors
        .send_async(ReaderRequest::Query(QueryRequest {
            size_hint,
            sql,
            work: Box::new(move |conn, size_hint, sql| {
                let _ = tx.send(query(conn, size_hint, sql));
            }),
            panic: Box::new(move |text| {
                let _ = tx_panic.send(Err(on_panic(ActorFailure::Panicked(text))));
            }),
            enqueued,
        }))
        .await
        .map_err(|_| on_fail(ActorFailure::Closed))?;

    rx.recv_async()
        .await
        .map_err(|_| on_fail(ActorFailure::Died))?
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
        actors(1)
    }

    /// `n` actors, each on its own in-memory connection holding the same `t(id)` fixture.
    fn actors(n: usize) -> flume::Sender<ReaderRequest> {
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
        spawn_read_actors(conns, Duration::from_millis(100), Duration::from_secs(1))
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
            panic: Box::new(|_text| {}),
        })
    }

    /// A request with nothing to answer, to fill a queue.
    fn noop_request() -> ReaderRequest {
        ReaderRequest::Query(QueryRequest {
            size_hint: 0,
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(|_conn, _size_hint, _sql| {}),
            panic: Box::new(|_text| {}),
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
            panic: Box::new(|_text| {}),
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
        let tx = actors(3);
        assert_eq!(tx.capacity(), Some(3 * QUEUE_PER_ACTOR));
    }

    /// The saturation rate limit: the first saturation warns, the interval silences the ones
    /// behind it, and the interval after the warning lets one through again.
    #[test]
    fn a_saturated_pool_warns_once_per_interval() {
        let pool = Saturation::new(4, Duration::from_millis(100), Duration::from_secs(1));
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

    #[tokio::test]
    async fn a_full_queue_blocks_its_sender() {
        let tx = actor();
        // Park the actor inside a query: it signals that it is in there and then waits on a
        // receiver nobody ever answers, so nothing is taken off the queue.
        let (park, parked) = flume::bounded(1);
        let (_keep, keep) = flume::bounded::<()>(1);
        tx.send(ReaderRequest::Query(QueryRequest {
            size_hint: 0,
            sql: Cow::from("SELECT 1"),
            enqueued: Instant::now(),
            work: Box::new(move |_conn, _size_hint, _sql| {
                let _ = park.send(());
                let _ = keep.recv();
            }),
            panic: Box::new(|_text| {}),
        }))
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

    #[cfg(feature = "external-state-machine")]
    #[tokio::test]
    async fn shutdown_ends_every_actor() {
        let tx = actors(3);
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
            panic: Box::new(move |text| {
                let _ = reply.send(Err::<i64, Error>(Error::Error(text.into())));
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
            panic: Box::new(move |text| {
                let _ = reply.send(Err::<Vec<i64>, Error>(Error::Error(text.into())));
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
            panic: Box::new(move |text| {
                let _ = reply_panic.send(Err(Error::Error(text.into())));
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
