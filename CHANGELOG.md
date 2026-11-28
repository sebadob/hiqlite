# Changelog

## UNRELEASED

`openraft` was updated from 0.9 to 0.10.

Persisted data is unchanged: the WAL logs and WAL metadata, the SQLite state machine's metadata and
the snapshot files load as they are. Both directions are pinned by tests, so an existing data dir
can simply be kept.

For `LogSync::IntervalMillis(_)` and `LogSync::ImmediateAsync`, you can expect an increased
throughput performance of ~10%. When using `LogSync::Immediate`, I was able to get up to 350%
more throughput in some situations. This was very specific to the benchmark setup though, where
you have a few writers that fire many requests. It will not be that much when you have lots of
writers with a single request each.

### Breaking

#### `v0.15` Migration Guard

The `v0_15_auto_cache_migration` module is gone. It only ever cleaned up the cache data of a v0.14
data dir, its compile guard trips as soon as the version moves on, and the `HQL_CACHE_WAL_*`
migration vars are not evaluated anymore.

**CAUTION: This means you MUST NOT try to upgrade to this version from < v0.15 directly! It would
break your cluster. You MUST start at least any 0.15 or 0.16 cleanly BEFORE upgrading!**

#### Raft Network

Raft _network_ messages are not compatible anymore. **A rolling upgrade is not possible**:

1. stop the whole cluster
2. update all nodes at once
3. start them again

> There is a new guard in place that will refuse a restart if online cluster nodes with an older
> version are reachable. This is only a helper. You need to take care of it yourself.

#### `backup` Feature

the `backup` feature does not pull in `s3` anymore. Backups have never needed S3 - the cron job, the
vacuum into the local `backups` dir, the local cleanup and a `file:` restore are plain disk work -
and the encryption keys, `cryptr` and `s3-simple` were the price for all of it.

`backup` is now `cron` + `sqlite`, and the push to a bucket, the bucket cleanup and the `s3:`
restore source live behind `s3`, which keeps implying `backup`. A default-features build has local
backups and no S3 client in it. Add `s3` where you had backups before and want them offsite.

#### API Secret

The API secret travels in the standard `Authorization` header now, as a bearer token -
`Authorization: Bearer <secret_api>` - and not in the custom `X-API-SECRET` anymore. `Authorization`
is the header that loggers, proxies and client libraries treat as sensitive by default, which is
exactly what a secret needs. The value has to carry the `Bearer ` scheme, matched
case-insensitively, and the secret behind it is compared byte for byte in constant time. A raw
secret without the scheme is refused with an answer that says what to send, and the old header
authenticates nothing. `Client` code is unchanged: the HTTP client builds the header itself. Tooling
that talks to the API by hand needs `-H "Authorization: Bearer <secret_api>"`, and old and new nodes
cannot authenticate each other, which is the same stop-the-world upgrade the openraft 0.10 wire
change forces anyway.

#### rustls Crypto Backend

TLS now has a selectable rustls crypto backend: the new `aws_lc_rs` and `ring` features, with
`aws_lc_rs` the default. Exactly one must be selected whenever TLS is compiled in, and selecting both,
or TLS with neither, is a compile error. Whichever you pick, the other backend's crates are never
compiled in. `fips` is the FIPS-validated aws-lc-rs build; it implies `aws_lc_rs` and excludes `ring`.

This is breaking for builds that disable default features and enable a TLS feature (`sqlite`, `cache`,
`backup`, `dashboard`, `server`, `full`): they must now name `aws_lc_rs` or `ring`. Default-features
builds are unchanged. The `s3` feature pulls `cryptr` 0.13, whose provider follows the one you pick.

`aws_lc_rs` prefers the post-quantum hybrid group X25519MLKEM768 in TLS 1.3 key exchange. `ring` has
no post-quantum group and negotiates X25519.

#### WAL Header v2 and the Startup Replay Gate

The WAL header grows a fixed slot that carries the Raft `committed` log id, and the header version
goes to 2. Existing files keep their version 1 header and are read and written exactly as before;
only files created from now on are version 2, so a log directory mixes versions freely.

**CAUTION: Downgrading to 0.16 is not supported.** 0.16 reads version 1 headers only, so it skips
every file this version rolled over to version 2, with a warning, and comes back with its log behind
its own state machine. In a cluster, the way back is an empty log directory on every node and a
resync from the leader.

The pointer is what lets a node replay its own committed log at startup without talking to anybody.
openraft reads it out of the log store and re-applies `(last_applied, committed]` before the node
binds its router: no leader, no peers, no engine, and no open API port while it runs, on every node.
A node whose state machine was wiped by a crash - or by `auto-heal` - is back on its own persisted
`committed` state before it serves a single request, in a cluster as well as on a single node.
`wait_for_cluster_sync()` waits for that replay first, on every topology, and only then for the
cluster tip.

Publishing the pointer costs no extra write: the WAL writer copies it into the header of the file it
is flushing anyway, so `save_committed` is an in-memory assignment and never an fsync. Reading it
back clamps it to the durable log tail, because the WAL acknowledges appends before flushing them
and a crash can leave the tail behind the committed position; an unclamped pointer would make openraft
re-apply a range that is not in the log and the node could not start at all.

**CAUTION: A node now refuses to start when a WAL file it cannot read sits behind the newest one.**
It used to log a warning and start without that file. With `max_in_snapshot_log_to_keep` at 1, the
newest file often holds the whole log tail after the last snapshot, so starting without a log file
means serving reads from a state machine that is behind its own committed log. The newest file is the
one exception `auto-heal` handles: a roll-over creates it and writes its header before anything is
appended to it, so a process dying inside that window leaves a headerless file that holds no entries
at all, and `auto-heal` re-creates it empty and keeps the rest of the log. Every older file was
sealed and fsynced before a newer one ever existed, so an unreadable file behind them is a real hole
in the log - restore it from a backup, or start the node with an empty log directory. The error names
the file.

The deep integrity check on an unclean start now scans every WAL file, not just the newest one, and
rolls the log back behind the first corrupt record, dropping the files after it. A startup replay
re-applies `(last_applied, committed]`, and that read fails hard on a bad record, so the range a
replay can ask for has to be checked and repaired: the log ends up a valid prefix, the pointer
clamps to the new tail, and the dropped entries are re-synced from the leader.

Only a rollback drops files. A sealed file whose header never reached its last record is repaired by
extending that header, and every newer file stays. A rollback that would take the log tail below the
purge frontier is refused before any file is deleted, so the log directory is left intact.

`wal_size` goes to 4 MiB. A single log entry has to fit into one WAL file on its own, so the size
caps the largest statement a node can accept, and the cap is now exact: an entry that does not fit
fails with `WalSizeExceeded` when the `oversized-entry-error` feature is on, and panics the writer
when it is off. Files written before the increase keep their size; the new one applies from the next
roll-over.

The upgrade itself is safe: the `data_09` fixture, a real data dir written by the last release on
openraft 0.9, starts and serves all its data, snapshots included.

#### Caches Are Identified by Name

The cache group's log entries and snapshots identify a cache by its `CacheVariants` variant name
instead of its position in the enum. The derive builds the set of names at compile time as a perfect
hash (`phf`), and each node finds a cache handler by its name in O(1). There is no index anymore, so
the enum is misuse resistant:

- Reordering variants, or inserting one in the middle, is safe now.
- A node whose enum lacks a variant, during a rolling deploy that adds one, keeps that cache's
  entries without a variant and carries them in its snapshots, instead of panicking in `apply()`.
  A leader on the old enum refuses writes to the new cache with an `unknown cache` error.
- Installing a snapshot empties a local cache the snapshot does not carry.

`cache_index.meta` and its startup check are gone, and so is `HQL_CACHE_META_RESET`. A rename is a
new, empty cache, and the old name's entries stay until they expire or a `clear_cache_all()`.

The cache log and snapshot format changed, so the existing cache data cannot be read anymore. On
the first start, a node that finds a `cache_index.meta` drops its cache log and cache snapshots
once, and every node starts with an empty cache. Your database is untouched. With the
`in-memory-snapshots` feature and `cache_storage_disk = true`, there is no `cache_index.meta`:
delete `logs_cache` and `state_machine_cache` in the data dir by hand before the first start.

`CacheVariants` is now `hiqlite_cache_name()` and `hiqlite_cache_names()`; `hiqlite_cache_index()`,
`hiqlite_cache_variants()` and the `cache_index.meta` helpers are gone. Only a hand-written impl
needs a change.

#### Errors

Source breaking: the `Error` enum's openraft variants are split per group (`RaftErrorDb` /
`RaftErrorKv` and friends), and the metrics functions return `RaftMetrics<TypeConfigSqlite>` /
`RaftMetrics<TypeConfigCache>` instead of `RaftMetrics<u64, Node>`.

Source breaking: the `Error` enum has a new `Unavailable` variant, which answers
`503 Service Unavailable`. A `match` over `Error` that has no wildcard arm needs one.

#### `query_consistent()`

`query_consistent()` and `query_consistent_map()` take the `ReadPolicy` as an argument, and
`hiqlite::ReadPolicy` is a re-export of openraft's enum so an app can name it. `ReadIndex` is the
strong path, `LeaseRead` lets a leader answer on its own lease and is the faster one.

#### Owned Rows

A result set that travels through the network, or into the dashboard, keeps its column names once
per result set now instead of once per row, and the values of a row sit in that column order.
`Row::Owned` carries the column index of the result set it belongs to, and `ColumnOwned` is gone:
naming a column was the job of the result set, never of a single row.

A value is still taken out of the row when you read it, so a column can only be read once, and a
second read of the same column is the same `Error::QueryParams` it always was.

Public API changes:

1. `Row::Owned(row)` stays a single-field variant, and its field is the new `RowOwned`: the values
   of the row together with the column index of its result set, shared through an `Arc`.
2. The old `RowOwned` struct is `RowValues` now: the values of one row in column order, with no
   accessors of its own. Its `get()` and `try_get()` are not public API anymore; a row is read
   through `Row::get()` / `Row::try_get()`, which did not change, or through the new `RowOwned`'s
   own `get()` / `try_get()` / `columns()`.
3. `ColumnOwned` is removed. The column info of a prepared statement is `ColumnInfo`.

Migration:

- Apps that use `query_map()`, `query_try_map()` and the `*_as()` calls with derived `FromRow` /
  `TryFromRow` structs need **no change at all**. `Row::get("name")` and `Row::try_get("name")`
  take the column name exactly like before, on borrowed rows and on owned rows.
- A `match` on `Row::Owned(row)` is unchanged; the `row` it binds is a `RowOwned` now, which is the
  readable row, not the bare values it used to bind.
- Code that kept the old `RowOwned` struct around and read it directly reads `Row::Owned` instead.
  The name `RowOwned` belongs to the readable row from now on, never to its bare values.

### Changes

#### Pipelined Raft Replication

A leader no longer waits for a follower's answer before it sends the next append. Appends to a
follower travel as a pipeline, and the follower answers them in the order it received them, so a
write's round trip is covered by the writes behind it instead of by nothing.

- Writes on a link with distance get their throughput back. The round trip to a far voter is no
  longer paid once per write, and a write that arrives while its leader is already replicating does
  not queue behind that round trip. This is the change that makes the far-node shape of
  `geo-replication.md` faster, and it is measurable on a plain LAN as well.
- Nothing to configure. Replication behaviour is unchanged for a single writer that waits for its
  own ack, and for every read.
