use crate::client::stream::{ClientBackupPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{Client, Error, Response};
use chrono::{NaiveDateTime, Utc};
use tokio::fs;
use tokio::sync::oneshot;
use tracing::{debug, error, warn};

use cryptr::stream::writer::channel_writer::ChannelReceiver;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[derive(Debug)]
pub struct BackupListing {
    pub name: String,
    pub last_modified: i64,
    pub size: Option<u64>,
}

impl Client {
    /// Create an on-demand backup of the SQLite state machine.
    ///
    /// You usually don't need to call this manually, because Hiqlite will automatically run a
    /// cron job every night and push backups to object storage, when you have the `s3` feature
    /// enabled.
    ///
    /// **Note:**
    /// Each raft node will create a backup on local disk, but only the current leader will
    /// encrypt and push it to s3 storage. This is why you will typically only see the leaders
    /// node id inside your bucket and not the other ones.
    ///
    /// The backup will be created in the background and run on other threads. This means it will
    /// not be finished immediately when this function returns.
    #[cold]
    pub async fn backup(&self) -> Result<(), Error> {
        match self.backup_execute().await {
            Ok(res) => Ok(res),
            Err(err) => {
                let is_leader = self.is_leader_db_with_state().await.is_some();
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
                    let is_leader = self.is_leader_db_with_state().await.is_some();
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

        if let Some(state) = self.is_leader_db_with_state().await {
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
                .map_err(|err| Error::Error(err.to_string().into()))?;
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::Backup(res) => res,
                _ => unreachable!(),
            }
        }
    }

    /// Get the file handle to a local backup.
    pub async fn backup_file_local(&self, filename: &str) -> Result<fs::File, Error> {
        if let Some(state) = self.inner.state.clone() {
            let path = format!("{}/{filename}", state.backups_dir);
            let file = fs::File::open(path).await?;
            Ok(file)
        } else {
            Err(Error::Config(
                "Backups cannot be listed for remote clients".into(),
            ))
        }
    }

    pub fn backup_s3_stream(&self, object: String) -> Result<ChannelReceiver, Error> {
        if let Some(state) = self.inner.state.clone() {
            if let Some(s3) = state.s3_config.clone() {
                s3.pull_channel(object)
            } else {
                Err(Error::Config("No S3 bucket configured".into()))
            }
        } else {
            Err(Error::Config(
                "Backups cannot be listed for remote clients".into(),
            ))
        }
    }

    /// List all existing local backups.
    pub async fn backup_list_local(&self) -> Result<Vec<BackupListing>, Error> {
        if let Some(state) = self.inner.state.clone() {
            let mut res = Vec::with_capacity(4);

            let dir = &state.backups_dir;
            let mut list = fs::read_dir(dir).await?;
            while let Some(entry) = list.next_entry().await? {
                let meta = entry.metadata().await?;
                if meta.is_dir() {
                    continue;
                }

                let fname = entry.file_name();
                let name = fname.to_str().unwrap_or_default().to_string();
                if !name.starts_with("backup_node_") {
                    debug!("Found non-backup file: {name}");
                    continue;
                }

                let last_modified: chrono::DateTime<Utc> = meta.modified()?.into();

                #[cfg(unix)]
                let size = Some(meta.size());
                #[cfg(not(unix))]
                let size = None;

                res.push(BackupListing {
                    name,
                    last_modified: last_modified.timestamp(),
                    size,
                });
            }

            Ok(res)
        } else {
            Err(Error::Config(
                "Backups cannot be listed for remote clients".into(),
            ))
        }
    }

    /// List all existing S3 backups.
    pub async fn backup_list_s3(&self) -> Result<Vec<BackupListing>, Error> {
        if let Some(state) = self.inner.state.clone() {
            let mut res = Vec::with_capacity(16);

            if let Some(s3) = state.s3_config.as_ref() {
                let list = s3.bucket.list("", None).await?;
                for bucket in list {
                    if bucket.name != s3.bucket.name {
                        // it's possible that the creds have access to multiple buckets
                        continue;
                    }

                    for obj in bucket.contents {
                        if !obj.key.starts_with("backup_node_") {
                            debug!("Found non-backup file: {}", obj.key);
                            continue;
                        }

                        let s = obj.last_modified.as_str();
                        let last_modified = if s.len() < 19 {
                            warn!("last modified timestamp from S3 too short");
                            0
                        } else {
                            NaiveDateTime::parse_from_str(&s[..19], "%Y-%m-%dT%H:%M:%S")
                                .map_err(|err| {
                                    error!(
                                        "Error parsing timestamp from S3: {}",
                                        obj.last_modified
                                    );
                                    Error::S3(format!(
                                        "Cannot parse last_modified timestamp from S3: {}",
                                        err
                                    ))
                                })?
                                .and_utc()
                                .timestamp()
                        };

                        res.push(BackupListing {
                            name: obj.key,
                            last_modified,
                            size: Some(obj.size),
                        });
                    }
                }
            }

            Ok(res)
        } else {
            Err(Error::Config(
                "Backups cannot be listed for remote clients".into(),
            ))
        }
    }
}
