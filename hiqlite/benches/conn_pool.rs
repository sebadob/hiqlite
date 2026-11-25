//! Today's `deadpool` read pool against the read actors that replace it.
//!
//! Four read connections and eight readers fighting over them - the production shape of a node
//! (`read_pool_size` 4, many concurrent queries) - on an on-disk database, every variant running
//! the same query.  Only the thing that gets the query onto a connection differs:
//!
//!   * `async/deadpool` - today's production path, exactly as `query::query_owned_local` runs it:
//!     `pool.get().await`, the query inside `task::spawn_blocking`, the guard dropped on release.
//!   * `async/actor` - the shipped design: one OS thread per connection, the query travelling to
//!     that thread as a closure, the answer coming back over a channel.  No acquire, no release,
//!     no `spawn_blocking`.  This is `reader::spawn_read_actors` in bench shape, request shape
//!     included.
//!   * `async/actor+stamp` - the same actors with the saturation probe on its hot path: the stamp
//!     travels inside the payload, the actor reads it back at its dequeue and stays below the
//!     threshold, so it reports nothing.  The delta to `async/actor` is what the observability
//!     costs in the caller and inside the actor.
//!   * `async/actor+probe` - the same again with the measured queue wait reported back over a
//!     channel, which is what the saturation and queue-depth runs read.
//!   * `sync/deadpool` - the same pool with the async taken out: OS threads spinning on `try_get`.
//!     The ceiling of what that pool can do, so the actor's share of it is measurable.
//!   * `local` - no pool and no handoff at all: every reader owns its own connection.
//!
//! The workloads are the production mappings, allocations included:
//!
//!   * `trivial` - a cached `SELECT 1`, where the handoff dominates.
//!   * `query_map` - `query_owned_local`: column info, bind, owned row values, the column names.
//!   * `direct_map` - the borrowed row mapped straight into the typed value, which is what a
//!     request carrying a closure makes possible.
//!   * `owned_then_map` - the same typed value built from the owned row form, which is what a
//!     request carrying data has to do.
//!   * `direct_map_narrow` and `owned_then_map_narrow` - the same pair on a one-column `INTEGER`
//!     result, the OLTP shape.
//!
//! `QUERY` is the wide shape (`INTEGER` + `TEXT` + a 200-byte `BLOB`), `QUERY_NARROW` the narrow
//! one; both read 100 rows out of a table far bigger than any connection's page cache.
//!
//! Latency is measured from the caller of a query, which is the only place the handoff is
//! visible, and throughput is the median over the rounds of one full run of `workers * ops`
//! operations.
//!
//! The saturation run measures the two times a probe can mean, side by side, because they are not
//! the same time: the caller's stamp around the send only covers the time the queue had no room
//! for the payload, and the queue is `QUEUE_PER_ACTOR` slots deep per connection.  The actor's
//! stamp covers the time a query waits for a free connection, which is the number a saturation
//! warning is about.
//!
//! The queue depth run walks the other axis: how many requests the channel keeps in front of each
//! connection.  A deep queue absorbs a burst into the channel, where a query whose caller has
//! already given up still runs on an actor; a shallow one parks the senders, where a dropped
//! caller leaves nothing behind.
//!
//! Run it inside a fixed core envelope, or the numbers are not comparable between runs:
//!
//! ```text
//! just bench-conn-pool                 # the criterion matrix
//! just bench-conn-pool -- --summary    # the throughput and latency table
//! just bench-conn-pool -- --sweep      # connection counts x reader counts
//! just bench-conn-pool -- --saturation # queue wait vs send block, per reader count
//! just bench-conn-pool -- --queue      # requests queued per connection, per reader count
//! just bench-conn-pool -- --queue=2:4  # one shape: connections x readers, every queue depth
//! just bench-conn-pool -- --memory=connections
//! just bench-conn-pool -- --memory=stmts:1024
//! just bench-conn-pool -- --memory=pool:10000
//! ```
//!
//! The closed-loop model is worth knowing before reading a number: every reader issues its next
//! query the moment the previous one lands, so a low-latency design looks best at low reader
//! counts.  The sweep is what shows the shape across reader counts.

use std::fmt::Write;
use std::future::Future;
use std::hint::{black_box, spin_loop};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};

use criterion::Criterion;
use criterion::Throughput;
use criterion::criterion_group;
use deadpool::unmanaged::Object;
use deadpool::unmanaged::Pool;
use rusqlite::{Connection, OpenFlags};
use std::env;
use thread_priority::ThreadPriority;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;
use tokio::task::spawn_blocking;

/// Read connections in the pool - matches the production default of a 4-core box.
const CONNECTIONS: usize = 4;
/// Readers contending for those connections.
const WORKERS: usize = 8;
/// Acquire/work/release operations each reader performs in one measured run.
const OPS: usize = 2_000;
/// Rows in the benchmark table; the query binds `id >= ?1` and returns the tail of them.
const ROWS: usize = 20_000;
/// Bytes per row payload.
const BLOB: usize = 200;
/// Rounds per variant in the summary table; the first one warms the pool and the caches up.
const SUMMARY_ROUNDS: usize = 4;
/// Criterion samples per variant.
const SAMPLE_SIZE: usize = 10;
/// Criterion time budget per variant.
const MEASUREMENT_TIME: Duration = Duration::from_secs(2);
/// Connection counts the sweep walks.
const SWEEP_CONNS: [usize; 4] = [2, 4, 6, 8];
/// Reader counts the sweep walks.
const SWEEP_WORKERS: [usize; 7] = [1, 2, 4, 8, 16, 32, 64];
/// Total operations per sweep round, split over the readers, so every round does the same amount
/// of work no matter how many readers share it.
const SWEEP_OPS: usize = 32_000;
/// Rounds per sweep cell; the first one warms the pool up.
const SWEEP_ROUNDS: usize = 3;
/// Reader counts the saturation run walks, at the production connection count.
const SAT_WORKERS: [usize; 5] = [4, 8, 16, 32, 64];
/// Rounds per saturation cell; the first one warms the caches up.
const SAT_ROUNDS: usize = 2;
/// The queue wait a saturation warning would fire at.
const SAT_THRESHOLD: Duration = Duration::from_millis(100);
/// Requests a read actor keeps queued behind its `recv` before its senders have to wait.
const QUEUE_PER_ACTOR: usize = 4;
/// The queue depths the depth run walks, in requests per connection.
const QUEUE_DEPTHS: [usize; 4] = [1, 2, 4, 8];

/// The shape of the main matrix: the production default, with the scheduler in charge.
const SHAPE: Shape = Shape {
    workers: WORKERS,
    ops: OPS,
};

const TRIVIAL: &str = "SELECT 1";
/// A range scan of `ROW_SPAN` wide rows, so one query maps a fixed, small result set out of a
/// database that is far bigger than a connection's page cache.
const QUERY: &str = "SELECT id, name, payload FROM t WHERE id >= ?1 AND id < ?1 + 100";
/// The narrow form of the same query: one INTEGER column, no TEXT, no BLOB.
const QUERY_NARROW: &str = "SELECT id FROM t WHERE id >= ?1 AND id < ?1 + 100";
/// Rows a query maps per operation; it is spelled out in `QUERY` and asserted here.
const ROW_SPAN: usize = 100;

/// The row-vector capacity `hiqlite`'s `query_owned_local` starts its result at.
const ROWS_HINT: usize = 4;

/// The tokio runtime every async variant runs on, built once for the whole bench.
static RT: LazyLock<Runtime> = LazyLock::new(|| {
    Builder::new_multi_thread()
        .worker_threads(thread::available_parallelism().map_or(4, usize::from))
        .enable_all()
        .build()
        .expect("tokio runtime")
});

