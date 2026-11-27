use crate::app_state::AppState;
#[cfg(feature = "s3")]
use crate::helpers::parse_duration;
use crate::helpers::{
    atomic_file_switch, set_path_access, sync_file_data, validate_db_backup_snapshot,
};
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

    /// The source in the form of the `HQL_BACKUP_RESTORE` env var.
    fn as_request(&self) -> String {
        match self {
            #[cfg(feature = "s3")]
            Self::S3(obj) => format!("s3:{obj}"),
            Self::File(path) => format!("file:{path}"),
        }
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

/// What `restore_backup_start` did with a restore request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RestoreStart {
    /// No restore was asked for, or the requested backup has been restored already.
    None,
    /// Node 1 applied a backup: the one of the request, or the one of an earlier restore that was
    /// interrupted. `restore_backup_finish` must run once the Raft is up.
    Applied(Option<BackupSource>),
    /// Any other node cleared its state, or moved it aside with `backup_restore_keep_previous`,
    /// and joins the restored cluster empty. `discard_pre_restore_state` records the restore and
    /// removes the state that was moved aside once this node is a member of that cluster.
    Rejoin(BackupSource),
}

/// Restore the backup that was given to `start_node_from_backup`, or the one named by the
/// `HQL_BACKUP_RESTORE` env var when no explicit source was given.
///
/// Every node of the cluster must be started with the same restore source: node 1 applies the
/// backup, all other nodes wipe their own state (or move it aside with
/// `backup_restore_keep_previous`) and join the new cluster clean. Once that is done, the source
/// is recorded with `RestoredMarker`, and a later start with the same source does nothing.
///
/// Nothing is touched while another live process holds the state machine lock of this
/// `data_dir`.
pub(crate) async fn restore_backup_start(
    node_config: &NodeConfig,
    explicit: Option<BackupSource>,
) -> Result<RestoreStart, Error> {
    let src = match explicit {
        Some(src) => Some(src),
        None => BackupSource::from_env()?,
    };
    let data_dir = &node_config.data_dir;

    let Some(src) = src else {
        if !fs::try_exists(path_pending_restore(data_dir)).await? {
            return Ok(RestoreStart::None);
        }
        warn!("Found a validated backup that has not been applied yet - finishing the restore");
        let mut lock = RestoreLock::acquire(data_dir).await?;
        apply_pending_restore(node_config, &mut lock).await?;
        if node_config.node_id == 1 {
            return Ok(RestoreStart::Applied(None));
        }
        return Ok(RestoreStart::None);
    };
    info!("Found backup restore request {:?}", src);

    if RestoredMarker::matches(data_dir, &src).await? {
        info!(
            "Backup {src:?} has been restored on this node already - ignoring the restore request. \
            Remove it, or remove {} to restore the same backup again.",
            RestoredMarker::path(data_dir)
        );
        let aside = path_pre_restore(data_dir);
        if fs::try_exists(&aside).await? {
            info!("Removing the previous state in {aside}, it was kept after the restore");
            fs::remove_dir_all(&aside).await?;
        }
        return Ok(RestoreStart::None);
    }

    if node_config.node_id == 1 {
        restore_backup(node_config, src.clone()).await?;
        return Ok(RestoreStart::Applied(Some(src)));
    }

    let mut lock = RestoreLock::acquire(data_dir).await?;
    RestoredMarker::remove(data_dir).await?;
    if node_config.backup_restore_keep_previous {
        // Node 1 has not even looked at the backup yet, and it may still fail to apply it. The
        // state is only moved aside, and removed once the restored cluster has taken this node in.
        warn!("Moving existing state aside and starting restore cluster join");
        move_state_aside(data_dir).await?;
    } else {
        warn!("Cleaning up existing files and start restore cluster join");
        cleanup_restore_data_dirs(data_dir).await?;
    }
    lock.state_replaced();

    Ok(RestoreStart::Rejoin(src))
}

/// The record of the last finished restore of a node: the restore source, in the form of the
/// `HQL_BACKUP_RESTORE` env var, see `RestoredMarker::identity`. It lets a node that is started again with the same restore
/// request skip it, instead of applying the backup again and losing everything written since.
struct RestoredMarker;

impl RestoredMarker {
    fn path(data_dir: &str) -> String {
        format!(
            "{}/restored_backup",
            StateMachineSqlite::path_base(data_dir)
        )
    }

