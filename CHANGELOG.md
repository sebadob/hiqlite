# Changelog

## v0.12.0

### Breaking

The `shutdown_delay_millis` config option was removed. It is not necessary to set it manually anymore. Instead, more
automatic detection is being applied and a necessary delay to smooth out rolling releases or make sure the readiness
of a container is being caught is added without the need for additional config.

Apart from that, lots of improvements have been made to rolling releases and how WebSocket re-connects and node startups
are being handled in general. There is a new `/ready` endpoint on the public API as well. It can be used in e.g.
Kubernetes to smooth out rolling releases and detect a pod shutdown before it becomes unable to handle Raft requests.
To do so, it is important however to not have too high `periodSeconds`, and the `headless` service needs to
`publishNotReadyAddresses` ports before ready, like so:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: hiqlite-headless
  namespace: hiqlite
spec:
  clusterIP: None
  # only do that on the headless service
  publishNotReadyAddresses: true
  selector:
    app: hiqlite
  ports:
    - name: raft
      protocol: TCP
      port: 8100
      targetPort: 8100
    - name: api
      protocol: TCP
      port: 8200
      targetPort: 8200
```

Then you can make use of the new readiness check in the `StatefulSet`:

```yaml
readinessProbe:
  httpGet:
    scheme: HTTP
    port: 8200
    path: /ready
  initialDelaySeconds: 5
  # Do not increase, otherwise a shutdown might start before k8s catches it.
  periodSeconds: 3
  # Require 2 failures because you may get one during a leader switch.
  failureThreshold: 2
livenessProbe:
  httpGet:
    scheme: HTTP
    port: 8200
    path: /health
    initialDelaySeconds: 60
    periodSeconds: 30
    # Require 2 failures because you may get one during a leader switch.
    failureThreshold: 2
