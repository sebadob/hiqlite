use crate::app_state::AppState;
use crate::query::rows::{
    ColIdx, ColumnInfo, Mapper, MapperTry, RowMapper, RowOwned, RowValues, RowsOwned, Setup,
    TryRowMapper,
};
use crate::query::stream::{QueryStream, StreamVerdict};
use crate::reader::{ActorFailure, QueryRequest, ReaderRequest, query_on_actor};
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::store::state_machine::sqlite::param::Param;
use crate::store::state_machine::sqlite::state_machine::StateMachineSqlite;
use crate::{Error, Params};
use openraft::Raft;
use openraft::raft::ReadPolicy;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

#[cfg(debug_assertions)]
use crate::store::state_machine::sqlite::writer;

pub mod cust_types;
pub mod rows;
pub mod stream;

/// Capacity for the rows of a query where we don't know the result size upfront. A query with a
/// single row never grows past it, a big result pays one reallocation per doubling.
pub(crate) const ROWS_HINT: usize = 4;

pub(crate) async fn query_consistent_local<S>(
    raft: &Raft<TypeConfigSqlite, StateMachineSqlite>,
    log_statements: bool,
    read_actors: &flume::Sender<ReaderRequest>,
    stmt: S,
    params: Params,
    read_policy: ReadPolicy,
) -> Result<RowsOwned, Error>
where
    S: Into<Cow<'static, str>>,
{
    let _ = raft.ensure_linearizable(read_policy).await?;
    query_owned_local(log_statements, read_actors, stmt, params).await
}

pub(crate) async fn query_owned_local<S>(
    log_statements: bool,
    read_actors: &flume::Sender<ReaderRequest>,
    sql: S,
    params: Params,
) -> Result<RowsOwned, Error>
where
    S: Into<Cow<'static, str>>,
{
    let sql: Cow<'static, str> = sql.into();
    if log_statements {
        info!("query_owned_local:\n{sql}\n{params:?}");
    }

    query_on_actor(
        read_actors,
        sql,
        ROWS_HINT,
        move |conn, size_hint, sql| query_owned_local_on_conn(conn, sql, params, size_hint),
        ActorFailure::into_error,
    )
    .await
}

#[inline]
pub(crate) async fn query_map<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
    hint: usize,
) -> Result<Vec<T>, Error>
where
    T: for<'r> From<&'r mut rows::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let sql: Cow<'static, str> = sql.into();
    if state.raft_db.log_statements {
        info!("query_map_typed:\n{sql}\n{params:?}");
    }

    query_on_actor(
        &state.raft_db.read_actors,
        sql,
        hint,
        move |conn, size_hint, sql| query_map_on_conn(conn, sql, params, size_hint),
        ActorFailure::into_error,
    )
    .await
}

#[inline]
pub(crate) async fn query_try_map<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
    hint: usize,
) -> Result<Vec<Result<T, Error>>, Error>
where
    T: for<'r> TryFrom<&'r mut rows::Row<'r>, Error = crate::Error> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let sql: Cow<'static, str> = sql.into();
    if state.raft_db.log_statements {
        info!("query_try_map_typed:\n{sql}\n{params:?}");
    }

    query_on_actor(
        &state.raft_db.read_actors,
        sql,
        hint,
        move |conn, size_hint, sql| query_try_map_on_conn(conn, sql, params, size_hint),
        ActorFailure::into_error,
    )
    .await
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
    let mut rows: Vec<T> = query_map(state, sql, params, 1).await?;
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
pub(crate) async fn query_try_map_one<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<Result<T, Error>, Error>
where
    T: for<'r> TryFrom<&'r mut rows::Row<'r>, Error = crate::Error> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<Result<T, Error>> = query_try_map(state, sql, params, 1).await?;
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
    let mut rows: Vec<T> = query_map(state, sql, params, 1).await?;
    if rows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(rows.swap_remove(0)))
    }
}

#[inline]
pub(crate) async fn query_try_map_optional<T, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<Option<Result<T, Error>>, Error>
where
    T: for<'r> TryFrom<&'r mut rows::Row<'r>, Error = crate::Error> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<Result<T, Error>> = query_try_map(state, sql, params, 1).await?;
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
        info!("query_as:\n{sql}\n{params:?}");
    }

    // `query_as` grows its result freely, like it does with a `Vec::new` today.
    query_on_actor(
        &state.raft_db.read_actors,
        sql,
        0,
        move |conn, size_hint, sql| query_as_on_conn(conn, sql, params, size_hint),
        ActorFailure::into_error,
    )
    .await
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

