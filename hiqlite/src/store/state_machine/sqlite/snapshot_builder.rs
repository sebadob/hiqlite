use crate::helpers::deserialize_serde;
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::store::state_machine::sqlite::state_machine::{StateMachineData, StateMachineSqlite};
use crate::store::state_machine::sqlite::writer::{SnapshotRequest, WriterRequest};
use openraft::RaftSnapshotBuilder;
use openraft::storage::Snapshot;
use openraft::type_config::alias::{SnapshotMetaOf, SnapshotOf};
use std::io;
use tokio::sync::oneshot;
use tokio::{fs, task};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SQLiteSnapshotBuilder {
    // pub last_applied_log_id: Option<LogId<NodeId>>,
    // pub last_membership: StoredMembership<NodeId, Node>,
    #[cfg(feature = "backup")]
    pub path_backups: String,
    pub path_snapshots: String,
    pub write_tx: flume::Sender<WriterRequest>,
}

impl RaftSnapshotBuilder<TypeConfigSqlite> for SQLiteSnapshotBuilder {
    type SnapshotData = tokio::fs::File;

    #[tracing::instrument(level = "trace", skip(self))]
    async fn build_snapshot(
        &mut self,
    ) -> Result<SnapshotOf<TypeConfigSqlite, Self::SnapshotData>, io::Error> {
        // - build new snapshot id
        // - make sure target path exists
        // - send snapshot request to db writer
        // - await vaccuum response
        // - open db snapshot file
        // - return snapshot handle

        let snapshot_id = Uuid::now_v7();

        // No need to do a temp file + atomic rename here. The write task does that already.
        let path = format!("{}/{}", self.path_snapshots, snapshot_id);
        let (ack, rx) = oneshot::channel();
        let req = WriterRequest::Snapshot(SnapshotRequest {
            snapshot_id,
            path: path.clone(),
            ack,
        });
        self.write_tx
            .send_async(req)
            .await
            .expect("Sender to always be listening");

        let resp = rx.await.expect("to always receive a snapshot response")?;
        let snapshot = fs::File::open(path).await?;

        let path_snapshots = self.path_snapshots.clone();
        #[cfg(feature = "backup")]
        let path_backups = self.path_backups.clone();
        // cleanup can easily happen in the background
        task::spawn(snapshots_cleanup(
            path_snapshots,
            #[cfg(feature = "backup")]
            path_backups,
            snapshot_id,
        ));

        let snapshot = Snapshot {
            meta: SnapshotMetaOf::<TypeConfigSqlite> {
                last_log_id: resp.meta.last_applied_log_id,
                last_membership: resp.meta.last_membership,
            },
            snapshot,
        };

        Ok(snapshot)
    }
}

/// How many published snapshots the cleanup after a build keeps: the one just built and the one
/// before it.
///
/// The older one is what a rebuild of the database falls back to when the newest one is not
/// usable. That fallback only works while the log still holds every entry after the older
/// snapshot, which `max_in_snapshot_log_to_keep` decides, so keeping more than one older snapshot
/// would only cost disk space.
const SNAPSHOTS_KEEP: usize = 2;

/// Removes everything from the snapshots folder except the newest `SNAPSHOTS_KEEP` published
/// snapshots, and always `keep_id`, the one just built.
async fn snapshots_cleanup(
    path_snapshots: String,
    #[cfg(feature = "backup")] path_backups: String,
    keep_id: Uuid,
) -> Result<(), io::Error> {
    let mut keep = StateMachineSqlite::published_snapshots(&path_snapshots)
        .await?
        .into_iter()
        .take(SNAPSHOTS_KEEP)
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let keep_id = keep_id.to_string();
    if !keep.contains(&keep_id) {
        keep.push(keep_id);
    }

    let mut list = tokio::fs::read_dir(&path_snapshots).await?;
    let mut deletes = Vec::new();
    loop {
        let entry = match list.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(err) => {
                warn!("Error reading directory entries: {err:?}");
                break;
            }
        };
        let file_name = entry.file_name();
        let name = file_name.to_str().unwrap_or("UNKNOWN");

        let meta = entry.metadata().await?;

        // we only expect sub-dirs in the snapshot dir
        if meta.is_dir() {
            warn!("Invalid folder in snapshots dir: {name}");
            continue;
        }

        if !keep.iter().any(|k| k == name) {
            deletes.push(name.to_string());
        }
    }

    #[cfg(feature = "backup")]
    {
        debug!("Cleaning up possibly existing old backup restore files");
        let restore_path = format!("{}/{}", path_backups, crate::backup::BACKUP_DB_NAME);
        let _ = fs::remove_file(&restore_path).await;
    }

    for file_name in deletes {
        let path = format!("{path_snapshots}/{file_name}");
        if let Err(err) = fs::remove_file(path).await {
            error!("Error removing old snapshot {file_name}: {err}");
        }
    }

    Ok(())
}
