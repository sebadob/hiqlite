use crate::db_client::stream::{ClientExecutePayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::query::rows::RowOwned;
use crate::store::state_machine::sqlite::state_machine::{Query, QueryWrite};
use crate::{DbClient, Error, Params, Response};
use std::borrow::Cow;
use tokio::sync::oneshot;

impl DbClient {
    /// `EXECUTE` a modifying query
    ///
    /// This method may return stale value because it does not force to read on a legal leader.
    /// TODO maybe convert these params into borrowed ones because of cloning needed anyway?
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
                if self.was_leader_update_error(&err).await {
                    self.execute_req(sql).await
                } else {
                    Err(err)
                }
            }
        }
    }

    #[inline(always)]
    async fn execute_req(&self, sql: Query) -> Result<usize, Error> {
        if let Some(state) = self.is_this_local_leader().await {
            let res = state.raft.client_write(QueryWrite::Execute(sql)).await?;
            let resp: Response = res.data;
            match resp {
                Response::Execute(res) => res.result,
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.tx_client
                .send_async(ClientStreamReq::Execute(ClientExecutePayload {
                    request_id: self.new_request_id(),
                    sql,
                    ack,
                }))
                .await
                .expect("Client Stream Manager to always be running");
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::Execute(res) => res,
                _ => unreachable!(),
            }
        }
    }

    pub async fn execute_returning_map<S, T>(&self, sql: S, params: Params) -> Result<Vec<T>, Error>
    where
        S: Into<Cow<'static, str>>,
        T: for<'r> From<crate::Row<'r>> + Send + 'static,
    {
        let rows: Vec<crate::Row> = self.execute_returning::<S>(sql, params).await?;
        let mut res: Vec<T> = Vec::with_capacity(rows.len());
        for row in rows {
            res.push(T::from(row))
        }
        Ok(res)
    }

    pub async fn execute_returning<S>(
        &self,
        sql: S,
        params: Params,
    ) -> Result<Vec<crate::Row>, Error>
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
                if self.was_leader_update_error(&err).await {
                    self.execute_returning_req(sql).await?
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

    #[inline]
    async fn execute_returning_req(&self, sql: Query) -> Result<Vec<RowOwned>, Error> {
        if let Some(state) = self.is_this_local_leader().await {
            let res = state
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
            self.tx_client
                .send_async(ClientStreamReq::ExecuteReturning(ClientExecutePayload {
                    request_id: self.new_request_id(),
                    sql,
                    ack,
                }))
                .await
                .expect("Client Stream Manager to always be running");
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
