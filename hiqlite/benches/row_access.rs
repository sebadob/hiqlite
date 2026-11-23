//! Column access costs of the `query_map` / `query_as` mapping paths.
//!
//! Every benchmark maps the same rows of the same table into the same struct; only the way the
//! columns are read differs:
//!
//!   * `by_name` — `row.get("name")`, the name resolution rusqlite does with a linear scan over
//!     `sqlite3_column_name`, one C call per candidate column, per field, per row.
//!   * `by_index` — `row.get(idx)`, rusqlite's bounds-checked direct column read.
//!   * `col_idx` — the map `query::rows::ColIdx` builds once per query, then indexed reads.
//!   * `hoisted` — the same map, but the field names are resolved into indices once per query, so
//!     a row is walked by index: the ceiling of any cheaper per-row name lookup.
//!   * `memo_raw` — a per-query memo keyed by the pointer of the lookup key: the ceiling of an
//!     automatic lookup cache, and not sound for heap-built keys.
//!   * `memo` — the same memo with a key byte check on every pointer hit: the safe version of it.
//!   * `serde` — `serde_rusqlite::from_rows`, what `query_as` uses: the names of a query are
//!     collected once and each row is walked positionally.
//!
//! `w8` and `w24` are the same shape with 8 and 24 columns, so the quadratic growth of the
//! name-based path is visible.  `build` measures the fixed per-query cost of building the map,
//! which is what a statement-level cache would save.
//!
//! Run with `cargo bench --features sqlite,macros` and profile the hot path with
//! `samply record <bench binary> --bench --profile-time 5 w24/by_name`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rusqlite::Connection;
use rusqlite::types::Value;
use rustc_hash::FxHashMap;
use serde::Deserialize;

const ROWS: usize = 2000;

/// (column name, SQLite type) of the narrow table; `opt` at index 6 is nullable.
const C8: [(&str, &str); 8] = [
    ("id", "INTEGER"),
    ("name", "TEXT"),
    ("email", "TEXT"),
    ("tag", "TEXT"),
    ("num", "INTEGER"),
    ("rate", "REAL"),
    ("opt", "INTEGER"),
    ("blob", "BLOB"),
];

/// The wide table; `opt`, `c13` and `c19` are nullable.
const C24: [(&str, &str); 24] = [
    ("id", "INTEGER"),
    ("name", "TEXT"),
    ("email", "TEXT"),
    ("tag", "TEXT"),
    ("num", "INTEGER"),
    ("rate", "REAL"),
    ("opt", "INTEGER"),
    ("blob", "BLOB"),
    ("c9", "INTEGER"),
    ("c10", "TEXT"),
    ("c11", "INTEGER"),
    ("c12", "REAL"),
    ("c13", "INTEGER"),
    ("c14", "TEXT"),
    ("c15", "INTEGER"),
    ("c16", "TEXT"),
    ("c17", "REAL"),
    ("c18", "INTEGER"),
    ("c19", "TEXT"),
    ("c20", "INTEGER"),
    ("c21", "TEXT"),
    ("c22", "INTEGER"),
    ("c23", "REAL"),
    ("c24", "TEXT"),
];

const NULLABLE: [usize; 3] = [6, 12, 18];

/// Field names of `R8` in declaration order, the order a `FromRow` derive knows upfront.
const N8: [&str; 8] = ["id", "name", "email", "tag", "num", "rate", "opt", "blob"];

/// Field names of `R24` in declaration order.
const N24: [&str; 24] = [
    "id", "name", "email", "tag", "num", "rate", "opt", "blob", "c9", "c10", "c11", "c12", "c13",
    "c14", "c15", "c16", "c17", "c18", "c19", "c20", "c21", "c22", "c23", "c24",
];

/// Only a few fields are summed by the mapping functions; the rest must exist to be mapped.
#[allow(dead_code)]
#[derive(Deserialize)]
struct R8 {
    id: i64,
    name: String,
    email: String,
    tag: String,
    num: i64,
    rate: f64,
    opt: Option<i64>,
    blob: Vec<u8>,
}

/// Only a few fields are summed by the mapping functions; the rest must exist to be mapped.
#[allow(dead_code)]
#[derive(Deserialize)]
struct R24 {
    id: i64,
    name: String,
    email: String,
    tag: String,
    num: i64,
    rate: f64,
    opt: Option<i64>,
    blob: Vec<u8>,
    c9: i64,
    c10: String,
    c11: i64,
    c12: f64,
    c13: Option<i64>,
    c14: String,
    c15: i64,
    c16: String,
    c17: f64,
    c18: i64,
    c19: Option<String>,
    c20: i64,
    c21: String,
    c22: i64,
    c23: f64,
    c24: String,
}

