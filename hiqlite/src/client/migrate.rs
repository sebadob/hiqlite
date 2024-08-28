use crate::client::stream::{ClientMigratePayload, ClientStreamReq};
use crate::migration::{Migration, Migrations};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{Client, Error, Response};
use rust_embed::RustEmbed;
use tokio::sync::oneshot;

impl Client {
    /// Execute database migrations.
    ///
    /// To make this work, you currently need to add `rust-embed` to your `Cargo.toml`.
    /// Then embed the Migrations into your binary to be able to execute them in a typüe-safe way:
    /// ```rust, notest
    /// #[derive(rust_embed::Embed)]
    /// #[folder = "migrations"]
    /// struct Migrations;
    ///
    /// client.migrate::<Migrations>().await?;
    /// ```
    ///
    /// You might want to take a look at the
    /// [sqlite-only](https://github.com/sebadob/hiqlite/tree/main/examples/sqlite-only) example.
    #[cold]
    pub async fn migrate<T: RustEmbed>(&self) -> Result<(), Error> {
        match self.migrate_execute(Migrations::build::<T>()).await {
            Ok(res) => Ok(res),
            Err(err) => {
                if self
                    .was_leader_update_error(&err, &self.inner.leader_db, &self.inner.tx_client_db)
                    .await
                {
                    self.migrate_execute(Migrations::build::<T>()).await
                } else {
                    Err(err)
                }
            }
        }
    }

    #[cold]
    pub(crate) async fn migrate_execute(&self, migrations: Vec<Migration>) -> Result<(), Error> {
        if let Some(state) = self.is_leader_db().await {
            let res = state
                .raft_db
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
            self.inner
                .tx_client_db
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
