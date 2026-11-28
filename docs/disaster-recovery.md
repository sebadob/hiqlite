# Disaster Recovery

Two facts decide everything:

- A node that is lost rebuilds itself from the cluster. No backup needed.
- A cluster that is gone is rebuilt from a backup. That is the only moment you need one.

Read the scenario you are in, then follow the steps. Nothing here is time-critical except one
thing: do not start writing to a half-restored cluster.

## One node lost, cluster alive

The cluster still has a majority, so it is healthy. The lost node is the problem, not the data.

1. If the node is still running and confused: stop it.
2. Clear whatever is left of its data dir, or give it a new one.
3. Start it. It joins, fetches the current snapshot and log, and serves reads when it is caught up.
4. If you want to watch it before it votes again, start it once with `HQL_LEARNER_ONLY=true`, check
   its log position, promote it with `Client::promote_node(node_id)`, then remove the env var.

Do not wipe a node that has an intact data dir just to be tidy. A clean node with its files
restarts in place, which is cheaper than a full re-sync.

## The whole cluster is gone

You need a backup. If you have none, you have a data-loss event: stop, decide what you can
recreate, and start a fresh cluster. See `backups.md` for the drill that prevents this.

Restore applies on node 1. Every other node needs the restore variable set as well, and what it does
there is different: it wipes the state it knows - the database and its WAL sidecars, the snapshots,
both log directories, the cache data - and joins the new cluster empty. Nodes that start with their
old files carry the log and the snapshots of the dead cluster, and those never line up with a
restored node 1. That mismatch is the hour you do not want to spend.

Those nodes clear their state before node 1 has applied anything. If you want to keep it until the
restore has worked, set `backup_restore_keep_previous = true`: each node then moves its state into
`{data_dir}/pre_restore`, with the layout of a `data_dir`, and removes it once it is a member of the
restored cluster. Until then, every node needs as much additional disk space as its database,
snapshots and logs take.

1. Confirm every node is stopped. A restore against a running cluster is the one way to make this
   worse. A restore that finds the state machine lock of its `data_dir` held by a live process
   refuses to start and touches nothing.
2. Pick the newest backup. `Client::backup_list_s3` gives you names and timestamps; the newest
   object is usually what you want, and it is the newest one the last leader made. However, you
   probably do not have access to the client if you are in this situation. Just check the file names
   manually.
3. Set the restore source on **every** node, with the same value:

    ```bash
    export HQL_BACKUP_RESTORE="s3:backup_node_2_1700000000.sqlite"
    ```

    For a plain SQLite file you took out of the node, use the file prefix:

    ```bash
    export HQL_BACKUP_RESTORE="file:/root/backup.sqlite"
    ```

    Only node 1 reads the value. On the other nodes the variable does one thing: it clears the state
    they know. A `file:` path has to exist on node 1 only.

4. Check the encryption keys before you start. The key that encrypted the object must be in
   `enc_keys`, and `enc_key_active` must be one of the entries. A backup you cannot decrypt is not
   a backup.
5. Start node 1 alone. It applies the backup, forms the cluster by itself, and finishes the
   restore while it leads: it takes a snapshot, rolls the log, and only then serves its API. Give
   it the time.
6. Verify the data: row counts, and a change you know happened after the backup time.
7. Start the other nodes with the variable still set. Each one clears the state it knows and joins
   node 1.
