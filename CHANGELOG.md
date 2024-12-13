# Changelog

## v0.3.3

Further improvements for node shutdowns during rolling releases.

The shutdown adds a delay on purpose for smoothing out Kubernetes rolling releases and
make the whole process more graceful, because a whole new leader election might be necessary.
The delay will be the `max(1500, sqlite.election_timeout_max, cache.election_timeout_max) * 3`
in ms, and it will be added before shutting down the Raft layer and afterward as well.

In future versions, there will be the possibility to trigger a graceful leader election
upfront, but this has not been stabilized in this version.

## v0.3.2

This version will make Raft cluster formation and re-joins of nodes after restarts more robust. Additional checks and
logic have been added to figure out the correct behavior automatically. The tricky part about this for cache nodes is,
that they don't persist their state, which is usually needed for Raft. This makes the auto-setup of cache Raft's more
challenging, but this version seems pretty good in that regard so far.

Additionally, a new optional config variable has been added to set an initial delay for `/health` checks:

```
# Configures the initial delay in seconds that should be applied
# to `<API>/health` checks. During the first X seconds after node
# start, health checks will always return true to solve a chicken
# and egg problem when you want to cold-start a cluster while
# relying on `readinessProbe` checks.
# default: 30
HQL_HEALTH_CHECK_DELAY_SECS=30
```

## v0.3.1

This version only bumps the `svelte` dependency for the prebuilt dashboard to fix some build steps and bugs.

## v0.3.0

### Changes

#### New `Client` Functions and `impl`s for `Param`

The `hiqlite::Client` now provides a few more helpful functions and features:

- `put_bytes()` for the cache to be able to just cache raw bytes without any serialization
- `query_raw()` will return the raw `hiqlite::Row`s from the database in cases where you might just want to
  have results without deserializing into a `struct`.
- `query_raw_one()` - the same as above, just returns a single `hiqlite::Row`
- `query_raw_not_empty()` is a DX improvement. Often when you need `query_raw()`, you use it to retrieve a specific
  set of columns or `COUNT(*)`, but you also don't want to check that the `Row`s are not empty before accessing.
  This function will `Err()` if no `Row`s have been returned and therefore reduced boilerplate in these situations.
- The migration process will panic and error early on migration hash mismatches and provide more clear
  logging and information where exactly the issue is.
- The new `query_as_optional()` and `query_map_optional()` return a `Result<Option<T>>` and don't error in case of
  no rows returned.
- `execute_returning()`s return type has been changed to properly return a wrapping `Result<_>` like the others
- `execute_returning_map_one()` and `execute_returning_one()` are available as well now to reduce boilerplate.
- `batch()` will exit and error early, if the writer had issues with bad syntax. Because of the internal design
  of the `Batch` reader, it is impossible to recover from syntax errors. Therefore the whole batch will not be applied
  and the transaction will be rolled back in that case.

In addition, there is now:

- `impl<T> From<&Option<T>> for Param`
- `impl From<&String> for Param`
- `impl TryFrom<ValueOwned> for Option<bool>`

#### Error Handling

`Error::ConstraintViolation` and `Error::QueryReturnedNoRows` have been added to be able to handle errors in downstream
applications more granular.

All query fn's from the client that end with `*_one` and should only return a single row have been changed slightly.
Before, they would ignore any Rows after the first one, which can lead to very nasty bugs in production. They now only
return an `Ok(_)` if exactly a single Row has been returned and will error otherwise.

#### Access Rights Restrictions

To prevent information leakage from a world readable database by default, all Hiqlite folders will be restricted with
proper access rights after creation to make them only accessible by the user.

#### Dynamically Available Dashboard

If you have the `dashboard` feature enabled, you can choose to not provide a `HQL_PASSWORD_DASHBOARD` at startup.
This value was mandatory before and is optional now.

If not given, all routes for the dashboard will not exist. This makes it possible to compile with the `dashboard`
feature set and decide at runtime, if you maybe only want to enable is when necessary.

#### Dashboard Improvements

The dashboard has received a few improvements like resizeable result table columns and a nicer look. It has also been
upgraded to the latest Svelte 5 stable.