/// Run a query on the thread that owns its connection and hand its mapped rows to a consumer one
/// at a time, so it walks a result set of any size without ever holding all of it.
///
/// One read actor is leased for the whole stream and the statement is built on its connection once:
/// the mapping's setup comes from that very statement, and the call returns only once that
/// statement is built.  A statement that cannot be prepared, a param that cannot be bound and a
/// mapping whose columns are missing from the result set are the `Err` of this call, exactly like
/// they are for `query_map`.
///
/// The scan runs on the actor and hands over exactly one row at a time: `stream::PREFETCH` is a
/// rendezvous capacity, so it produces a row only when its consumer asks for one and parks on the
/// row nobody has asked for.  A stream you drop ends the query at its next row, and a stream you
/// hold on to without consuming it parks its actor, and with it the connection that actor owns, for
/// as long as you hold it.
///
/// A cursor that fails mid-scan and a row mapping that panics are verdicts about a running query,
/// so they travel to the consumer as the `Err` of a row, which is what keeps a truncated result
/// from ever looking complete.
pub(crate) async fn query_stream<M, S>(
    state: &Arc<AppState>,
    sql: S,
    params: Params,
) -> Result<QueryStream<M::Item>, Error>
where
    M: StreamMapper,
    S: Into<Cow<'static, str>>,
{
    let sql: Cow<'static, str> = sql.into();
    if state.raft_db.log_statements {
        info!("query_stream:\n{sql}\n{params:?}");
    }

    let (rows_tx, rows_rx) = flume::bounded(stream::PREFETCH);
    let (ready_tx, ready_rx) = flume::bounded(1);
    let (verdict_tx, verdict_rx) = flume::bounded(1);
    let verdict_tx_panic = verdict_tx.clone();

    state
        .raft_db
        .read_actors
        .send_async(ReaderRequest::Stream(QueryRequest {
            size_hint: 0,
            sql,
            work: Box::new(move |conn, _size_hint, sql| {
                stream_on_conn::<M>(conn, sql, &params, &ready_tx, &rows_tx, &verdict_tx);
            }),
            panic: Box::new(move |text| {
                let _ = verdict_tx_panic.send(StreamVerdict::Err(Error::Error(text.into())));
            }),
            enqueued: Instant::now(),
        }))
        .await
        .map_err(|_| Error::Sqlite("the read pool is closed".into()))?;

    let Ok(()) = ready_rx.recv_async().await else {
        // The connection stopped before its statement was built, so its own verdict is our error.
        return match verdict_rx.recv_async().await {
            // Only reachable when the caller of this call was dropped mid-build.
            Ok(StreamVerdict::Done) => Err(Error::Sqlite(
                "the stream of a query was dropped before its statement was built".into(),
            )),
            Ok(StreamVerdict::Err(err)) => Err(err),
            Err(_) => Err(Error::Sqlite(
                "the read pool of the query stream died before its statement was built".into(),
            )),
        };
    };

    Ok(QueryStream::new(rows_rx, verdict_rx))
}

/// The mapping a streaming query runs on the thread that owns its connection: a setup resolved
/// once from the statement, and the mapping of one row.  Every `*_stream` variant of the
/// `Client` shares the cursor loop and the channel of `query_stream` through this.
pub(crate) trait StreamMapper {
    type Item: Send + 'static;
    type Setup: Send + 'static;

    fn setup(stmt: &rusqlite::Statement) -> Result<Self::Setup, Error>;

