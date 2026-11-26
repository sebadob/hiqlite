# Operations

Running a cluster you already have: health, metrics, restarts, the dashboard, and the routine jobs
that keep a node boring.

## Health and readiness

- `/ready` - is this node usable right now. It answers 503 while the node is starting, while it is
  shutting down, and while it is not a member of a healthy group. This is the readiness probe
  target.
- `/health` - is this node healthy. It answers 503 while a group has no leader yet, and while a
  group is not running. A first failing check is retried once after a short wait, because a leader
  switch takes a moment to settle. That is why a liveness probe with `failureThreshold: 2` is the
  right setting and `failureThreshold: 1` is not.
- Both endpoints answer 503, never 400, for "not usable right now, ask again later". A 400 says the
  request itself is wrong, and a node that is replaying its log says nothing about your request.
- `health_check_delay` - a grace window at the very start of a node, during which health checks
  report healthy. It exists so a node that is forming, joining, or catching up is not punished.
- `/ping` answers, `/version` reports the running version.
- From your code: `Client::is_healthy_db`, `Client::is_healthy_cache`,
  `Client::wait_until_healthy_db`, `Client::wait_until_healthy_cache`, and the `_timeout` variants.

Probe settings, including why the readiness period must stay short: `standalone-server.md`.

## Startup replay

A node re-applies the log it found on its own disk before it serves anything. While it runs, the API
port can be closed entirely, on a node that restores a snapshot and on the first node of a cluster.
There is no window where `/ready` answers 200 on a state machine that is behind its own committed
log.

What this means for probes:

- A `startupProbe` on `/health` is **required**, with `failureThreshold * periodSeconds` above the
  worst-case replay. A probe against a closed port fails exactly like a slow one, and liveness and
  readiness only start counting once the startup probe passes. Without it, a long replay gets the
  pod restarted mid-replay, and the replay starts over.
- `/ready` answers 503 while the node is re-applying. The first 200 means the replay reached this
  node's persisted `committed` pointer. On a single node that is everything it acknowledged before
  it stopped. In a cluster the pointer is only as fresh as the last WAL flush that carried it, so a
  read at the first 200 can sit behind what this node served before the restart, by up to one flush
  interval, until the leader re-sends the entries it is missing.
- The replay is bounded by **this node's own committed log tail**, never by the cluster tip. A node
  that was offline for a week replays its own log first and catches up from the leader afterwards,
  so `startupProbe` headroom is sized by how much log this node can have pending, not by how far
  the cluster has moved.
- `/health` is liveness only: it answers as soon as the group has a leader, and it never waits for
  the replay. Once the port is open, a long replay cannot make it fail.
- `Client::wait_for_cluster_sync()` waits for the replay first and for the cluster tip second. The
  replay phase needs no peer, so a node whose cluster is unreachable completes it and blocks only on
  the tip. It is the call to make when a read must not go back in time across a restart.

## Damaged log files

An unclean start runs a deep integrity check over **every** WAL file, oldest first. A corrupt record
rolls the log back to the last good one and drops the files behind it, so the log stays a valid
prefix: the replay clamps to the new tail and the missing entries are re-synced from the leader.
Only a rollback drops files. The rollback never goes below the purge frontier, and a node that
would need one refuses to start before deleting anything, so the log directory is still intact.

A WAL file with no readable header is healed only when it is the newest one - that is the shape a
crash leaves behind when it kills the process between creating a new WAL file and writing its header,
and such a file holds no entries. A headerless file behind the newest one is a real hole in the log,
and the node refuses to start and names the file. Restore it from a backup, or start the node with an
empty log directory.

## See the cluster

```bash
AUTH="Authorization: Bearer $SECRET"

curl -s -H "$AUTH" http://10.0.0.1:8200/cluster/membership | jq
curl -s -H "$AUTH" http://10.0.0.1:8200/cluster/metrics/sqlite | jq '.leader, .voters, .learners, .last_log_index'
curl -s -H "$AUTH" http://10.0.0.1:8200/cluster/election | jq
```

