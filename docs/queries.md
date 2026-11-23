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

## Checked SQL

`sql!()` checks a query string while your crate compiles and hands it back as the string itself, so
it takes the place of a string literal at any call site that takes a query:

```rust, notest
use hiqlite::macros::sql;

let rows = client.query_map(sql!("SELECT id, num FROM test"), params!()).await?;
```

`sql_map!(Type, "…")` is the shape that also checks the **columns** of the query against the
fields of the type it maps to. They must be the same names, in any order and any case, which is
the same matching a mapped row does, and a mismatch stops the build at the call site:

```rust, notest
use hiqlite::macros::sql_map;

let row = client
    .query_map_one(sql_map!(Test, "SELECT id, num FROM test WHERE id = $1"), params!(1))
    .await?;
```

```text, notest
error: this query returns the columns (id, ts) - the mapped type must read exactly these, in any
order and case; `sql_map!` checks them at compile time
```

The `FromRow` derive publishes the shape of every type it maps, including the parts it
flattens, so a derived type needs nothing. A type mapped only through `TryFromRow`, and a
hand-written mapping, publishes it with `#[derive(SqlShape)]` next to that mapping: same fields, same `column`
attributes, and no mapping emitted. A type that is not a struct lists the names it reads
(`impl SqlShape for T { const SHAPE: u64 = sql_shape(&["id", "num"]); }`).

The string is prepared against an in-memory SQLite built from the `migrations` directory that sits
next to your crate's `Cargo.toml`, the same directory `Client::migrate` applies (see Migrations
below). A table that is not there, a column that is not there, a keyword SQLite cannot parse: the
build stops with SQLite's own message, pointing at the string.

- `sql!()` takes exactly one statement. `sql_batch!()` is the multi-statement shape for
  `Client::batch` and `Client::txn`.
- The macro emits your literal unchanged, so nothing about your calls adapts and nothing runs at
  runtime.
- Results are cached in `.hiqlite-sql/` in the workspace root, keyed by your migrations and the
  query text. The directory is gitignored, deleting it is always safe, and compiling refills it.
  Editing a migration file recompiles the crates that check against it and invalidates their
  verdicts. Adding one does not: a compiler notices a file it reads, not a file that appeared, so
  after you add a migration, rebuild the crates that use `sql!()` against it - a clean build of
  them, or any change to their code - to get their queries checked against the new schema.
- A crate without a `migrations` directory is not checked: you get one `warning:` line per compile
  and your queries behave exactly as before. The check never breaks a build for a reason inside
  itself.
- Migrations somewhere else are set with `HQL_MIGRATIONS`, which cargo hands to the compiler from
  `.cargo/config.toml`. The value is relative to the `Cargo.toml` of the crate that uses the macro,
  like the `#[folder]` of `RustEmbed`, and an absolute path is used as it is:

  ```toml, notest
  # .cargo/config.toml of the crate, or of the workspace it lives in
  [env]
  HQL_MIGRATIONS = "src/migrations"

  # relative to the directory of this config file, for a workspace member
  HQL_MIGRATIONS = { value = "../shared/migrations", relative = true }
  ```

  The macro reads it while it compiles and never at runtime, and it never loads a `.env` file. A
  directory that cannot be read, and one without `.sql` files, is a warning naming `HQL_MIGRATIONS`
  and leaves your queries unchecked.
- A shape exists exactly where a check can run: a release build that does not opt into
  `sql-check-release` gets no shape impl from the derives at all, and compares nothing.
- The checks run in debug builds. A release build compiles your literals untouched: no SQLite is
  built, no cache is read, nothing extra is emitted. The `sql-check-release` feature of `hiqlite`
  turns the checks on for release builds as well.
- Not checked: the parameters (they are `params!()`, a separate call), the **types** of the columns
  you map (that is what `query_try_map()` reports as an error at runtime), and any query you build
  at runtime with `format!`.

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

A `flatten` field is mapped by the `From<&mut Row>` of its own type: the way a nested struct, an
`enum` or any custom type gets in. Every part of it has to publish the names it reads, by deriving
`FromRow` or `SqlShape`, or by a `SqlShape` impl, because a `sql_map!()` of the parent checks all of
them together: a part that publishes nothing is an error at its parent, naming that part.

A field whose column is not in the result set is reported by the first row that would read it: the
panicking family names that field in its panic, `TryFromRow` answers it as an error. A query that
returns no rows answers an empty `Vec`, whatever its fields are named - there is no row to read the
missing column.

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
