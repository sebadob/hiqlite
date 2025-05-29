# Hiqlite

Hiqlite is an embeddable SQLite database that can form a Raft cluster to provide strong consistency, high availability
(which is where `Hiqlite` derives from), replication, automatic leader fail-over and self-healing features.

## Why

Why another SQLite replication solution? Other projects exist already that can do this. The problem is that none of
them checks all boxes. They either require an additional independent process running on the side which can do async
replication, need a special file system, have bad throughput / latency, or are running as a server.

I don't think that running SQLite as a server is a good solution. Yes, it is very resource friendly, and it may be a
good choice when you are heavily resource constrained, but you lose its biggest strength when doing this: having
all your data local, which makes reads superfast without network latency.

Hiqlite builds on top of `rusqlite` and provides an async wrapper around it. For the Raft logic, it builds on top of
`openraft` while providing its own storage and network implementations.

## Goal

Rust is such an efficient language that you most often only need a single process to achieve whatever you need, for most
applications at least. An embedded SQLite makes everything very convenient. You get very fast local reads and at the
same time, it comes with the benefit that you don't have to manage an additional database, which you need to set up,
configure and more importantly maintain. And embedded SQLite will bring database updates basically for free when you
build a new application version.

When configured correctly, SQLite offers very good performance and can handle most workloads these days. In very
first benchmarks that I did to find out if the project makes sense at all, I got up to 24.5k single inserts / s on a
cheap consumer grade M2 SSD. These tests were done on localhost with 3 different processes, but still with real
networking in between them. On another machine with older SATA SSDs it reached up to 16.5k inserts / s.

At the end, the goal is that you can have the simplicity and all the advantages of an embedded SQLite while still being
able to run your application highly available (which is almost always mandatory for me) and having automatic fail-over
and self-healing capabilities in case of any errors or problems.

## Currently implemented and working features