- `GET /cluster/membership` - every member of every Raft group of that node.
- `GET /cluster/metrics/{raft_type}` - the state of one group: the leader, voters, learners, the log
  position, the snapshot position. `raft_type` is `sqlite` or `cache`.
- `GET /cluster/election` - which nodes may nominate themselves.
- From your code: `Client::metrics_db`, `Client::metrics_cache`, `Client::cluster_membership`,
  `Client::cluster_election_state`, `Client::is_leader_db`, `Client::is_leader_cache`.

Worth scraping from outside: `/ready`, `/health`, `/version`, `/cluster/metrics/{raft_type}`.
Everything under `/cluster` needs the API secret.

## Catch-up for read-only nodes

A learner serves local reads, and a local read is only as fresh as the log it has applied. Before
you route traffic to a node that just joined:

```rust, notest
client.wait_for_cluster_sync().await?;
client.wait_for_cluster_sync_timeout(Duration::from_secs(30)).await?;
```

These compare the local state to the cluster tip and pass when the gap is small enough to serve.

## Restarts

Order for a planned restart of a whole cluster: leader last is not required, because a graceful
shutdown hands leadership away by itself. What matters is that you do it serially, with the cluster
healthy between the steps, and that a majority is never down at once.

A graceful shutdown does three things you want: the node stops advertising readiness, leadership
moves to another member, and the state is written so the next start is a normal start.

A hard kill costs you a rebuild: the node reconstructs its state from the latest snapshot and the
log on the next start. Correct, and slow. Use `Client::shutdown`, the shutdown handle, and a
grace period that is long enough for the teardown to finish.

Kubernetes specifics, including the probe and grace-period settings: `standalone-server.md`.

## The dashboard

Enabled when `password_dashboard` is set, served on the API port. It runs queries against the local
database of that node, lists tables, and shows metrics.

It is a debugging tool for a running node, and it needs real authentication in front of it.
`insecure_cookie` is for local testing only. If you can avoid exposing it, avoid exposing it.

## Logs

- Expect a lot of `info` output while a cluster settles: elections, joins, snapshots, log
  roll-overs. A quiet log means a quiet cluster.
- `log_statements` logs every statement the node applies. Turn it on for hours, not weeks.
- Alert on: a node that is not ready for more than a few minutes, a leader that changes more than a
  couple of times per hour, a log position that stops moving, a restore that fails, and any panic.

## Disk

What grows, and what to do about it:

- The Raft log grows between snapshots and is truncated when a snapshot is created.
  `logs_until_snapshot` sets the size of that window; a bigger window buys write throughput and
  costs disk. Tune it together with `wal_size`.
- `wal_size` is a fixed size, not a maximum: a statement whose log entry does not fit into the WAL
  fails. If you insert large blobs, size the WAL for them.
- Snapshots accumulate per policy; each one is a copy of the state.
- Local backups are the biggest predictable consumer: your database size, times your retention, on
  every node. See `backups.md`.
- A disk that fills up makes a node stop being able to ack. Watch free space, not just usage.

## Capacity

Rate limits are your stability valve. When writes arrive faster than the disk can ack, heartbeats
start missing and healthy leaders look dead. Set `rate_limit_db_rps` slightly above your real peak
and the cluster degrades by waiting instead of by falling apart.

Signals that you are at the limit: write latency pinned at a floor, election churn under load,
rate-limit waits in your own timings, statements that fail because the WAL cannot hold them. The
levers, in order: batch your writes, raise the rate limit headroom, shorten the snapshot window,
buy a faster disk.

## Routine

- Weekly: look at leader stability, the log positions of each node, free disk, and the backup list.
- Monthly: check that the newest backup in the bucket is from this month.
- Quarterly: run the restore drill on a scratch box. See `disaster-recovery.md`.
- Every release: read the changelog before you touch a node. See `upgrades.md`.