    /// What identifies a restore: the source, and for a local file also its size and
    /// modification time, so a new backup copied over the same path is applied. An S3 object
    /// name carries its timestamp already.
    async fn identity(src: &BackupSource) -> String {
        let request = src.as_request();
        match src {
            #[cfg(feature = "s3")]
            BackupSource::S3(_) => request,
            BackupSource::File(path) => match fs::metadata(path).await {
                Ok(meta) => {
                    let modified = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .unwrap_or_default();
                    format!("{request}\n{} {}", meta.len(), modified.as_nanos())
                }
                Err(_) => request,
            },
        }
    }

    async fn matches(data_dir: &str, src: &BackupSource) -> Result<bool, Error> {
        match fs::read_to_string(Self::path(data_dir)).await {
            Ok(restored) => Ok(restored == Self::identity(src).await),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    async fn write(data_dir: &str, src: &BackupSource) -> Result<(), Error> {
        let path = Self::path(data_dir);
        let tmp = format!("{path}.tmp");
        fs::write(&tmp, Self::identity(src).await).await?;
        atomic_file_switch(tmp, path).await
    }

    async fn remove(data_dir: &str) -> Result<(), Error> {
        match fs::remove_file(Self::path(data_dir)).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

/// The state machine lock of a `data_dir`, held while a restore changes its state.
///
/// It is the same lock file and the same advisory lock the state machine itself holds while it
/// runs, so a restore never touches the state of a live node. The lock file doubles as a crash
/// marker: a restore that created it removes it again, and one that found it removes it only once
/// the state it belongs to has been replaced.
struct RestoreLock {
    path: String,
    remove_on_drop: bool,
    _file: std::fs::File,
}

impl RestoreLock {
    async fn acquire(data_dir: &str) -> Result<Self, Error> {
        fs::create_dir_all(StateMachineSqlite::path_base(data_dir)).await?;
        let path = StateMachineSqlite::path_lock_file(data_dir);
        let existed = fs::try_exists(&path).await?;
        let file = {
            let path = path.clone();
            task::spawn_blocking(move || StateMachineSqlite::try_lock_file(&path)).await??
        };
        Ok(Self {
            path,
            remove_on_drop: !existed,
            _file: file,
        })
    }

    /// The state the crash marker belongs to is gone, the marker goes with it on release.
    fn state_replaced(&mut self) {
        self.remove_on_drop = true;
    }
}

impl Drop for RestoreLock {
    fn drop(&mut self) {
        if self.remove_on_drop
            && let Err(err) = std::fs::remove_file(&self.path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            error!("Cannot remove SQLite crash marker {}: {err}", self.path);
        }
    }
}

/// Every state directory under `data_dir` that must not survive a backup restore: the SQLite DB
/// (including its WAL sidecars), raft snapshots, the raft WAL and the cache data + its WAL (only
/// with the `cache` feature).
fn restore_state_dirs(data_dir: &str) -> Vec<String> {
    vec![
        StateMachineSqlite::path_db(data_dir),
        StateMachineSqlite::path_snapshots(data_dir),
        logs::logs_dir_db(data_dir),
        #[cfg(feature = "cache")]
        StateMachineMemory::path_sm(data_dir),
        #[cfg(feature = "cache")]
        logs::logs_dir_cache(data_dir),
    ]
}

/// Where a node other than 1 keeps the state it had before it joined a restored cluster, with
/// `backup_restore_keep_previous`. The
/// directory has the layout of a `data_dir` itself.
pub(crate) fn path_pre_restore(data_dir: &str) -> String {
    format!("{data_dir}/pre_restore")
}

/// Move the state that must not survive a restore into `path_pre_restore`.
///
/// The state is first collected in a temporary directory, which is renamed into place as the last
/// step, so an interrupted move is picked up again by the next start instead of being mistaken for
/// a finished one. When a finished one already exists, the restore it was made for has not been
/// confirmed yet: it still holds the state from before that restore, and the current state is
/// only what the node built while it tried to join. That one is cleared, the older one is kept.
async fn move_state_aside(data_dir: &str) -> Result<(), Error> {
    let aside = path_pre_restore(data_dir);
    if fs::try_exists(&aside).await? {
        warn!(
            "{aside} still holds the state from before an earlier restore - keeping it and \
            clearing the current state instead"
        );
        return cleanup_restore_data_dirs(data_dir).await;
    }

    let staging = format!("{aside}.tmp");
    let crash_marker = StateMachineSqlite::path_lock_file(data_dir);
    let sources = restore_state_dirs(data_dir);
    let targets = restore_state_dirs(&staging);
    for (src, target) in sources.iter().zip(targets.iter()) {
        if !fs::try_exists(src).await? {
            continue;
        }
        if let Some(parent) = Path::new(target).parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::rename(src, target).await.map_err(|err| {
            Error::Sqlite(format!("Cannot move {src} aside to {target}: {err}").into())
        })?;
    }
    // A crash marker belongs to the state it was found with. It stays where it is until the
    // restore lock is released, so the state that was moved aside gets a copy of it.
    if fs::try_exists(&crash_marker).await? {
        let marker = StateMachineSqlite::path_lock_file(&staging);
        if let Some(parent) = Path::new(&marker).parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&marker, b"").await?;
    }
    if !fs::try_exists(&staging).await? {
        info!("No existing state found to move aside");
        return Ok(());
    }

    fs::rename(&staging, &aside).await?;
    #[cfg(unix)]
    fs::File::open(data_dir).await?.sync_all().await?;
    warn!(
        "The previous state of this node has been moved to {aside}. It is removed as soon as \
        this node is a member of the restored cluster."
    );

    Ok(())
}

/// Remove the state that was moved aside by `move_state_aside`, as soon as this node is a member
/// of the cluster it joined and has applied the state replicated from its leader. From then on,
/// that cluster holds everything this node needs, and the old state is never read again.
///
/// If the Raft stops before that, the state stays where it is.
pub(crate) async fn discard_pre_restore_state(
    state: Arc<AppState>,
    data_dir: String,
    src: BackupSource,
) {
    let aside = path_pre_restore(&data_dir);
    let mut rx = state.raft_db.raft.metrics();
    loop {
        let joined = {
            let metrics = rx.borrow_watched();
            metrics.current_leader.is_some()
                && metrics.last_applied.is_some()
                && metrics
                    .membership_config
                    .membership()
                    .get_node(&state.id)
                    .is_some()
        };
        if joined {
            break;
        }
        if rx.changed().await.is_err() {
            warn!("Raft stopped before the restored cluster took this node in - keeping {aside}");
            return;
        }
    }

    // The marker goes first: a start in between skips the restore and removes what is left aside.
    if let Err(err) = RestoredMarker::write(&data_dir, &src).await {
        error!("Cannot record the finished restore of {src:?}: {err}");
        return;
    }
    match fs::remove_dir_all(&aside).await {
        Ok(()) => info!("Joined the restored cluster - removed the previous state in {aside}"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => error!("Cannot remove the previous state in {aside}: {err}"),
    }
}

/// Delete all state under `data_dir` that must not survive a backup restore, see
/// `restore_state_dirs`. Only these known paths are removed - never `data_dir` itself, which may
/// be a direct container mount that cannot be deleted, and never anything unknown (e.g. the staged
/// backup in `state_machine/backups`). Missing paths are fine; any other error is propagated, so a
/// failed cleanup can never silently leave stale state behind. The crash marker is removed by the
/// `RestoreLock` that guards the cleanup.
async fn cleanup_restore_data_dirs(data_dir: &str) -> Result<(), Error> {
    for path in restore_state_dirs(data_dir) {
        match fs::remove_dir_all(&path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(Error::Sqlite(
                    format!("Cannot remove old data dir {path}: {err}").into(),
                ));
            }
        }
    }

    Ok(())
}

async fn stage_local_backup(source: String, directory: &str) -> Result<String, Error> {
    let path = format!("{directory}/restore-{}.tmp", uuid::Uuid::now_v7());
    task::spawn_blocking(move || {
        if !std::fs::metadata(&source)?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "backup source must be a regular file",
            ));
        }
        let mut source = std::fs::File::open(source)?;
        let mut staged = std::fs::File::create_new(&path)?;
        if let Err(err) = std::io::copy(&mut source, &mut staged) {
            drop(staged);
            let _ = std::fs::remove_file(&path);
            return Err(err);
        }
        Ok(path)
    })
    .await?
    .map_err(Error::from)
}

