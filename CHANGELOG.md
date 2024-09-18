# Changelog

## UNRELEASED

This releases fixes some usability issues of the initial version. It also brings clearer documentation in a lot of
places.

- Removed the `compression` feature from `rust_embed` for better compatibility with other crates in the same project.
- `hiqlite::Client::is_leader_db()` + `::is_leader_cache` are now `pub` and can be used in downstream applications.
- New functions `hiqlite::Client::clear_cache()` + `::clear_cache_all()` to clear caches without a restart.
- A few (on purpose) `panic!`, `assert!` and `expect()` have been removed in favor of returned `Result` errors.
- Hiqlite now always tries to install a `rustls` crypto provider which makes setup easier and less error-prone.
  It will only log a `debug` if it fails to do so, which can only happen when it has been done already.
- Additional type affinity checks for SQLite to be able to convert more things without user interaction, like for
  instance `INT` will be caught as well, even though it is no SQLite type by definition. Basically, the rules from
  3.1 https://www.sqlite.org/datatype3.html are applied. The "correct" SQLite type definitions will always be faster
  though and should always be preferred.
- Additional type conversion for:
    - `bool` - was missing, `true` is now an `INTEGER` of `1` and `0` for `false`
    - `chrono::DateTime<Utc>`
    - `chrono::DateTime<Local>`
    - `chrono::DateTime<FixedOffset>`
    - `chrono::NaiveDate`
    - `chrono::NaiveTime`
    - `chrono::NaiveDateTime`
    - `serde_json::Value`
      The additional `chrono` and `serde_json` types are stored as `TEXT` inside the DB for compatibility with the
      underlying `rusqlite` crate. Using `INTEGER` for `chrono::Naive*` and `Utc` types would be more efficient, which
      may
      be added as a possibility in the future.  
      Note: These auto type conversions only work when you implement the `From<hiqlite::Row>` and do not work with the
      auto-conversion from deriving `serde::Deserialize`.
- `openraft` has been bumped to `v0.9.16` which solves some issues with a not-so-pretty rolling restart of Kubernetes
  StatefulSets for instance due to a race condition.
- `HIQLITE_BACKUP_RESTORE` env var to restore from a backup has been renamed to `HQL_BACKUP_RESTORE` to match the other
  config wars regarding the prefix.

## v0.1.0

With this first version, it starts to make sense to use Hiqlite in real applications to further stabilize it.  
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