/// The file database every variant reads: one file, opened by every connection, exactly like a
/// node opens its read connections.
static DB_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = std::env::temp_dir().join("hiqlite-conn-pool-mem");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("db.sqlite");
    let bytes = seed_file(&path);
    println!("file database: {ROWS} rows x {BLOB} bytes, {bytes} bytes on disk");
    path
});

/// The owned value shape of `hiqlite::query::rows::ValueOwned`: TEXT and BLOB are copies out of
/// the connection's pages, allocated per row and per column.
enum Own {
    Int(i64),
    Text(String),
    Blob(Vec<u8>),
}

impl Own {
    fn as_i64(&self) -> i64 {
        match self {
            Self::Int(i) => *i,
            Self::Text(_) | Self::Blob(_) => panic!("the id column is an INTEGER"),
        }
    }

    fn as_text(&self) -> &str {
        match self {
            Self::Text(t) => t,
            Self::Int(_) | Self::Blob(_) => panic!("the name column is a TEXT"),
        }
    }

    fn as_blob(&self) -> &[u8] {
        match self {
            Self::Blob(b) => b,
            Self::Int(_) | Self::Text(_) => panic!("the payload column is a BLOB"),
        }
    }
}

/// The column type `hiqlite` resolves once per statement from the column name.
#[derive(Clone, Copy)]
enum Kind {
    Int,
    Text,
    Blob,
}

/// The column info `ColumnInfo::mapping_cols_from_stmt` builds per call: a name and a type.
struct Col {
    name: String,
    kind: Kind,
}

fn col_info(stmt: &rusqlite::Statement) -> Vec<Col> {
    stmt.columns()
        .map(|c| {
            let name = c.name().to_string();
            let kind = match name.as_str() {
                "id" => Kind::Int,
                "name" => Kind::Text,
                _ => Kind::Blob,
            };
            Col { name, kind }
        })
        .collect()
}

/// `RowValues::from_row_column`: one `Vec` per row, one owned copy per TEXT and BLOB column.
fn row_values(row: &rusqlite::Row, cols: &[Col]) -> Vec<Own> {
    let mut values = Vec::with_capacity(cols.len());
    for (i, info) in cols.iter().enumerate() {
        values.push(match info.kind {
            Kind::Int => Own::Int(row.get::<_, i64>(i).expect("id")),
            Kind::Text => Own::Text(row.get::<_, String>(i).expect("name")),
            Kind::Blob => Own::Blob(row.get::<_, Vec<u8>>(i).expect("payload")),
        });
    }
    values
}

/// A typed row: the shape a mapped type has, built straight from the borrowed row.
struct Thing {
    id: i64,
    name: String,
    payload: Vec<u8>,
}

impl Thing {
    /// A caller consumes the values, which is what keeps the mapping from being optimised away.
    fn consume(&self) {
        let _ = black_box((&self.id, &self.name, &self.payload));
    }
}

/// `query_owned_local` as it runs in production: column info, bind, query, owned row values and
/// the column-name vector, all built on the thread that owns the connection.
fn query_owned_local(conn: &Connection, seq: usize) -> usize {
    let mut stmt = conn
        .prepare_cached(QUERY)
        .expect("prepare cached statement");
    let cols = col_info(&stmt);
    bind_id(&mut stmt, seq);

    let mut rows = stmt.raw_query();
    let mut rows_owned = Vec::with_capacity(ROWS_HINT);
    while let Some(row) = rows.next().expect("read row") {
        rows_owned.push(row_values(row, &cols));
    }

    let names = cols.into_iter().map(|c| c.name).collect::<Vec<String>>();
    black_box(&names);

    // A caller consumes the values, which is what keeps them from being optimised away.
    for row in &rows_owned {
        for value in row {
            match value {
                Own::Int(i) => {
                    let _ = black_box(i);
                }
                Own::Text(t) => {
                    let _ = black_box(t);
                }
                Own::Blob(b) => {
                    let _ = black_box(b);
                }
            }
        }
    }
    rows_owned.len()
}

/// The mapping a request carrying a mapper does: the borrowed row goes straight into the typed
/// value, on the thread that owns the connection.
fn direct_map(conn: &Connection, seq: usize) -> usize {
    let mut stmt = conn
        .prepare_cached(QUERY)
        .expect("prepare cached statement");
    bind_id(&mut stmt, seq);

    let mut rows = stmt.raw_query();
    let mut res = Vec::with_capacity(ROWS_HINT);
    while let Some(row) = rows.next().expect("read row") {
        res.push(Thing {
            id: row.get::<_, i64>(0).expect("id"),
            name: row.get::<_, String>(1).expect("name"),
            payload: row.get::<_, Vec<u8>>(2).expect("payload"),
        });
    }
    for row in &res {
        row.consume();
    }
    res.len()
}

/// The mapping a data-only request does: the actor builds the owned row form, the typed value is
/// built from it after the row is already copied out of the connection.
fn owned_then_map(conn: &Connection, seq: usize) -> usize {
    let mut stmt = conn
        .prepare_cached(QUERY)
        .expect("prepare cached statement");
    let cols = col_info(&stmt);
    bind_id(&mut stmt, seq);

    let mut rows = stmt.raw_query();
    let mut owned = Vec::with_capacity(ROWS_HINT);
    while let Some(row) = rows.next().expect("read row") {
        owned.push(row_values(row, &cols));
    }
    let names = cols.into_iter().map(|c| c.name).collect::<Vec<String>>();

    let mut res = Vec::with_capacity(owned.len());
    for values in &owned {
        res.push(Thing {
            id: values[0].as_i64(),
            name: values[1].as_text().to_string(),
            payload: values[2].as_blob().to_vec(),
        });
    }
    let _ = black_box(&names);
    for row in &res {
        row.consume();
    }
    res.len()
}

/// The narrow query mapped straight from the borrowed row: one `i64` per row, nothing owned.
fn direct_map_narrow(conn: &Connection, seq: usize) -> usize {
    let mut stmt = conn
        .prepare_cached(QUERY_NARROW)
        .expect("prepare cached statement");
    bind_id(&mut stmt, seq);

    let mut rows = stmt.raw_query();
    let mut res = Vec::with_capacity(ROWS_HINT);
    while let Some(row) = rows.next().expect("read row") {
        res.push(row.get::<_, i64>(0).expect("id"));
    }
    for id in &res {
        let _ = black_box(id);
    }
    res.len()
}

/// The narrow query through the owned row form first: a `Vec` of values per row, then the `i64`.
fn owned_then_map_narrow(conn: &Connection, seq: usize) -> usize {
    let mut stmt = conn
        .prepare_cached(QUERY_NARROW)
        .expect("prepare cached statement");
    let cols = col_info(&stmt);
    bind_id(&mut stmt, seq);

    let mut rows = stmt.raw_query();
    let mut owned = Vec::with_capacity(ROWS_HINT);
    while let Some(row) = rows.next().expect("read row") {
        owned.push(row_values(row, &cols));
    }
    let names = cols.into_iter().map(|c| c.name).collect::<Vec<String>>();
    let res = owned
        .iter()
        .map(|values| values[0].as_i64())
        .collect::<Vec<i64>>();
    let _ = black_box((&names, &res));
    res.len()
}

fn bind_id(stmt: &mut rusqlite::Statement, seq: usize) {
    let max = ROWS as u64 - ROW_SPAN as u64;
    let id = i64::try_from(seq as u64 % max).expect("row id fits into i64") + 1;
    stmt.raw_bind_parameter(1, id).expect("bind id");
}

