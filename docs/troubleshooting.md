# Troubleshooting

Symptom first, then what to check in order. The fix is usually the first thing in the list.

## A node does not start

- A config panic that names a key. A bad node line, an unknown flag, a node id that is not in the
  list, a secret shorter than 16 characters, missing `enc_keys` with backups or the dashboard
  enabled, an unparseable `backup_cron`, and a `backup_file_prefix` that is not a plain file name
  prefix.
  These are fatal on purpose; fix the config.
- A restore request the build cannot honour: `HQL_BACKUP_RESTORE` with an `s3:` source in a build
  without the `s3` feature, or a value that is neither `s3:` nor `file:`. The node refuses to start
  and says so. Restore the source, build with `s3`, or unset the variable - the case you never want
  to discover is a restore that got quietly ignored. See `backups.md`.
- A listen address that cannot be bound: the port is taken by another process, or by the other port
  of the same node.
- A data dir the process cannot write, or a data dir shared with a second node. One node, one
  directory.
- A cache index message that lists a stored variant set and your current one: the `CacheVariants`
  enum changed. A rename needs `HQL_CACHE_META_RESET=true` on every node, for a single start; a
  removal needs that plus a full cache clear on every node. See `caches.md`.
- A cold-restart message about versions: this release needs the whole cluster stopped at once, and a
  member is still running an older version. See `upgrades.md`.
- A lock file left in the data dir: the node was killed hard. The first start rebuilds its state
  from a snapshot and the log. Let it finish.

## A node does not join

- Its Raft port is unreachable from the leader, or the leader's is unreachable from it.
- `secret_raft` differs on the two nodes.
- TLS mismatch: one side has certificates, the other does not; or a certificate the peer does not
  accept, with no-verify off.
- `HQL_LEARNER_ONLY=true` is set, so it joins as a learner and never votes. That is a feature; check
  `GET /cluster/membership` before you call it a bug.
- The node list on the leader does not contain the new node. A leader only knows what is in its
  list. Restart the members with the shared list.
- The node id in the config is not the node id in the node line.

## Leaders keep changing

- Writes outrunning the disk. The cluster is healthy and looks dead because heartbeats get starved.
  Set `rate_limit_db_rps` and `rate_limit_db_burst` above your real peak. See `performance.md`.
- One voter with much worse latency than the others: it is in every write path. Move it out of the
  voters and keep it as a learner.
- A node that is behind the cluster: it cannot become leader, so elections resolve late. Catch it up
  or replace it.
- A forced election you triggered with `Client::elect_node(node_id, true)`, and nobody turned
  self-nomination back off.
- Kubernetes killing a leader because the probes are too strict. Readiness and liveness with
  `failureThreshold: 2`: `standalone-server.md`.

## Writes time out, or a client reports a leader redirect

1. Is there a leader? `GET /cluster/metrics/sqlite` shows it.
2. The redirect payload names the node the client should talk to. A remote client that was given one
   node address cannot follow: give it every node.
3. The node you are talking to is a learner, and only a leader takes writes.
4. Disk full, or a WAL that cannot hold the entry. Check the node log.
5. A majority is down, so nothing can commit. Count your live voters against your node list.

## Auth failures

- The header shape. It is `Authorization: Bearer <secret_api>`. A raw secret without the scheme is
  refused, with a message that says so.
- A proxy that strips the `Authorization` header.
- `secret_api` differs between the client and the node.
- Mixed versions: old and new nodes cannot authenticate each other. See `upgrades.md`.
- The dashboard: it has its own login, its own password, and `insecure_cookie` only for plain-HTTP
  testing.

## A restore does not finish

The order in `disaster-recovery.md` is the checklist. The most common four: the cluster was not
fully stopped, the encryption key of that backup is not in `enc_keys`, the restore was attempted on
a node that is not node 1, and the other nodes were started without the variable, so they kept the
log and snapshots of the dead cluster and cannot join the restored one.

## No leader after a restore

Compare `GET /cluster/membership` on node 1 against your `nodes` config. The usual cause is a member
that is not reachable, an address that changed, or a node started twice with the same id. Fix the
list, restart the nodes.

## Memory grows

- Cache entries with no TTL. Every `put` with `None` is permanent.
- In-memory snapshots with the `in-memory-snapshots` feature: expected, and roughly double.
- A cache you never clear, plus `get_snapshot` calls that copy it.
- `notify()` with no `listen()` on a node. The queue has no bound. See `caches.md`.

## A lock is stuck

The guard is alive somewhere: a client that took `Client::lock` and never dropped it. A lock cannot
outlive its lease window, so a lock that never frees means a live client, not a broken lease. Check
your code paths that can return early without dropping the guard.

## A panic you did not expect

- A `FromRow` type mismatch. The column cannot map to the field, and no retry fixes it. Change the
  type, the column, or use the `try_` query variants. See `queries.md`.
- A non-deterministic function on the write path: `now()`, `random()`, `date()` and friends. Pass
  the value as a param. See `limits.md`.
- A config that cannot work: bad node line, port taken, data dir unreadable. The node refuses to
  start so you find out at startup, not at 3am.
- Anything else: keep the log, keep the node list, and open an issue. A recoverable condition should
  not end in a panic, and the maintainers want to know when one does.

## Disk full

A node that cannot write stops being able to ack, so the cluster cannot commit. Free space first,
then look at what grew: local backups (retention), the log between snapshots
(`logs_until_snapshot`), snapshots themselves, cache files.

## Statements fail with a WAL complaint

`wal_size` is a fixed size, not a maximum. A statement whose log entry does not fit fails. Raise
`wal_size` to fit your largest statement, and keep the total log volume sane.

## Logs are unbearably chatty

`info` is chatty while a cluster settles: elections, joins, snapshots. Raise the level for your
deploy (`-l warn` on the server binary, your own subscriber filter in an app) and keep `debug` for
the twenty minutes when you actually need it.

## Last resort, in this order

1. Restart the node that is wrong, gracefully.
2. Start it as a learner, catch it up, promote it.
3. Replace it: new node as learner, promote, demote the old one.
4. Restore from a verified backup.
5. `HQL_DANGER_RAFT_STATE_RESET=true`, and only for a single node whose Raft log volume is corrupt
   with no healthy peer left to re-sync from. It deletes the Raft log, metadata, and snapshots,
   keeps your database, and the cache content is gone. Steps 2 and 3 are the better fix, and they
   are on the list for exactly that reason; on a live cluster this can leave you with an
   inconsistent cluster whose only way out is a full restore. See `disaster-recovery.md`.

## Before you open an issue

Version, node list, what you ran, the full log of the failing node, `GET /cluster/membership` and
`GET /cluster/metrics/sqlite` from each node, and what you already tried. Redact secrets.
