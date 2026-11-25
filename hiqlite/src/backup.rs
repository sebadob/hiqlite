use crate::app_state::AppState;
#[cfg(feature = "s3")]
use crate::helpers::parse_duration;
use crate::helpers::{atomic_file_switch, set_path_access, validate_db_backup_snapshot};
use crate::store::logs;
use crate::store::state_machine::sqlite::state_machine::{QueryWrite, StateMachineSqlite};
use crate::{Client, Error, NodeConfig, NodeId};
use chrono::Utc;
use openraft::type_config::async_runtime::watch::WatchReceiver;
use std::env;
use std::ops::Sub;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio::{fs, task, time};
use tracing::{debug, error, info, warn};

#[cfg(feature = "s3")]
use crate::s3::S3Config;
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::state_machine::StateMachineMemory;

pub const BACKUP_DB_NAME: &str = "restore.sqlite";

/// The default prefix of every backup file name, used when `backup_file_prefix` is not set.
pub const DEFAULT_BACKUP_FILE_PREFIX: &str = "backup_node";

/// The timestamp every parsed backup file name must be newer than. Anything older is treated as
/// a bogus name and is never cleaned up, so a wild timestamp can never delete a fresh backup.
const TS_MIN: i64 = 1_704_063_600;

#[derive(Debug, Clone)]
pub struct BackupConfig {
    cron_schedule: cron::Schedule,
    /// How long backups stay in the S3 bucket. The local retention is `backup_keep_for_local` on
    /// the `NodeConfig`, and without the `s3` feature there is nothing to retain offsite.
    #[cfg(feature = "s3")]
    keep_for: Duration,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            cron_schedule: cron::Schedule::from_str("0 30 2 * * * *").unwrap(),
            #[cfg(feature = "s3")]
            keep_for: Duration::from_hours(30 * 24),
        }
    }
}

impl BackupConfig {
    pub fn new(
        cron_schedule: &str,
        #[cfg(feature = "s3")] keep_for: Duration,
    ) -> Result<Self, Error> {
        Ok(Self {
            cron_schedule: cron::Schedule::from_str(cron_schedule)
                .map_err(|_| Error::Config("Invalid syntax for cron_schedule".into()))?,
            #[cfg(feature = "s3")]
            keep_for,
        })
    }

    pub fn from_env() -> Self {
        let cron_str = env::var("HQL_BACKUP_CRON").unwrap_or_else(|_| "0 30 2 * * * *".to_string());
        let cron_schedule =
            cron::Schedule::from_str(&cron_str).expect("Invalid syntax for HQL_BACKUP_CRON");

        #[cfg(feature = "s3")]
        let keep_for = env::var("HQL_BACKUP_KEEP_FOR")
            .ok()
            .as_deref()
            .map_or(Duration::from_hours(30 * 24), |v| {
                parse_duration(v).expect("Cannot parse HQL_BACKUP_KEEP_FOR as Duration")
            });

        Self {
            cron_schedule,
            #[cfg(feature = "s3")]
            keep_for,
        }
    }
}

/// Where a restore reads its backup from.
///
/// Every backup file that Hiqlite writes - locally and in the bucket - is named
/// `{backup_file_prefix}_{node_id}_{unix_ts}.sqlite`, and a source always names exactly one of these
/// files. `start_node_from_backup` takes this type directly, the `HQL_BACKUP_RESTORE` env var is
/// its string form: `s3:<object name>` or `file:<path>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupSource {
    /// The object name inside the configured S3 bucket. Needs the `s3` feature.
    #[cfg(feature = "s3")]
    S3(String),
    /// A local file, given by its full path on this node.
    File(String),
}

impl BackupSource {
    fn from_env() -> Result<Option<Self>, Error> {
        let Some(var) = env::var("HQL_BACKUP_RESTORE").ok() else {
            return Ok(None);
        };

        #[cfg(feature = "s3")]
        if let Some(obj) = var.strip_prefix("s3:") {
            return Ok(Some(Self::S3(obj.to_string())));
        }

        #[cfg(not(feature = "s3"))]
        if var.starts_with("s3:") {
            return Err(Error::S3(
                "HQL_BACKUP_RESTORE asks for a restore from S3, but this build has no S3 support. \
                Restore from a local file with 'file:<path>' instead."
                    .to_string(),
            ));
        }

        if let Some(file) = var.strip_prefix("file:") {
            return Ok(Some(Self::File(file.to_string())));
        }

        Err(Error::Config(
            format!(
                "HQL_BACKUP_RESTORE must start with either 's3:' or 'file:' - \
                unknown prefix: '{var}'"
            )
            .into(),
        ))
    }
}