- Snapshot installs, votes and leadership transfers are untouched: they keep travelling one at a
  time, in order, on the same connection as the appends.
- A follower that cannot answer closes its connection, and the leader re-syncs to it like it
  already did on a dead append. No write is acknowledged by a pipeline that stopped answering.
- An append is as big as the log entries the leader has queued, and the limit on them counts entries
  and never bytes. An append that is big travels as one message, is collected and decoded as one
  message on the follower, and every other message on that connection queues up behind it. A node
  logs a warning when an append goes over 8 MB. Keep transactions small enough to stay out of that
  log.

The round trip this mechanism hides is the one to a distant voter. Measurements of the pipelined
transport against the previous one, on a machine built for running them, are still to come.

#### Local Read Threads

A local read does not wait for a pooled connection anymore. The node keeps one thread per read
connection for the whole life of the process, and a query goes to the thread that owns its
connection: the statement it prepares, the pages it reads and the cache it fills stay there, and
nothing is taken and handed back on every query. The `deadpool` dependency is gone from both the
`sqlite` and the `external-state-machine` feature.

- Every local read answers faster, the small ones most. The table is the whole chain in one
  `just bench-conn-pool -- --summary` run on an Apple M4 Pro (8 performance + 4 efficiency cores)
  inside the recipe's `taskpolicy -c utility` core envelope: 4 read connections, 8 concurrent
  readers, 2000 operations per reader over 4 rounds, an on-disk database of 20000 wide rows
  (4.6 MB), every query a range scan of 100 of them. Queries per second, and the median latency of
  one read.

    | the read                                     | pool today  | new, shipped |
    | -------------------------------------------- | ----------- | ------------ |
    | `query_map()`, the typed row from owned rows | 107k / 73µs | 162k / 48µs  |
    | the same row mapped from the borrowed row    | 127k / 62µs | 215k / 36µs  |
    | the wide row built through the owned form    | 90k / 87µs  | 124k / 61µs  |
    | a typed row, one integer column              | 186k / 42µs | 350k / 22µs  |
    | the same narrow row through the owned form   | 166k / 48µs | 299k / 25µs  |
    | a cached `SELECT 1`                          | 274k / 24µs | 538k / 13µs  |

    Against the pool as it ships today, the five real queries gain +37% to +88% in throughput and
    30% to 49% in median latency, and the cached `SELECT 1` doubles. Against the fastest shared
    connections can answer at all - the same queries through the same pool with its async handoff
    taken out - the new path runs at 75% to 110% of that ceiling on the five real queries, and at
    60% of it on the cached `SELECT 1`. That ceiling is a burst ceiling: it parks its waits outside
    the measured query and its own worst read is 0.4 ms to 2.9 ms, so it is a throughput ratio and
    not a latency one.

- A streaming read runs on one of these threads for as long as you consume it, so a node serves at
  most `read_pool_size` streams at once. Streams you never read starve the other local reads.
- The external state machine's restore closes the read pool and recreates it, and it waits up to 30s
  for the reads that are already running.
- A local read that waited longer than 100 ms for its connection is visible in the log now, and the
  line names which of the two problems it is: one query holding a connection for a long time, with
  the time it ran, or a pool with more reads than connections, with how many are queued. A pool
  that stays saturated warns once every 10 seconds, not once per query.
- `saturated_pool_threshold` (default `100ms`) is the queue wait that warns, `slow_query_threshold`
  (default `1s`) is the time a single read may run on its connection before the node logs it, naming
  the statement and never its parameters. They are independent, so a `slow_query_threshold` above
  `saturated_pool_threshold` still gives you two distinct warnings. A stream is measured from its
  first row to its drop, the pauses of its consumer included.
- `log_slow_queries` (default `true`) decides whether the slow-query line names the statement. A
  query typed into the dashboard has its values inline in its text, so a slow dashboard query writes
  them into the log. Set this to `false` when you do not want statement text in your log at all. The
  saturation warning never names a statement, so it is unaffected.

#### Elections Are Requests, Not Commands

A campaign is openraft's Pre-Vote round and nothing else: the term is untouched when the node cannot
win, a live leader holding its lease is refused, and a node whose own elections are disabled is
refused by its own gate. There is no way to bypass that round, and that is deliberate: a campaign
past the gate needs only its own vote plus one peer's grant to reach a quorum of three, so it can
install a leader the operator has declared must never lead, and nothing in that cluster moves it off
the seat afterwards.

A leader change that must happen is a graceful one. `Client::transfer_leadership(node_id)` asks the
node holding the seat to hand it over, `Client::elect_node(node_id)` is the probe a healthy cluster
may refuse, and a graceful shutdown hands the seat over on its own.

#### Streaming Queries

`Client::query_map_stream()`, `Client::query_try_map_stream()` and `Client::query_raw_stream()` are
the non-collecting shapes of `query_map()`, `query_try_map()` and `query_raw()`: they hand back a
`hiqlite::QueryStream` whose rows arrive as you consume them, so a very big result costs the memory
of a single row at a time, and an API response can start writing while the query is still running.

- `stream.next().await?` is the next row, `None` after the last one
- `stream.into_json_response()` is an API response that answers with one JSON object per line while
  its query is still running: a mapped type needs `serde::Serialize`, a raw row writes itself as an
  object keyed by the column names of the query, and `hiqlite::JsonLinesExt` is the same method for
  a combinator chain over a stream
- `stream.into_json_body()` is that body on its own, for a status code and headers of your own
  around the rows
- a `QueryStream` is a futures `Stream`: `hiqlite::StreamExt` is re-exported, so `.map()`,
  `.take()` and the rest work on it without a `futures-util` dependency of your own, and a
  conversion you apply in flight stays lazy - a `take(n)` never reads the rows behind it
- `query_raw_stream()` hands over `hiqlite::RowOwned` rows, read by column name, and `row.columns()`
  on any of them is the header of its result set, shared through the `Arc` of the row
- dropping a stream stops its query and returns its connection to the read pool, while a stream you
  hold on to without consuming it parks that connection out of the pool for as long as you hold it
- a statement that cannot be built, and a param that cannot be bound, fail the call itself, exactly
  like they fail `query_map()`, and a query that fails while you consume it is the `Err` of
  `next()`, so a truncated result can never read as a complete one
- all three are local-client calls, a remote client answers `Error::Config`, and there is no
  streaming consistent query on purpose: one pauses Raft replication, and that pause has to end as
  fast as the query that needs it
- `examples/query-streaming` is the whole thing on a real axum API: a single node with 2.000 seeded
  rows, endpoints whose JSON lines you can watch arrive with `curl -N`, and a page on `/` that reads
  the very same body in JavaScript and fills a table row by row

#### Row Name Lookup

Resolving a column name of a row was a linear scan through the result set, and SQLite was asked
for the name of every candidate column: a struct with N fields over N columns did N * (N + 1) / 2
of those calls, for every single row. Every query now builds the name-to-index map of its result
set once, which turns each later lookup into a single hash probe.

`cargo bench -p hiqlite --features sqlite,macros` maps the same 2000 rows into the same struct and
only changes how the columns are read (release build, Apple silicon, medians):

- 8 columns: `query_map()` went from 691 µs to 398 µs, a **1.7x** speedup
- 24 columns: from 3.88 ms to 1.19 ms, a **3.3x** speedup

`query_map()` did not catch up to `query_as()`, it passed it. The same 2000 rows through
`serde_rusqlite`, the positional read `query_as()` uses, take 468 µs at 8 columns and 1.37 ms at
24, so the name-based path is about 12 - 18% faster on the narrow and 14 - 15% on the wide result
set. `row.get(idx)`, rusqlite's direct positional read, is the floor of this work at 374 µs and
1.06 ms, and the new name lookup is within 6 - 12% of it.

The map is built once per query, so a single-row query pays that build instead: 0.29 µs for 8
columns and 0.79 µs for 24. Everything that maps more than a handful of rows is ahead, and wide
result sets gain the most, because the old scan grew with the square of the column count.

Owned rows, what a remote `query_map()` and the `execute_returning()` calls hand back, gained on
top of this: with the names on the result set instead of on every row (see `Owned Rows`), building
2000 rows over 24 columns got about 38% cheaper, and reading them by name dropped from 1.26 ms to
0.24 ms, a **5.3x** speedup. Those numbers are from the same kind of harness as above, but owned
rows are built by hand there, so they are not the numbers of the bench file.

#### Compile-time SQL Checking with `sql!()`

`sql!()` checks a query string at compile time and hands it back as the string itself, so it takes
the place of a plain string literal at any call site that takes a query:

```rust
let rows = client.query_map(sql!("SELECT id, ts FROM test"), params!()).await?;
```

The string is prepared against an in-memory SQLite built from the `migrations` directory that sits
next to the `Cargo.toml` of the crate using the macro - the same directory `Client::migrate()`
applies - so a typo in a table name, a column that does not exist, or a broken keyword fails the
build with SQLite's own message, pointing at the string:

```rust,notest
error: no such column: descrption in SELECT descrption FROM test at offset 7
  --> src/db.rs:41:25
   |
41 |     let rows = client.query_map(sql!("SELECT id, descrption FROM test"), params!()).await?;
   |                               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

- The macro is a validator only: it emits the literal it was given, byte for byte. Nothing to
  adapt, no runtime cost, and no new dependencies.
- `sql!()` takes exactly one statement, the shape of `execute()`, `query()` and all their variants.
  `sql_batch!()` is the multi-statement shape for `batch()` and `txn()`.
- `sql_map!(Type, "…")` checks the columns the query returns against the fields of the type it
  maps to: the same names, in any order and any case, which is the matching a mapped row does. A
  query that reads a column the type has no field for, or a field the query does not return, fails
  the build. The `FromRow` derive publishes the shape of every type it maps, including the parts it
  flattens; a `TryFromRow` type and a hand-written mapping publish it with the `SqlShape` derive,
  or by listing their column names in a `SqlShape` impl.
- One in-memory database per schema and compiler process, and every verdict is cached in
  `.hiqlite-sql/` in the workspace root, keyed by a fingerprint of your migrations and a hash of the
  query string. A query the compiler has seen before costs a hash lookup, so hundreds of `sql!()`
  calls in one repo do not build hundreds of databases. The directory is gitignored and you can
  delete it whenever you want.
- Editing a migration file recompiles the crates that use `sql!()` and changes the fingerprint, so
  the cache invalidates itself; the version of SQLite Hiqlite checks against is part of it too.
  Adding a migration file is not something a compiler can see from the outside, so a new file does
  not recompile anything by itself: after you add one, rebuild the crates that check against it
  (`cargo build` after a `touch`, or a clean build of them) to fold it into their schema and get
  the fresh verdicts.
- A crate without a `migrations` directory, and any failure inside the checker itself, produces one
  `warning:` line and leaves queries unchecked. The check can never break a build.
- The checks run in debug builds, and a shape exists exactly where a check can run: the `FromRow`
  and `SqlShape` derives emit their shape impl in debug builds, and in a release build only when
  `sql-check-release` is on.
- Not checked: the number of bound parameters (that is `params!()`, a separate call the macro
  cannot see), the types of the columns you map (`query_try_map()` reports those as an error at
  runtime), and queries built at runtime with `format!`.

The `sqlite` and `macros` features enable it, which is what a `FromRow` derive needs anyway. The
proc-macro crate builds the bundled SQLite once per target (a C compile at build-script time, not
in your runtime), and each `rustc` process pays one in-memory database plus one migration replay,
a few milliseconds, before every check after that is a hash lookup.

#### Hoisted Field Mapping

The name lookup above is per row and per field. Mapping a type that derives `FromRow` /
`TryFromRow` hoists it out of the row: the field names are resolved against the result set once,
before the first row, and every row reads its columns by index only.

```rust, notest
#[derive(Debug, FromRow)]
struct MyStruct {
    pub id: i64,
    pub ts: i64,
}