```

### Bugfix

The `hiqlite-wal` had a bug where the `last_purged_log_id` was overwritten with `None` during a log truncation, even
if it had a value from a log purge before. If the node restarted before another log purge fixed it, it would result in
an error during startup. The new version includes a check + fix, if you start up an instance with a data set that
currently has this issue.

## hiqlite v0.11.1

Bugfix when reading in the `password_dashboard` via TOML. The base64 decoding step was missing, while it was working
just fine when read via ENV.

## v0.11.0

This is a rather small release. Some external dependencies have been bumped to the latest versions. The biggest change
is the pretty important bugfix below.

### Bugfix

- It was possible to get into situations where the automatic WAL file cleanup was not working as expected, even when
  the log IDs were covered by the latest snapshot. This could lead to the volume filling up endlessly.

## hiqlite v0.10.1

This version removes an `unwrap()` during KV GET operations, that was reachable under some circumstances, for instance
when you cancel an async `hiqlite::Client.get()` before awaiting the result.

## v0.10.0

This is a rather small release. The main thing about it is that `rocksdb` was removed completely after `hiqlite-wal`
has proven to be stable. This makes `hiqlite` a lot more light-weight and makes it possible to compile it to `musl`
targets.

Another noticeable change is that for HA deployments, the shutdown handler adds a 7 second pre-shutdown delay. After
this delay, the cluster leave (for ephemeral caches) and shutdown procedures will be executed, followed by the already
existing post-shutdown delay. This new 7 second pre-delay is not strictly necessary, but it makes rolling releases in
e.g. K8s a lot smoother without the need for specific additional configuration for readiness probes and such, that can
be messed up pretty easily. 7 additional seconds when doing a shutdown don't hurt and they would be necessary anyway to
have a smooth restart.

Apart from that, internal dependencies like SQLite have been bumped and the Rust version was changed to 2024.

## hiqlite v0.9.1

Fixed a bug for local backup cleanup. In some situations, the `backup_keep_days_local` config variable was not read
properly, and in addition, the path for the cleanup could end up wrong as well. This made it possible that the local
backup cleanup would not work at all in some situations.

## v0.9.0

### Changes

#### Non-Deterministic Functions Overwrite

Any form of `hiqlite::Client::execute_*` will `panic!`, if you use non-deterministic functions inside your DB modifying
queries. These will always lead to inconsistency in a Raft cluster, if they are not re-written (which is a waste of
resources imho), so they must never be used. This is not considered a breaking change, since they should not have been
used anyway. This feature only acts as a safety-net.

#### List / Fetch Backups

It is now possible to list backups. For local ones, the `hiqlite::Client` provides the possibility to get a
`tokio::fs::File` handle, while S3 backups can be streamed via a `ChannelReceiver`.

This version also changes the way that filenames for local backups are built. The timestamp for the filename will be
the exact same for all local backups in a cluster. This makes downloading via a load balancer a lot easier.

## v0.8.0

This is a rather small release. Mostly only breaking because of a small API change inside `hiqlite-wal`, which now can
resolve all `LockFile` situations automatically. This means that for `hiqlite`, the config variable `wal_ignore_lock`
has been removed. It's not needed anymore.

Apart from that, you get 2 new variables you can use to define the listen address for the API and Raft servers. This
solves an issue in IPv6-only environments, and makes it possible to bind to a specific IP only instead of the default
listening on all interfaces from before.

```toml
# You can set the listen addresses for both the API and Raft servers.
# These need to somewaht match the definition for the `nodes` above,
# with the difference, that a `node` address can be resolved via DNS,
# while the listen addresses must be IP addresses.
#
# The default for both of these is "0.0.0.0" which makes them listen
# on all interfaces.
# overwritten by: HQL_LISTEN_ADDR_API
listen_addr_api = "0.0.0.0"
# overwritten by: HQL_LISTEN_ADDR_RAFT
listen_addr_raft = "0.0.0.0"
```

## hiqlite-wal v0.7.1

The `hiqlite-wal-v0.7.0` had a bug when truncating WAL logs and shifted the front offset instead of the back.
This could happen, if a leader goes down with a already added, but not fully commited log entry. In such a case, the old
leader would need to truncate the Logs until the last fully commited point.

## hiqlite v0.7.1

The only purpose of this release is to fix a docs build error on `docs.rs`.

## v0.7.0

### Breaking

#### `rocksdb` deprecated

Rocksdb is a really good and fast KV database, but it comes with quite a few issues. The first big thing is, that it's
pure overkill to only be used as a Raft Logs Store. Logs come in sequential order, are immutable, and append-only.
Rocksdb is a huge dependency and it takes a long time to compile. It also adds ~7mb of release binary size and is almost
impossible to compile to `musl`.

To overcome these issues, `hiqlite-wal` has been created from the ground up. It provides memory-mapped WAL files, that
are append-only and perfectly serve the purpose of a Raft Log Store. It is very light-weight in terms of code-,
binary size and compile time, and only provides the functionality we actually need. If also has implementations that try
to auto-recover lost WAL records in case of an application crash in the middle of writing. This is a situation you could
not easily get out of with `rocksdb` as well. Even if it fails recovering everything it needs, it will at least make
everything work on its own in probably almost all cases, and only if it cannot, you at least have the possibility to
easily fix it.

The default Log Store is `hiqlite-wal`, but you can keep on using Rocksdb (at least for some time) with the `rocksdb`
feature. In future versions, Rocksdb will probably be removed completely. If you want to upgrade an existing database,
you can enable the `migrate-rocksdb` feature, which will trigger a Log Store migration during start up. Hiqlite will
then check if it can find an existing Rocksdb, migrate all Logs it can find, and then remove the old Rocksdb files.  
**CAUTION:** Even though this migration has been tested on quite a few instances without any issues, you really should
have a backup before doing it. For a single instance, this is not too important, but for a distributed, already existing
cluster it definitely is!

When you have a setup that for instance runs inside containers and a volume can never be mounted to multiple pods, you
probably want to enable `wal_ignore_lock` / `HQL_WAL_IGNORE_LOCK`, which handles a start after a crash automatically.
But if it may be possible, that a crashed process my still be running somehow, and accessing the database files, you
need to handle this manually and delete the lock file after you killed that process.

### Changes

#### Config as TOML

In addition to reading your whole config from ENV vars, you can now also `NodeConfig::from_toml()` with the new `toml`
feature enabled, which is the case by default now. Take a look at
the [hiqlite.toml](https://github.com/sebadob/hiqlite/blob/main/hiqlite.toml) for an example.

All values read from a TOML file can be overwritten by the matching ENV var. If both values are given, the ENV var will
have the higher priority. This makes it possible to have defaults in your TOML and temporarily overwrite something for
whatever reason.

#### Stability Improvements

Many small fixes and additional checks have been added in lots of places to improve the shutdown / restart stability
with an in-memory only Cache layer. The main issue with this is, that the Raft Logs are ephemeral, which could lead to
many different issues. However, at least to my knowledge, all the existing issues and edge cases has been taken care
of and it should be perfectly stable now to use in-memory only Raft Logs.

#### Distributed Counters

With the `counters` feature, which depends on `cache`, you can now have distributed, raft-backed counters for things
like rate-limiting for instance. The `hiqlite::Client` exposes some new functions to work with them:

- `counter_get()`
- `counter_set()`
- `counter_add()`
- `counter_del()`
- `clear_counters()`

#### `jemalloc` feature

You can enable the `jemalloc` feature and Hiqlite will pull in `jemalloc` as the global allocator.

#### `hiqlite-wal` config options

The new `hiqlite-wal` provides some new config options. You can set a custom `log_sync` / flush strategy depending on
your needs, set the `wal_size` for WAL files, and `cache_storage_disk` will define if for the Cache layer, either an
in-memory Raft Log Store will be used, of if true (default now), WAL + Snapshots for the Cache will be written to disk.
This brings the possibility to have persistent caches, that can be rebuilt from disk even after a complete shutdown.
The Cache / KV store itself will still be in-memory only and therefore have very fast read access, even though writes
will be limited by your disk speed.

More information can be found in the example [hiqlite.toml](https://github.com/sebadob/hiqlite/blob/main/hiqlite.toml).

## v0.6.0

### Breaking

tl;dr

For the a Cache enum, when you had before:

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, hiqlite::EnumIter, hiqlite::ToPrimitive)]
enum Cache {
    One,
    Two,
}
```

