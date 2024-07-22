use crate::store::logs;
use crate::store::state_machine::sqlite::state_machine::{
    PathBackups, PathDb, PathLockFile, PathSnapshots, StateMachineData, StateMachineSqlite,
};
use crate::{Error, NodeConfig};
use std::env;
use tokio::{fs, task};
use tracing::{debug, info, warn};

/// Check if the env var `HIQLITE_BACKUP_RESTORE` is set and restores the given backup if so.
pub(crate) async fn check_restore_apply(node_config: &NodeConfig) -> Result<(), Error> {
    if let Ok(name) = env::var("HIQLITE_BACKUP_RESTORE") {
        warn!(
            "Found HIQLITE_BACKUP_RESTORE={} - starting backup restore process",
            name
        );
        restore_backup(node_config, &name).await?;
    }
    Ok(())
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

    let path_base = StateMachineSqlite::path_base(&node_config.data_dir);
    let (
        PathDb(path_db),
        PathBackups(path_backups),
        PathSnapshots(path_snapshots),
        PathLockFile(path_lock_file),
    ) = StateMachineSqlite::build_folders(&node_config.data_dir, false).await;
    let path_logs = logs::logs_dir(&node_config.data_dir);

    let path_backup_s3 = format!("{}/restore.sqlite", path_base);
    s3_config.pull(backup_name, &path_backup_s3).await?;

    is_metadata_ok(path_backup_s3.clone()).await?;
    debug!("Database backup metadata is ok");

    debug!("Removing old data");
    let _ = fs::remove_dir_all(&path_db).await;
    let _ = fs::remove_dir_all(&path_backups).await;
    let _ = fs::remove_dir_all(&path_snapshots).await;
    let _ = fs::remove_dir_all(&path_lock_file).await;
    let _ = fs::remove_dir_all(&path_logs).await;

    fs::create_dir_all(&path_db).await?;
    let path_db_full = format!("{}/{}", path_db, &node_config.filename_db);
    debug!(
        "Copy Database backup in place from {} to {}",
        path_backup_s3, path_db_full
    );
    let bytes = fs::copy(&path_backup_s3, path_db_full).await?;
    assert!(bytes > 0);
    info!("Database backup copied {} bytes", bytes);

    debug!("Removing database temp file {}", path_backup_s3);
    fs::remove_file(&path_backup_s3).await?;

    info!("Database backup restore finished");

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
