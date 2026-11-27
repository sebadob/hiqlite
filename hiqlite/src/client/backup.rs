use crate::client::helpers::await_channel_response;
use crate::client::stream::{ClientBackupPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::sqlite::state_machine::QueryWrite;
use crate::{Client, Error, Response};
use chrono::Utc;
use tokio::fs;
use tokio::sync::oneshot;
use tracing::debug;
use tracing::error;

#[cfg(feature = "s3")]
use chrono::NaiveDateTime;
#[cfg(feature = "s3")]
use cryptr::stream::writer::channel_writer::ChannelReceiver;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(feature = "s3")]
use tracing::warn;

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
    ///
    /// # Errors
    ///
    /// [`Error::RateLimit`] when the DB request rate limit is exhausted. Then the [`Error`] of
    /// the backup run itself: a Raft write error, [`Error::Error`] for a backup file the local
    /// disk refuses to write, [`Error::S3`] for a failed upload, and exactly one re-send after
    /// [`Error::LeaderChange`].
    #[cold]
    pub async fn backup(&self) -> Result<(), Error> {
        self.rate_limit_db().await?;

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
                    .was_leader_update_error(
                        &err,
                        &self.inner.leader_db,
                        &self.inner.leader_lookup_db,
                        &self.inner.tx_client_db,
                    )
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
        let ts = Utc::now().timestamp();

        if let Some(state) = self.is_leader_db_with_state().await {
            let res = Self::client_write_local(
                &state.raft_db.raft,
                QueryWrite::Backup((current_leader, ts)),
            )
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
                    ts,
                    ack,
                }))
                .await
                .map_err(|err| Error::Error(err.to_string().into()))?;
            let res = await_channel_response(rx).await??;
            match res {
                ApiStreamResponsePayload::Backup(res) => res,
                _ => unreachable!(),
            }
        }
    }

    /// Get the file handle to a local backup.
    ///
    /// The name must be exactly `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite` - the shape Hiqlite
    /// writes - so a caller cannot reach outside of the backups directory (e.g. via `../` or an
    /// embedded path separator).
    ///
    /// # Errors
    ///
    /// [`Error::BadRequest`] when the name does not match the backup file shape,
    /// [`Error::Config`] for a remote client that has no local node, and [`Error::Error`] when
    /// the file itself cannot be opened.
    pub async fn backup_file_local(&self, filename: &str) -> Result<fs::File, Error> {
        if let Some(state) = self.inner.state.clone() {
            if !valid_backup_filename(&state.backup_file_prefix, filename) {
                return Err(Error::BadRequest(
                    format!(
                        "invalid backup filename: {filename} - expected \
                        {}_{{node_id}}_{{unix_ts}}.sqlite",
                        state.backup_file_prefix
                    )
                    .into(),
                ));
            }

            let path = format!("{}/{filename}", state.backups_dir);
            let file = fs::File::open(path).await?;
            Ok(file)
        } else {
            Err(Error::Config(
                "Backups cannot be listed for remote clients".into(),
            ))
        }
    }

    ///
    /// # Errors
    ///
    /// [`Error::Config`] when no S3 bucket is configured, or when this client is remote and has
    /// no local node.
    #[cfg(feature = "s3")]
    pub fn backup_s3_stream(&self, object: String) -> Result<ChannelReceiver, Error> {
        if let Some(state) = self.inner.state.clone() {
            if let Some(s3) = state.s3_config.clone() {
                Ok(s3.pull_channel(object))
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
    ///
    /// Only files with the exact `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite` shape are listed -
    /// the in-flight temp file and anything a shared directory may hold are not.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client that has no local node, and [`Error::Error`] when
    /// the backups directory cannot be read.
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
                if !valid_backup_filename(&state.backup_file_prefix, &name) {
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
    ///
    /// Only objects with the exact `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite` shape are listed,
    /// so a shared bucket reports your cluster's backups and nothing else.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client that has no local node, and [`Error::S3`] when the
    /// bucket listing fails or answers with a `last_modified` that cannot be parsed.
    #[cfg(feature = "s3")]
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
                        if !valid_backup_filename(&state.backup_file_prefix, &obj.key) {
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
                                        "Cannot parse last_modified timestamp from S3: {err}"
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

/// Backups are always created as `{prefix}_{node_id}_{ts}.sqlite` directly in the flat backups
/// dir; only that exact shape is accepted, so a caller cannot reach outside of it (e.g. via "../"
/// or an embedded path separator).
fn valid_backup_filename(prefix: &str, filename: &str) -> bool {
    let Some(rest) = filename.strip_prefix(prefix) else {
        return false;
    };
    let Some(stem) = rest
        .strip_prefix('_')
        .and_then(|s| s.strip_suffix(".sqlite"))
    else {
        return false;
    };
    // `stem` is `{node_id}_{ts}`; both must be plain integers, which also rules out any embedded
    // path separator.
    let Some((node_id, ts)) = stem.rsplit_once('_') else {
        return false;
    };
    node_id.parse::<u64>().is_ok() && ts.parse::<i64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::valid_backup_filename;

    #[test]
    fn valid_backup_filenames_are_accepted() {
        assert!(valid_backup_filename(
            "backup_node",
            "backup_node_1_1700000000.sqlite"
        ));
        assert!(valid_backup_filename(
            "backup_node",
            "backup_node_42_1700000000.sqlite"
        ));
    }

    #[test]
    fn a_custom_prefix_only_accepts_its_own_files() {
        assert!(valid_backup_filename("app-a", "app-a_1_1700000000.sqlite"));
        // the default prefix is not a prefix of a custom one and the other way round
        assert!(!valid_backup_filename(
            "app-a",
            "backup_node_1_1700000000.sqlite"
        ));
        assert!(!valid_backup_filename(
            "backup_node",
            "app-a_1_1700000000.sqlite"
        ));
        // a longer prefix that starts with ours must not match
        assert!(!valid_backup_filename("app", "app-a_1_1700000000.sqlite"));
    }

    #[test]
    fn invalid_backup_filenames_are_rejected() {
        for name in [
            "",
            "backup_node_1",
            "backup_node_1_.sqlite",
            "backup_node_1_1700000000.sqlite~",
            "backup_node_1_1700000000.sqlite.gz",
            ".sqlite",
            "../backup_node_1_1700000000.sqlite",
            "backup_node_1_1700000000.sqlite/",
            "backup_node_1/../../etc/passwd",
            "backup_node_1\\..\\..\\windows",
            "node_backup_1_1700000000.sqlite",
        ] {
            assert!(
                !valid_backup_filename("backup_node", name),
                "should reject: {name:?}"
            );
        }
    }
}
