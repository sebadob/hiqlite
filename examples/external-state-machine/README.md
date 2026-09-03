# External State Machine

This example shows how to use the opt-in `external-state-machine` feature.

It is meant for applications that **already own a consensus log** (their own Raft group, an ordered event stream, ...)
and only want to reuse Hiqlite's SQLite state-machine machinery:

- a single serialized writer
- a read-only connection pool
- exact-retry receipts so lost replies can be recovered without re-executing
- a durable "last applied" frontier persisted in the same SQLite transaction as the mutation
- validated page-image snapshots

It does **not** start a Hiqlite Raft node, network service, membership store, or the dashboard. Only the
`external-state-machine` feature is enabled, all default features are disabled.

The consensus log is mocked with a plain in-memory `Vec`. Replace `MockLog` with whatever already produces
committed entries in a total order in your application.

The example walks through:

1. defining a typed, deterministic `DeterministicSqliteOperation`
2. mapping your log entries to `ExternalCommit`s with a dense `CommitSequence`
3. replaying the log into the engine (fresh start and recovery use the same code path)
4. handling entries that do not touch SQLite with `advance_committed`
5. exact retries and conflicting retries
6. building, validating and installing a snapshot on a second node
7. clean shutdown and reopen

```
cargo run
```
