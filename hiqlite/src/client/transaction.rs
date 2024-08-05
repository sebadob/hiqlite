use crate::client::stream::{ClientStreamReq, ClientTransactionPayload};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::{Query, QueryWrite};
use crate::{Client, Error, Params, Response};
use std::borrow::Cow;
use tokio::sync::oneshot;

impl Client {
    /// Takes multiple queries and executes all of them in a single transaction.
    pub async fn txn<C, Q>(&self, sql: Q) -> Result<Vec<Result<usize, Error>>, Error>
    where
        Q: IntoIterator<Item = (C, Params)>,
        C: Into<Cow<'static, str>>,
    {
        let queries: Vec<Query> = sql
            .into_iter()
            .map(|(q, params)| Query {
                sql: q.into(),
                params,
            })
            .collect();

        match self.txn_execute(queries.clone()).await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    self.txn_execute(queries).await
                } else {
                    Err(err)
                }
            }
        }
    }

    #[inline(always)]
    async fn txn_execute(&self, queries: Vec<Query>) -> Result<Vec<Result<usize, Error>>, Error> {
        if let Some(state) = self.is_leader_db().await {
            let res = state
                .raft_db
                .raft
                .client_write(QueryWrite::Transaction(queries))
                .await?;
            let resp: Response = res.data;
            match resp {
                Response::Transaction(res) => res,
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.inner
                .tx_client_db
                .send_async(ClientStreamReq::Transaction(ClientTransactionPayload {
                    request_id: self.new_request_id(),
                    queries,
                    ack,
                }))
                .await
                .expect("Client Stream Manager to always be running");
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::Transaction(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