// no new call and no changed call: the hoisted path is picked from the mapped type
let rows: Vec<MyStruct> = client.query_map("SELECT ts, id FROM test", params!()).await?;
```

`query_map()`, `query_try_map()`, their `_one` and `_optional` variants and `query_consistent_map()`
are unchanged, and each of them picks its mapping from the type it maps. The derive registers its
type in a per-process registry of hoisted mappings, from the body of the `From` / `TryFrom` impl it
emits, and a query of a registered type takes the hoisted path. A type with only a hand-written
`impl From<&mut Row>` is never registered, so it is mapped by the names its own body reads, exactly
like it always was. One call handles both, and no existing `From<&mut Row>` impl needs any change.

Registration is emitted for the types whose hoisted mapping needs no bounds beyond their own: a flat
type with no lifetime parameter. A type that flattens a field, and a generic type, have their
hoisted mapping only under the bounds of their parts, which is more than the body of their `From`
impl can assume - a flattened type only has to provide a `From<&mut Row>` at all - so those types
stay on the name-based path. Nothing about them changed, and nothing about them has to change:
their mapping reads its columns by name like it always did, and the field mapping they get is the
same values in the same order.

Two details of a registry that lives in the process: the very first query of a type is the one that
registers it, so it maps its rows by name, and a type with a lifetime parameter is never `'static`,
so it stays on the name-based path for good.

The derive does not emit the hoisted side for a `flatten` field: a flattened part only promises a
`From<&mut Row>`, and the names it reads are the parent's business, not its indices (see
`Shapes of Flattened Types`). Such a type is mapped by names, like the body you write for it.

- 8 columns: 407 µs to 388 µs for 2000 rows, **4.8%** faster and within 2% of rusqlite's
  positional floor of 380 µs
- 24 columns: 1.25 ms to 1.07 ms, **14%** faster and at the positional floor of 1.11 ms
- the resolution is cheaper than the map it replaces, because the field names of a derived type are
  a sorted compile-time table: 0.29 µs to 0.18 µs at 8 columns, 0.79 µs to 0.42 µs at 24
- single-row mapping: 17% faster at 8 columns, 28% at 24

The row loop is the `hoisted` group of the bench file, run with `cargo bench --features
sqlite,macros`. The resolve step and the single-row numbers are not in the bench file, because they
are crate-internal; they are measured in the same style of harness on the same machine, and the
`build` group of the bench file shows the map cost the resolve step replaces. A real
`query_map()` call reaches the registered mapping through one function pointer per row, which
measures within the noise of the direct call in the same harness, so a query of a derived type
lands a hair above the `hoisted` group rather than exactly on it.

Resolution is by name, so the order of your fields and the order of the columns in the query stay
independent, a column that only differs in case is matched like `row.get("name")` always matched it,
and a duplicated column name maps to the first occurrence. A field whose column is missing from the
result set is loud on the first row that reads it: the panicking family (`FromRow` with
`query_map()`) panics naming that column, the non-panicking family (`TryFromRow` with
`query_try_map()`) answers `Error::QueryParams` naming it. A query whose result set has no rows
answers empty like it always did, because there is no row that would read the missing column.

Public items added for this, all of them additive: the `FromRowIndexed` / `TryFromRowIndexed`
traits, `Row::get_idx()` / `Row::try_get_idx()`, `ColIndex`, `FieldMaps`, `FieldNames` and the two
`#[doc(hidden)]` registration functions the derive calls. A hand-written `From<&mut Row>` impl
needs none of them; implement `FromRowIndexed` by hand, call `register_hoisted()` once for the type,
and it maps through the hoisted path as well.

#### Shapes of Flattened Types

`sql_map!()` checks the columns a query returns against the shape of the type it maps. A type that
flattens a field can only publish its shape when every part publishes its own column names, and a
part that publishes nothing is now a compile error **naming that part**. Before this, the columns a
hand-written `From<&mut Row>` reads were silently left out of the check, so a `sql_map!()` of a
nested type checked fewer columns than the mapping reads.

The fix for a part whose `From<&mut Row>` you write yourself is the new `#[derive(SqlShape)]`: the
shape-only counterpart of `FromRow`, with the same fields and the same `#[column(...)]` attributes,
emitting only the shape and no mapping. A type that is not a struct - an `enum`, for instance -
writes the `SqlShape` impl the docs of `SqlShape` show.

```rust, notest
#[derive(SqlShape)]
struct Geo {
    lat: i32,
    lon: i32,
}

// the mapping of `Geo` stays the hand-written one you already have
impl From<&mut hiqlite::Row<'_>> for Geo { /* reads `lat` and `lon` */ }

#[derive(FromRow)]
struct Entity {
    id: i64,
    #[column(flatten)]
    geo: Geo,
}

let row = client
    .query_map_one(sql_map!(Entity, "SELECT id, lat, lon FROM complex"), params!())
    .await?;
```

Only `sql_map!()` needs a shape. A type that publishes none, whose queries go through `sql!()` or a
plain string, compiles and runs exactly as it did, and a type that derives `FromRow` needs no change
at all: its shape is published by the derive, including the parts it flattens.

#### `GET /backup` and `GET /metrics`

`GET /backup` on the API port reports a node's backup state as JSON: the local backups on disk, the
last successful and the last failed local backup since the node started, and with `s3` the newest
backup this node has seen in the bucket. Every node reports the bucket, not only the leader that
uploads: each other node watches the bucket for the object named in the backup entry, and keeps the
newest one in `s3_latest.json` in its backups dir, so it survives a restart.

`GET /metrics` returns the same as Prometheus gauges. Both are read-only and have their own secret,
the new optional `secret_metrics` (TOML key, `HQL_SECRET_METRICS`, at least 16 characters and
different from `secret_api`), so a scrape config never needs the secret that can modify the
cluster. Without it, both answer `404`. See `docs/backups.md`.

#### `backup_file_prefix` config var

The new `backup_file_prefix` (TOML key, `HQL_BACKUP_FILE_PREFIX`) is the prefix of every backup
file name.

A backup is always `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite`, on disk and in the bucket, and
`backup_node` is the default, which is what every existing file is named. Listing, the local cleanup
and the S3 cleanup job accept that exact shape of your own prefix only, and nothing that does not
match is ever deleted - so several clusters can share one bucket, and files that are not Hiqlite
backups in a shared directory or bucket are untouched. `Client::backup_file_local` validates the
name against the same rule.

#### `backup_restore_keep_previous` config var

On a backup restore, every node other than 1 deletes its state before it joins the restored cluster,
at a time when node 1 has not applied the backup yet. The new opt-in `backup_restore_keep_previous`
(TOML key, `HQL_BACKUP_RESTORE_KEEP_PREVIOUS`, default `false`) moves that state into
`{data_dir}/pre_restore` instead, and removes it once the node is a member of the restored cluster.
Until then, every node needs as much additional disk space as its database, snapshots and logs take.

A restore, on any node, now also refuses to touch a `data_dir` whose state machine lock is held by
a live process.

#### Shutdown Behavior

A graceful shutdown on the leader hands leadership over before it leaves, so a shutdown doesn't cost
the cluster an election. It tries the voters in the order openraft follows when a removed leader
steps down - the greatest matching log first - and gives each candidate a slice of the remaining
budget, because a transfer that lands on a node which is unreachable, or whose log is behind, is
silently ignored. Only the leader does anything, and a single node has nobody to hand over to, so it
shuts down exactly like before.

#### `shutdown_delay_millis` config var

The pause at the start of a graceful shutdown on a multi-node cluster was a fixed 9.5 s. It is now
`shutdown_delay_millis` (TOML key, `HQL_SHUTDOWN_DELAY_MILLIS`, `NodeConfig::shutdown_delay`),
9500 by default. v0.12 removed the old key of the same name in favor of the automatic delay; the
automatic behavior stays the default, and the key is back only as an override, for orchestrators
with a short grace period. `/ready` answers 503 for the whole delay, so readiness probes and load
balancers drain the node before it goes away. Keep it at least one readiness check interval plus
1 s, or a load balancer may still route requests to a node that is shutting down and the clients
see errors. A cluster refuses to start with less than 3000 ms (`MIN_CLUSTER_SHUTDOWN_DELAY`); a
single instance skips the delay and ignores the value. A 3-node cluster measured 9.55 to 9.75 s from SIGTERM to exit with the default.

`Client::shutdown()` and `ShutdownHandle::wait()` now give up after the delay plus
`SHUTDOWN_TEARDOWN_BUDGET` (10.5 s) instead of a fixed 20 s, which is the same 20 s with the
default delay. Keep the sum below your orchestrator's grace period, 30 s for a Kubernetes pod by
default.

#### Test Suite

`openraft`'s storage test suite now runs against both cache log stores, the WAL and the in-memory
one, together with the memory state machine.

#### Extended API and Cluster Control

> You should never automate changing anything on these endpoints. They should always be triggered
> manually, in very specific situations.

##### `/cluster/election`

- `GET`: answers the server state, leader, voter flag, and the effective `enable_elect` of every
  group
- `PUT`: with `{ "enable_elect": true|false }` changes it for all groups of that node in one call
- `POST`: makes the node campaign for the leadership of all its groups. openraft runs its Pre-Vote
  round first, so the term is untouched when the node cannot win, and a node whose own elections are
  disabled is refused by its own gate. A learner is refused on `POST`, it can never win an election.
  A runtime change is in-memory only: a restart applies the marker of the node line again, and every
  change is logged.

The matching `Client` helpers are `cluster_election_state()`, `set_node_election(node_id, enable)`
and `elect_node(node_id)`. They work on a remote client for the whole cluster. An embedded
client can only reach its own node.

#### Membership Control

Membership control without a `{raft_type}`, acting on all groups of a node in the same way:

- `GET /cluster/membership` answers voters, learners, leader and server state of every group
- `POST /cluster/promote` with `{ "node_id": 3 }` moves a learner into the voter set
- `DELETE /cluster/membership` with `{ "node_id": 1, "stay_as_learner": true }` takes a voter out
  again
- `POST /cluster/transfer_leader` with `{ "node_id": 2 }` hands the leadership of both groups to one
  specific node. They answer the full membership of the node that ran them, so the call that changed
  something also shows the new shape. They run on the leader: a follower answers with the same
  redirect payload a write gets, and clients follow it. The existing `{raft_type}` routes for a full
  voter set and for a node leaving are untouched and stay the way to repair a half-applied change.

Demoting the current leader with `stay_as_learner: true` is the switchover of a master-slave setup:
openraft steps it down onto the most up-to-date remaining voter, by the same rule as the shutdown
handover - no election timeout, and the leader grants the target its own vote. A leader that would
demote _itself_ to a learner is refused, because it keeps leading in that shape and openraft cannot
complete the change: hand the leadership over with `transfer_leader` first and run the demotion on
the node that took it. `transfer_leader` is a request, so a node whose log is behind ignores it, and
a learner is refused as a target outright.

The matching `Client` helpers are `cluster_membership()`, `promote_node(node_id)`,
`demote_node(node_id, stay_as_learner)` and `transfer_leadership(node_id)`. A remote client sends
them to the leader it knows and follows a leader redirect, an embedded client runs them on its own
node.

