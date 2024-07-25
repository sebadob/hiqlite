use crate::client_stream::{ClientBackupPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{DbClient, Error, Response};
use tokio::sync::oneshot;

impl DbClient {
    #[cold]
    pub async fn backup(&self) -> Result<(), Error> {
        match self.backup_execute().await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self.was_leader_update_error(&err).await {
                    self.backup_execute().await
                } else {
                    Err(err)
                }
            }
        }
    }

    #[cold]
    async fn backup_execute(&self) -> Result<(), Error> {
        if let Some(state) = self.is_this_local_leader().await {
            let res = state.raft.client_write(QueryWrite::Backup).await?;
            let resp: Response = res.data;
            match resp {
                Response::Backup(res) => res,
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.tx_client
                .send_async(ClientStreamReq::Backup(ClientBackupPayload {
                    request_id: self.new_request_id(),
                    ack,
                }))
                .await
                .expect("Client Stream Manager to always be running");
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::Backup(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