/// What a reader does with the connection it ends up on.
#[derive(Clone, Copy)]
enum Work {
    /// A cached `SELECT 1`: the handoff's own cost dominates.
    Trivial,
    /// The wide range select, read row by row.
    Query,
    /// `query_owned_local` as production runs it: owned row values built on the connection.
    QueryMap,
    /// The borrowed row mapped straight into the typed value.
    DirectMap,
    /// The owned row form first, then the typed value built from it.
    OwnedThenMap,
    /// The narrow query mapped straight from the borrowed row.
    DirectMapNarrow,
    /// The narrow query through the owned row form first.
    OwnedThenMapNarrow,
}

impl Work {
    fn label(self) -> &'static str {
        match self {
            Self::Trivial => "trivial",
            Self::Query => "query",
            Self::QueryMap => "query_map",
            Self::DirectMap => "direct_map",
            Self::OwnedThenMap => "owned_then_map",
            Self::DirectMapNarrow => "direct_map_narrow",
            Self::OwnedThenMapNarrow => "owned_then_map_narrow",
        }
    }
}

/// The shape of one measured run: how many readers fight for the connections, and how many
/// operations each of them performs per round.  The connection count lives in the pool.
#[derive(Clone, Copy)]
struct Shape {
    workers: usize,
    ops: usize,
}

/// The result of one full run over all readers.
struct Run {
    ops_per_sec: f64,
    p50: Duration,
    p90: Duration,
    p99: Duration,
    max: Duration,
}

/// A pool that hands out a guard derefing to a connection, so every variant shares the reader
/// loop: acquire the guard, run the work on a thread that owns it, hand it back.
///
/// # Panics
///
/// Every implementation panics when the pool is closed, which cannot happen while a reader owns
/// part of it.
trait ConnPool: Send + Sync + 'static {
    type Guard: Deref<Target = Connection> + Send + 'static;

    /// Async acquire - the path a tokio task takes.
    fn acquire(&self) -> impl Future<Output = Self::Guard> + Send;

    /// Blocking acquire - the path a plain thread, and a `spawn_blocking` thread, takes.
    fn acquire_sync(&self) -> Self::Guard;

    /// Hand the connection back to the pool.
    fn release(&self, guard: Self::Guard);
}

/// Today's production pool: `deadpool::unmanaged::Pool`.
struct Deadpool(Pool<Connection>);

impl ConnPool for Deadpool {
    type Guard = Object<Connection>;

    async fn acquire(&self) -> Self::Guard {
        self.0.get().await.expect("pool is never closed")
    }

    fn acquire_sync(&self) -> Self::Guard {
        loop {
            if let Ok(obj) = self.0.try_get() {
                return obj;
            }
            spin_loop();
        }
    }

    fn release(&self, guard: Self::Guard) {
        drop(guard);
    }
}

/// Channel payload: owns the connection, so `release` hands it back to the queue.
struct Chan(Connection);

impl Deref for Chan {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.0
    }
}

/// An unbounded `crossbeam::channel` as a pool: no async, no mutex, no permit counting.  The
/// strongest pool shape to sweep connection counts against.
struct Crossbeam {
    tx: crossbeam::channel::Sender<Chan>,
    rx: crossbeam::channel::Receiver<Chan>,
}

impl ConnPool for Crossbeam {
    type Guard = Chan;

    async fn acquire(&self) -> Self::Guard {
        let rx = self.rx.clone();
        spawn_blocking(move || rx.recv().expect("sender is never dropped"))
            .await
            .expect("acquire task never panics")
    }

    fn acquire_sync(&self) -> Self::Guard {
        self.rx.recv().expect("sender is never dropped")
    }

    fn release(&self, guard: Self::Guard) {
        self.tx.send(guard).expect("receiver is never dropped");
    }
}

/// The query a read actor runs on its own connection, with the row capacity its caller expects.
type Query = Box<dyn for<'a> FnOnce(&'a Connection, usize) + Send>;

/// The queue-wait probe carried inside the payload: the caller stamps the instant it hands the
/// request to the pool, and the actor reads it back at its own dequeue, which is the moment the
/// query got a connection.  `report` is bench-only - a node answers a metric and a rate-limited
/// log at the threshold instead - and `None` is the hot path a node runs on.
struct Probe {
    enqueued: Instant,
    report: Option<flume::Sender<Duration>>,
}

/// A request to a read actor, in the shape `reader::QueryRequest` has: the query as a closure
/// that answers its own reply channel, and the row capacity the caller expects.
struct Request {
    size_hint: usize,
    work: Query,
    /// The probe, when the caller attached one.
    probe: Option<Probe>,
}

/// The actor shape: one thread per connection, requests going onto one shared `flume` queue the
/// threads compete for, answers coming back over a channel.  No acquire, no release, no
/// connection handoff, and no `spawn_blocking`.
struct Actors {
    tx: flume::Sender<Request>,
}

impl Actors {
    /// One actor thread per connection, at the priority a node runs its actors at.  The cloned
    /// receivers compete for the queue, so the queue balances the load and no routing is needed.
    /// One actor thread per connection, at the production queue depth.
    fn new(count: usize) -> Self {
        Self::with_queue(count, QUEUE_PER_ACTOR)
    }

    /// One actor thread per connection, with `queue` requests of room per connection in front of
    /// them.  The queue depth is what decides how deep a burst is absorbed before a sender parks.
    fn with_queue(count: usize, queue: usize) -> Self {
        Self::with_queue_conns(count, queue, conns(count))
    }

    /// The same actors on connections opened with a given set of flags.
    fn with_queue_flags(count: usize, flags: OpenFlags, query_only: bool) -> Self {
        Self::with_queue_conns(count, QUEUE_PER_ACTOR, conns_with(count, flags, query_only))
    }

    fn with_queue_conns(count: usize, queue: usize, conns: Vec<Connection>) -> Self {
        let (tx, rx) = flume::bounded::<Request>(count * queue);
        for conn in conns {
            let rx = rx.clone();
            thread::Builder::new()
                .name("actor".into())
                .spawn(move || {
                    let _ = ThreadPriority::Max.set_for_current();
                    while let Ok(req) = rx.recv() {
                        // The queue wait is only visible here: the send resolves as soon as the
                        // queue has room, and the request then sits in it until this line runs.
                        if let Some(probe) = req.probe {
                            let waited = probe.enqueued.elapsed();
                            if let Some(report) = probe.report {
                                let _ = report.send(waited);
                            }
                        }
                        (req.work)(&conn, req.size_hint);
                    }
                })
                .expect("spawn actor thread");
        }
        // The actors own the only receiver handles, so dropping this one lets them exit once the
        // last sender goes away.
        drop(rx);
        Self { tx }
    }

    async fn call(&self, kind: Work, seq: usize) -> u64 {
        let (reply, answer) = flume::bounded(1);
        self.tx
            .send_async(Request {
                size_hint: ROWS_HINT,
                work: Box::new(move |conn, _size_hint| {
                    let _ = reply.send(work(conn, kind, seq));
                }),
                probe: None,
            })
            .await
            .expect("the actors are alive");
        answer.recv_async().await.expect("the actor answers")
    }

    /// The probe variant: the caller stamps before the payload goes onto the queue, reads the
    /// stamp back as soon as the queue has taken it, and the actor reports the queue wait it
    /// measured at its own dequeue.  Returns the checksum, the send block and the queue wait.
    async fn call_probed(&self, kind: Work, seq: usize) -> (u64, Duration, Duration) {
        let (reply, answer) = flume::bounded(1);
        let (report, waited) = flume::bounded(1);
        let enqueued = Instant::now();
        let send = {
            let t0 = Instant::now();
            self.tx
                .send_async(Request {
                    size_hint: ROWS_HINT,
                    work: Box::new(move |conn, _size_hint| {
                        let _ = reply.send(work(conn, kind, seq));
                    }),
                    probe: Some(Probe {
                        enqueued,
                        report: Some(report),
                    }),
                })
                .await
                .expect("the actors are alive");
            t0.elapsed()
        };
        let queue = waited
            .recv_async()
            .await
            .expect("the actor measured the queue wait");
        let checksum = answer.recv_async().await.expect("the actor answers");
        (checksum, send, queue)
    }

