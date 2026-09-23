// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::from_row::impl_from_row;
use crate::into_cache_data::impl_cache_variants;
use syn::{DeriveInput, parse_macro_input};

mod from_row;
mod into_cache_data;

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
#[proc_macro_derive(FromRow, attributes(column))]
pub fn from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_from_row(input) {
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
