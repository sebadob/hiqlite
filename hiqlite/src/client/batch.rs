use crate::client::stream::{ClientBatchPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{Client, Error, Response};
use std::borrow::Cow;
use tokio::sync::oneshot;

impl Client {
    /// Takes an arbitrary SQL String with multiple queries and executes all of them as a batch
    pub async fn batch<S>(&self, sql: S) -> Result<Vec<Result<usize, Error>>, Error>
    where
        S: Into<Cow<'static, str>>,
    {
        let sql = sql.into();
        match self.batch_execute(sql.clone()).await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    self.batch_execute(sql).await
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn batch_execute(
        &self,
        sql: Cow<'static, str>,
    ) -> Result<Vec<Result<usize, Error>>, Error> {
        if let Some(state) = self.is_leader_db().await {
            let res = state
                .raft_db
                .raft
                .client_write(QueryWrite::Batch(sql))
                .await?;
            let resp: Response = res.data;
            match resp {
                Response::Batch(res) => Ok(res.result),
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.inner
                .tx_client_db
                .send_async(ClientStreamReq::Batch(ClientBatchPayload {
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
                ApiStreamResponsePayload::Batch(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
