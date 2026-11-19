# Operations

Running a cluster you already have: health, metrics, restarts, the dashboard, and the routine jobs
that keep a node boring.

## Health and readiness

- `/ready` - is this node usable right now. It answers 503 while the node is starting, while it is
  shutting down, and while it is not a member of a healthy group. This is the readiness probe
  target.
- `/health` - is this node healthy. A first failing check is retried once after a short wait,
  because a leader switch takes a moment to settle. That is why a liveness probe with
  `failureThreshold: 2` is the right setting and `failureThreshold: 1` is not.
- `health_check_delay` - a grace window at the very start of a node, during which health checks
  report healthy. It exists so a node that is forming, joining, or catching up is not punished.
- `/ping` answers, `/version` reports the running version.
- From your code: `Client::is_healthy_db`, `Client::is_healthy_cache`,
  `Client::wait_until_healthy_db`, `Client::wait_until_healthy_cache`, and the `_timeout` variants.

Probe settings, including why the readiness period must stay short: `standalone-server.md`.

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
