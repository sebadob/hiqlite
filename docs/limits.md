# Limitations

The hard rules of the system. Each one costs you something if you ignore it, and each one is
checkable in your own code.

## Writes

- Non-deterministic functions are refused in any statement that modifies the database.
  `now()`, `random()`, `randomblob()`, `date()`, `datetime()`, `time()`, `strftime()`,
  `julianday()` and friends panic on the write path, on purpose: a value that differs between
  nodes would diverge the cluster. Pass the value as a param from your code. On read-only
  statements they are fine.
- There is one writer. Statements are applied in cluster order by a single writer, so a long
  transaction blocks other writes for its duration. Prefer many short transactions over one big
  one, and batch inserts.
- A write is acknowledged by a majority of voters. The latency of your slowest voter is inside
  every write. Keep voters in one region and mark a far node, see `topology.md`.
- A transaction is one unit of work for the cluster: it is applied atomically, and you cannot keep
  it open across calls. If you need a lock over several calls, take a distributed lock.
- Two-phase commit with an external system is not a thing here. Nothing makes a write atomic
  against a queue, an S3 object, or another database.

## Reads

- A local read on a follower can be slightly behind the leader. When that is not acceptable, use
  `Client::query_consistent` or `Client::query_consistent_map` (Caution: very expensive - stops the
  whole raft for that query).
- A consistent read asks the leader to confirm the current state. It is a round trip and it is
  slower than a local read. Do not put it in a hot loop.
- Reads are as fast as local SQLite reads, which is the point of the project. Keep them local.

## Versions

Upgrades roll one node at a time, with no downtime. A release that cannot roll says so in the
`Breaking` section of the changelog, and for that release the whole cluster is stopped and restarted
together. See `upgrades.md`.

Data written by a version stays readable by newer versions. Going back to an older version is not
promised. Plan forward-only.

## Data

- SQLite integers are `i64`. Unsigned and smaller columns need the `cast_ints` or
  `cast_ints_unchecked` features, and `cast_ints_unchecked` assumes you only insert values in
  range.
- Backups cover the SQLite database. Cache groups are not backed up; a cache node is rebuilt from
  its peers.
- Data on disk is a plain SQLite file. Encryption of the volume is your job. Network traffic is
  authenticated by secrets and can be TLS encrypted; backups are encrypted with your `enc_keys`.
- Log ids are finite. You will not reach the limit in a normal lifetime of a cluster, and a restore
  moves the log to a new range anyway.

## Caches

- Cache keys are strings. Values are serialized. A TTL is per entry, per cache.
- A disk-backed cache is the safe default: it survives a restart with its content. A fully
  in-memory cache is faster and re-syncs from peers after a restart at the cost of way more memory.
- Keeping cache snapshots in memory needs the `in-memory-snapshots` feature and roughly doubles
  the memory per entry. It exists for cache-only nodes without a volume.
- Cache-only deployments have no SQLite state machine. Everything that reads a database row is
  out of scope there.

## Locks and messages

- Distributed locks have a lease. A live client extends it; a crashed client cannot block a key
  forever. Locks are for the multi-step cases a transaction cannot cover, not for every write.
- `notify()` needs a consumer on every node. Behind a node that never calls `listen()` the
  messages pile up. Delivery guarantees differ: a real cluster member gets every message once; a
  remote client that stopped listening does not catch up.

## Single node

- A single node is a valid deployment for a cache, for a dev box, and for an app that accepts
  losing the host. It is not high availability: the data lives in one place.
- On a single node, set the log sync level to the most durable one. There is no peer to re-sync
  from. See `configuration.md`.

## Operational

- A node that was killed hard rebuilds its state machine from the latest snapshot and the log on
  the next start. That is correct and it costs time. Always shut nodes down gracefully. A shutdown
  of a HA node may take up to 30 seconds.
- The dashboard is basic on purpose, and it works with the `sqlite` feature only.
- The default log level is chatty. Expect a lot of `info` lines around elections, joins, and
  snapshots while the cluster is settling.
- The proxy bounds concurrent streams. It is a hop, not a replica.

## What Hiqlite is not

- Not a SQLite wire-protocol server for external tools and BI.
- Not a multi-master database. There is one leader per Raft group. You can "write" on every node
  though. It will forward the request to the current leader under the hood.
- Not an analytics engine. It is an OLTP store with replicated state.
- Not a queue. `listen_notify` is a notification channel, not a delivery system.
