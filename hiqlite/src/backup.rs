use crate::Error;
use std::env;

/// Check if the env var `HIQLITE_BACKUP_RESTORE` is set and restores the given backup if so.
pub(crate) async fn check_restore_apply() -> Result<(), Error> {
    if let Ok(name) = env::var("HIQLITE_BACKUP_RESTORE") {
        restore_backup(&name).await?;
    }
    Ok(())
}

/// Apply the given backup from S3 storage.
///
/// **CAUTION: This function MUST BE CALLED when the Raft is not running!**
///
/// You only need to invoke this manually if you want to apply a backup in another
/// way than with the `HIQLITE_BACKUP_RESTORE` env var, which is being done automatically.
pub async fn restore_backup(backup_name: &str) -> Result<(), Error> {
    // - try to download the backup from s3 to a temp file outside of logs / state_machine
    // - on success, connect to the DB and check that we have default `_metadata`
    // - TODO maybe write the backup name itself in the backup task to verify additionally? or maybe sha256 of the
    //   finished DB file to check integrity
    // - if DB is okay, delete all existing logs/* + state_machine/*
    // - re-create folders
    // - copy the backup DB in place of the final target DB to start clean again (will create a fresh Raft cluster)

    todo!()
}