    /// The hot path a node runs on: the stamp travels inside the payload, the actor reads it back
    /// at its dequeue, stays below the threshold and reports nothing.  Returns the checksum and the
    /// time the send was blocked.
    async fn call_stamped(&self, kind: Work, seq: usize) -> (u64, Duration) {
        let (reply, answer) = flume::bounded(1);
        let enqueued = Instant::now();
        let send = {
            let t0 = Instant::now();
            self.tx
                .send_async(Request {
                    size_hint: ROWS_HINT,
                    work: Box::new(move |conn, _size_hint| {
                        let _ = reply.send(work(conn, kind, seq));
                    }),
                    probe: Some(Probe {
                        enqueued,
                        report: None,
                    }),
                })
                .await
                .expect("the actors are alive");
            t0.elapsed()
        };
        let checksum = answer.recv_async().await.expect("the actor answers");
        (checksum, send)
    }
}

// ---------------------------------------------------------------------------
// work
// ---------------------------------------------------------------------------

/// Runs the workload on the connection and returns a checksum of what it read.
fn work(conn: &Connection, kind: Work, seq: usize) -> u64 {
    match kind {
        Work::Trivial => {
            let mut stmt = conn.prepare_cached(TRIVIAL).expect("prepare SELECT 1");
            let mut rows = stmt.raw_query();
            let row = rows.next().expect("SELECT 1").expect("one row");
            u64::try_from(row.get::<_, i64>(0).expect("column 0")).expect("row count fits")
        }
        Work::QueryMap => u64::try_from(query_owned_local(conn, seq)).expect("row count fits"),
        Work::DirectMap => u64::try_from(direct_map(conn, seq)).expect("row count fits"),
        Work::OwnedThenMap => u64::try_from(owned_then_map(conn, seq)).expect("row count fits"),
        Work::DirectMapNarrow => {
            u64::try_from(direct_map_narrow(conn, seq)).expect("row count fits")
        }
        Work::OwnedThenMapNarrow => {
            u64::try_from(owned_then_map_narrow(conn, seq)).expect("row count fits")
        }
        Work::Query => {
            let mut stmt = conn
                .prepare_cached(QUERY)
                .expect("prepare cached statement");
            bind_id(&mut stmt, seq);
            let mut rows = stmt.raw_query();
            let mut acc = 0u64;
            while let Some(row) = rows.next().expect("read row") {
                acc += u64::try_from(row.get::<_, i64>(0).expect("id")).expect("id fits");
            }
            acc
        }
    }
}

// ---------------------------------------------------------------------------
// runners
// ---------------------------------------------------------------------------

/// The async shape: `workers` tokio tasks, DB work inside `spawn_blocking`, like
/// `query::query_owned_local`.
fn run_async<P: ConnPool>(pool: &Arc<P>, kind: Work, shape: Shape, rounds: usize) -> Run {
    let mut per_sec = Vec::with_capacity(rounds);
    let mut lat: Vec<Duration> = Vec::with_capacity(rounds * shape.workers * shape.ops);

    for _ in 0..rounds {
        let p = pool.clone();
        let start = Instant::now();
        let done = RT.block_on(async move {
            let mut set = JoinSet::new();
            for worker in 0..shape.workers {
                let p = p.clone();
                set.spawn(async move {
                    let mut lat = Vec::with_capacity(shape.ops);
                    for seq in 0..shape.ops {
                        let t0 = Instant::now();
                        let guard = p.acquire().await;
                        let guard = spawn_blocking(move || {
                            black_box(work(&guard, kind, seq + worker));
                            guard
                        })
                        .await
                        .expect("work task never panics");
                        p.release(guard);
                        lat.push(t0.elapsed());
                    }
                    lat
                });
            }
            let mut lat = Vec::with_capacity(shape.workers * shape.ops);
            while let Some(res) = set.join_next().await {
                lat.extend(res.expect("worker task never panics"));
            }
            lat
        });
        per_sec.push(ops_per_sec(shape.workers * shape.ops, start.elapsed()));
        lat.extend(done);
    }
    stats(per_sec, lat)
}

/// The sync shape: `workers` OS threads, DB work inline.  The ceiling of the same pool with the
/// async taken out.
fn run_sync<P: ConnPool>(pool: &Arc<P>, kind: Work, shape: Shape, rounds: usize) -> Run {
    let mut per_sec = Vec::with_capacity(rounds);
    let mut lat: Vec<Duration> = Vec::with_capacity(rounds * shape.workers * shape.ops);

    for _ in 0..rounds {
        let p = pool.clone();
        let gate = Arc::new(Barrier::new(shape.workers + 1));
        let mut handles = Vec::with_capacity(shape.workers);
        for worker in 0..shape.workers {
            let p = p.clone();
            let gate = gate.clone();
            handles.push(
                thread::Builder::new()
                    .name(format!("worker-{worker}"))
                    .spawn(move || {
                        gate.wait();
                        let mut lat = Vec::with_capacity(shape.ops);
                        for seq in 0..shape.ops {
                            let t0 = Instant::now();
                            let guard = p.acquire_sync();
                            black_box(work(&guard, kind, seq + worker));
                            p.release(guard);
                            lat.push(t0.elapsed());
                        }
                        lat
                    })
                    .expect("spawn worker thread"),
            );
        }
        // Every worker exists and is parked at the gate: the thread spawn ramp is out of the
        // measured window, and all of them start on the same beat.
        gate.wait();
        let start = Instant::now();
        let mut done = Vec::with_capacity(shape.workers * shape.ops);
        for h in handles {
            done.extend(h.join().expect("worker thread never panics"));
        }
        per_sec.push(ops_per_sec(shape.workers * shape.ops, start.elapsed()));
        lat.extend(done);
    }
    stats(per_sec, lat)
}

/// The actor shape: `workers` tokio tasks, each request answered by the actor that took it off
/// the shared queue.
fn run_actors(actors: &Arc<Actors>, kind: Work, shape: Shape, rounds: usize) -> Run {
    let mut per_sec = Vec::with_capacity(rounds);
    let mut lat: Vec<Duration> = Vec::with_capacity(rounds * shape.workers * shape.ops);

    for _ in 0..rounds {
        let p = actors.clone();
        let start = Instant::now();
        let done = RT.block_on(async move {
            let mut set = JoinSet::new();
            for worker in 0..shape.workers {
                let p = p.clone();
                set.spawn(async move {
                    let mut lat = Vec::with_capacity(shape.ops);
                    for seq in 0..shape.ops {
                        let t0 = Instant::now();
                        black_box(p.call(kind, seq + worker).await);
                        lat.push(t0.elapsed());
                    }
                    lat
                });
            }
            let mut lat = Vec::with_capacity(shape.workers * shape.ops);
            while let Some(res) = set.join_next().await {
                lat.extend(res.expect("worker task never panics"));
            }
            lat
        });
        per_sec.push(ops_per_sec(shape.workers * shape.ops, start.elapsed()));
        lat.extend(done);
    }
    stats(per_sec, lat)
}

