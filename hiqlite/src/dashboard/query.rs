use crate::network::AppStateExt;
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::{ColumnInfo, RowValues, RowsOwned, ValueOwned};
use crate::reader::{ActorFailure, query_on_actor};
use crate::store::state_machine::sqlite::state_machine::{
    FORBIDDEN_NON_DET_FNS, Query, QueryWrite,
};
use crate::{Error, Params};
use serde::ser::{SerializeMap as _, SerializeSeq as _};
use serde::{Serialize, Serializer};
use std::borrow::Cow;
use tokio::sync::oneshot;
use tracing::debug;

/// The result of a dashboard query, serialized in the JSON shape the dashboard reads: a list of
/// objects, one per row, keyed by the column names of the query.
pub(crate) struct DashboardRows<'a> {
    columns: &'a [String],
    rows: &'a [RowValues],
}

impl<'a> DashboardRows<'a> {
    pub(crate) fn new(rows: &'a RowsOwned) -> Self {
        Self {
            columns: &rows.columns,
            rows: &rows.rows,
        }
    }
}

/// One row paired with the column names it is keyed by.
struct RowJson<'a> {
    columns: &'a [String],
    row: &'a RowValues,
}

impl Serialize for RowJson<'_> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut obj = ser.serialize_map(Some(self.columns.len()))?;
        for (name, value) in self.columns.iter().zip(self.row.values.iter()) {
            obj.serialize_entry(name, &value)?;
        }
        obj.end()
    }
}

impl Serialize for DashboardRows<'_> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut rows = ser.serialize_seq(Some(self.rows.len()))?;
        for row in self.rows {
            rows.serialize_element(&RowJson {
                columns: self.columns,
                row,
            })?;
        }
        rows.end()
    }
}

pub(crate) async fn dashboard_query_dynamic(
    state: AppStateExt,
    sql: String,
) -> Result<RowsOwned, Error> {
    if state.raft_db.log_statements {
        debug!("dashboard query:\n{sql}");
    }

    // SQLite itself decides if the query reads or writes: it is prepared on a read connection,
    // and only a statement that would modify the database goes through the Raft.
    let rows = query_on_actor(
        &state.raft_db.read_actors,
        Cow::from(sql.clone()),
        0,
        move |conn, _size_hint, sql| dashboard_query_on_conn(conn, sql),
        ActorFailure::into_error,
    )
    .await?;

    if let Some(rows) = rows {
        Ok(rows)
    } else {
        // The write path panics on non-deterministic functions. Dashboard queries are
        // manual, so catch them up front and return a readable error instead.
        if let Some(fn_name) = find_forbidden_non_det_fn(&sql) {
            return Err(Error::BadRequest(
                format!(
                    "`{fn_name}()` is non-deterministic and must never be used for writing \
                    queries in a Raft cluster"
                )
                .into(),
            ));
        }

        let sql = Query {
            sql: sql.into(),
            params: Params::new(),
        };

        // TODO check for `RETURNING` to execute `query` instead
        let rows_affected = match execute_dynamic(&state, sql.clone()).await {
            Ok(r) => r,
            Err(err) => {
                if let Some((id, node)) = err.is_forward_to_leader() {
                    state
                        .tx_client_stream
                        .send_async(crate::client::stream::ClientStreamReq::LeaderChange((
                            id,
                            node.clone(),
                        )))
                        .await
                        .map_err(|err| Error::Error(err.to_string().into()))?;
                    execute_dynamic(&state, sql.clone()).await?
                } else {
                    return Err(err);
                }
            }
        };

        // Saturate at `i64::MAX`: the value is only reported to the UI as a number.
        let affected = i64::try_from(rows_affected).unwrap_or(i64::MAX);
        Ok(RowsOwned::new(
            vec!["rows_affected".to_string()],
            vec![RowValues {
                values: vec![Some(ValueOwned::Integer(affected))],
            }],
        ))
    }
}

