use crate::store::state_machine::sqlite::writer::{SnapshotRequest, WriterRequest};
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::{Node, NodeId};
use openraft::{
    RaftSnapshotBuilder, Snapshot, SnapshotMeta, StorageError, StorageIOError, StoredMembership,
};
use tokio::sync::oneshot;
use tokio::{fs, task};
use tracing::{debug, error, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SQLiteSnapshotBuilder {
    // pub last_applied_log_id: Option<LogId<NodeId>>,
    // pub last_membership: StoredMembership<NodeId, Node>,
    pub path_snapshots: String,
    pub write_tx: flume::Sender<WriterRequest>,
}

impl RaftSnapshotBuilder<TypeConfigSqlite> for SQLiteSnapshotBuilder {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigSqlite>, StorageError<NodeId>> {
        // - build new snapshot id
        // - make sure target path exists
        // - send snapshot request to db writer
        // - await vaccuum response
        // - open db snapshot file
        // - return snapshot handle

        let snapshot_id = Uuid::now_v7();
        fs::create_dir_all(&self.path_snapshots)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;

        let path = format!("{}/{}", self.path_snapshots, snapshot_id);
        let (ack, rx) = oneshot::channel();
        let req = WriterRequest::Snapshot(SnapshotRequest {
            snapshot_id,
            // last_membership: self.last_membership.clone(),
            path: path.clone(),
            ack,
        });
        self.write_tx
            .send_async(req)
            .await
            .expect("Sender to always be listening");

        let resp = rx.await.expect("to always receive a snapshot response")?;
        let snapshot = fs::File::open(path).await.map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;

        let path_snapshots = self.path_snapshots.clone();
        // cleanup can easily happen in the background
        task::spawn(snapshots_cleanup(path_snapshots, snapshot_id));

        Ok(Snapshot {
            meta: SnapshotMeta {
                last_log_id: resp.meta.last_applied_log_id,
                last_membership: resp.meta.last_membership,
                snapshot_id: snapshot_id.to_string(),
            },
            snapshot: Box::new(snapshot),
        })
    }
}

async fn snapshots_cleanup(
    path_snapshots: String,
    keep_id: Uuid,
) -> Result<(), StorageError<NodeId>> {
    let mut list = tokio::fs::read_dir(&path_snapshots)
        .await
        .map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;

    let keep_id = keep_id.to_string();
    let mut deletes = Vec::new();
    while let Ok(Some(entry)) = list.next_entry().await {
        let file_name = entry.file_name();
        let name = file_name.to_str().unwrap_or("UNKNOWN");

        let meta = entry.metadata().await.map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;

        // we only expect sub-dirs in the snapshot dir
        if meta.is_dir() {
            warn!("Invalid folder in snapshots dir: {}", name);
            continue;
        }

        if name != keep_id {
            deletes.push(name.to_string());
        }
    }

    for file_name in deletes {
        let path = format!("{}/{}", path_snapshots, file_name);
        if let Err(err) = fs::remove_file(path).await {
            error!("Error removing old snapshot {}: {}", file_name, err);
        }
    }

    Ok(())
}
