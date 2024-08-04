use crate::app_state::AppState;
use crate::s3::S3Config;
use crate::store::logs;
use crate::store::state_machine::sqlite::state_machine::{
    PathBackups, PathDb, PathLockFile, PathSnapshots, QueryWrite, StateMachineData,
    StateMachineSqlite,
};
use crate::{Client, Error, NodeConfig};
use chrono::{DateTime, Utc};
use std::env;
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio::{fs, task, time};
use tracing::{debug, error, info, warn};

pub const BACKUP_DB_NAME: &str = "restore.sqlite";

#[derive(Debug, Clone)]
pub struct BackupConfig {
    cron_schedule: cron::Schedule,
    keep_days: u16,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            cron_schedule: cron::Schedule::from_str("0 30 2 * * * *").unwrap(),
            keep_days: 30,
        }
    }
}

impl BackupConfig {
    pub fn new(cron_schedule: &str, keep_days: u16) -> Result<Self, Error> {
        Ok(Self {
            cron_schedule: cron::Schedule::from_str(cron_schedule)
                .map_err(|_| Error::Config("Invalid syntax for cron_schedule".into()))?,
            keep_days,
        })
    }

    pub fn from_env() -> Self {
        let cron_str = env::var("HQL_BACKUP_CRON").unwrap_or_else(|_| "0 30 2 * * * *".to_string());
        let cron_schedule =
            cron::Schedule::from_str(&cron_str).expect("Invalid syntax for HQL_BACKUP_CRON");

        let keep_days = env::var("HQL_BACKUP_KEEP_DAYS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u16>()
            .expect("Cannot parse HQL_BACKUP_KEEP_DAYS to u16");

        Self {
            cron_schedule,
            keep_days,
        }
    }
}

pub fn start_cron(client: Client, s3_config: Arc<S3Config>, backup_config: BackupConfig) {
    task::spawn(async move {
        info!("Backup cron task started");

        loop {
            let dur = {
                let now = chrono::Local::now();
                let next = backup_config
                    .cron_schedule
                    .upcoming(chrono::Local)
                    .next()
                    .expect("No next cron task found");
                if next <= now {
                    // don't set it to 0 to not go crazy in case of a bad config or timing
                    Duration::from_secs(1)
                } else {
                    Duration::from_secs((next.timestamp() - now.timestamp()) as u64)
                }
            };

            time::sleep(dur).await;
            info!("Executing backup now");
            loop {
                match backup_cron_job(&client, &s3_config, backup_config.keep_days).await {
                    Ok(_) => {
                        info!("Backup task finished successfully");
                        break;
                    }
                    Err(err) => {
                        if err.is_forward_to_leader().is_some() {
                            warn!("Raft leader voting in progress - retrying in 10 seconds");
                            time::sleep(Duration::from_secs(10)).await;
                        } else {
                            error!("Error during backup task execution: {}", err);
                            break;
                        }
                    }
                }
            }
        }
    });
}

async fn backup_cron_job(
    client: &Client,
    s3_config: &Arc<S3Config>,
    keep_days: u16,
) -> Result<(), Error> {
    client.backup().await?;

    // the backup task will be async in the background, but we can start cleaning up already
    let threshold = Utc::now().sub(chrono::Duration::days(keep_days as i64));

    let list = s3_config.bucket.list("", None).await?;
    for bucket in list {
        if bucket.name != s3_config.bucket.name {
            info!("Found non-configured bucket {} - skipping", bucket.name);
            continue;
        }

        for object in bucket.contents.iter() {
            if let Some(dt) = dt_from_backup_name(&object.key) {
                if dt < threshold {
                    info!("Deleting expired backup: {}", object.key);
                    s3_config.bucket.delete(object.key.clone()).await?;
                }
            }
        }
    }

    Ok(())
}

fn dt_from_backup_name(name: &str) -> Option<DateTime<Utc>> {
    if let Some(backup) = name.strip_prefix("backup_node_") {
        let (_, rest) = match backup.split_once("_") {
            None => {
                error!("Invalid backup filename format on S3: {}", name);
                return None;
            }
            Some(s) => s,
        };
        let ts = match rest.strip_suffix(".sqlite") {
            None => {
                error!(
                    "Invalid backup filename on S3 - '.sqlite' suffix missing: {}",
                    name
                );
                return None;
            }
            Some(ts) => ts,
        };

        match ts.parse::<i64>() {
            Ok(ts) => DateTime::from_timestamp(ts, 0),
            Err(err) => {
                error!("Error parsing TS from remote backup as i64: {}", err);
                None
            }
        }
    } else {
        None
    }
}

/// Check if the env var `HIQLITE_BACKUP_RESTORE` is set and restores the given backup if so.
/// Returns `Ok(true)` if backup has been applied.
/// This will only run if the current node ID is `1`.
pub(crate) async fn restore_backup_start(node_config: &NodeConfig) -> Result<bool, Error> {
    if let Ok(name) = env::var("HIQLITE_BACKUP_RESTORE") {
        warn!("Found HIQLITE_BACKUP_RESTORE={}", name);

        if node_config.node_id == 1 {
            warn!("Starting restore process on Node 1");
            restore_backup(node_config, &name).await?;
            return Ok(true);
        } else {
            warn!("Cleaning up existing files and start restore cluster join");
            let _ = fs::remove_dir_all(node_config.data_dir.as_ref()).await;
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

    fs::create_dir_all(&path_db).await?;
    let path_db_full = format!("{}/{}", path_db, node_config.filename_db);
    info!(
        "Fetched backup check ok - copying into its final place: {} -> {}",
        path_backup_s3, path_db_full
    );
    fs::copy(&path_backup_s3, path_db_full).await?;

    info!("Cleaning up fetched backup from {}", path_backup_s3);
    fs::remove_file(path_backup_s3).await?;

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

// pub fn restore_backup_finish(state: Arc<AppState>, nodes_count: usize) {
//     task::spawn(restore_backup_cleanup_task(state, nodes_count));
// }

#[tracing::instrument(level = "debug", skip_all)]
#[cfg(feature = "backup")]
pub async fn restore_backup_finish(state: &Arc<AppState>) {
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
        time::sleep(Duration::from_millis(50)).await;
    }

    while state.raft_db.raft.current_leader().await.is_none() {
        time::sleep(Duration::from_millis(50)).await;
    }

    debug_assert!(
        state.raft_db.raft.current_leader().await == Some(state.id),
        "It should never happen that node 1 is not the raft leader during backup restore"
    );

    let reqs = 10;
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

    let last_log;
    loop {
        let metrics = state.raft_db.raft.metrics().borrow().clone();
        if let Some(last_applied) = metrics.last_applied {
            if last_applied.index >= reqs {
                debug!("Found high enough last_applied log id");
                last_log = last_applied.index;
                break;
            }
        }
        time::sleep(Duration::from_millis(50)).await;
    }

    debug!("Taking snapshot now");
    state
        .raft_db
        .raft
        .trigger()
        .snapshot()
        .await
        .expect("Snapshot trigger to always succeed at this point");

    // while let Err(_err) = state.raft_db.raft.trigger().snapshot().await {
    //     debug_assert!("")
    //     time::sleep(Duration::from_millis(500)).await;
    // }

    // wait until snapshot has been built
    while state.raft_db.raft.metrics().borrow().snapshot.is_none() {
        info!("Waiting for snapshot build to finish");
        time::sleep(Duration::from_millis(100)).await;
    }

    debug!("Purging logs");
    while let Err(err) = state.raft_db.raft.trigger().purge_log(last_log).await {
        error!("Error during logs purge: {}", err);
    }

    info!("restore_backup_finish task successful");
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
