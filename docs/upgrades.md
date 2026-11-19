# Upgrades

## Rolling is the default

Hiqlite is designed to be upgraded without downtime. A rolling release - one node at a time, the
same mechanism that makes a pod restart smooth - is the normal path, and the one you should plan
for. Data written by a version stays readable by newer versions, the storage format is kept
compatible across releases, and a restarted node rejoins and catches up from the leader.

## The exception is in the changelog

A release that cannot be rolled says so in the `Breaking` section of the changelog. That is the only
check that changes your procedure, and it is the reason the changelog is the first thing you read
before every upgrade: what is new, what broke, what you have to do.

An example of such a release is v0.17. It replaces the Raft network layer, so a node of that version
cannot exchange cluster messages with members that run an older version. Mixing them gives you a
cluster that cannot replicate, and an inconsistent state is what you are left with. v0.17 therefore
needs a cold restart of the whole cluster.

There is no general trick that makes a mixed-version cluster work: not a node at a time, not a
shadow cluster, not a blue/green pair sharing the same volumes. When the changelog says cold
restart, it means every node at once.

## The guard for that migration

The v0.17 migration ships a startup check, so a missed changelog entry fails loudly instead of
quietly. On start, a node asks every other node in its node list for its `/version`:

- A member that does not answer is treated as shut down, which is what the check expects.
- A member that answers must already run the new version. An older version, or a member whose
  `/version` endpoint does not exist, makes the node refuse to start with a message telling you to
  shut the whole cluster down first.
- The check runs twice, with a short delay, to catch a member that comes up right after the first
  pass.
- A single node has no peers to check, so the check is free.

`HQL_DISABLE_COLD_RESTART_GUARD=true` skips it. It exists for tests, and skipping it on a release
that needs a cold restart is how you get the broken cluster the guard is there to prevent. The check
belongs to that migration, not to upgrades in general.

## Rolling upgrade: the procedure

1. Read the changelog entry for the target version, and every version in between. Look specifically
   for a `Breaking` entry that names a cold restart.
2. Take a backup, and know that it restores. Rolling is safe; a verified backup is the insurance.
3. One node at a time. Graceful shutdown, node rejoins, node is healthy and caught up, then the
   next one.
4. Never take two nodes out of a three-node cluster at once. The majority has to survive every step.
5. Expect a leadership handover on each restarted node. That is the shutdown doing its job, not a
   problem.
6. Watch while you roll: `GET /cluster/membership`, the leader in `GET /cluster/metrics/sqlite`, a
   log position that keeps moving, no elections you did not cause.
7. Finish with `/version` on every node, so nothing was left behind.

In Kubernetes, the StatefulSet in `standalone-server.md` is already configured for this. Changing
its image tag is a rolling upgrade, and that is the normal case.

## Cold restart: the procedure

Only for a release whose changelog demands it.

1. Read the `Breaking` entry, and follow what it says.
2. Take a backup and verify it. See `disaster-recovery.md`.
3. Announce the window. Writes stop for the duration of the restart.
4. Stop every node, gracefully. Let each one finish its shutdown; do not kill them.
5. Replace the binary or the image tag on every node. Pin the tag.
6. Start the nodes. Node 1 first is fine, and members that come up while the leader is forming
   simply join.
7. Check: `GET /cluster/membership` on each node, the leader in `GET /cluster/metrics/sqlite`,
   `/ready` on all of them.
8. Write a row, read it back on a follower, and watch the log for a leader change you did not cause.

In Kubernetes, do not roll a release that needs a cold restart. Set `updateStrategy: type: Recreate`
for that rollout, or scale to zero, change the image, and scale back up.

## Data compatibility

Data written by a version stays readable by newer versions. Existing files are not rewritten by an
upgrade, and the storage format is kept compatible in both directions, which is what makes rolling
releases possible at all.

Going back to an older version is not promised. Plan forward-only, and keep the last working tag
available for a roll-back of the binary only if the data has not been touched by the new version.

## Config changes

A release can rename a config key, change a default, or move a feature flag. The check is cheap:
diff your config against `hiqlite::REFERENCE_CONFIG` of the new version, and let the node tell you.
An invalid config panics at startup with the key named, which is the fastest possible feedback.

Feature flags are Cargo features. A release that moves a feature out of `default` changes what your
binary does without changing a line of your code, so diff your feature list too.

## If the upgrade fails

Do not improvise on the data. The order is: stop everything, restore the verified backup onto node 1
alone with the restore variable set on every node, verify, bring the rest of the cluster up on the
version that worked. See `disaster-recovery.md`.

## Toolchains

The supported Rust version lives in the crate manifest. `just msrv-verify` is how the repository
checks it. Nothing in your application depends on a Hiqlite version supporting your toolchain.
