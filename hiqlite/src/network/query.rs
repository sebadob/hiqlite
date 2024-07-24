// TODO
// - query_optional
// - read transaction ?
// - query_simple
// - Batch (same as simple?)

use crate::app_state::AppState;
use crate::{Error, Params};
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::task;
use tracing::info;

pub(crate) async fn query_map<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<Vec<T>, Error>
where
    T: for<'r> From<&'r rusqlite::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let stmt: Cow<'static, str> = stmt.into();
    if state.log_statements {
        info!("query_map:\n{}\n{:?}", stmt, params)
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
            res.push(T::from(row));
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
    T: for<'r> From<&'r rusqlite::Row<'r>> + Send + 'static,
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
