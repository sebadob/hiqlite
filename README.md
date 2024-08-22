# Hiqlite

Hiqlite is an embeddable SQLite database that can form a Raft cluster to provide strong consistency, high availability
(which is where `Hiqlite` derives from), replication, automatic leader fail-over and self-healing features.

## Project Status

This project is in an early phase and I have some things on the TODO before I can release the first v0.1.0.
Until these TODO's are finished, I will not not care about any changelog or something like that, because it costs more
time and effort than it's worth at this point.

However, you can take a look at the integration test (`hiqlite/tests/`) or the example. These do work fine so far.
I do have many panics (that hopefully don't happen) and assertions in case of logic errors all over the code.
I'd rather have my application panic so I can catch the error immediately than missing an error log and ending up in
an inconsistent state.

Issues and discussions are not available on purpose in this early stage. It would simply not make any sense before
v0.1.0. I will also push directly to `main` until it's hitting the first release, which will most probably break the
examples from time to time. There is a working [0.0.6 Tag](https://github.com/sebadob/hiqlite/tree/v0.0.6), just in
case I do break them.

The project itself is pretty useable by now. The TODO's are mainly about documentation, more examples and making
the `Client` work for remote-only databases. The `hiqlite_server` is working fine so far as well. It can generate a
basic starter config for testing on localhost and should be straight forward to use.

## Why

Why another SQLite replication solution? Other projects exist already that can do this. The problem is that none of
them checks all boxes. They either require an additional independent process running on the side which can do async
replication, need a special file system, or are running as a server.

I don't think that running SQLite as a server is a good solution. Yes, it is very resource friendly, and it may
be a good solution when you are heavily resource constrained, but you loose its biggest strength when doing this: having
all you data local, which makes reads superfast without network latency.
Hiqlite builds on top of `rusqlite` and provides an async wrapper around it to make it easy usable with `tokio`. For the
Raft logic, it builds on top of`openraft` while providing its own storage and network implementations.

## Goal

Rust is such an efficient language that you usually only need one process to achieve whatever you need, for most
applications at least. An embedded SQLite makes the whole process very convenient. You get very fast local reads and at
the same time, it comes with the benefit that you don't have to manage an additional database, which you need to set up,
configure and more importantly maintain. And embedded SQLite will bring database updates basically for free when you
build a new version.

When configured correctly, SQLite offers very good performance and can handle most workloads these days. In very
first benchmarks that I did to find out if the project makes sense in the first place, I got up to 24.5k single
inserts / s on a cheap consumer grade M2 SSD. These tests were done on localhost with 3 different processes, but still
with real networking in between them. On another machine with older SATA SSDs it reached up to 16.5k inserts / s.

At the end, the goal is that you can have the simplicity and all the advantages of an embedded SQLite while still being
able to run your application highly available (which is almost always mandatory for me) and having automatic fail-over
in case of any errors or problems.

## Currently implemented and working features

- full Raft cluster setup
- everything a Raft is expected to do (thanks to [openraft](https://github.com/datafuselabs/openraft))
- persistent storage for Raft logs (with [rocksdb](https://github.com/rust-rocksdb/rust-rocksdb)) and SQLite state
  machine
- "magic" auto setup, no need to do any manual init or management for the Raft
- self-healing - each node can automatically recover from:
    - lost cached WAL buffers for the state machine
    - lost cached WAL buffer for the logs store
    - complete loss of the state machine DB (SQLite)
    - complete loss of the logs storage (rocksdb)
    - complete loss of the whole volume itself
- automatic database migrations
- fully authenticated networking
- optional TLS everywhere for a zero-trust philosophy
- fully encrypted backups to s3, cron job or manual (
  with [s3-simple](https://github.com/sebadob/s3-simple) + [cryptr](https://github.com/sebadob/cryptr) )
- restore from remote backup (with log index roll-over)
- strongly consistent, replicated `execute` queries
    - on a leader node, the client will not even bother with using networking
    - on a non-leader node, it will automatically switch over to a network connection so the request
      is forwarded and initiated on the current Raft leader
- strongly consistent, replicated `execute` queries with returning statement through the Raft
    - you can either get a raw handle to the custom `RowOwned` struct
    - or you can map the `RETURNING` statement to an existing struct
- consistent read / select queries on leader
- transaction executes
- simple `String` batch executes
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

## TODOs before v0.1.0

- real world stability testing and fixes
- proper documentation

## Crate Features

### `default`

By default, the following features are enabled:

- `auto-heal`
- `backup`
- `sqlite`

### `auto-heal`

This feature allows for auto-healing the State Machine (SQLite) in case of an un-graceful shutdown.
To redurce I/O and improve performance, Hiqlite does not write the `last_applied_log_id` from the Raft messages
into SQLite with each write. If it would do that, we would need to execute 1 extra query for each incoming
request, which effectively would double the amount of I/O if we just think about single `EXECUTE` queries.
Instead of doing that, it tracks the last applied ID in memory and only persists it into the DB in the
following situations:

- a new snapshot creation has been triggered
- a backup has been triggered
- the metadata of the whole Raft changes (leader change, a node has joined, ...)
- the node is being shut down

To make sure it would not start up a database where the last ID has not been persisted correctly, Hiqlite
creates a lock file at startup (like most other DB's). If this file exists with the next start, it means that
the application has been killed (host crashed, `kill -9`, ...), because otherwise it would remove the lock
file after the `last_applied_log_id` has been persisted correctly.

The `auto-heal` feature enabled the functionality to recover an un-graceful shutdown automatically by simply
deleting the whole existing SQLite and rebuilding it from the latest snapshot + raft logs to always reach a
clean state.

If you have special needs, you may not want this. I can't think of a situation where it would make much sense
to disable it, but you could do it.

### `backup`

This feature allows the creation of automatic backups for disaster recovery. It pulls in `cron` as an additional
dependency and enabled `sqlite` and `s3` features as well, because it does not make sense without these.

When `backup` is enabled, you will get the (by default) nightly backup cron job and you can manually trigger
backup creation's via the `hiqlite::Client`. Backups without pushing them to an S3 storage don't make too much
sense, because even when a cluster node would lose its whole volume, it would simply be rebuilt from the current
raft leader via snapshot + log replication.

Backups will be created locally first on each of the Raft nodes. Afterward, only the leader will encrypt the
backup and push it to the configured S3 bucket for disaster recovery.

Auto-restoring from a backup on S3 storage will also be possible with this feature enabled.

### `cache`

This feature will start another independent raft group (can run without `sqlite` enabled as well).
The `hiqlite::Client` will get new functions like `get()` and `put()`. The `cache` feature will build multiple
raft-replicated, in-memory caches on all nodes. Basically an in-memory KV store with optional per cache per entry
TTL for each key.

### `dashboard`

This feature is the one that makes the crate size on crates.io that big. Hiqlite comes with pre-built, static
HTML files to optionally serve a simple dashboard. With this dashboard, you have the possibility to run queries
against your database, which typically is not that easy for a SQLite in production, which is probably deployed
inside some container.

The dashboard will be served alongside the API HTTP server. It is very basic for now, but it gets the job done.
It will pull in quite a few extra dependencies and enable `sqlite` feature, because it does not work with the
`cache` or other features currently.

![dashboard screenshot](https://raw.githubusercontent.com/sebadob/hiqlite/main/dashboard/screenshot.png)

### `dlock`

The `dlock` feature gives you access to distributed locks, synchronized over all Raft nodes. It depends on
the `cache` feature to work.

In some cases, you can't achieve what you need to do within a single query or inside a transaction. For instance,
you need to fetch data from the DB, compute stuff with it, and write something back to the DB while the data
on the DB must be locked the whole time. Because transactions with Hiqlite can't let you hold a lock directly
on the DB (because of the Raft replication), you get distributed locks.

You can lock any key, then do whatever you need, and as soon as the `Lock` you will get is being dropped, it will
be released automatically.

**Important:**   
In the current version, a distributed lock is only valid for max 10 seconds, to avoid issues with network segmentation
or crashed nodes while they were holding some locks. If a lock is older than 10 seconds, it will be considered being
"dead" in the current implementation to get rid of never-ending locks.

### `full`

This feature will simply enable everything apart from the `server` feature:

- auto-heal
- backup
- cache
- dashboard
- dlock
- listen_notify
- s3
- shutdown-handle
- sqlite
- webpki-roots

### `listen_notify`

Sometimes, you need something simple like Postgres' listen / notify to send real time messages between nodes of your
deployment, without the need for message delivery guarantees or something like that. That is exactly what the
`listen_notify` feature will let you do. It pulls in a few additional dependencies and enables the `cache` feature it
depends on.

Depending on your setup, you will get different levels of message delivery guarantees. The classic Postgres listen /
notify will forward messages, if another connection is listening, and drop them if not, pretty simple.  
With Hiqlite, if your node is a real Raft member, meaning it is not using a remote client, you will have a guaranteed
once delivery with any form of `listen()`. If however you have a remote client, which is connected to a remote Hiqlite
cluster without a local replicated state, you will not receive missed messages, if you stopped listening for some time.
In this case, you will have the classic Postgres behavior.

**Important:**  
If you enabled this feature and you `notify()` via the `hiqlite::Client`, you must make sure to actually consume the
messages on each node. Behind the scenes, Hiqlite uses an unbound channel to never block these. This channel could fill
up if you `notify()` without `listen()`.

### `s3`

You would probably never just enable the `s3` feature on its own in the current implementation. It has been outsourced
for a possible future feature expansion. It depends on the `backup` feature and both will pull in each other as a
dependency right now.  
This feature will enable the possibility to push encrypted State Machine (SQLite) backups to a configured `s3` bucket.

### `server`

This feature only exists to make it possible to run Hiqlite as a standalone DB / Cluster, if you really want this.
It will build a binary which spins up a cluster with the given configuration, or you you can use it to install Hiqlite
to spin up instances easily with

`cargo install hiqlite --features server`

You should never enable the `server` feature if you are using Hiqlite as a crate and run it inside your application,
which should always be preferred, because it would make all operations a lot faster because of local data and less
network round-trips. Embedding Hiqlite is actually one of its biggest advantages over a server / client database like
Postgres, which would never be able to even come close to the read and `SELECT` speeds of a local SQLite instance.

### `shutdown-handle`

As mentioned in other places already, a Hiqlite node should always be shut down gracefully to prevent full State Machine
rebuilds with each restart. Most applications already have some sort of shutdown handles or can listen automatically.
If you already have something like that, you can leave this feature disabled and simply call
`hiqlite::Client.shutdown()`
before exiting your `main()`.  
In any other case, you can enable the `shutdown-handle` and register an automatic shutdown handle like shown in the
examples, which you can `.await` just before exiting your `main()`.

### `sqlite`

This is the main feature for Hiqlite, the main reason why it has been created. The `sqlite` feature will spin up a
Raft cluster which uses `rocksdb` for Raft replication logs and a `SQLite` instance as the State Machine.

This SQLite database will always be on disk and never in-memory only. Actually, the in-memory SQLite is slower than
on-disk with all the applied default optimizations. The reason is that an in-memory SQLite cannot use a WAL file. This
makes it slower than on-disk with a WAL file and proper `PRAGMA` settings in all of my tests.  
Another issue with an in-memory SQLite is that you will get into problems with queries blocking each other all the time
as soon as you have multiple connections for the same reason as above: no WAL file.

This has its own feature though, because you may only be interested in having an in-memory cache / KV store sometimes.
In this case, you can disable the default features and only enable `cache` or whatever you need. You would not even
need any volume attached to your container in that case.

### `webpki-roots`

This feature will simply enable baked-in TLS ROOT CA's to be independent of any OS trust store, like for instance
when you don't even have one inside your minimal docker container.

## Known Issues

There are currently some known issues:

1. Sometimes a node can hang on shutdown. In this case it needs to be killed manually.
   As mentioned already, I was not able to reproduce this so far. It happens to me somewhere in the range of 1 out of
   50 - 100 shutdowns. This could be solved by simply adding a timeout to the shutdown handler, but I did not do that
   on purpose at the current stage. I would rather find the issue and fix it, even if it takes time because of not
   being easily reproducible than ignoring the issue with a timeout.
2. When creating synthetic benchmarks for testing write throughput at the absolute max, you will see error logs because
   of missed Raft heartbeats and leader switches, even though the network and everything else is fine. The reason is
   simply that the Raft heartbeats in the current implementation come in-order with the Raft data replication. So, if
   you generate an insane amount of Raft data which takes time to replicate, because you end up being effectively I/O
   bound by your physical disk, these heartbeats can get lost, because they won't happen in-time.
   This issue will be resolved with the next major release of `openraft`, where heartbeats will be sent separately from
   the main data replication.