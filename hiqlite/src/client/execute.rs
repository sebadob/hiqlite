use crate::client::helpers::await_channel_response;
use crate::client::stream::{ClientExecutePayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::RowsOwnedChecked;
use crate::store::state_machine::sqlite::state_machine::{Query, QueryWrite};
use crate::{Client, Error, Params, Response};
use std::borrow::Cow;
use tokio::sync::oneshot;

impl Client {
    /// Execute any modifying / non-read-only query on the database.
    /// Returns the affected rows on success.
    ///
    /// ```rust, notest
    /// client
    ///     .execute(
    ///         "INSERT INTO test (id, num, description) VALUES ($1, $2, $3)",
    ///         params!("id1", 123, "my description"),
    ///     )
    ///     .await?;
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::RateLimit`] when the DB request rate limit is exhausted,
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built from the given SQL and params, [`Error::Sqlite`] when it fails inside the state
    /// machine, and [`Error::Error`] when the request cannot be handed to the leader's stream,
    /// [`Error::Connect`] when its answer times out, and exactly one re-send after
    /// [`Error::LeaderChange`].
    pub async fn execute<S>(&self, sql: S, params: Params) -> Result<usize, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        self.rate_limit_db().await?;

        let sql = Query {
            sql: sql.into(),
            params,
        };

        match self.execute_req(sql.clone()).await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    self.execute_req(sql).await
                } else {
                    Err(err)
                }
            }
        }
    }

    #[inline]
    async fn execute_req(&self, sql: Query) -> Result<usize, Error> {
        if let Some(state) = self.is_leader_db_with_state().await {
            let res =
                Self::client_write_local(&state.raft_db.raft, QueryWrite::Execute(sql)).await?;
            let resp: Response = res.data;
            match resp {
                Response::Execute(res) => res.result,
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.inner
                .tx_client_db
                .send_async(ClientStreamReq::Execute(ClientExecutePayload {
                    request_id: self.new_request_id(),
                    sql,
                    ack,
                }))
                .await
                .map_err(|err| Error::Error(err.to_string().into()))?;
            let res = await_channel_response(rx).await??;
            match res {
                ApiStreamResponsePayload::Execute(res) => res,
                _ => unreachable!(),
            }
        }
    }

    /// Execute a query on the database that includes a `RETURNING` statement.
    ///
    /// Returns the rows mapped to the output type on success. This only works for types that
    /// `impl From<&mut hiqlite::Row<'_>>`
    ///
    /// # Errors
    ///
    /// Everything [`Self::execute_returning`] returns is passed through unchanged. A row that
    /// cannot be mapped into `T` is the `Err` of its slot in the returned `Vec`, not an error
    /// of this call.
    pub async fn execute_returning_map<S, T>(
        &self,
        sql: S,
        params: Params,
    ) -> Result<Vec<Result<T, Error>>, Error>
    where
        S: Into<Cow<'static, str>>,
        T: for<'a, 'r> From<&'a mut crate::Row<'r>> + Send + 'static,
    {
        self.rate_limit_db().await?;

        let rows: Vec<Result<crate::Row, Error>> = self.execute_returning::<S>(sql, params).await?;
        let mut res: Vec<Result<T, Error>> = Vec::with_capacity(rows.len());
        for row in rows {
            res.push(row.map(|mut row| T::from(&mut row)));
        }
        Ok(res)
    }

    /// Execute a query on the database that includes a `RETURNING` statement.
    ///
    /// Returns the row mapped to the output type on success. This only works for types that
    /// `impl From<&mut hiqlite::Row<'_>>`.
    ///
    /// Throws an error if not exactly 1 row has been returned.
    ///
    /// # Errors
    ///
    /// Everything [`Self::execute_returning_map`] returns, plus [`Error::QueryReturnedNoRows`]
    /// when the query returns no row at all and [`Error::Sqlite`] when it returns more than
    /// one.
    pub async fn execute_returning_map_one<S, T>(&self, sql: S, params: Params) -> Result<T, Error>
    where
        S: Into<Cow<'static, str>>,
        T: for<'a, 'r> From<&'a mut crate::Row<'r>> + Send + 'static,
    {
        self.rate_limit_db().await?;

        let mut rows = self.execute_returning_map::<S, T>(sql, params).await?;
        if rows.is_empty() {
            Err(Error::QueryReturnedNoRows("no rows returned".into()))
        } else if rows.len() > 1 {
            Err(Error::Sqlite(
                format!("cannot map {} rows into one", rows.len()).into(),
            ))
        } else {
            rows.swap_remove(0)
        }
    }

    /// Execute a query on the database that includes a `RETURNING` statement.
    /// Returns the raw rows on success.
    ///
    /// # Errors
    ///
    /// [`Error::RateLimit`] when the DB request rate limit is exhausted,
    /// [`Error::PrepareStatement`] and [`Error::QueryParams`] for a statement that cannot be
    /// built, [`Error::Sqlite`] for a failure inside the state machine, and [`Error::Error`]
    /// when the request cannot be handed to the leader's stream, [`Error::Connect`] when its
    /// answer times out, and exactly one re-send after [`Error::LeaderChange`]. A row that
    /// cannot be mapped is the `Err` of its slot in the returned `Vec`.
    pub async fn execute_returning<S>(
        &self,
        sql: S,
        params: Params,
    ) -> Result<Vec<Result<crate::Row<'_>, Error>>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        self.rate_limit_db().await?;

        let sql = Query {
            sql: sql.into(),
            params,
        };

        let rows = match self.execute_returning_req(sql.clone()).await {
            Ok(res) => res,
            Err(err) => {
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    self.execute_returning_req(sql).await?
                } else {
                    return Err(err);
                }
            }
        };

        let (cols, rows) = rows.into_parts();

        let mut res: Vec<Result<crate::Row, Error>> = Vec::with_capacity(rows.len());
        for row in rows {
            res.push(row.map(|row| crate::Row::Owned(row, cols.clone())));
        }
        Ok(res)
    }

    /// Execute a query on the database that includes a `RETURNING` statement.
    ///
    /// Returns a single raw row. Will throw an error if rows returned is not exactly 1.
    ///
    /// # Errors
    ///
    /// Everything [`Self::execute_returning`] returns, plus [`Error::QueryReturnedNoRows`] when
    /// the query returns no row at all and [`Error::Sqlite`] when it returns more than one.
    pub async fn execute_returning_one<S>(
        &self,
        sql: S,
        params: Params,
    ) -> Result<crate::Row<'_>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        self.rate_limit_db().await?;

        let mut rows = self.execute_returning(sql, params).await?;
        if rows.is_empty() {
            Err(Error::QueryReturnedNoRows("no rows returned".into()))
        } else if rows.len() > 1 {
            Err(Error::Sqlite(
                format!("cannot map {} rows into one", rows.len()).into(),
            ))
        } else {
            rows.swap_remove(0)
        }
    }

    #[inline]
    pub(crate) async fn execute_returning_req(
        &self,
        sql: Query,
    ) -> Result<RowsOwnedChecked, Error> {
        if let Some(state) = self.is_leader_db_with_state().await {
            let res = state
                .raft_db
                .raft
                .client_write(QueryWrite::ExecuteReturning(sql))
                .await?;
            let resp: Response = res.data;
            match resp {
                Response::ExecuteReturning(res) => res.result,
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.inner
                .tx_client_db
                .send_async(ClientStreamReq::ExecuteReturning(ClientExecutePayload {
                    request_id: self.new_request_id(),
                    sql,
                    ack,
                }))
                .await
                .map_err(|err| Error::Error(err.to_string().into()))?;
            let res = await_channel_response(rx).await??;
            match res {
                ApiStreamResponsePayload::ExecuteReturning(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
