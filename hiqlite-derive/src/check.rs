// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use quote::quote;
use rusqlite::Batch;
use rusqlite::Connection;
use rusqlite::Statement;
use rusqlite::fallible_iterator::FallibleIterator;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;

/// The tables every Hiqlite database creates itself, before any app migration runs.
///
/// Keep in sync with the writer in the `hiqlite` crate: `_metadata` is created on connection
/// setup and `_migrations` on the first migration run.
const INTERNAL_DDL: &str = "
CREATE TABLE IF NOT EXISTS _metadata
(
    key  TEXT    NOT NULL
        CONSTRAINT _metadata_pk
            PRIMARY KEY,
    data BLOB    NOT NULL
);
CREATE TABLE IF NOT EXISTS _migrations
(
    id   INTEGER    NOT NULL
        CONSTRAINT _migrations_pk
            PRIMARY KEY,
    name TEXT       NOT NULL,
    ts   INTEGER    NOT NULL,
    hash TEXT       NOT NULL
);
";

/// Layout version of the cache records; bump it when the record format changes.
const CACHE_VERSION: &str = "v3";

/// The verdict of a query and the columns of its `SELECT`, keyed like the cache records.
type MemoTable = Mutex<HashMap<String, (Result<(), String>, Vec<String>)>>;

/// Results of this compiler process, so a repeated query never touches the disk twice.
static MEMO: LazyLock<MemoTable> = LazyLock::new(|| MemoTable::new(HashMap::new()));

/// The in-memory databases of this compiler process, one per schema fingerprint.
static DATABASES: LazyLock<Mutex<HashMap<String, Connection>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Warnings about the checker itself, each printed once per compiler process.
static WARNINGS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// The schema fingerprints whose files rustc already watches.
static WATCHED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// The directory of the crate being compiled, the one cargo points `CARGO_MANIFEST_DIR` at.
fn manifest_dir() -> PathBuf {
    std::env::var("CARGO_MANIFEST_DIR").map_or_else(|_| PathBuf::from("."), PathBuf::from)
}

/// Where the directory of migration files came from.
#[derive(Clone, Copy)]
pub(crate) enum DirSource {
    /// The `migrations` directory next to the crate's `Cargo.toml`, the convention.
    Default,
    /// The `HQL_MIGRATIONS` directory the crate configured in `.cargo/config.toml`.
    Configured,
}

/// The env var cargo passes to the compiler for a `migrations` directory that is not the
/// convention next to the `Cargo.toml`; the value is relative to that `Cargo.toml` unless absolute.
const MIGRATIONS_VAR: &str = "HQL_MIGRATIONS";

/// The migration set of the crate being compiled, loaded exactly once per compiler process.
#[derive(Debug)]
pub(crate) enum Schema {
    /// The migrations apply cleanly to an in-memory database, which is the schema queries check.
    Ready {
        /// sha256 over the migration set and the SQLite version, the key of caches and databases.
        fingerprint: String,
        /// The migration files in apply order, watched so cargo rebuilds on any change.
        files: Vec<PathBuf>,
        /// The internal DDL plus all migration statements, in apply order.
        sql: String,
    },

    /// Nothing can be checked; the message says what is missing, and it names `HQL_MIGRATIONS`.
    Absent { msg: &'static str },

    /// The `migrations` directory cannot be used; a warning, never a build error.
    Broken { msg: String },
}

impl Schema {
    /// The migration set of the crate currently being compiled.
    pub(crate) fn current() -> &'static Self {
        static SCHEMA: LazyLock<Schema> =
            LazyLock::new(|| Schema::load_from(&Schema::dir().0, Schema::dir().1));
        &SCHEMA
    }