fn setup(table: &str, cols: &[(&str, &str)]) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    let ddl: Vec<String> = cols.iter().map(|(c, t)| format!("{c} {t}")).collect();
    conn.execute(&format!("CREATE TABLE {table} ({})", ddl.join(", ")), [])
        .unwrap();

    let names: Vec<&str> = cols.iter().map(|(c, _)| *c).collect();
    let ph: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();
    let mut insert = conn
        .prepare(&format!(
            "INSERT INTO {table} ({}) VALUES ({})",
            names.join(", "),
            ph.join(", ")
        ))
        .unwrap();

    for i in 0..ROWS {
        let row = i64::try_from(i).expect("row index fits into i64");
        let values: Vec<Value> = cols
            .iter()
            .enumerate()
            .map(|(k, (_, t))| {
                let col = i64::try_from(k).expect("column index fits into i64");
                match *t {
                    "INTEGER" if NULLABLE.contains(&k) => Value::Null,
                    "INTEGER" => Value::Integer(row + col),
                    "REAL" => Value::Real(
                        f64::from(i32::try_from(i).expect("row index fits into i32")) * 1.5,
                    ),
                    "BLOB" => Value::Blob(vec![k as u8; 16]),
                    _ => Value::Text(format!("value-{k}-{i}")),
                }
            })
            .collect();
        insert.execute(rusqlite::params_from_iter(values)).unwrap();
    }
    drop(insert);
    conn
}

fn sql(table: &str, cols: &[(&str, &str)]) -> String {
    let names: Vec<&str> = cols.iter().map(|(c, _)| *c).collect();
    format!("SELECT {} FROM {table}", names.join(", "))
}

/// The name -> index map `query::rows::ColIdx` builds once per query.
fn col_idx(conn: &Connection, query: &str) -> FxHashMap<String, u16> {
    let stmt = conn.prepare_cached(query).unwrap();
    stmt.column_names()
        .enumerate()
        .map(|(idx, name)| (name.to_string(), idx as u16))
        .collect()
}

/// Maps every row of a query with the given mapping function.
fn map_all<F>(conn: &Connection, query: &str, map: &mut F) -> usize
where
    F: FnMut(&rusqlite::Row) -> usize,
{
    let mut stmt = conn.prepare_cached(query).unwrap();
    let mut rows = stmt.raw_query();
    let mut acc = 0;
    while let Ok(Some(row)) = rows.next() {
        acc += map(row);
    }
    acc
}

fn group<F>(c: &mut Criterion, name: &str, conn: &Connection, query: &str, mut map: F)
where
    F: FnMut(&rusqlite::Row) -> usize,
{
    let mut g = c.benchmark_group(name);
    g.sample_size(30);
    g.bench_function("rows", |b| {
        b.iter(|| black_box(map_all(conn, query, &mut map)));
    });
    g.finish();
}

/// Maps rows through a per-query `ColIdx`, the path `query_map` takes.  The map is built inside
/// the measured closure, because that is where `query_map` builds it: once per query.
fn group_idx<F>(c: &mut Criterion, name: &str, conn: &Connection, query: &str, mut map: F)
where
    F: FnMut(&rusqlite::Row, &FxHashMap<String, u16>) -> usize,
{
    let mut g = c.benchmark_group(name);
    g.sample_size(30);
    g.bench_function("rows", |b| {
        b.iter(|| {
            let cols = col_idx(conn, query);
            black_box(map_all(conn, query, &mut |row| map(row, &cols)));
        });
    });
    g.finish();
}

/// Maps rows through field indices that were resolved from the map once per query.  This is what a
/// `FromRow`-style path could do, because it knows all its field names before it touches a row: N
/// probes per query instead of N probes per row, and the ceiling for any cheaper name lookup.
fn group_hoisted<F>(
    c: &mut Criterion,
    name: &str,
    conn: &Connection,
    query: &str,
    fields: &[&str],
    map: F,
) where
    F: Fn(&rusqlite::Row, &[u16]) -> usize,
{
    let mut g = c.benchmark_group(name);
    g.sample_size(30);
    g.bench_function("rows", |b| {
        b.iter(|| {
            let cols = col_idx(conn, query);
            let idx: Vec<u16> = fields.iter().map(|name| cols[*name]).collect();
            black_box(map_all(conn, query, &mut |row| map(row, &idx)));
        });
    });
    g.finish();
}

