use crate::app_state::AppState;
use crate::store::logs;
use crate::store::state_machine::sqlite::state_machine::{
    PathBackups, PathDb, PathLockFile, PathSnapshots, QueryWrite, StateMachineData,
    StateMachineSqlite,
};
use crate::{Error, NodeConfig};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio::{fs, task, time};
use tracing::{debug, error, info, warn};

pub const BACKUP_DB_NAME: &str = "restore.sqlite";

/// Check if the env var `HIQLITE_BACKUP_RESTORE` is set and restores the given backup if so.
/// Returns `Ok(true)` if backup has been applied.
/// This will only run if the current node ID is `1`.
pub(crate) async fn check_restore_apply(node_config: &NodeConfig) -> Result<bool, Error> {
    if node_config.node_id == 1 {
        if let Ok(name) = env::var("HIQLITE_BACKUP_RESTORE") {
            warn!(
                "Found HIQLITE_BACKUP_RESTORE={} - starting restore process",
                name
            );
            restore_backup(node_config, &name).await?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Apply the given backup from S3 storage.
///
/// **CAUTION: This function MUST BE CALLED when the Raft is not running!**
///
/// You only need to invoke this manually if you want to apply a backup in another
/// way than with the `HIQLITE_BACKUP_RESTORE` env var, which is being done automatically.
pub async fn restore_backup(node_config: &NodeConfig, backup_name: &str) -> Result<(), Error> {
    let s3_config = match &node_config.s3_config {
        None => {
            return Err(Error::S3(
                "No `S3Config` given, cannot restore backup".to_string(),
            ));
        }
        Some(c) => c,
    };

    info!("Starting database restore from backup {}", backup_name);

    // let path_base = StateMachineSqlite::path_base(&node_config.data_dir);
    let (
        PathDb(path_db),
        PathBackups(path_backups),
        PathSnapshots(path_snapshots),
        PathLockFile(path_lock_file),
    ) = StateMachineSqlite::build_folders(&node_config.data_dir, false).await;
    let path_logs = logs::logs_dir(&node_config.data_dir);

    fs::create_dir_all(&path_backups).await?;
    let path_backup_s3 = format!("{}/{}", path_backups, BACKUP_DB_NAME);
    s3_config.pull(backup_name, &path_backup_s3).await?;

    is_metadata_ok(path_backup_s3.clone()).await?;
    debug!("Database backup metadata is ok");

    debug!("Removing old data");
    let _ = fs::remove_dir_all(&path_db).await;
    // let _ = fs::remove_dir_all(&path_backups).await;
    let _ = fs::remove_dir_all(&path_snapshots).await;
    let _ = fs::remove_dir_all(&path_lock_file).await;
    let _ = fs::remove_dir_all(&path_logs).await;

    // fs::create_dir_all(&path_snapshots).await?;
    // debug!(
    //     "Copy Database backup in place from {} to {}",
    //     path_backup_s3, path_db_full
    // );
    // let bytes = fs::copy(&path_backup_s3, path_db_full).await?;
    // assert!(bytes > 0);
    // info!("Database backup copied {} bytes", bytes);

    // debug!("Removing database temp file {}", path_backup_s3);
    // fs::remove_file(&path_backup_s3).await?;

    Ok(())
}

async fn is_metadata_ok(path_db: String) -> Result<(), Error> {
    task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(path_db)?;
        let mut stmt = conn.prepare_cached("SELECT data FROM _metadata WHERE key = 'meta'")?;
        let bytes = stmt.query_row((), |row| {
            let bytes: Vec<u8> = row.get(0)?;
            Ok(bytes)
        })?;
        let _meta: StateMachineData = bincode::deserialize(&bytes).unwrap();

        // TODO we could maybe add the expected backup id as well, if it should make sense...
        Ok::<(), Error>(())
    })
    .await??;
    Ok(())
}

// TODO we still need the after restore task, but differently:
// after all nodes have joined the cluster again, wait for all nodes to have replicated a log id
// of at least 10, then trigger a snapshot and purge logs afterward.

/// If we applied a backup, it means we did a log-id rollover as well.
/// In this case, if a remote node needs to recover from failure before we purge the very
/// first log with id 1, it will not bother fetching a snapshot from remote, which it must
/// do in case of a backup restore to not end up in an inconsistent state.
/// We will take a snapshot as soon as we have a last log id > 1 and then purge
/// immediately.
pub fn restore_backup_cleanup(state: Arc<AppState>, nodes_count: usize) {
    task::spawn(restore_backup_cleanup_task(state, nodes_count));
}

#[tracing::instrument(level = "debug", skip_all)]
#[cfg(feature = "backup")]
async fn restore_backup_cleanup_task(state: Arc<AppState>, nodes_count: usize) {
    loop {
        match state.raft_db.raft.is_initialized().await {
            Ok(res) => {
                if res {
                    break;
                }
            }
            Err(err) => {
                error!("{}", err);
            }
        }
        debug!("Waiting for Raft init");
        time::sleep(Duration::from_millis(500)).await;
    }

    while state.raft_db.raft.current_leader().await.is_none() {
        time::sleep(Duration::from_millis(100)).await;
    }

    let reqs = 10;
    if state.raft_db.raft.current_leader().await == Some(state.id) {
        for _ in 0..reqs {
            let start = Instant::now();
            match state.raft_db.raft.client_write(QueryWrite::RTT).await {
                Ok(_) => {
                    info!("Raft RTT: {} micros", start.elapsed().as_micros());
                }
                Err(err) => {
                    error!("Raft RTT request error: {}", err);
                }
            }
        }
    }

    let last_log;
    loop {
        let metrics = state.raft_db.raft.metrics().borrow().clone();
        let members = metrics.membership_config.voter_ids().count();

        if members == nodes_count {
            if let Some(last_applied) = metrics.last_applied {
                if last_applied.index >= reqs {
                    debug!("Found high enough last_applied log id");
                    last_log = last_applied.index;
                    break;
                }
            }
        }
        time::sleep(Duration::from_millis(500)).await;
    }

    // TODO wait until all nodes from the config are members before taking a snapshot?

    debug!("Taking snapshot now");
    while let Err(_err) = state.raft_db.raft.trigger().snapshot().await {
        time::sleep(Duration::from_millis(500)).await;
    }

    // wait until snapshot has been built
    while state.raft_db.raft.metrics().borrow().snapshot.is_none() {
        info!("Waiting for snapshot build to finish");
        time::sleep(Duration::from_millis(500)).await;
    }

    debug!("Purging logs");
    while let Err(err) = state.raft_db.raft.trigger().purge_log(last_log).await {
        error!("Error during logs purge: {}", err);
    }

    // give the replication in the background a second to catch up before proceeding
    // we don't care about a bit longer startup after backup
    time::sleep(Duration::from_millis(3000)).await;

    info!("restore_backup_cleanup_task finished successfully");
}

// pub async fn snapshot_after_restore(
//     tx_writer: &flume::Sender<WriterRequest>,
//     data_dir: &str,
// ) -> Result<(), Error> {
//     let snapshot_id = Uuid::now_v7();
//     fs::create_dir_all(&self.path_snapshots)
//         .await
//         .map_err(|err| StorageError::IO {
//             source: StorageIOError::write(&err),
//         })?;
//
//     let path = format!("{}/{}", self.path_snapshots, snapshot_id);
//     let (ack, rx) = oneshot::channel();
//     let req = WriterRequest::Snapshot(SnapshotRequest {
//         snapshot_id,
//         // last_membership: self.last_membership.clone(),
//         path: path.clone(),
//         ack,
//     });
//     self.write_tx
//         .send_async(req)
//         .await
//         .expect("Sender to always be listening");
// }
