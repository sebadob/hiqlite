# Backups

Backups exist for one job: recovering a cluster that is gone. A node that dies does not need one -
it rebuilds itself from its peers. If you take exactly one thing from this page, take that.

Backups cover the SQLite database. Cache groups are not backed up: a cache node is rebuilt from the
other nodes.

## Schedule and retention

- `backup_cron` - when the automatic backup runs, cron syntax with a seconds field. Nightly is the
  default shape; pick a time your cluster is quiet.
- `backup_keep_for` - how long the encrypted copies stay in the bucket.
- `backup_keep_for_local` - how long the local copies stay on each node.
- Retention deletes. Set it to how long you can live without, not to a number you like.

## Where backups are

Every node writes a backup into a `backups` directory inside its own `data_dir`. Only the leader
encrypts its copy and pushes it to S3, which is why the object names in your bucket carry the
current leader's node id, and why the set of objects changes after leader switches. That is normal.

Backups are local with the `backup` feature. The encrypted push to S3, the bucket cleanup and the
`s3:` restore source need the `s3` feature on top; a build without it keeps its backups on disk and
restores from a file.

Encryption uses `enc_keys` and `enc_key_active`. A backup is unreadable without the key that made
it. Keep every key you ever used in `enc_keys` as long as a backup made with it exists.

## File names, and sharing a bucket

A backup is always named `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite`, on the local disk and in
the bucket. The prefix is `backup_node` by default and yours to set with `backup_file_prefix` /
`HQL_BACKUP_FILE_PREFIX`.

Set one prefix per application when clusters share a bucket. The cleanup job only ever deletes
objects whose name matches your prefix and the exact shape: a foreign object, an object of a cluster
with another prefix, and even a name that merely starts with your prefix (`backup_node_v2_...`) is
left alone. Listing applies the same rule, so a shared bucket shows you your own backups.

Pick a prefix that is specific to your application, and never reuse one cluster's prefix for a
second cluster that is not a member of the first.

## Manual backup

```rust, notest
client.backup().await?;
```

`Client::backup` starts the backup in the background and returns; it is not finished when the call
returns. `POST /backup` on the API port does the same on a node.

## Backup status

`GET /backup` on the API port reports this node's backups. Like `GET /metrics` below, it needs
`secret_metrics` as `Authorization: Bearer <secret_metrics>` and answers `404` while that secret is
not set: other nodes never call it, so it is meant for monitoring and internal tooling.

```json
{
  "node_id": 2,
  "local": { "count": 3, "newest": { "ts": 1700000000, "file": "backup_node_1_1700000000.sqlite" } },
  "last_backup": { "ts": 1700000002, "file": "backup_node_1_1700000000.sqlite" },
  "last_backup_error": null,
  "s3_latest": { "object": "backup_node_1_1700000000.sqlite", "ts": 1700000000, "confirmed_at": 1700000030 },
  "s3_error": null
}
```

`local` is read from the backups directory, so it survives a restart. `last_backup` and
`last_backup_error` are kept in memory since the node started; their `ts` is when the backup
finished, and an error entry also carries the `error` message.

With the `s3` feature, every node reports the bucket, not only the leader. Only the node that
requested a backup (the leader) uploads it, but every node knows the object name from the log
entry, so every other node watches the bucket for that object after its own local backup is done,
for up to an hour. `s3_latest` is the newest backup a node has seen in the bucket; it is persisted
as `s3_latest.json` in the backups directory, so it survives a restart, and a node that has none
yet looks it up in the bucket once at startup, listing only the objects under its backup prefix.
The watch itself is a single `HEAD` request for the object, with a growing pause between checks. `s3_error` is the last backup this node could not
confirm: an upload that failed for good on the leader, or an object that did not show up in time
on any other node.

## Metrics

`GET /metrics` on the API port returns Prometheus gauges labeled with `node_id`. It is protected by
its own secret, `secret_metrics` (`HQL_SECRET_METRICS`), as `Authorization: Bearer
<secret_metrics>`, and it does not exist (`404`) while that secret is not set. Keep it different
from `secret_api`: a scrape config often lives next to other apps or in git, and `secret_api` can
modify the whole cluster, while `secret_metrics` only reads metrics.

