# Configuration

A node is configured from code, from a TOML file, or from env vars. All three are equal: the same
keys, the same validation. `hiqlite::REFERENCE_CONFIG` is the annotated key list with every
default, and `hiqlite generate-config` writes it into a file you can edit.

A full reference config can be found in the repo root: `REFERENCE_CONFIG.toml`

## Three ways

```rust, notest
// code
let config = NodeConfig {
    node_id: 1,
    nodes: vec![
        Node { id: 1, addr_raft: "10.0.0.1:8100".into(), addr_api: "10.0.0.1:8200".into() },
        Node { id: 2, addr_raft: "10.0.0.2:8100".into(), addr_api: "10.0.0.2:8200".into() },
        Node { id: 3, addr_raft: "10.0.0.3:8100".into(), addr_api: "10.0.0.3:8200".into() },
    ],
    secret_raft: "change-me-raft-secret".into(),
    secret_api: "change-me-api-secret".into(),
    ..Default::default()
};

// TOML
let config = NodeConfig::from_toml("hiqlite.toml", None, None, None).await?;

// env vars, from the process environment and a `.env` file
let config = NodeConfig::from_env();
```

Every key has a matching env var named `HQL_<KEY>`, except the encryption keys which are
`ENC_KEYS` and `ENC_KEY_ACTIVE`. `NodeConfig::from_env_all` reads a config file, then a file you
name, then the environment. The standalone server reads `$HOME/.hiqlite/hiqlite.toml` and lets env
vars override it; `--node-id` overrides the node id so one file serves every node.

## Node identity

- `node_id` - this node's id. Must appear in `nodes`.
- `nodes` - the full cluster list, every node, same order everywhere.
  Format: `id addr_raft addr_api [election_disable]`. An unknown flag in a node line is a fatal
  config error, on purpose.
- `node_id_from` - `"k8s"` derives the node id from the pod name, so a StatefulSet needs one shared
  config. See `standalone-server.md`.
- `listen_addr_api`, `listen_addr_raft` - what the node binds. Defaults to all interfaces.
- `learner_only` - start as a learner: replicates and serves reads, no vote. Promote it when it is
  caught up. See `topology.md`.
- `election_disable` comes from the node line, not from its own key: a node marked in the list
  never nominates itself. See `topology.md`.

## Secrets

- `secret_raft` - authenticates cluster traffic.
- `secret_api` - authenticates API requests, including your client's.
- Minimum 16 characters each, and use different values. The API secret travels as
  `Authorization: Bearer <secret_api>`, so it lands in every proxy and request log if you let it.
  See `security.md`.
- `secret_metrics` - optional, authenticates the read-only `GET /metrics` and `GET /backup` only.
  At least 16 characters and different from `secret_api`, so a Prometheus scrape config never holds
  a secret that can modify the cluster. Without it, both answer `404`.
- `enc_keys`, `enc_key_active` - encryption for backups and for the dashboard. Mandatory when you
  enable backups, S3, or the dashboard. Keep every key you ever used in `enc_keys` as long as a
  backup made with it exists.

### Secrets file

`secrets_file` points at a second TOML file that mirrors the same table and holds only secret
values. In the main config, set any secret-bearing key to the sentinel `$SECRETS` and the value is
loaded from the file by key name. The secret-bearing keys are `secret_raft`, `secret_api`,
`secret_metrics`, `s3_key`, `s3_secret`, `enc_keys`, `enc_key_active`, `password_dashboard`.

This keeps the config diffable and reviewable, and lets you mount secrets the way your platform
does it: a file at `/run/secrets/hiqlite.toml`, a Kubernetes secret, a systemd credential.

## Storage

- `data_dir` - the node's directory. One node per directory, always. It holds the SQLite database,
  the Raft log, snapshots, cache files, and local backups.
- `filename_db` - the database file name inside it.
- `log_sync` - how the Raft log is flushed. `immediate` blocks until the data is on disk,
  `immediate_async` flushes without waiting, `interval_<ms>` flushes on a timer when buffers are
  dirty. Highest throughput is `interval`, highest safety is `immediate`. On a single node, use
  `immediate`: there is no peer to re-sync from.
