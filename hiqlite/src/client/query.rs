use crate::client::stream::{ClientQueryPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::RowOwned;
use crate::store::state_machine::sqlite::state_machine::Query;
use crate::{query, Client, Error, Params};
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

    /// This is the most efficient and fastest way to query data from sqlite into a struct.
    /// It is mandatory, that the struct implements `From<Row<'_>>` for this to work.
    /// If you want a more comfortable and easier way and don't need the most efficiency and
    /// speed, take a look at `.query_as()`.
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
    /// Errors if no rows are returned and ignores additional results if more than one row returned.
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

    /// Converts data returned from a sql query into a struct. This struct must derive
    /// serde::Deserialize. This is the easiest and most straight forward way of doing it, but not
    /// the fastest and most efficient one. If you want to optimize memory and speed, you should
    /// use `.query_map()`.
    ///
    /// Note: This does not work for remote-only clients
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
    /// Errors if no rows are returned and ignores additional results if more than one row returned.
    ///
    /// Note: This does not work for remote-only clients
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

    async fn query_remote<S>(
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
        //
        // let rows = match self.query_remote_req(query.clone(), consistent).await {
        //     Ok(res) => res,
        //     Err(err) => {
        //         if self.was_leader_update_error(&err).await {
        //             self.query_remote_req(query, consistent).await?
        //         } else {
        //             return Err(err);
        //         }
        //     }
        // };

        // let mut res: Vec<crate::Row> = Vec::with_capacity(rows.len());
        // for row in rows {
        //     res.push(crate::Row::Owned(row))
        // }
        // Ok(res)
        let res = match self.query_remote_req(query.clone(), consistent).await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self.was_leader_update_error(&err).await {
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

    async fn query_remote_req(
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
            .tx_client
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