/// A manual dashboard query on a read actor's own connection.
///
/// It is also the classification of the query, done by SQLite itself on the prepared statement.
/// Returns `None` without running anything when the statement modifies the database: such a
/// query must go through the Raft. A read connection is opened read-only, so a misrouted write
/// would only fail here and never change the local database.
///
/// Transaction control, `ATTACH` and `DETACH` count as read-only for SQLite, while they change
/// the state of the connection they run on. A read connection serves the next queries of the
/// node, so these statements are rejected: they have no result and nothing to show anyway.
///
/// `prepare` and not `prepare_cached`: a dashboard query is typed by a human, so it is not a
/// statement the node runs again, and its SQL must not occupy a slot of the actor's cache.
fn dashboard_query_on_conn(
    conn: &rusqlite::Connection,
    sql: &str,
) -> Result<Option<RowsOwned>, Error> {
    // SQLite applies most `PRAGMA` assignments while it prepares them, so this check must run
    // before the statement is prepared.
    reject_pragma_assignment(conn, sql)?;

    let mut stmt = match conn.prepare(sql) {
        Ok(stmt) => stmt,
        Err(rusqlite::Error::Empty) => return Err(Error::BadRequest("invalid query".into())),
        Err(rusqlite::Error::MultipleStatement) => {
            return Err(Error::BadRequest(
                "the dashboard executes a single statement per query".into(),
            ));
        }
        Err(err) => return Err(err.into()),
    };

    // An `EXPLAIN` never executes the statement it explains, and a `PRAGMA` never goes through
    // the Raft: an assignment was rejected before, and the query form only reads.
    let is_local = stmt.is_explain() != 0 || take_word(sql).0.eq_ignore_ascii_case("pragma");
    if !is_local {
        if !stmt.readonly() {
            return Ok(None);
        }
        if stmt.column_count() == 0 {
            return Err(Error::BadRequest(
                "the statement changes the state of the connection and not the database, which \
                the dashboard does not execute"
                    .into(),
            ));
        }
    }

    let columns = ColumnInfo::mapping_cols_from_stmt(stmt.columns());

    let mut rows = stmt.raw_query();
    let mut rows_owned = Vec::new();
    loop {
        match rows.next() {
            Ok(Some(row)) => rows_owned.push(RowValues::from_row_column(row, &columns)),
            Ok(None) => break,
            Err(err) => {
                // never silently show a truncated result in the dashboard
                return Err(Error::Sqlite(err.to_string().into()));
            }
        }
    }

    let names = columns.into_iter().map(|c| c.name).collect();
    Ok(Some(RowsOwned::new(names, rows_owned)))
}

#[inline]
async fn execute_dynamic(state: &AppStateExt, sql: Query) -> Result<usize, Error> {
    if is_this_local_leader(state).await? {
        debug!("Executing dynamic dashboard query as local leader");
        let res = state
            .raft_db
            .raft
            .client_write(QueryWrite::Execute(sql))
            .await?;
        let resp: crate::Response = res.data;
        match resp {
            crate::Response::Execute(res) => res.result,
            _ => unreachable!(),
        }
    } else {
        debug!("Executing dynamic dashboard query on remote leader");
        let (ack, rx) = oneshot::channel();
        state
            .tx_client_stream
            .send_async(crate::client::stream::ClientStreamReq::Execute(
                crate::client::stream::ClientExecutePayload {
                    request_id: state.new_request_id(),
                    sql,
                    ack,
                },
            ))
            .await
            .map_err(|err| Error::Error(err.to_string().into()))?;
        let res = rx
            .await
            .expect("To always receive an answer from Client Stream Manager")?;
        match res {
            ApiStreamResponsePayload::Execute(res) => res,
            _ => unreachable!(),
        }
    }
}