    fn map(row: &rusqlite::Row<'_>, setup: &Self::Setup) -> Result<Self::Item, Error>;
}

/// The `StreamMapper` of `Client::query_map_stream`: the very mapping `Client::query_map` runs,
/// on the query's own thread, so a mapped row costs the allocations of the mapped type only.
pub(crate) struct Map<T>(std::marker::PhantomData<T>);

impl<T> StreamMapper for Map<T>
where
    T: for<'r> From<&'r mut rows::Row<'r>> + Send + 'static,
{
    type Item = T;
    type Setup = (Mapper<T>, Setup);

    fn setup(stmt: &rusqlite::Statement) -> Result<Self::Setup, Error> {
        let mapper = Mapper::new();
        let setup = mapper.setup(stmt)?;
        Ok((mapper, setup))
    }

    fn map(row: &rusqlite::Row<'_>, (mapper, setup): &Self::Setup) -> Result<Self::Item, Error> {
        let mut row = rows::Row::Borrowed(row, setup.row_cols());
        Ok(mapper.map(&mut row, setup))
    }
}

/// The `StreamMapper` of `Client::query_try_map_stream`: a row that cannot be mapped is the `Err`
/// of its item, exactly like it is the `Err` of its slot in `Client::query_try_map`.
pub(crate) struct TryMap<T>(std::marker::PhantomData<T>);

impl<T> StreamMapper for TryMap<T>
where
    T: for<'r> TryFrom<&'r mut rows::Row<'r>, Error = crate::Error> + Send + 'static,
{
    type Item = Result<T, Error>;
    type Setup = (MapperTry<T>, Setup);

    fn setup(stmt: &rusqlite::Statement) -> Result<Self::Setup, Error> {
        let mapper = MapperTry::new();
        let setup = mapper.setup(stmt)?;
        Ok((mapper, setup))
    }

    fn map(row: &rusqlite::Row<'_>, (mapper, setup): &Self::Setup) -> Result<Self::Item, Error> {
        let mut row = rows::Row::Borrowed(row, setup.row_cols());
        Ok(mapper.map(&mut row, setup))
    }
}

/// The `StreamMapper` of `Client::query_raw_stream`: the bare row in its owned form, sharing the
/// one column index of the result set through an `Arc`.
pub(crate) struct Raw;

impl StreamMapper for Raw {
    type Item = RowOwned;
    type Setup = (Vec<ColumnInfo>, Arc<ColIdx>);

    fn setup(stmt: &rusqlite::Statement) -> Result<Self::Setup, Error> {
        let columns = ColumnInfo::mapping_cols_from_stmt(stmt.columns());
        Ok((columns, Arc::new(ColIdx::from_stmt(stmt))))
    }

    fn map(row: &rusqlite::Row<'_>, (columns, cols): &Self::Setup) -> Result<Self::Item, Error> {
        Ok(RowOwned::new(
            RowValues::from_row_column(row, columns),
            cols.clone(),
        ))
    }
}

fn query_owned_local_on_conn(
    conn: &rusqlite::Connection,
    sql: &str,
    params: Params,
    size_hint: usize,
) -> Result<RowsOwned, Error> {
    let mut stmt = conn.prepare_cached(sql)?;
    let columns = ColumnInfo::mapping_cols_from_stmt(stmt.columns());

    #[cfg(debug_assertions)]
    writer::check_stmt_params_count(&stmt, &params, sql);

    let mut idx = 1;
    #[allow(clippy::explicit_counter_loop)]
    for param in params {
        stmt.raw_bind_parameter(idx, param.into_sql())?;
        idx += 1;
    }

    let mut rows = stmt.raw_query();
    let mut rows_owned = Vec::with_capacity(size_hint);
    loop {
        match rows.next() {
            Ok(Some(row)) => rows_owned.push(RowValues::from_row_column(row, &columns)),
            Ok(None) => break,
            Err(err) => {
                // never silently return a truncated result
                return Err(Error::Sqlite(err.to_string().into()));
            }
        }
    }

    let names = columns.into_iter().map(|c| c.name).collect();
    Ok(RowsOwned::new(names, rows_owned))
}

fn query_map_on_conn<T>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: Params,
    size_hint: usize,
) -> Result<Vec<T>, Error>
where
    T: for<'r> From<&'r mut rows::Row<'r>> + Send + 'static,
{
    let mut stmt = conn.prepare_cached(sql)?;

    #[cfg(debug_assertions)]
    writer::check_stmt_params_count(&stmt, &params, sql);

    let mut idx = 1;
    #[allow(clippy::explicit_counter_loop)]
    for param in params {
        stmt.raw_bind_parameter(idx, param.into_sql())?;
        idx += 1;
    }

    let mapper = rows::Mapper::<T>::new();
    let setup = mapper.setup(&stmt)?;
    let mut rows = stmt.raw_query();
    let mut res = Vec::with_capacity(size_hint);
    loop {
        match rows.next() {
            Ok(Some(row)) => {
                let mut row = rows::Row::Borrowed(row, setup.row_cols());
                res.push(mapper.map(&mut row, &setup));
            }
            Ok(None) => break,
            Err(err) => return Err(Error::Sqlite(err.to_string().into())),
        }
    }
    Ok(res)
}

