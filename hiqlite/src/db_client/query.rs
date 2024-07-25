use crate::db_client::stream::{ClientQueryConsistentPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::RowOwned;
use crate::store::state_machine::sqlite::state_machine::Query;
use crate::{query, DbClient, Error, Params};
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use tokio::sync::oneshot;

impl DbClient {
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
    pub async fn query_map_consistent<T, S>(&self, stmt: S, params: Params) -> Result<Vec<T>, Error>
    where
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        let rows: Vec<crate::Row> = self.query_consistent::<S>(stmt, params).await?;
        let mut res: Vec<T> = Vec::with_capacity(rows.len());
        for row in rows {
            res.push(T::from(row))
        }
        Ok(res)
    }

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
        let query = Query {
            sql: stmt.into(),
            params,
        };

        let rows = match self.query_consistent_req(query.clone()).await {
            Ok(res) => res,
            Err(err) => {
                if self.was_leader_update_error(&err).await {
                    self.query_consistent_req(query).await?
                } else {
                    return Err(err);
                }
            }
        };

        let mut res: Vec<crate::Row> = Vec::with_capacity(rows.len());
        for row in rows {
            res.push(crate::Row::Owned(row))
        }
        Ok(res)
    }

    async fn query_consistent_req(&self, query: Query) -> Result<Vec<RowOwned>, Error> {
        if let Some(state) = self.is_this_local_leader().await {
            query::query_consistent_local(
                &state.raft,
                state.log_statements,
                state.read_pool.clone(),
                query.sql,
                query.params,
            )
            .await
        } else {
            let (ack, rx) = oneshot::channel();
            self.tx_client
                .send_async(ClientStreamReq::QueryConsistent(
                    ClientQueryConsistentPayload {
                        request_id: self.new_request_id(),
                        ack,
                        query,
                    },
                ))
                .await
                .expect("Client Stream Manager to always be running");
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::QueryConsistent(res) => res,
                _ => unreachable!(),
            }
        }
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
        if let Some(state) = &self.state {
            query::query_map(state, stmt, params).await
        } else {
            todo!("query_map for remote clients")
        }
    }

    /// Works in the same way as `query_map()`, but returns only one result.
    /// Errors if no rows are returned and ignores additional results if more than one row returned.
    pub async fn query_map_one<T, S>(&self, stmt: S, params: Params) -> Result<T, Error>
    where
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.state {
            query::query_map_one(state, stmt, params).await
        } else {
            todo!("query_map_one for remote clients")
        }
    }

    /// Converts data returned from a sql query into a struct. This struct must derive
    /// serde::Deserialize. This is the easiest and most straight forward way of doing it, but not
    /// the fastest and most efficient one. If you want to optimize memory and speed, you should
    /// use `.query_map()`.
    pub async fn query_as<T, S>(&self, stmt: S, params: Params) -> Result<Vec<T>, Error>
    where
        T: DeserializeOwned + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.state {
            query::query_as(state, stmt, params).await
        } else {
            todo!("query_as for remote clients")
        }
    }

    /// Works in the same way as `query_as()`, but returns only one result.
    /// Errors if no rows are returned and ignores additional results if more than one row returned.
    pub async fn query_as_one<T, S>(&self, stmt: S, params: Params) -> Result<T, Error>
    where
        T: DeserializeOwned + Send + 'static,
        S: Into<Cow<'static, str>>,
    {
        if let Some(state) = &self.state {
            query::query_as_one(state, stmt, params).await
        } else {
            todo!("query_as_one for remote clients")
        }
    }
}
