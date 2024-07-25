use crate::client_stream::{ClientMigratePayload, ClientStreamReq};
use crate::migration::{Migration, Migrations};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{DbClient, Error, Response};
use rust_embed::RustEmbed;
use tokio::sync::oneshot;

impl DbClient {
    #[cold]
    pub async fn migrate<T: RustEmbed>(&self) -> Result<(), Error> {
        match self.migrate_execute(Migrations::build::<T>()).await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self.was_leader_update_error(&err).await {
                    self.migrate_execute(Migrations::build::<T>()).await
                } else {
                    Err(err)
                }
            }
        }
    }

    #[cold]
    async fn migrate_execute(&self, migrations: Vec<Migration>) -> Result<(), Error> {
        if let Some(state) = self.is_this_local_leader().await {
            let res = state
                .raft
                .client_write(QueryWrite::Migration(migrations))
                .await?;
            let resp: Response = res.data;
            match resp {
                Response::Migrate(res) => res,
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();
            self.tx_client
                .send_async(ClientStreamReq::Migrate(ClientMigratePayload {
                    request_id: self.new_request_id(),
                    migrations,
                    ack,
                }))
                .await
                .expect("Client Stream Manager to always be running");
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::Migrate(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