    /// Read every migration of `dir`, build the schema once, and report the fingerprint.
    ///
    /// `source` decides how a directory with nothing to check is reported: the convention is a
    /// quiet "not checked", a configured directory is a warning naming `HQL_MIGRATIONS`, because
    /// there the files were meant to be there.
    fn load_from(dir: &Path, source: DirSource) -> Self {
        let empty_msg = match source {
            DirSource::Default => {
                "no `migrations` directory next to the Cargo.toml of this crate - queries are \
                 not checked; set HQL_MIGRATIONS in `.cargo/config.toml` to point at one"
            }
            DirSource::Configured => {
                "the HQL_MIGRATIONS directory has no `.sql` files to check against"
            }
        };
        let Ok(entries) = fs::read_dir(dir) else {
            return Self::Absent { msg: empty_msg };
        };

        let mut migrations: Vec<(u32, PathBuf, Vec<u8>)> = Vec::with_capacity(8);
        for entry in entries {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("sql") {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some((id, _)) = file_name.split_once('_') else {
                return Self::broken(format!(
                    "migration file `{file_name}` does not match the format \
                     `<integer>_<migration_name>.sql`"
                ));
            };
            let Ok(id) = id.parse::<u32>() else {
                return Self::broken(format!(
                    "migration file `{file_name}` does not start with an integer index"
                ));
            };
            match fs::read(&path) {
                Ok(content) => migrations.push((id, path, content)),
                Err(err) => {
                    return Self::broken(format!("cannot read migration `{file_name}`: {err}"));
                }
            }
        }
        if migrations.is_empty() {
            return Self::Absent { msg: empty_msg };
        }
        migrations.sort_by_key(|(id, _, _)| *id);
        if let Some(msg) = Schema::check_ids(&migrations) {
            return Self::broken(msg);
        }

        let fingerprint = Schema::fingerprint(&migrations);
        let files = migrations
            .iter()
            .map(|(_, path, _)| path.clone())
            .collect::<Vec<_>>();

        let mut sql = String::from(INTERNAL_DDL);
        for (id, _, content) in &migrations {
            let Ok(text) = std::str::from_utf8(content) else {
                return Self::broken(format!("migration `{id}` is not valid UTF-8"));
            };
            sql.push_str(text);
            // A file may end without a semicolon; SQLite ignores the empty statement that adds.
            sql.push_str(";\n");
        }

        match Schema::build(&sql) {
            Ok(conn) => {
                drop(conn);
                Self::Ready {
                    fingerprint,
                    files,
                    sql,
                }
            }
            Err(msg) => Self::broken(msg),
        }
    }

    fn broken(msg: String) -> Self {
        Self::Broken { msg }
    }

    /// The ids a migration set must form: 1, 2, 3 …, exactly like the writer validates it.  A
    /// duplicate id would make the apply order, and with it the fingerprint, depend on the order
    /// `read_dir` happens to return.
    fn check_ids(migrations: &[(u32, PathBuf, Vec<u8>)]) -> Option<String> {
        let duplicates = migrations.windows(2).any(|pair| pair[0].0 == pair[1].0);
        for (expected, (id, path, _)) in (1_u32..).zip(migrations.iter()) {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if *id != expected {
                let msg = if duplicates {
                    format!("migration id {id} is used by more than one file in `migrations`")
                } else {
                    format!(
                        "`migrations` must start at id 1 and have no gaps, `{name}` has id {id} \
                         where {expected} is expected"
                    )
                };
                return Some(msg);
            }
        }
        None
    }

    /// The migration directory of the crate being compiled, and where it came from.
    fn dir() -> (PathBuf, DirSource) {
        let manifest = manifest_dir();
        let configured = std::env::var(MIGRATIONS_VAR)
            .ok()
            .and_then(|value| Self::resolve_dir(&value, &manifest));
        match configured {
            Some(dir) => (dir, DirSource::Configured),
            None => (manifest.join("migrations"), DirSource::Default),
        }
    }

    /// The value of [`MIGRATIONS_VAR`] as a directory: relative to `manifest`, absolute as it is,
    /// and `None` when it names nothing.
    fn resolve_dir(value: &str, manifest: &Path) -> Option<PathBuf> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        let dir = PathBuf::from(value);
        Some(if dir.is_absolute() {
            dir
        } else {
            manifest.join(value)
        })
    }

    /// sha256 over everything a verdict depends on: the migration files and the SQLite version.
    /// Any change here invalidates every cached result of that schema.
    fn fingerprint(migrations: &[(u32, PathBuf, Vec<u8>)]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(CACHE_VERSION.as_bytes());
        hasher.update(rusqlite::version().as_bytes());
        for (id, path, content) in migrations {
            hasher.update(id.to_string().as_bytes());
            hasher.update(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .as_bytes(),
            );
            hasher.update(content);
        }
        hex::encode(hasher.finalize())
    }

    /// A fresh in-memory database with the full schema applied.
    fn build(sql: &str) -> Result<Connection, String> {
        let conn = Connection::open_in_memory().map_err(|err| format!("check db open: {err}"))?;
        conn.execute_batch(sql).map_err(|err| {
            format!("the migrations do not apply to an in-memory database: {err}")
        })?;
        Ok(conn)
    }

