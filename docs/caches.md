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

Each variant is one cache, and the variant's position is its index: variant 0 is cache 0, variant 1
is cache 1. The index, not the name, is what the cluster stores. It is encoded in the cache group's
log entries and in its snapshots. Every node in the cluster must run the same enum in the same
order, which is trivially true when the enum lives in the crate every node is built from.

Implement `CacheVariants` by hand only when a derive is impossible. The derive guarantees dense
ascending indexes; a hand-written implementation is where the index panic comes from.

## The cache index is positional

**Never reorder a variant, and never insert one in the middle.** An insert in the middle re-indexes
everything behind it. The index is positional, so a shifted enum writes your values into the wrong
caches - session data landing in the rate-limit cache - and nothing fails on the write. Your enum
is append-only: when you need a new cache, add the variant at the end. A rename, a removal, or a
swap of two names is a deploy with steps - see "Removing or Renaming a variant".

The node enforces this at startup, in two checks.

1. The index list must be dense and ascending, `0..n-1`, with no gaps and no duplicates. A
   hand-written implementation gets this wrong and the node panics at startup, naming the range it
   expected.
2. The node keeps a fingerprint of the enum it has persisted - the `<index> <name>` pairs in
   `cache_index.meta`, inside the cache state machine directory - and compares your enum against it
   on every start. A pure append at the end is accepted, and the fingerprint grows with it. A
   re-order, an insert in the middle, a removal, or a rename makes the node refuse to start, and it
   prints the stored list and your current list side by side. That message is the whole diagnosis:
   this deploy would have mixed up your caches.

The fingerprint check applies when cache state is persisted: `cache_storage_disk` on, and
`in-memory-snapshots` off. A node that keeps no cache state on disk has nothing to be incompatible
with.

The variant name is part of the fingerprint, and it is what you see in logs. A rename changes the
fingerprint, and the check treats it as one. See "Removing or Renaming a variant" for how to do it
right.

## Removing or Renaming a variant

**Keeping the variant and stopping to use it is the answer for most cases.** A variant you never
touch is not free: the node runs a background task for it, allocates its channels, and carries it in
the cache snapshots. It is cheap next to a re-index, and it is the only option with no operational
work attached.

When you do change the enum, the startup check is what keeps you safe: the node refuses a deploy
that would mix your caches up, so you find out at startup instead of in production.

**A single rename needs the metadata reset and nothing else.** The index keeps its entries, only the
name on that index changes, so the cache still means the same thing. `HQL_CACHE_META_RESET=true`
deletes the stored fingerprint, the node accepts your current enum, and the fingerprint is written
from it. Nothing else is touched: your database, the Raft logs, and your cache content stay exactly
where they are.

```bash
HQL_CACHE_META_RESET=true ./your-app   # for that one start, then unset it
```

Every node keeps its own fingerprint file, so every node of the cluster needs that reset on its
start; a node that starts without it refuses. It is env only, so set it for a single start and
remove it right after. It applies wherever the fingerprint check applies: `cache_storage_disk` on
and `in-memory-snapshots` off. A node that persists no cache state has no fingerprint to reset.

**A removal needs the metadata reset on every node and a full cache clear, and that guarantee is
yours to give.** The entries of the removed index are still in the persisted state, and every index
behind it has shifted, so nothing may read a cache before every node has cleared all of them:

```rust, notest
// the removal deploy: clear everything first, on every node
client.clear_cache_all().await?;
```

Skip that and the node answers with the entries of a shifted index: data from the wrong cache, and
no error anywhere. Your durable data is in the database; treat everything you keep only in caches as
gone.

**Swapping the names of two variants is a removal, not a rename.** Each index keeps its entries and
each name ends up pointing at somebody else's data. Metadata reset on every node, then a full cache
clear, exactly like a removal.

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

- A node refuses to start with a stored-vs-current variant list: your enum changed. Revert it,
  append only, or reset the cache index metadata on every node for a rename. See "Removing or
  Renaming a variant".
- A cache that answers with the entries of a different cache after you removed a variant: the caches
  were not cleared at startup. Clear all caches on every node and start clean.
- A node answers "cache index out of range": the enum of that node does not match the cluster. Fix
  the enum, restart the nodes, and treat the entries in the mismatched cache as garbage.
- Cache entries gone after a restart: the cache was in-memory. Decide once, deliberately, which
  caches are allowed to be empty after a restart.
- A counter that looks wrong: two code paths use different caches for the same key.
- A lock that never comes free: your client is alive and holding it. Check the guard's lifetime
  before you blame the lease window.
