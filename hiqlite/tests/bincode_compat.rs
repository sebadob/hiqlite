//! 1:1 byte-compatibility tests: `bincode` (v2) vs `bincode-next`, serde path.
//!
//! This is the guarantee that lets us swap crates without changing persisted
//! bytes on the SQLite/Raft-log path. It does NOT depend on hiqlite's concrete
//! types: it exercises the *entire* serde data model those types are built
//! from (every int width, floats, bool, Option, String/Cow, Vec incl. blobs and
//! empty, tuples, arrays, maps, and all four enum variant kinds). If two
//! encoders agree on this closed set under a given config, they agree on any
//! serde type composed of it — which is exactly what `QueryWrite`, `Param`,
//! `Response`, openraft entries, etc. are.
//!
//! For each value and each config we assert:
//!   * bincode2 bytes == bincode-next bytes (the 1:1 claim), and
//!   * bidirectional cross-decode: each crate decodes the other's bytes back to
//!     the original, consuming exactly `len` bytes. That proves a new node can
//!     read data written by an old one and vice versa.
//!
//! Configs mirror `hiqlite-wal/src/log_store_impl.rs`:
//!   * legacy   = LittleEndian + Fixint  (the persisted Raft-log format today)
//!   * standard = LittleEndian + Varint  (bincode's default; the wire format)

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Kitchen-sink struct covering every primitive hiqlite serializes. Field set
/// is intentionally exhaustive rather than minimal: this is the compatibility
/// contract, not a sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct KitchenSink {
    i8v: i8,
    i16v: i16,
    i32v: i32,
    i64v: i64,
    isizev: isize,
    u8v: u8,
    u16v: u16,
    u32v: u32,
    u64v: u64,
    usizev: usize,
    f32v: f32,
    f64v: f64,
    boolv: bool,
    opt_none: Option<i64>,
    opt_some: Option<String>,
    strv: String,
    cow: Cow<'static, str>,
    vec_i64: Vec<i64>,
    blob: Vec<u8>,
    pair: (usize, usize),
    array3: [u8; 3],
    mapv: BTreeMap<String, i64>,
    nested: NestedEnum,
}

/// All four serde enum variant shapes, with mixed payload types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum NestedEnum {
    Unit,
    Newtype(i64),
    Tuple(usize, String, f64),
    Struct { a: i32, b: bool },
}

/// Assert the 1:1 + cross-decode contract for one value under both configs.
fn check<T>(v: T, label: &str)
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + PartialEq,
{
    // legacy = LittleEndian + Fixint (the persisted Raft-log format).
    let b2 = bincode::serde::encode_to_vec(&v, bincode::config::legacy()).unwrap();
    let bn = bincode_next::serde::encode_to_vec(&v, bincode_next::config::legacy()).unwrap();
    assert_eq!(b2, bn, "{label}: legacy bytes differ");

    let (d, n) =
        bincode_next::serde::decode_from_slice::<T, _>(&b2, bincode_next::config::legacy())
            .unwrap();
    assert_eq!(
        d, v,
        "{label}: bincode-next failed to decode bincode2 legacy bytes"
    );
    assert_eq!(
        n,
        b2.len(),
        "{label}: legacy consumed-length mismatch (bincode-next)"
    );

    let (d, n) = bincode::serde::decode_from_slice::<T, _>(&bn, bincode::config::legacy()).unwrap();
    assert_eq!(
        d, v,
        "{label}: bincode2 failed to decode bincode-next legacy bytes"
    );
    assert_eq!(
        n,
        bn.len(),
        "{label}: legacy consumed-length mismatch (bincode2)"
    );

    // standard = LittleEndian + Varint (the default wire format).
    let b2s = bincode::serde::encode_to_vec(&v, bincode::config::standard()).unwrap();
    let bns = bincode_next::serde::encode_to_vec(&v, bincode_next::config::standard()).unwrap();
    assert_eq!(b2s, bns, "{label}: standard bytes differ");

    let (d, n) =
        bincode_next::serde::decode_from_slice::<T, _>(&b2s, bincode_next::config::standard())
            .unwrap();
    assert_eq!(
        d, v,
        "{label}: bincode-next failed to decode bincode2 standard bytes"
    );
    assert_eq!(
        n,
        b2s.len(),
        "{label}: standard consumed-length mismatch (bincode-next)"
    );

    let (d, n) =
        bincode::serde::decode_from_slice::<T, _>(&bns, bincode::config::standard()).unwrap();
    assert_eq!(
        d, v,
        "{label}: bincode2 failed to decode bincode-next standard bytes"
    );
    assert_eq!(
        n,
        bns.len(),
        "{label}: standard consumed-length mismatch (bincode2)"
    );
}

