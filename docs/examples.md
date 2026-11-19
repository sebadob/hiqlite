# Examples

Every example is a runnable project with its own README and its own config, in the `examples`
folder of the repository. Start from the folder, not from this page: code changes, and a pointer
that cannot rot beats a copy.

- `walkthrough` - the basic cluster: three nodes, most options hardcoded, `--node-id` on the
  command line. The first thing to run.
- `sqlite-only` - the database path with a single node: writes, reads, migrations. No cache, no
  locks, no notifications.
- `cache-only` - a single node with caches only: no SQLite state machine, no volume needed.
- `derive-complex-types` - row mapping with the derives, including the `column` attributes for
  names, conversions, and the trickier column types.
- `complex-types` - the same mapping by hand, with your own `From` and `TryFrom` implementations.
- `bench` - a write-throughput benchmark for comparing your setups: concurrency, run length,
  transactions. See `performance.md`.
- `external-state-machine` - the opt-in engine for apps that own their consensus log, with a mocked
  log: sequence, operations, retries, snapshot, restore. See `external-state-machine.md`.

```bash
git clone https://github.com/sebadob/hiqlite
cd hiqlite/examples/walkthrough
cargo run -- --node-id 1
```

Each example is a workspace member, so the `hiqlite` dependency is the code in this repository. To
run an example against a published version, change its `Cargo.toml` to a version and drop the path
dependency.

The configs in the repository root (`hiqlite.toml`, `hiqlite.env`) are what the examples read.
`hiqlite::REFERENCE_CONFIG` explains every key in them.
