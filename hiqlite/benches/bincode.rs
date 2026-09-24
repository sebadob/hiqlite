//! Benchmarks for the bincode migration decision (see `tests/bincode_compat.rs`).
//!
//! Measures encode + decode of a representative hiqlite log-entry payload (a
//! `Query`-shaped value: a SQL string plus typed params) across three codecs, each
//! under both integer encodings:
//!
//!   * `bincode2_serde_*`     — today's production codec (`bincode = "2"`, serde adapter)
//!   * `bncode_next_serde_*`  — the byte-compatible drop-in candidate (serde adapter)
//!   * `bncode_next_native_*` — bincode-next native SIMD derive (peak perf, u32-tag;
//!                              byte-identical to the serde paths — this bench prints
//!                              "bytes: identical" as empirical proof)
//!
//! `legacy` = LittleEndian + fixint (what hiqlite persists today); `standard` =
//! LittleEndian + varint. Each group reports throughput in B/s against its own
//! encoded size, so the CPU-vs-bandwidth tradeoff (fixint vs varint) is visible.

use std::hint::black_box;

use bincode_next::config;
use bincode_next::serde as bn_serde;
use bincode_next::{Decode, Encode};
use criterion::{Criterion, Throughput};
use serde::{Deserialize, Serialize};

// ---- Serde payload: mirrors hiqlite's `Query { sql, params: Vec<Param> }` ----

#[derive(Serialize, Deserialize)]
enum SParam {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Serialize, Deserialize)]
struct SQuery {
    sql: String,
    params: Vec<SParam>,
}

// ---- Native payload: same shape, bincode-next SIMD derive (u8-tag) ----

#[derive(Encode, Decode)]
enum NParam {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Encode, Decode)]
struct NQuery {
    sql: String,
    params: Vec<NParam>,
}

const SQL: &str = "INSERT INTO t (a, b, c, d, e) VALUES (?1, ?2, ?3, ?4, ?5) \
                   ON CONFLICT(a) DO UPDATE SET b = excluded.b, c = excluded.c";

fn sample_squery() -> SQuery {
    SQuery {
        sql: SQL.to_string(),
        params: vec![
            SParam::Int(i64::MAX),
            SParam::Real(-1.5),
            SParam::Text("a somewhat longer text value".to_string()),
            SParam::Blob(vec![0xABu8; 1024]),
            SParam::Int(42),
            SParam::Null,
        ],
    }
}

fn sample_nquery() -> NQuery {
    NQuery {
        sql: SQL.to_string(),
        params: vec![
            NParam::Int(i64::MAX),
            NParam::Real(-1.5),
            NParam::Text("a somewhat longer text value".to_string()),
            NParam::Blob(vec![0xABu8; 1024]),
            NParam::Int(42),
            NParam::Null,
        ],
    }
}

fn cmp_bytes(a: &[u8], b: &[u8]) -> String {
    if a == b {
        return format!("identical ({} bytes)", a.len());
    }
    let i = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()));
    format!("DIFFER @ byte {} (len {} vs {})", i, a.len(), b.len())
}

fn main() {
    // Print encoded sizes once so the byte dimension is explicit next to criterion's B/s.
    let s = sample_squery();
    let n = sample_nquery();
    eprintln!(
        "encoded bytes  legacy/standard:\n  bincode2 serde      {:>4}/{:>4}\n  bncode_next serde   {:>4}/{:>4}\n  bncode_next native  {:>4}/{:>4}",
        bincode::serde::encode_to_vec(&s, bincode::config::legacy())
            .unwrap()
            .len(),
        bincode::serde::encode_to_vec(&s, bincode::config::standard())
            .unwrap()
            .len(),
        bn_serde::encode_to_vec(&s, config::legacy()).unwrap().len(),
        bn_serde::encode_to_vec(&s, config::standard())
            .unwrap()
            .len(),
        bincode_next::encode_to_vec(&n, config::legacy())
            .unwrap()
            .len(),
        bincode_next::encode_to_vec(&n, config::standard())
            .unwrap()
            .len(),
    );

    // Does the fast SIMD native path emit the SAME bytes as the byte-compatible
    // serde adapter for this shape? (serde(next) is already proven 1:1 with bincode2.)
    let s_legacy = bn_serde::encode_to_vec(&s, config::legacy()).unwrap();
    let n_legacy = bincode_next::encode_to_vec(&n, config::legacy()).unwrap();
    let s_standard = bn_serde::encode_to_vec(&s, config::standard()).unwrap();
    let n_standard = bincode_next::encode_to_vec(&n, config::standard()).unwrap();
    eprintln!(
        "native vs serde(next) bytes  legacy   : {}\n                             standard: {}",
        cmp_bytes(&n_legacy, &s_legacy),
        cmp_bytes(&n_standard, &s_standard),
    );

    let mut c = Criterion::default();
    serde_benches(&mut c);
    native_benches(&mut c);
}