/// The full file name of a backup: `{prefix}_{node_id}_{ts}.sqlite`.
pub(crate) fn backup_file_name(prefix: &str, node_id: NodeId, ts: i64) -> String {
    format!("{prefix}_{node_id}_{ts}.sqlite")
}

/// A backup file prefix must be a plain single file name component: non-empty, never `.` / `..`,
/// and only characters that are safe inside a file name. Path separators are what would let a
/// prefix point outside the backups directory, so they are rejected.
pub(crate) fn is_valid_file_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix != "."
        && prefix != ".."
        && prefix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// The unix timestamp out of a `{prefix}_{node_id}_{ts}.sqlite` file name, or `None` when the
/// name is not ours.
///
/// The match is exact on purpose: the prefix must be followed by `_`, the node id and the
/// timestamp must both be plain numbers and the name must end in `.sqlite`. Anything else - a
/// foreign file in a shared bucket, a name under a longer prefix like `backup_node_v2_...`, a
/// name with a path in it - is `None`, and cleanup jobs never touch a name that is `None`.
pub(crate) fn backup_ts_from_name(name: &str, prefix: &str) -> Option<i64> {
    let stem = name
        .strip_prefix(prefix)
        .and_then(|s| s.strip_prefix('_'))
        .and_then(|s| s.strip_suffix(".sqlite"))?;
    let (node_id, ts) = stem.rsplit_once('_')?;
    if node_id.parse::<NodeId>().is_ok() {
        ts.parse::<i64>().ok()
    } else {
        None
    }
}

pub fn start_cron(
    client: Client,
    backup_config: BackupConfig,
    #[cfg(feature = "s3")] backup_file_prefix: String,
    #[cfg(feature = "s3")] s3_config: Option<Arc<S3Config>>,
) {
    task::spawn(Box::pin(async move {
        info!("Backup cron task started");

        loop {
            let dur = {
                let now = chrono::Local::now();
                let Some(next) = backup_config.cron_schedule.upcoming(chrono::Local).next() else {
                    // e.g. a cron with an impossible date like Feb 30: no upcoming event.
                    // Keep the task alive instead of panicking and silently losing backups.
                    warn!("Cron schedule has no upcoming event - retrying in 1 hour");
                    time::sleep(Duration::from_secs(3600)).await;
                    continue;
                };
                if next <= now {
                    // don't set it to 0 to not go crazy in case of a bad config or timing
                    Duration::from_secs(1)
                } else {
                    // `next > now`, so the delta is never negative; a gap below one second
                    // truncates to 0 and runs the backup immediately, like before.
                    let delta = (next - now).num_seconds();
                    Duration::from_secs(u64::try_from(delta).unwrap_or_default())
                }
            };
            time::sleep(dur).await;

            info!("Executing backup now");
            let mut success = false;
            let retries = 5;

            for _ in 0..retries {
                match backup_cron_job(
                    &client,
                    #[cfg(feature = "s3")]
                    backup_config.keep_for,
                    #[cfg(feature = "s3")]
                    &backup_file_prefix,
                    #[cfg(feature = "s3")]
                    s3_config.as_ref(),
                )
                .await
                {
                    Ok(()) => {
                        info!("Backup task finished successfully");
                        success = true;
                        break;
                    }
                    Err(err) => {
                        if err.is_forward_to_leader().is_some() {
                            debug!(
                                "Raft currently has no leader - retrying in 10 seconds\n{:?}",
                                err
                            );
                        } else {
                            // If we get here, it was most likely an S3 failure. The other things
                            // that could go wrong are disk / DB operations, which is somewhat
                            // unlikely.
                            error!("Error during backup task execution: {}", err);
                        }
                    }
                }
                time::sleep(Duration::from_secs(10)).await;
            }

            if !success {
                warn!("Backup task failed after {} retries", retries);
            }
        }
    }));
}