The backup gauges are `hiqlite_backup_local_count`,
`hiqlite_backup_local_newest_timestamp_seconds`, `hiqlite_backup_last_success_timestamp_seconds`,
`hiqlite_backup_last_error_timestamp_seconds` and, with `s3`,
`hiqlite_backup_s3_latest_timestamp_seconds` and `hiqlite_backup_s3_last_error_timestamp_seconds`.
A timestamp of `0` means "never", so an alert like
`time() - hiqlite_backup_s3_latest_timestamp_seconds > 2 * 86400` also fires for a cluster that has
no backup in the bucket at all, and it fires on every node.

## List and fetch

```rust, notest
let local = client.backup_list_local().await?;
let remote = client.backup_list_s3().await?;
for b in remote {
    // b.name, b.last_modified, b.size
}

// Names are `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite`, locally and in the bucket.
let mut file = client.backup_file_local("backup_node_2_1700000000.sqlite")?;
let mut stream = client.backup_s3_stream("backup_node_2_1700000000.sqlite".to_string())?;
```

`Client::backup_list_local` and `Client::backup_list_s3` return name, last modified, and size, for
the names of your prefix only. `Client::backup_file_local` hands you an open file of a local backup;
it refuses a name that is not your exact shape. `Client::backup_s3_stream` gives you a stream of an
object from the bucket, encrypted as stored. Local listing needs a local node: a remote client
cannot list.

## Verify for real

A backup you have never restored is a rumour.

1. List the bucket and pick the newest object.
2. Restore it onto a scratch node: a machine or a container, no peers, `nodes` containing only that
   node with node id 1, `HQL_BACKUP_RESTORE` set to `s3:<object>`, and the encryption keys of that
   era in `enc_keys`. Only node 1 applies a restore, so a scratch node with any other id does
   nothing but wipe its own data dir.
3. Let it start alone, then query it: row counts of your important tables, and something that
   changed after the backup time.
4. Confirm, delete the scratch data, remove the env var, and write down the date you did this.

Do it on a schedule, the same way you do the backups themselves.

## Restore from code, without the env var

The env var is not the only door. `hiqlite::start_node_from_backup(node_config, source)` and
`hiqlite::start_node_with_cache_from_backup(node_config, source)` take the backup as a parameter:
`BackupSource::File(path)` for a local file, `BackupSource::S3(object)` for an object in the bucket
(with `s3`). The env var is not read at all by these two.

```rust, notest
let client = hiqlite::start_node_from_backup(
    config,
    hiqlite::BackupSource::File("/backups/app-a_1_1700000000.sqlite".to_string()),
)
.await?;
```

This is what an app needs when one process runs several Hiqlite instances with their own names and
their own backups: each instance gets its own `data_dir`, its own `backup_file_prefix`, and its own
source, and none of them can read the other's env var.

Everything else about a restore is identical to the env var path: give every node of the cluster the
same source, node 1 applies the backup, the other nodes wipe their state and join the restored
cluster. Start them with it; removing it later is just starting normally again.

## Failure modes

- No S3 config, backups stay local: you have copies that die with the volume they are on.
- A cron expression the parser rejects: the node refuses to start with a message naming
  `HQL_BACKUP_CRON`. Fix it before you deploy.
- A prefix the parser rejects: `backup_file_prefix` with anything outside letters, digits, `.` and
  `-` makes the node refuse to start. A prefix is part of every file name, so a loose one is a
  broken cleanup rule.
- A restore request the build cannot honour: an `s3:` source in a build without the `s3` feature,
  or a value that is neither `s3:` nor `file:`. The node refuses to start and names
  `HQL_BACKUP_RESTORE`. This is the case where a silently ignored restore would be the worst
  outcome.
- The key is gone: a rotated key removed from `enc_keys` makes its backups unreadable. Restore the
  key entry, restart, restore the backup.
- Retention ate the only copy: `backup_keep_for` shorter than the time since your last verified
  restore.
- A leader change during a run: you get the backup of whichever node was leader when it started.
  Nothing is corrupted; you simply have fewer objects than nodes.
- Disk full during a backup: the node reports the failure and keeps running. Check the local
  directory size against your volume.

## Sizing

A backup is a copy of your whole database, made on every node, once per schedule. Multiply that by
your retention on the local side, and by your retention in the bucket. If the numbers are bigger
than your volume, shorten `backup_keep_for_local` before you shorten `backup_keep_for`.

Next: **[Disaster Recovery](disaster-recovery.md)**.