/// Apply the given backup and move it onto the node's database path.
///
/// **CAUTION: This function MUST BE CALLED when the Raft is not running!**
///
/// You only need this if you apply a backup in another way than `start_node_from_backup` (or the
/// `HQL_BACKUP_RESTORE` env var), which both do it for you at startup.
///
/// The backup is fetched and validated next to the current state, which stays untouched until
/// the validated copy is durable under its pending name. From then on, an interrupted restore is
/// finished by the next start, with or without a restore request, so a crash at any point leaves
/// either the previous state or the backup, never neither.
pub async fn restore_backup(node_config: &NodeConfig, src: BackupSource) -> Result<(), Error> {
    info!("Starting database restore from backup {:?}", src);
    let mut lock = RestoreLock::acquire(&node_config.data_dir).await?;

    #[cfg(feature = "s3")]
    if let BackupSource::S3(_) = &src
        && node_config.s3_config.is_none()
    {
        return Err(Error::S3(
            "No `S3Config` given, cannot restore backup".to_string(),
        ));
    }

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
            let path_backup = format!("{path_backups}/restore-{}.tmp", uuid::Uuid::now_v7());
            if let Err(err) = s3_config.pull(&s3_obj, &path_backup).await {
                let _ = fs::remove_file(&path_backup).await;
                return Err(err);
            }
            path_backup
        }
        BackupSource::File(path_src) => stage_local_backup(path_src, &path_backups).await?,
    };
    let preparation = async {
        sync_file_data(&path_backup).await?;
        validate_backup_db(path_backup.clone()).await
    }
    .await;
    if let Err(err) = preparation {
        let _ = fs::remove_file(&path_backup).await;
        return Err(err);
    }
    debug!("Database backup validation passed (metadata + integrity check)");

    // The state is about to change, a record of an earlier restore no longer describes it.
    RestoredMarker::remove(&node_config.data_dir).await?;
    atomic_file_switch(&path_backup, path_pending_restore(&node_config.data_dir)).await?;

    apply_pending_restore(node_config, &mut lock).await
}

