use crate::app_state::AppState;
use crate::helpers::{deserialize, set_path_access};
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
use std::path::Path;
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

#[derive(Debug, PartialEq)]
pub enum BackupSource {
    S3(String),
    File(String),
}

impl BackupSource {
    fn from_env() -> Option<Self> {
        let var = env::var("HQL_BACKUP_RESTORE").ok()?;

        if let Some(obj) = var.strip_prefix("s3:") {
            return Some(Self::S3(obj.to_string()));
        }

        if let Some(file) = var.strip_prefix("file:") {
            return Some(Self::File(file.to_string()));
        }

        error!(
            "HQL_BACKUP_RESTORE must start with either 's3:' or 'file:'. \
            Cannot restore from backup - unknown prefix: {}",
            var
        );
        None
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
            let mut success = false;
            let retries = 5;

            for _ in 0..retries {
                match backup_cron_job(&client, &s3_config, backup_config.keep_days).await {
                    Ok(_) => {
                        info!("Backup task finished successfully");
                        success = true;
                        break;
                    }
                    Err(err) => {
                        if err.is_forward_to_leader().is_some() {
                            warn!(
                                "Raft currently has no leader - retrying in 10 seconds\n{:?}",
                                err
                            );
                            time::sleep(Duration::from_secs(10)).await;
                        } else {
                            error!("Error during backup task execution: {}", err);
                            break;
                        }
                    }
                }
            }

            if !success {
                warn!("Backup task failed after {} retries", retries);
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

pub(crate) async fn backup_local_cleanup(backup_path: String) -> Result<(), Error> {
    // 2024/01/01 00:00:00
    let ts_min = 1704063600;

    let keep_days = env::var("HQL_BACKUP_KEEP_DAYS_LOCAL")
        .unwrap_or_else(|_| "3".to_string())
        .parse::<u32>()
        .unwrap_or_else(|_| {
            error!("Error parsing HQL_BACKUP_KEEP_DAYS_LOCAL to u32, using default of 3 days");
            3
        });
    let ts_threshold = Utc::now()
        .sub(chrono::Duration::days(keep_days as i64))
        .timestamp();

    let path = Path::new(&backup_path);
    let mut dir_entries = tokio::fs::read_dir(path).await?;

    while let Ok(Some(entry)) = dir_entries.next_entry().await {
        if entry.metadata().await?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        if let Some(s) = name.to_str() {
            // format!("backup_node_{}_{}.sqlite", node_id, Utc::now().timestamp());
            if !s.starts_with("backup_node_") && !s.ends_with(".sqlite") {
                continue;
            }

            let stripped = s.strip_suffix(".sqlite").unwrap_or(s);
            let (_, ts) = stripped.rsplit_once('_').unwrap_or_default();

            match ts.parse::<i64>() {
                Ok(ts) => {
                    if ts > ts_min && ts < ts_threshold {
                        debug!("Cleaning up backup {s}");
                        let p = format!("{backup_path}{s}");
                        let _ = tokio::fs::remove_dir_all(p).await;
                    }
                }
                Err(err) => {
                    error!("Cannot parse ts from file {s}: {err}")
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

/// Check if the env var `HQL_BACKUP_RESTORE` is set and restores the given backup if so.
/// Returns `Ok(true)` if backup has been applied.
/// This will only run if the current node ID is `1`.
pub(crate) async fn restore_backup_start(node_config: &NodeConfig) -> Result<bool, Error> {
    if let Some(src) = BackupSource::from_env() {
        info!("Found backup restore request {:?}", src);

        if node_config.node_id == 1 {
            restore_backup(node_config, src).await?;
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
/// way than with the `HQL_BACKUP_RESTORE` env var, which is being done automatically.
pub async fn restore_backup(node_config: &NodeConfig, src: BackupSource) -> Result<(), Error> {
    info!("Starting database restore from backup {:?}", src);

    if let BackupSource::S3(_) = &src {
        if node_config.s3_config.is_none() {
            return Err(Error::S3(
                "No `S3Config` given, cannot restore backup".to_string(),
            ));
        }
    }

    let (
        PathDb(path_db),
        PathBackups(path_backups),
        PathSnapshots(path_snapshots),
        PathLockFile(path_lock_file),
    ) = StateMachineSqlite::build_folders(&node_config.data_dir, false).await;
    let path_logs = logs::logs_dir_db(&node_config.data_dir);

    fs::create_dir_all(&path_backups).await?;
    set_path_access(&path_backups, 0o700).await?;

    let (path_backup, remove_src) = match src {
        BackupSource::S3(s3_obj) => {
            let s3_config = match &node_config.s3_config {
                None => {
                    return Err(Error::S3(
                        "No `S3Config` given, cannot restore backup".to_string(),
                    ));
                }
                Some(c) => c,
            };
            let path_backup = format!("{path_backups}/{BACKUP_DB_NAME}");
            s3_config.pull(&s3_obj, &path_backup).await?;
            (path_backup, true)
        }
        BackupSource::File(path_src) => {
            let (path, filename) = path_src.rsplit_once('/').unwrap_or(("", &path_src));
            debug!("Given backup path full: '{path_src}', after parsing: '{path}' / '{filename}'");
            let path_backup = format!("{path_backups}/{filename}");

            fs::copy(path_src, &path_backup).await?;
            (path_backup, false)
        }
    };

    is_metadata_ok(path_backup.clone()).await?;
    debug!("Database backup metadata is ok");

    debug!("Removing old data");
    let _ = fs::remove_dir_all(&path_db).await;
    // let _ = fs::remove_dir_all(&path_backups).await;
    let _ = fs::remove_dir_all(&path_snapshots).await;
    let _ = fs::remove_dir_all(&path_lock_file).await;
    let _ = fs::remove_dir_all(&path_logs).await;

    fs::create_dir_all(&path_db).await?;
    set_path_access(&path_db, 0o700).await?;

    let path_db_full = format!("{}/{}", path_db, node_config.filename_db);
    info!(
        "Given backup check ok - copying into its final place: {} -> {}",
        path_backup, path_db_full
    );
    fs::copy(&path_backup, &path_db_full).await?;
    set_path_access(&path_db_full, 0o700).await?;

    if remove_src {
        info!("Cleaning up S3 backup from {}", path_backup);
        fs::remove_file(path_backup).await?;
    }

    Ok(())
}

async fn is_metadata_ok(path_db: String) -> Result<(), Error> {
    if env::var("HQL_BACKUP_SKIP_VALIDATION") == Ok("true".to_string()) {
        return Ok(());
    }

    task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(path_db)?;
        let mut stmt = conn.prepare_cached("SELECT data FROM _metadata WHERE key = 'meta'")?;
        let bytes = stmt.query_row((), |row| {
            let bytes: Vec<u8> = row.get(0)?;
            Ok(bytes)
        })?;
        let _meta: StateMachineData = deserialize(&bytes).unwrap();

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
