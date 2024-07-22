use crate::execute_query::TestData;
use crate::start::build_config;
use crate::{check, log, start, TEST_DATA_DIR};
use hiqlite::{params, start_node, DbClient, Error, Param};
use std::time::Duration;
use tokio::{fs, time};

pub async fn test_self_healing(
    mut client_1: DbClient,
    mut client_2: DbClient,
    client_3: DbClient,
) -> Result<(), Error> {
    check::is_client_db_healthy(&client_1).await?;
    check::is_client_db_healthy(&client_2).await?;
    check::is_client_db_healthy(&client_3).await?;

    log("Test recovery in case of state machine crash on non-leader");
    let client_healed = if !is_leader(&client_1, 1).await? {
        client_1 = shutdown_lock_sm_db_restart(client_1, 1).await?;
        &client_1
    } else {
        client_2 = shutdown_lock_sm_db_restart(client_2, 2).await?;
        &client_2
    };
    check::is_client_db_healthy(&client_healed).await?;
    log("Client has self-healed successfully");

    log("Test recovery from state machine data loss on non-leader");
    let client_healed = if !is_leader(&client_1, 1).await? {
        client_1 = shutdown_remove_sm_db_restart(client_1, 1).await?;
        &client_1
    } else {
        client_2 = shutdown_remove_sm_db_restart(client_2, 2).await?;
        &client_2
    };
    check::is_client_db_healthy(&client_healed).await?;
    log("Client has self-healed successfully");

    // TODO maybe hide this test behind feature flag and test independently ?
    // TODO this is the not-so-pretty version of the logs heal.
    // TODO when we just shut down one node, delete logs and restart, the leader keeps thinking
    // it's still offline even when it's not. -> investigate
    log("Test recovery from logs data los on non-leaders");
    // a logs recovery needs a cluster restart currently
    let folder_logs = if is_leader(&client_1, 1).await? {
        client_2.shutdown().await?;
        client_3.shutdown().await?;
        client_1.shutdown().await?;

        folder_logs(2)
    } else if is_leader(&client_1, 2).await? {
        client_1.shutdown().await?;
        client_3.shutdown().await?;
        client_2.shutdown().await?;

        folder_logs(3)
    } else {
        client_1.shutdown().await?;
        client_2.shutdown().await?;
        client_3.shutdown().await?;

        folder_logs(1)
    };
    time::sleep(Duration::from_millis(250)).await;
    log(format!("Deleting {}", folder_logs));
    fs::remove_dir_all(folder_logs).await?;

    log("Restarting whole cluster");
    let (client_1, client_2, client_3) = start::start_test_cluster().await?;
    check::is_client_db_healthy(&client_1).await?;
    check::is_client_db_healthy(&client_2).await?;
    check::is_client_db_healthy(&client_3).await?;

    log("Make sure the cluster is still working and we can insert data");
    let rows_affected = client_1
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(111, 0, "I don't care"),
        )
        .await?;
    assert_eq!(rows_affected, 1);

    let res: TestData = client_1
        .query_as_one("SELECT * FROM test WHERE id = $1", params!(111))
        .await?;
    assert_eq!(res.id, 111);
    assert_eq!(res.ts, 0);
    assert_eq!(res.description, "I don't care");

    // let client_healed = if !is_leader(&client_1, 1).await? {
    //     client_1 = shutdown_remove_logs_restart(client_1, 1).await?;
    //     &client_1
    // } else {
    //     client_2 = shutdown_remove_logs_restart(client_2, 2).await?;
    //     &client_2
    // };
    // client_healed.init().await?;

    // while let Err(err) = client_healed.is_healthy().await {
    //     let metrics = client_healed.metrics().await.unwrap();
    //     error!("{:?}\n", metrics);
    //
    //     let metrics = client_1.metrics().await.unwrap();
    //     error!("{:?}\n", metrics);
    //
    //     let metrics = client_3.metrics().await.unwrap();
    //     error!("{:?}\n", metrics);
    //
    //     time::sleep(Duration::from_millis(1000)).await;
    // }

    // client_healed.wait_until_healthy().await;
    // check::is_client_db_healthy(&client_1).await?;
    // check::is_client_db_healthy(&client_2).await?;
    // check::is_client_db_healthy(&client_3).await?;
    log("Client has self-healed successfully");

    time::sleep(Duration::from_secs(1)).await;

    // since we took ownership, we need to shut down the clients here
    client_1.shutdown().await?;
    log("client_1 shutdown complete after self heal tests");
    client_2.shutdown().await?;
    log("client_2 shutdown complete after self heal tests");
    client_3.shutdown().await?;
    log("client_3 shutdown complete after self heal tests");

    Ok(())
}

async fn shutdown_lock_sm_db_restart(client: DbClient, node_id: u64) -> Result<DbClient, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;
    time::sleep(Duration::from_millis(150)).await;

    let path_lock_file = format!("{}/lock", folder_state_machine(node_id));
    log(format!(
        "Mocking crashed instance - lock file not correctly cleaned up {}",
        path_lock_file
    ));
    fs::File::create_new(path_lock_file).await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node(build_config(node_id).await, true).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

async fn shutdown_remove_sm_db_restart(client: DbClient, node_id: u64) -> Result<DbClient, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;
    time::sleep(Duration::from_millis(150)).await;

    let folder_sm_db = format!("{}/db", folder_state_machine(node_id));
    log(format!("Deleting {}", folder_sm_db));
    fs::remove_dir_all(folder_sm_db).await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node(build_config(node_id).await, true).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

// TODO this version with just a single node is not working yet
// async fn shutdown_remove_logs_restart(client: DbClient, node_id: u64) -> Result<DbClient, Error> {
//     log(format!("Shutting down client {}", node_id));
//     client.shutdown().await?;
//     time::sleep(Duration::from_millis(3000)).await;
//
//     let folder_logs = folder_logs(node_id);
//     log(format!("Deleting {}", folder_logs));
//     fs::remove_dir_all(folder_logs).await?;
//
//     log(format!("Re-starting client {}", node_id));
//     let client = start_node(build_config(node_id).await, true).await?;
//     time::sleep(Duration::from_millis(150)).await;
//
//     Ok(client)
// }

fn folder_logs(node_id: u64) -> String {
    format!("{}/node_{}/logs", TEST_DATA_DIR, node_id)
}

fn folder_state_machine(node_id: u64) -> String {
    format!("{}/node_{}/state_machine", TEST_DATA_DIR, node_id)
}

// async fn choose_leader_client<'a>(
//     client_1: &'a DbClient,
//     client_2: &'a DbClient,
//     client_3: &'a DbClient,
// ) -> Result<(u64, &'a DbClient), Error> {
//     if is_leader(client_1, 1).await? {
//         Ok((1, client_1))
//     } else if is_leader(client_2, 2).await? {
//         Ok((2, client_2))
//     } else {
//         Ok((3, client_3))
//     }
// }
//
// async fn choose_non_leader_client<'a>(
//     client_1: &'a DbClient,
//     client_2: &'a DbClient,
// ) -> Result<(u64, &'a DbClient), Error> {
//     if !is_leader(client_1, 1).await? {
//         Ok((1, client_1))
//     } else {
//         Ok((2, client_2))
//     }
// }

async fn is_leader(client: &DbClient, node_id: u64) -> Result<bool, Error> {
    if let Some(leader) = client.metrics().await?.current_leader {
        Ok(leader == node_id)
    } else {
        Err(Error::LeaderChange("No leader exists right now".into()))
    }
}
