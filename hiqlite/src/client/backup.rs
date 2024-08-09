use crate::client::stream::{ClientBackupPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{Client, Error, Response};
use tokio::sync::oneshot;
use tracing::error;

impl Client {
    #[cold]
    pub async fn backup(&self) -> Result<(), Error> {
        match self.backup_execute().await {
            Ok(res) => Ok(res),
            Err(err) => {
                let is_leader = self.is_leader_db().await.is_some();
                error!(
                    "Error during backup: {}\n current leader: {}\nis this leader: {}",
                    err,
                    self.inner.leader_db.read().await.0,
                    is_leader
                );
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    // TODO sometimes the backup task can get stuck here -> leader not updating properly?
                    let is_leader = self.is_leader_db().await.is_some();
                    error!(
                        "was leader error during backup: {}\n current leader: {}\nis this leader: {}",
                        err,
                        self.inner.leader_db.read().await.0,
                        is_leader
                    );
                    self.backup_execute().await
                } else {
                    Err(err)
                }
            }
        }
    }

    #[cold]
    async fn backup_execute(&self) -> Result<(), Error> {
        let current_leader = self.inner.leader_db.read().await.0;

        if let Some(state) = self.is_leader_db().await {
            let res = state
                .raft_db
                .raft
                .client_write(QueryWrite::Backup(current_leader))
                .await?;
            let resp: Response = res.data;
            match resp {
                Response::Backup(res) => res,
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.inner
                .tx_client_db
                .send_async(ClientStreamReq::Backup(ClientBackupPayload {
                    request_id: self.new_request_id(),
                    node_id: current_leader,
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