/// Where a validated backup waits until it replaced the database. While this file exists, a
/// restore has been started and not finished yet.
fn path_pending_restore(data_dir: &str) -> String {
    format!(
        "{}/{BACKUP_DB_NAME}",
        StateMachineSqlite::path_backups(data_dir)
    )
}

/// Replace the current state with the validated backup waiting in `path_pending_restore`.
///
/// Every step can be repeated: the pending backup only disappears with the final rename onto the
/// database path, and until then the next start runs this again.
async fn apply_pending_restore(
    node_config: &NodeConfig,
    lock: &mut RestoreLock,
) -> Result<(), Error> {
    let path_pending = path_pending_restore(&node_config.data_dir);
    let path_db = StateMachineSqlite::path_db(&node_config.data_dir);

    debug!("Removing old data");
    // Old dirs are removed before the backup is in its final place, so a partly applied backup
    // can never meet still existing old WAL data. The pending backup in `state_machine/backups`
    // is kept until the very last step.
    cleanup_restore_data_dirs(&node_config.data_dir).await?;
    lock.state_replaced();

    fs::create_dir_all(&path_db).await?;
    set_path_access(&path_db, 0o700).await?;

    let path_db_full = format!("{}/{}", path_db, node_config.filename_db);
    info!(
        "Given backup check ok - moving into its final place: {} -> {}",
        path_pending, path_db_full
    );
    atomic_file_switch(&path_pending, &path_db_full).await?;
    set_path_access(&path_db_full, 0o700).await?;
    #[cfg(unix)]
    fs::File::open(StateMachineSqlite::path_backups(&node_config.data_dir))
        .await?
        .sync_all()
        .await?;

    Ok(())
}

pub(crate) async fn validate_backup_db(path_db: String) -> Result<(), Error> {
    if env::var("HQL_BACKUP_SKIP_VALIDATION").as_deref() == Ok("true") {
        return Ok(());
    }
    validate_db_backup_snapshot(path_db).await
}

/// Finish a restore on node 1 once its Raft is up: it takes a snapshot of the restored database
/// and purges the logs below it, so the log of the restored cluster never starts fresh. Then the
/// source of the restore, if a request named one, is recorded with `RestoredMarker`.
#[tracing::instrument(level = "debug", skip_all)]
#[cfg(feature = "backup")]
pub async fn restore_backup_finish(
    state: &Arc<AppState>,
    data_dir: &str,
    src: Option<&BackupSource>,
) {
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

    if let Some(src) = src
        && let Err(err) = RestoredMarker::write(data_dir, src).await
    {
        error!("Cannot record the finished restore of {src:?}: {err}");
    }

    info!("restore_backup_finish task successful");
}