fn query_try_map_on_conn<T>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: Params,
    size_hint: usize,
) -> Result<Vec<Result<T, Error>>, Error>
where
    T: for<'r> TryFrom<&'r mut rows::Row<'r>, Error = crate::Error> + Send + 'static,
{
    let mut stmt = conn.prepare_cached(sql)?;

    #[cfg(debug_assertions)]
    writer::check_stmt_params_count(&stmt, &params, sql);

    let mut idx = 1;
    #[allow(clippy::explicit_counter_loop)]
    for param in params {
        stmt.raw_bind_parameter(idx, param.into_sql())?;
        idx += 1;
    }

    let mapper = rows::MapperTry::<T>::new();
    let setup = mapper.setup(&stmt)?;
    let mut rows = stmt.raw_query();
    let mut res = Vec::with_capacity(size_hint);
    loop {
        match rows.next() {
            Ok(Some(row)) => {
                let mut row = rows::Row::Borrowed(row, setup.row_cols());
                res.push(mapper.map(&mut row, &setup));
            }
            Ok(None) => break,
            Err(err) => return Err(Error::Sqlite(err.to_string().into())),
        }
    }
    Ok(res)
}

fn query_as_on_conn<T>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: Params,
    size_hint: usize,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + Send + 'static,
{
    let mut stmt = conn.prepare_cached(sql)?;

    #[cfg(debug_assertions)]
    writer::check_stmt_params_count(&stmt, &params, sql);

    let mut idx = 1;
    #[allow(clippy::explicit_counter_loop)]
    for param in params {
        stmt.raw_bind_parameter(idx, param.into_sql())?;
        idx += 1;
    }

    let mut rows = serde_rusqlite::from_rows::<T>(stmt.raw_query());
    let mut res = Vec::with_capacity(size_hint);
    loop {
        match rows.next() {
            Some(Ok(ty)) => res.push(ty),
            Some(Err(err)) => return Err(Error::Sqlite(err.to_string().into())),
            None => break,
        }
    }
    Ok(res)
}

/// Run a streaming query on the actor that leased it and answer its scan verdict.
///
/// The verdict is what tells a consumer a scan that ended without a last row did not end cleanly,
/// so it is sent for every way the scan can leave the loop, including the panic the actor caught
/// around this call - which is why a failed scan never looks like a complete result.
fn stream_on_conn<M>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[Param],
    ready_tx: &flume::Sender<()>,
    rows_tx: &flume::Sender<Result<M::Item, Error>>,
    verdict_tx: &flume::Sender<StreamVerdict>,
) where
    M: StreamMapper,
{
    let verdict = match stream_scan::<M>(conn, sql, params, ready_tx, rows_tx) {
        Ok(()) => StreamVerdict::Done,
        Err(err) => StreamVerdict::Err(err),
    };
    let _ = verdict_tx.send(verdict);
}

fn stream_scan<M>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[Param],
    ready_tx: &flume::Sender<()>,
    rows_tx: &flume::Sender<Result<M::Item, Error>>,
) -> Result<(), Error>
where
    M: StreamMapper,
{
    let mut stmt = conn.prepare_cached(sql)?;

    #[cfg(debug_assertions)]
    writer::check_stmt_params_count(&stmt, params, sql);

    for (idx, param) in params.iter().enumerate() {
        stmt.raw_bind_parameter(idx + 1, param.clone().into_sql())?;
    }

    let setup = M::setup(&stmt)?;

    // The statement is built before the scan starts: a consumer that is gone stops us here.
    if ready_tx.send(()).is_err() {
        return Ok(());
    }

    let mut rows = stmt.raw_query();
    loop {
        match rows.next() {
            Ok(Some(row)) => {
                let item = M::map(row, &setup)?;
                // The consumer dropped its stream: stop the scan right here.
                if rows_tx.send(Ok(item)).is_err() {
                    return Ok(());
                }
            }
            Ok(None) => break,
            Err(err) => return Err(Error::Sqlite(err.to_string().into())),
        }
    }
    Ok(())
}
