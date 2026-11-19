use crate::backup::BACKUP_PATH_FILE;
use crate::execute_query::TestData;
use crate::start::{TlsCombo, build_config};
use crate::{Cache, backup, log};
use hiqlite::macros::params;
use hiqlite::{
    BackupSource, Client, Error, start_node_with_cache, start_node_with_cache_from_backup,
};
use std::env;
use tokio::task;

/// How the nodes are told which backup to restore: through the `HQL_BACKUP_RESTORE` env var, or
/// through the explicit source of `start_node_with_cache_from_backup`.
pub enum Restore {
    EnvVar,
    Explicit,
}

pub async fn start_test_cluster_with_backup(
    restore: Restore,
    from_fs: bool,
    combo: TlsCombo,
) -> Result<(Client, Client, Client), Error> {
    let source = if from_fs {
        BackupSource::File(BACKUP_PATH_FILE.to_string())
    } else {
        let path = backup::find_backup_file(1).await;
        let (_path, backup_name) = path.rsplit_once('/').unwrap();
        #[cfg(feature = "s3")]
        {
            BackupSource::S3(backup_name.to_string())
        }
        #[cfg(not(feature = "s3"))]
        {
            // Without the `s3` feature there is nothing to pull from, and the file we just found
            // is right on this disk.
            let _ = backup_name;
            BackupSource::File(path)
        }
    };

    match restore {
        Restore::EnvVar => {
            if from_fs {
                unsafe {
                    env::set_var("HQL_BACKUP_SKIP_VALIDATION", "true");
                }
            }
            unsafe {
                match &source {
                    #[cfg(feature = "s3")]
                    BackupSource::S3(obj) => {
                        env::set_var("HQL_BACKUP_RESTORE", format!("s3:{}", obj));
                    }
                    BackupSource::File(path) => {
                        env::set_var("HQL_BACKUP_RESTORE", format!("file:{}", path));
                    }
                }
            }
        }
        Restore::Explicit => {
            // the whole point of the explicit source: the env var is never set and never read
            unsafe {
                env::remove_var("HQL_BACKUP_RESTORE");
            }
            if from_fs {
                unsafe {
                    env::set_var("HQL_BACKUP_SKIP_VALIDATION", "true");
                }
            }
        }
    }

    let (handle_client_2, handle_client_3, handle_client_1) = match restore {
        Restore::EnvVar => (
            task::spawn(start_node_with_cache::<Cache>(build_config(2, combo).await)),
            task::spawn(start_node_with_cache::<Cache>(build_config(3, combo).await)),
            task::spawn(start_node_with_cache::<Cache>(build_config(1, combo).await)),
        ),
        Restore::Explicit => (
            task::spawn(start_node_with_cache_from_backup::<Cache>(
                build_config(2, combo).await,
                source.clone(),
            )),
            task::spawn(start_node_with_cache_from_backup::<Cache>(
                build_config(3, combo).await,
                source.clone(),
            )),
            task::spawn(start_node_with_cache_from_backup::<Cache>(
                build_config(1, combo).await,
                source.clone(),
            )),
        ),
    };

    let client_1 = handle_client_1.await??;
    let client_2 = handle_client_2.await??;
    let client_3 = handle_client_3.await??;

    unsafe {
        env::remove_var("HQL_BACKUP_SKIP_VALIDATION");
        env::remove_var("HQL_BACKUP_RESTORE");
    }

    Ok((client_1, client_2, client_3))
}

pub async fn test_db_is_healthy_after_restore(client: &Client) -> Result<(), Error> {
    log("Check old data still exists");
    let res: TestData = client
        .query_as_one("SELECT * FROM test WHERE id = $1", params!(3))
        .await?;
    assert_eq!(res.id, 3);
    assert_eq!(res.description, None);

    log("Make sure the database changes from before the restore have been reverted");
    let res: Result<TestData, Error> = client
        .query_as_one("SELECT * FROM test_changed WHERE id = $1", params!(3))
        .await;
    assert!(res.is_err());
    if let Err(err) = res {
        match err {
            Error::Sqlite(s) => {
                log(&s);
                log(s.contains("no such table"));
            }
            _ => panic!("Should be an Error::Sqlite"),
        }
    }

    Ok(())
}
