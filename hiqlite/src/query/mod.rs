use crate::app_state::AppState;
use crate::query::rows::{ColumnOwned, RowOwned};
use crate::store::state_machine::sqlite::state_machine::SqlitePool;
use crate::store::state_machine::sqlite::{TypeConfigSqlite, writer};
use crate::{Error, Params};
use openraft::Raft;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::task;
use tracing::info;

pub mod rows;

pub(crate) async fn query_consistent_local<S>(
    raft: &Raft<TypeConfigSqlite>,
    log_statements: bool,
    read_pool: SqlitePool,
    stmt: S,
    params: Params,
) -> Result<Vec<RowOwned>, Error>
where
    S: Into<Cow<'static, str>>,
{
    let _ = raft.ensure_linearizable().await?;
    query_owned_local(log_statements, read_pool, stmt, params).await
}

pub(crate) async fn query_owned_local<S>(
    log_statements: bool,
    read_pool: SqlitePool,
    sql: S,
    params: Params,
) -> Result<Vec<RowOwned>, Error>
where
    S: Into<Cow<'static, str>>,
{
    let sql: Cow<'static, str> = sql.into();
    if log_statements {
        info!("query_owned_local:\n{}\n{:?}", sql, params)
    }

    let conn = read_pool.get().await?;

    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(sql.as_ref())?;
        let columns = ColumnOwned::mapping_cols_from_stmt(stmt.columns())?;

        #[cfg(debug_assertions)]
        writer::check_stmt_params_count(&stmt, &params, &sql);

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

#[inline(always)]
pub(crate) async fn query_map<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<Vec<T>, Error>
where
    T: for<'r> From<&'r mut rows::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let sql: Cow<'static, str> = sql.into();
    if state.raft_db.log_statements {
        info!("query_map_typed:\n{}\n{:?}", sql, params)
    }

    let conn = state.raft_db.read_pool.get().await?;
    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(sql.as_ref())?;

        #[cfg(debug_assertions)]
        writer::check_stmt_params_count(&stmt, &params, &sql);

        let mut idx = 1;
        for param in params {
            stmt.raw_bind_parameter(idx, param.into_sql())?;
            idx += 1;
        }

        let mut rows = stmt.raw_query();
        let mut res = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            res.push(T::from(&mut rows::Row::Borrowed(row)));
        }
        Ok::<Vec<T>, Error>(res)
    })
    .await?
}

#[inline]
pub(crate) async fn query_map_one<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<T, Error>
where
    T: for<'r> From<&'r mut rows::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_map(state, sql, params).await?;
    if rows.is_empty() {
        Err(Error::QueryReturnedNoRows("no rows returned".into()))
    } else if rows.len() > 1 {
        Err(Error::Sqlite(
            format!("cannot map {} rows into one", rows.len()).into(),
        ))
    } else {
        Ok(rows.swap_remove(0))
    }
}

#[inline]
pub(crate) async fn query_map_optional<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<Option<T>, Error>
where
    T: for<'r> From<&'r mut rows::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_map(state, sql, params).await?;
    if rows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(rows.swap_remove(0)))
    }
}

#[inline]
pub(crate) async fn query_as<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let sql: Cow<'static, str> = sql.into();
    if state.raft_db.log_statements {
        info!("query_as:\n{}\n{:?}", sql, params)
    }

    let conn = state.raft_db.read_pool.get().await?;
    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(sql.as_ref())?;

        #[cfg(debug_assertions)]
        writer::check_stmt_params_count(&stmt, &params, &sql);

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

#[inline]
pub(crate) async fn query_as_one<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<T, Error>
where
    T: DeserializeOwned + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_as(state, sql, params).await?;
    if rows.is_empty() {
        Err(Error::QueryReturnedNoRows("no rows returned".into()))
    } else if rows.len() > 1 {
        Err(Error::Sqlite(
            format!("cannot map {} rows into one", rows.len()).into(),
        ))
    } else {
        Ok(rows.swap_remove(0))
    }
}

#[inline]
pub(crate) async fn query_as_optional<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<Option<T>, Error>
where
    T: DeserializeOwned + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_as(state, sql, params).await?;
    if rows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(rows.swap_remove(0)))
    }
}