8. Once every node reports ready and `GET /cluster/membership` is complete, remove
   `HQL_BACKUP_RESTORE` from every node. No restart is needed: it is only read at startup. A node
   that finished a restore records its source in `state_machine/restored_backup` (for a `file:`
   source also the file's size and modification time), so a restart with the same value still
   set logs that and does nothing. To apply the same unchanged backup once more, delete that file
   first.
9. Confirm the shape on each node: `Client::cluster_membership`, and `GET /cluster/membership`.

The restored cluster continues from a new log position above the restored database. You do not have
to line up log ids by hand, and you should not try.

An app that embeds Hiqlite can do the same thing from code: `start_node_from_backup` and
`start_node_with_cache_from_backup` take a `BackupSource` as a parameter and never read the env var,
which is what you need when one process runs several instances with their own backups. Same rule:
every node of that cluster gets the same source. See `backups.md`.

## Fresh machines, new namespace, new region

The restore is the same. What bites you is the config:

- `nodes` must list the new addresses, and node 1 must exist.
- `secret_raft` and `secret_api` must match every node you start.
- `enc_keys` and `enc_key_active` must contain the key of the backup.
- `data_dir` must be new and empty on every node.
- S3 credentials must be able to read the bucket.
- `HQL_BACKUP_RESTORE` must be set on every node you start, with the same value.

New volumes need no manual wiping: a node that starts with the restore variable clears the state
it knows by itself. Start node 1 alone, verify the data, and only then bring the peers up. They join
the leader node 1 has formed; a peer that starts before node 1 has nothing to join.

## No S3 left

Local backups live in each node's `backups` directory, under `backup_node_{node_id}_{ts}.sqlite`.
Take one out with `Client::backup_file_local` while a node is still bootable, put it on a machine
that is not the cluster, and restore with the `file:` prefix.

This path rots: it needs a volume that survives, and a person who knows where the file is. Make it
a documented step, not knowledge.

## The encryption key changed after the backup

Put the old key back into `enc_keys`, restart, restore. `enc_key_active` decides which key new
backups use; it does not limit which keys can decrypt. Keep old entries as long as their backups
exist.

## Restore does not finish

Check in this order:

1. Is the cluster really stopped? A restore against a live cluster fails or half-applies.
2. Is the source name exact? `s3:` needs the object name as listed, `file:` needs a path the node
   can read.
3. Is the encryption key present?
4. Is node 1 the node doing the restore, and did you start it alone? Only node 1 applies a backup,
   and it finishes the restore while it leads the cluster.
5. Do the other nodes have the variable set too? Their old log and snapshots belong to the dead
   cluster, and a node that keeps them cannot join the restored one.
6. Are S3 credentials valid, and is `s3_path_style` right for your provider?
7. Is the data dir writable, and does it contain stale state you meant to clear?

A restore that was interrupted after the backup had been validated, e.g. by a crash or a killed
pod, is finished by the next start of that node, with or without the variable: the validated backup
waits as `state_machine/backups/restore_pending.sqlite` until it replaced the database.

`HQL_BACKUP_SKIP_VALIDATION=true` skips the validation of the restored database. Use it when you
know the file is good and the validator is the thing that is wrong, and understand that you are
removing the last check between you and a broken database.

> `HQL_BACKUP_SKIP_VALIDATION` can also be used to bring an existing plain SQLite db file into
> Hiqlite. You can "restore" from that DB file, skip the validation (you know it's good), and that's
> it.

## No leader after a restore

Usually the node list: a member that is not reachable, an address that changed, a node started twice
with the same id. Read `GET /cluster/membership` on node 1, compare it to your `nodes` config, and
fix the config. Restart the nodes with the fixed list.

A single node that must come back alone starts as a one-node cluster: set `nodes` to that node only,
with node id 1, restore, verify, then rebuild the cluster config. A node with any other id ignores a
restore request in the sense that it does not apply the backup; it only clears its own state.

## Last resort: reset the Raft state

> **CAUTION.** This is the most destructive switch Hiqlite has, and a healthy cluster never needs
> it. The way better alternative is always to let the node re-sync from its members: start it as a
> learner, let it catch up, promote it. If the data is gone, restore from a backup. Use this switch
> for a single node whose Raft log volume is corrupt and which has no healthy peer left to re-sync
> from.

`HQL_DANGER_RAFT_STATE_RESET=true` deletes the Raft logs and the snapshots of both groups, and keeps
your database. Local backups stay. The cache content is gone.

**CAUTION:** This will lead to data losses if not everything from the WAL file was applied to the DB
**AND** you did a clean shutdown! Be VERY careful with this setting. If the node was force-killed,
it will always delete the DB and rebuilt it from snapshot + WAL files to guarantee consistency. If
you, at the same time, set `HQL_BACKUP_SKIP_VALIDATION`, you will end up with all data lost on that
node.

**A changed `CacheVariants` enum never needs this.** See `caches.md`.

## Actions that can lose acknowledged writes

- Restoring a backup that is older than the writes you want to keep. Everything after the backup
  time is gone, including the writes you were told succeeded.
- Starting a stale node as the only node of a new cluster.
- Wiping a data dir you did not have to wipe, on the node that held the most recent log.
- Restoring a subset of nodes and letting clients write to the mixed cluster.

Rule: one source of truth at a time. Restore one node, verify it, then let it bring the others.

## The drill

Quarterly, on a scratch box, on a version you actually run:

- restore the newest backup,
- run three queries you care about,
- write a row, take a backup, restore that,
- note the time it took and the space it needed,
- keep the notes next to the deployment, not in your head.

## What to send when you open an issue

The version, the scenario you followed, the node list, the exact log of the node that failed, the
`GET /cluster/membership` output of each node, and what you had already done. Redact secrets.