/// The probe shape: the same actors and the same readers, with the saturation probe attached to
/// every request.  Its `ops_per_sec` and latencies against `run_actors` are what the probe costs,
/// in the caller and inside the actor.
fn run_actors_probe(actors: &Arc<Actors>, kind: Work, shape: Shape, rounds: usize) -> Run {
    let mut per_sec = Vec::with_capacity(rounds);
    let mut lat: Vec<Duration> = Vec::with_capacity(rounds * shape.workers * shape.ops);

    for _ in 0..rounds {
        let p = actors.clone();
        let start = Instant::now();
        let done = RT.block_on(async move {
            let mut set = JoinSet::new();
            for worker in 0..shape.workers {
                let p = p.clone();
                set.spawn(async move {
                    let mut lat = Vec::with_capacity(shape.ops);
                    for seq in 0..shape.ops {
                        let t0 = Instant::now();
                        let (checksum, send, queue) = p.call_probed(kind, seq + worker).await;
                        black_box((checksum, send, queue));
                        lat.push(t0.elapsed());
                    }
                    lat
                });
            }
            let mut lat = Vec::with_capacity(shape.workers * shape.ops);
            while let Some(res) = set.join_next().await {
                lat.extend(res.expect("worker task never panics"));
            }
            lat
        });
        per_sec.push(ops_per_sec(shape.workers * shape.ops, start.elapsed()));
        lat.extend(done);
    }
    stats(per_sec, lat)
}

/// The stamp shape: the probe a node runs on, with the stamp inside the payload and the actor
/// reading it back at its dequeue below the threshold, so nothing is reported.  Its delta to
/// `run_actors` is the price of the observability on the hot path, in the caller and in the actor.
fn run_actors_stamp(actors: &Arc<Actors>, kind: Work, shape: Shape, rounds: usize) -> Run {
    let mut per_sec = Vec::with_capacity(rounds);
    let mut lat: Vec<Duration> = Vec::with_capacity(rounds * shape.workers * shape.ops);

    for _ in 0..rounds {
        let p = actors.clone();
        let start = Instant::now();
        let done = RT.block_on(async move {
            let mut set = JoinSet::new();
            for worker in 0..shape.workers {
                let p = p.clone();
                set.spawn(async move {
                    let mut lat = Vec::with_capacity(shape.ops);
                    for seq in 0..shape.ops {
                        let t0 = Instant::now();
                        let (checksum, send) = p.call_stamped(kind, seq + worker).await;
                        black_box((checksum, send));
                        lat.push(t0.elapsed());
                    }
                    lat
                });
            }
            let mut lat = Vec::with_capacity(shape.workers * shape.ops);
            while let Some(res) = set.join_next().await {
                lat.extend(res.expect("worker task never panics"));
            }
            lat
        });
        per_sec.push(ops_per_sec(shape.workers * shape.ops, start.elapsed()));
        lat.extend(done);
    }
    stats(per_sec, lat)
}

/// One measured query of the saturation run: the time the send was blocked, the queue wait the
/// actor measured, and the total time the caller spent on the query.
struct Sat {
    send: Duration,
    queue: Duration,
    total: Duration,
}

/// One measured round of the saturation run: every query's three times, and the wall time the
/// whole round took, which is the throughput of the shape.
struct Round {
    sat: Vec<Sat>,
    wall: Duration,
}

/// The saturation run: `workers` readers, more of them than the connections can serve at once, so
/// the queue is deep and the two times separate.
fn run_saturated(actors: &Arc<Actors>, kind: Work, shape: Shape) -> Round {
    let start = Instant::now();
    let sat = RT.block_on(async move {
        let mut set = JoinSet::new();
        for worker in 0..shape.workers {
            let p = actors.clone();
            set.spawn(async move {
                let mut sat = Vec::with_capacity(shape.ops);
                for seq in 0..shape.ops {
                    let t0 = Instant::now();
                    let (_checksum, send, queue) = p.call_probed(kind, seq + worker).await;
                    sat.push(Sat {
                        send,
                        queue,
                        total: t0.elapsed(),
                    });
                }
                sat
            });
        }
        let mut sat = Vec::with_capacity(shape.workers * shape.ops);
        while let Some(res) = set.join_next().await {
            sat.extend(res.expect("worker task never panics"));
        }
        sat
    });
    Round {
        sat,
        wall: start.elapsed(),
    }
}

/// The no-pool baseline: every reader owns its own connection, so there is no handoff, no
/// contention, and the connection never migrates between threads or cores.
fn run_local(kind: Work, shape: Shape, rounds: usize) -> Run {
    let mut per_sec = Vec::with_capacity(rounds);
    let mut lat: Vec<Duration> = Vec::with_capacity(rounds * shape.workers * shape.ops);

    for _ in 0..rounds {
        let gate = Arc::new(Barrier::new(shape.workers + 1));
        let mut handles = Vec::with_capacity(shape.workers);
        for worker in 0..shape.workers {
            let gate = gate.clone();
            handles.push(
                thread::Builder::new()
                    .name(format!("local-{worker}"))
                    .spawn(move || {
                        let conn = new_conn();
                        gate.wait();
                        let mut lat = Vec::with_capacity(shape.ops);
                        for seq in 0..shape.ops {
                            let t0 = Instant::now();
                            black_box(work(&conn, kind, seq + worker));
                            lat.push(t0.elapsed());
                        }
                        lat
                    })
                    .expect("spawn worker thread"),
            );
        }
        gate.wait();
        let start = Instant::now();
        let mut done = Vec::with_capacity(shape.workers * shape.ops);
        for h in handles {
            done.extend(h.join().expect("worker thread never panics"));
        }
        per_sec.push(ops_per_sec(shape.workers * shape.ops, start.elapsed()));
        lat.extend(done);
    }
    stats(per_sec, lat)
}

fn ops_per_sec(ops: usize, elapsed: Duration) -> f64 {
    let ops = u32::try_from(ops).expect("op count fits into u32");
    f64::from(ops) / elapsed.as_secs_f64()
}

/// Median throughput over the rounds, and the percentiles over every operation of every round.
fn stats(mut per_sec: Vec<f64>, mut lat: Vec<Duration>) -> Run {
    per_sec.sort_by(|a, b| a.partial_cmp(b).expect("throughput is never NaN"));
    lat.sort_unstable();
    Run {
        ops_per_sec: per_sec[per_sec.len() / 2],
        p50: pct(&lat, 50),
        p90: pct(&lat, 90),
        p99: pct(&lat, 99),
        max: lat[lat.len() - 1],
    }
}

fn pct(lat: &[Duration], p: usize) -> Duration {
    lat[(lat.len() * p / 100).min(lat.len() - 1)]
}

fn us(d: Duration) -> String {
    let mut s = String::with_capacity(16);
    write!(&mut s, "{:.1}us", d.as_secs_f64() * 1_000_000.0).expect("format a duration");
    s
}

fn fmt_k(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else {
        format!("{:.0}k", v / 1_000.0)
    }
}

// ---------------------------------------------------------------------------
// modes
// ---------------------------------------------------------------------------

