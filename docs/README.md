# Documentation

Hiqlite is an embedded SQLite that forms a Raft cluster: local reads at SQLite speed, replicated
writes, automatic leader fail-over, self-healing. These pages cover what you do with it and what
happens when you do it. The API reference is rustdoc:

```bash
cargo doc --open -p hiqlite --features full
```

## Start here

1. **[Getting Started](getting-started.md)** - your first node, first write, first read.
2. **[Limits](limits.md)** - the rules you must not break. Short, and worth reading first.
3. **[Configuration](configuration.md)** - every key, what it does, what is fatal.
4. **[Queries](queries.md)** - the data path: writes, reads, transactions, row mapping.

## Pages

- **[Getting Started](getting-started.md)** - shapes, features, node lines, a running node, first
  queries, graceful shutdown.
- **[Configuration](configuration.md)** - code, TOML, and env config; secrets, storage, logging,
  rate limits, TLS, validation, danger switches.
- **[Topology](topology.md)** - cluster size and fault tolerance, voters and learners, leadership,
  elections, geo-replication, growing and replacing nodes.
- **[Geo Replication](geo-replication.md)** - a far node that never leads: setup, hot switchover,
  region loss, watch-outs.
- **[Queries](queries.md)** - writes, transactions, batches, returning statements, reads, params,
  row mapping, migrations, remote clients.
- **[Caches](caches.md)** - caches and TTL, counters, distributed locks, listen and notify, and the
  cache index: why it is positional, and how to rename, remove, or swap a variant.
- **[Backups](backups.md)** - schedule, retention, where files are, the file name prefix and shared
  buckets, manual backups, listing, fetching, restoring from code, verifying, failure modes.
- **[Disaster Recovery](disaster-recovery.md)** - playbooks: a node lost, the cluster lost, fresh
  machines, no S3, restore failures, last resorts, the drill.
- **[Operations](operations.md)** - health and readiness, metrics, catch-up, restarts, dashboard,
  logs, disk, capacity, the routine.
- **[Security](security.md)** - the API secret and its header, secret hygiene and rotation, port
  exposure, TLS, backup encryption, data at rest.
- **[Standalone Server and Proxy](standalone-server.md)** - the binary, the proxy, Kubernetes, and
  the rolling restart behaviour that makes a pod restart smooth.
- **[External State Machine](external-state-machine.md)** - the engine for apps that already own
  consensus.
- **[Performance](performance.md)** - where the cost is, the dials, the symptom table, the limits of
  the design.
- **[Upgrades](upgrades.md)** - rolling releases with no downtime, the changelog `Breaking` section,
  the cold-restart case and its guard, the procedures, Kubernetes, a failed upgrade.
- **[Troubleshooting](troubleshooting.md)** - symptom, what to check, the fix.
- **[Examples](examples.md)** - the runnable projects in the repository and what each teaches.
- **[Glossary](glossary.md)** - the words the rest of these pages use.

## Tracks

- **Evaluating Hiqlite:** getting-started → limits → performance → glossary
- **Adopting it for a project:** configuration → topology → queries → caches → backups → security
- **Running it:** operations → disaster-recovery → troubleshooting → upgrades
- **Fixing something now:** troubleshooting, then the page for the subsystem

## Not in these pages

- Method signatures, generics, and per-method detail: rustdoc.
- The full config key list and defaults: `hiqlite::REFERENCE_CONFIG`.
- What a release changed and what you must do: the changelog.
- Where code lives: the repository's code map for contributors.