/// Deserializes rows with `serde_rusqlite`, the path `query_as` takes.
fn group_serde_8(c: &mut Criterion, conn: &Connection, query: &str) {
    let mut g = c.benchmark_group("w8/serde");
    g.sample_size(30);
    g.bench_function("rows", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare_cached(query).unwrap();
            let mut it = serde_rusqlite::from_rows::<R8>(stmt.raw_query());
            let mut acc = 0;
            while let Some(Ok(r)) = it.next() {
                acc += r.name.len() + r.email.len() + r.blob.len();
            }
            black_box(acc);
        });
    });
    g.finish();
}

fn group_serde_24(c: &mut Criterion, conn: &Connection, query: &str) {
    let mut g = c.benchmark_group("w24/serde");
    g.sample_size(30);
    g.bench_function("rows", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare_cached(query).unwrap();
            let mut it = serde_rusqlite::from_rows::<R24>(stmt.raw_query());
            let mut acc = 0;
            while let Some(Ok(r)) = it.next() {
                acc += r.name.len() + r.email.len() + r.blob.len();
            }
            black_box(acc);
        });
    });
    g.finish();
}

fn map8_by_name(row: &rusqlite::Row) -> usize {
    let r = R8 {
        id: row.get("id").unwrap(),
        name: row.get("name").unwrap(),
        email: row.get("email").unwrap(),
        tag: row.get("tag").unwrap(),
        num: row.get("num").unwrap(),
        rate: row.get("rate").unwrap(),
        opt: row.get("opt").unwrap(),
        blob: row.get("blob").unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map8_by_index(row: &rusqlite::Row) -> usize {
    let r = R8 {
        id: row.get(0).unwrap(),
        name: row.get(1).unwrap(),
        email: row.get(2).unwrap(),
        tag: row.get(3).unwrap(),
        num: row.get(4).unwrap(),
        rate: row.get(5).unwrap(),
        opt: row.get(6).unwrap(),
        blob: row.get(7).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map24_by_name(row: &rusqlite::Row) -> usize {
    let r = R24 {
        id: row.get("id").unwrap(),
        name: row.get("name").unwrap(),
        email: row.get("email").unwrap(),
        tag: row.get("tag").unwrap(),
        num: row.get("num").unwrap(),
        rate: row.get("rate").unwrap(),
        opt: row.get("opt").unwrap(),
        blob: row.get("blob").unwrap(),
        c9: row.get("c9").unwrap(),
        c10: row.get("c10").unwrap(),
        c11: row.get("c11").unwrap(),
        c12: row.get("c12").unwrap(),
        c13: row.get("c13").unwrap(),
        c14: row.get("c14").unwrap(),
        c15: row.get("c15").unwrap(),
        c16: row.get("c16").unwrap(),
        c17: row.get("c17").unwrap(),
        c18: row.get("c18").unwrap(),
        c19: row.get("c19").unwrap(),
        c20: row.get("c20").unwrap(),
        c21: row.get("c21").unwrap(),
        c22: row.get("c22").unwrap(),
        c23: row.get("c23").unwrap(),
        c24: row.get("c24").unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map24_by_index(row: &rusqlite::Row) -> usize {
    let r = R24 {
        id: row.get(0).unwrap(),
        name: row.get(1).unwrap(),
        email: row.get(2).unwrap(),
        tag: row.get(3).unwrap(),
        num: row.get(4).unwrap(),
        rate: row.get(5).unwrap(),
        opt: row.get(6).unwrap(),
        blob: row.get(7).unwrap(),
        c9: row.get(8).unwrap(),
        c10: row.get(9).unwrap(),
        c11: row.get(10).unwrap(),
        c12: row.get(11).unwrap(),
        c13: row.get(12).unwrap(),
        c14: row.get(13).unwrap(),
        c15: row.get(14).unwrap(),
        c16: row.get(15).unwrap(),
        c17: row.get(16).unwrap(),
        c18: row.get(17).unwrap(),
        c19: row.get(18).unwrap(),
        c20: row.get(19).unwrap(),
        c21: row.get(20).unwrap(),
        c22: row.get(21).unwrap(),
        c23: row.get(22).unwrap(),
        c24: row.get(23).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map8_via_idx(row: &rusqlite::Row, cols: &FxHashMap<String, u16>) -> usize {
    let r = R8 {
        id: row.get(cols["id"] as usize).unwrap(),
        name: row.get(cols["name"] as usize).unwrap(),
        email: row.get(cols["email"] as usize).unwrap(),
        tag: row.get(cols["tag"] as usize).unwrap(),
        num: row.get(cols["num"] as usize).unwrap(),
        rate: row.get(cols["rate"] as usize).unwrap(),
        opt: row.get(cols["opt"] as usize).unwrap(),
        blob: row.get(cols["blob"] as usize).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map24_via_idx(row: &rusqlite::Row, cols: &FxHashMap<String, u16>) -> usize {
    let r = R24 {
        id: row.get(cols["id"] as usize).unwrap(),
        name: row.get(cols["name"] as usize).unwrap(),
        email: row.get(cols["email"] as usize).unwrap(),
        tag: row.get(cols["tag"] as usize).unwrap(),
        num: row.get(cols["num"] as usize).unwrap(),
        rate: row.get(cols["rate"] as usize).unwrap(),
        opt: row.get(cols["opt"] as usize).unwrap(),
        blob: row.get(cols["blob"] as usize).unwrap(),
        c9: row.get(cols["c9"] as usize).unwrap(),
        c10: row.get(cols["c10"] as usize).unwrap(),
        c11: row.get(cols["c11"] as usize).unwrap(),
        c12: row.get(cols["c12"] as usize).unwrap(),
        c13: row.get(cols["c13"] as usize).unwrap(),
        c14: row.get(cols["c14"] as usize).unwrap(),
        c15: row.get(cols["c15"] as usize).unwrap(),
        c16: row.get(cols["c16"] as usize).unwrap(),
        c17: row.get(cols["c17"] as usize).unwrap(),
        c18: row.get(cols["c18"] as usize).unwrap(),
        c19: row.get(cols["c19"] as usize).unwrap(),
        c20: row.get(cols["c20"] as usize).unwrap(),
        c21: row.get(cols["c21"] as usize).unwrap(),
        c22: row.get(cols["c22"] as usize).unwrap(),
        c23: row.get(cols["c23"] as usize).unwrap(),
        c24: row.get(cols["c24"] as usize).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

/// The ceiling of an automatic "cache the last lookups" memo inside a per-query index: the key of
/// every derived lookup is a `&'static str` literal, so its pointer repeats on every row and a
/// pointer -> index slot answers the lookup without hashing.  This is not sound as a design: a
/// heap `String` buffer address can be freed and reused by a different string, so a real memo has
/// to verify the key bytes, which is what `memo` measures.
struct MemoRaw {
    map: FxHashMap<String, u16>,
    slots: Box<[(usize, u16)]>,
}

/// A memo that verifies the key bytes of a pointer hit, the price of doing the same trick safely.
struct MemoChecked {
    map: FxHashMap<String, u16>,
    slots: Box<[(usize, u16, Box<str>)]>,
}

impl MemoRaw {
    fn new(map: FxHashMap<String, u16>) -> Self {
        Self {
            map,
            slots: vec![(0, u16::MAX); 64].into_boxed_slice(),
        }
    }

    fn get(&mut self, name: &str) -> usize {
        let ptr = name.as_ptr() as usize;
        let slot = (ptr >> 5) & 63;
        let (p, idx) = self.slots[slot];
        if p == ptr {
            return idx as usize;
        }
        let idx = self.map[name];
        self.slots[slot] = (ptr, idx);
        idx as usize
    }
}

impl MemoChecked {
    fn new(map: FxHashMap<String, u16>) -> Self {
        Self {
            map,
            slots: vec![(0, u16::MAX, Box::from("")); 64].into_boxed_slice(),
        }
    }

    fn get(&mut self, name: &str) -> usize {
        let ptr = name.as_ptr() as usize;
        let slot = (ptr >> 5) & 63;
        {
            let (p, idx, key) = &self.slots[slot];
            if *p == ptr && &**key == name {
                return *idx as usize;
            }
        }
        let idx = self.map[name];
        self.slots[slot] = (ptr, idx, Box::from(name));
        idx as usize
    }
}

fn group_memo<F>(c: &mut Criterion, name: &str, conn: &Connection, query: &str, mut map: F)
where
    F: FnMut(&rusqlite::Row, &mut MemoRaw) -> usize,
{
    let mut g = c.benchmark_group(name);
    g.sample_size(30);
    g.bench_function("rows", |b| {
        b.iter(|| {
            let mut memo = MemoRaw::new(col_idx(conn, query));
            black_box(map_all(conn, query, &mut |row| map(row, &mut memo)));
        });
    });
    g.finish();
}

fn group_memo_checked<F>(c: &mut Criterion, name: &str, conn: &Connection, query: &str, mut map: F)
where
    F: FnMut(&rusqlite::Row, &mut MemoChecked) -> usize,
{
    let mut g = c.benchmark_group(name);
    g.sample_size(30);
    g.bench_function("rows", |b| {
        b.iter(|| {
            let mut memo = MemoChecked::new(col_idx(conn, query));
            black_box(map_all(conn, query, &mut |row| map(row, &mut memo)));
        });
    });
    g.finish();
}

fn map8_memo(row: &rusqlite::Row, m: &mut MemoRaw) -> usize {
    let r = R8 {
        id: row.get(m.get("id")).unwrap(),
        name: row.get(m.get("name")).unwrap(),
        email: row.get(m.get("email")).unwrap(),
        tag: row.get(m.get("tag")).unwrap(),
        num: row.get(m.get("num")).unwrap(),
        rate: row.get(m.get("rate")).unwrap(),
        opt: row.get(m.get("opt")).unwrap(),
        blob: row.get(m.get("blob")).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map24_memo(row: &rusqlite::Row, m: &mut MemoRaw) -> usize {
    let r = R24 {
        id: row.get(m.get("id")).unwrap(),
        name: row.get(m.get("name")).unwrap(),
        email: row.get(m.get("email")).unwrap(),
        tag: row.get(m.get("tag")).unwrap(),
        num: row.get(m.get("num")).unwrap(),
        rate: row.get(m.get("rate")).unwrap(),
        opt: row.get(m.get("opt")).unwrap(),
        blob: row.get(m.get("blob")).unwrap(),
        c9: row.get(m.get("c9")).unwrap(),
        c10: row.get(m.get("c10")).unwrap(),
        c11: row.get(m.get("c11")).unwrap(),
        c12: row.get(m.get("c12")).unwrap(),
        c13: row.get(m.get("c13")).unwrap(),
        c14: row.get(m.get("c14")).unwrap(),
        c15: row.get(m.get("c15")).unwrap(),
        c16: row.get(m.get("c16")).unwrap(),
        c17: row.get(m.get("c17")).unwrap(),
        c18: row.get(m.get("c18")).unwrap(),
        c19: row.get(m.get("c19")).unwrap(),
        c20: row.get(m.get("c20")).unwrap(),
        c21: row.get(m.get("c21")).unwrap(),
        c22: row.get(m.get("c22")).unwrap(),
        c23: row.get(m.get("c23")).unwrap(),
        c24: row.get(m.get("c24")).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map8_memo_checked(row: &rusqlite::Row, m: &mut MemoChecked) -> usize {
    let r = R8 {
        id: row.get(m.get("id")).unwrap(),
        name: row.get(m.get("name")).unwrap(),
        email: row.get(m.get("email")).unwrap(),
        tag: row.get(m.get("tag")).unwrap(),
        num: row.get(m.get("num")).unwrap(),
        rate: row.get(m.get("rate")).unwrap(),
        opt: row.get(m.get("opt")).unwrap(),
        blob: row.get(m.get("blob")).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map24_memo_checked(row: &rusqlite::Row, m: &mut MemoChecked) -> usize {
    let r = R24 {
        id: row.get(m.get("id")).unwrap(),
        name: row.get(m.get("name")).unwrap(),
        email: row.get(m.get("email")).unwrap(),
        tag: row.get(m.get("tag")).unwrap(),
        num: row.get(m.get("num")).unwrap(),
        rate: row.get(m.get("rate")).unwrap(),
        opt: row.get(m.get("opt")).unwrap(),
        blob: row.get(m.get("blob")).unwrap(),
        c9: row.get(m.get("c9")).unwrap(),
        c10: row.get(m.get("c10")).unwrap(),
        c11: row.get(m.get("c11")).unwrap(),
        c12: row.get(m.get("c12")).unwrap(),
        c13: row.get(m.get("c13")).unwrap(),
        c14: row.get(m.get("c14")).unwrap(),
        c15: row.get(m.get("c15")).unwrap(),
        c16: row.get(m.get("c16")).unwrap(),
        c17: row.get(m.get("c17")).unwrap(),
        c18: row.get(m.get("c18")).unwrap(),
        c19: row.get(m.get("c19")).unwrap(),
        c20: row.get(m.get("c20")).unwrap(),
        c21: row.get(m.get("c21")).unwrap(),
        c22: row.get(m.get("c22")).unwrap(),
        c23: row.get(m.get("c23")).unwrap(),
        c24: row.get(m.get("c24")).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map8_hoisted(row: &rusqlite::Row, idx: &[u16]) -> usize {
    let r = R8 {
        id: row.get(idx[0] as usize).unwrap(),
        name: row.get(idx[1] as usize).unwrap(),
        email: row.get(idx[2] as usize).unwrap(),
        tag: row.get(idx[3] as usize).unwrap(),
        num: row.get(idx[4] as usize).unwrap(),
        rate: row.get(idx[5] as usize).unwrap(),
        opt: row.get(idx[6] as usize).unwrap(),
        blob: row.get(idx[7] as usize).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn map24_hoisted(row: &rusqlite::Row, idx: &[u16]) -> usize {
    let r = R24 {
        id: row.get(idx[0] as usize).unwrap(),
        name: row.get(idx[1] as usize).unwrap(),
        email: row.get(idx[2] as usize).unwrap(),
        tag: row.get(idx[3] as usize).unwrap(),
        num: row.get(idx[4] as usize).unwrap(),
        rate: row.get(idx[5] as usize).unwrap(),
        opt: row.get(idx[6] as usize).unwrap(),
        blob: row.get(idx[7] as usize).unwrap(),
        c9: row.get(idx[8] as usize).unwrap(),
        c10: row.get(idx[9] as usize).unwrap(),
        c11: row.get(idx[10] as usize).unwrap(),
        c12: row.get(idx[11] as usize).unwrap(),
        c13: row.get(idx[12] as usize).unwrap(),
        c14: row.get(idx[13] as usize).unwrap(),
        c15: row.get(idx[14] as usize).unwrap(),
        c16: row.get(idx[15] as usize).unwrap(),
        c17: row.get(idx[16] as usize).unwrap(),
        c18: row.get(idx[17] as usize).unwrap(),
        c19: row.get(idx[18] as usize).unwrap(),
        c20: row.get(idx[19] as usize).unwrap(),
        c21: row.get(idx[20] as usize).unwrap(),
        c22: row.get(idx[21] as usize).unwrap(),
        c23: row.get(idx[22] as usize).unwrap(),
        c24: row.get(idx[23] as usize).unwrap(),
    };
    r.name.len() + r.email.len() + r.blob.len()
}

fn bench(c: &mut Criterion) {
    let conn8 = setup("t8", &C8);
    let q8 = sql("t8", &C8);
    group(c, "w8/by_name", &conn8, &q8, map8_by_name);
    group(c, "w8/by_index", &conn8, &q8, map8_by_index);
    group_idx(c, "w8/col_idx", &conn8, &q8, map8_via_idx);
    group_hoisted(c, "w8/hoisted", &conn8, &q8, &N8, map8_hoisted);
    group_memo(c, "w8/memo_raw", &conn8, &q8, map8_memo);
    group_memo_checked(c, "w8/memo", &conn8, &q8, map8_memo_checked);
    group_serde_8(c, &conn8, &q8);

    let conn24 = setup("t24", &C24);
    let q24 = sql("t24", &C24);
    group(c, "w24/by_name", &conn24, &q24, map24_by_name);
    group(c, "w24/by_index", &conn24, &q24, map24_by_index);
    group_idx(c, "w24/col_idx", &conn24, &q24, map24_via_idx);
    group_hoisted(c, "w24/hoisted", &conn24, &q24, &N24, map24_hoisted);
    group_memo(c, "w24/memo_raw", &conn24, &q24, map24_memo);
    group_memo_checked(c, "w24/memo", &conn24, &q24, map24_memo_checked);
    group_serde_24(c, &conn24, &q24);

    // The fixed per-query cost of building the map, which is all a statement cache would save.
    for (label, conn, query) in [("8cols", &conn8, &q8), ("24cols", &conn24, &q24)] {
        let mut g = c.benchmark_group(format!("build/{label}"));
        g.bench_function("col_idx", |b| {
            b.iter(|| {
                black_box(col_idx(conn, query).len());
            });
        });
        g.finish();
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
