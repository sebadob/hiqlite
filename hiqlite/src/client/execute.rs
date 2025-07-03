use crate::client::stream::{ClientExecutePayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::RowOwned;
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
    pub async fn execute<S>(&self, sql: S, params: Params) -> Result<usize, Error>
    where
        S: Into<Cow<'static, str>>,
    {
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

    #[inline(always)]
    async fn execute_req(&self, sql: Query) -> Result<usize, Error> {
        if let Some(state) = self.is_leader_db_with_state().await {
            let res = state
                .raft_db
                .raft
                .client_write(QueryWrite::Execute(sql))
                .await?;
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
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::Execute(res) => res,
                _ => unreachable!(),
            }
        }
    }

    /// Execute a query on the database that includes a `RETURNING` statement.
    ///
    /// Returns the rows mapped to the output type on success. This only works for types that
    /// `impl<'r> From<hiqlite::Row<'r>>`
    pub async fn execute_returning_map<S, T>(
        &self,
        sql: S,
        params: Params,
    ) -> Result<Vec<Result<T, Error>>, Error>
    where
        S: Into<Cow<'static, str>>,
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
    {
        let rows: Vec<Result<crate::Row, Error>> = self.execute_returning::<S>(sql, params).await?;
        let mut res: Vec<Result<T, Error>> = Vec::with_capacity(rows.len());
        for row in rows {
            res.push(row.map(T::from))
        }
        Ok(res)
    }

    /// Execute a query on the database that includes a `RETURNING` statement.
    ///
    /// Returns the row mapped to the output type on success. This only works for types that
    /// `impl<'r> From<hiqlite::Row<'r>>`.
    ///
    /// Throws an error if not exactly 1 row has been returned.
    pub async fn execute_returning_map_one<S, T>(&self, sql: S, params: Params) -> Result<T, Error>
    where
        S: Into<Cow<'static, str>>,
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
    {
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
    pub async fn execute_returning<S>(
        &self,
        sql: S,
        params: Params,
    ) -> Result<Vec<Result<crate::Row<'_>, Error>>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
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

        let mut res: Vec<Result<crate::Row, Error>> = Vec::with_capacity(rows.len());
        for row in rows {
            res.push(row.map(crate::Row::Owned))
        }
        Ok(res)
    }

    /// Execute a query on the database that includes a `RETURNING` statement.
    ///
    /// Returns a single raw row. Will throw an error if rows returned is not exactly 1.
    pub async fn execute_returning_one<S>(
        &self,
        sql: S,
        params: Params,
    ) -> Result<crate::Row<'_>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
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
    ) -> Result<Vec<Result<RowOwned, Error>>, Error> {
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
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::ExecuteReturning(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