async fn backup_cron_job(
    client: &Client,
    #[cfg(feature = "s3")] keep_for: Duration,
    #[cfg(feature = "s3")] backup_file_prefix: &str,
    #[cfg(feature = "s3")] s3_config: Option<&Arc<S3Config>>,
) -> Result<(), Error> {
    client.backup().await?;

    #[cfg(feature = "s3")]
    {
        if let Some(s3_config) = s3_config {
            // the backup task will be async in the background, but we can start cleaning up already
            let threshold = Utc::now().sub(keep_for).timestamp();

            let list = s3_config.bucket.list("", None).await?;
            for bucket in list {
                if bucket.name != s3_config.bucket.name {
                    info!("Found non-configured bucket {} - skipping", bucket.name);
                    continue;
                }

                for object in &bucket.contents {
                    // Only our own backup files are ever deleted here. Anything a shared bucket
                    // holds that does not match `{prefix}_{node_id}_{ts}.sqlite` exactly is left
                    // untouched, which is what makes one bucket for many clusters safe.
                    let Some(ts) = backup_ts_from_name(&object.key, backup_file_prefix) else {
                        debug!("Found non-backup object - skipping: {}", object.key);
                        continue;
                    };
                    if ts < threshold {
                        info!("Deleting expired backup: {}", object.key);
                        s3_config.bucket.delete(object.key.clone()).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

pub(crate) async fn backup_local_cleanup(
    backup_path: String,
    keep_for: Duration,
    backup_file_prefix: &str,
) -> Result<(), Error> {
    let ts_threshold = Utc::now().sub(keep_for).timestamp();

    let path = Path::new(&backup_path);
    let mut dir_entries = tokio::fs::read_dir(path).await?;

    loop {
        let entry = match dir_entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(err) => {
                warn!("Error reading directory entries: {err:?}");
                break;
            }
        };
        if entry.metadata().await?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        if let Some(s_orig) = name.to_str() {
            // A backup is written to a temp file named "...sqlite~" and renamed into
            // place; a crash between the two orphans that temp file. Strip the trailing
            // '~' so it is judged by the same age rule as its final-name counterpart:
            // old orphans are reclaimed, while an in-progress backup (fresh ts) is kept.
            let s = s_orig.strip_suffix('~').unwrap_or(s_orig);

            // same rule as on S3: a name that is not exactly ours is never removed
            let Some(ts) = backup_ts_from_name(s, backup_file_prefix) else {
                debug!("Found non-backup file - skipping: {s_orig}");
                continue;
            };

            if ts > TS_MIN && ts < ts_threshold {
                // remove the on-disk file under its real name (keep any '~' suffix)
                let p = format!("{backup_path}/{s_orig}");
                info!("Cleaning up local backup {s_orig} ({p})");
                if let Err(err) = tokio::fs::remove_file(p).await {
                    error!(?err, "Error removing local backup");
                }
            }
        }
    }

    Ok(())
}

/// Restore the backup that was given to `start_node_from_backup`, or the one named by the
/// `HQL_BACKUP_RESTORE` env var when no explicit source was given.
/// Returns `Ok(true)` when the backup has been applied, which needs the node ID `1`.
///
/// Every node of the cluster must be started with the same restore source: node 1 applies the
/// backup, all other nodes wipe their own state and join the new cluster clean.
pub(crate) async fn restore_backup_start(
    node_config: &NodeConfig,
    explicit: Option<BackupSource>,
) -> Result<bool, Error> {
    let src = match explicit {
        Some(src) => Some(src),
        None => BackupSource::from_env()?,
    };

    if let Some(src) = src {
        info!("Found backup restore request {:?}", src);

        if node_config.node_id == 1 {
            restore_backup(node_config, src).await?;
            return Ok(true);
        }
        warn!("Cleaning up existing files and start restore cluster join");
        cleanup_restore_data_dirs(&node_config.data_dir).await?;
    }

    Ok(false)
}

/// Delete all state under `data_dir` that must not survive a backup restore: the SQLite DB
/// (including its WAL sidecars), raft snapshots, lock file, both raft WALs and the cache data
/// (+ its WAL, only present with the `cache` feature). Only these known paths are removed -
/// never `data_dir` itself, which may be a direct container mount that cannot be deleted, and
/// never anything unknown (e.g. the staged backup in `state_machine/backups`). Missing paths
/// are fine; any other error is propagated, so a failed cleanup can never silently leave stale
/// state behind.
async fn cleanup_restore_data_dirs(data_dir: &str) -> Result<(), Error> {
    let sqlite_paths = [
        StateMachineSqlite::path_db(data_dir),
        StateMachineSqlite::path_snapshots(data_dir),
        StateMachineSqlite::path_lock_file(data_dir),
        logs::logs_dir_db(data_dir),
    ];
    #[cfg(feature = "cache")]
    let cache_paths = [
        StateMachineMemory::path_sm(data_dir),
        logs::logs_dir_cache(data_dir),
    ];

    for path in &sqlite_paths {
        match fs::remove_dir_all(path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Error::Sqlite(
                    format!("Cannot remove old data dir {path}: {err}").into(),
                ));
            }
        }
    }

    #[cfg(feature = "cache")]
    for path in &cache_paths {
        match fs::remove_dir_all(path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Error::Cache(
                    format!("Cannot remove old data dir {path}: {err}").into(),
                ));
            }
        }
    }

    Ok(())
}

/// Apply the given backup and move it onto the node's database path.
///
/// **CAUTION: This function MUST BE CALLED when the Raft is not running!**
///
/// You only need this if you apply a backup in another way than `start_node_from_backup` (or the
/// `HQL_BACKUP_RESTORE` env var), which both do it for you at startup.
pub async fn restore_backup(node_config: &NodeConfig, src: BackupSource) -> Result<(), Error> {
    info!("Starting database restore from backup {:?}", src);

    #[cfg(feature = "s3")]
    if let BackupSource::S3(_) = &src
        && node_config.s3_config.is_none()
    {
        return Err(Error::S3(
            "No `S3Config` given, cannot restore backup".to_string(),
        ));
    }

    let path_db = StateMachineSqlite::path_db(&node_config.data_dir);
    let path_backups = StateMachineSqlite::path_backups(&node_config.data_dir);

    fs::create_dir_all(&path_backups).await?;
    set_path_access(&path_backups, 0o700).await?;

    let path_backup = match src {
        #[cfg(feature = "s3")]
        BackupSource::S3(s3_obj) => {
            let Some(s3_config) = &node_config.s3_config else {
                return Err(Error::S3(
                    "No `S3Config` given, cannot restore backup".to_string(),
                ));
            };
            let path_backup = format!("{path_backups}/{BACKUP_DB_NAME}");
            s3_config.pull(&s3_obj, &path_backup).await?;
            path_backup
        }
        BackupSource::File(path_src) => {
            let (path, filename) = path_src.rsplit_once('/').unwrap_or(("", &path_src));
            debug!("Given backup path full: '{path_src}', after parsing: '{path}' / '{filename}'");
            let path_backup = format!("{path_backups}/{filename}");

            fs::copy(path_src, &path_backup).await?;
            path_backup
        }
    };
    fs::File::open(&path_backup).await?.sync_data().await?;

    validate_backup_db(path_backup.clone()).await?;
    debug!("Database backup validation passed (metadata + integrity check)");

    debug!("Removing old data");
    // Removing old dirs even before the backup is in its final place. Important to never end up
    // in an inconsistent state with a partly applied backups and maybe still existing old WAL data.
    // You apply the backup for a reason -> the current data is broken anyway. If anything fails
    // between here and getting the backup into place, you start fresh anyway. The staged backup
    // in `state_machine/backups` has been validated already and is intentionally kept.
    cleanup_restore_data_dirs(&node_config.data_dir).await?;

    fs::create_dir_all(&path_db).await?;
    set_path_access(&path_db, 0o700).await?;

    let path_db_full = format!("{}/{}", path_db, node_config.filename_db);
    info!(
        "Given backup check ok - moving into its final place: {} -> {}",
        path_backup, path_db_full
    );
    atomic_file_switch(path_backup, &path_db_full).await?;
    set_path_access(&path_db_full, 0o700).await?;

    Ok(())
}

pub(crate) async fn validate_backup_db(path_db: String) -> Result<(), Error> {
    if env::var("HQL_BACKUP_SKIP_VALIDATION").as_deref() == Ok("true") {
        return Ok(());
    }
    validate_db_backup_snapshot(path_db).await
}

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
        let metrics = state.raft_db.raft.metrics().borrow_watched().clone();
        if let Some(last_applied) = metrics.last_applied
            && last_applied.index >= reqs
        {
            debug!("Found high enough last_applied log id");
            last_log = last_applied.index;
            break;
        }
        time::sleep(Duration::from_millis(50)).await;
    }

    debug!("Taking snapshot now");
    if let Err(err) = state.raft_db.raft.trigger().snapshot().await {
        // e.g. the raft is shutting down: do not panic the task, just bail out - the
        // wait-for-snapshot loop below would never finish anyway
        error!("Error triggering snapshot: {err}");
        return;
    }

    // wait until snapshot has been built
    while state
        .raft_db
        .raft
        .metrics()
        .borrow_watched()
        .snapshot
        .is_none()
    {
        info!("Waiting for snapshot build to finish");
        time::sleep(Duration::from_millis(100)).await;
    }

    debug!("Purging logs");
    while let Err(err) = state.raft_db.raft.trigger().purge_log(last_log).await {
        // It's very important to retry until success. If we had an upper limit and exit early,
        // the only thing we could do is actually panic. This is important because the first
        // WAL logs entry MUST NOT start fresh. Otherwise, it would screw up the backup logic.
        // -> retry until success, which will usually succeed instantly
        error!("Error during logs purge: {}", err);
        time::sleep(Duration::from_millis(100)).await;
    }

    info!("restore_backup_finish task successful");
}

