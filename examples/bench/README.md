# Simple Insert Benchmark

This is a very simple "benchmark" to compare different configurations and setups. It's only purpose is to check for
improvements or degradations when the config or setup is changed.

This is almost only about insert performance, since it is the crucial factor with a Raft and SQLite. Reads are always
local (apart from consistent reads on purpose) and other existing network databases don't come even close to the speed
in that case.

All inserted data is prepared and kept in memory before the actual `INSERT`s start to influence the tests as little as
possible. For the same reason, statement logging is disabled as well.  
A `cluster` run writes through the embedded client of node 1, which is the shape Hiqlite is deployed in: the client
lives inside the node process, and its writes go straight into that node's own Raft.

We activate the `jemalloc` feature, which is quite a bit faster than glibc `malloc`. For cache performance, remember
that this example runs the Cache fully in-memory, Snapshots included: it builds with the `in-memory-snapshots` feature,
so the Cache group never touches a disk and its numbers measure replication, not storage. Disk-backed caches provide a
lot more consistency and can even rebuild the whole in-memory cache from the WAL + Snapshot on disk, which means even a
restart does not make you lose cached data. A pure in-memory version is a lot faster though.

## The Shape Of A Run

`cluster` runs node 1 and the benchmark client in the parent process and spawns nodes 2 and 3 as separate OS
processes. That is the shape a real deployment has, and it is the shape whose replication numbers mean something. A
remote client pays a network round trip for every write, and that cost buries the replication cost a run is supposed
to measure.

```
cargo run --release -- cluster -c 64 -r 1000000
```

Each follower is started as `bench node --id N`, and you can run that yourself to assemble a cluster by hand across
machines. A node stays up until the process that spawned it terminates it. The run starts only once both groups have
a leader, because a group without one answers every write with `ForwardToLeader`, and a fresh cluster elects its DB
group and its Cache group at different moments.

- `--node-threads` is the tokio worker budget of every node process, node 1 and its client included. A node is
  CPU-bound on its apply path, so give every node at least one core of its own.
- `--node-cpus` pins the node processes, one core list per node in node id order, space separated, e.g.
  `--node-cpus "0-4,16-20 5-9,21-25 10-14,26-30"`. The first entry pins node 1 together with the client, the rest pin
  the spawned followers. Linux only: node 1 is pinned with `sched_setaffinity`, the followers are started under
  `taskset -c`. This is the only real core pinning there is.

Pin whole physical cores to a node. On a machine with hyperthreading, the two threads of one core are usually
numbered `N` and `N + physical_cores`, so on a 16 core box the siblings of core 0 are 0 and 16. A range like
`0-9 10-19 20-29` looks clean and is not: it hands the second node the second hyperthread of cores 0 to 3, which the
first node already owns. Two nodes on one physical core share its execution resources, and every per-node budget
becomes meaningless. Give each node both siblings of a set of cores, and check a core with
`cat /sys/devices/system/cpu/cpu0/topology/thread_siblings_list`. On a NUMA box, keep a node inside one NUMA node as
well, with `numactl --cpunodebind=N --membind=N`.
- Each node writes its own log to `data/node_<id>.log`, so a node that dies during startup is readable.

Core advice: the efficiency cores of an Apple chip are around 3.5x slower than the performance cores, and macOS has no
thread affinity. There, size the budgets to the number of performance cores and run nothing else. A QoS clamp is not
the lever: `taskpolicy -c maintenance` throttles a process onto the efficiency cores, it does not pin it to the
performance cores. On Linux, pin with `--node-cpus` and the budgets are yours to pick.

## Sweeping Raft Depth

`sweep.sh` runs the same binary over a grid of replication depths and `max_payload_entries`, so one build covers the
whole sweep. The depth is read from `HQL_RAFT_STREAM_DEPTH`, which is a temporary knob for exactly this measurement.

```
./sweep.sh                                   # 1000000 rows, concurrency 64, 1 rep, payloads 128 256 512
./sweep.sh 1000000 64 1 128 256 512         # the same, spelled out
DEPTHS="1 8 32" NODE_CPUS="0-4,16-20 5-9,21-25 10-14,26-30" ./sweep.sh 1000000 64 2 128 256 512
```

Everything else is an env knob: `DEPTHS` (default `1 32`), `NODE_THREADS` (8), `NODE_CPUS` (unset), `BENCH_ARGS`
(extra bench flags, e.g. `--cache-only`), `BENCH` (the binary), `CELL_TIMEOUT` (1800 s per cell). Output lands in
`sweep/<timestamp>/sweep.log` with every cell and its exit code, and `sweep/<timestamp>/table.txt` with the per-payload
averages. A cell that fails is recorded and the sweep continues.

Nothing else should run while a sweep runs. A cell is only comparable when its nodes own their cores.

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

```bash
cargo run --release -- single -c 1 -r 1000 \
    --cache-rps 100 \
    --cache-burst 200 \
    --db-rps 100 \
    --db-burst 200 
```

```bash
cargo run --release -- cluster -c 64 -r 1000000 \
    --cache-rps 20000 \
    --db-rps 20000
```

> Note: When you test with rate-limiting, the transactional inserts in this basic benchmark are disabled.
> The reason is that you might want to test for stability with a very high number of rows to insert, so that the test
> runs for multiple minutes, maybe. The current transactional insert tests are built in a way that they would exceed
> WAL sizes very quickly.
>
> Also, when you rate-limit too heavily, it might happen that the cache GET tests fail after insert, simply because
> they expired.

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