A very simple rate-limiting has been added for the dashboard login. Only a single password hashing task can exist at a
time, guarded by a `Mutex`. Concurrent logins will never be needed here and this is a good prevention against
brute-force and DoS at the same time.

I do have `spow` set up for the dashboard login, but it is not in use currently. The reason is that the WASM will not
run in plain HTTP contexts, and it would therefore always require TLS. However, you may not wish to add TLS here because
it is maybe in a physically separate network, or inside its own VPN, or you simply only do a port forward via for
instance `kubectl` to your localhost when you want to access the dashboard. If I can find a workaround for the WASM
issue, I will add `spow` again in the future for additional security.

#### S3 Path Style

You need to set `HQL_S3_PATH_STYLE` to either `true` or `false` now, while `true` was the default before.

#### Sync Immediate

A new config value has been added to `NodeConfig` called `sync_immediate`, with a corresponding optional env var
`HQL_SYNC_IMMEDIATE`. With this value set to `true`, an immediate flush + sync to disk will be done after each single
Raft logs batch. This is a pretty big tradeoff but may be necessary in some situations.

```
# Enables immediate flush + sync to disk after each Log Store Batch.
# The situations where you would need this are very rare, and you
# should use it with care.
#
# The default is `false`, and a flush + sync will be done in 200ms
# intervals. Even if the application should crash, the OS will take
# care of flushing left-over buffers to disk and no data will get
# lost. If something worse happens, you might lose the last 200ms 
# of commits (on that node, not the whole cluster). This is only
# important to know for single instance deployments. HA nodes will
# sync data from other cluster members after a restart anyway.
#
# The only situation where you might want to enable this option is
# when you are on a host that might lose power out of nowhere, and
# it has no backup battery, or when your OS / disk itself is unstable.
#
# `sync_immediate` will greatly reduce the write throughput and put
# a lot more pressure on the disk. If you have lots of writes, it
# can pretty quickly kill your SSD for instance.
#HQL_SYNC_IMMEDIATE=false
```

#### Prepared Statement Cache Size

The max cache size for prepared statements is now configurable as well.
The default is and has been 1024 before, which is probably a good size for production usage. However, if you are heavily
resource constrained, you now have the possibility to reduce the cache size via the `NodeConfig`.

### Bugfix

The Backup creation routine has reset the "wrong" metadata after backup creation. This is not an issue during runtime
usually, because it will get overwritten with the correct data again very soon, but could cause issues if the instance
crashes before this can happen. Now, the internal metadata for the newly created backup will be reset correctly instead
of the live database.

## v0.2.1

### Race conditions during rolling releases

A few additional checks and fixes have been applied to fight possible race conditions during for instance a rolling
release of a Kubernetes StatefulSet when using the cache layer. Since it has no persistence, the Raft group formation
could get into state where Kubernetes had to kill one of the containers after a rolling release to get them healthy
again. This should now be fine as it was smooth like expected in the last tests.

### Backups

The backups behavior has been changed slightly. It is still the case that only the current Raft leader will push the
backup to S3 storage (if configured), but each node will create an keep its own local backup. This helps when you don't
have S3 available and will make sure, that you will still have a backup "somewhere" even when you lose a full node.

### Self-Healing

The self-healing tests have been simplified to make them easier to maintain in the future. They do not decide between
different folders being lost because you usually lose the whole volume or nothing at all. So if any issue comes up on
a Raft member node, the easiest solution is to simply delete the whole volume, restart and let it rebuild anyway.

## v0.2.0

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
      underlying `rusqlite` crate. Using `INTEGER` for `chrono::Naive*` and `Utc` types would be faster more efficient,
      which may be changed in the future. For now, all Date and Time-like types are converted to `TEXT`.  
      *Note:* These auto type conversions only work when you implement the `From<hiqlite::Row>` and do not work with the
      auto-conversion from deriving `serde::Deserialize`.
- `openraft` has been bumped to `v0.9.16` which solves some issues with a not-so-pretty rolling restart of Kubernetes
  StatefulSets for instance due to a race condition.
- `HIQLITE_BACKUP_RESTORE` env var to restore from a backup has been renamed to `HQL_BACKUP_RESTORE` to match the other
  config vars regarding the prefix.

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