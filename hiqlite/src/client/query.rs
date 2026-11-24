use crate::client::helpers::await_channel_response;
use crate::client::stream::{ClientQueryPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::{
    ColIdx, Mapper, MapperTry, Row, RowMapper, RowOwned, RowValues, RowsOwned, TryRowMapper,
};
use crate::store::state_machine::sqlite::state_machine::Query;
use crate::{Client, Error, Params, query};
use openraft::raft::ReadPolicy;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::oneshot;

impl Client {
    /// Execute a consistent query. This query will run on the leader node only and pause Raft
    /// replication at a point, where all "current" logs have been applied to at least a quorum
    /// of all nodes. This means whatever result this query returns, at least hals of the nodes + 1
    /// will have the exact same result and it will be the same even if you would end up in a
    /// network segmentation and loose half of your data directly afterward.
    ///
    /// The `read_policy` defines how the leader makes sure that it still is the leader, and that
    /// those logs are committed, before it answers:
    ///
    /// - `ReadPolicy::ReadIndex` confirms it against a quorum of all nodes - the strong option
    ///   and the one to pick when in doubt.
    /// - `ReadPolicy::LeaseRead` trusts the leader's own leadership lease, which skips the network
    ///   round-trip and is faster, but it is slightly weaker and sensitive to clock drift.
    ///
    /// This query is very expensive compared to the other ones. It needs network roud-trips, pauses
    /// the raft and allocates a lot more memory, because it is working with owned data rather than
    /// with borrowed local one for quick mapping.
    /// You should only use it, if you really need to.
    ///
    /// # Errors
    ///
    /// [`Error::RateLimit`] when the DB request rate limit is exhausted, then the answer of the
    /// read on the leader: [`Error::PrepareStatement`] and [`Error::QueryParams`] for a
    /// statement that cannot be built from the given SQL and params, [`Error::Sqlite`] for a
    /// query the state machine cannot run, and on a remote client [`Error::Error`] when the
    /// answer channel of the leader is closed and [`Error::Connect`] when it times out, with
    /// exactly one re-send after [`Error::LeaderChange`], and the
    /// [`Error::CheckIsLeaderErrorDb`] of a leadership the leader could not confirm under the
    /// given `read_policy`.
    pub async fn query_consistent<S>(
        &self,
        stmt: S,
        params: Params,
        read_policy: ReadPolicy,
    ) -> Result<Vec<crate::Row<'_>>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        // A consistent query it not a write operation, but it needs to travel to the leader,
        // and the raft needs to be blocked.
        self.rate_limit_db().await?;

        self.query_remote::<S>(stmt, params, Some(read_policy))
            .await
    }

    /// Execute a consistent query. This query will run on the leader node only and pause Raft
    /// replication at a point where all "current" logs have been applied to at least a quorum
    /// of all nodes. This means whatever result this query returns, at least hals of the nodes + 1
    /// will have the exact same result, and it will be the same even if you end up in a network
    /// segmentation and lose half of your data directly afterwards.
    ///
    /// The `read_policy` defines how the leader makes sure that it still is the leader, and that
    /// those logs are committed, before it answers - see `query_consistent()`.
    ///
    /// This query is very expensive compared to the other ones. It needs network round-trips, pauses
    /// the raft, and allocates a lot more memory, because it is working with owned data rather than
    /// with borrowed local one for quick mapping.
    /// You should only use it if you really need to.
    ///
    /// # Errors
    ///
    /// [`Error::RateLimit`] when the DB request rate limit is exhausted, and everything
    /// [`Self::query_consistent`] can return. The mapping of a row is not an error - see the
    /// panics.
    ///
    /// # Panics
    ///
    /// If the returned data from the DB does not match the requested output type.
    pub async fn query_consistent_map<T, S>(
        &self,
        stmt: S,
        params: Params,
        read_policy: ReadPolicy,
    ) -> Result<Vec<T>, Error>
    where
        T: for<'a, 'r> From<&'a mut crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        self.rate_limit_db().await?;

        let mapper = Mapper::<T>::new();
        let (cols, rows) = self
            .query_remote_parts(stmt, params, Some(read_policy))
            .await?;
        let setup = mapper.setup_owned(&cols)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let mut row = Row::Owned(RowOwned::new(row, setup.row_cols_owned()));
                mapper.map(&mut row, &setup)
            })
            .collect())
    }
    /// Query data from the database and map it to the given `struct`.
    ///
    /// The `struct` must implement `impl From<&mut hiqlite::Row<'_>>` for this to work:
    ///
    /// ```rust, ignore
    /// #[derive(Debug)]
    /// struct MyStruct {
    ///     pub id: String,
    ///     pub num: i64,
    ///     pub description: Option<String>,
    /// }
    ///
    /// impl From<&mut Row<'_>> for MyStruct {
    ///     fn from(row: &mut Row<'_>) -> Self {
    ///         Self {
    ///             id: row.get("id"),
    ///             num: row.get("num"),
    ///             description: row.get("description"),
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// This gives
    /// you the most amount of flexibility to achieve more complicated or optimized mapping.
    /// If you want a more comfortable and easier way, take a look at `.query_as()`.
    ///
    /// A type derived with `FromRow` is mapped through the hoisted index path: its field names are
    /// resolved against the result set once, before the first row, and every row reads its columns
    /// by index, so a row never looks a name up.  The first query of a type is the one that
    /// registers it, so that one maps by name.  A hand-written `impl From<&mut Row>` is mapped by
    /// the names it reads in its own body.
    ///
    /// ```rust, ignore
    /// let res: Vec<MyStruct> = client
    ///     .query_map("SELECT * FROM test", params!())
    ///     .await?;
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`].
    ///
    /// # Panics
    ///
    /// If the returned data from the DB does not match the requested output type.
    pub async fn query_map<T, S>(&self, stmt: S, params: Params) -> Result<Vec<T>, Error>
    where
        T: for<'a, 'r> From<&'a mut crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_map(state, stmt, params, query::ROWS_HINT).await
        } else {
            let mapper = Mapper::<T>::new();
            let (cols, rows) = self.query_remote_parts(stmt, params, None).await?;
            let setup = mapper.setup_owned(&cols)?;
            Ok(rows
                .into_iter()
                .map(|row| {
                    let mut row = Row::Owned(RowOwned::new(row, setup.row_cols_owned()));
                    mapper.map(&mut row, &setup)
                })
                .collect())
        }
    }

    /// The non-collecting version of `query_map`: the rows arrive as you consume them, mapped on
    /// the thread that runs the query.
    ///
    /// ```rust, ignore
    /// let mut stream = client
    ///     .query_map_stream::<MyStruct, _>("SELECT * FROM test", params!())
    ///     .await?;
    ///
    /// while let Some(row) = stream.next().await? {
    ///     let id: i64 = row.id;
    /// }
    /// ```
    ///
    /// The rows come as a [`crate::QueryStream`], which implements
    /// `futures_util::stream::Stream`, so the whole set of combinators works on it once
    /// [`crate::StreamExt`] is in scope, and any conversion you apply in flight stays lazy - a
    /// `take(n)` never reads the rows behind it, and a stream you drop early ends the query at its
    /// next row.
    ///
    /// This is the streaming shape of a very big result: a query over a million rows costs the
    /// memory of the single row it is handing over, and it goes straight out of your API with
    /// [`crate::QueryStream::into_json_response`], which answers with one JSON document per line.
    /// A combinator chain out of [`crate::StreamExt`] answers with the same method from
    /// [`crate::JsonLinesExt`].
    ///
    /// **Note:**
    /// This works for local clients only, not for `hiqlite::Client::remote()`, and there is no
    /// streaming version of a consistent query on purpose: a consistent query pauses Raft
    /// replication, and that pause must be over as fast as the query that needs it is.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client - a stream has no leader to read it from - plus the
    /// statement, param and mapping-setup failures of `query_map`, because the statement of a
    /// stream is built before the stream is handed back, and [`Error::Sqlite`] when a connection
    /// cannot be taken from the read pool.  A cursor that fails mid-scan, and a row mapping that
    /// panicked on the thread running the query, are the `Err` of [`crate::QueryStream::next`]: a
    /// verdict about a query that is already running.
    ///
    /// # Panics
    ///
    /// On the thread running the query, if the returned data from the DB does not match the
    /// requested output type.
    pub async fn query_map_stream<T, S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<query::stream::QueryStream<T>, Error>
    where
        T: for<'a, 'r> From<&'a mut crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_stream::<query::Map<T>, _>(state, stmt, params).await
        } else {
            Err(Error::Config(
                "`query_map_stream()` only works for local clients, you need to use \
                `query_map()` for remote"
                    .into(),
            ))
        }
    }

    /// Non-panicking version of `query_map`.
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`]. A row that cannot be mapped into `T` is the `Err` of its
    /// slot in the returned `Vec`.
    pub async fn query_try_map<T, S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<Vec<Result<T, Error>>, Error>
    where
        T: for<'a, 'r> TryFrom<&'a mut crate::Row<'r>, Error = crate::Error> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_try_map(state, stmt, params, query::ROWS_HINT).await
        } else {
            let mapper = MapperTry::<T>::new();
            let (cols, rows) = self.query_remote_parts(stmt, params, None).await?;
            let setup = mapper.setup_owned(&cols)?;
            Ok(rows
                .into_iter()
                .map(|row| {
                    let mut row = Row::Owned(RowOwned::new(row, setup.row_cols_owned()));
                    mapper.map(&mut row, &setup)
                })
                .collect())
        }
    }

    /// The non-collecting version of `query_try_map`: the rows arrive as you consume them, mapped
    /// on the thread that runs the query.
    ///
    /// Each item of the stream is a row: the `Ok` of it is the mapped row, the `Err` of it is the
    /// row that cannot be mapped into `T`, and the `Err` of [`crate::QueryStream::next`] is a
    /// failure of the query itself - the same split `query_try_map` makes between a slot of its
    /// `Vec` and its own `Err`.
    ///
    /// ```rust, ignore
    /// let mut stream = client
    ///     .query_try_map_stream::<MyStruct, _>("SELECT * FROM test", params!())
    ///     .await?;
    ///
    /// while let Some(res) = stream.next().await? {
    ///     // the outer `?` is the query's, this one is the row's
    ///     let row = res?;
    /// }
    /// ```
    ///
    /// **Note:**
    /// This works for local clients only, not for `hiqlite::Client::remote()`.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client, the statement and param failures of
    /// `query_try_map`, and [`Error::Sqlite`] when a connection cannot be taken from the read pool.
    /// A query that fails mid-scan is the `Err` of [`crate::QueryStream::next`], as in
    /// `query_map_stream`.
    pub async fn query_try_map_stream<T, S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<query::stream::QueryStream<Result<T, Error>>, Error>
    where
        T: for<'a, 'r> TryFrom<&'a mut crate::Row<'r>, Error = crate::Error> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_stream::<query::TryMap<T>, _>(state, stmt, params).await
        } else {
            Err(Error::Config(
                "`query_try_map_stream()` only works for local clients, you need to use \
                `query_try_map()` for remote"
                    .into(),
            ))
        }
    }

    /// Works in the same way as `query_map()`, but returns only one result.
    ///
    /// Errors if not exactly a single row has been returned.
    ///
    /// ```rust, ignore
    /// let res: MyStruct = client
    ///     .query_map_one("SELECT * FROM test WHERE id = $1", params!("id1"))
    ///     .await?;
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`], [`Error::QueryReturnedNoRows`] when the query returns no
    /// row at all and [`Error::Sqlite`] when it returns more than one.
    ///
    /// # Panics
    ///
    /// If the returned data from the DB does not match the requested output type.
    pub async fn query_map_one<T, S>(&self, stmt: S, params: Params) -> Result<T, Error>
    where
        T: for<'r> From<&'r mut crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_map_one(state, stmt, params).await
        } else {
            let mapper = Mapper::<T>::new();
            let (cols, mut rows) = self.query_remote_parts(stmt, params, None).await?;
            let setup = mapper.setup_owned(&cols)?;
            if rows.is_empty() {
                Err(Error::QueryReturnedNoRows("No rows returned".into()))
            } else if rows.len() > 1 {
                Err(Error::Sqlite(
                    format!("cannot map {} rows into one", rows.len()).into(),
                ))
            } else {
                let mut row =
                    Row::Owned(RowOwned::new(rows.swap_remove(0), setup.row_cols_owned()));
                Ok(mapper.map(&mut row, &setup))
            }
        }
    }

    /// Non-panicking version of `query_map`.
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`], [`Error::QueryReturnedNoRows`] when the query returns no
    /// row at all and [`Error::Sqlite`] when it returns more than one. A row that cannot be
    /// mapped into `T` is the returned `Err`.
    pub async fn query_try_map_one<T, S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<Result<T, Error>, Error>
    where
        T: for<'r> TryFrom<&'r mut crate::Row<'r>, Error = crate::Error> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_try_map_one(state, stmt, params).await
        } else {
            let mapper = MapperTry::<T>::new();
            let (cols, mut rows) = self.query_remote_parts(stmt, params, None).await?;
            let setup = mapper.setup_owned(&cols)?;
            if rows.is_empty() {
                Err(Error::QueryReturnedNoRows("No rows returned".into()))
            } else if rows.len() > 1 {
                Err(Error::Sqlite(
                    format!("cannot map {} rows into one", rows.len()).into(),
                ))
            } else {
                let mut row =
                    Row::Owned(RowOwned::new(rows.swap_remove(0), setup.row_cols_owned()));
                Ok(mapper.map(&mut row, &setup))
            }
        }
    }

    /// Works in the same way as `query_map_one()`, but returns only one result as an `Option<T>`.
    /// If no rows have been returned (without database errors), you will get an `Ok(None)`.
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`], and [`Error::Sqlite`] when the query returns more than
    /// one row. No rows at all is `Ok(None)`.
    ///
    /// # Panics
    ///
    /// If the returned data from the DB does not match the requested output type.
    pub async fn query_map_optional<T, S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<Option<T>, Error>
    where
        T: for<'r> From<&'r mut crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_map_optional(state, stmt, params).await
        } else {
            let mapper = Mapper::<T>::new();
            let (cols, mut rows) = self.query_remote_parts(stmt, params, None).await?;
            let setup = mapper.setup_owned(&cols)?;
            if rows.is_empty() {
                Ok(None)
            } else {
                let mut row =
                    Row::Owned(RowOwned::new(rows.swap_remove(0), setup.row_cols_owned()));
                Ok(Some(mapper.map(&mut row, &setup)))
            }
        }
    }

    /// Non-panicking version of `query_map`.
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`], and [`Error::Sqlite`] when the query returns more than
    /// one row. No rows at all is `Ok(None)`; a row that cannot be mapped into `T` is the `Err`
    /// inside the `Some`.
    pub async fn query_try_map_optional<T, S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<Option<Result<T, Error>>, Error>
    where
        T: for<'r> TryFrom<&'r mut crate::Row<'r>, Error = crate::Error> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_try_map_optional(state, stmt, params).await
        } else {
            let mapper = MapperTry::<T>::new();
            let (cols, mut rows) = self.query_remote_parts(stmt, params, None).await?;
            let setup = mapper.setup_owned(&cols)?;
            if rows.is_empty() {
                Ok(None)
            } else {
                let mut row =
                    Row::Owned(RowOwned::new(rows.swap_remove(0), setup.row_cols_owned()));
                Ok(Some(mapper.map(&mut row, &setup)))
            }
        }
    }
    /// Converts data returned from a sql query into a struct which derives `serde::Deserialize`.
    ///
    /// This is the easiest and most straight forward way of getting data. This is most often the
    /// fasted way of mapping values while needing a little bit more memory.
    ///
    /// ```rust, ignore
    /// let res: Vec<Entity> = client
    ///     .query_as("SELECT * FROM test", params!())
    ///     .await?;
    /// ```
    ///
    /// **Note:**
    /// This works for local clients only, not for `hiqlite::Client::remote()` and not for
    /// `query_consistent`.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client - serde mapping needs the local rows - plus
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`].
    pub async fn query_as<T, S>(&self, stmt: S, params: Params) -> Result<Vec<T>, Error>
    where
        T: DeserializeOwned + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_as(state, stmt, params).await
        } else {
            Err(Error::Config(
                "`query_as()` only works for local clients, you need to use \
                `query_map()` for remote"
                    .into(),
            ))
        }
    }

    /// Works in the same way as `query_as()`, but returns only one result.
    ///
    /// Errors if no rows are returned and ignores additional results if more than one row returned.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client, [`Error::PrepareStatement`] and
    /// [`Error::QueryParams`] for a statement that cannot be built from the given SQL and
    /// params, [`Error::Sqlite`] for a query the state machine cannot run, and on a remote
    /// client [`Error::Error`] when the answer channel of the leader is closed and
    /// [`Error::Connect`] when it times out, with exactly one re-send after
    /// [`Error::LeaderChange`], [`Error::QueryReturnedNoRows`] when the query returns no row at
    /// all; more than one row is mapped to the first one and the rest is ignored.
    pub async fn query_as_one<T, S>(&self, stmt: S, params: Params) -> Result<T, Error>
    where
        T: DeserializeOwned + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_as_one(state, stmt, params).await
        } else {
            Err(Error::Config(
                "`query_as()` only works for local clients, you need to use \
                `query_map()` for remote"
                    .into(),
            ))
        }
    }

    /// Works in the same way as `query_as_one()`, but returns only one result as `Option<T>`.
    /// Unlike the `query_as_one()`, this does not throw an error if no Rows have been returned,
    /// but just returns `None`.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client, [`Error::PrepareStatement`] and
    /// [`Error::QueryParams`] for a statement that cannot be built from the given SQL and
    /// params, [`Error::Sqlite`] for a query the state machine cannot run, and on a remote
    /// client [`Error::Error`] when the answer channel of the leader is closed and
    /// [`Error::Connect`] when it times out, with exactly one re-send after
    /// [`Error::LeaderChange`], and [`Error::Sqlite`] when the query returns more than one row.
    /// No rows at all is `Ok(None)`.
    pub async fn query_as_optional<T, S>(&self, stmt: S, params: Params) -> Result<Option<T>, Error>
    where
        T: DeserializeOwned + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_as_optional(state, stmt, params).await
        } else {
            Err(Error::Config(
                "`query_as_optional()` only works for local clients, you need to use \
                `query_map()` for remote"
                    .into(),
            ))
        }
    }

    /// A raw query will return the bare `Row` without doing any deserialization or mapping.
    /// This can be useful if you just need to know if a query succeeds, or if you need to manually
    /// work with the result without being able to convert it into a type.
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`].
    pub async fn query_raw<S>(&self, stmt: S, params: Params) -> Result<Vec<crate::Row<'_>>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            let (cols, rows) = query::query_owned_local(
                state.raft_db.log_statements,
                state.raft_db.read_pool.clone(),
                stmt,
                params,
            )
            .await?
            .into_parts();
            Ok(rows
                .into_iter()
                .map(|row| crate::Row::Owned(RowOwned::new(row, cols.clone())))
                .collect())
        } else {
            self.query_remote(stmt, params, None).await
        }
    }

    /// The non-collecting version of `query_raw`: bare rows, owned and streaming, as you consume
    /// them.
    ///
    /// ```rust, ignore
    /// let mut stream = client
    ///     .query_raw_stream("SELECT id, description FROM test", params!())
    ///     .await?;
    ///
    /// while let Some(mut row) = stream.next().await? {
    ///     // every row carries the header of its result set
    ///     println!("columns: {:?}", row.columns());
    ///     let id: i64 = row.get("id");
    ///     let description: Option<String> = row.try_get("description").unwrap();
    ///     println!("{id}: {description:?}");
    /// }
    /// ```
    ///
    /// Each row is an [`crate::RowOwned`]: the values of a row that has been read out of the
    /// statement, with the column index of its result set shared through an `Arc`, so you read it
    /// by name exactly like a row of `query_raw`.  The header of the result set is the
    /// [`crate::RowOwned::columns`] of any of its rows, shared through that same `Arc`, and an
    /// `RowOwned` serializes itself as a JSON object keyed by exactly those names, so a raw stream
    /// is a streaming JSON response with a single
    /// [`crate::QueryStream::into_json_response`] call.
    ///
    /// **Note:**
    /// This works for local clients only, not for `hiqlite::Client::remote()`.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client, [`Error::PrepareStatement`] and
    /// [`Error::QueryParams`] for a statement that cannot be built from the given SQL and params,
    /// [`Error::Sqlite`] when a connection cannot be taken from the read pool, and the `Err` of
    /// [`crate::QueryStream::next`] for a cursor that fails mid-scan.
    ///
    /// # Panics
    ///
    /// On the thread reading the stream, if a column's value cannot be converted into the type an
    /// `RowOwned::get()` declares, like every other owned row does.
    pub async fn query_raw_stream<S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<query::stream::QueryStream<crate::RowOwned>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_stream::<query::Raw, _>(state, stmt, params).await
        } else {
            Err(Error::Config(
                "`query_raw_stream()` only works for local clients, you need to use \
                `query_raw()` for remote"
                    .into(),
            ))
        }
    }

    /// A raw query will return the bare `Row` without doing any deserialization or mapping.
    ///
    /// This version will return exactly one `Row`.
    /// Throws an error if the returned rows are not exactly one.
    ///
    /// # Errors
    ///
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] for a query the state machine
    /// cannot run, and on a remote client [`Error::Error`] when the answer channel of the
    /// leader is closed and [`Error::Connect`] when it times out, with exactly one re-send
    /// after [`Error::LeaderChange`], [`Error::Sqlite`] when the query returns no row at all
    /// and [`Error::Sqlite`] when it returns more than one.
    pub async fn query_raw_one<S>(&self, stmt: S, params: Params) -> Result<crate::Row<'_>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        let mut rows = self.query_raw(stmt, params).await?;
        if rows.is_empty() {
            Err(Error::Sqlite("No rows returned".into()))
        } else if rows.len() > 1 {
            Err(Error::Sqlite(
                format!("cannot map {} rows into one", rows.len()).into(),
            ))
        } else {
            Ok(rows.swap_remove(0))
        }
    }

    /// Executes a query on remote host and returns raw rows.
    pub(crate) async fn query_remote<S>(
        &self,
        stmt: S,
        params: Params,
        read_policy: Option<ReadPolicy>,
    ) -> Result<Vec<crate::Row<'_>>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        let (cols, rows) = self.query_remote_parts(stmt, params, read_policy).await?;
        Ok(rows
            .into_iter()
            .map(|row| crate::Row::Owned(RowOwned::new(row, cols.clone())))
            .collect())
    }

    /// Executes a query on a remote host and hands back its rows together with the column index of
    /// the result set they belong to, which is what the hoisted index path resolves against.
    async fn query_remote_parts<S>(
        &self,
        stmt: S,
        params: Params,
        read_policy: Option<ReadPolicy>,
    ) -> Result<(Arc<ColIdx>, Vec<RowValues>), Error>
    where
        S: Into<Cow<'static, str>>,
    {
        let query = Query {
            sql: stmt.into(),
            params,
        };

        let (cols, rows) = match self
            .query_remote_req(query.clone(), read_policy.clone())
            .await
        {
            Ok(res) => Ok(res),
            Err(err) => {
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    self.query_remote_req(query, read_policy).await
                } else {
                    return Err(err);
                }
            }
        }?
        .into_parts();
        Ok((cols, rows))
    }

    pub(crate) async fn query_remote_req(
        &self,
        query: Query,
        read_policy: Option<ReadPolicy>,
    ) -> Result<RowsOwned, Error> {
        let (ack, rx) = oneshot::channel();
        let request_id = self.new_request_id();

        let payload = match read_policy {
            Some(read_policy) => ClientStreamReq::QueryConsistent(
                ClientQueryPayload {
                    request_id,
                    query,
                    ack,
                },
                read_policy,
            ),
            None => ClientStreamReq::Query(ClientQueryPayload {
                request_id,
                query,
                ack,
            }),
        };

        self.inner
            .tx_client_db
            .send_async(payload)
            .await
            .map_err(|err| Error::Error(err.to_string().into()))?;
        let res = await_channel_response(rx).await??;
        match res {
            ApiStreamResponsePayload::Query(res)
            | ApiStreamResponsePayload::QueryConsistent(res) => res,
            _ => unreachable!(),
        }
    }
}
