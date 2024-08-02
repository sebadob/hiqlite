use crate::start::build_config;
use crate::{check, log, Cache, TEST_DATA_DIR};
use hiqlite::{start_node, Client, Error};
use std::time::Duration;
use tokio::{fs, time};

pub async fn test_self_healing(
    mut client_1: Client,
    mut client_2: Client,
    client_3: Client,
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
    check::is_client_db_healthy(&client_1).await?;
    check::is_client_db_healthy(&client_2).await?;
    check::is_client_db_healthy(&client_3).await?;
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
    check::is_client_db_healthy(&client_1).await?;
    check::is_client_db_healthy(&client_2).await?;
    check::is_client_db_healthy(&client_3).await?;
    log("Client has self-healed successfully");

    // TODO the auto-heal-logs feature does not work here -> implement a manual check and rebuild?
    // log("Test recovery from logs data loss on non-leader");
    // let client_healed = if !is_leader(&client_1, 1).await? {
    //     client_1 = shutdown_remove_logs_restart(client_1, 1).await?;
    //     &client_1
    // } else {
    //     client_2 = shutdown_remove_logs_restart(client_2, 2).await?;
    //     &client_2
    // };
    // // replication will take a few moments
    // time::sleep(Duration::from_millis(200)).await;
    // check::is_client_db_healthy(client_healed).await?;
    // log("Client has self-healed successfully");

    // TODO this test fails with `cache` enabled -> a valid update can never set matching to None
    log("Check recovery from full volume loss");
    let client_healed = if !is_leader(&client_1, 1).await? {
        client_1 = shutdown_remove_all_restart(client_1, 1).await?;
        &client_1
    } else {
        client_2 = shutdown_remove_all_restart(client_2, 2).await?;
        &client_2
    };
    // full replication will take a few moments, vote takes a bit longer sometimes
    time::sleep(Duration::from_millis(100)).await;
    check::is_client_db_healthy(client_healed).await?;
    check::is_client_db_healthy(&client_1).await?;
    check::is_client_db_healthy(&client_2).await?;
    check::is_client_db_healthy(&client_3).await?;
    log("Client has self-healed successfully");

    // Node 1 is a bit special, as it assumes that it will be responsible for a very first
    // start of a pristine cluster. We need to make sure that a lost volume for client 1 does work
    // properly in a way, that it would not create its own node but join the existing cluster.
    log("Check recovery from full volume loss on Node 1");

    // In most cases, client_1 is the leader at this point,
    // so we will give the others enough time to vote a new leader.
    client_1 = shutdown_remove_all_restart(client_1, 1).await?;
    // full replication will take a few moments, vote takes a bit longer sometimes
    log("Waiting for cluster to become healthy again");
    client_1.wait_until_healthy().await;
    check::is_client_db_healthy(&client_1).await?;
    log("Client has self-healed and re-joined successfully");

    // since we took ownership, we need to shut down the clients here
    client_1.shutdown().await?;
    log("client_1 shutdown complete after self heal tests");
    client_2.shutdown().await?;
    log("client_2 shutdown complete after self heal tests");
    client_3.shutdown().await?;
    log("client_3 shutdown complete after self heal tests");

    Ok(())
}

async fn shutdown_lock_sm_db_restart(client: Client, node_id: u64) -> Result<Client, Error> {
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
    let client = start_node::<Cache>(build_config(node_id).await).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

async fn shutdown_remove_all_restart(client: Client, node_id: u64) -> Result<Client, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;
    time::sleep(Duration::from_millis(2000)).await;

    let folder = folder_base(node_id);
    log(format!("Deleting {}", folder));
    fs::remove_dir_all(folder).await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node::<Cache>(build_config(node_id).await).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

async fn shutdown_remove_sm_db_restart(client: Client, node_id: u64) -> Result<Client, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;
    time::sleep(Duration::from_millis(200)).await;

    let folder_sm_db = format!("{}/db", folder_state_machine(node_id));
    log(format!("Deleting {}", folder_sm_db));
    fs::remove_dir_all(folder_sm_db).await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node::<Cache>(build_config(node_id).await).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

// async fn shutdown_remove_logs_restart(client: DbClient, node_id: u64) -> Result<DbClient, Error> {
//     log(format!("Shutting down client {}", node_id));
//     client.shutdown().await?;
//     time::sleep(Duration::from_millis(250)).await;
//
//     let folder_logs = folder_logs(node_id);
//     log(format!("Deleting {}", folder_logs));
//     fs::remove_dir_all(folder_logs).await?;
//
//     log(format!("Re-starting client {}", node_id));
//     let client = start_node(build_config(node_id).await).await?;
//     time::sleep(Duration::from_millis(150)).await;
//
//     Ok(client)
// }

async fn is_leader(client: &Client, node_id: u64) -> Result<bool, Error> {
    if let Some(leader) = client.metrics_db().await?.current_leader {
        Ok(leader == node_id)
    } else {
        Err(Error::LeaderChange("No leader exists right now".into()))
    }
}

fn folder_base(node_id: u64) -> String {
    format!("{}/node_{}", TEST_DATA_DIR, node_id)
}

// fn folder_logs(node_id: u64) -> String {
//     format!("{}/logs", folder_base(node_id))
// }

fn folder_state_machine(node_id: u64) -> String {
    format!("{}/state_machine", folder_base(node_id))
}