    /// The migration files rustc must watch, as tokens.  `Watch` decides which of them a
    /// crate still needs.
    fn watch_tokens(&self) -> Vec<proc_macro2::TokenStream> {
        let Self::Ready { files, .. } = self else {
            return Vec::new();
        };
        files
            .iter()
            .map(|path| {
                let path =
                    syn::LitStr::new(&path.to_string_lossy(), proc_macro2::Span::call_site());
                quote! {
                    const _: &str = ::core::include_str!(#path);
                }
            })
            .collect()
    }
}

/// Where a check came out the way it did.
pub(crate) enum Verdict {
    /// SQLite prepared everything, with the result columns of the first statement.
    Passed { columns: Vec<String> },
    /// SQLite refused the query; the message is its own.
    Rejected(String),
    /// The checker cannot work: reported once as a warning, the query stays unchecked, and nothing
    /// about it is cached.
    Checker(String),
}

/// The in-memory database of this compiler process, opened once per schema fingerprint.
///
/// A database enters `DATABASES` by one `insert` of a finished connection and is never altered
/// afterwards, so the lock is taken through `into_inner()` after a poison: a schema that built
/// before a panic stays usable for the queries after it.
pub(crate) struct SchemaDb;

impl SchemaDb {
    /// Run `f` against the prepared database of `schema`, keeping its failure apart from a failure
    /// of the database itself.
    pub(crate) fn with(schema: &Schema, f: impl FnOnce(&Connection) -> Verdict) -> Verdict {
        let Schema::Ready {
            fingerprint, sql, ..
        } = schema
        else {
            return Verdict::Checker("the schema of this crate is not usable".to_string());
        };

        let mut dbs = DATABASES.lock().unwrap_or_else(|err| err.into_inner());

        if !dbs.contains_key(fingerprint) {
            match Schema::build(sql) {
                Ok(conn) => {
                    dbs.insert(fingerprint.clone(), conn);
                }
                Err(msg) => return Verdict::Checker(msg),
            }
        }

        f(&dbs[fingerprint])
    }
}

/// The migration files rustc has to see, emitted once per crate.
///
/// An `include_str!` of a file registers that file as a dependency of the crate being compiled,
/// which is what makes cargo rebuild when a migration changes. That is a property of the crate, not
/// of the call site that asked for it, so the first `sql!()` of a schema emits one per migration
/// file and every later one emits none: a crate with a thousand queries embeds each migration once.
///
/// `WATCHED` only grows by one `insert` of a finished fingerprint and the lock is taken through
/// `into_inner()` after a poison, where the worst a poison can cost is the same set of migrations
/// watched twice.
pub(crate) struct Watch;

impl Watch {
    /// The `include_str!` tokens this crate still needs for `schema`, none once it has them.
    pub(crate) fn tokens(schema: &Schema) -> Vec<proc_macro2::TokenStream> {
        let Schema::Ready { fingerprint, .. } = schema else {
            return Vec::new();
        };
        let fresh = WATCHED
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(fingerprint.clone());
        if fresh {
            schema.watch_tokens()
        } else {
            Vec::new()
        }
    }
}

/// Prepares statements exactly like the writer does: every statement of a string, in order.
pub(crate) struct Prepare;

impl Prepare {
    /// Prepare everything in `sql`. `single` rejects a string with more than one statement.
    ///
    /// A single statement is described as prepared: SQLite expands a `SELECT *` against the schema
    /// at prepare time, so its result columns are known without running anything.
    pub(crate) fn run(conn: &Connection, sql: &str, single: bool) -> Verdict {
        let mut batch = Batch::new(conn, sql);
        let mut count = 0_usize;
        let mut columns = Vec::new();

        loop {
            match batch.next() {
                Ok(Some(stmt)) => {
                    if single && count == 0 {
                        columns = Prepare::columns_of(&stmt);
                    }
                    drop(stmt);
                    count += 1;
                }
                // A failure of the batch *is* the answer: the statement SQLite refuses.
                Ok(None) => break,
                Err(err) => return Verdict::Rejected(err.to_string()),
            }
        }

        if count == 0 {
            return Verdict::Rejected("the query string has no statement".to_string());
        }
        if single && count > 1 {
            return Verdict::Rejected(format!(
                "`sql!` takes exactly one statement, this has {count}; use `sql_batch!` for a \
                 batch"
            ));
        }
        Verdict::Passed { columns }
    }

    /// The result columns of a prepared statement, exactly the names a mapped row will read.
    fn columns_of(stmt: &Statement<'_>) -> Vec<String> {
        (0..stmt.column_count())
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect()
    }
}

/// The shared on-disk cache: one record per query string, per schema fingerprint.
pub(crate) struct Cache;

impl Cache {
    /// `<workspace root>/.hiqlite-sql`, the manifest directory when there is no workspace.
    pub(crate) fn root() -> &'static Path {
        static ROOT: LazyLock<PathBuf> = LazyLock::new(Cache::find_root);
        &ROOT
    }