- `wal_size` - the write-ahead log size budget.
- `logs_until_snapshot` - how many log entries accumulate before a snapshot. Higher values give
  write throughput and cost disk space and slower snapshot creation. Tune it together with
  `wal_size`.
- `read_pool_size` - the node's local read connections, one per read actor thread. Writes use their
  own single connection, so this only sizes reads. A streaming query takes one of these threads for
  as long as you consume it, so a node runs at most `read_pool_size` streams at once.
  Default: one per core, at least 4 and at most 1024.
- `saturated_pool_threshold` - the queue wait at which the node logs that its read pool is
  saturated. The warning names which problem it is: one query holding a connection for a long time,
  with the time it ran, or more reads than connections, with how many are queued. A pool that stays
  saturated warns once every 10 seconds, not once per query. Default: 100ms.
- `slow_query_threshold` - the time a read may run on its connection before the node logs it as a
  slow query, naming the statement and never its parameters. Independent of
  `saturated_pool_threshold` and normally higher than it: one is how long a read waited for a
  connection, the other how long a read that got one took. A stream counts from its first row to its
  drop, the pauses of its consumer included, because its read actor is held for exactly that long.
  Default: 1s.
- `log_slow_queries` - log the statement text of a read that ran over `slow_query_threshold`. A
  statement built by the `sql!()` macros carries no value in it, and parameters never reach the log.
  A query typed into the dashboard is different: its values are inline in its text, so a slow
  dashboard query writes them into the log - the dashboard is a debugging tool and usually not part
  of a production deployment. Set it to `false` to keep statement text out of the log entirely. The
  saturation warning is unaffected, because it never names a statement. Default: true.
- `prepared_statement_cache_capacity` - how many prepared statements a connection keeps.
- `cache_storage_disk` - disk-backed caches (default) versus in-memory caches. See `caches.md`.
- `HQL_CACHE_META_RESET=true` - deletes the stored cache-index fingerprint and writes it from the
  enum you start with, which is what a renamed cache variant needs. Only that fingerprint is
  deleted: database, Raft logs, and cache content are untouched. Every node keeps its own
  fingerprint, so set it on every node for that start. Env only, then unset. A variant removal
  needs more than this - every node has to clear all caches at startup.
  every node has to clear all caches at startup. See `caches.md`.

## Logging

- `log_statements` - log every SQL statement the node applies. Useful for a few hours of
  debugging, expensive to keep on.
- The node emits tracing spans and events. Your app installs the subscriber, so the format and the
  level are yours. The standalone server takes `-l debug|info|warn|error`.

## Rate limits

- `rate_limit_db_rps`, `rate_limit_db_burst`, `rate_limit_cache_rps`, `rate_limit_cache_burst`.
- These bound the write load you can put into the Raft. When your disk cannot keep up, the cluster
  starts missing heartbeats and leaders look dead while they are healthy. A limit that is slightly
  above your real peak is the cheapest stability you can buy. A burst of about 2x the `rps` is a
  sane start.

## Dashboard

- `password_dashboard` - the stored form of the dashboard password. `hiqlite generate-config -p`
  prints it.
- `insecure_cookie` - cookie settings for plain HTTP testing. Never in production.

## Backups

- `backup_cron` - when the automatic backup runs, cron syntax with seconds.
- `backup_keep_for_local` - retention of the local copies on each node.
- `backup_file_prefix` - the prefix of every backup file name, `backup_node` by default. A backup
  is always `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite`, and only names matching that shape of
  your own prefix are ever listed, cleaned up, and deleted. Set one per application when clusters
  share a bucket. See `backups.md`.
- `backup_keep_for` - retention of the encrypted copies on S3. S3 side only.
- `s3_url`, `s3_bucket`, `s3_region`, `s3_key`, `s3_secret`, `s3_path_style` - all of these need
  the `s3` feature; without it backups stay on the local disk.
- `HQL_BACKUP_RESTORE` - env only, read at startup only. `s3:<object>` or `file:<path>`. Node 1
  applies the backup; every other node that sees the variable clears the state it knows and joins
  empty. Set it on all nodes for a full-cluster restore, and remove it once the cluster is alive. An
  `s3:` value on a build without the `s3` feature is refused at startup instead of being ignored.
  Apps with several instances in one process use `start_node_from_backup` and pass the source
  directly. See `disaster-recovery.md`.