- full Raft cluster setup
- everything a Raft is expected to do (thanks to [openraft](https://github.com/datafuselabs/openraft))
- persistent storage for Raft logs (with [rocksdb](https://github.com/rust-rocksdb/rust-rocksdb)) and SQLite state
  machine
- "magic" auto setup, no need to do any manual init or management for the Raft
- self-healing - each node can automatically recover from un-graceful shutdowns ~~and even full data volume loss~~
  (Note: There is a known bug that sometimes can lead to a Raft lock, if the full volume has been lost. This can be
  fixed, but needs manual interaction and a cluster restart right now)
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
- `query_map()` for local reads for `structs` that implement `impl<'r> From<hiqlite::Row<'r>>` which is the
  more flexible method with more manual work
- in addition to SQLite, multiple in-memory K/V caches with optional independent TTL per entry per cache - K/V caches
  are disk-backed and store their WAL file + Snapshots on disk, which means they are easy on your memory, and they can
  rebuild their in-memory data after a restart
- listen / notify to send real-time messages through the Raft
- `dlock` feature provides access to distributed locks
- standalone binary with the `server` feature which can run as a single node, cluster, or proxy to an existing cluster
- integrated simple dashboard UI for debugging the database in production - pretty basic for now but it gets the job
  done

## Performance

I added a [bench example](https://github.com/sebadob/hiqlite/tree/main/examples/bench) for easy testing on different
hardware and setups. This example is very simple and it mostly cares about `INSERT` performance, which is usually the
bottleneck when using Raft, because of 2 network round-trips for each write by design.

The performance can vary quite a bit, depending on your setup and hardware, of course. Quite a lot of work has been put
into performance tuning already and I would say, it will be able to handle everything you throw at it. When you reach
the threshold, you are probably in an area where you usually would not rely on a single database instance with something
like a Postgres anymore as well.  
SSDs and fast memory make quite a big difference of course. Regarding the CPU, the whole system is designed to benefit
more from fewer cores with higher single core speed like Workstation CPU's or AMD Epyc 4004 series. The reason is the
single writer at a time limitation from SQLite.

Just to give you some raw numbers so you can get an idea how fast it currently is, some numbers below. These values were
taken using the [bench example](https://github.com/sebadob/hiqlite/tree/main/examples/bench).

The benchmarks activate the `jemalloc` feature, which is quite a bit faster than glibc `malloc` but is not supported on
Windows MSVC target for instance. For cache performance, keep in mind that we use them in the disk-backed version and
not purely in-memory. Disk-backed provides a lot more consistency and can even rebuild the whole in-memory cache from
the WAL + Snapshot on disk, which means even a restart does not make you lose cached data. A pure in-memory version will
be a lot faster though. The disk-backed caches are limited by your disks IOPS and throughput only.

When you take a look at the numbers below, you will see that with higher concurrency, the SQLite implementation can
reach the physical limits of the disk, when it has roughly the same throughput as the cache does. This is actually
really impressive, considering that SQLite only allows a single writer at the same time.

Test command (`-c` adjusted each time for different concurrency):

```
cargo run --release -- cluster -c 4 -r 100000
```

### Beefy Workstation

AMD Ryzen 9950X, DDR5-5200 with highly optimized timings, M2 SSD Gen4

**SQLite:**

| Concurrency | 100k single `INSERT` | 100k transactional `INSERT` |
|-------------|----------------------|-----------------------------| 
| 4           | ~31.000 / s          | ~710.000 / s                |
| 16          | ~60.000 / s          | ~593.000 / s                |
| 64          | ~91.000 / s          | ~528.000 / s                |

For a simple `SELECT`, we have 2 different metrics. By default, `hiqlite` caches all prepared statements.
A simple `SELECT` with a fresh connection, which has not been prepared and cached yet, it took ~180-210 micros.
Once the connection has been used once and the statement has been cached, this drops down dramatically to
6 -25 micros (hard to measure these short ones).

**Cache (disk-backed):**

| Concurrency | 100k single PUT | single entry GET |
|-------------|-----------------|------------------| 
| 4           | ~35.000 / s     | ~6 micros        |
| 16          | ~78.000 / s     |                  |
| 64          | ~94.000 / s     |                  |

**Cache (full in-memory):**

| Concurrency | 100k single PUT |
|-------------|-----------------| 
| 4           | ~89.000 / s     |
| 16          | ~262.000 / s    |
| 64          | ~489.000 / s    |

### Older Workstation

AMD Ryzen 3900X, DDR4-3000, 2x M2 SSD Gen3 as Raid 0

**SQLite:**

| Concurrency | 100k single `INSERT` | 100k transactional `INSERT` |
|-------------|----------------------|-----------------------------| 
| 4           | ~9.200 / s           | ~388.000 / s                |
| 16          | ~17.500 / s          | ~335.000 / s                |
| 64          | ~27.800 / s          | ~299.000 / s                |

**Cache (disk-backed):**

| Concurrency | 100k single PUT | single entry GET |
|-------------|-----------------|------------------| 
| 4           | ~10.200 / s     | ~14 micros       |
| 16          | ~22.100 / s     |                  |
| 64          | ~29.100 / s     |                  |

**Cache (full in-memory):**

| Concurrency | 100k single PUT |
|-------------|-----------------| 
| 4           | ~24.700 / s     |
| 16          | ~78.800 / s     |
| 64          | ~177.000 / s    |

## Crate Features

### `default`

By default, the following features are enabled:

- `auto-heal`
- `backup`
- `sqlite`

### `auto-heal`

This feature allows for auto-healing the State Machine (SQLite) in case of an un-graceful shutdown.
To reduce I/O and improve performance, Hiqlite does not write the `last_applied_log_id` from the Raft messages
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

Auto-restoring from a backup on S3 storage will also be possible with this feature enabled. The likelihood that you
need to do this, is pretty low though.

#### You lose a cluster node

If you lost a cluster node for whatever reason, you don't need a backup. Just shut down the node, get rid of any
possibly left over data, and restart it. The node will join the cluster and fetch the latest snapshot + logs from
the current leader node.

#### You lose the full cluster

If you end up in a situation where you lost the complete cluster, it is the only moment when you probably need
restore from backup as disaster recovery. The process is simple:

1. Have the cluster shut down. This is probably the case anyway, if you need to restore from a backup.
2. Provide a backup file name on S3 storage with the `HQL_BACKUP_RESTORE` value with prefix `s3:` (encrypted), or a file
   on disk (plain sqlite file) with the prefix `file:`.
3. Start up the cluster again.
4. After the restart, make sure to remove the `HQL_BACKUP_RESTORE` env value.

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

### `jemalloc`

This feature enables the `jemallocator` instead of using the default glibc `malloc`. It is a lot more performant, solves
some issues with memory fragmentation and can be tuned for specific use cases. However, it does not work on Windows
MSVC targets and out of the box, without any tuning, it will use a bit more memory than default `malloc`.

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

### `migrate-rocksdb`

When enabled, it will add `rocksdb` to the dependencies and check at startup, if there are maybe rocksdb files and
log storage in the logs folder. It will then migrate the existing rocksdb Raft Log Store to `hiqlite-wal` and remove
the old rocksdb files afterward.

CAUTION: Just to be safe, you should have a backup of an existing instance before using the migrate feature, since it
tries to perform a manual, programmatic migration from `rocksdb` to `hiqlite-wal`.

### `rocksdb`

Uses `rocksdb` for the Raft Log Storage instead of the default `hiqlite-wal`. If you want to use an already existing
Hiqlite instance with a newer version, you might want to activate this feature temporarily until you created some
backups, just to be safe when it comes to `migrate-rocksdb`.

Apart from that, using the default `hiqlite-wal` is the better option in any scenario. It is only limited by your disk,
it is a lot more light weight, more efficient and can compile to any target, while rocksdb e.g. is almost impossible
to compile for `musl`. In future versions, `rocksdb` will most probably be removed completely.

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

## Standalone Server / Cluster

Even though it is recommended to embed `hiqlite` into your application, you can run it standalone as well.

### Local Start

The easiest way would be to install the binary with

```
cargo install hiqlite --features server
```

and then just execute it:

```
hiqlite -h
```

The current implementation is still a bit basic, but it will help you to get it up and running. I suggest to start
with generating a template config file with

```
hiqlite generate-config -h
```

If you want to just test it without TLS, add the `--insecure-cookie` option, and you may generate a testing password
with `-p 123SuperSafe` or something like that. Once you have you config, you can start a node with

```
hiqlite serve -h
```

The `--node-id` must match a value from `HQL_NODES` inside your config. When you overwrite the node id at startup,
you can re-use the same config for multiple nodes.

### Example Config

Take a look at the [examples](https://github.com/sebadob/hiqlite/tree/main/examples) or the example
[config](https://github.com/sebadob/hiqlite/blob/main/config) to get an idea about the possible config values.
The `NodeConfig` can be created programmatically or fully created `from_env()` vars.

### Cluster inside Kubernetes

There is no Helm chart or anything like that yet, but starting the Hiqlite server inside K8s is very simple.

#### Namespace

Let's run it inside a new namespace called `hiqlite`:

```
kubectl create ns hiqlite
```

#### Config

Create a `config.yaml` which holds your config:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: hiqlite-config
  namespace: hiqlite
data:
  config: |
    HQL_NODE_ID_FROM=k8s

    HQL_NODES="
    1 hiqlite-0.hiqlite-headless:8100 hiqlite-0.hiqlite-headless:8200
    2 hiqlite-1.hiqlite-headless:8100 hiqlite-1.hiqlite-headless:8200
    3 hiqlite-2.hiqlite-headless:8100 hiqlite-2.hiqlite-headless:8200
    "

    HQL_LOG_STATEMENTS=false
    HQL_LOGS_UNTIL_SNAPSHOT=10000
    HQL_BACKUP_KEEP_DAYS=3

    HQL_S3_URL=https://s3.example.com
    HQL_S3_BUCKET=test
    HQL_S3_REGION=example
    HQL_S3_PATH_STYLE=true

    HQL_INSECURE_COOKIE=true
```

#### Secrets

Create a `secrets.yaml`. To have an easy time with the `ENC_KEYS`, since the CLI does not provide a generator yet, you
can copy the value from your `generate-config` step above and re-use the value here, or just re-use the below example
values:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: hiqlite-secrets
  namespace: hiqlite
type: Opaque
stringData:
  HQL_SECRET_RAFT: 123SuperMegaSafeRandomValue
  HQL_SECRET_API: 123SuperMegaSafeRandomValue

  HQL_S3_KEY: YourS3KeyId
  HQL_S3_SECRET: YourS3Secret

  ENC_KEYS: "
  bVCyTsGaggVy5yqQ/UzluN29DZW41M3hTSkx6Y3NtZmRuQkR2TnJxUTYzcjQ=
  "
  ENC_KEY_ACTIVE: bVCyTsGaggVy5yqQ

  # This is a base64 encoded Argon2ID hash for the password: 123SuperMegaSafe
  HQL_PASSWORD_DASHBOARD: JGFyZ29uMmlkJHY9MTkkbT0xOTQ1Nix0PTIscD0xJGQ2RlJDYTBtaS9OUnkvL1RubmZNa0EkVzJMeTQrc1dxZ0FGd0RyQjBZKy9iWjBQUlZlOTdUMURwQkk5QUoxeW1wRQ==
```

#### StatefulSet

The last one for testing (leaving Ingress out for this simple example) will create a StatefulSet, a load balanced
Service you could access via a `NodePort` to reach the dashboard, and a headless Service to the nodes can create
direct connections to each other. Create an `sts.yaml`:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: hiqlite
  namespace: hiqlite
spec:
  selector:
    app: hiqlite
  type: NodePort
  ports:
    - name: raft
      protocol: TCP
      port: 8100
      targetPort: 8100
    - name: api
      protocol: TCP
      port: 8200
      targetPort: 8200
---
apiVersion: v1
kind: Service
metadata:
  name: hiqlite-headless
  namespace: hiqlite
spec:
  clusterIP: None
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
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: hiqlite
  namespace: hiqlite
  labels:
    app: hiqlite
spec:
  replicas: 3
  selector:
    matchLabels:
      app: hiqlite
  serviceName: hiqlite-headless
  template:
    metadata:
      labels:
        app: hiqlite
    spec:
      containers:
        - name: hiqlite
          image: ghcr.io/sebadob/hiqlite:0.6.0
          imagePullPolicy: Always
          securityContext:
            allowPrivilegeEscalation: false
          ports:
            - containerPort: 8100
            - containerPort: 8200
          env:
            - name: HQL_SECRET_RAFT
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: HQL_SECRET_RAFT
            - name: HQL_SECRET_API
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: HQL_SECRET_API
            - name: HQL_S3_KEY
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: HQL_S3_KEY
            - name: HQL_S3_SECRET
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: HQL_S3_SECRET
            - name: ENC_KEYS
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: ENC_KEYS
            - name: ENC_KEY_ACTIVE
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: ENC_KEY_ACTIVE
            - name: HQL_PASSWORD_DASHBOARD
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: HQL_PASSWORD_DASHBOARD
          volumeMounts:
            - mountPath: /app/config
              subPath: config
              name: hiqlite-config
            - mountPath: /app/data
              name: hiqlite-data
          livenessProbe:
            httpGet:
              scheme: HTTP
              port: 8200
              path: /health
            initialDelaySeconds: 10
            periodSeconds: 30
          resources:
            requests:
              memory: 32Mi
              cpu: 100m
      # add your image pull secrets name here in case you use a private container registry
      #imagePullSecrets:
      #  - name: harbor
      volumes:
        - name: hiqlite-config
          configMap:
            name: hiqlite-config
  volumeClaimTemplates:
    - metadata:
        name: hiqlite-data
      spec:
        accessModes:
          - "ReadWriteOnce"
        resources:
          requests:
            storage: 256Mi
        # In case you want to specify the storage class.
        # You should always(!) prefer local over some replicated abstraction layer.
        # Hiqlite cares about replication itself already.
        #storageClassName: local-path
```

#### Apply Files

The last step is to simply `kubectl apply -f` the `config.yaml` and `secrets.yaml` followed by the `sts.yaml` last.
This should bring up a 3 node, standalone Hiqlite cluster.

## Cluster Proxy

If you want to connect to a cluster without being able to reach each node via its configured address in `HQL_NODES`,
like in the Kubernetes example cluster above, you can also start a server binary in proxy mode with

```
hiqlite proxy -h
```

Let's do a quick example to start a proxy inside K8s to access the above testing cluster from the outside. This
example assumes the above ConfigMap and Secrets do exist already. If this is the case, we only need to add a Deployment:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: hiqlite-proxy
  namespace: hiqlite
spec:
  type: NodePort
  selector:
    app: hiqlite-proxy
  ports:
    - name: api
      protocol: TCP
      port: 8200
      targetPort: 8200
      nodePort: 30820
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: hiqlite-proxy
  namespace: hiqlite
  labels:
    app: hiqlite-proxy
spec:
  replicas: 2
  selector:
    matchLabels:
      app: hiqlite-proxy
  template:
    metadata:
      labels:
        app: hiqlite-proxy
    spec:
      containers:
        - name: hiqlite-proxy
          image: ghcr.io/sebadob/hiqlite:0.6.0
          command: [ "/app/hiqlite", "proxy" ]
          imagePullPolicy: Always
          securityContext:
            allowPrivilegeEscalation: false
          ports:
            - containerPort: 8100
            - containerPort: 8200
          env:
            - name: HQL_SECRET_API
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: HQL_SECRET_API
            - name: ENC_KEYS
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: ENC_KEYS
            - name: ENC_KEY_ACTIVE
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: ENC_KEY_ACTIVE
            - name: HQL_PASSWORD_DASHBOARD
              valueFrom:
                secretKeyRef:
                  name: hiqlite-secrets
                  key: HQL_PASSWORD_DASHBOARD
          volumeMounts:
            - mountPath: /app/config
              subPath: config
              name: hiqlite-config
          livenessProbe:
            httpGet:
              scheme: HTTP
              port: 8200
              path: /ping
            initialDelaySeconds: 10
            periodSeconds: 30
          resources:
            requests:
              memory: 32Mi
              cpu: 100m
      # add your image pull secrets name here in case you use a private container registry
      #imagePullSecrets:
      #  - name: harbor
      volumes:
        - name: hiqlite-config
          configMap:
            name: hiqlite-config
```

After `kubectl apply -f` this deployment, you can use a remote Client to connect via this proxy with

```rust, notest
hiqlite::Client::remote()
```

like shown in the
[bench example](https://github.com/sebadob/hiqlite/blob/70cc7500316dd138c0e1bd417a915af216fb19b2/examples/bench/src/main.rs#L147).

## Known Issues

There are currently some known issues:

1. When creating synthetic benchmarks for testing write throughput at the absolute max, you will see error logs because
   of missed Raft heartbeats and leader switches, even though the network and everything else is fine. The reason is
   that the Raft heartbeats in the current implementation come in-order with the Raft data replication. So, if you
   generate an insane amount of Raft data which takes time to replicate, because you end up being effectively I/O
   bound by your physical disk, these heartbeats can get lost, because they won't happen in-time. This issue will be
   resolved with the next major release of `openraft`, where heartbeats will be sent separately from the main data
   replication.
2. In the current version, the logging output is very verbose on the `info` level. This is on purpose until everything
   has been stabilized. In future versions, this will be reduced quite a bit.
