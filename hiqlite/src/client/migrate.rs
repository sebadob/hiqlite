use crate::client::stream::{ClientMigratePayload, ClientStreamReq};
use crate::migration::{Migration, Migrations};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{AppliedMigration, Client, Error, Params, Response};
use rust_embed::RustEmbed;
use tokio::sync::oneshot;
use tracing::{info, warn};

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
        let applied: Vec<AppliedMigration> = self
            .query_map("SELECT * FROM _migrations ORDER BY id ASC", Params::new())
            .await
            .unwrap_or_default();
        let mut migrations = Migrations::build::<T>();

        // At least the beginning of the just built and already applied migrations must match.
        // We can skip already existing ones early, so they are not sent through the Raft each
        // time when a client restarts.
        for (i, migration) in applied.iter().enumerate() {
            match migrations.get(i) {
                None => {
                    warn!(
                        "Found already applied migration {}_{} / {} which does not exist in given \
                        migrations. Nothing to do.",
                        migration.id, migration.name, migration.hash
                    );
                    return Ok(());
                }
                Some(to_migrate) => {
                    if to_migrate.id != migration.id {
                        panic!(
                            "ID mismatch for '{}' between given and already applied migration: {} != {}",
                            to_migrate.name, to_migrate.id, migration.id
                        );
                    }

                    if to_migrate.hash != migration.hash {
                        panic!(
                            "HASH mismatch for '{}' between given and already applied migration: {} != {}",
                            to_migrate.name, to_migrate.hash, migration.hash
                        );
                    }
                }
            }
        }
        if let Some(last_applied) = applied.last() {
            migrations.retain(|m| m.id > last_applied.id);
        }
        if migrations.is_empty() {
            info!("All migrations have been applied already - nothing to migrate");
            return Ok(());
        }

        match self.migrate_execute(migrations).await {
            Ok(_) => Ok(()),
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
        if let Some(state) = self.is_leader_db_with_state().await {
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
                .map_err(|err| Error::Error(err.to_string().into()))?;
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
