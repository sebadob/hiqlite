use crate::{TEST_DATA_DIR, log};
use hiqlite::macros::params;
use hiqlite::{Client, Error};
use std::time::Duration;
use tokio::{fs, time};

pub static BACKUP_PATH_FILE: &str = "tests/data_test/backup.sqlite";

/// The `backup_file_prefix` every test node is built with - see `start::build_config_with_nodes`.
pub const TEST_FILE_PREFIX: &str = "test_bkp";

pub async fn test_backup(client_1: &Client) -> Result<(), Error> {
    log("Creating backup request via client_1");
    client_1.backup().await?;

    // these test backups are tiny and very quick -> async background task
    time::sleep(Duration::from_millis(100)).await;

    log("Find backup DB");

    // the client will never see the backup path, so we need to
    // build it on our own in the tests
    let metrics = client_1.metrics_db().await?;
    let leader = metrics.current_leader.unwrap();
    let path = find_backup_file(leader).await;

    log("Backup file name must carry the configured prefix");
    let name = path.rsplit_once('/').map(|(_, name)| name).unwrap();
    assert!(
        name.starts_with(&format!("{TEST_FILE_PREFIX}_")),
        "backup name {name} must start with '{TEST_FILE_PREFIX}_'"
    );
    let list = client_1.backup_list_local().await?;
    assert!(
        list.iter().any(|b| b.name == name),
        "our backup must be listed: {name}"
    );

    log("Plant foreign files in the backups dir, with a timestamp older than the local retention");
    // `backup_node_...` is the default prefix: these are the files of a *different* cluster that
    // shares the directory. The local cleanup job runs after every backup and must leave them
    // alone because they do not match our prefix.
    let dir = path.rsplit_once('/').map(|(dir, _)| dir).unwrap();
    let foreign_default_prefix = format!("{dir}/backup_node_1_1700000000.sqlite");
    let foreign_not_a_backup = format!("{dir}/notes.txt");
    fs::write(&foreign_default_prefix, b"not ours").await?;
    fs::write(&foreign_not_a_backup, b"not ours").await?;

    log("Backup again to run the local cleanup job, then check the foreign files survived");
    client_1.backup().await?;
    time::sleep(Duration::from_millis(500)).await;
    assert!(
        fs::try_exists(&foreign_default_prefix).await?,
        "cleanup must not delete a file of another prefix"
    );
    assert!(
        fs::try_exists(&foreign_not_a_backup).await?,
        "cleanup must not delete a file that is no backup"
    );
    let list = client_1.backup_list_local().await?;
    assert!(
        !list.iter().any(|b| b.name == "notes.txt"),
        "a file that is no backup must not be listed"
    );
    assert!(
        !list.iter().any(|b| b.name.starts_with("backup_node_")),
        "files of another prefix must not be listed"
    );

    // copy the file into a 2nd location for later restore from file testing
    fs::copy(&path, BACKUP_PATH_FILE).await?;

    let conn_bkp = rusqlite::Connection::open(BACKUP_PATH_FILE)?;

    log("Check that a regular connection to the backup db is working");
    let res = conn_bkp.query_row("SELECT 1", [], |row| {
        let i: i64 = row.get(0)?;
        Ok(i)
    })?;
    assert_eq!(res, 1);

    // we want to remove the `_metadata` table locally to test the validation skip later on
    conn_bkp.execute("DROP TABLE _metadata", [])?;

    Ok(())
}

pub async fn test_backup_restore_prerequisites(client: &Client) -> Result<(), Error> {
    // We want to introduce changes to the current database which can be compared to the
    // backup that has been backed up right beforehand.
    client
        .execute(
            r#"
    CREATE TABLE test_changed
    (
        id INTEGER NOT NULL
             CONSTRAINT test_pk
               PRIMARY KEY
    )
    "#,
            params!(),
        )
        .await?;

    client
        .execute("INSERT INTO test_changed VALUES ($1)", params!(1337))
        .await?;

    client.execute("DROP TABLE test", params!()).await?;

    // wait for writes to finish safely before shutting down afterward immediately
    time::sleep(Duration::from_millis(100)).await;

    Ok(())
}

pub async fn find_backup_file(node_id: u64) -> String {
    let path_base = format!("{}/node_{}/state_machine/backups", TEST_DATA_DIR, node_id);
    let mut ls = fs::read_dir(&path_base).await.unwrap();

    // the dir can hold planted foreign files; only our own prefix is a test backup
    while let Some(file) = ls.next_entry().await.unwrap() {
        let file_name = file.file_name();
        let name = file_name.to_str().unwrap();
        if name.starts_with(&format!("{TEST_FILE_PREFIX}_")) {
            return format!("{}/{}", path_base, name);
        }
    }
    panic!("Backup folder is empty when it should not be");
}

// pub fn ls_recursive(path: &str, indent: usize) -> String {
//     use std::fmt::Write;
//
//     let mut ls = std::fs::read_dir(path).unwrap();
//     let mut base_indent = String::with_capacity(indent);
//     for _ in 0..indent {
//         write!(base_indent, " ").unwrap();
//     }
//
//     let mut res = String::default();
//     while let Some(Ok(entry)) = ls.next() {
//         let file_name = entry.file_name();
//         let name = file_name.to_str().unwrap();
//         writeln!(res, "{}{}", base_indent, name).unwrap();
//
//         if entry.metadata().unwrap().is_dir() {
//             let sub = ls_recursive(entry.path().to_str().unwrap(), indent + 2);
//             writeln!(res, "{}", sub).unwrap();
//         }
//     }
//
//     if res.is_empty() {
//         writeln!(res, "{}<empty>", base_indent).unwrap();
//     }
//
//     res
// }
