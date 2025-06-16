# Hiqlite Architecture

This document is WIP and does not explain everything in detail yet, but it will give you a first overlook how the
internal parts fit together.

Hiqlite uses [openraft](https://github.com/datafuselabs/openraft) for the Raft internal logic. `openraft` provides the
building blocks for a Raft application without the implementations for storage and network. Hiqlite comes with `sqlite`
feature enabled by default, which will provide a Raft Logs Storage based on `rocksdb` and a State Machine based on
SQLite via `rusqlite` under the hood. If you activate the `cache` feature, the Raft Logs Storage will be an in-memory
`VecDeque` and multiple in-memory KV Stores based on in-memory `BTreeMap`s.
The network connections between nodes are realised with multiplexing WebSockets. The Raft internal network is also
running on a separate HTTP server to be able to either run the replication traffic on a fully separated network for
better load distribution and security, or to just not expose any internal endpoints to the public.

## Raft Logs Store

In the early days, `rocksdb` was used the storage engine for Raft Logs. However, by now Hiqlite comes with its own WAL
Log Storage implementation `hiqlite-wal`. The `rocksdb` feature can be considered deprecated and will be removed in the
near future.

`hiqlite-wal` spawns a single WAL writer process which takes care of metadata writing, log file creation, log roll-over,
WAL locking, CRC CHKSUMs for each WAL record, and so on. This is single threaded on purpose to avoid most of the usual
overhead from mutexes. You will always have a single WAL writer (2 if you additionally enable the `cache` feature), and
1 or more WAL readers, depending on the configuration and Raft setup. Writers and readers a dedicated, sync threads and
they avoid `async` on purpose to reduce the latency.

All communication with these sync threads is done via `flume` channels, which provide a very nice and stable bridge
interface between sync and async code. The tasks only care about writing or reading data, nothing else. They don't
interpret results but just forward them to code running on other tasks, and they don't care about serialization (
mostly), which is outsourced as much as possible as well. With this design approach, even though there is only a single
writing task, we can achieve very high throughput. The only thing these tasks care about is providing and sync, as fast
as possible interface to the underlying data. Another big benefit of this approach is that we don't have an overhead
coming from locks or other sync primitives. This means no locking, no waiting, just writing and reading data while never
being blocked by other concurrent writers.

### WAL Lock File

`hiqlite-wal` creates a `lock.hql` file. This is used to make sure that only a single process at a time will try to
write WAL files and that not multiple instances may end up using the same data because of misconfiguration. It serves a
double purpose though. If the file exists at startup, it will be considered a non-clean start and a deeper integrity
check of the latest WAL file will be triggered, which checks every single WAL record and makese sure the header data
matches the content. It can even recover orphaned records, if the application crashed before and was unable to write
everything from buffers to disk.

While the WAL writer is running, it holds a cross-platform lock on the file. This makes sure no other hiqlite process
tries to write to the same data directory. If the process exits, either gracefully or because of a crash, this lock is
released automatically. If it's a crash, the file itself will stay, but unlocked. During a graceful shutdown though, the
file will also be removed. This makes it possible at the next start to resolve the last state properly and take action.

## SQLite State Machine

### Default PRGAMAs

Modern SQLite can be very fast, if you tune and configure it correctly. All of this is done by default when Hiqlite
opens database connections. A few `PRAGMA`s are set automatically to provide a good compromise between memory usage, I/O
and performance. The following values have been chosen as good default values:

- `journal_mode=WAL`
- `synchronous=OFF`

- `page_size=4096`
- `journal_size_limit=16384`
- `wal_autocheckpoint=4000`

- `auto_vacuum=INCREMENTAL`
- `foreign_keys=ON`
- `optimize=0x10002`

The biggest improvements come from `journal_mode=WAL` + `synchronous=OFF`. `WAL` mode will make sure, that writes are a
lot faster and that writes will never block reads and vice versa. The default for `synchronous` is `FULL`, which makes
sure all data is flushed to disk before returning from the query. However, this comes with a huge performance penalty.
Usually, in `WAL` mode you would choose `synchronous=NORMAL`. In `NORMAL` mode, you may lose the very last data written
if your DB crashes, but apart from that, your database cannot become corrupted. `NORMAL` mode gives a huge boost to
write
throughput already, if you can live with some data loss. Hiqlite goes a step further by setting `synchronous=OFF`. This
can lead to a corrupt database file on crash, but that is not an issue at all, because of the Raft logs.
Hiqlite creates a database lock file on startup, which will be cleaned up on a graceful shutdown. Via this file, it
knows after a restart, if a crash has happened beforehand. If this is the case, it will simply delete the whole database
and rebuild it cleanly from the latest snapshot + Raft logs. This means a restart after a crash might take a few seconds
longer (depending on your total DB size), but you will always have a clean, consistent state, even with
`synchronous=OFF`. At the same time, `synchronous=OFF` gives another ~18% performance boost after first tests compared
to `synchronous=NORMAL` already.

