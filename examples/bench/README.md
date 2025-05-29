# Simple Insert Benchmark

This is a very simple "benchmark" to compare different configurations and setups. It's only purpose is to check for
improvements or degradations when the config or setup is changed.

This is almost only about insert performance, since it is the crucial factor with a Raft and SQLite. Reads are always
local (apart from consistent reads on purpose) and other existing network databases don't come even close to the speed
in that case.

All inserted data is prepared and kept in memory before the actual `INSERT`s start to influence the tests as little as
possible. For the same reason, statement logging is disabled as well.  
When data is inserted into a local cluster, it is being done from the leader node, which removes one additional network
round-trip for each insert. A way to decide between leader and follower may be implemented in the future to actually
see the difference.

We activate the `jemalloc` feature, which is quite a bit faster than glibc `malloc`. For cache performance, remember
that we use them in the disk-backed version and not purely in-memory. Disk-backed provides a lot more consistency and
can even rebuild the whole in-memory cache from the WAL + Snapshot on disk, which means even a restart does not make
you lose cached data. A pure in-memory version will be a lot faster though.

## Running Benchmarks

The most important thing: You should always run these in release mode. The results will be a lot different.

You can run benchmarks on a single node. A single node Raft "cluster" is absolutely valid. The benchmark uses `clap`
for argument parsing, and it should be pretty straight forward to use, but here are some examples:

**Single node with concurrency 1 and 1000 inserts:**

```
cargo run --release -- single -c 1 -r 1000
```

**3 node cluster with concurrency 2 and 10000 inserts:**

```
cargo run --release -- cluster -c 2 -r 10000
```

**3 node cluster with concurrency 16 and 100000 inserts:**

```
cargo run --release -- cluster -c 16 -r 100000
```

**3 node cluster with concurrency 64, 100000 inserts and a snapshot taken every 100000 inserts:**

```
cargo run --release -- cluster -c 64 -r 100000 -l 100000
```

The throughput you can achieve here depends highly on your disk and the snapshot threshold for logs.  
For instance, by default Hiqlite will trigger a DB snapshot every 10_000 logs which provides a good balance between
disk space used and throughput. If you require higher throughput though, you can trade disk space for it and for
instance increase the threshold to 100_000 logs. Keep in mind that there is a limit to this value which makes sense,
because the higher it is, the longer snapshots and logs purging will take when it happens.

The biggest gains can be made when you can batch inserts into transactions, as you can see from executing the benchmark.

### Note

If you are doing huge amounts of concurrent INSERTs, you may encounter raft heartbeat timeouts.
This is due to a limitation that the heartbeats are sent with the replication logs and may get delayed too much with
very high amounts of writes. If you need such a throughput, take a look at the example code `node_config()`.
You should increase the values for `heartbeat_interval`, `election_timeout_min` and `election_timeout_max`.

This limitation will go away with the next release of `openraft`, when heartbeats will be sent async and not be
included in the replication anymore.