/// What this node knows about its backups, for `GET /backup` and `/metrics`.
///
/// Each node creates its own local backup when a backup entry is applied, and the last local
/// backup and failure live in memory: after a restart the newest local backup is still found on
/// disk. With `s3`, only the node that requested the backup (the leader) pushes it, but every node
/// knows the object name from the log entry, so every other node watches the bucket until the
/// object shows up. The newest backup seen on S3 is persisted next to the local backups, so all
/// nodes report it, and it survives a restart.
#[derive(Debug)]
pub(crate) struct BackupStatus {
    inner: std::sync::Mutex<BackupStatusInner>,
    #[cfg(feature = "s3")]
    s3_latest_path: std::path::PathBuf,
    /// Serializes writes of `s3_latest_path`.
    #[cfg(feature = "s3")]
    persist_lock: tokio::sync::Mutex<()>,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub(crate) struct BackupStatusInner {
    pub last_backup: Option<BackupEvent>,
    pub last_backup_error: Option<BackupEvent>,
    /// The newest backup this node has seen on S3, see [`S3Backup`].
    #[cfg(feature = "s3")]
    pub s3_latest: Option<S3Backup>,
    /// The last backup this node could not confirm on S3: an upload that failed for good on the
    /// node that pushes it, or a backup that did not show up in the bucket in time on any other.
    #[cfg(feature = "s3")]
    pub s3_error: Option<BackupEvent>,
}

/// One backup outcome: when it finished (unix seconds) and the backup file name, plus the error
/// message for a failure.
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

/// The newest backup a node has seen on S3.
#[cfg(feature = "s3")]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct S3Backup {
    /// The object name, `{prefix}_{node_id}_{ts}.sqlite`.
    pub object: String,
    /// The timestamp in the object name: when the backup was requested.
    pub ts: i64,
    /// When this node confirmed the object (unix seconds).
    pub confirmed_at: i64,
}

/// The file in the backups dir that keeps [`S3Backup`] across restarts.
#[cfg(feature = "s3")]
const S3_LATEST_FILE: &str = "s3_latest.json";

/// How long a node that did not push a backup keeps watching the bucket for it. The pushing node
/// retries its upload for a few minutes on its own, and a big database takes a while to encrypt and
/// upload, so this is generous on purpose.
#[cfg(feature = "s3")]
pub(crate) const S3_CONFIRM_TIMEOUT: Duration = Duration::from_hours(1);