    fn find_root() -> PathBuf {
        let manifest = manifest_dir();
        let mut root = manifest.clone();
        for ancestor in manifest.ancestors().skip(1) {
            if Cache::is_workspace_root(ancestor) {
                root = ancestor.to_path_buf();
                break;
            }
        }
        root.join(".hiqlite-sql")
    }

    fn is_workspace_root(dir: &Path) -> bool {
        let Ok(text) = fs::read_to_string(dir.join("Cargo.toml")) else {
            return false;
        };
        text.lines()
            .any(|line| line.trim() == "[workspace]" || line.trim().starts_with("[workspace."))
    }

    /// The cached verdict and columns of `check`, `None` when it was never checked against this
    /// schema, and `None` for a record that was not written to its end: a half record is no
    /// verdict, and refilling it is what a missing record does.
    pub(crate) fn lookup(
        root: &Path,
        fingerprint: &str,
        check: &Check,
    ) -> Option<(Result<(), String>, Vec<String>)> {
        let path = Cache::path(root, fingerprint, check);
        let Ok(record) = fs::read_to_string(path) else {
            return None;
        };
        let body = record.strip_suffix(Cache::RECORD_END)?;
        let mut lines = body.splitn(3, '\n');
        let hash = lines.next().unwrap_or_default();
        if hash != Cache::hash(check.sql) {
            // The file name is a hash; a record naming a different hash is a collision.
            return None;
        }
        let msg = Cache::unescape(lines.next().unwrap_or_default());
        let columns = lines
            .next()
            .unwrap_or_default()
            .split('\t')
            .filter(|name| !name.is_empty())
            .map(Cache::unescape)
            .collect();
        let res = if msg.is_empty() { Ok(()) } else { Err(msg) };
        Some((res, columns))
    }

    /// The line a record ends with. A write that stopped early leaves a record without it, and a
    /// record is only ever read as a verdict when its last line is this one.
    const RECORD_END: &'static str = "\n.\n";

