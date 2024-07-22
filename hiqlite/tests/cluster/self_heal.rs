use crate::execute_query::TestData;
use crate::start::build_config;
use crate::{check, debug, log, TEST_DATA_DIR};
use hiqlite::{params, start_node, DbClient, Error, Param};
use std::time::Duration;
use tokio::{fs, time};

pub async fn test_self_healing(
    mut client_1: DbClient,
    mut client_2: DbClient,
    mut client_3: DbClient,
) -> Result<(), Error> {
    check::is_client_db_healthy(&client_1).await?;
    check::is_client_db_healthy(&client_2).await?;
    check::is_client_db_healthy(&client_3).await?;

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

async fn shutdown_remove_sm_db_restart(client: DbClient, node_id: u64) -> Result<DbClient, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;
    time::sleep(Duration::from_millis(150)).await;

    let dir_sm_db = format!("{}/node_{}/state_machine/db", TEST_DATA_DIR, node_id);
    log(format!("Deleting {}", dir_sm_db));
    fs::remove_dir_all(dir_sm_db).await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node(build_config(node_id).await, true).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

async fn choose_leader_client<'a>(
    client_1: &'a DbClient,
    client_2: &'a DbClient,
    client_3: &'a DbClient,
) -> Result<(u64, &'a DbClient), Error> {
    if is_leader(client_1, 1).await? {
        Ok((1, client_1))
    } else if is_leader(client_2, 2).await? {
        Ok((2, client_2))
    } else {
        Ok((3, client_3))
    }
}

async fn choose_non_leader_client<'a>(
    client_1: &'a DbClient,
    client_2: &'a DbClient,
) -> Result<(u64, &'a DbClient), Error> {
    if !is_leader(client_1, 1).await? {
        Ok((1, client_1))
    } else {
        Ok((2, client_2))
    }
}

async fn is_leader(client: &DbClient, node_id: u64) -> Result<bool, Error> {
    if let Some(leader) = client.metrics().await?.current_leader {
        Ok(leader == node_id)
    } else {
        Err(Error::LeaderChange("No leader exists right now".into()))
    }
}
