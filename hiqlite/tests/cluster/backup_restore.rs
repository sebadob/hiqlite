use crate::execute_query::TestData;
use crate::{backup, log, start};
use hiqlite::{params, start_node, DbClient, Error, Param};
use std::env;

pub async fn start_test_cluster_with_backup() -> Result<(DbClient, DbClient, DbClient), Error> {
    let path = backup::find_backup_file(1).await;
    let (_path, backup_name) = path.rsplit_once('/').unwrap();
    env::set_var("HIQLITE_BACKUP_RESTORE", backup_name);

    let client_3 = start_node(start::build_config(3).await, true).await?;
    let client_2 = start_node(start::build_config(2).await, true).await?;
    let client_1 = start_node(start::build_config(1).await, true).await?;

    env::remove_var("HIQLITE_BACKUP_RESTORE");

    Ok((client_1, client_2, client_3))
}

pub async fn test_db_is_healthy_after_restore(client: &DbClient) -> Result<(), Error> {
    let data = TestData {
        id: 3,
        ts: 0, // Timestamp will be different anyway, we don't care right now
        description: "My First Row from client 3".to_string(),
    };

    log("Check old data still exists");
    let res: TestData = client
        .query_as_one("SELECT * FROM test WHERE id = $1", params!(3))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.description, data.description);

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