/// The table the design decision is made on: today's path, the actors, and the async-free ceiling
/// of the same pool, at the production shape.
fn summary(_: &mut Criterion) {
    println!(
        "\n{CONNECTIONS} connections, {WORKERS} readers, {OPS} ops per reader, {SUMMARY_ROUNDS} rounds\n"
    );
    println!(
        "{:<48} {:>12} {:>10} {:>10} {:>10} {:>10}",
        "variant", "ops/s", "p50", "p90", "p99", "max"
    );
    for kind in [
        Work::Trivial,
        Work::QueryMap,
        Work::DirectMap,
        Work::OwnedThenMap,
        Work::DirectMapNarrow,
        Work::OwnedThenMapNarrow,
    ] {
        let rows = [
            (
                "async/deadpool (today)",
                run_async(
                    &Arc::new(Deadpool(Pool::from(conns_mutex(CONNECTIONS)))),
                    kind,
                    SHAPE,
                    SUMMARY_ROUNDS,
                ),
            ),
            (
                "async/actor (mutex handle)",
                run_actors(
                    &Arc::new(Actors::with_queue_flags(
                        CONNECTIONS,
                        OpenFlags::SQLITE_OPEN_READ_WRITE,
                        true,
                    )),
                    kind,
                    SHAPE,
                    SUMMARY_ROUNDS,
                ),
            ),
            (
                "async/actor (shipped)",
                run_actors(
                    &Arc::new(Actors::new(CONNECTIONS)),
                    kind,
                    SHAPE,
                    SUMMARY_ROUNDS,
                ),
            ),
            (
                "async/actor+stamp (hot path)",
                run_actors_stamp(
                    &Arc::new(Actors::new(CONNECTIONS)),
                    kind,
                    SHAPE,
                    SUMMARY_ROUNDS,
                ),
            ),
            (
                "async/actor+probe (probe)",
                run_actors_probe(
                    &Arc::new(Actors::new(CONNECTIONS)),
                    kind,
                    SHAPE,
                    SUMMARY_ROUNDS,
                ),
            ),
            (
                "sync/deadpool (ceiling)",
                run_sync(
                    &Arc::new(Deadpool(Pool::from(conns_mutex(CONNECTIONS)))),
                    kind,
                    SHAPE,
                    SUMMARY_ROUNDS,
                ),
            ),
        ];
        for (label, run) in rows {
            println!(
                "{:<48} {:>12.0} {:>10} {:>10} {:>10} {:>10}",
                format!("{label}/{}", kind.label()),
                run.ops_per_sec,
                us(run.p50),
                us(run.p90),
                us(run.p99),
                us(run.max),
            );
        }
        println!();
    }
}

/// The criterion matrix, so a change to the read path is tracked against its baseline.
fn bench(c: &mut Criterion) {
    for kind in [Work::DirectMap, Work::OwnedThenMap] {
        let pool: Arc<Deadpool> = Arc::new(Deadpool(Pool::from(conns_mutex(CONNECTIONS))));
        let actors = Arc::new(Actors::new(CONNECTIONS));

        let mut g = c.benchmark_group(format!("async/deadpool/{}", kind.label()));
        g.sample_size(SAMPLE_SIZE);
        g.measurement_time(MEASUREMENT_TIME);
        g.throughput(Throughput::Elements((SHAPE.workers * SHAPE.ops) as u64));
        g.bench_function("ops", |b| {
            b.iter(|| black_box(run_async(&pool, kind, SHAPE, 1).ops_per_sec));
        });
        g.finish();

        let mut g = c.benchmark_group(format!("async/actor/{}", kind.label()));
        g.sample_size(SAMPLE_SIZE);
        g.measurement_time(MEASUREMENT_TIME);
        g.throughput(Throughput::Elements((SHAPE.workers * SHAPE.ops) as u64));
        g.bench_function("ops", |b| {
            b.iter(|| black_box(run_actors(&actors, kind, SHAPE, 1).ops_per_sec));
        });
        g.finish();

        let mut g = c.benchmark_group(format!("async/actor-probe/{}", kind.label()));
        g.sample_size(SAMPLE_SIZE);
        g.measurement_time(MEASUREMENT_TIME);
        g.throughput(Throughput::Elements((SHAPE.workers * SHAPE.ops) as u64));
        g.bench_function("ops", |b| {
            b.iter(|| black_box(run_actors_probe(&actors, kind, SHAPE, 1).ops_per_sec));
        });
        g.finish();

        let mut g = c.benchmark_group(format!("sync/deadpool/{}", kind.label()));
        g.sample_size(SAMPLE_SIZE);
        g.measurement_time(MEASUREMENT_TIME);
        g.throughput(Throughput::Elements((SHAPE.workers * SHAPE.ops) as u64));
        g.bench_function("ops", |b| {
            b.iter(|| black_box(run_sync(&pool, kind, SHAPE, 1).ops_per_sec));
        });
        g.finish();
    }
}

/// The open flags of the read connections: what the mutex SQLite puts around every call on a
/// connection costs an actor thread, and what the read-only open costs.  Same actors, same queue,
/// same workload - only the flags of the handle the actor owns are different.
fn flags(c: &mut Criterion) {
    let shapes = [
        (
            "rw+mutex",
            OpenFlags::SQLITE_OPEN_READ_WRITE,
            false,
            "read-write handle, the mutex SQLite takes on every call",
        ),
        (
            "rw",
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            false,
            "read-write handle, no mutex",
        ),
        (
            "ro+mutex",
            OpenFlags::SQLITE_OPEN_READ_ONLY,
            true,
            "read-only handle, with the mutex",
        ),
        (
            "ro",
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            true,
            "read-only handle, no mutex",
        ),
    ];

    for kind in [Work::Trivial, Work::DirectMap, Work::OwnedThenMap] {
        for (label, flags, query_only, note) in shapes {
            let actors = Arc::new(Actors::with_queue_flags(CONNECTIONS, flags, query_only));
            let mut g = c.benchmark_group(format!("flags/{}/{}", kind.label(), label));
            g.sample_size(SAMPLE_SIZE);
            g.measurement_time(MEASUREMENT_TIME);
            g.throughput(Throughput::Elements((SHAPE.workers * SHAPE.ops) as u64));
            g.bench_function("ops", |b| {
                b.iter(|| black_box(run_actors(&actors, kind, SHAPE, 1).ops_per_sec));
            });
            g.finish();
            println!("flags/{}/{}: {note}", kind.label(), label);
        }
    }
}

/// The raw cost of the probe's own machinery: a single clock read, and the stamp pair a probe
/// registration is.  The actor's share of the probe is one `elapsed()` per dequeued request, so
/// the pair is the floor of what the hot path pays.
fn probes(c: &mut Criterion) {
    let mut g = c.benchmark_group("probe");
    g.sample_size(SAMPLE_SIZE);
    g.measurement_time(MEASUREMENT_TIME);
    g.bench_function("clock-read", |b| {
        b.iter(|| black_box(Instant::now()));
    });
    g.bench_function("stamp-pair", |b| {
        b.iter(|| {
            let stamp = Instant::now();
            black_box(stamp.elapsed().as_nanos());
        });
    });
    g.finish();
}

/// Connections x readers: the axis a node's `read_pool_size` moves on.  Readers above the
/// connection count is where a read path falls over, and that is true of every shape of it.
fn sweep() {
    println!(
        "\n{SWEEP_OPS} ops per round, {SWEEP_ROUNDS} rounds - cells are ops/s | p99, columns are \
         readers, rows are connections"
    );
    for kind in [Work::Query, Work::DirectMap] {
        sweep_table(
            "sync/crossbeam",
            kind,
            &SWEEP_CONNS,
            &SWEEP_WORKERS,
            &|conns, workers| {
                run_sync(
                    &Arc::new(crossbeam_pool(conns)),
                    kind,
                    sweep_shape(workers),
                    SWEEP_ROUNDS,
                )
            },
        );
        sweep_table(
            "async/actor",
            kind,
            &SWEEP_CONNS,
            &SWEEP_WORKERS,
            &|conns, workers| {
                run_actors(
                    &Arc::new(Actors::new(conns)),
                    kind,
                    sweep_shape(workers),
                    SWEEP_ROUNDS,
                )
            },
        );
    }
    sweep_row("local (no pool)", Work::Query, &SWEEP_WORKERS, &|workers| {
        run_local(Work::Query, sweep_shape(workers), SWEEP_ROUNDS)
    });
}

