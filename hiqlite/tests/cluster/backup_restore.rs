use crate::execute_query::TestData;
use crate::start::build_config;
use crate::{backup, log, Cache};
use hiqlite::{params, start_node_with_cache, Client, Error, Param};
use std::env;
use tokio::task;

pub async fn start_test_cluster_with_backup() -> Result<(Client, Client, Client), Error> {
    let path = backup::find_backup_file(1).await;
    let (_path, backup_name) = path.rsplit_once('/').unwrap();
    env::set_var("HQL_BACKUP_RESTORE", backup_name);

    let handle_client_2 = task::spawn(start_node_with_cache::<Cache>(build_config(2).await));
    let handle_client_3 = task::spawn(start_node_with_cache::<Cache>(build_config(3).await));
    let handle_client_1 = task::spawn(start_node_with_cache::<Cache>(build_config(1).await));

    let client_1 = handle_client_1.await??;
    let client_2 = handle_client_2.await??;
    let client_3 = handle_client_3.await??;

    env::remove_var("HQL_BACKUP_RESTORE");

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
