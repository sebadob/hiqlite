# Caches, Counters, Locks, Notifications

The `cache` feature runs replicated in-memory key/value stores on every node. On top of it sit
counters, distributed locks, and listen/notify.

## Declare your caches

```rust, notest
use hiqlite::macros::CacheVariants;

#[derive(Debug, CacheVariants)]
enum Cache {
    One,
    Two,
}

let client = hiqlite::start_node_with_cache::<Cache>(config).await?;
```

Each variant is one cache, and the variant name identifies it in the cluster. The name is what the
cache group's log entries and snapshots carry, and each node maps it to its own position in the
enum. The derive builds that map at compile time, a perfect hash, so a lookup stays O(1).

Implement `CacheVariants` by hand only when a derive is impossible.

## Changing the enum

The order of the variants does not matter: reorder them, or insert one in the middle, as you like.

**Adding a variant** is a rolling deploy like any other. A node that still runs the old enum keeps
the entries of the new cache without a variant, logs a warning naming it, and carries it in its
snapshots, so every node holds the same state. When it is upgraded, the values are there under the
new variant. While the leader still runs the old enum, it refuses writes to the new cache with an
`unknown cache` error.

**A rename is a new, empty cache.** The entries of the old name stay in the cluster as a cache without
a variant, until they expire or a `clear_cache_all()`. Clear the old cache before the rename if its
entries have no expiry.

**A removal** works the same way: the entries of the removed name stay until they expire or a
`clear_cache_all()`.

**Keeping the variant and stopping to use it is the answer for most cases.** A variant you never
touch is not free: the node runs a background task for it and carries it in the cache snapshots, but
it needs no operational work at all.

A broken Raft state is never solved from the caches side. The way better alternative is always to
let the node re-sync from its healthy members: start it as a learner, catch it up, promote it. See
`disaster-recovery.md`.

## Keys and values

```rust, notest
use hiqlite::macros::params;

// a value that expires after 60 seconds
client.put(Cache::One, "session:42", &user, Some(60)).await?;

// no expiry
client.put(Cache::One, "config:flags", &flags, None).await?;

let user: Option<User> = client.get(Cache::One, "session:42").await?;
let previous: Option<User> = client.replace(Cache::One, "session:42", &other_user, None).await?;
let taken: Option<User> = client.get_remove(Cache::One, "session:42").await?;
client.delete(Cache::One, "session:42").await?;

let all: BTreeMap<String, User> = client.get_snapshot(Cache::One).await?;
client.clear_cache(Cache::One).await?;
client.clear_cache_all().await?;
```

- Keys are strings. Values are serialized with serde; `put_bytes`, `get_bytes`,
  `replace_bytes`, `get_remove_bytes` skip serialization and store raw bytes.
- `ttl` is the lifetime of the entry in seconds from now, or `None` for no expiry.
- `replace` writes the new value and returns the previous one when the key existed. `get_remove`
  returns the value and deletes it in one cluster step.
- `get_snapshot` gives you the whole cache as a map. It is a read of local state: use it
  deliberately, not in a loop. (Caution: this is very expensive for big caches)

> Cache entries are serialize with `bincode_next` under the hood, which does not support `serde`s
> `Deserialize::Any`. This means could must not annotate `struct`s with `skip_serializing`. If a
> value is `None`, serialize it, don't skip it.

## Disk-backed and in-memory

`cache_storage_disk = true` (the default) keeps the cache's log and snapshots on disk. The cache
rebuilds its content after a restart, so a reboot does not empty it, and memory stays flat.

`cache_storage_disk = false` keeps the log in memory: faster, less durable, and a node that is
killed hard comes back inconsistent and must re-sync from its peers. Use it when the cache is a
cache, not when it holds state your app cannot lose.

`in-memory-snapshots` additionally keeps snapshots out of the temp files, so a cache-only node needs
no volume at all. It roughly doubles the memory per entry.

## Counters

```rust, notest
let value = client.counter_add(Cache::Two, "logins:today", 1).await?;
client.counter_set(Cache::Two, "logins:today", 0).await?;
let current = client.counter_get(Cache::Two, "logins:today").await?;
client.counter_del(Cache::Two, "logins:today").await?;
client.clear_counters(Cache::Two).await?;
```

A counter is an integer entry that every node can increment in one cluster step. `counter_add`
returns the new value. Counters live in a cache you picked, so they inherit its storage settings.

## Distributed locks

```rust, notest
let lock = client.lock("balance:42").await?;
if lock.is_locked() {
    // read, compute, write - the key is yours while the guard lives
}
```

`Client::lock` takes a key and returns a guard. The lock is released when the guard is dropped, and
the server caps it at a lease window: a client that dies stops holding the key.

Use a lock when your work spans several calls and a single transaction cannot cover it. For
everything a transaction can cover, use `Client::txn` - it is one cluster step and it is much
cheaper.

## Listen and notify

```rust, notest
client.notify(&MyPayload { id: 1 }).await?;

// on every node
let payload: MyPayload = client.listen().await?;
let raw = client.listen_bytes().await?;          // (timestamp, bytes)
let next = client.listen_after(ts_micros).await?; // continue after a known message
let fresh = client.listen_after_start().await?;  // only what arrives from now on
let maybe = client.try_listen::<MyPayload>()?;   // non-blocking
```

Notifications travel through the cluster, so every node sees them.

- A real cluster member receives every message exactly once.
- A remote client receives what is sent while it listens. It does not catch up on what it missed.
- Consume the messages on every node that enabled the feature. A node that only notifies and never
  listens lets the queue grow.

The `/listen` endpoint on the API port is the websocket your remote listeners use; `Client` uses it
for you.

## Cache-only deployment

Disable the default features, keep `cache`, and you run a replicated KV store with no SQLite state
machine: no database file, no queries, no migrations. Add `counters`, `dlock`, `listen_notify` as
needed. Everything that reads a row is out of scope.

## Failure modes

- A write answers "unknown cache": the leader runs an enum without that variant. It goes away once
  the leader is upgraded. See "Changing the enum".
- Cache entries gone after a restart: the cache was in-memory. Decide once, deliberately, which
  caches are allowed to be empty after a restart.
- A counter that looks wrong: two code paths use different caches for the same key.
- A lock that never comes free: your client is alive and holding it. Check the guard's lifetime
  before you blame the lease window.