`page_size=4096`, `journal_size_limit=16384` and `wal_autocheckpoint=4000` all work together. The `page_size=4096` is
the default in current versions of SQLite already, but it is set again to just make sure it is correct. The default for
`journal_size_limit` is `4096`, which will lead to a max 4MB big WAL file. This value has been increased to 16MB max
just to trade a bit of disk space for better throughput. Because of the bigger max WAL file size, fewer sync's to disk
into the main DB file are needed, which means less I/O. The `wal_autocheckpoint` default is `1000`, which matches a 4MB
WAL file with a `page_size` of 4KB, and this has been increased to `4000` to match the 16MB WAL file.

`auto_vacuum=INCREMENTAL` makes sure that the DB file fragmentation will be kept low while not `VACUUM`ing too much.
Because `auto_vacuum=INCREMENTAL` on its own is not enough, the Hiqlite writer task (mentioned below) will `VACUUM` at
certain checkpoints like creating new snapshots or backups.

`foreign_keys=ON` is enabled by default because (to me) a relational database without foreign keys does not make much
sense.`optimize=0x10002` is executed with each new connections being opened to make sure queries stay fast.
Additionally, the writer task will optimize periodically again when creating snapshots or after applying migrations.

### Write

The SQLite State Machine writer is implemented in the same way as the `rocksdb` writer task. Hiqlite spawns a single
writer task with its on single, lock-free, raw connection to have the least amount of overhead. SQLite biggest weakness
is that it only allows a single writer to a database at any time. It does not have fancy table or even row locks like
many other databases. This is both a good and a bad thing.

The bad thing about it is obvious - only a single write to the whole database can happen at the same time. At the same
time, this is also the good thing. Locking the database is a lot simpler and therefore faster than with other databases.
The DB does not need to find the lock for the table or maybe only row it wants to modify first, both when locking and
unlocking. On top of that, because we can only do a single write at the same time, the sync writer task works with only
a single connection, which gets rid of any connection pool or other locking mechanisms.

Write to SQLite are designed in the same way as for `rocksdb` logs store above. All operations like serializing,
interpreting `Result`s, and so on, are outsourced as much as possible. This makes sure, that the task mostly only needs
to care about a single thing - writing to the database, then simply forwarding the result and executing the next write.

### Read

Reading from the SQLite happens in a different way though. Because Hiqlite is running in `WAL` mode, writes can never
block reads and vice versa. While SQLite only allows a single writer at the same time, reads can happen concurrently.
For this reasons, Hiqlite creates 4 read-only connections and manages them inside a `deadpool` connection pool. You will
usually not get a handle to this pool directly, because you simply don't need to. The `hiqlite::Client` will use these
pooled read connections automatically when you execute functions like for instance `query_as()`. The only thing you need
to care about is the Client. When, where and how the read pool is being used depends on your setup and situation. For
instance, if you have a local client, meaning you have an embedded replica of the Raft + SQLite, most of these queries
will be executed locally, which makes them superfast compared to any queries against a typical network database.
Usually,
a local, simple `SELECT` query can be done in ~ 30 - 70µs, depending on your machine of course. If you are executing a
consistent query or you have a remote Client, the read pool on the current leader node will be used, which means you
will
have the overhead of a network round trip.

## Network

The network between nodes uses WebSocket multiplexing. Each Raft member node will open 2 WebSocket connections to each
other member. One connection is for the Raft internal replication, while the other is opened by the `hiqlite::Client` to
the current Raft leader node. Each of these connections is multiplexing requests. A central router or manager task will
be spawned. This task will do quite a few things.

The WebSocket connection to the remote node will be opened and then split into sender and receiver parts. Each of these
will run inside its own `tokio::task` To be able to stream requests without waiting for responses first. This means each
WebSocket connection will have the central router / manager + reader + writer task. The router will listen to requests
from the `hiqlite::Client` and prepare the payloads to be sent over the wire. It will also handle responses and map them
properly depending on a `request_id` that each Client maintains internally. Depending on this id, the manager task can
map responses to requests and return the result to the correct location without ever blocking anything.