- `HQL_BACKUP_SKIP_VALIDATION=true` - skips the integrity validation of a backup before it is
  applied, and lets you bring an existing plain SQLite file in as a restore source.
- See `backups.md` and `disaster-recovery.md`.

## Raft tuning

`NodeConfig::raft_config` is a `hiqlite::RaftConfig`. The defaults are tuned for a normal LAN and
they work. Touch these only when you know which number you are chasing: heartbeat and election
timing, snapshot policy, the maximum append-entries batch, and the cluster name. The snapshot policy
decides when a snapshot is created; `logs_until_snapshot` caps how big the log grows in between.
`raft_config` is a Rust field, not a key of `hiqlite.toml`: the TOML path always builds the defaults
in `NodeConfig::default_raft_config`, so these numbers are set in code, on the `NodeConfig` you hand
to `start_node`.

`election_timeout_min` and `election_timeout_max` (1500 ms and 3000 ms) are the two numbers behind a
leader change. A leader's lease runs for `election_timeout_max` from the last quorum ack of its
heartbeats, and a node grants a vote only once the lease it relies on has lapsed. A machine loaded
enough to delay those acks past 3 s therefore changes its leader by itself, with nothing wrong
beyond the timing - and a node handed a vote it would normally refuse can then take the seat. Raise
min and max together on shared or busy hardware, keeping the max above the min, which is what
spreads candidates apart; a higher pair also means a slower takeover when a leader really dies.

`api_batch_capacity` and `api_batch_linger_ms` decide how writes are collected on the leader. A
write that arrives while a batch is being built joins it, and the batch goes to the log store in one
shot: one append for the WAL, one result per write. The capacity is 4096 by default and the linger
is 0 ms, which means a batch is exactly what is already queued and no write ever waits for company.
A linger of a few milliseconds packs fuller batches on a hot write path and pays for it with that
much extra latency on a write that arrives at an idle leader.

Two of these are not timing, and they are easy to mix up. `enable_tick = false` pauses heartbeat
sending only. The switch that keeps a node from nominating itself is `enable_elect`, and that is
what the `election_disable` marker of a node line sets. See `geo-replication.md`.

## TLS

- `tls_auto_certificates` - generate self-signed certificates for Raft and API traffic. Clients
  connect with verification off, which is the point: it is encrypted and authenticated traffic
  without a PKI.
- `tls_raft_key`, `tls_raft_cert`, `tls_api_key`, `tls_api_cert` - your own certificates. Key and
  certificate must be given together.
- `tls_raft_danger_tls_no_verify`, `tls_api_danger_tls_no_verify` - accept certificates you did
  not issue. Testing and self-signed setups only.
- `webpki-roots` - baked-in root certificates for minimal containers with no OS trust store.
- See `security.md`.

## Health

- `health_check_delay` - a grace window right after start, during which a health check reports the
  node as healthy. It exists so a node that is still forming, joining, or catching up is not
  punished by probes. See `standalone-server.md`.

## Danger switches

These exist for rare, specific, single-node edge cases. They are not fixes for a cache index, a
stale node, or a leader that will not settle.

- `HQL_DANGER_RAFT_STATE_RESET=true` - **CAUTION: only for a single node whose Raft log volume is
  corrupt and which has no healthy peer to re-sync from.** It deletes the Raft logs, the snapshots
  of both groups, and the cache-index fingerprint. It keeps your database file, and your cache
  content is gone. The way better alternative in a cluster is always to let the node re-sync: start
  it as a learner, let it catch up, promote it. Used on a node of a live cluster, this can leave you
  with an inconsistent cluster. It is env only, and it is named `DANGER` for a reason. See
  `disaster-recovery.md`.
- `HQL_DISABLE_COLD_RESTART_GUARD=true` - skips the version check that refuses a node from joining
  a cluster with older members. A testing switch. See `upgrades.md`.

## Validation

A broken config is fatal at startup and the node panics: an unknown flag in a node line, a node id
that is not in the list, secrets shorter than 16 characters, missing encryption keys when backups
or the dashboard are enabled, a data dir the node cannot write, a port that is taken, an
unparseable cron expression. These need a human fix, and a loud start is the fastest way to get one.
