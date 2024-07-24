use crate::app_state::AppState;
use crate::network::api::{ApiStreamResponse, ApiStreamResponsePayload, WsWriteMsg};
use crate::network::AppStateExt;
use crate::query::rows::{ColumnOwned, RowOwned};
use crate::store::state_machine::sqlite::state_machine::SqlitePool;
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::{Error, Params};
use openraft::Raft;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::task;
use tracing::{error, info};

pub mod rows;

// TODO
// - query_optional

pub(crate) async fn query_consistent<S>(
    state: AppStateExt,
    stmt: S,
    params: Params,
    request_id: usize,
    tx_ws_writer: flume::Sender<WsWriteMsg>,
) where
    S: Into<Cow<'static, str>>,
{
    let res = query_consistent_local(
        &state.raft,
        state.log_statements,
        state.read_pool.clone(),
        stmt,
        params,
    )
    .await;

    if let Err(err) = tx_ws_writer
        .send_async(WsWriteMsg::Payload(ApiStreamResponse {
            request_id,
            result: Ok(ApiStreamResponsePayload::QueryConsistent(res)),
        }))
        .await
    {
        error!("{}", err);
    }
}

pub(crate) async fn query_consistent_local<S>(
    raft: &Raft<TypeConfigSqlite>,
    log_statements: bool,
    read_pool: Arc<SqlitePool>,
    stmt: S,
    params: Params,
) -> Result<Vec<RowOwned>, Error>
where
    S: Into<Cow<'static, str>>,
{
    let stmt: Cow<'static, str> = stmt.into();
    if log_statements {
        info!("query_consistent:\n{}\n{:?}", stmt, params)
    }

    let conn = read_pool.get().await?;
    let _ = raft.ensure_linearizable().await?;

    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(stmt.as_ref())?;
        let columns = ColumnOwned::mapping_cols_from_stmt(stmt.columns())?;

        let mut idx = 1;
        for param in params {
            stmt.raw_bind_parameter(idx, param.into_sql())?;
            idx += 1;
        }

        let mut rows = stmt.raw_query();
        let mut rows_owned = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            rows_owned.push(RowOwned::from_row_column(row, &columns));
        }

        Ok::<Vec<RowOwned>, Error>(rows_owned)
    })
    .await?
}

pub(crate) async fn query_map<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<Vec<T>, Error>
where
    T: for<'r> From<rows::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let stmt: Cow<'static, str> = stmt.into();
    if state.log_statements {
        info!("query_map_typed:\n{}\n{:?}", stmt, params)
    }

    let conn = state.read_pool.get().await?;
    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(stmt.as_ref())?;

        let mut idx = 1;
        for param in params {
            stmt.raw_bind_parameter(idx, param.into_sql())?;
            idx += 1;
        }

        let mut rows = stmt.raw_query();
        let mut res = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            res.push(T::from(rows::Row::Borrowed(row)));
        }
        Ok::<Vec<T>, Error>(res)
    })
    .await?
}

pub(crate) async fn query_map_one<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<T, Error>
where
    T: for<'r> From<rows::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_map(state, stmt, params).await?;
    if rows.is_empty() {
        Err(Error::Sqlite("no rows returned".into()))
    } else {
        Ok(rows.swap_remove(0))
    }
}

pub(crate) async fn query_as<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let stmt: Cow<'static, str> = stmt.into();
    if state.log_statements {
        info!("query_as:\n{}\n{:?}", stmt, params)
    }

    let conn = state.read_pool.get().await?;
    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(stmt.as_ref())?;

        let mut idx = 1;
        for param in params {
            stmt.raw_bind_parameter(idx, param.into_sql())?;
            idx += 1;
        }

        let mut rows = serde_rusqlite::from_rows::<T>(stmt.raw_query());
        let mut res = Vec::new();
        while let Some(Ok(ty)) = rows.next() {
            res.push(ty);
        }
        Ok::<Vec<T>, Error>(res)
    })
    .await?
}

pub(crate) async fn query_as_one<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<T, Error>
where
    T: DeserializeOwned + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_as(state, stmt, params).await?;
    if rows.is_empty() {
        Err(Error::Sqlite("no rows returned".into()))
    } else {
        Ok(rows.swap_remove(0))
    }
}