/// What this node knows about its own backups since it started, for `GET /backup`.
///
/// Each node creates its own local backup when a backup entry is applied, and only the node that
/// requested the backup (the leader) pushes it to S3, so this is always a per-node view. It lives
/// in memory: after a restart, the newest local backup is still found on disk, but the S3 and
/// error records start empty.
#[derive(Debug, Default)]
pub(crate) struct BackupStatus {
    inner: std::sync::Mutex<BackupStatusInner>,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub(crate) struct BackupStatusInner {
    pub last_backup: Option<BackupEvent>,
    pub last_backup_error: Option<BackupEvent>,
    #[cfg(feature = "s3")]
    pub last_s3_upload: Option<BackupEvent>,
    #[cfg(feature = "s3")]
    pub last_s3_upload_error: Option<BackupEvent>,
}

/// One backup or upload outcome: when it finished (unix seconds) and the backup file name, plus the
/// error message for a failure.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct BackupEvent {
    pub ts: i64,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BackupEvent {
    fn now(file: &str, error: Option<String>) -> Self {
        Self {
            ts: Utc::now().timestamp(),
            file: file.to_string(),
            error,
        }
    }
}

impl BackupStatus {
    fn update(&self, f: impl FnOnce(&mut BackupStatusInner)) {
        // a poisoned lock only means a panic happened while recording, the data is still usable
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut inner);
    }

