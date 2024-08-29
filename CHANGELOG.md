# v0.1.0

With this versions, it starts to make sense to use the crate in real applications to further stabilize it.  
This first release comes with the following features:

- full Raft cluster setup
- everything a Raft is expected to do (thanks to [openraft](https://github.com/datafuselabs/openraft))
- persistent storage for Raft logs (with [rocksdb](https://github.com/rust-rocksdb/rust-rocksdb)) and SQLite state
  machine
- "magic" auto setup, no need to do any manual init or management for the Raft
- self-healing - each node can automatically recover from:
    - lost cached WAL buffer for the state machine
    - complete loss of the state machine DB (SQLite)
    - complete loss of the whole volume itself
- automatic database migrations
- fully authenticated networking
- optional TLS everywhere for a zero-trust philosophy
- fully encrypted backups to s3, cron job or manual (
  with [s3-simple](https://github.com/sebadob/s3-simple) + [cryptr](https://github.com/sebadob/cryptr))
- restore from remote backup (with log index roll-over)
- strongly consistent, replicated `EXECUTE` queries
    - on a leader node, the client will not even bother with using networking
    - on a non-leader node, it will automatically switch over to a network connection so the request
      is forwarded and initiated on the current Raft leader
- strongly consistent, replicated `EXECUTE` queries with returning statement through the Raft
    - you can either get a raw handle to the custom `RowOwned` struct
    - or you can map the `RETURNING` statement to an existing struct
- transaction executes
- simple `String` batch executes
- consistent read / select queries on leader
- `query_as()` for local reads with auto-mapping to `struct`s implementing `serde::Deserialize`.
  This will end up behind a `serde` feature in the future which is not implemented yet.
- `query_map()` for local reads for `structs` that implement `impl<'r> From<hiqlite::Row<'r>>` which is the
  faster method with more manual work
- in addition to SQLite - multiple in-memory K/V caches with optional independent TTL per entry per cache
- listen / notify to send real-time messages through the Raft
- `dlock` feature provides access to distributed locks
- standalone binary with the `server` feature which can run as a single node, cluster, or proxy to an existing cluster
- integrated simple dashboard UI for debugging the database in production - pretty basic for now but it gets the job
  done