#[test]
fn full_data_model_is_byte_identical() {
    let sink = KitchenSink {
        i8v: i8::MIN,
        i16v: i16::MAX,
        i32v: -1_000_000,
        i64v: i64::MIN,
        isizev: isize::MAX,
        u8v: u8::MAX,
        u16v: 0,
        u32v: u32::MAX,
        u64v: u64::MAX,
        usizev: usize::MAX,
        f32v: -2.25,
        f64v: 1e308,
        boolv: true,
        opt_none: None,
        opt_some: Some("some".into()),
        strv: "a string with a space and ünïcode".into(),
        cow: Cow::Borrowed("borrowed"),
        vec_i64: vec![0, -1, i64::MAX],
        blob: vec![0u8, 1, 2, 254, 255],
        pair: (7usize, 99usize),
        array3: [1, 2, 3],
        mapv: {
            let mut m = BTreeMap::new();
            m.insert("k1".to_string(), 1i64);
            m.insert("k2".to_string(), -2i64);
            m
        },
        nested: NestedEnum::Struct { a: -3, b: true },
    };
    check(sink.clone(), "KitchenSink");

    // Exercise the other three enum variant shapes at the top level too.
    check(NestedEnum::Unit, "enum::unit");
    check(NestedEnum::Newtype(42), "enum::newtype");
    check(NestedEnum::Tuple(3usize, "x".into(), 1.5f64), "enum::tuple");
}

#[test]
fn edge_values_are_byte_identical() {
    // Every integer width, both extremes where meaningful.
    check(i8::MIN, "i8::MIN");
    check(i16::MAX, "i16::MAX");
    check(i32::MIN, "i32::MIN");
    check(i64::MAX, "i64::MAX");
    check(u8::MAX, "u8::MAX");
    check(u16::MIN, "u16::MIN");
    check(u32::MAX, "u32::MAX");
    check(u64::MIN, "u64::MIN");

    // Floats (finite, non-NaN so we test the value path, not NaN bit quirks).
    check(0.0f32, "f32 zero");
    check(f64::MIN_POSITIVE, "f64 min positive");

    check(true, "bool true");
    check(false, "bool false");

    // Option at the top level (serde discriminant 0/1).
    check(Option::<i64>::None, "Option None");
    check(Some(5i64), "Option Some(i64)");
    check(Some("s".to_string()), "Option Some(String)");

    // Strings and Cow<str> (Cow serializes as the inner str).
    check(String::new(), "empty String");
    check(Cow::<str>::Borrowed(""), "empty Cow<str>");
    check(Cow::<str>::Owned("owned".into()), "owned Cow<str>");

    // Collections, including empty (length prefix == 0) and blobs.
    check(Vec::<i64>::new(), "empty Vec<i64>");
    check(vec![0i64; 1], "Vec<i64> len 1");
    check(BTreeMap::<String, i64>::new(), "empty BTreeMap");
    check(vec![0u8; 32], "Vec<u8> blob len 32");

    // Tuples and arrays.
    check((0usize, 0usize), "(usize, usize) zero");
    check([0u8; 16], "[u8; 16]");
}

/// Sanity that the two configs really are different code paths (so a passing
/// byte-equality test above is meaningful, not both sides using one config).
#[test]
fn legacy_and_standard_configs_differ() {
    // A small value: fixint is always 8 bytes for u64; varint is 1 byte.
    let v = 1u64;
    let b2l = bincode::serde::encode_to_vec(v, bincode::config::legacy()).unwrap();
    let b2s = bincode::serde::encode_to_vec(v, bincode::config::standard()).unwrap();
    assert_eq!(b2l.len(), 8, "fixint u64 must be 8 bytes");
    assert_eq!(b2s.len(), 1, "varint small u64 must be 1 byte");
    assert_ne!(b2l, b2s);

    // And bincode-next agrees with both.
    let bn_l = bincode_next::serde::encode_to_vec(v, bincode_next::config::legacy()).unwrap();
    let bn_s = bincode_next::serde::encode_to_vec(v, bincode_next::config::standard()).unwrap();
    assert_eq!(bn_l, b2l);
    assert_eq!(bn_s, b2s);
}