    pub(crate) fn backup_ok(&self, file: &str) {
        self.update(|s| s.last_backup = Some(BackupEvent::now(file, None)));
    }

    pub(crate) fn backup_failed(&self, file: &str, err: &Error) {
        self.update(|s| {
            s.last_backup_error = Some(BackupEvent::now(file, Some(err.to_string())));
        });
    }

    #[cfg(feature = "s3")]
    pub(crate) fn s3_upload_ok(&self, file: &str) {
        self.update(|s| s.last_s3_upload = Some(BackupEvent::now(file, None)));
    }

    #[cfg(feature = "s3")]
    pub(crate) fn s3_upload_failed(&self, file: &str, err: &str) {
        self.update(|s| {
            s.last_s3_upload_error = Some(BackupEvent::now(file, Some(err.to_string())));
        });
    }

    pub(crate) fn get(&self) -> BackupStatusInner {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// The local backups of this node that are on disk right now, under the configured prefix.
#[derive(Debug, Default, serde::Serialize)]
pub(crate) struct LocalBackups {
    pub count: usize,
    /// The newest backup's file name and the timestamp in its name (the time it was requested).
    pub newest: Option<LocalBackup>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct LocalBackup {
    pub ts: i64,
    pub file: String,
}

/// Lists finished local backups. Temporary `~` files of a backup in progress are not counted.
pub(crate) async fn local_backups(backup_path: &str, prefix: &str) -> Result<LocalBackups, Error> {
    let mut res = LocalBackups::default();
    let mut list = match fs::read_dir(backup_path).await {
        Ok(list) => list,
        // no backup has ever been created on this node
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(res),
        Err(err) => return Err(err.into()),
    };
    while let Some(entry) = list.next_entry().await? {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(ts) = backup_ts_from_name(name, prefix) else {
            continue;
        };
        res.count += 1;
        if res.newest.as_ref().is_none_or(|n| ts > n.ts) {
            res.newest = Some(LocalBackup {
                ts,
                file: name.to_string(),
            });
        }
    }
    Ok(res)
}

/// The response of `GET /backup`.
#[derive(Debug, serde::Serialize)]
pub(crate) struct BackupInfo {
    pub node_id: NodeId,
    pub local: LocalBackups,
    #[serde(flatten)]
    pub status: BackupStatusInner,
}

impl BackupInfo {
    /// The same information in the Prometheus text format. A timestamp of `0` means "never since
    /// this node started" (or "no backup on disk" for the local one), so an alert on
    /// `time() - hiqlite_backup_local_newest_timestamp_seconds` also fires for a node without any.
    pub(crate) fn to_prometheus(&self) -> String {
        use std::fmt::Write;

        let node = self.node_id;
        let ts = |e: &Option<BackupEvent>| e.as_ref().map_or(0, |e| e.ts);
        let mut out = String::with_capacity(1024);
        let mut gauge = |name: &str, help: &str, value: i64| {
            let _ = writeln!(out, "# HELP {name} {help}");
            let _ = writeln!(out, "# TYPE {name} gauge");
            let _ = writeln!(out, "{name}{{node_id=\"{node}\"}} {value}");
        };

        gauge(
            "hiqlite_backup_local_count",
            "Finished local backups of this node on disk.",
            i64::try_from(self.local.count).unwrap_or(i64::MAX),
        );
        gauge(
            "hiqlite_backup_local_newest_timestamp_seconds",
            "Timestamp in the name of the newest local backup on disk, 0 if there is none.",
            self.local.newest.as_ref().map_or(0, |n| n.ts),
        );
        gauge(
            "hiqlite_backup_last_success_timestamp_seconds",
            "When this node last finished a local backup since it started, 0 if never.",
            ts(&self.status.last_backup),
        );
        gauge(
            "hiqlite_backup_last_error_timestamp_seconds",
            "When a local backup of this node last failed since it started, 0 if never.",
            ts(&self.status.last_backup_error),
        );
        #[cfg(feature = "s3")]
        {
            gauge(
                "hiqlite_backup_s3_last_upload_success_timestamp_seconds",
                "When this node last finished an S3 upload since it started, 0 if never.",
                ts(&self.status.last_s3_upload),
            );
            gauge(
                "hiqlite_backup_s3_last_upload_error_timestamp_seconds",
                "When an S3 upload of this node last failed for good since it started, 0 if never.",
                ts(&self.status.last_s3_upload_error),
            );
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::{backup_file_name, backup_ts_from_name, is_valid_file_prefix};

    #[test]
    fn a_backup_file_name_round_trips_to_its_timestamp() {
        let name = backup_file_name("backup_node", 2, 1_700_000_000);
        assert_eq!(name, "backup_node_2_1700000000.sqlite");
        assert_eq!(
            backup_ts_from_name(&name, "backup_node"),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn a_custom_prefix_only_matches_its_own_files() {
        assert_eq!(
            backup_ts_from_name("app-a_1_1700000000.sqlite", "app-a"),
            Some(1_700_000_000)
        );
        // the default prefix is not ours and ours is not the default one
        assert_eq!(
            backup_ts_from_name("backup_node_1_1700000000.sqlite", "app-a"),
            None
        );
        assert_eq!(
            backup_ts_from_name("app-a_1_1700000000.sqlite", "backup_node"),
            None
        );
        // a longer name that starts with our prefix must not match
        assert_eq!(
            backup_ts_from_name("app-a_v2_1_1700000000.sqlite", "app-a"),
            None
        );
        assert_eq!(
            backup_ts_from_name("app-ab_1_1700000000.sqlite", "app-a"),
            None
        );
    }

    #[test]
    fn foreign_files_have_no_backup_timestamp() {
        for name in [
            "",
            "backup_node",
            "backup_node_",
            "backup_node_1",
            "backup_node_1_.sqlite",
            "backup_node_1_1700000000",
            "backup_node_1_1700000000.sqlite~",
            "backup_node_1_1700000000.sqlite.gz",
            "backup_node_x_1700000000.sqlite",
            "backup_node_1_1700000000.sqlite/",
            "backup_node_1/../../etc/passwd",
            "backup_node_1\\..\\..\\windows",
            "../backup_node_1_1700000000.sqlite",
            "some/backup_node_1_1700000000.sqlite",
            "notes.txt",
        ] {
            assert_eq!(
                backup_ts_from_name(name, "backup_node"),
                None,
                "must have no ts: {name:?}"
            );
        }
    }

    #[test]
    fn only_plain_file_name_prefixes_are_valid() {
        for prefix in ["backup_node", "app-a", "app.bak", "App1", "backup_node_"] {
            assert!(is_valid_file_prefix(prefix), "must accept: {prefix:?}");
        }
        for prefix in ["", ".", "..", "a/b", "a\\b", "a b", "a\0", "näme"] {
            assert!(!is_valid_file_prefix(prefix), "must reject: {prefix:?}");
        }
    }
}