#### Geo-Replication Possibilities

Nodes can be marked to never start an election on their own: a node line accepts an optional 4th
value, `id addr_raft addr_api election_disable`.

A marked node starts with `raft_config.enable_elect = false` for both of its Raft groups. It stays
a full voting member: it grants votes like every other voter, it counts towards the quorum, and a
leader can hand leadership over to it at any time - it just never nominates itself.

This makes it possible to create somewhat of a Geo Replicated backup node with a 2 node cluster
(2 local, 1 far node), without paying the latency overhead that you would typically have with Raft
over WAN. Each write needs 2 network round-trips, which means if the far node would become the
leader, or necessary to reach a quorum, your latency would spike. This makes it possible to run
2 local fast nodes HA, and the 3rd node can be geo-replicated. This has some caveats though. If one
of the local nodes goes down, your latency is bound by the far node until the local one comes up
again. This is not a big issue for restarts or upgrades, but should be kept in mind for longer
downtimes. On the upside, with an extenced API now, this would even make it possible to do a hot
switch-over to a disastery recovery location. This will never be handled automatically though, and
needs manual triggers.

> The whole thing is documented in `docs/geo-replication.md`: the config, the API, the handover
> rules, the failover playbooks, monitoring, and the list of things that will bite you.

#### New Start Functions

New start functions `hiqlite::start_node_from_backup(node_config, source)` and
`hiqlite::start_node_with_cache_from_backup(node_config, source)`. They take the backup to restore
as a parameter - `hiqlite::BackupSource::File(path)` or `hiqlite::BackupSource::S3(object)` - and
never read `HQL_BACKUP_RESTORE`, which is what an app needs that runs several Hiqlite instances in
one process, each with its own data dir, its own prefix and its own backup. Everything else is
the env var behaviour: pass the same source to every node of the cluster, node 1 applies it, the
others wipe their state and join.

`HQL_BACKUP_RESTORE` is evaluated at startup, and a value it cannot honour is now an error that
stops the node instead of a log line and a normal boot: an `s3:` source on a build without the `s3`
feature, and a value without the `s3:` or `file:` prefix. Restoring from a backup is a deliberate
act, and a disaster-recovery request that gets quietly ignored is the worst way to find that out.

#### Cache Index Metadata Reset

Hiqlite writes a normalized version of the Cache Index enum to disk. It compares it against the
previous version on each startup. This is necessary, because the caches are indexed by a positional
int for speed instead of by string / hash. However, this comes with the cost that the Cache Index
enum is append-only and must never change.

The new `HQL_CACHE_META_RESET=true` re-initializes the cache-index fingerprint of your
`CacheVariants` enum for the caches group. It deletes that fingerprint only: the database, the Raft
logs, and your cache content are untouched. A renamed variant needs it on every node's start, for
one start, and nothing else.

**CAUTION:** A removed variant needs a guarantee from you: every node has to clear all of its caches
at startup, because the entries of the removed index are still persisted, and swapping the names of
two variants counts as a removal too. If all you are doing is renaming a variant because you like a
different name better, but the data inside stays the same, there is nothing else to do.

#### Documentation

Up until now, all "documentation" lived either in the readme, or in sources. It got kind of messy,
and this version starts with adding proper documentation in `docs/`. This is probably not complete
yet, but a good starting point.

### Bugfix

- `/ready` answered `400 Bad Request` while a node was replaying its log, while it had no leader
  yet, while it was not a member of a healthy group, and while it was shutting down. All four are
  "not usable right now, ask again later", which is `503`, and `docs/operations.md` has always said
  `503`. A readiness probe that reads `400` has no way to tell a node that is catching up from a
  node that is broken, so it is the number, not the answer, that was wrong. `/health` uses `503`
  for the same states now, where it used `409` and `500`.
- A node 1 without any Raft state could form a second cluster next to the existing one. Before it
  initializes, node 1 asks the other nodes whether a cluster exists, but it stopped asking after the
  first one and counted a connection error as "not initialized". With node 2 unreachable and node 3
  holding the cluster, node 1 initialized on its own and accepted writes; the two clusters even
  shared the same log ids (a new cluster always starts with node 1 in term 1), so Raft never noticed
  the conflict when they met again. Node 1 now asks every other node in each round, joins as soon as
  one of them is initialized, and initializes only once a majority of the other nodes answered that
  they were never initialized. A wrong API secret or an initialized node whose Raft is not running
  yet no longer counts as "not initialized" either. If no other node is reachable at all for 10
  seconds, node 1 still forms the cluster alone, because that is how a fresh cluster started one
  node after the other (`podManagementPolicy: OrderedReady`) and the disaster recovery path look.
  The new `init_require_peers` (`HQL_INIT_REQUIRE_PEERS`, default `false`) turns that off: node 1
  then never forms a cluster without the other nodes answering.
  `GET /cluster/membership/{raft_type}` answers a group that was never initialized with the new
  `Error::RaftNotInitialized` instead of a `Config` error string.
- `wait_for_cluster_sync()`'s single-instance fast path only looked at the node list, which a local
  client never has, so it never fired for an embedded single node. A local client now takes the
  member count from its own Raft membership, like `shutdown_execute` does.

## v0.16.0

The `cryptr` and `s3-simple` dependencies were pulled in with no `backup` or `s3` targets enabled.
This is not the case anymore.

The `external-state-machine` feature now comes with its own serialization codec. This mean it does
not pull in `bincode-next` as a dependency anymore. It is byte-compatible with the bincode legacy
config that was used up until now, so it's a drop-in replacement.

Apart from that, this release basically only updates dependencies. The dashboard was migrated from
SvelteKit2 to 3 because of security issues in SvelteKit2, and the underlying `rusqlite` and
therefore SQLite was bumped to the latest version as well. Even though a `cargo update` was able to
solve the latest `rustls` security issue in your main app, `hiqlite` pins this now to at least
`0.23.46` to force it to the fixed version.

The `external-state-machine` writer was adapted to the new `rusqlite` API. The SQLite authorizer
guard around a caller operation is installed before the transaction is opened, with a short-lived
internal phase for Hiqlite's own `BEGIN`/`COMMIT` and `_hiqlite_external_*` bookkeeping. Nothing
changes for implementors of `DeterministicSqliteOperation`.

## hiqlite-v0.15.3

This version slims down the loaded dependencies when using the `external-state-machine`, and it removes the built-in
split-brain check task. The split-brain-check was only used for very early debugging in production. By now, the Raft
cluster management is super stable, and it's just not necessary anymore. It also logged lots of errors when you were
using in-cluster TLS.

## hiqlite-v0.15.2

The new "internal wait for WAL sync" from `v0.15.1` had a flawed logic. It did not work as intended, and it always ran
into the 30-second timeout, which made it pretty useless. It was removed again. You will have the new client functions
`wait_for_cluster_sync` and `wait_for_cluster_sync_timeout`. They work as expected.

## hiqlite-v0.15.1

### Stale WebSocket Tasks

It was possible that WebSocket tasks become stale and therefore leak memory for the Raft-internal communication during
Raft leader switches. This did not happen all the time, but it was kind of likely to happen. Everything is wrapped in
task-aborting structs now, and we have additional TCP keepalives and timeouts for the underlying socket as an additional
safety-net.

This change should reduce idle memory consumption for very long-running clusters when you have a rather unstable
network, or nodes, or frequent restarts.

### Dashboard `panic`

There was a reachable `panic` via the dashboard (after you authenticated), when you sent a query that would be invalid
SQL with UTF-8 characters that span multiple bytes.

### Startup Replay Check

During startup, we have an additional check that waits until all possibly replayed WAL logs are applied to the state
machine before starting the API. This prevents race conditions on systems with slow disks. There is also an additional
function for the `hiqlite::Client` that can you can call after the startup to optionally wait for the node to be in sync
with the rest of the cluster. After longer downtime of one of the nodes, it may take a few seconds to get all the
missing information from the current leader. This function is `wait_for_cluster_sync` or
`wait_for_cluster_sync_timeout`.

### Disaster Recovery

During the backup restore procedure with enabled `cache` feature, the cache data was not cleaned up properly on node 1.
All other nodes did it as they should, but the node 1 logic forgot about the dirs and only cleaned up the DB state
machine data. This means a disaster recovery with a cache needed a manual cleanup of the `data_dir` upfront. This is now
handled automatically as expected.

## hiqlite-derive-v0.15.1

The `0.15.0` release introduced a regression where `Option<T>` types were not considered optional anymore because of
their generic argument.

## hiqlite-wal-0.15.1

- When the writer task creates a new WAL file on roll-over, it is now flushed and synced to disk synchronously.
- When a flush / sync fails with `SyncImmediate`, the writer now exists and stops working in favor of data consistency.

## v0.15.0

This version brings a huge number of tiny (and some big) bugfixes. The list is too long to write it down here
meaningfully. In general: if you had any issue, this version probably fixes it.

### BREAKING - VERY IMPORTANT

**IF YOU DON'T FOLLOW THE STEPS BELOW, YOU MIGHT END UP WITH INCONSISTENT DATA!**

You **MUST** clean up all **CACHE** files from an older version when you migrate to this one! Also, you **MUST NOT** do
a **ROLLING RELEASE**. You will have short downtime with this upgrade. When you simply remove the cache data, and you
have other cluster nodes running, the cleaned-up node will restart, join the existing cluster, and sync the just
cleaned-up data. This **MUST NOT** happen!

Your cache data that you need to delete before a **full, clean cluster restart** lives in:

- `<data_dir>/logs_cache/`
- `<data_dir>/state_machine_cache/`

Hiqlite is trying to help and guard you as much as possible. There are safeguards and startup checks in place. You can
either delete the data manually, or, when running inside a container where this can be very annoying, there are helper
env vars that trigger an automation in code. However, these are **not enabled by default**! This is important because
otherwise they will delete cache data on their own, which would be very unexpected. You can trigger the auto-cleanup by
setting:

```bash
HQL_CACHE_WAL_AUTO_MIGRATE=true
```

If this is set, Hiqlite will check if the cleanup is necessary and if so, it will also try to connect to all configured
cluster members to make sure they are other not running, or they are already running the new version properly. If you
screwed up badly and somehow made it crash despite the safeguards (they try their best, but it's possible
theoretically), there is another env var to trigger a forced cleanup, even when Hiqlite detects that it was started with
the new version already. You can then set `HQL_CACHE_WAL_FORCE_MIGRATE=true`. But you **MUST** remove this var
immediately after startup again, because apart from `HQL_CACHE_WAL_AUTO_MIGRATE`, it will trigger on each restart
regardless!

**CAUTION: If you decide to delete the cache data manually, the safeguards during startup that check for still running
cluster nodes on old versions CANNOT TRIGGER correctly! They will assume you are starting a fresh cluster, which is
always safe. If you do the manual version, you are responsible for guaranteeing a full cluster shutdown before
restarting the first node with the new one!**

This might be a bit annoying because you cannot do a rolling release, and you will have short downtime, but it will be
worth it. The reason for this is an internal rework of the WAL file and some Raft types. Before, it was, for instance,
not possible to change the enabled features when using the cache because it had the potential to screw up your Raft
logs. A second reason is that the wire type for Raft networking was optimized as well. It made sense to do this in one
batch so that you only have this downtime once.