#[inline]
pub(crate) async fn is_this_local_leader(state: &AppStateExt) -> Result<bool, Error> {
    match state.raft_db.raft.current_leader().await {
        None => Err(Error::LeaderChange(
            "Leader election has not finished yet".into(),
        )),
        Some(current) => {
            if state.id == current {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

/// Finds a forbidden non-deterministic function in a manual dashboard query.
/// String literals and comments are skipped, so `'now()'` is not taken for a call.
fn find_forbidden_non_det_fn(sql: &str) -> Option<&'static str> {
    let lowered = sql.to_ascii_lowercase();
    let bytes = lowered.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        if in_string {
            if bytes[i] == b'\'' {
                // SQL escapes a quote by doubling it: '' stays inside the string
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }

        match bytes[i] {
            b'\'' => {
                in_string = true;
                i += 1;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                // skip to end of line
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                // skip until */
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            _ => {
                if let Some(name) = FORBIDDEN_NON_DET_FNS.iter().copied().find(|name| {
                    let needle = name.as_bytes();
                    bytes[i..].starts_with(needle)
                        && bytes[i..].get(needle.len()) == Some(&b'(')
                        && (i == 0 || {
                            let prev = bytes[i - 1];
                            !prev.is_ascii_alphanumeric() && prev != b'_'
                        })
                }) {
                    return Some(name);
                }
                i += 1;
            }
        }
    }
    None
}

/// Rejects a manual dashboard query with a `PRAGMA` that sets a value, in any of its statements.
///
/// SQLite applies most pragma assignments while it prepares the statement, before a single step,
/// and that includes an `EXPLAIN PRAGMA` and a pragma in the tail of a multi-statement query,
/// which `rusqlite` prepares to detect it. On a read connection, the assignment would change a
/// connection that serves the next queries of this node, its `query_only` guard included. Through
/// the Raft, it would change the writer of every node. A dashboard pragma only ever reads.
fn reject_pragma_assignment(conn: &rusqlite::Connection, sql: &str) -> Result<(), Error> {
    for start in statement_starts(sql) {
        let (mut keyword, mut rest) = take_word(&sql[start..]);
        if keyword.eq_ignore_ascii_case("explain") {
            (keyword, rest) = take_word(rest);
            if keyword.eq_ignore_ascii_case("query") {
                let (_plan, after) = take_word(rest);
                (keyword, rest) = take_word(after);
            }
        }

        if keyword.eq_ignore_ascii_case("pragma") && !is_pragma_query(conn, rest)? {
            return Err(Error::BadRequest(
                "the dashboard only executes a `PRAGMA` that reads its value, never one that \
                sets it"
                    .into(),
            ));
        }
    }
    Ok(())
}

/// If the rest of a `PRAGMA` statement, after the keyword, only reads.
///
/// Without a value, a pragma reports its current one. With a value, as `name = value` or
/// `name(value)`, SQLite itself tells if the pragma is a query: only then, like `table_info(t)`,
/// its table-valued function `pragma_name` takes the value as the hidden `arg` column.
fn is_pragma_query(conn: &rusqlite::Connection, rest: &str) -> Result<bool, Error> {
    let Some((mut name, mut rest)) = take_name(rest) else {
        return Ok(false);
    };
    if let Some(after_dot) = skip_trivia(rest).strip_prefix('.') {
        let Some((n, r)) = take_name(after_dot) else {
            return Ok(false);
        };
        (name, rest) = (n, r);
    }

    match skip_trivia(rest).as_bytes().first() {
        None | Some(b';') => Ok(true),
        Some(b'(' | b'=') => {
            let args: i64 = conn.query_row(
                "SELECT count(*) FROM pragma_table_xinfo(?1) WHERE name = 'arg' AND hidden = 1",
                [format!("pragma_{name}")],
                |row| row.get(0),
            )?;
            Ok(args > 0)
        }
        Some(_) => Ok(false),
    }
}

/// The byte offsets where the statements of `sql` start: the start of the text, and every
/// position after a `;` that is not inside a string, a quoted identifier or a comment.
fn statement_starts(sql: &str) -> Vec<usize> {
    let bytes = sql.as_bytes();
    let mut starts = vec![0];
    let mut i = 0;
    while i < bytes.len() {
        let close = match bytes[i] {
            b'\'' => Some(b'\''),
            b'"' => Some(b'"'),
            b'`' => Some(b'`'),
            b'[' => Some(b']'),
            _ => None,
        };
        if let Some(close) = close {
            // A doubled quote is an escaped one: the scan simply leaves and re-enters the quote.
            i += 1;
            while i < bytes.len() && bytes[i] != close {
                i += 1;
            }
            i += 1;
            continue;
        }

        if bytes[i..].starts_with(b"--") || bytes[i..].starts_with(b"/*") {
            i = bytes.len() - skip_trivia(&sql[i..]).len();
            continue;
        }

        if bytes[i] == b';' {
            starts.push(i + 1);
        }
        i += 1;
    }
    starts
}

/// `sql` without its leading whitespace and comments.
fn skip_trivia(mut sql: &str) -> &str {
    loop {
        let trimmed = sql.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if let Some(comment) = trimmed.strip_prefix("--") {
            sql = comment.find('\n').map_or("", |end| &comment[end..]);
        } else if let Some(comment) = trimmed.strip_prefix("/*") {
            sql = comment.find("*/").map_or("", |end| &comment[end + 2..]);
        } else {
            return trimmed;
        }
    }
}

/// The leading bare word of `sql` after whitespace and comments, and the text after it. The word
/// is empty when `sql` does not start with one.
fn take_word(sql: &str) -> (&str, &str) {
    let sql = skip_trivia(sql);
    let end = sql
        .find(|c: char| c.is_ascii() && !c.is_ascii_alphanumeric() && c != '_' && c != '$')
        .unwrap_or(sql.len());
    sql.split_at(end)
}

/// The leading bare word of `sql`, or its leading quoted name without the quotes.
fn take_name(sql: &str) -> Option<(&str, &str)> {
    let sql = skip_trivia(sql);
    let close = match sql.as_bytes().first()? {
        b'\'' => '\'',
        b'"' => '"',
        b'`' => '`',
        b'[' => ']',
        _ => {
            let (word, rest) = take_word(sql);
            return (!word.is_empty()).then_some((word, rest));
        }
    };
    let quoted = &sql[1..];
    let end = quoted.find(close)?;
    Some((&quoted[..end], &quoted[end + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_fn_scan_catches_only_real_calls() {
        // forbidden calls are detected ...
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES (now())"),
            Some("now")
        );
        assert_eq!(
            find_forbidden_non_det_fn("UPDATE t SET at = strftime('%s','now') WHERE id = 1"),
            Some("strftime")
        );
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES (datetime('now'))"),
            Some("datetime")
        );
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES (NOW())"),
            Some("now")
        );

        // ... while names that merely contain a forbidden fn do not match
        assert_eq!(find_forbidden_non_det_fn("SELECT * FROM my_now"), None);
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES ('a strftime b')"),
            None
        );

        // ... and forbidden names inside string literals or comments are not calls
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES ('now()')"),
            None
        );
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES ('a''now()''b')"),
            None
        );
        assert_eq!(
            find_forbidden_non_det_fn("-- now()\nINSERT INTO t VALUES (1)"),
            None
        );
        assert_eq!(
            find_forbidden_non_det_fn("/* now() */ INSERT INTO t VALUES (1)"),
            None
        );
    }

    /// A database with one row in `t`, and a connection to it opened like a read actor's.
    fn read_conn(name: &str) -> (rusqlite::Connection, rusqlite::Connection) {
        let dir = std::env::temp_dir().join(format!(
            "hiqlite_dashboard_query_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("dashboard.db");

        let writer = rusqlite::Connection::open(&db).unwrap();
        writer.pragma_update(None, "journal_mode", "WAL").unwrap();
        writer
            .execute_batch(
                "CREATE TABLE t (a INTEGER UNIQUE REFERENCES t (a)); INSERT INTO t VALUES (1);",
            )
            .unwrap();

        let reader =
            rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        reader.pragma_update(None, "query_only", true).unwrap();
        reader
            .busy_timeout(std::time::Duration::from_secs(30))
            .unwrap();
        (writer, reader)
    }

    fn count(conn: &rusqlite::Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM t", (), |row| row.get(0))
            .unwrap()
    }

    fn pragma(conn: &rusqlite::Connection, name: &str) -> i64 {
        conn.query_row(&format!("PRAGMA {name}"), (), |row| row.get(0))
            .unwrap()
    }

    fn is_bad_request(res: &Result<Option<RowsOwned>, Error>) -> bool {
        matches!(res, Err(Error::BadRequest(_)))
    }

    #[test]
    fn reads_run_on_the_read_connection() {
        let (_writer, reader) = read_conn("reads");

        for sql in [
            "SELECT a FROM t",
            "-- a comment\nSELECT a FROM t",
            "/* a comment */ SELECT a FROM t",
            "\n\t  select a from t",
            "WITH x AS (SELECT a FROM t) SELECT a FROM x",
            "VALUES (1)",
            "EXPLAIN QUERY PLAN SELECT a FROM t",
            // the table info button of the dashboard
            "PRAGMA table_info(t)",
            "PRAGMA main.table_info(t)",
            "PRAGMA table_info = 't'",
            "PRAGMA table_xinfo(\"t\")",
            "PRAGMA index_list(t)",
            "PRAGMA index_info(sqlite_autoindex_t_1)",
            "PRAGMA foreign_key_list(t)",
            "PRAGMA integrity_check(10)",
            "PRAGMA busy_timeout",
            "SELECT * FROM pragma_table_info('t')",
        ] {
            let rows = dashboard_query_on_conn(&reader, sql)
                .unwrap_or_else(|err| panic!("`{sql}` must read: {err:?}"))
                .unwrap_or_else(|| panic!("`{sql}` is a read and must not go through the Raft"));
            assert!(!rows.rows.is_empty(), "`{sql}` must return its rows");
        }
    }

    #[test]
    fn writes_go_through_the_raft() {
        let (writer, reader) = read_conn("writes");

        for sql in [
            "INSERT INTO t VALUES (2)",
            "WITH x AS (SELECT 2 AS a) INSERT INTO t SELECT a FROM x",
            "-- a comment\nWITH x AS (SELECT 1) UPDATE t SET a = 3",
            "WITH x AS (SELECT 1) DELETE FROM t",
            "CREATE TABLE u (a INTEGER)",
        ] {
            assert!(
                dashboard_query_on_conn(&reader, sql).unwrap().is_none(),
                "`{sql}` writes and must go through the Raft"
            );
        }
        assert_eq!(
            count(&writer),
            1,
            "a classification must never execute a write"
        );
    }

    #[test]
    fn explain_never_executes_the_explained_write() {
        let (writer, reader) = read_conn("explain");

        let rows = dashboard_query_on_conn(&reader, "EXPLAIN INSERT INTO t VALUES (2)")
            .unwrap()
            .expect("an `EXPLAIN` only reads");
        assert!(!rows.rows.is_empty());
        assert_eq!(count(&writer), 1);
    }

    #[test]
    fn connection_state_statements_are_rejected() {
        let (_writer, reader) = read_conn("conn_state");

        for sql in ["BEGIN", "SAVEPOINT s", "ATTACH ':memory:' AS m"] {
            assert!(
                is_bad_request(&dashboard_query_on_conn(&reader, sql)),
                "`{sql}` must be rejected"
            );
        }
        assert!(
            reader.is_autocommit(),
            "a read connection must never be left inside a transaction"
        );
        let attached: i64 = reader
            .query_row("SELECT count(*) FROM pragma_database_list", (), |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            attached, 1,
            "a read connection must never attach a database"
        );
    }

    #[test]
    fn pragma_assignments_are_rejected_before_they_are_prepared() {
        let (writer, reader) = read_conn("pragma");
        let busy_timeout = pragma(&reader, "busy_timeout");
        let foreign_keys = pragma(&reader, "foreign_keys");

        for sql in [
            "PRAGMA query_only = 0",
            "PRAGMA query_only = OFF",
            "pragma busy_timeout=0",
            "PRAGMA busy_timeout(0)",
            "PRAGMA main.cache_size = 10",
            "PRAGMA \"busy_timeout\" = 0",
            "/* a comment */ PRAGMA foreign_keys = ON",
            "PRAGMA foreign_keys = OFF",
            "PRAGMA journal_mode = DELETE",
            "PRAGMA journal_mode(DELETE)",
            "PRAGMA user_version = 3",
            "PRAGMA user_version(5)",
            "PRAGMA main.user_version(5)",
            "PRAGMA query_only(0)",
            "EXPLAIN PRAGMA busy_timeout = 0",
            "EXPLAIN QUERY PLAN PRAGMA busy_timeout = 0",
            "SELECT 1; PRAGMA busy_timeout = 0",
            "SELECT ';'; /* ; */ PRAGMA busy_timeout = 0",
        ] {
            assert!(
                is_bad_request(&dashboard_query_on_conn(&reader, sql)),
                "`{sql}` must be rejected"
            );
        }

        assert_eq!(pragma(&reader, "query_only"), 1);
        assert_eq!(pragma(&reader, "foreign_keys"), foreign_keys);
        assert_eq!(pragma(&reader, "busy_timeout"), busy_timeout);
        assert_eq!(pragma(&writer, "user_version"), 0);
        let journal_mode: String = writer
            .query_row("PRAGMA journal_mode", (), |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "wal");
    }

    #[test]
    fn multiple_statements_are_rejected() {
        let (writer, reader) = read_conn("multi");

        for sql in [
            "SELECT a FROM t; DELETE FROM t",
            "DELETE FROM t; SELECT a FROM t",
            "INSERT INTO t VALUES (2); INSERT INTO t VALUES (3)",
        ] {
            assert!(
                is_bad_request(&dashboard_query_on_conn(&reader, sql)),
                "`{sql}` must be rejected"
            );
        }
        assert_eq!(count(&writer), 1);

        // a trailing `;` or comment is not a second statement
        assert!(
            dashboard_query_on_conn(&reader, "SELECT a FROM t; -- done")
                .unwrap()
                .is_some()
        );
        assert!(is_bad_request(&dashboard_query_on_conn(
            &reader,
            " -- only a comment"
        )));
    }

    #[test]
    fn statement_starts_skip_quotes_and_comments() {
        assert_eq!(statement_starts("SELECT 1"), vec![0]);
        assert_eq!(statement_starts("SELECT 1; SELECT 2"), vec![0, 9]);
        assert_eq!(
            statement_starts("SELECT ';', \"a;b\", [c;d], `e;f`"),
            vec![0]
        );
        assert_eq!(statement_starts("SELECT 'it''s; here'"), vec![0]);
        assert_eq!(statement_starts("SELECT 1 -- ;\n/* ; */"), vec![0]);
        assert_eq!(statement_starts("SELECT 1 /* ; */;"), vec![0, 17]);
    }
}
