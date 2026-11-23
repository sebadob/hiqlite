// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::from_row::impl_from_row;
use crate::into_cache_data::impl_cache_variants;
use crate::try_from_row::impl_try_from_row;
use syn::{DeriveInput, parse_macro_input};

mod from_row;
mod indexed;
mod into_cache_data;
mod shape;
mod try_from_row;

#[cfg(feature = "sql-check")]
mod check;
#[cfg(feature = "sql-check")]
mod sql_macro;

/// Derives `From<&mut hiqlite::Row<'_>>` for a struct, mapping each named field to a column.
///
/// By default a field is read by its own name via `row.get(name)`. The following per-field
/// `#[column(...)]` attributes are supported:
///
/// - `rename = "some_column"` reads a different column name; can be combined with one of the
///   conversion attributes below
/// - `skip` uses `Default::default()` instead of reading the column
/// - `flatten` converts via `TryFrom<&mut hiqlite::Row>` — use this for nested structs, enums, or
///   any custom type that implements `From<&mut hiqlite::Row<'_>>`
/// - `parse` reads a `String` and calls `str::parse()` (requires `T: FromStr`)
/// - `from_string` reads a `String` and converts via `From<String>`
/// - `from_i64` reads an `i64` and converts via `From<i64>` (std implements this for i64 and i128)
/// - `from_i32` reads an `i64`, panics if it does not fit into an `i32` (instead of clamping), and
///   converts via `From<i32>` (std implements this for i8/i16/i32)
///
/// Unsigned integer targets (u8/u16/u32/u64/usize) are not supported by `from_i32`/`from_i64`;
/// enable the `cast_ints` feature and drop the attribute, or use `parse`.
///
/// Both derives also implement the hoisted mapping traits `hiqlite::FromRowIndexed` and
/// `hiqlite::TryFromRowIndexed`, and register the type in the query engine, which is what makes
/// `query_map()` and `query_try_map()` resolve the field names of the struct once per query and map
/// every row by index.
///
/// Registration has to be something the mapping body can always run, so three kinds of struct stay
/// on the name-based path they have always taken: a struct with a lifetime parameter is never
/// `'static` and cannot enter the registry; a struct with a `flatten` field cannot assume that its
/// nested type implements the hoisted traits, because that type only has to provide a
/// `From<&mut Row>`; and a struct with a hand-written `impl From<&mut Row>` registers nothing.
///
/// The shape a `sql_map!()` checks a query against is published by every type, and a `flatten` field
/// contributes the shape of its own type, as a bound on it.  Every part of a flattening struct
/// therefore has to publish names: by deriving `FromRow`, by deriving [`SqlShape`] next to a
/// hand-written mapping, or by implementing `hiqlite::SqlShape` by hand.  A part that publishes
/// none is an error at its parent, naming it, because the columns of a hand-written body are the one
/// thing no derive can find out.
#[proc_macro_derive(FromRow, attributes(column))]
pub fn from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_from_row(input) {
        Ok(ts) => ts.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derives `TryFrom<&mut hiqlite::Row<'_>>` for a struct, the non-panicking counterpart of
/// [`FromRow`]: same column mapping and per-field `#[column(...)]` attributes, but every
/// failure is returned as `hiqlite::Error` instead of panicking.
///
/// By default a field is read by its own name via `row.try_get(name)`. The following per-field
/// `#[column(...)]` attributes are supported:
///
/// - `rename = "some_column"` reads a different column name; can be combined with one of the
///   conversion attributes below
/// - `skip` uses `Default::default()` instead of reading the column
/// - `flatten` converts via `TryFrom<&mut hiqlite::Row>` — use this for nested structs, enums, or
///   any custom type that implements `TryFrom<&mut hiqlite::Row<'_>>`
/// - `parse` reads a `String` and calls `str::parse()` (requires `T: FromStr`)
/// - `from_string` reads a `String` and converts via `From<String>`
/// - `from_i64` reads an `i64` and converts via `From<i64>` (std implements this for i64 and i128)
/// - `from_i32` reads an `i64`, fails if it does not fit into an `i32` (instead of clamping), and
///   converts via `From<i32>` (std implements this for i8/i16/i32)
///
/// Unsigned integer targets (u8/u16/u32/u64/usize) are not supported by `from_i32`/`from_i64`;
/// enable the `cast_ints` feature and drop the attribute, or use `parse`.
///
/// A `TryFromRow` type publishes no shape of its own, because `query_try_map()` reports a column
/// mismatch as an error at runtime.  Where you want the compile-time column check of [`sql_map!`]
/// for one, derive [`SqlShape`] next to it.
#[proc_macro_derive(TryFromRow, attributes(column))]
pub fn try_from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_try_from_row(input) {
        Ok(ts) => ts.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derives `hiqlite::CacheVariants` for a flat enum (no variant values, no generics).
///
/// The variant order is the cache index and must stay stable across versions: only append new
/// variants at the end. See the `CacheVariants` trait docs for compatibility rules.
#[proc_macro_derive(CacheVariants)]
pub fn cache_variants(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_cache_variants(input) {
        Ok(ts) => ts.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derives `hiqlite::SqlShape` for a struct: the column names its fields read, published for
/// [`sql_map!`], and nothing else — no row mapping, no registration. A release build that does not
/// opt into `sql-check-release` gets no shape impl at all, because nothing in it can compare one.
///
/// A type mapped by a hand-written `impl From<&mut Row>` has no derive that could know its columns,
/// and the parent that flattens it needs exactly those names: a `sql_map!()` of the parent is
/// checked against the columns of the query, and a column that no part can name cannot be checked.
/// Publish them here, on the same fields and with the same `#[column(...)]` attributes the
/// hand-written body reads, where `rename` applies and `skip` and `flatten` mean what they mean in
/// `FromRow`:
///
/// ```text
/// #[derive(SqlShape)]
/// struct Sub {
///     #[column(rename = "sub_id")]
///     id: i64,
///     name: String,
/// }
///
/// // the mapping stays hand-written: it reads `sub_id` and `name`
/// impl From<&mut hiqlite::Row<'_>> for Sub { /* ... */ }
///
/// #[derive(FromRow)]
/// struct Parent {
///     id: i64,
///     #[column(flatten)]
///     sub: Sub,
/// }
/// ```
///
/// A type that is not a struct — an enum, a wrapper — publishes its shape by hand, exactly like the
/// `sql_shape` fn in the docs of the trait shows:
///
/// ```text
/// impl hiqlite::SqlShape for MyEnum {
///     const SHAPE: u64 = hiqlite::sql_shape(&["enum_value"]);
/// }
/// ```
///
/// Only the compile-time column check needs a shape.  A type that publishes none, and whose queries
/// go through `sql!()` or a plain string, compiles and runs unchanged.
///
/// Derive it next to a mapping this crate does not emit: `FromRow` publishes the shape of the type it
/// maps already, and deriving both would implement `hiqlite::SqlShape` twice.
#[proc_macro_derive(SqlShape, attributes(column))]
pub fn sql_shape(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match shape::impl_shape(input) {
        Ok(ts) => ts.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Checks a SQL string at compile time and returns it, unchanged, as a `Cow<'static, str>`.
///
/// The string is prepared against an in-memory SQLite built from the `migrations` directory that
/// sits next to the `Cargo.toml` of the crate using the macro - the same directory `migrate()`
/// applies. A statement SQLite cannot prepare fails the build with SQLite's own message, pointing
/// at the string:
///
/// ```text
/// error: no such column: descrption
///   --> src/db.rs:41:31
///    |
/// 41 |     let rows = client.query_map(sql!("SELECT id, descrption FROM test"), params!()).await?;
///    |                               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
/// ```
///
/// The macro is a validator only: it emits the literal it was given, unchanged, so it plugs into
/// every call site that takes a query - `execute`, `query`, `query_map`, `query_try_map` and their
/// variants - with nothing to adapt and no runtime cost.
///
/// The check runs in debug builds. A release build compiles the literal untouched: no database is
/// built, no cache is read, and nothing is emitted. The `sql-check-release` feature of `hiqlite`
/// turns the check on for release builds as well.
///
/// ```text
/// client.query(sql!("SELECT id FROM test WHERE ts > ?"), params!(1))
/// ```
///
/// Not checked: the number of bound parameters, because `params!()` is a separate call the macro
/// cannot see; the fields of the type a query maps to, which [`sql_map!`] checks; column types
/// against the fields of a mapped struct; and anything built at runtime with `format!`, which keeps
/// taking a plain string.
///
/// A `migrations` directory somewhere else is set with the `HQL_MIGRATIONS` env var, which cargo
/// hands to the compiler from `.cargo/config.toml`. The value is relative to the `Cargo.toml` of
/// the crate using the macro, exactly like a `#[folder]` of `RustEmbed`, and an absolute path is
/// used as it is:
///
/// ```text
/// # .cargo/config.toml of the crate, or of the workspace it lives in
/// [env]
/// HQL_MIGRATIONS = "src/migrations"
///
/// # resolved against the directory of this config file, for a workspace member
/// HQL_MIGRATIONS = { value = "../shared/migrations", relative = true }
/// ```
///
/// The var is read at compile time only, it is never read at runtime, and nothing is loaded from a
/// `.env` file. A directory that cannot be read, and one with no `.sql` files, is a warning naming
/// `HQL_MIGRATIONS`, and the queries of that crate stay unchecked.
///
/// A crate without a `migrations` directory, and any problem inside the checker itself, is
/// reported once as a warning and leaves queries unchecked - the check can never break a build.
#[cfg(feature = "sql-check")]
#[proc_macro]
pub fn sql(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    sql_macro::Expansion::Single.expand(input)
}

/// Checks a query's result columns against the fields of the type it maps to, and emits the query.
///
/// The type is checked by its column names: they must be exactly the columns the query returns, in
/// any order and any case, which is the same matching a mapped row does. A query that reads a
/// column the type has no field for, or a field the query does not return, fails the build at the
/// call site:
///
/// ```text
/// error: this query returns the columns (id, ts) - the mapped type must read exactly these, in
/// any order and case; `sql_map!` checks them at compile time
/// ```
///
/// The `FromRow` derive publishes the shape of every type it maps, so a derived type needs nothing
/// extra; a hand-written mapping implements `hiqlite::SqlShape` by listing the column names it
/// reads. A type with flattened fields is checked against the names of all its parts, since the
/// query has to return all of them.
///
/// ```text
/// client.query_map_one(sql_map!(Test, "SELECT id, ts FROM test WHERE id = $1"), params!(1))
/// ```
///
/// Column *types* are not checked - that is what `query_try_map` reports as an error at runtime -
/// and neither is the number of bound parameters. Like `sql!`, this runs in debug builds only,
/// unless the crate opts into `sql-check-release`.
#[cfg(feature = "sql-check")]
#[proc_macro]
pub fn sql_map(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    sql_macro::Expansion::Mapped.expand(input)
}

/// The multi-statement counterpart of [`sql`], for `batch()` and `txn()`.
///
/// Every statement of the string is prepared in order, the way the writer runs a batch. Results
/// are cached per query string and schema, so hundreds of invocations cost a hash lookup.
#[cfg(feature = "sql-check")]
#[proc_macro]
pub fn sql_batch(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    sql_macro::Expansion::Batch.expand(input)
}