Another very important, and probably the most complex task the manager handles is network issues and reconnects. As long
as the network is up and running, all of this is pretty easy and straightforward. If the connection is being dropped
however because of for instance a network segmentation, or if a nodes needs to perform a leader switch, things can get
a lot more complicated.  
The most important thing is that you never want to lose any requests. Because the connection is multiplexed, it means
that there might be some logic and tasks running on the remote node wo which the Client just lost the connection to,
that will return a result. If this result can't be returned, everything possible should be done to somehow get this
result, just in case it was a short break and the connection comes back up just a few moments later. This is especially
important when it comes to queries that modify the database. You don't want to lose this result and later just retry,
because the connection broke, when this query has already been executed successfully, you just did not get the result.  
To encounter this problem, internal buffers are used in all WebSocket clients and servers. Clients will buffer the
requests from the `hiqlite::Client` for 10 seconds, just in case the network comes back quickly. If after 10 seconds,
there still is no result after a re-connect, the client will receive a connection error and the result will be
considered lost. On the server side, the same happens. All currently running tasks will be buffered, if the connection
has been broken while executing something. This makes sure that when this client reconnects, all pending results will be
returned before processing anything new.

In case of a leader switch, the Client will drop all outstanding results. The reason is pretty simple. When the Client
receives a leader switch error, it means that all other following queries on the remote host will fail in the exact same
way. All queries that can be executed locally and are leader independent will not be sent over the wire in the first
place. But each function inside the Client will retry once automatically in case of a leader switch to have as few
errors on the user side as possible. There should never be more than one leader switch error at the same time, so the
retry should always succeed or a least not return a leader switch error again.

## Listen / Notify

The `listen_notify` feature will start an additional handler task for Postgres-like listen / notify. The feature
behavior itself is described in the README already. However, the handler is pretty simple. It own the `tx` for the
internal `hiqlite::Client`, which will always receive any notifications and therefore provide a guaranteed once
delivery. In addition, whenever a new client subscribed to events / notifications via the `axum::Sse` endpoint, the `tx`
for this endpoint will be stored inside the listen / notify handler as well. Each notification will be sent to all the
listeners as well and if they return an error because of a closed channel or something like that, the `tx` will simply
be removed from the store.

## Distributed Locks

The distributed locks handler task will work similar to the listen / notify. If the `dlock` feature is enabled, the
handler task will be spawned which will hold all locks in-memory in a lock free local HashMap. Each lock is indexed via
a `String`. If the requested lock is not locked already, it will simply return the response that the lock was
successful + a locking id.

However, this becomes a bit more complex if it is currently locked. These messages are coming through the Raft and they
must not block and return immediately. In case of an already locked lock / index, a locking id will be returned with the
information, that the lock must be awaited, since it's locked already. If the client receives this message, it will open
another listener on this handler via its local client outside the Raft replication. This will work depending on the
locking ID it received, because it might be the case that there are other locking requests in the queue beforehand. To
handle this, the `dlock` handler also maintains a local queue will all the locking ids waiting to lock an index. When
the `Await` returns, the client will receive a `Locked` message with its ID again. If the client has the lock, it will
create a `Lock` struct and return it. On `drop()`, this lock will send a message through the Raft again to `Unlock` the
index with its own locking id appended.

Distributed locks become tricky during network issues. In case a client holds a `Lock` and then the network goes down
before it can release the lock, or maybe even the application or the OS crashes. If this happens, the lock could end up
in a state where it would be impossible to unlock again, because the locking id would be lost as well. To counter this
issue, the timestamp will always be saved with each new lock. When a new locking request comes in for an already locked
index, the timestamp will be compared to `now()`. If the current lock has been locked more than 10 seconds ago, it will
be considered "dead" and the new lock will be granted.  
In the current implementation, it is not possible to hold a lock for more than 10 seconds. This could be achieved with
the possibility to refresh a timestamp, but it has not been implemented so far.

## Concurrency

Everything described above for the Network will apply twice in case you have both the `sqlite` and the `cache` feature
enabled. Each of these features will create its own, fully independent Raft group + networking. With both features
enabled, Hiqlite will still start only 2 HTTP servers (Raft internal + public API), but separate network connections
will be opened and you will end up with 4 WebSocket streams, each split into 3 tasks for the multiplexing and
non-blocking streaming.

With all features enabled, Hiqlite will spawn :

