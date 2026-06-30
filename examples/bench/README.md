# Simple Insert Benchmark

This is a very simple "benchmark" to compare different configurations and setups. It's only purpose is to check for
improvements or degradations when the config or setup is changed.

This is almost only about insert performance, since it is the crucial factor with a Raft and SQLite. Reads are always
local (apart from consistent reads on purpose) and other existing network databases don't come even close to the speed
in that case.

All inserted data is prepared and kept in memory before the actual `INSERT`s start to influence the tests as little as
possible. For the same reason, statement logging is disabled as well.  
When data is inserted into a local cluster, it is being done from the leader node, which removes one additional network
round-trip for each insert. A way to decide between leader and follower may be implemented in the future to actually see
the difference.

We activate the `jemalloc` feature, which is quite a bit faster than glibc `malloc`. For cache performance, remember
that we use them in the disk-backed version and not purely in-memory. Disk-backed provides a lot more consistency and
can even rebuild the whole in-memory cache from the WAL + Snapshot on disk, which means even a restart does not make you
lose cached data. A pure in-memory version will be a lot faster though.

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
For instance, by default Hiqlite will trigger a DB snapshot every 10_000 logs which provides a good balance between disk
space used and throughput. If you require higher throughput though, you can trade disk space for it and for instance
increase the threshold to 100_000 logs. Keep in mind that there is a limit to this value which makes sense, because the
higher it is, the longer snapshots and logs purging will take when it happens.

The biggest gains can be made when you can batch inserts into transactions, as you can see from executing the benchmark.

## Rate-Limiting

Hiqlite comes with rate-limiting for all Raft-write actions. This means things like cache PUTs or `.execute*` calls,
everything that writes to the Raft, is being rate-limited. All reads are local (apart from consistent queries), and they
are not affected by this.

By setting appropriate rate-limits for your deployment, which depends on lots of factors like disk IOPS and throughput,
CPU and memory, speed, and so on, you can guarantee that your cluster will always stay stable, not matter how high
requests might spike.

It will not be 100% exactly the value you specify. It might get slightly higher. In additional to the rate-limiting
bucket, there are 2 small overflow buffers in case the limit was hit. When this happens, the call will await the next
refill-tick, as long as the buffer is not full. If it is, an error will be returned immediately to guarantee cluster
stability and avoid increased memory usage.

You can test rate-limiting with this example as well, e.g.:

```
cargo run --release -- single -c 1 -r 10000 \
    --cache-rps 100 \
    --cache-burst 200 \
    --db-rps 100 \
    --db-burst 200 
```

```
cargo run --release -- cluster -c 64 -r 1000000 \
    --cache-rps 20000 \
    --db-rps 20000
```

> Note: When you test with rate-limiting, the transactional inserts in this basic benchmark are disabled.
> The reason is that you might want to test for stability with a very high number of rows to insert, so that the test
> runs for multiple minutes, maybe. The current transactional insert tests are built in a way that they would exceed
> WAL sizes very quickly.

### CAUTION

**This rate-limiting is not enforced on a global level in the current leader node!**

Each client has its own rate-limiter. This is crucial. If the leader enforced it, all limited requests would need to
travel through the network only for getting limited. This means if you have a traffic spike that your deployment cannot
handle, it would still flood the network. Instead, each client enforces this locally BEFORE sending out any network
requests. At the same time, this means if you limit to e.g. 100 RPS while having a 3-node cluster, it will effectively
mean ~300 RPS for the whole cluster (assuming even load balancing).

### Note

If you are doing huge amounts of concurrent INSERTs, you may encounter raft heartbeat timeouts. This is due to a
limitation that the heartbeats are sent with the replication logs and may get delayed too much with very high amounts of
writes. If you need such a throughput, take a look at the example code `node_config()`. You should increase the values
for `heartbeat_interval`, `election_timeout_min` and `election_timeout_max`.

This limitation will go away with the next release of `openraft`, when heartbeats will be sent async and not be included
in the replication anymore.
