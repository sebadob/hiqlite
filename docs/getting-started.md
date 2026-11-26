# Getting Started

Hiqlite is an embedded SQLite that forms a Raft cluster: local reads at SQLite speed, replicated
writes, automatic leader fail-over, self-healing. This page takes you from an empty project to a
running node and its first queries.

## Pick your shape

- **Embedded** (the normal case): your binary contains Hiqlite and runs a node. Fastest reads, no
  extra service to operate.
- **Standalone server**: Hiqlite runs as its own process or pod, and your apps talk to it over the
  API. See `standalone-server.md`.
- **Remote client**: your app runs no node and talks to an existing cluster. You lose local reads.
  See `Client::remote` in `queries.md`.
- **Proxy**: one entry point in front of a cluster for clients that cannot reach every node. See
  `standalone-server.md`.

## Add the crate

```toml
[dependencies]
hiqlite = { version = "<pinned>", default-features = false, features = ["sqlite", "toml", "macros"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

`default-features = false` and an explicit feature list keeps your binary honest. The features that
change what you can do:

- `sqlite` - the replicated SQLite state machine and all its queries.
- `cache` - replicated in-memory caches, `cache_storage_disk` decides how they are stored.
- `counters` - distributed counters on top of `cache`.
- `dlock` - distributed locks on top of `cache`.
- `listen_notify` - real-time notifications across the cluster.
- `backup` - scheduled and manual backups, the local cleanup job, and restore from a local file.
- `s3` - on top of `backup`: push backups to S3 encrypted, and clean up the bucket.
- `dashboard` - a small web UI to query the database on the API port.
- `toml` - read your config from a TOML file.
- `macros` - the derives and the `params!` macro.
- `cast_ints`, `cast_ints_unchecked` - read SQLite integers as smaller and unsigned types.
- `shutdown-handle` - graceful shutdown on a signal.
- `jemalloc`, `webpki-roots` - allocator and baked-in root certificates.
- `server` - build the standalone binary. Never enable it when you embed Hiqlite.
- `external-state-machine` - for apps that already own consensus. See `external-state-machine.md`.

`full` turns on the regular cluster features. `default` is `auto-heal`, `backup`, `sqlite`, `toml`.
The default `backup` is the local kind: nothing leaves the node until you add `s3`.

## The node line

A cluster is a list of nodes. Every node knows the whole list, in the same order:

```
<id> <raft address> <api address> [election_disable]
```

- `id` is a `u64` and must be unique. Node 1 does the initial formation work.
- The Raft port carries cluster traffic and must never be public.
- The API port carries client traffic, cluster control, health, and the dashboard.
- `election_disable` marks a node that never nominates itself. See `topology.md`.

The same lines in TOML (`nodes = [...]`) and in env (`HQL_NODES`, separated by `\n`).

## Minimal embedded node

```rust, notest
use hiqlite::{Error, NodeConfig};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = NodeConfig::from_toml("hiqlite.toml", None, None, None).await?;
    let client = hiqlite::start_node(config).await?;

    // Always leave the cluster cleanly.
    let mut shutdown_handle = client.shutdown_handle()?;

    // ... your app ...

    shutdown_handle.wait().await?;
    Ok(())
}
```

`shutdown_handle` needs the `shutdown-handle` feature. If your app already owns a shutdown
sequence, call `client.shutdown().await?` at the end of it instead.

`NodeConfig::from_env()` reads the same values from env vars. `hiqlite::REFERENCE_CONFIG` is the
annotated key list with defaults. See `configuration.md`.

## First write and first read

```rust, notest
use hiqlite::macros::{params, FromRow};

#[derive(FromRow, Debug)]
struct Test {
    id: String,
    num: i64,
    description: Option<String>,
}

client
    .execute(
        "INSERT INTO test (id, num, description) VALUES ($1, $2, $3)",
        params!("id1", 123, "my description"),
    )
    .await?;

let row: Test = client
    .query_map_one("SELECT id, num, description FROM test WHERE id = $1", params!("id1"))
    .await?;
```

Values always travel as params, never as string concatenation. A write goes through the cluster:
on the leader it stays local, on any other node it is forwarded to the leader automatically.

Reads are local SQLite reads. Mapping rows to structs: `FromRow` and `TryFromRow` in `queries.md`.

## Migrations

Migrations are plain SQL files in your project, embedded at compile time, applied in name order on
startup, and run once each:

```rust, notest
use hiqlite::macros::embed::*;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

client.migrate::<Migrations>().await?;
```

`Client::try_migrate` returns an error instead of panicking on an invalid migration.

## A three-node cluster on one machine

```bash
export HQL_NODES="1 127.0.0.1:8100 127.0.0.1:8200\n2 127.0.0.1:8101 127.0.0.1:8201\n3 127.0.0.1:8102 127.0.0.1:8202"
export HQL_SECRET_RAFT=change-me-raft-secret
export HQL_SECRET_API=change-me-api-secret

HQL_NODE_ID=1 HQL_DATA_DIR=./n1 cargo run
HQL_NODE_ID=2 HQL_DATA_DIR=./n2 cargo run
HQL_NODE_ID=3 HQL_DATA_DIR=./n3 cargo run
```

Node 1 forms the cluster. Nodes that come up later join it, fetch the current snapshot and log,
and are serving reads as soon as they are caught up. Nothing is seeded by hand.

Before a node 1 without any Raft state forms a new cluster, it asks the other nodes. It joins as
soon as one of them already belongs to a cluster, and it forms a new one once a majority of the
other nodes answered that they were never initialized. While some of them are unreachable and the
rest is not enough for that majority, it keeps asking, because an unreachable node may hold the
existing cluster. If no other node is reachable at all for 10 seconds, node 1 forms the cluster
alone: that is a fresh cluster whose nodes are started one after the other. A node 1 that lost its
data during a full outage looks exactly the same, so in that case start the other nodes first.

For a node to join, three things must be true: it can reach the Raft ports of the members, its
secrets match, and its data dir is a directory it owns that no other node uses.

## Graceful shutdown

Always. A node that is killed hard rebuilds its whole state machine on the next start, which is
correct and slow. `Client::shutdown` and the shutdown handle hand leadership to another member
first, so a restart does not trigger an election.

## Remote client only

```rust, notest
let client = Client::remote(
    vec!["10.0.0.1:8200".to_string(), "10.0.0.2:8200".to_string(), "10.0.0.3:8200".to_string()],
    false,
    false,
    "change-me-api-secret".to_string(),
    false,
    None,
    None,
)
.await?;
```

Give it every node address, so it finds the leader after a leader change. Reads go over the
network. That is the cost of not running a node.

## Examples

The `examples` folder in the repository has a runnable project for each shape: `walkthrough`,
`sqlite-only`, `cache-only`, `complex-types`, `derive-complex-types`, `bench`,
`external-state-machine`. Each has its own README and its own config.

## Next

`configuration.md` for every key, `queries.md` for the data path, `limits.md` for the rules you
must not break, `topology.md` for cluster shape and leadership, `operations.md` for running it.
