use crate::network::api::ApiStreamResponsePayload;
use crate::network::AppStateExt;
use crate::query::rows::{ColumnOwned, RowOwned, ValueOwned};
use crate::store::state_machine::sqlite::state_machine::{Query, QueryWrite};
use crate::{params, Error};
use tokio::sync::oneshot;
use tokio::task;
use tracing::info;

pub(crate) async fn dashboard_query_dynamic(
    state: AppStateExt,
    sql: String,
) -> Result<Vec<RowOwned>, Error> {
    if sql.len() < 8 {
        return Err(Error::Sqlite("invalid query".into()));
    }

    if state.raft_db.log_statements {
        info!("dashboard query:\n{}", sql)
    }

    // we need to check if we can do a local select query or if it is
    // modifying and needs to go through the raft
    let sql_start = sql[..7].to_lowercase();
    let is_select = sql_start.starts_with("select") || sql_start.starts_with("explain");

    if is_select {
        let conn = state.raft_db.read_pool.get().await?;

        task::spawn_blocking(move || {
            let mut stmt = conn.prepare(&sql)?;

            let columns = ColumnOwned::mapping_cols_from_stmt(stmt.columns())?;

            let mut rows = stmt.raw_query();
            let mut rows_owned = Vec::new();
            while let Ok(Some(row)) = rows.next() {
                rows_owned.push(RowOwned::from_row_column(row, &columns));
            }

            Ok::<Vec<RowOwned>, Error>(rows_owned)
        })
        .await?
    } else {
        let sql = Query {
            sql: sql.into(),
            params: params!(),
        };

        let rows_affected = execute_dynamic(&state, sql.clone()).await?;
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
            .expect("Client Stream Manager to always be running");
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
