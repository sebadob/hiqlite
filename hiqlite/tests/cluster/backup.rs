use crate::{log, TEST_DATA_DIR};
use hiqlite::{params, Client, Error, Param};
use std::time::Duration;
use tokio::{fs, time};

pub async fn test_backup(client_1: &Client) -> Result<(), Error> {
    log("Creating backup request via client_1");
    client_1.backup().await?;

    // these test backups are tiny and very quick -> async background task
    time::sleep(Duration::from_millis(50)).await;

    log("Find backup DB");

    // the client will never see the backup path, so we need to
    // build it on our own in the tests
    let metrics = client_1.metrics_db().await?;
    let leader = metrics.current_leader.unwrap();
    let path = find_backup_file(leader).await;

    let conn_bkp = rusqlite::Connection::open(path).unwrap();

    log("Check that a regular connection to the backup db is working");
    let res = conn_bkp
        .query_row("SELECT 1", [], |row| {
            let i: i64 = row.get(0)?;
            Ok(i)
        })
        .unwrap();
    assert_eq!(res, 1);

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

    Ok(())
}

pub async fn find_backup_file(node_id: u64) -> String {
    let path_base = format!("{}/node_{}/state_machine/backups", TEST_DATA_DIR, node_id);
    let mut ls = fs::read_dir(&path_base).await.unwrap();

    if let Some(file) = ls.next_entry().await.unwrap() {
        let file_name = file.file_name();
        let name = file_name.to_str().unwrap();
        return format!("{}/{}", path_base, name);
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