With this release, you can change defined features for an existing app later on without breaking any possibly existing
Raft or WAL files. If you had features disabled, the logs will have placeholder indexes. They don't cost us anything
expect for just a very few lines of "dead" code when unneeded, but we gain stability for that. All they do is basically
skip some indices. Since we needed this breaking change anyway, we also added a few additional possible Raft messages
in order (cleaner code and maintenance) to provide the new atomic cache operations mentioned below.

In the end, it's just a cache. You only need to clear the cache and **NOT THE DATABASE**! The database WAL +
State Machine are fully compatible.

**Even though this migration is pretty easy, you should definitely create a backup right before you do this!**

#### Other Breaking Changes

- The following config vars have been renamed:
    ```
    health_check_delay_secs     -> health_check_delay
    HQL_HEALTH_CHECK_DELAY_SECS -> HQL_HEALTH_CHECK_DELAY
    backup_keep_days            -> backup_keep_for
    HQL_BACKUP_KEEP_DAYS        -> HQL_BACKUP_KEEP_FOR
    backup_keep_days_local      -> backup_keep_for_local
    HQL_BACKUP_KEEP_DAYS_LOCAL  -> HQL_BACKUP_KEEP_FOR_LOCAL
    ```
    Apart from that, all these values are now of type `Duration`. This means you can either provide an Integer, and they
    will be interpreted as seconds, or you provide them as a String like '120s', '2m', '3h', ..., and they will be parsed
    with the given unit. The available units are: `s, m, h, d, w, y`.
- The `listen_notify_local` feature was dropped and folded into `listen_notify`. The `*_local` feature only existed
  to prevent the `eventsource-client` from being pulled in, which was needed for remote-only clients. The connection
  type was reworked from SEE (b64 encoded text data) into raw binary over WebSocket, which is much faster and more
  efficient.
- The database type conversions now fail loudly with a `panic` when there is an explicit mismatch between the returned
  type from the DB and the Rust type it should be converted into. These hat the potential before to become silent `NULL`
  s. The new behavior highlights unrecoverable logic issues during development immediately.
- It is not really a breaking change, but worth mentioning here: The default for `log_sync` was changed. The default
  before was `ImmediateAsync`. This was too loose as a default. You commit buffers into the kernel, but you don't know
  when the data is flushed to disk. The kernel was an internal ticker every 30 seconds, which is a long time. You data
  was safe, of course, as long as you did not have a kernel panic, but that's not a good default. The new default is
  `IntervalMillis(200)`. Data is still collected async in memory, but will be force-flushed every 200ms to disk. There
  is almost no real throughput penalty compared to `ImmediateAsync`, but you have way better consistency (even though
  more stress on your SSD).
- The ENV var `HQL_TLS_{variant}_DANGER_TLS_NO_VERIFY` was renamed to `HQL_TLS_{variant}_NO_VERIFY`. Apart from that,
  there is now a `REFERENCE_CONFIG.toml` in the crates root dir. It was updated and added into the tests. You will also
  be able to grab it directly from the crate, es it is embedded at compile time (with the `config-embed` feature).
- `counters` is now part of the `full` feature.
- The MSRV for all crates was bumped to 1.95.

### Changes

A lot of internal code has been reworked in terms of security, robustness, stability, future maintenance improvements.

The `bincode` codec was replaced by the byte-compatible `bincode-next` serde adapter (faster, SIMD-accelerated
decoding). On-disk and wire formats are unchanged; the legacy `bincode` crate is now a dev-dependency only. `bincode`
itself was fine, but it was unmaintained, which did not look good during things like `cargo audit`.

#### New Cache Functions

In combination with the breaking change WAL rework mentioned above, a few new cache functions were added. These are:

- `get_remove()`
- `get_remove_bytes()`
- `replace()`
- `replace_bytes()`

They are atomic functions, and they do what their name suggests. The `replace` fn's replace the current value with the
new one and return the old value, if is existed, in an atomic operation.

#### Cache Collision Safety

It was possible that cache entries into the exact same cache that ended on the exact same TTL (second precision) could
overwrite each others TTL, leaving one of the entries in place until forever and never being cleaned up. The TTLs are
still in second precision, but the internal cache handler works with microseconds. If it detects a collision, it simply
advances the conflicting new TTL in 1-microsecond steps until it finds a free slot. This is not 100% collision-free of
course, because if you insert 1.000.000 entries into the exact same cache that have the exact same calculated absolute
TTL consistently, you would get an overflow, but that is

1. very unlikely
2. easy to counter (use multiple caches)
3. the current approach is the fastest and most efficient

#### Distributed Lock Stability

The distributed locks handler had some stability issues. It was possible that some locks could end up being stale
and never woken when there was a conflict. This handler was completely reworked, and this should not happen anymore.

Locks are now self-renewing: as long as the `Lock` handle is alive, the client sends a heartbeat that extends the lease
on all cluster nodes. The hard timeout (10 s in release builds, 2 s in debug builds) now only applies to dead clients,
so a held lock no longer expires while its owner is still running. The new `Lock` you get also provides a helper
function `is_locked()` that you can use to double-check your lock is still hold, even if you have a network partition
when you are beyond the 10s timeout with a very long-running job. The `Lock` is also not cloneable anymore. It made
no sense that it derived `Clone`, because it will release as soon as one of them is dropped, which makes a clone pretty
useless.

#### `TryFromRow` Macro

The `FromRow` macro `panic`s if there is a type mismatch between the returned data and the Rust `struct` you are trying
to map the row to. I always favor a `panic` over an error in case of an unrecoverable error, but there were people
asking for graceful errors in such a case. You can now optionally derive `TryFromRow` and use one of the new
non-panicking query functions from the client:

- `query_try_map()`
- `query_try_map_one()`
- `query_try_map_optional()`

These will behave exactly like the already existing counterparts without the `try`, with the only exception that they
will return a `Result<T, hiqlite::Error>` for the value in case the mapping fails.

> The other existing macros have been optimized quite a bit as well. They don't provide new functionality but way
> better debugging capabilities.

#### Rate-Limiting Fixes

It was possible that the rate-limiting bucket inside the client got more permits than intended.

#### External State Machine

If you already have an external state machine, you can now provide your own and re-use all Hiqlites parts.

## v0.14.0

### Breaking

`NodeConfig::from_toml` and `NodeConfig::from_toml_table` take a new `secrets: Option<toml::Table>` argument (after
`table` / `table_name`, before the optional `enc_keys`). Existing callers can pass `None` to keep the previous behavior.

### Raft Rate-Limiting

You can now rate-limit all Raft write actions. These are things like cache PUT or e.g. execute queries. This limit does
not affect read actions, since they are local and do not travel through the network. With this rate-limiting, if tuned
properly for your deployment, you can guarantee that you never saturate your disk, and therefore can never get in a
situation where you overwhelm the cluster, no matter how high requests might spike.

Just as a very rough guideline: When you only write tiny queries that `INSERT` 3 columns per row, and you limit the Raft
to 50.000 requests / s, your disk already writes 350+ MB/s. How you want to tune this value depends a lot on how much
your server can handle and how expensive your queries are. It's impossible to provide a reasonable default value here.

Each client has its own rate-limiter. This is crucial. If the leader enforced it, all limited requests would need to
travel through the network only for getting limited. This means if you have a traffic spike that your deployment cannot
handle, it would still flood the network. Instead, each client enforces this locally BEFORE sending out any network
requests. At the same time, this means if you limit to e.g. 100 RPS while having a 3-node cluster, it will effectively
mean ~300 RPS for the whole cluster (assuming even load balancing).

You have new config values:

```toml
# To guarantee the stabiliy of your Raft cluster, you can
# rate-limit all write operations to the Raft (excluding
# management overhead). This is in request per second. It
# guarantees that the cluster can never be overwhelmed
# because maybe the disks cannot keep up. Usually, when
# this happens, you would see dropped or missing heartbeats
# and errors that the leader is down, even when the cluster
# is healthy.
# The burst can usually be 2-2x the rps.
#
# default: not set
# overwritten by: HQL_RL_CACHE_RPS
rate_limit_cache_rps = 50000
# overwritten by: HQL_RL_CACHE_BURST
rate_limit_cache_burst = 100000
# overwritten by: HQL_RL_DB_RPS
rate_limit_db_rps = 100000
# overwritten by: HQL_RL_DB_BURST
rate_limit_db_burst = 200000
```

### Learner-Only mode

Added `NodeConfig::learner_only`, `learner_only`, and `HQL_LEARNER_ONLY` to let newly joining nodes stay learners during
startup reconciliation instead of being promoted to voting members. This makes it possible to add read-only cluster
members.

### New feature: `in-memory-snapshots`

By default, even if you set `cache_storage_disk = false` to keep the WAL in-memory, Cache snapshots will still be
written to disk into a temp file. This means the `data_dir` must be writable. Snapshots are only necessary if a new node
joins the Raft cluster, and then only the leader needs to stream the latest snapshot to the new node. This means in most
cases, you never need them. Writing them into a temp file is the most efficient solution. However, with this feature,
you can opt-in to keep snapshots in-memory as well. This will not need any disk at all (as long as you don't enable
`sqlite` of course). This is costly, though, because you effectively double your memory requirements for each single
cache entry.

### Improvements for stack sizes

Quite a few tasks were pretty big and consumed a lot of space on the stack. Some even so much that `tokio` auto-boxed
them. All of them were tuned to not throw warnings anymore. There are a few left inside `openraft`. To my knowledge, we
cannot control these sizes indirectly with our trait impl's. This needs some further investigation.

### Secrets from file

Added support for loading secret-bearing config values from a separate secrets source. Any of `secret_raft`,
`secret_api`, `s3_key`, `s3_secret`, `enc_keys`, `enc_key_active`, and `password_dashboard` may be set to the
case-sensitive sentinel `"$SECRETS"`, in which case the real value is looked up by the same key in a secrets table. That
table is provided either via the new `secrets` argument of `NodeConfig::from_toml` / `NodeConfig::from_toml_table`, or,
when that is `None`, via a `secrets_file` config option (or `HQL_SECRETS_FILE`) pointing to a TOML file that mirrors the
config structure. This keeps the main config diffable and version-controllable while secrets are managed separately
(systemd `LoadCredential`, Docker / Kubernetes secrets, ...).

## hiqlite-v0.13.2

This release only exists to fix the removal of the `auto_doc_cfg` feature on docs.rs in the hope that docs now build
again without errors.

## hiqlite-v0.13.1

When using the secure cookie with the Dashboard, the builder string was not formatted correctly and the `Path` was not
set.

## v0.13.0

This is a pretty exciting release with very much improved DX. ~12% improvements in throughput have been made as well.

### Breaking

#### Changed `impl From<hiqlite::Row<'_>>`

To make the new derive macros work, and make it possible to theoretically infinitely nest the parsing when you use
`#[column(flatten)]` in combination with `FromRow` (see below), the necessary `impl` was changed slightly from:

```rust
impl From<hiqlite::Row<'_>> for MyStruct {
    fn from(mut row: hiqlite::Row<'_>) -> Self {
        todo!()
    }
}
```

to:

```rust
impl From<&mut hiqlite::Row<'_>> for MyStruct {
    fn from(row: &mut hiqlite::Row<'_>) -> Self {
        todo!()
    }
}
```

This migration is easily done with 2 global _find and replace_:

- rename all `impl From<hiqlite::Row<'_>> for` to `impl From<&mut hiqlite::Row<'_>>`
- rename all `fn from(mut row: hiqlite::Row<'_>) -> Self {` to `fn from(row: &mut hiqlite::Row<'_>) -> Self {`

