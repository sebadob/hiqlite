# Queries

The data path: how a write gets into the cluster, how a read comes out, and how rows become your
types.

## Writes

```rust, notest
use hiqlite::macros::params;

// a single statement, rows affected
client
    .execute("UPDATE test SET num = $1 WHERE id = $2", params!(42, "id1"))
    .await?;

// many statements, one commit
let results = client
    .txn([
        ("INSERT INTO test (id, num) VALUES ($1, $2)", params!("a", 1)),
        ("UPDATE test SET num = $3 WHERE id = $4", params!(2, "a")),
    ])
    .await?;
for per_statement in results {
    let rows_affected = per_statement?;
}

// many statements in one string
let results = client
    .batch("INSERT INTO test (id, num) VALUES ('b', 1); DELETE FROM test WHERE id = 'a';")
    .await?;
```

- `Client::execute` - one statement, params bound, returns rows affected.
- `Client::txn` - a list of `(sql, params)` applied as one unit. You get a result per statement in
  the order you gave them. An outer error means the unit did not commit.
- `Client::batch` - several statements in one string. **These are not prepared statements**, so you
  must validate and sanitize the string yourself. Never build a batch from user input.
- Rate limiting, when configured, applies to all of these.

Placeholders are `$1`, `$2`, ... and values come from `params!`. A write on the leader is applied
locally; a write on any other node is forwarded to the leader, and you do not implement that.

## Params

`params!()` for none, `params!(a, b, ...)` for values. Supported: null, bool, the integer types,
`f32`, `f64`, `&str`, `String`, `&[u8]`, `Vec<u8>`, `serde_json::Value`, `chrono` dates, times and
datetimes, `url::Url`, `uuid::Uuid`, `Option` of these, and column bindings via `StmtColumn` and
`StmtIndex`.

Never build SQL by concatenating values. Non-deterministic functions are refused on the write path,
see `limits.md`.

## Writes that give rows back

```rust, notest
let row = client
    .execute_returning_one(
        "INSERT INTO test (id, num) VALUES ($1, $2) RETURNING id, num",
        params!("c", 7),
    )
    .await?;
let id: String = row.get("id");

let rows = client
    .execute_returning_map::<_, Test>(
        "UPDATE test SET num = $1 WHERE id = $2 RETURNING *",
        params!(9, "c"),
    )
    .await?;
```

- `Client::execute_returning` - raw rows.
- `Client::execute_returning_one` - exactly one raw row, error otherwise.
- `Client::execute_returning_map` - rows mapped to your type.
- `Client::execute_returning_map_one` - exactly one mapped row, error otherwise.

## Reads

```rust, notest
// mapped to a struct that derives FromRow
let rows: Vec<Test> = client.query_map("SELECT * FROM test", params!()).await?;
let one: Test = client.query_map_one("SELECT * FROM test WHERE id = $1", params!("a")).await?;

// mapped with serde
let rows: Vec<Test> = client.query_as("SELECT * FROM test", params!()).await?;

// raw rows
let rows = client.query_raw("SELECT id, num FROM test", params!()).await?;
for mut row in rows {
    let id: String = row.get("id");
    let num: i64 = row.get("num");
}
```

Every read has the same three shapes: many, one, optional.

- `Client::query_map`, `Client::query_map_one`, `Client::query_map_optional` - types that implement
  `From<&mut Row>`, which `FromRow` derives for you.
- `Client::query_try_map`, `Client::query_try_map_one`, `Client::query_try_map_optional` - the same,
  with a per-row error instead of a panic.
- `Client::query_as`, `Client::query_as_one`, `Client::query_as_optional` - serde `Deserialize`
  types. Less code, less control.
- `Client::query_raw`, `Client::query_raw_one` - `Row` handles, `get` and `try_get` by column name.
- `Client::query_consistent`, `Client::query_consistent_map` - the read runs on the leader and
  confirms the current state. Pay a round trip for it, only where a stale read is wrong.

A local read is as fast as your local SQLite is. A consistent read is a network call.

## Mapping rows

```rust, notest
use hiqlite::macros::FromRow;

#[derive(FromRow, Debug)]
struct Test {
    id: String,
    num: i64,
    description: Option<String>,
}
```

`FromRow` maps by column name and panics on a type mismatch: a column that can never map to that
field is a code bug, and a panic is faster to find than a logged error. `TryFromRow` gives you a
`Result` per row instead. Both take optional `column` attributes for names and conversions; the
`derive-complex-types` example shows them.

A SQLite `INTEGER` is an `i64`. To read it as `u32`, `i16`, `u8` and friends, enable `cast_ints`
(checked) or `cast_ints_unchecked` (faster, assumes your inserts are in range). `VecText` stores a
`Vec<T>` in a text column. `Row` and `RowOwned` are the raw row handles.

## Transactions and locks

`Client::txn` commits a set of statements as one unit. It is not a session: you cannot read, decide
in your code, then write inside the same transaction.

When your flow is read → compute → write and the row must not change underneath you, take a
distributed lock instead:

```rust, notest
let lock = client.lock("balance:id1").await?;
// read, compute, write
assert!(lock.is_locked());
// released when the guard is dropped
```

Locks have a lease window. A live client keeps extending it; a crashed one cannot block the key
forever. See `caches.md`.

## Remote clients

`Client::remote(nodes, tls, tls_no_verify, api_secret, with_proxy, rate_limit_cache, rate_limit_db)`
gives you a client without a local node. Pass every node address so you survive a leader change,
and `with_proxy: true` when you point it at a proxy. Everything works, reads included, over the
network.

## Errors

Recoverable failures return `Error`: a leader redirect, a network failure, a query that returned no
rows, a statement SQLite rejects. A leader redirect means the node you talked to is not the leader;
the reply names the current leader. The `Client` of an embedded node follows the leader on its own.

Type mismatches in a row mapping panic. They cannot be fixed by retrying. Use the `try_` variants
when your schema can legitimately produce a value your type cannot hold.

## Migrations

```rust, notest
use hiqlite::macros::embed::*;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

client.migrate::<Migrations>().await?;
```

Files are applied in name order, once each, and `AppliedMigration` tells you what ran.
`Client::try_migrate` returns an error instead of panicking on a bad migration.
