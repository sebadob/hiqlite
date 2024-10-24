use crate::client::stream::{ClientQueryPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::RowOwned;
use crate::store::state_machine::sqlite::state_machine::Query;
use crate::{query, Client, Error, Params, Row};
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use tokio::sync::oneshot;

impl Client {
    /// Execute a consistent query. This query will run on the leader node only and pause Raft
    /// replication at a point, where all "current" logs have been applied to at least a quorum
    /// of all nodes. This means whatever result this query returns, at least hals of the nodes + 1
    /// will have the exact same result and it will be the same even if you would end up in a
    /// network segmentation and loose half of your data directly afterward.
    ///
    /// This query is very expensive compared to the other ones. It needs network roud-trips, pauses
    /// the raft and allocates a lot more memory, because it is working with owned data rather than
    /// with borrowed local one for quick mapping.
    /// You should only use it, if you really need to.
    pub async fn query_consistent<S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<Vec<crate::Row>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        self.query_remote::<S>(stmt, params, true).await
    }

    /// Execute a consistent query. This query will run on the leader node only and pause Raft
    /// replication at a point, where all "current" logs have been applied to at least a quorum
    /// of all nodes. This means whatever result this query returns, at least hals of the nodes + 1
    /// will have the exact same result, and it will be the same even if you would end up in a
    /// network segmentation and loose half of your data directly afterward.
    ///
    /// This query is very expensive compared to the other ones. It needs network round-trips, pauses
    /// the raft and allocates a lot more memory, because it is working with owned data rather than
    /// with borrowed local one for quick mapping.
    /// You should only use it, if you really need to.
    pub async fn query_consistent_map<T, S>(&self, stmt: S, params: Params) -> Result<Vec<T>, Error>
    where
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        Ok(self
            .query_remote(stmt, params, true)
            .await?
            .into_iter()
            .map(T::from)
            .collect())
    }

    /// Query data from the database and map it to the given `struct`.
    ///
    /// The `struct` must implement `impl<'r> From<hiqlite::Row<'r>>` for this to work:
    ///
    /// ```rust, notest
    /// #[derive(Debug)]
    /// struct MyStruct {
    ///     pub id: String,
    ///     pub num: i64,
    ///     pub description: Option<String>,
    /// }
    ///
    /// impl<'r> From<Row<'r>> for MyStruct {
    ///     fn from(mut row: Row<'r>) -> Self {
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
    /// ```rust, notest
    /// let res: Vec<MyStruct> = client
    ///     .query_map("SELECT * FROM test", params!())
    ///     .await?;
    /// ```
    pub async fn query_map<T, S>(&self, stmt: S, params: Params) -> Result<Vec<T>, Error>
    where
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_map(state, stmt, params).await
        } else {
            Ok(self
                .query_remote(stmt, params, false)
                .await?
                .into_iter()
                .map(T::from)
                .collect())
        }
    }

    /// Works in the same way as `query_map()`, but returns only one result.
    ///
    /// Errors if no rows are returned and ignores additional results if more than one row returned.
    ///
    /// ```rust, notest
    /// let res: MyStruct = client
    ///     .query_map_one("SELECT * FROM test WHERE id = $1", params!("id1"))
    ///     .await?;
    /// ```
    pub async fn query_map_one<T, S>(&self, stmt: S, params: Params) -> Result<T, Error>
    where
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_map_one(state, stmt, params).await
        } else {
            let mut rows = self.query_remote(stmt, params, false).await?;
            if rows.is_empty() {
                return Err(Error::Sqlite("No rows returned".into()));
            }
            Ok(T::from(rows.swap_remove(0)))
        }
    }

    /// Converts data returned from a sql query into a struct which derives `serde::Deserialize`.
    ///
    /// This is the easiest and most straight forward way of getting data. This is most often the
    /// fasted way of mapping values while needing a little bit more memory.
    ///
    /// ```rust, notest
    /// let res: Vec<Entity> = client
    ///     .query_as("SELECT * FROM test", params!())
    ///     .await?;
    /// ```
    ///
    /// **Note:**
    /// This works for local clients only, not for `hiqlite::Client::remote()` or `query_consistent`.
    pub async fn query_as<T, S>(&self, stmt: S, params: Params) -> Result<Vec<T>, Error>
    where
        T: DeserializeOwned + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_as(state, stmt, params).await
        } else {
            Err(Error::Config("`query_as()` only works for local clients, you need to use `query_map()` for remote".into()))
        }
    }

    /// Works in the same way as `query_as()`, but returns only one result.
    ///
    /// Errors if no rows are returned and ignores additional results if more than one row returned.
    pub async fn query_as_one<T, S>(&self, stmt: S, params: Params) -> Result<T, Error>
    where
        T: DeserializeOwned + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            query::query_as_one(state, stmt, params).await
        } else {
            Err(Error::Config("`query_as()` only works for local clients, you need to use `query_map()` for remote".into()))
        }
    }

    /// A raw query will return the bare `Row` without doing any deserialization or mapping.
    /// This can be useful if you just need to know if a query succeeds, or if you need to manually
    /// work with the result without being able to convert it into a type.
    pub async fn query_raw<S>(&self, stmt: S, params: Params) -> Result<Vec<crate::Row>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.inner.state {
            let rows = query::query_owned_local(
                state.raft_db.log_statements,
                state.raft_db.read_pool.clone(),
                stmt,
                params,
            )
            .await?;
            Ok(rows.into_iter().map(Row::Owned).collect())
        } else {
            self.query_remote(stmt, params, false).await
        }
    }

    /// A raw query will return the bare `Row` without doing any deserialization or mapping.
    ///
    /// This version works the same as `query_raw()` with the exception that it will return an
    /// error, if no rows have been returned. This eliminates the need for manual `is_empty()` checks
    /// each time.
    pub async fn query_raw_not_empty<S>(
        &self,
        stmt: S,
        params: Params,
    ) -> Result<Vec<crate::Row>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        let rows = self.query_raw(stmt, params).await?;
        if rows.is_empty() {
            Err(Error::Sqlite("No rows returned".into()))
        } else {
            Ok(rows)
        }
    }

    /// A raw query will return the bare `Row` without doing any deserialization or mapping.
    ///
    /// This version will return exactly one `Row`. It will error if none has been returned from
    /// the database and will ignore any rows other than the first one.
    pub async fn query_raw_one<S>(&self, stmt: S, params: Params) -> Result<crate::Row, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        let mut rows = self.query_raw(stmt, params).await?;
        if rows.is_empty() {
            Err(Error::Sqlite("No rows returned".into()))
        } else {
            Ok(rows.swap_remove(0))
        }
    }

    /// Executes a query on remote host and returns raw rows.
    /// This is mostly used internally and not directly.
    pub(crate) async fn query_remote<S>(
        &self,
        stmt: S,
        params: Params,
        consistent: bool,
    ) -> Result<Vec<crate::Row>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        let query = Query {
            sql: stmt.into(),
            params,
        };

        let res = match self.query_remote_req(query.clone(), consistent).await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    self.query_remote_req(query, consistent).await
                } else {
                    return Err(err);
                }
            }
        }?
        .into_iter()
        .map(crate::Row::Owned)
        .collect();

        Ok(res)
    }

    pub(crate) async fn query_remote_req(
        &self,
        query: Query,
        consistent: bool,
    ) -> Result<Vec<RowOwned>, Error> {
        let (ack, rx) = oneshot::channel();

        let payload = if consistent {
            ClientStreamReq::QueryConsistent(ClientQueryPayload {
                request_id: self.new_request_id(),
                ack,
                query,
            })
        } else {
            ClientStreamReq::Query(ClientQueryPayload {
                request_id: self.new_request_id(),
                ack,
                query,
            })
        };

        self.inner
            .tx_client_db
            .send_async(payload)
            .await
            .expect("Client Stream Manager to always be running");
        let res = rx
            .await
            .expect("To always receive an answer from Client Stream Manager")?;
        match res {
            ApiStreamResponsePayload::Query(res) => {
                assert!(!consistent);
                res
            }
            ApiStreamResponsePayload::QueryConsistent(res) => {
                assert!(consistent);
                res
            }
            _ => unreachable!(),
        }
    }
}