> If you did not use the `'_` lifetime elision and used for instance `impl<'a> From<hiqlite::Row<'a>>` before, you need
> to adjust the global find of course. Both versions work just fine.

The function body will work in the same way.

#### `hiqlite_macros` embedded into `hiqlite`

The `hiqlite_macros` crate was necessary to add to your `Cargo.toml`, if you wanted to get the `params!()` macro. With
the addition of the new `derive` macros and an internal rework, this has been moved into the `hiqlite` main crate behind
the feature flag `macros`. The migration is easily done with a global find and replace. Remove the `hiqlite_macros`
dependency, add the `macros` feature to `hiqlite`, and then replace all `use hiqlite_macros::` with
`use hiqlite::macros::`.

The `hiqlite_macros` crate will only work for versions until this one. The `derive` macros needed their own crate
anyway, and the `params!()` macro was added to the `hiqlite` main crate directly.

#### No `strum` dependency anymore and `CacheIndex` is gone

While `strum` is a really nice crate, adding another dependency for `hiqlite` felt weird. Only the `strum::EnumIter`
derive was needed anyway if you wanted to use the cache. This is gone now. Instead of `strum::EnumIter`, you now only
derive `hiqlite::macros::CacheVariants` (with the `macros` feature).

In addition, you can remove the `impl CacheIndex`. The derive macro will take care of everything necessary.

#### `hiqlite::ServerTlsConfig` moved into `hiqlite::tls::ServerTlsConfig`

Because of additional features, the `ServerTlsConfig` was moved into the `tls` module.

#### `full` feature changes

Since it's unnecessary when you use auto TLS certificates, the `webpki-roots` feature is not added anymore with the
`full` feature. In addition, full does not include `listen_notify` anymore, but `listen_notify_local` instead, which
makes way more sense in almost all situations.

### Changes

#### `FromRow` macro

If you use any or the `query_map*` functions, you had to manually `impl From<hiqlite::Row<'_>>` before. This is not the
case anymore. You now get the `FromRow` derive macro. In combination with quite a few possible `column` attributes, most
`struct`s don't need a manual implementation anymore.

For instance, if you define the following:

```rust
#[derive(Debug, FromRow)]
struct Entity {
    id: i64,
    #[column(rename = "name_db")]
    name: String,
    desc: Option<String>,
    vec_wrap: VecText<'\n'>,
    #[column(flatten)]
    sub: EntitySub,
    #[column(skip)]
    skipped: Option<String>,
    some_int: i32,
    #[column(flatten)]
    my_enum: MyEnum,
    #[column(parse)]
    left_right: Option<LeftRight>,
    #[column(rename = "ud", from_string)]
    up_down: UpDown,
    #[column(from_i64, rename = "num")]
    number: Number,
}
```

... the `FromRow` macro will generate the following output:

```rust
impl ::std::convert::From<&mut ::hiqlite::Row<'_>> for Entity {
    #[inline]
    fn from(row: &mut ::hiqlite::Row) -> Self {
        use ::std::str::FromStr;
        Self {
            id: row.get("id"),
            name: row.get("name_db"),
            desc: row.get("desc"),
            vec_wrap: row.get("vec_wrap"),
            sub: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),
            skipped: ::std::default::Default::default(),
            some_int: row.get("some_int"),
            my_enum: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),
            left_right: row
                .get::<Option<String>>("left_right")
                .map(|s| s.parse().unwrap()),
            up_down: row.get::<String>("ud").into(),
            number: row.get::<i64>("num").into(),
        }
    }
}
```

