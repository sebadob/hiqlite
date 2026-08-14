use crate::network::AppStateExt;
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::{ColumnOwned, RowOwned, ValueOwned};
use crate::store::state_machine::sqlite::state_machine::{Query, QueryWrite, FORBIDDEN_NON_DET_FNS};
use crate::{Error, Params};
use tokio::sync::oneshot;
use tokio::task;
use tracing::debug;

pub(crate) async fn dashboard_query_dynamic(
    state: AppStateExt,
    sql: String,
) -> Result<Vec<RowOwned>, Error> {
    if sql.len() < 8 {
        return Err(Error::BadRequest("invalid query".into()));
    }

    if state.raft_db.log_statements {
        debug!("dashboard query:\n{}", sql)
    }

    // we need to check if we can do a local select query or if it is
    // modifying and needs to go through the raft
    let sql_start = sql[..7].to_lowercase();
    let is_select = sql_start.starts_with("select")
        || sql_start.starts_with("explain")
        || sql_start.starts_with("pragma");

    if is_select {
        let conn = state.raft_db.read_pool.get().await?;

        task::spawn_blocking(move || {
            let mut stmt = conn.prepare(&sql)?;

            let columns = ColumnOwned::mapping_cols_from_stmt(stmt.columns())?;

            let mut rows = stmt.raw_query();
            let mut rows_owned = Vec::new();
            loop {
                match rows.next() {
                    Ok(Some(row)) => rows_owned.push(RowOwned::from_row_column(row, &columns)),
                    Ok(None) => break,
                    Err(err) => {
                        // never silently show a truncated result in the dashboard
                        return Err(Error::Sqlite(err.to_string().into()));
                    }
                }
            }

            Ok::<Vec<RowOwned>, Error>(rows_owned)
        })
        .await?
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

        let affected = if rows_affected > i64::MAX as usize {
            i64::MAX
        } else {
            rows_affected as i64
        };
        Ok(vec![RowOwned {
            columns: vec![ColumnOwned {
                name: "rows_affected".to_string(),
                value: ValueOwned::Integer(affected),
            }],
        }])
    }
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

#[inline(always)]
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
                        && (i == 0
                            || {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_fn_scan_catches_only_real_calls() {
        // forbidden calls are detected ...
        assert_eq!(find_forbidden_non_det_fn("INSERT INTO t VALUES (now())"), Some("now"));
        assert_eq!(
            find_forbidden_non_det_fn("UPDATE t SET at = strftime('%s','now') WHERE id = 1"),
            Some("strftime")
        );
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES (datetime('now'))"),
            Some("datetime")
        );
        assert_eq!(find_forbidden_non_det_fn("INSERT INTO t VALUES (NOW())"), Some("now"));

        // ... while names that merely contain a forbidden fn do not match
        assert_eq!(find_forbidden_non_det_fn("SELECT * FROM my_now"), None);
        assert_eq!(
            find_forbidden_non_det_fn("INSERT INTO t VALUES ('a strftime b')"),
            None
        );

        // ... and forbidden names inside string literals or comments are not calls
        assert_eq!(find_forbidden_non_det_fn("INSERT INTO t VALUES ('now()')"), None);
        assert_eq!(find_forbidden_non_det_fn("INSERT INTO t VALUES ('a''now()''b')"), None);
        assert_eq!(
            find_forbidden_non_det_fn("-- now()\nINSERT INTO t VALUES (1)"),
            None
        );
        assert_eq!(
            find_forbidden_non_det_fn("/* now() */ INSERT INTO t VALUES (1)"),
            None
        );
    }
}