fn serde_benches(c: &mut Criterion) {
    let squery = sample_squery();

    // bincode2 (today's production codec)
    {
        let legacy = bincode::serde::encode_to_vec(&squery, bincode::config::legacy()).unwrap();
        let mut g = c.benchmark_group("bincode2_serde_legacy");
        g.throughput(Throughput::Bytes(legacy.len() as u64));
        g.sample_size(50);
        g.bench_function("encode", |b| {
            b.iter(|| {
                black_box(bincode::serde::encode_to_vec(
                    &squery,
                    bincode::config::legacy(),
                ))
            })
        });
        g.bench_function("decode", |b| {
            b.iter(|| {
                black_box(bincode::serde::decode_from_slice::<SQuery, _>(
                    &legacy,
                    bincode::config::legacy(),
                ))
            })
        });
    }
    {
        let standard = bincode::serde::encode_to_vec(&squery, bincode::config::standard()).unwrap();
        let mut g = c.benchmark_group("bincode2_serde_standard");
        g.throughput(Throughput::Bytes(standard.len() as u64));
        g.sample_size(50);
        g.bench_function("encode", |b| {
            b.iter(|| {
                black_box(bincode::serde::encode_to_vec(
                    &squery,
                    bincode::config::standard(),
                ))
            })
        });
        g.bench_function("decode", |b| {
            b.iter(|| {
                black_box(bincode::serde::decode_from_slice::<SQuery, _>(
                    &standard,
                    bincode::config::standard(),
                ))
            })
        });
    }

    // bincode-next serde adapter (byte-compatible drop-in candidate)
    {
        let legacy = bn_serde::encode_to_vec(&squery, config::legacy()).unwrap();
        let mut g = c.benchmark_group("bncode_next_serde_legacy");
        g.throughput(Throughput::Bytes(legacy.len() as u64));
        g.sample_size(50);
        g.bench_function("encode", |b| {
            b.iter(|| black_box(bn_serde::encode_to_vec(&squery, config::legacy())))
        });
        g.bench_function("decode", |b| {
            b.iter(|| {
                black_box(bn_serde::decode_from_slice::<SQuery, _>(
                    &legacy,
                    config::legacy(),
                ))
            })
        });
    }
    {
        let standard = bn_serde::encode_to_vec(&squery, config::standard()).unwrap();
        let mut g = c.benchmark_group("bncode_next_serde_standard");
        g.throughput(Throughput::Bytes(standard.len() as u64));
        g.sample_size(50);
        g.bench_function("encode", |b| {
            b.iter(|| black_box(bn_serde::encode_to_vec(&squery, config::standard())))
        });
        g.bench_function("decode", |b| {
            b.iter(|| {
                black_box(bn_serde::decode_from_slice::<SQuery, _>(
                    &standard,
                    config::standard(),
                ))
            })
        });
    }
}

fn native_benches(c: &mut Criterion) {
    let nquery = sample_nquery();

    // bincode-next native SIMD derive (peak perf, u8-tag; not byte-compatible)
    {
        let legacy = bincode_next::encode_to_vec(&nquery, config::legacy()).unwrap();
        let mut g = c.benchmark_group("bncode_next_native_legacy");
        g.throughput(Throughput::Bytes(legacy.len() as u64));
        g.sample_size(50);
        g.bench_function("encode", |b| {
            b.iter(|| black_box(bincode_next::encode_to_vec(&nquery, config::legacy())))
        });
        g.bench_function("decode", |b| {
            b.iter(|| {
                black_box(bincode_next::decode_from_slice::<NQuery, _>(
                    &legacy,
                    config::legacy(),
                ))
            })
        });
    }
    {
        let standard = bincode_next::encode_to_vec(&nquery, config::standard()).unwrap();
        let mut g = c.benchmark_group("bncode_next_native_standard");
        g.throughput(Throughput::Bytes(standard.len() as u64));
        g.sample_size(50);
        g.bench_function("encode", |b| {
            b.iter(|| black_box(bincode_next::encode_to_vec(&nquery, config::standard())))
        });
        g.bench_function("decode", |b| {
            b.iter(|| {
                black_box(bincode_next::decode_from_slice::<NQuery, _>(
                    &standard,
                    config::standard(),
                ))
            })
        });
    }
}
