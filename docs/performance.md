# Performance

Measure on your own hardware. Numbers from somebody else's SSD are a story, not a plan, which is why
this page has no tables of measurements.

## Benchmark first

The `bench` example in the repository is the starting point. Three nodes, real network, configurable
concurrency and run length:

```bash
cargo run --release -- cluster -c 4 -r 100000
```

Run it on your disk, your network, and your statement shapes. It is single-insert oriented, which is
the worst case for a replicated write, and the most useful baseline.

## Where the cost is

- A write is acknowledged by a majority of voters. You pay for a network round trip to your slowest
  voter, on every write. Voters in one region, far node marked: `topology.md`.
- There is one writer. Writes are applied in order by a single connection, so writes serialize
  against each other. Batching and transactions exist for this reason.
- Reads are local SQLite reads, on a connection its own thread owns, with cached prepared
  statements. That is where the speed of this design lives. Keep reads local.
- Your disk decides write throughput. Memory and CPU matter after it.
- Single-core speed matters more than core count, because of the single writer.

## Write shape

In order of cost per unit of work:

1. Single `Client::execute` calls: one cluster step each.
2. `Client::batch`: many statements in one cluster step, unprepared, so sanitize your input.
3. `Client::txn`: many statements as one unit, prepared, one cluster step.

A transaction with a hundred inserts is dramatically cheaper than a hundred inserts. If your writes
come in groups, send them as groups.

Large values cost twice: the log entry carries them, and the WAL must fit them. `wal_size` is a
fixed size, not a ceiling. If you insert big blobs, size the WAL for the biggest one, and keep the
WAL to a few files at a time.

## Dials

- `log_sync` - the durability/throughput trade-off. `immediate` waits for the flush,
  `immediate_async` flushes without waiting, `interval_<ms>` flushes on a timer. Interval is the
  default and the fastest; on a single node, use `immediate`. See `configuration.md`.
- `logs_until_snapshot` - a longer window between snapshots buys write throughput and costs disk
  and snapshot time. Tune it with `wal_size`.
- `read_pool_size` - the node's local read connections, one per read actor thread, one per core by
  default. A read is prepared, run and mapped on that thread, so its caches stay warm there. More
  threads than cores buys nothing; fewer is what makes reads queue, and a stream you do not consume
  holds one of them.
- `prepared_statement_cache_capacity` - statements you use repeatedly stay prepared. A statement
  used for the first time is more expensive than the ones after it, which is why a warm process is
  faster than a fresh one.
- `rate_limit_db_rps` / `rate_limit_db_burst` - the ceiling that keeps a saturated disk from turning
  into a leader that looks dead. Set them slightly above your real peak.
- `jemalloc` - faster allocation on the targets that support it, a bit more memory out of the box,
  not available on Windows MSVC targets.
- `cache_storage_disk` - disk-backed caches are consistent and flat in memory; in-memory caches are
  faster and lose their content on a hard kill. `in-memory-snapshots` removes the last disk
  dependency and roughly doubles the memory per entry.

## Cache throughput

A cache write is a cluster step, so it has the same network cost as a database write. A cache read
is a local memory lookup and is very fast. If your cache writes dominate, you are paying Raft costs
for data that may not need replication: decide per cache whether it needs to be replicated at all.

## Symptom table

- Write latency pinned at a floor, throughput flat: you are at your quorum round trip. Batch, and
  check your slowest voter.
- Elections under load, healthy network: writes outrunning the disk. Set rate limits. See
  `operations.md`.
- Reads slow on a fresh process: cold prepared-statement cache. Warm it, raise the capacity.
- Writes fail with a WAL sizing complaint: `wal_size` too small for your biggest statement.
- Memory climbing with cache entries: no TTL, in-memory snapshots, or a cache you never clear.
- A restart that takes minutes: the node is rebuilding its state after a hard kill. Fix the
  shutdown, not the startup. See `operations.md`.

## Limits of the design

One writer, a majority in the write path, and a single-node ceiling on write throughput: these are
the shape of replicated SQLite, not a bug to tune away. When you reach them, the answers are
batching, smaller transactions, faster storage, and fewer voters in the quorum - in that order.

The rules that never move: `limits.md`.