- 2 Raft groups which will spawn a few tasks / threads
  internally ([openraft docs](https://docs.rs/openraft/latest/openraft/docs/internal/threading/index.html))
- 4 x 3 = 12 tasks the networking between nodes
- 2 tasks for the HTTP servers
- 1 writer task for `rocksdb` + 1 reader task (depending on setup maybe multiple via `openraft`)
- 1 writer task for SQLite + temporary tasks in case of snapshots, backups, uploads, ...
- 1 temporary task for each SQLite read / `SELECT` query being executed
- 1 task for the in-memory KV store
- 1 task for in-memory KV TTL, to cleanup and expire values when necessary
- 1 task for the `listen_notify` handler
- 1 task for the `dlock` handler

On top of this, there are a few other tasks being spawned without having much impact, like for instance a timer task
for flushing WAL to disk or the shutdown handler.

All these are real `tokio` async or blocking tasks, which means they can make full use of all available CPU cores.
However, you will probably reach higher throughput on smaller CPUs with higher single core speed than with big high
core count server CPUs. The reason is the single SQLite writer task limitation. It is possible to achieve a little
bit higher throughput with 2 SQLite writer tasks, but really not that much, while making the whole thing quite a bit
more complex and more error-prone. This is not worth it, considering how high the throughput already is, if you use
a fast enough SSD. In my testing, I was I/O bound by the latency of the physical SSD.

## Consistency

Hiqlite uses statement based replication. The query itself + params are serialized and sent into the Raft. After they
were commited and accepted by a quorum, the values are then forwarded to the single writer thread, which then creates a
cached, prepared statement and finally binds the params.

Consistency is guaranteed by the Raft of course. On SQLite level, when you have multiple DB connections, this could get
you into trouble, because a query might fail when the database is busy and you reach a timeout. The first mitigation is
using a WAL, which should be the preferred way these days with SQLite most of the time anyway. This makes sure that
writes never block reads and vice versa. This leaves you with the last potential issue. You could have multiple writers
to the same DB, and even in WAL mode, they could block each other, if one of them for instance executes an extremely
long running query, which would trigger a busy error on another writer. To be able to guarantee that all writes on all
SQLite instances are executed with the same result, there is only a single database connection (no pool or anything
else) from a single thread. This connection is the only one with write access to the DB. It executes all these
statements and because there is only one and the DB is in WAL mode, SQLite guarantees that all queries executed at that
stage will have the same result on all nodes.

But, there is one big thing you must never do, and that is, even though the SQLite could be accessed out of band
directly, you must never execute anything, that does not travel through the Raft, because then the consistency can of
course not be guaranteed anymore, since executes could lead to different results.

The reads are (usually) local all the time. They don't even use the Raft and access the DB directly. Read queries use
connections from a pool and are initialized with read only access, even before they are added to the pool. You don't get
any of these directly, but you can execute read queries via the Hiqlite client. It exposes all different kinds of
methods to make it work for you. This guarantees, that non connection from this read pool can modify the DB out of band.

Theoretically, the entire DB is in the Raft, at least each single statement. At some point you will of course have a
log roll over and a snapshot being created. The snapshots, at least for the DB, are just the result of a `VACCUUM INTO`
query.

If a node crashes, or the shutdown functionality provided by the Hiqlite client is not used, an existing lock file will
be found at startup. To be 100% sure that the DB is consistent, it will be deleted and rebuilt from the latest
Snapshot + Raft Logs. Each snapshot also contains some Raft metadata to always know, which Raft Log ID is the last one
that was applied to the DB.

This lock file can either be ignored and "repaired" automatically with the `auto-heal` feature, or you can have Hiqlite
panic on startup if it finds a non-consistent state / clean environment. Without the `auto-heal` enabled, it will leave
it up to you to take a look and take action. With it enabled, it will automatically rebuild the DB.
When you run it inside a container for instance and the volume can never be mounted to multiple containers at the same
time, enabling `auto-heal` is safe to do. Only if it might be possible that some already running process uses the same
data, you probably want to have it disabled and handle starts after a crash manually. Fixing this is then as easy as
making 100% sure that nothing is using the data, and there is no orphaned process somewhere in the background, and then
just delete the lockfile and restart.

So, as long as you don't modify the database out of band without using the Hiqlite client, consistency is guaranteed
that way. The `hiqlite::Client` exposes many functions for you to do any work on the database you might want to, just
don't use anything else manually. The good thing is, if you decide to drop Hiqlite at some point for whatever reason,
you can grab that database file and use it directly. Just never do both things at the same time. But, this is true for
any database or application in general. If you modify its files without it knowing about it, bad things will happen.