    /// One field of a record on one line: the separators that would split a verdict line are
    /// escaped, so a SQLite message over several lines and a column name containing a tab come
    /// back through [`Cache::lookup`] exactly as they went in.
    fn escape(field: &str) -> String {
        let mut out = String::with_capacity(field.len());
        for c in field.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                c => out.push(c),
            }
        }
        out
    }

    /// The inverse of [`Cache::escape`]; an unfinished escape sequence at the end is kept as text.
    fn unescape(field: &str) -> String {
        let mut out = String::with_capacity(field.len());
        let mut chars = field.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') | None => out.push('\\'),
                Some(other) => out.push(other),
            }
        }
        out
    }

    /// Store the verdict. File creation is the lock between parallel compiler processes: the
    /// first one to create the record wins, every later one is a no-op.
    pub(crate) fn store(
        root: &Path,
        fingerprint: &str,
        check: &Check,
        msg: Option<&str>,
        columns: &[String],
    ) {
        let path = Cache::path(root, fingerprint, check);
        let Some(dir) = path.parent() else { return };
        if fs::create_dir_all(dir).is_err() {
            Warn::once("the query cache is not writable, results are not cached");
            return;
        }

        let record = format!(
            "{}\n{}\n{}{}",
            Cache::hash(check.sql),
            Cache::escape(msg.unwrap_or("")),
            columns
                .iter()
                .map(|name| Cache::escape(name))
                .collect::<Vec<String>>()
                .join("\t"),
            Cache::RECORD_END,
        );
        // Written to a private name and renamed into place, so a process that dies mid-write
        // leaves its own temp file and never a half-written verdict under the real name.
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        let Ok(mut file) = OpenOptions::new().write(true).create_new(true).open(&tmp) else {
            return;
        };
        if file.write_all(record.as_bytes()).is_err() || fs::rename(&tmp, &path).is_err() {
            Warn::once("the query cache is not writable, results are not cached");
        }
    }

    fn path(root: &Path, fingerprint: &str, check: &Check) -> PathBuf {
        root.join(CACHE_VERSION)
            .join(fingerprint)
            .join(Cache::key(fingerprint, check))
    }

    /// The record name of a check: a column check and a plain check of the same string are two
    /// different questions, and so are a single statement and a batch of the same string -
    /// `sql!()` and `sql_batch!()` must never read each other's verdict, so every part of what a
    /// call site asks for is part of the key.
    fn key(fingerprint: &str, check: &Check) -> String {
        let mut hasher = Sha256::new();
        hasher.update(fingerprint.as_bytes());
        hasher.update([u8::from(check.single), u8::from(check.columns), 0_u8]);
        hasher.update(check.sql.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// The identity of a query inside a record, kept separate from the message it stores.
    fn hash(sql: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(sql.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// The in-process mirror of the cache, keyed like the records.
///
/// A verdict is written by one `insert` of a finished pair and read back whole, so a poison on this
/// lock cannot carry a half-written entry: both locks are taken through `into_inner()`, which lets
/// a compiler process that panicked elsewhere go on checking the queries it compiles next.
pub(crate) struct Memo;

impl Memo {
    /// The verdict and the columns of `check`, when this compiler process already knows them.
    pub(crate) fn get(
        fingerprint: &str,
        check: &Check,
    ) -> Option<(Result<(), String>, Vec<String>)> {
        let key = Cache::key(fingerprint, check);
        MEMO.lock()
            .unwrap_or_else(|err| err.into_inner())
            .get(&key)
            .cloned()
    }

    pub(crate) fn put(
        fingerprint: &str,
        check: &Check,
        res: &Result<(), String>,
        columns: &[String],
    ) {
        let key = Cache::key(fingerprint, check);
        MEMO.lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(key, (res.clone(), columns.to_vec()));
    }
}

/// Warnings about the checker itself, printed once per message and compiler process.
///
/// The set only grows by one `insert` of a finished message, so a poison carries nothing
/// half-written and the lock is taken through `into_inner()`; the worst it can do is print a
/// warning twice.
pub(crate) struct Warn;

impl Warn {
    pub(crate) fn once(msg: &str) {
        let msg = msg.to_string();
        let fresh = WARNINGS
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(msg.clone());
        if fresh {
            eprintln!("warning: hiqlite compile-time sql check: {msg}");
        }
    }
}

/// The compile-time entry point: prepare a query against the crate's schema, change nothing.
/// What one macro call site asks the checker about.
pub(crate) struct Check<'a> {
    /// The query string, exactly as it is written at the call site.
    pub(crate) sql: &'a str,
    /// `true` for a macro that takes exactly one statement.
    pub(crate) single: bool,
    /// `true` when the call site wants the result columns of the statement.
    pub(crate) columns: bool,
}

/// What a check produced.
pub(crate) struct Checked {
    /// The query was checked against a schema, which is what makes a crate watch its migration
    /// files.
    pub(crate) checked: bool,
    /// The result columns of the statement, when the call site asked for them.
    pub(crate) columns: Vec<String>,
}

pub(crate) struct QueryCheck;

impl QueryCheck {
    /// Whether this compiler process runs the checks at all: debug builds do, a release build only
    /// when the crate opted into `sql-check-release`. Release builds are untouched by the check,
    /// including the code it emits.
    pub(crate) fn is_active() -> bool {
        cfg!(debug_assertions) || Self::is_forced()
    }

    /// Whether the crate opted into checking release builds, which also decides whether the shape
    /// comparison a `sql_map!` emits is gated behind `cfg!(debug_assertions)`.
    pub(crate) fn is_forced() -> bool {
        cfg!(feature = "sql-check-release")
    }

    /// Check `check` against the schema built from the crate's `migrations` directory.
    ///
    /// A query is checked when SQLite prepared it; `Err` carries SQLite's own message about that
    /// query. `Ok` with `checked: false` means nothing could be checked: a release build, a missing
    /// `migrations` directory, unreadable migration files, and unwritable cache paths are reported
    /// once as warnings and leave the query unchecked. The check never breaks a build for a reason
    /// inside itself.
    pub(crate) fn validate(check: &Check) -> Result<Checked, String> {
        let unchecked = || Checked {
            checked: false,
            columns: Vec::new(),
        };
        if !Self::is_active() {
            return Ok(unchecked());
        }

        let schema = Schema::current();
        let fingerprint = match schema {
            Schema::Absent { msg } => {
                Warn::once(msg);
                return Ok(unchecked());
            }
            Schema::Broken { msg } => {
                Warn::once(msg);
                return Ok(unchecked());
            }
            Schema::Ready { fingerprint, .. } => fingerprint,
        };

        if let Some((res, columns)) = Memo::get(fingerprint, check) {
            return res.map(|()| Checked {
                checked: true,
                columns,
            });
        }
        if let Some((res, columns)) = Cache::lookup(Cache::root(), fingerprint, check) {
            Memo::put(fingerprint, check, &res, &columns);
            return res.map(|()| Checked {
                checked: true,
                columns,
            });
        }

        let (res, columns) =
            match SchemaDb::with(schema, |conn| Prepare::run(conn, check.sql, check.single)) {
                Verdict::Passed { columns } => (Ok(()), columns),
                Verdict::Rejected(msg) => (Err(msg), Vec::new()),
                Verdict::Checker(msg) => {
                    // A broken check is never a broken build, and it is never cached as a verdict.
                    Warn::once(&msg);
                    return Ok(unchecked());
                }
            };

        Cache::store(
            Cache::root(),
            fingerprint,
            check,
            res.as_ref().err().map(String::as_str),
            &columns,
        );
        Memo::put(fingerprint, check, &res, &columns);
        res.map(|()| Checked {
            checked: true,
            columns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    /// A fixture directory is a configured one: its files were meant to be there.
    fn load(name: &str) -> Schema {
        Schema::load_from(&fixtures(name), DirSource::Configured)
    }

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("hiqlite-sql-check-{}-{name}", std::process::id()))
    }

    fn ready() -> Schema {
        match load("migrations") {
            schema @ Schema::Ready { .. } => schema,
            other => panic!("the fixture migrations must build a schema, got {other:?}"),
        }
    }

    /// The check a plain `sql!()` call site asks for.
    fn plain(sql: &str) -> Check<'_> {
        Check {
            sql,
            single: true,
            columns: false,
        }
    }

    /// The check a `sql_map!()` call site asks for.
    fn shape(sql: &str) -> Check<'_> {
        Check {
            sql,
            single: true,
            columns: true,
        }
    }

    /// The check a `sql_batch!()` call site asks for.
    fn batch(sql: &str) -> Check<'_> {
        Check {
            sql,
            single: false,
            columns: false,
        }
    }

    /// The check of a query, with a checker failure made loud: in a test it is a bug.
    fn check(schema: &Schema, sql: &str, single: bool) -> Result<Vec<String>, String> {
        match SchemaDb::with(schema, |conn| Prepare::run(conn, sql, single)) {
            Verdict::Passed { columns } => Ok(columns),
            Verdict::Rejected(msg) => Err(msg),
            Verdict::Checker(msg) => panic!("the checker itself must work here: {msg}"),
        }
    }

    #[test]
    fn the_derive_crate_itself_has_no_migrations_dir() {
        // `sql!` in this crate has no schema to check against; that must not be an error.
        assert!(matches!(Schema::current(), Schema::Absent { .. }));
    }

    #[test]
    fn the_watched_files_are_emitted_once_per_schema() {
        let schema = ready();
        let first = Watch::tokens(&schema);
        assert_eq!(first.len(), 2, "the fixture has two migration files");
        assert!(
            Watch::tokens(&schema).is_empty(),
            "a crate watches its migrations once, not once per query"
        );
        assert!(
            Watch::tokens(&Schema::Absent { msg: "" }).is_empty(),
            "a crate without migrations watches nothing"
        );
    }

    #[test]
    fn the_absent_warning_names_the_migrations_env_var() {
        let schema = Schema::load_from(&fixtures("empty"), DirSource::Default);
        match schema {
            Schema::Absent { msg } => {
                assert!(msg.contains(MIGRATIONS_VAR), "{msg}");
                assert!(msg.contains("not checked"), "{msg}");
            }
            other => panic!("an empty directory must leave nothing to check, got {other:?}"),
        }
    }

    #[test]
    fn a_configured_directory_without_files_is_named() {
        let schema = load("empty");
        match schema {
            Schema::Absent { msg } => assert!(msg.contains(".sql"), "{msg}"),
            other => panic!("an empty directory must leave nothing to check, got {other:?}"),
        }
    }

    #[test]
    fn the_migrations_env_var_is_relative_to_the_manifest() {
        let manifest = Path::new("/crate");
        assert_eq!(
            Schema::resolve_dir("db/migrations", manifest).unwrap(),
            PathBuf::from("/crate/db/migrations"),
        );
        assert_eq!(
            Schema::resolve_dir("/opt/shared/migrations", manifest).unwrap(),
            PathBuf::from("/opt/shared/migrations"),
        );
        assert!(Schema::resolve_dir("  ", manifest).is_none());
    }

    #[test]
    fn a_prepared_select_names_its_columns_without_running() {
        // A `SELECT *` is expanded against the schema at prepare time, which is all a shape check
        // needs: nothing of the query is run.
        let schema = ready();
        let columns = check(&schema, "SELECT * FROM test WHERE ts > ?1", true).unwrap();
        assert_eq!(columns, ["id", "ts", "description"]);
    }

    #[test]
    fn a_valid_query_prepares_against_the_compiled_schema() {
        let schema = ready();
        check(&schema, "SELECT id, ts FROM test WHERE ts > ?", true).unwrap();
        check(
            &schema,
            "SELECT COUNT(*) FROM test WHERE description IS NOT NULL",
            true,
        )
        .unwrap();
    }

    #[test]
    fn a_missing_column_is_named_in_the_error() {
        let schema = ready();
        let err = check(&schema, "SELECT descrption FROM test", true).unwrap_err();
        assert!(err.contains("descrption"), "{err}");
    }

    #[test]
    fn a_missing_table_is_named_in_the_error() {
        let schema = ready();
        let err = check(&schema, "SELECT id FROM nope", true).unwrap_err();
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn a_single_check_rejects_a_multi_statement_string() {
        let schema = ready();
        let err = check(
            &schema,
            "INSERT INTO test (id, ts) VALUES (1, 1); DELETE FROM test",
            true,
        )
        .unwrap_err();
        assert!(err.contains("sql_batch!"), "{err}");
        check(
            &schema,
            "INSERT INTO test (id, ts) VALUES (1, 1); DELETE FROM test",
            false,
        )
        .unwrap();
    }

    #[test]
    fn an_empty_string_is_rejected() {
        let schema = ready();
        assert!(check(&schema, "   ", true).is_err());
    }

    #[test]
    fn a_migration_name_without_an_index_is_reported_not_fatal() {
        let schema = load("bad_name");
        match schema {
            Schema::Broken { msg } => {
                assert!(msg.contains("first.sql"), "{msg}");
                assert!(msg.contains("<integer>_<migration_name>.sql"), "{msg}");
            }
            other => panic!("expected a broken schema, got {other:?}"),
        }
    }

    #[test]
    fn a_migration_index_that_is_not_an_integer_is_reported_not_fatal() {
        let schema = load("bad_index");
        match schema {
            Schema::Broken { msg } => {
                assert!(msg.contains("no_index_here.sql"), "{msg}");
                assert!(msg.contains("integer index"), "{msg}");
            }
            other => panic!("expected a broken schema, got {other:?}"),
        }
    }

    #[test]
    fn a_migration_id_used_twice_is_reported_not_fatal() {
        let schema = load("dup");
        match schema {
            Schema::Broken { msg } => assert!(msg.contains("more than one file"), "{msg}"),
            other => panic!("expected a broken schema, got {other:?}"),
        }
    }

    #[test]
    fn a_migration_id_gap_is_reported_not_fatal() {
        let schema = load("gap");
        match schema {
            Schema::Broken { msg } => assert!(msg.contains("no gaps"), "{msg}"),
            other => panic!("expected a broken schema, got {other:?}"),
        }
    }

    #[test]
    fn a_migration_that_does_not_apply_is_reported_not_fatal() {
        let schema = load("broken_syntax");
        assert!(matches!(schema, Schema::Broken { .. }));
    }

    #[test]
    fn a_changed_migration_changes_the_fingerprint() {
        let files = vec![(
            1_u32,
            PathBuf::from("1_one.sql"),
            b"CREATE TABLE a (a INT)".to_vec(),
        )];
        let before = Schema::fingerprint(&files);
        let changed = vec![(
            1_u32,
            PathBuf::from("1_one.sql"),
            b"CREATE TABLE a (b INT)".to_vec(),
        )];
        assert_ne!(before, Schema::fingerprint(&changed));
        let renamed = vec![(
            2_u32,
            PathBuf::from("2_one.sql"),
            b"CREATE TABLE a (a INT)".to_vec(),
        )];
        assert_ne!(before, Schema::fingerprint(&renamed));
    }

    #[test]
    fn a_verdict_survives_a_round_trip_through_the_cache() {
        let root = temp_root("round_trip");
        let fp = "fp";
        assert!(Cache::lookup(&root, fp, &plain("SELECT 1")).is_none());

        Cache::store(&root, fp, &plain("SELECT 1"), None, &[]);
        let (res, columns) = Cache::lookup(&root, fp, &plain("SELECT 1")).unwrap();
        assert!(res.is_ok(), "{res:?}");
        assert!(columns.is_empty(), "a plain check collects no columns");

        Cache::store(
            &root,
            fp,
            &plain("SELECT nope"),
            Some("no such table: nope"),
            &[],
        );
        let (res, _) = Cache::lookup(&root, fp, &plain("SELECT nope")).unwrap();
        assert_eq!(res.unwrap_err(), "no such table: nope");
        // A query with multi-line text must survive the record's own newline separator.
        let multi = "SELECT 1\nFROM test\nWHERE ts > ?";
        Cache::store(&root, fp, &plain(multi), None, &[]);
        assert!(Cache::lookup(&root, fp, &plain(multi)).unwrap().0.is_ok());
    }

    #[test]
    fn the_columns_of_a_shape_check_survive_the_cache() {
        let root = temp_root("shape_round_trip");
        let fp = "fp";
        let query = "SELECT id, ts FROM test";
        Cache::store(
            &root,
            fp,
            &shape(query),
            None,
            &["id".to_string(), "ts".to_string()],
        );
        let (res, columns) = Cache::lookup(&root, fp, &shape(query)).unwrap();
        assert!(res.is_ok(), "{res:?}");
        assert_eq!(columns, ["id", "ts"]);
        // The same string checked for its columns and checked plainly are two questions with two
        // records, so a cached verdict of one never answers the other.
        assert!(Cache::lookup(&root, fp, &plain(query)).is_none());
    }

    #[test]
    fn a_batch_verdict_never_answers_a_single_statement_check() {
        let root = temp_root("single_vs_batch");
        let fp = "fp";
        let multi = "INSERT INTO t VALUES (1); DELETE FROM t";

        Cache::store(&root, fp, &batch(multi), None, &[]);
        let (res, _) = Cache::lookup(&root, fp, &batch(multi)).unwrap();
        assert!(res.is_ok(), "the batch check of a batch is its own verdict");
        assert!(
            Cache::lookup(&root, fp, &plain(multi)).is_none(),
            "a batch verdict must not clear a single-statement check of the same string"
        );

        Cache::store(
            &root,
            fp,
            &plain(multi),
            Some("`sql!` takes exactly one statement"),
            &[],
        );
        assert_eq!(
            Cache::lookup(&root, fp, &plain(multi))
                .unwrap()
                .0
                .unwrap_err(),
            "`sql!` takes exactly one statement",
        );
        assert!(
            Cache::lookup(&root, fp, &batch(multi)).unwrap().0.is_ok(),
            "a rejected single check must not poison a batch check of the same string"
        );
    }

    #[test]
    fn a_record_without_its_end_line_is_no_verdict() {
        let root = temp_root("torn_record");
        let fp = "fp";
        let check = plain("SELECT 1");
        Cache::store(&root, fp, &check, None, &[]);
        let path = Cache::path(&root, fp, &check);
        let full = fs::read_to_string(&path).unwrap();
        // What a write that stopped early leaves behind: the verdict without its last line.
        fs::write(&path, full.strip_suffix(Cache::RECORD_END).unwrap()).unwrap();

        assert!(
            Cache::lookup(&root, fp, &check).is_none(),
            "a half record is not a verdict, and refilling it is what a missing one does"
        );
        Cache::store(&root, fp, &check, None, &[]);
        assert!(
            Cache::lookup(&root, fp, &check).unwrap().0.is_ok(),
            "the record of a query is refilled after a torn one"
        );
    }

    #[test]
    fn every_field_of_a_record_comes_back_as_it_went_in() {
        let root = temp_root("escaped_fields");
        let fp = "fp";
        let check = shape("SELECT a FROM t");
        let msg = "near \"FROM\": syntax error\nin statement 2";
        Cache::store(
            &root,
            fp,
            &check,
            Some(msg),
            &["a\tb".to_string(), "c\nd".to_string()],
        );
        let (res, columns) = Cache::lookup(&root, fp, &check).unwrap();
        assert_eq!(
            res.unwrap_err(),
            msg,
            "a message over two lines stays one message"
        );
        assert_eq!(
            columns,
            ["a\tb", "c\nd"],
            "a separator inside a column name is not a second column"
        );
    }

    #[test]
    fn the_cache_key_is_per_schema_and_per_query() {
        assert_eq!(
            Cache::key("fp", &plain("SELECT 1")),
            Cache::key("fp", &plain("SELECT 1"))
        );
        assert_ne!(
            Cache::key("fp", &plain("SELECT 1")),
            Cache::key("other", &plain("SELECT 1"))
        );
        assert_ne!(
            Cache::key("fp", &plain("SELECT 1")),
            Cache::key("fp", &plain("SELECT 2"))
        );
        assert_ne!(
            Cache::key("fp", &plain("SELECT 1")),
            Cache::key("fp", &shape("SELECT 1"))
        );
        assert_ne!(
            Cache::key("fp", &plain("SELECT 1")),
            Cache::key("fp", &batch("SELECT 1")),
            "a single statement and a batch of one string are two questions with two records"
        );
    }

    #[test]
    fn the_internal_tables_of_the_check_database_exist() {
        let schema = ready();
        check(&schema, "SELECT id, name, ts, hash FROM _migrations", true).unwrap();
        check(&schema, "SELECT key, data FROM _metadata", true).unwrap();
    }
}