For more in-depth information, take a look at
the [derive-complex-types](https://github.com/sebadob/hiqlite/tree/main/examples/derive-complex-types) example.

#### New type `VecText<const S: char>`

This is a smart wrapper type for cases when you want to save `Vec<_>` into your DB, and of course parse them during
`query_map*` as well. SQLite can work with arrays, but it needs virtual tables to do this, which comes with a
performance hit and additional overhead on DB level, which is far from ideal.

To overcome this, I added `VecText<const S: char>`. This type will store and `Vec<_>` inside `TEXT` columns, separated
by the custom separator. This means way easier and quicker handling on the database, and you can even lazily convert
into a `Vec<_>` later when you actually need it. This means fewer memory allocations upfront and faster queries.

It will basically store all values as a CSV with the exception that you can choose the separation character to match
your input data. For instance, you might have values inside your `Vec<_>` which may contain `,`, which makes it of
course impossible to use it as a separator. But let's say values will never contain `\n` or `;`, then you can use these
for separation.

This type can be used inside the `params!()` macro directly, and it can be parsed from DB columns easily, since it's
only a `String` behind the scenes until you actually need it to be a `Vec<_>`. The basic usage may look like this:

```rust,notest
use hiqlite::VecText;

let values = vec![
    "Entry 1".to_string(),
    "Entry 2".to_string(),
    "And another one".to_string(),
];
let v: VecText<'\n' > = VecText::new( & values).unwrap();
// `parse()` will work here as well instead of `try_into_vec()`.
// The different impls just provide more flexibility.
let r = v.try_into_vec::<String>().unwrap();
assert_eq!(values, r);

let v: VecText<',' > = VecText::new(& [1, 2, - 3]).unwrap();
let r = v.parse::<i32>().unwrap();
assert_eq!(&[1, 2, -3], r.as_slice());

let v: VecText<';' > = VecText::new(& [1, 2, 13]).unwrap();
let r = v.parse::<u8>().unwrap();
assert_eq!(&[1, 2, 13], r.as_slice());
```

#### New features `cast_ints` and `cast_ints_unchecked`

##### `cast_ints`

This feature will make it possible to do integer casts when converting from a DB INTEGER. SQLites `INTEGER` is
technically always an `i64`. This means you cannot safely convert into e.g. a `u32`. However, since Rust is strictly
typed, you can assume that even though columns can store `i64`, they will never contain values outside a `u32`.

This version will do boundary checks. For instance, when you want to cast to `u32` and your DB column contains a value
bigger than `u32::MAX`, it will only contain up to `u32::MAX`. When it's below `0`, it will only contain `0`, and never
overflow.

DB column values can be cast into the following types with this feature:

- `u64`
- `i32`
- `u32`
- `i16`
- `u16`
- `i8`
- `u8`
- `Option<any_of_the_above_ints>`

##### `cast_ints_unchecked`

This works in the same way as `cast_ints`, with the exception that it's a tiny bit faster. It will do an unchecked cast
without boundary checks. When you insert into properly typed values with your queries, this is safe to use.

#### `url::Url` + `uuid::Uuid` as DB values

`url::Url` and `uuid::Uuid` can now be converted into `hiqlite::Param` and parsed from DB values automatically. The
`url::Url` will be `TEXT`-backed, while the `uuid::Uuid` can be stored in a `BLOB` column for better efficiency and
sorting.

#### Improved `params!()` macro

The performance and efficiency of the `params!()` macro has been improved. The macro is typed out very specifically for
up to 24 given parameters. This means proper upfront memory allocation for the `Vec<_>` under the hood. Only if 24
params are exceeded, it changes to a dynamic version that starts with a pre-allocated capacity of 25 and then grows as
needed.

#### Auto-TLS certificates

The way how you could add TLS certificates was changed slightly because of the new addition to auto-generate them.
Before, you would have added TLS certificates like so:

```rust,notest
use hiqlite::ServerTlsConfig;

config.tls_raft = Some(ServerTlsConfig {
    key: "../../tls/key.pem".into(),
    cert: "../../tls/cert-chain.pem".into(),
    danger_tls_no_verify: true,
});
```

Now, you are doing it like so:

```rust,notest
use hiqlite::tls::{ServerTlsConfig, ServerTlsConfigCerts};

config.tls_raft = Some(ServerTlsConfig::Specific(ServerTlsConfigCerts {
    key: "../../tls/key.pem".into(),
    cert: "../../tls/cert-chain.pem".into(),
    danger_tls_no_verify: true,
});
```

On top of that, you can also use auto-generated certificates. If set, `hiqlite` will generate self-signed certificates
on startup, and all connections will be encrypted by default. This means you don't need to care about expiring
certificates and renewing them. With each restart, a new pair of certificates will be generated. Nodes do not validate
them for even more ease of use, because they don't have to. They will do a mutual-authenticating handshake using the
given secrets anyway. This makes using TLS a lot easier now without any additional maintenance.

Auto-Certificates can be used like so:

```rust,notest
use hiqlite::tls::ServerTlsConfig;

config.tls_raft = Some(ServerTlsConfig::TlsAutoCertificates);
```

When setting up your instance via TOML, you have a new config variable:

```toml
# The `tls_auto_certificates` will generate self-signed TLS
# certificates for internal Raft and API traffic. Clients will
# simply not validate the certificates for ease of use because
# they don't have to. They do a 3-way handshake anyway, which
# validates both client and server without the secret ever being
# sent over the network.
#
# If you specify specific certificates with either `tls_raft_*` or
# `tls_api_*`, they will be used instead.
#
# default: false
# overwritten by: HQL_TLS_AUTO_CERTS
tls_auto_certificates = true
```

## hiqlite-v0.12.2

### Changes

It is now possible to trigger a backup manually via direct API POST request, with something like:

```bash
curl -i -XPOST -H 'X-API-SECRET: SuperSecureSecret1337' localhost:8200/backup
```

Apart from that, a small DX change was added. When `debug_assertions` are active, the prepared statement will always be
compared to the given params it will make sure that the amount of params accepted matches exactly the length of the
amount of params given. Such a check existed before already, but if you had nullable params at the end of your query,
these could slip through.

### Bugfix

- When no S3 config was given, the backup cron job was not started and no local backups were created.

## hiqlie-v0.12.1

Tiny fix for `hiqlite` to improve quality of life and DX. The "wait for non-leader" during startup with an already
initialized Raft will be skipped for single instance deployments. This makes startups immediate again.

## v0.12.0

### Breaking

The `shutdown_delay_millis` config option was removed. It is not necessary to set it manually anymore. Instead, more
automatic detection is being applied and a necessary delay to smooth out rolling releases or make sure the readiness of
a container is being caught is added without the need for additional config.

Apart from that, lots of improvements have been made to rolling releases and how WebSocket re-connects and node startups
are being handled in general. There is a new `/ready` endpoint on the public API as well. It can be used in e.g.
Kubernetes to smooth out rolling releases and detect a pod shutdown before it becomes unable to handle Raft requests. To
do so, it is important however to not have too high `periodSeconds`, and the `headless` service needs to
`publishNotReadyAddresses` ports before ready, like so:

```yaml
apiVersion: v1
kind: Service
metadata:
    name: hiqlite-headless
    namespace: hiqlite
spec:
    clusterIP: None
    # only do that on the headless service
    publishNotReadyAddresses: true
    selector:
        app: hiqlite
    ports:
        - name: raft
          protocol: TCP
          port: 8100
          targetPort: 8100
        - name: api
          protocol: TCP
          port: 8200
          targetPort: 8200
```

Then you can make use of the new readiness check in the `StatefulSet`:

```yaml
readinessProbe:
    httpGet:
        scheme: HTTP
        port: 8200
        path: /ready
    initialDelaySeconds: 5
    # Do not increase, otherwise a shutdown might start before k8s catches it.
    periodSeconds: 3
    # Require 2 failures because you may get one during a leader switch.
    failureThreshold: 2
livenessProbe:
    httpGet:
        scheme: HTTP
        port: 8200
        path: /health
        initialDelaySeconds: 60
        periodSeconds: 30
        # Require 2 failures because you may get one during a leader switch.
        failureThreshold: 2
```

### Bugfix

The `hiqlite-wal` had a bug where the `last_purged_log_id` was overwritten with `None` during a log truncation, even if
it had a value from a log purge before. If the node restarted before another log purge fixed it, it would result in an
error during startup. The new version includes a check + fix, if you start up an instance with a data set that currently
has this issue.

## hiqlite v0.11.1

Bugfix when reading in the `password_dashboard` via TOML. The base64 decoding step was missing, while it was working
just fine when read via ENV.

## v0.11.0

This is a rather small release. Some external dependencies have been bumped to the latest versions. The biggest change
is the pretty important bugfix below.

### Bugfix

- It was possible to get into situations where the automatic WAL file cleanup was not working as expected, even when the
  log IDs were covered by the latest snapshot. This could lead to the volume filling up endlessly.

## hiqlite v0.10.1

This version removes an `unwrap()` during KV GET operations, that was reachable under some circumstances, for instance
when you cancel an async `hiqlite::Client.get()` before awaiting the result.

## v0.10.0

This is a rather small release. The main thing about it is that `rocksdb` was removed completely after `hiqlite-wal`
has proven to be stable. This makes `hiqlite` a lot more light-weight and makes it possible to compile it to `musl`
targets.

Another noticeable change is that for HA deployments, the shutdown handler adds a 7 second pre-shutdown delay. After
this delay, the cluster leave (for ephemeral caches) and shutdown procedures will be executed, followed by the already
existing post-shutdown delay. This new 7 second pre-delay is not strictly necessary, but it makes rolling releases in
e.g. K8s a lot smoother without the need for specific additional configuration for readiness probes and such, that can
be messed up pretty easily. 7 additional seconds when doing a shutdown don't hurt and they would be necessary anyway to
have a smooth restart.

Apart from that, internal dependencies like SQLite have been bumped and the Rust version was changed to 2024.

## hiqlite v0.9.1

Fixed a bug for local backup cleanup. In some situations, the `backup_keep_for_local` config variable was not read
properly, and in addition, the path for the cleanup could end up wrong as well. This made it possible that the local
backup cleanup would not work at all in some situations.

## v0.9.0

### Changes

#### Non-Deterministic Functions Overwrite

Any form of `hiqlite::Client::execute_*` will `panic!`, if you use non-deterministic functions inside your DB modifying
queries. These will always lead to inconsistency in a Raft cluster, if they are not re-written (which is a waste of
resources imho), so they must never be used. This is not considered a breaking change, since they should not have been
used anyway. This feature only acts as a safety-net.

#### List / Fetch Backups

It is now possible to list backups. For local ones, the `hiqlite::Client` provides the possibility to get a
`tokio::fs::File` handle, while S3 backups can be streamed via a `ChannelReceiver`.

This version also changes the way that filenames for local backups are built. The timestamp for the filename will be the
exact same for all local backups in a cluster. This makes downloading via a load balancer a lot easier.

## v0.8.0

This is a rather small release. Mostly only breaking because of a small API change inside `hiqlite-wal`, which now can
resolve all `LockFile` situations automatically. This means that for `hiqlite`, the config variable `wal_ignore_lock`
has been removed. It's not needed anymore.

Apart from that, you get 2 new variables you can use to define the listen address for the API and Raft servers. This
solves an issue in IPv6-only environments, and makes it possible to bind to a specific IP only instead of the default
listening on all interfaces from before.

```toml
# You can set the listen addresses for both the API and Raft servers.
# These need to somewaht match the definition for the `nodes` above,
# with the difference, that a `node` address can be resolved via DNS,
# while the listen addresses must be IP addresses.
#
# The default for both of these is "0.0.0.0" which makes them listen
# on all interfaces.
# overwritten by: HQL_LISTEN_ADDR_API
listen_addr_api = "0.0.0.0"
# overwritten by: HQL_LISTEN_ADDR_RAFT
listen_addr_raft = "0.0.0.0"
```

## hiqlite-wal v0.7.1

The `hiqlite-wal-v0.7.0` had a bug when truncating WAL logs and shifted the front offset instead of the back. This could
happen, if a leader goes down with a already added, but not fully commited log entry. In such a case, the old leader
would need to truncate the Logs until the last fully commited point.

## hiqlite v0.7.1

The only purpose of this release is to fix a docs build error on `docs.rs`.

## v0.7.0

### Breaking

#### `rocksdb` deprecated

Rocksdb is a really good and fast KV database, but it comes with quite a few issues. The first big thing is, that it's
pure overkill to only be used as a Raft Logs Store. Logs come in sequential order, are immutable, and append-only.
Rocksdb is a huge dependency and it takes a long time to compile. It also adds ~7mb of release binary size and is almost
impossible to compile to `musl`.

To overcome these issues, `hiqlite-wal` has been created from the ground up. It provides memory-mapped WAL files, that
are append-only and perfectly serve the purpose of a Raft Log Store. It is very light-weight in terms of code-, binary
size and compile time, and only provides the functionality we actually need. If also has implementations that try to
auto-recover lost WAL records in case of an application crash in the middle of writing. This is a situation you could
not easily get out of with `rocksdb` as well. Even if it fails recovering everything it needs, it will at least make
everything work on its own in probably almost all cases, and only if it cannot, you at least have the possibility to
easily fix it.

The default Log Store is `hiqlite-wal`, but you can keep on using Rocksdb (at least for some time) with the `rocksdb`
feature. In future versions, Rocksdb will probably be removed completely. If you want to upgrade an existing database,
you can enable the `migrate-rocksdb` feature, which will trigger a Log Store migration during start up. Hiqlite will
then check if it can find an existing Rocksdb, migrate all Logs it can find, and then remove the old Rocksdb files.  
**CAUTION:** Even though this migration has been tested on quite a few instances without any issues, you really should
have a backup before doing it. For a single instance, this is not too important, but for a distributed, already existing
cluster it definitely is!

When you have a setup that for instance runs inside containers and a volume can never be mounted to multiple pods, you
probably want to enable `wal_ignore_lock` / `HQL_WAL_IGNORE_LOCK`, which handles a start after a crash automatically.
But if it may be possible, that a crashed process my still be running somehow, and accessing the database files, you
need to handle this manually and delete the lock file after you killed that process.

### Changes

#### Config as TOML

In addition to reading your whole config from ENV vars, you can now also `NodeConfig::from_toml()` with the new `toml`
feature enabled, which is the case by default now. Take a look at
the [hiqlite.toml](https://github.com/sebadob/hiqlite/blob/main/hiqlite.toml) for an example.

All values read from a TOML file can be overwritten by the matching ENV var. If both values are given, the ENV var will
have the higher priority. This makes it possible to have defaults in your TOML and temporarily overwrite something for
whatever reason.

#### Stability Improvements

Many small fixes and additional checks have been added in lots of places to improve the shutdown / restart stability
with an in-memory only Cache layer. The main issue with this is, that the Raft Logs are ephemeral, which could lead to
many different issues. However, at least to my knowledge, all the existing issues and edge cases has been taken care of
and it should be perfectly stable now to use in-memory only Raft Logs.

#### Distributed Counters

With the `counters` feature, which depends on `cache`, you can now have distributed, raft-backed counters for things
like rate-limiting for instance. The `hiqlite::Client` exposes some new functions to work with them:

- `counter_get()`
- `counter_set()`
- `counter_add()`
- `counter_del()`
- `clear_counters()`

#### `jemalloc` feature

You can enable the `jemalloc` feature and Hiqlite will pull in `jemalloc` as the global allocator.

#### `hiqlite-wal` config options

The new `hiqlite-wal` provides some new config options. You can set a custom `log_sync` / flush strategy depending on
your needs, set the `wal_size` for WAL files, and `cache_storage_disk` will define if for the Cache layer, either an
in-memory Raft Log Store will be used, of if true (default now), WAL + Snapshots for the Cache will be written to disk.
This brings the possibility to have persistent caches, that can be rebuilt from disk even after a complete shutdown. The
Cache / KV store itself will still be in-memory only and therefore have very fast read access, even though writes will
be limited by your disk speed.

More information can be found in the example [hiqlite.toml](https://github.com/sebadob/hiqlite/blob/main/hiqlite.toml).

## v0.6.0

### Breaking

tl;dr

For the a Cache enum, when you had before:

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, hiqlite::EnumIter, hiqlite::ToPrimitive)]
enum Cache {
    One,
    Two,
}
```

You now only need:

```rust
#[derive(Debug, strum::EnumIter)]
enum Cache {
    One,
    Two,
}

impl CacheIndex for Cache {
    fn to_usize(self) -> usize {
        self as usize
    }
}
```

And for the migrations, before:

```rust
#[derive(rust_embed::Embed)]
#[folder = "migrations"]
struct Migrations;
```

You now need:

```rust
use hiqlite_macros::embed::*;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;
```

And `hiqlite::params` has been moved into `hiqlite_macros::params`.

**The long version:**

`hiqlite::params` macro has been removed and is now inside a new crate `hiqlite-macros`. This made it possible to not
need the manual import of `hiqlite::Param` each time when using this macro.

Additionally, you don't need to add `rust_embed`, `num-traits` and possibly `num-derive` macros manually anymore.
`num-traits` and `num-derive` have been dropped completely and instead of adding these external dependencies to your
application, you now need a tiny, usually 5 lines long, boilerplate `impl CacheIndex for MyCacheEnum`. The re-export of
the `strum` macro from `hiqlite::` has been removed as well, because it needed the dependency manually anyway, which was
weird to use.

The `rust_embed` dependency is now also re-exported from `hiqlite-macros` in a way that actually works without you
needing to add it manually.

The `serde::Serialize` + `serde::Deserialize` trait bounds for your `Cache` index enum have been removed as well.

On the long run, the idea is to also be able to successfully re-export `strum`, which cannot be done currently because
of some unfortunate limitations, or create our own macro to derive the `Iterator` trait. But for now, this is quite a
big improvement already.

### Changes

- Support for fixed-size u8 array conversion.
- You can now refer to the output from statements execute before during a `hiqlite::Client::txn()`
  via `RETURNING` + `StmtIndex`. Take a look at the `sqlite-only` example.
- Shutdown has been made quicker and more resilient, especially for `cache` Rafts which do not have a persistent state
  between restarts.
- Internal code and performance improvements and dependencies have been bumped.
- MSRV has been bumped to `1.85.1`

## v0.5.0

- All internal dependencies have been bumped to the latest stable version.
- If you don't need the `listen_notify` feature for remote clusters, you can now choose to only use
  `listen_notify_local` instead, which will pull in less dependencies.
- The `asm` feature has been removed from `sha2` to make it compile on windows.
- The shutdown delay of 10 seconds is not being applied anymore if you only run a single instance, which usually is the
  case during local development. This greatly improves the DX.
- `hiqlite::Client::query_raw_not_empty()` has been removed as it was redundant. `query_raw_one()` behaves in the same
  way.
- MSRV has been bumped to `1.82.0`

## v0.4.0

### Updates

Rocksdb major version has been updated from `8` to `9.9.3` under the hood. Some other smaller dependencies have been
bumped to the latest versions as well.

### Bugfix

The dashboard cookie did not set a `path` when used in production and therefore browsers would reject it because of its
`__Host-` prefix. The path has been added properly to fix this issue.

## v0.3.3

Fixes an issue where the DB writer would panic because of a mismatch in DB Migrations validation. This could have
happened if migrations are applied between applications starts. The `hiqlite::Client` in that case would only send the
migrations over the network that have not been applied already and optimizes already existing ones away. However, the
additional validation inside the DB writer (to make sure the client did not mess up) was too strict, and it would error.

## v0.3.2

This version will make Raft cluster formation and re-joins of nodes after restarts more robust. Additional checks and
logic have been added to figure out the correct behavior automatically. The tricky part about this for cache nodes is,
that they don't persist their state, which is usually needed for Raft. This makes the auto-setup of cache Raft's more
challenging, but this version seems pretty good in that regard so far.

Additionally, a new optional config variable has been added to set an initial delay for `/health` checks:

```
# Configures the initial delay in seconds that should be applied
# to `<API>/health` checks. During the first X seconds after node
# start, health checks will always return true to solve a chicken
# and egg problem when you want to cold-start a cluster while
# relying on `readinessProbe` checks.
# default: 30
HQL_HEALTH_CHECK_DELAY=30
```

## v0.3.1

This version only bumps the `svelte` dependency for the prebuilt dashboard to fix some build steps and bugs.

## v0.3.0

### Changes

#### New `Client` Functions and `impl`s for `Param`

The `hiqlite::Client` now provides a few more helpful functions and features:

- `put_bytes()` for the cache to be able to just cache raw bytes without any serialization
- `query_raw()` will return the raw `hiqlite::Row`s from the database in cases where you might just want to have results
  without deserializing into a `struct`.
- `query_raw_one()` - the same as above, just returns a single `hiqlite::Row`
- `query_raw_not_empty()` is a DX improvement. Often when you need `query_raw()`, you use it to retrieve a specific set
  of columns or `COUNT(*)`, but you also don't want to check that the `Row`s are not empty before accessing. This
  function will `Err()` if no `Row`s have been returned and therefore reduced boilerplate in these situations.
- The migration process will panic and error early on migration hash mismatches and provide more clear logging and
  information where exactly the issue is.
- The new `query_as_optional()` and `query_map_optional()` return a `Result<Option<T>>` and don't error in case of no
  rows returned.
- `execute_returning()`s return type has been changed to properly return a wrapping `Result<_>` like the others
- `execute_returning_map_one()` and `execute_returning_one()` are available as well now to reduce boilerplate.
- `batch()` will exit and error early, if the writer had issues with bad syntax. Because of the internal design of the
  `Batch` reader, it is impossible to recover from syntax errors. Therefore the whole batch will not be applied and the
  transaction will be rolled back in that case.

In addition, there is now:

- `impl<T> From<&Option<T>> for Param`
- `impl From<&String> for Param`
- `impl TryFrom<ValueOwned> for Option<bool>`

#### Error Handling

`Error::ConstraintViolation` and `Error::QueryReturnedNoRows` have been added to be able to handle errors in downstream
applications more granular.

All query fn's from the client that end with `*_one` and should only return a single row have been changed slightly.
Before, they would ignore any Rows after the first one, which can lead to very nasty bugs in production. They now only
return an `Ok(_)` if exactly a single Row has been returned and will error otherwise.

#### Access Rights Restrictions

To prevent information leakage from a world readable database by default, all Hiqlite folders will be restricted with
proper access rights after creation to make them only accessible by the user.

#### Dynamically Available Dashboard

If you have the `dashboard` feature enabled, you can choose to not provide a `HQL_PASSWORD_DASHBOARD` at startup. This
value was mandatory before and is optional now.

If not given, all routes for the dashboard will not exist. This makes it possible to compile with the `dashboard`
feature set and decide at runtime, if you maybe only want to enable is when necessary.

#### Dashboard Improvements

The dashboard has received a few improvements like resizeable result table columns and a nicer look. It has also been
upgraded to the latest Svelte 5 stable.

A very simple rate-limiting has been added for the dashboard login. Only a single password hashing task can exist at a
time, guarded by a `Mutex`. Concurrent logins will never be needed here and this is a good prevention against
brute-force and DoS at the same time.

I do have `spow` set up for the dashboard login, but it is not in use currently. The reason is that the WASM will not
run in plain HTTP contexts, and it would therefore always require TLS. However, you may not wish to add TLS here because
it is maybe in a physically separate network, or inside its own VPN, or you simply only do a port forward via for
instance `kubectl` to your localhost when you want to access the dashboard. If I can find a workaround for the WASM
issue, I will add `spow` again in the future for additional security.

#### S3 Path Style

You need to set `HQL_S3_PATH_STYLE` to either `true` or `false` now, while `true` was the default before.

#### Sync Immediate

A new config value has been added to `NodeConfig` called `sync_immediate`, with a corresponding optional env var
`HQL_SYNC_IMMEDIATE`. With this value set to `true`, an immediate flush + sync to disk will be done after each single
Raft logs batch. This is a pretty big tradeoff but may be necessary in some situations.

```
# Enables immediate flush + sync to disk after each Log Store Batch.
# The situations where you would need this are very rare, and you
# should use it with care.
#
# The default is `false`, and a flush + sync will be done in 200ms
# intervals. Even if the application should crash, the OS will take
# care of flushing left-over buffers to disk and no data will get
# lost. If something worse happens, you might lose the last 200ms
# of commits (on that node, not the whole cluster). This is only
# important to know for single instance deployments. HA nodes will
# sync data from other cluster members after a restart anyway.
#
# The only situation where you might want to enable this option is
# when you are on a host that might lose power out of nowhere, and
# it has no backup battery, or when your OS / disk itself is unstable.
#
# `sync_immediate` will greatly reduce the write throughput and put
# a lot more pressure on the disk. If you have lots of writes, it
# can pretty quickly kill your SSD for instance.
#HQL_SYNC_IMMEDIATE=false
```

#### Prepared Statement Cache Size

The max cache size for prepared statements is now configurable as well. The default is and has been 1024 before, which
is probably a good size for production usage. However, if you are heavily resource constrained, you now have the
possibility to reduce the cache size via the `NodeConfig`.

### Bugfix

The Backup creation routine has reset the "wrong" metadata after backup creation. This is not an issue during runtime
usually, because it will get overwritten with the correct data again very soon, but could cause issues if the instance
crashes before this can happen. Now, the internal metadata for the newly created backup will be reset correctly instead
of the live database.

## v0.2.1

### Race conditions during rolling releases

A few additional checks and fixes have been applied to fight possible race conditions during for instance a rolling
release of a Kubernetes StatefulSet when using the cache layer. Since it has no persistence, the Raft group formation
could get into state where Kubernetes had to kill one of the containers after a rolling release to get them healthy
again. This should now be fine as it was smooth like expected in the last tests.

### Backups

The backups behavior has been changed slightly. It is still the case that only the current Raft leader will push the
backup to S3 storage (if configured), but each node will create an keep its own local backup. This helps when you don't
have S3 available and will make sure, that you will still have a backup "somewhere" even when you lose a full node.

### Self-Healing

The self-healing tests have been simplified to make them easier to maintain in the future. They do not decide between
different folders being lost because you usually lose the whole volume or nothing at all. So if any issue comes up on a
Raft member node, the easiest solution is to simply delete the whole volume, restart, and let it rebuild anyway.

## v0.2.0

This releases fixes some usability issues of the initial version. It also brings clearer documentation in a lot of
places.

- Removed the `compression` feature from `rust_embed` for better compatibility with other crates in the same project.
- `hiqlite::Client::is_leader_db()` + `::is_leader_cache` are now `pub` and can be used in downstream applications.
- New functions `hiqlite::Client::clear_cache()` + `::clear_cache_all()` to clear caches without a restart.
- A few (on purpose) `panic!`, `assert!` and `expect()` have been removed in favor of returned `Result` errors.
- Hiqlite now always tries to install a `rustls` crypto provider which makes setup easier and less error-prone. It will
  only log a `debug` if it fails to do so, which can only happen when it has been done already.
- Additional type affinity checks for SQLite to be able to convert more things without user interaction, like for
  instance `INT` will be caught as well, even though it is no SQLite type by definition. Basically, the rules from
  3.1 https://www.sqlite.org/datatype3.html are applied. The "correct" SQLite type definitions will always be faster
  though and should always be preferred.
- Additional type conversion for:
    - `bool` - was missing, `true` is now an `INTEGER` of `1` and `0` for `false`
    - `chrono::DateTime<Utc>`
    - `chrono::DateTime<Local>`
    - `chrono::DateTime<FixedOffset>`
    - `chrono::NaiveDate`
    - `chrono::NaiveTime`
    - `chrono::NaiveDateTime`
    - `serde_json::Value`
      The additional `chrono` and `serde_json` types are stored as `TEXT` inside the DB for compatibility with the
      underlying `rusqlite` crate. Using `INTEGER` for `chrono::Naive*` and `Utc` types would be faster more efficient,
      which may be changed in the future. For now, all Date and Time-like types are converted to `TEXT`.  
      _Note:_ These auto type conversions only work when you implement the `From<hiqlite::Row>` and do not work with the
      auto-conversion from deriving `serde::Deserialize`.
- `openraft` has been bumped to `v0.9.16` which solves some issues with a not-so-pretty rolling restart of Kubernetes
  StatefulSets for instance due to a race condition.
- `HIQLITE_BACKUP_RESTORE` env var to restore from a backup has been renamed to `HQL_BACKUP_RESTORE` to match the other
  config vars regarding the prefix.

## v0.1.0

With this first version, it starts to make sense to use Hiqlite in real applications to further stabilize it.  
This first release comes with the following features:

- full Raft cluster setup
- everything a Raft is expected to do (thanks to [openraft](https://github.com/datafuselabs/openraft))
- persistent storage for Raft logs (with [rocksdb](https://github.com/rust-rocksdb/rust-rocksdb)) and SQLite state
  machine
- "magic" auto setup, no need to do any manual init or management for the Raft
- self-healing - each node can automatically recover from:
    - lost cached WAL buffer for the state machine
    - complete loss of the state machine DB (SQLite)
    - complete loss of the whole volume itself
- automatic database migrations
- fully authenticated networking
- optional TLS everywhere for a zero-trust philosophy
- fully encrypted backups to s3, cron job, or manual
  (with [s3-simple](https://github.com/sebadob/s3-simple) + [cryptr](https://github.com/sebadob/cryptr))
- restore from remote backup (with log index roll-over)
- strongly consistent, replicated `EXECUTE` queries
    - on a leader node, the client will not even bother with using networking
    - on a non-leader node, it will automatically switch over to a network connection so the request is forwarded and
      initiated on the current Raft leader
- strongly consistent, replicated `EXECUTE` queries with returning statement through the Raft
    - you can either get a raw handle to the custom `RowOwned` struct
    - or you can map the `RETURNING` statement to an existing struct
- transaction executes
- simple `String` batch executes
- consistent read / select queries on leader
- `query_as()` for local reads with auto-mapping to `struct`s implementing `serde::Deserialize`. This will end up behind
  a `serde` feature in the future which is not implemented yet.
- `query_map()` for local reads for `structs` that implement `impl<'r> From<hiqlite::Row<'r>>` which is the faster
  method with more manual work
- in addition to SQLite - multiple in-memory K/V caches with optional independent TTL per entry per cache
- listen / notify to send real-time messages through the Raft
- `dlock` feature provides access to distributed locks
- standalone binary with the `server` feature which can run as a single node, cluster, or proxy to an existing cluster
- integrated simple dashboard UI for debugging the database in production - pretty basic for now but it gets the job
  done