impl BackupStatus {
    /// Builds an empty status for the backups in `backup_path`. With `s3`, follow it up with
    /// [`Self::load_s3_latest`].
    pub(crate) fn new(backup_path: &str) -> Self {
        #[cfg(not(feature = "s3"))]
        let _ = backup_path;

        Self {
            inner: std::sync::Mutex::new(BackupStatusInner::default()),
            #[cfg(feature = "s3")]
            s3_latest_path: Path::new(backup_path).join(S3_LATEST_FILE),
            #[cfg(feature = "s3")]
            persist_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Loads the newest S3 backup this node persisted before it was restarted. A missing or
    /// unreadable file only means it is not known yet; it never stops the node from starting.
    #[cfg(feature = "s3")]
    pub(crate) async fn load_s3_latest(&self) {
        let path = &self.s3_latest_path;
        let latest = match fs::read(path).await {
            Ok(bytes) => match serde_json::from_slice::<S3Backup>(&bytes) {
                Ok(latest) => latest,
                Err(err) => {
                    warn!("Ignoring unreadable {}: {err}", path.display());
                    return;
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => {
                warn!("Cannot read {}: {err}", path.display());
                return;
            }
        };
        self.update(|s| s.s3_latest = Some(latest));
    }

    fn update<R>(&self, f: impl FnOnce(&mut BackupStatusInner) -> R) -> R {
        // a poisoned lock only means a panic happened while recording, the data is still usable
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut inner)
    }

    pub(crate) fn backup_ok(&self, file: &str) {
        self.update(|s| s.last_backup = Some(BackupEvent::now(file, None)));
    }

    pub(crate) fn backup_failed(&self, file: &str, err: &Error) {
        self.update(|s| {
            s.last_backup_error = Some(BackupEvent::now(file, Some(err.to_string())));
        });
    }

    /// Records `object` as being on S3 and persists it, unless a newer backup is known already.
    #[cfg(feature = "s3")]
    pub(crate) async fn s3_confirmed(&self, object: &str, ts: i64) {
        let latest = S3Backup {
            object: object.to_string(),
            ts,
            confirmed_at: Utc::now().timestamp(),
        };
        let is_newer = self.update(|s| {
            if s.s3_latest.as_ref().is_some_and(|l| l.ts >= ts) {
                return false;
            }
            s.s3_latest = Some(latest.clone());
            true
        });
        if !is_newer {
            return;
        }

        // Two confirmations may race here. The lock keeps their writes apart, and each one writes
        // whatever is newest by the time it holds it, so an older backup never ends up on disk.
        let _guard = self.persist_lock.lock().await;
        let Some(latest) = self.get().s3_latest else {
            return;
        };
        if let Err(err) = self.persist_s3_latest(&latest).await {
            warn!(
                "Cannot persist {}: {err} - the newest S3 backup is reported until the next \
                 restart only",
                self.s3_latest_path.display()
            );
        }
    }

    #[cfg(feature = "s3")]
    pub(crate) fn s3_failed(&self, object: &str, err: &str) {
        self.update(|s| {
            s.s3_error = Some(BackupEvent::now(object, Some(err.to_string())));
        });
    }

    /// Writes into a temp file and moves it into place, so a crash never leaves half a file.
    #[cfg(feature = "s3")]
    async fn persist_s3_latest(&self, latest: &S3Backup) -> std::io::Result<()> {
        let tmp = self.s3_latest_path.with_extension("json~");
        fs::write(&tmp, serde_json::to_vec(latest)?).await?;
        fs::rename(&tmp, &self.s3_latest_path).await
    }

    pub(crate) fn get(&self) -> BackupStatusInner {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Whether `object` exists in the configured bucket, with a single `HEAD` request. A `404` means
/// "not there (yet)"; any other failure is an error.
#[cfg(feature = "s3")]
async fn s3_has_object(s3: &S3Config, object: &str) -> Result<bool, Error> {
    match s3.bucket.head(object).await {
        Ok(_) => Ok(true),
        Err(crate::s3::S3Error::HttpFailWithBody(404, _)) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

/// Watches the bucket on a node that did not push the backup itself, until `object` shows up or
/// [`S3_CONFIRM_TIMEOUT`] has passed since the backup was requested at `ts`.
#[cfg(feature = "s3")]
pub(crate) async fn await_s3_backup(
    s3: Arc<S3Config>,
    status: Arc<BackupStatus>,
    object: String,
    ts: i64,
) {
    let timeout = i64::try_from(S3_CONFIRM_TIMEOUT.as_secs()).unwrap_or(i64::MAX);
    let mut wait = Duration::from_secs(10);
    loop {
        time::sleep(wait).await;
        match s3_has_object(&s3, &object).await {
            Ok(true) => {
                debug!("Backup {object} confirmed on S3");
                status.s3_confirmed(&object, ts).await;
                return;
            }
            Ok(false) => {}
            Err(err) => debug!("Cannot check S3 for backup {object}: {err}"),
        }
        if Utc::now().timestamp() - ts >= timeout {
            warn!(
                "Backup {object} did not show up on S3 within {} minutes",
                timeout / 60
            );
            status.s3_failed(
                &object,
                &format!("not found on S3 within {} minutes", timeout / 60),
            );
            return;
        }
        wait = (wait * 2).min(Duration::from_mins(5));
    }
}

/// Fills in the newest backup on S3 for a node that has none persisted yet, e.g. a node that was
/// added after the last backup, or one upgraded from a version without `s3_latest.json`. It only
/// runs once: after that, every backup entry the node applies keeps it up to date. The listing is
/// limited to this node's backup prefix, so it never walks the rest of the bucket.
#[cfg(feature = "s3")]
pub(crate) async fn seed_s3_latest(s3: Arc<S3Config>, status: Arc<BackupStatus>, prefix: String) {
    if status.get().s3_latest.is_some() {
        return;
    }
    let list = match s3.bucket.list(&format!("{prefix}_"), None).await {
        Ok(list) => list,
        Err(err) => {
            warn!("Cannot list S3 to find the newest backup: {err}");
            return;
        }
    };
    let newest = list
        .iter()
        .filter(|b| b.name == s3.bucket.name)
        .flat_map(|b| &b.contents)
        .filter_map(|o| backup_ts_from_name(&o.key, &prefix).map(|ts| (ts, &o.key)))
        .max_by_key(|(ts, _)| *ts);
    if let Some((ts, object)) = newest {
        status.s3_confirmed(object, ts).await;
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
    /// The same information in the Prometheus text format, for `/metrics`. A timestamp of `0`
    /// means "never since this node started" (or "none" for the local and the S3 one), so an alert on
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
                "hiqlite_backup_s3_latest_timestamp_seconds",
                "Timestamp in the name of the newest backup this node has seen on S3, 0 if none.",
                self.status.s3_latest.as_ref().map_or(0, |l| l.ts),
            );
            gauge(
                "hiqlite_backup_s3_last_error_timestamp_seconds",
                "When this node last failed to confirm a backup on S3 since it started, 0 if never.",
                ts(&self.status.s3_error),
            );
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::{backup_file_name, backup_ts_from_name, is_valid_file_prefix};

    #[tokio::test]
    async fn local_restore_stages_a_separate_copy_of_a_read_only_backup() {
        let directory = std::env::temp_dir().join(uuid::Uuid::now_v7().to_string());
        std::fs::create_dir(&directory).unwrap();
        let source = directory.join("backup.sqlite");
        std::fs::write(&source, b"backup").unwrap();
        let original = std::fs::metadata(&source).unwrap().permissions();
        let mut permissions = original.clone();
        permissions.set_readonly(true);
        std::fs::set_permissions(&source, permissions).unwrap();

        let staged = super::stage_local_backup(
            source.to_str().unwrap().to_owned(),
            directory.to_str().unwrap(),
        )
        .await
        .unwrap();
        super::sync_file_data(&staged).await.unwrap();
        assert_ne!(std::path::Path::new(&staged), source.as_path());
        assert_eq!(std::fs::read(staged).unwrap(), b"backup");
        assert_eq!(std::fs::read(&source).unwrap(), b"backup");
        std::fs::set_permissions(source, original).unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn a_restore_removes_a_crash_marker_once_the_state_is_replaced() {
        let dir = "test_data/restore_cleanup_crash_marker";
        let path = super::StateMachineSqlite::path_lock_file(dir);
        let path = std::path::Path::new(&path);
        super::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        super::fs::write(path, b"").await.unwrap();

        // a restore that fails before it replaced anything keeps the marker of the old state
        drop(super::RestoreLock::acquire(dir).await.unwrap());
        assert!(super::fs::try_exists(path).await.unwrap());

        let mut lock = super::RestoreLock::acquire(dir).await.unwrap();
        super::cleanup_restore_data_dirs(dir).await.unwrap();
        lock.state_replaced();
        drop(lock);
        assert!(!super::fs::try_exists(path).await.unwrap());

        // a restore that created the lock file never leaves it behind
        drop(super::RestoreLock::acquire(dir).await.unwrap());
        assert!(!super::fs::try_exists(path).await.unwrap());
    }

    /// The newest S3 backup survives a restart, and an older confirmation, e.g. a slow watcher
    /// of an earlier backup, never moves it back.
    #[cfg(feature = "s3")]
    #[tokio::test]
    async fn the_newest_s3_backup_is_persisted_and_only_moves_forward() {
        use super::BackupStatus;

        let dir = "test_data/backup_status_s3_latest";
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();

        let status = BackupStatus::new(dir);
        status.load_s3_latest().await;
        assert!(status.get().s3_latest.is_none());

        status.s3_confirmed("bkp_1_200.sqlite", 200).await;
        status.s3_confirmed("bkp_1_100.sqlite", 100).await;
        assert_eq!(status.get().s3_latest.unwrap().object, "bkp_1_200.sqlite");

        let restarted = BackupStatus::new(dir);
        restarted.load_s3_latest().await;
        let latest = restarted.get().s3_latest.unwrap();
        assert_eq!(latest.object, "bkp_1_200.sqlite");
        assert_eq!(latest.ts, 200);

        restarted.s3_failed("bkp_1_300.sqlite", "not found");
        assert_eq!(restarted.get().s3_latest.unwrap().ts, 200);
        assert_eq!(restarted.get().s3_error.unwrap().file, "bkp_1_300.sqlite");

        // a broken file is ignored, it never stops the node from starting
        std::fs::write(format!("{dir}/s3_latest.json"), b"{").unwrap();
        let broken = BackupStatus::new(dir);
        broken.load_s3_latest().await;
        assert!(broken.get().s3_latest.is_none());

        std::fs::remove_dir_all(dir).unwrap();
    }

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