/// One row over the reader counts, for a shape that has no connection-count axis.
fn sweep_row(name: &str, kind: Work, workers: &[usize], run: &dyn Fn(usize) -> Run) {
    println!(
        "\n{name} / {} - every reader owns its connection",
        kind.label()
    );
    let mut header = String::new();
    for &workers in workers {
        write!(&mut header, " {workers:>12}").expect("format a header cell");
    }
    println!("{:>7}{header}", "conns");
    let mut row = String::new();
    for &workers in workers {
        let run = run(workers);
        write!(
            &mut row,
            " {:>12}",
            format!("{}|{}", fmt_k(run.ops_per_sec), us(run.p99))
        )
        .expect("format a cell");
    }
    println!("{:>7}{row}", "own");
}

fn sweep_table(
    name: &str,
    kind: Work,
    conns: &[usize],
    workers: &[usize],
    run: &dyn Fn(usize, usize) -> Run,
) {
    println!("\n{name} / {}", kind.label());
    let mut header = String::new();
    for &workers in workers {
        write!(&mut header, " {workers:>12}").expect("format a header cell");
    }
    println!("{:>7}{header}", "conns");
    for &conns in conns {
        let mut row = String::new();
        for &workers in workers {
            let run = run(conns, workers);
            write!(
                &mut row,
                " {:>12}",
                format!("{}|{}", fmt_k(run.ops_per_sec), us(run.p99))
            )
            .expect("format a cell");
        }
        println!("{conns:>7}{row}");
    }
}

/// A sweep cell: the reader count, with the round's total work split over it.
fn sweep_shape(workers: usize) -> Shape {
    Shape {
        workers,
        ops: (SWEEP_OPS / workers).max(1),
    }
}

// ---------------------------------------------------------------------------
// saturation
// ---------------------------------------------------------------------------

/// The two times a saturation probe can mean, measured side by side: the block a caller's stamp
/// around the send sees, which is only the time the queue had no room for the payload, and the
/// queue wait the actor's stamp sees, which is the time the query waited for a free connection.
fn saturation() {
    println!(
        "\n{CONNECTIONS} connections, queue {} deep, {SWEEP_OPS} ops per round split over the \
         readers, {SAT_ROUNDS} rounds - the first round warms the caches and is not measured",
        CONNECTIONS * QUEUE_PER_ACTOR,
    );
    for kind in [Work::Query, Work::DirectMap] {
        println!("\nasync/actor+probe / {}", kind.label());
        println!(
            "{:>8} {:>28} {:>28} {:>28} {:>13} {:>13}",
            "readers",
            "send p50|p99|max",
            "queue p50|p99|max",
            "total p50|p99|max",
            ">100ms send",
            ">100ms queue"
        );
        for &workers in &SAT_WORKERS {
            let shape = sweep_shape(workers);
            let actors = Arc::new(Actors::new(CONNECTIONS));
            run_saturated(&actors, kind, shape);
            let round = run_saturated(&actors, kind, shape);
            let sat = &round.sat;

            let send = times(sat, |s| s.send);
            let queue = times(sat, |s| s.queue);
            let total = times(sat, |s| s.total);
            println!(
                "{:>8} {:>28} {:>28} {:>28} {:>12.1}% {:>12.1}%",
                workers,
                sat_cell(&send),
                sat_cell(&queue),
                sat_cell(&total),
                share(&send) * 100.0,
                share(&queue) * 100.0,
            );
        }
    }
}

/// The queue depth axis for one shape: `conns` connections, `workers` readers, every depth.  The
/// shape matters here more than anywhere else in this bench: a deep queue decouples an actor from
/// the wake of a parked sender, and that decoupling is worth most when the threads of the run
/// outnumber the cores it lands on.
fn queue_depth(conns: usize, workers: usize) {
    let shape = sweep_shape(workers);
    println!(
        "\n{conns} connections, {workers} readers, {} ops per round, {SAT_ROUNDS} rounds - the \
         first round warms the caches and is not measured",
        shape.workers * shape.ops
    );
    for kind in [Work::Query, Work::DirectMap] {
        println!("\nasync/actor+probe / {}", kind.label());
        println!(
            "{:>11} {:>12} {:>28} {:>28} {:>13}",
            "conns/queue", "ops/s", "total p50|p99|max", "queue p50|p99|max", ">100ms queue"
        );
        for &queue in &QUEUE_DEPTHS {
            let actors = Arc::new(Actors::with_queue(conns, queue));
            run_saturated(&actors, kind, shape);
            let round = run_saturated(&actors, kind, shape);
            let sat = &round.sat;
            let total = times(sat, |s| s.total);
            let waited = times(sat, |s| s.queue);
            println!(
                "{:>11} {:>7}k {:>28} {:>28} {:>12.1}%",
                format!("{conns}/{queue}"),
                fmt_k(ops_per_sec(shape.workers * shape.ops, round.wall)),
                sat_cell(&total),
                sat_cell(&waited),
                share(&waited) * 100.0,
            );
        }
    }
}

/// The depth axis over the reader counts, at the production connection count.
fn queue_sweep() {
    for &workers in &SAT_WORKERS {
        queue_depth(CONNECTIONS, workers);
    }
}

/// One measured time of a round, sorted.
fn times(sat: &[Sat], f: impl Fn(&Sat) -> Duration) -> Vec<Duration> {
    let mut lat: Vec<Duration> = sat.iter().map(f).collect();
    lat.sort_unstable();
    lat
}

/// `p50|p99|max` of a sorted latency vector.
fn sat_cell(lat: &[Duration]) -> String {
    format!(
        "{}|{}|{}",
        us(pct(lat, 50)),
        us(pct(lat, 99)),
        us(lat[lat.len() - 1])
    )
}

/// The share of a sorted latency vector above the warning threshold.
fn share(lat: &[Duration]) -> f64 {
    let over = lat.iter().filter(|d| **d > SAT_THRESHOLD).count();
    f64::from(u32::try_from(over).expect("count fits into u32"))
        / f64::from(u32::try_from(lat.len()).expect("count fits into u32"))
}

// ---------------------------------------------------------------------------
// memory
// ---------------------------------------------------------------------------

/// Resident bytes of the read side of a node, one configuration per process: SQLite does not
/// return freed pages to the OS, so a second configuration measured in the same process inherits
/// the first one's memory.
fn measure_memory(spec: &str) {
    println!("\nresident bytes of this process, {spec}");
    println!("  baseline: {}", rss());
    let path = &*DB_PATH;

    if spec == "connections" {
        let mut pool: Vec<Connection> = Vec::new();
        for n in [1usize, 2, 4, 8, 16, 32, 64] {
            for _ in pool.len()..n {
                let conn = new_conn();
                warm(&conn);
                pool.push(conn);
            }
            println!("  {:>5} connections, range scan: {}", n, rss());
        }
        return;
    }

    if let Some(count) = spec.strip_prefix("stmts:") {
        let count: usize = count.parse().expect("statement count is a number");
        let mut pool: Vec<Connection> = Vec::new();
        for _ in 0..16 {
            let conn = new_conn();
            fill_stmt_cache(&conn, count);
            pool.push(conn);
        }
        println!("  16 connections, {count} cached statements: {}", rss());
        return;
    }

    if let Some(count) = spec.strip_prefix("pool:") {
        let count: usize = count.parse().expect("connection count is a number");
        raise_fd_limit();
        let mut pool: Vec<Connection> = Vec::new();
        let mut opened = 0usize;
        let mut first_error = String::new();
        for _ in 0..count {
            match Connection::open(path) {
                Ok(conn) => {
                    conn.pragma_update(None, "query_only", true).ok();
                    warm_n(&conn, 5);
                    pool.push(conn);
                    opened += 1;
                }
                Err(e) => {
                    if first_error.is_empty() {
                        first_error = e.to_string();
                    }
                }
            }
        }
        println!(
            "  {opened:>5} of {count} connections opened, range scan each: {} - {first_error}",
            rss()
        );
    }
}