You now only need:

```rust
#[derive(Debug, strum::EnumIter)]
enum Cache {
    One,
    Two,
}

impl CacheIndex for Cache {
    fn to_usize(self) -> usize {
        self as usize
    }
}
```

And for the migrations, before:

```rust
#[derive(rust_embed::Embed)]
#[folder = "migrations"]
struct Migrations;
```

You now need:

```rust
use hiqlite_macros::embed::*;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;
```

And `hiqlite::params` has been moved into `hiqlite_macros::params`.

**The long version:**

`hiqlite::params` macro has been removed and is now inside a new crate `hiqlite-macros`. This made it possible to not
need the manual import of `hiqlite::Param` each time when using this macro.

Additionally, you don't need to add `rust_embed`, `num-traits` and possibly `num-derive` macros manually anymore.
`num-traits` and `num-derive` have been dropped completely and instead of adding these external dependencies to your
application, you now need a tiny, usually 5 lines long, boilerplate `impl CacheIndex for MyCacheEnum`. The re-export
of the `strum` macro from `hiqlite::` has been removed as well, because it needed the dependency manually anyway, which
was weird to use.

The `rust_embed` dependency is now also re-exported from `hiqlite-macros` in a way that actually works without you
needing to add it manually.

The `serde::Serialize` + `serde::Deserialize` trait bounds for your `Cache` index enum have been removed as well.

On the long run, the idea is to also be able to successfully re-export `strum`, which cannot be done currently because
of some unfortunate limitations, or create our own macro to derive the `Iterator` trait. But for now, this is quite a
big improvement already.

### Changes

- Support for fixed-size u8 array conversion.
- You can now refer to the output from statements execute before during a `hiqlite::Client::txn()`
  via `RETURNING` + `StmtIndex`. Take a look at the `sqlite-only` example.
- Shutdown has been made quicker and more resilient, especially for `cache` Rafts which do not have a
  persistent state between restarts.
- Internal code and performance improvements and dependencies have been bumped.
- MSRV has been bumped to `1.85.1`

## v0.5.0

- All internal dependencies have been bumped to the latest stable version.
- If you don't need the `listen_notify` feature for remote clusters, you can now choose to only use
  `listen_notify_local` instead, which will pull in less dependencies.
- The `asm` feature has been removed from `sha2` to make it compile on windows.
- The shutdown delay of 10 seconds is not being applied anymore if you only run a single instance, which usually is the
  case during local development. This greatly improves the DX.
- `hiqlite::Client::query_raw_not_empty()` has been removed as it was redundant. `query_raw_one()` behaves in the
  same way.
- MSRV has been bumped to `1.82.0`

## v0.4.0

### Updates

Rocksdb major version has been updated from `8` to `9.9.3` under the hood.
Some other smaller dependencies have been bumped to the latest versions as well.

### Bugfix

The dashboard cookie did not set a `path` when used in production and therefore browsers would reject it because of
its `__Host-` prefix. The path has been added properly to fix this issue.

## v0.3.3

Fixes an issue where the DB writer would panic because of a mismatch in DB Migrations validation. This could have
happened if migrations are applied between applications starts. The `hiqlite::Client` in that case would only send the
migrations over the network that have not been applied already and optimizes already existing ones away.
However, the additional validation inside the DB writer (to make sure the client did not mess up) was too strict, and it
would error.

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