/// Prepares `count` distinct statements on a connection, which is what fills its prepared
/// statement cache.  `prepare_cached` is the entry point that caches.
fn fill_stmt_cache(conn: &Connection, count: usize) {
    for i in 0..count {
        let sql = format!("SELECT {i} /* pad */");
        let mut stmt = conn.prepare_cached(&sql).expect("prepare cached statement");
        let mut rows = stmt.query([]).expect("run statement");
        while let Some(row) = rows.next().expect("read row") {
            black_box(row.get::<_, i64>(0).expect("read value"));
        }
    }
}

/// Runs the query workload on a connection so its page cache and statement cache are warm.
fn warm(conn: &Connection) {
    warm_n(conn, 200);
}

/// The same, for `n` operations.  A 10k-connection run needs a much lighter warm-up.
fn warm_n(conn: &Connection, n: u64) {
    for seq in 0..n {
        let max = ROWS as u64 - ROW_SPAN as u64;
        let id = i64::try_from(seq % max).expect("row id fits") + 1;
        let mut stmt = conn
            .prepare_cached(QUERY)
            .expect("prepare cached statement");
        let mut rows = stmt.query(rusqlite::params![id]).expect("run query");
        while let Some(row) = rows.next().expect("read row") {
            black_box(row.get::<_, i64>(0).expect("read id"));
        }
    }
}

// ---------------------------------------------------------------------------
// connections
// ---------------------------------------------------------------------------

/// Opens a read connection on the shared file, the way a node opens its read connections now:
/// a read-only handle with no mutex, the shape the read actors own.
fn new_conn() -> Connection {
    new_conn_with(
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        true,
    )
}

/// The shape the old pool ran: a read-write handle with the SQLite mutex on, plus `query_only` as
/// its only write guard.  The baseline of every number here has to be this, because it is what
/// users run today.
fn new_conn_mutex() -> Connection {
    new_conn_with(OpenFlags::SQLITE_OPEN_READ_WRITE, true)
}

/// A read connection at one shape of the open flags, with the read pragmas of a node on it.
///
/// The seed connection already put the file into WAL, so a read connection never sets the journal
/// mode - the same split a node runs, where the pragmas that change the file belong to the writer.
fn new_conn_with(flags: OpenFlags, query_only: bool) -> Connection {
    let conn = Connection::open_with_flags(&*DB_PATH, flags).expect("open read connection");
    conn.pragma_update(None, "synchronous", "OFF")
        .expect("synchronous");
    if query_only {
        conn.pragma_update(None, "query_only", true)
            .expect("query_only");
    }
    conn.set_prepared_statement_cache_capacity(1024);
    conn
}

fn conns(count: usize) -> Vec<Connection> {
    (0..count).map(|_| new_conn()).collect()
}

fn conns_mutex(count: usize) -> Vec<Connection> {
    (0..count).map(|_| new_conn_mutex()).collect()
}

fn conns_with(count: usize, flags: OpenFlags, query_only: bool) -> Vec<Connection> {
    (0..count)
        .map(|_| new_conn_with(flags, query_only))
        .collect()
}

fn crossbeam_pool(count: usize) -> Crossbeam {
    let (tx, rx) = crossbeam::channel::unbounded();
    for conn in conns_mutex(count) {
        tx.send(Chan(conn)).expect("fresh channel");
    }
    Crossbeam { tx, rx }
}

/// Seeds the file database with the write-path pragmas of a node.
fn seed_file(path: &Path) -> u64 {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    let seed = Connection::open(path).expect("open seed connection");
    seed.pragma_update(None, "journal_mode", "WAL")
        .expect("journal_mode");
    seed.pragma_update(None, "page_size", 4096)
        .expect("page_size");
    seed.pragma_update(None, "synchronous", "OFF")
        .expect("synchronous");
    seed.execute_batch(&format!(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, payload BLOB NOT NULL);
         INSERT INTO t (id, name, payload)
         WITH RECURSIVE s(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM s WHERE i < {ROWS})
         SELECT i, 'name-' || i, zeroblob({BLOB}) FROM s;"
    ))
    .expect("seed the file database");
    drop(seed);
    std::fs::metadata(path).expect("stat the database").len()
}

// ---------------------------------------------------------------------------
// process
// ---------------------------------------------------------------------------

/// Raises the open-file limit: every connection holds its own descriptor on the database, its
/// `-wal` and its `-shm` file, and the default soft limit is a few hundred.
fn raise_fd_limit() {
    let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
    // SAFETY: `limit` is a zeroed `rlimit` of the documented layout, and `getrlimit` only writes
    // it for `RLIMIT_NOFILE` of this process.
    unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) };
    let want = limit.rlim_max.min(100_000);
    limit.rlim_cur = want;
    // SAFETY: `limit` was read above and only `rlim_cur` is changed, never above `rlim_max`.
    let ok = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const limit) };
    println!("  open-file limit raised to {want} (setrlimit {ok})");
}

/// Resident bytes, from the resident-page count of this process.
///
/// `libc` deprecates `mach_task_self` in favour of the `mach2` crate; a bench probe is not a
/// reason to add a dependency, so the deprecation is accepted here and nowhere else.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[allow(
    deprecated,
    reason = "libc wants a mach2 dependency for the mach task port of this process"
)]
fn rss() -> usize {
    let mut info: libc::mach_task_basic_info = unsafe { std::mem::zeroed() };
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    // SAFETY: `info` is a zeroed `mach_task_basic_info` and `count` is its `natural_t` word
    // count, which is what `task_info` reads and writes for this flavor; the task port is this
    // process's own.
    let ok = unsafe {
        libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            (&raw mut info).cast::<libc::integer_t>(),
            &raw mut count,
        )
    };
    // A wrong flavor answers `KERN_FAILURE` and leaves the struct zeroed, which reads as a
    // resident size of zero, so a failed probe must never look like a measurement.
    assert_eq!(ok, 0, "task_info(MACH_TASK_BASIC_INFO) failed with {ok}");
    usize::try_from(info.resident_size).expect("resident size is never negative")
}

/// Current resident size of this process, in bytes, from the resident-page count in `statm`.
#[cfg(target_os = "linux")]
fn rss() -> usize {
    let statm = std::fs::read_to_string("/proc/self/statm").expect("read /proc/self/statm");
    let pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .expect("resident pages")
        .parse()
        .expect("resident pages are a number");
    usize::try_from(pages.saturating_mul(4096)).expect("resident size fits")
}

criterion_group!(matrix, summary, bench, probes, flags);

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Some(spec) = args.iter().find_map(|a| a.strip_prefix("--memory=")) {
        measure_memory(spec);
    } else if args.iter().any(|a| a == "--summary") {
        summary(&mut Criterion::default());
    } else if args.iter().any(|a| a == "--sweep") {
        sweep();
    } else if args.iter().any(|a| a == "--flags") {
        flags(&mut Criterion::default());
    } else if args.iter().any(|a| a == "--saturation") {
        saturation();
    } else if let Some(spec) = args.iter().find_map(|a| a.strip_prefix("--queue=")) {
        let (conns, workers) = spec.split_once(':').expect("--queue=CONNS:WORKERS");
        queue_depth(
            conns.parse().expect("connection count is a number"),
            workers.parse().expect("reader count is a number"),
        );
    } else if args.iter().any(|a| a == "--queue") {
        queue_sweep();
    } else {
        matrix();
    }
}
