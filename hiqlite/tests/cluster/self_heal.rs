use crate::start::build_config;
use crate::{cache, check, log, Cache, TEST_DATA_DIR};
use futures_util::future::join_all;
use hiqlite::{start_node_with_cache, Client, Error};
use std::time::Duration;
use tokio::{fs, time};

pub async fn test_self_healing(
    mut client_1: Client,
    mut client_2: Client,
    client_3: Client,
) -> Result<(), Error> {
    check::is_client_db_healthy(&client_1, Some(1)).await?;
    check::is_client_db_healthy(&client_2, Some(2)).await?;
    check::is_client_db_healthy(&client_3, Some(3)).await?;

    log("Test cache recovery from snapshot + logs");
    time::sleep(Duration::from_secs(2)).await;
    let metrics = client_1.metrics_cache().await?;
    assert!(metrics.last_log_index.unwrap() > 5);
    if !is_leader(&client_1, 1).await? {
        client_1 = modify_cache_restart_after_purge(client_1, 1).await?;
    } else {
        client_2 = modify_cache_restart_after_purge(client_2, 2).await?;
    };
    check::is_client_db_healthy(&client_1, Some(1)).await?;
    check::is_client_db_healthy(&client_2, Some(2)).await?;
    check::is_client_db_healthy(&client_3, Some(3)).await?;
    log("Client has self-healed successfully");

    log("Test recovery in case of state machine crash on non-leader");
    time::sleep(Duration::from_secs(2)).await;
    if !is_leader(&client_1, 1).await? {
        client_1 = shutdown_lock_sm_db_restart(client_1, 1).await?;
    } else {
        client_2 = shutdown_lock_sm_db_restart(client_2, 2).await?;
    };
    check::is_client_db_healthy(&client_1, Some(1)).await?;
    check::is_client_db_healthy(&client_2, Some(2)).await?;
    check::is_client_db_healthy(&client_3, Some(3)).await?;
    log("Client has self-healed successfully");

    log("Test recovery from state machine data loss on non-leader");
    time::sleep(Duration::from_secs(2)).await;
    let client_healed = if !is_leader(&client_1, 1).await? {
        client_1 = shutdown_remove_sm_db_restart(client_1, 1).await?;
        &client_1
    } else {
        client_2 = shutdown_remove_sm_db_restart(client_2, 2).await?;
        &client_2
    };
    client_healed.wait_until_healthy_db().await;
    check::is_client_db_healthy(&client_healed, None).await?;
    check::is_client_db_healthy(&client_1, Some(1)).await?;
    check::is_client_db_healthy(&client_2, Some(2)).await?;
    check::is_client_db_healthy(&client_3, Some(3)).await?;
    log("Client has self-healed successfully");

    log("Check recovery from full volume loss");
    let client_healed = if !is_leader(&client_1, 1).await? {
        client_1 = shutdown_remove_all_restart(client_1, 1).await?;
        &client_1
    } else {
        client_2 = shutdown_remove_all_restart(client_2, 2).await?;
        &client_2
    };
    // full replication will take a few moments, vote takes a bit longer sometimes
    client_healed.wait_until_healthy_db().await;
    check::is_client_db_healthy(client_healed, None).await?;
    check::is_client_db_healthy(&client_1, Some(1)).await?;
    check::is_client_db_healthy(&client_2, Some(2)).await?;
    check::is_client_db_healthy(&client_3, Some(3)).await?;
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
    client_1.wait_until_healthy_db().await;
    check::is_client_db_healthy(&client_1, Some(1)).await?;
    log("Client has self-healed and re-joined successfully");

    join_all([
        client_1.shutdown(),
        client_2.shutdown(),
        client_3.shutdown(),
    ])
    .await;

    Ok(())
}

async fn modify_cache_restart_after_purge(client: Client, node_id: u64) -> Result<Client, Error> {
    // we want to trigger a snapshot -> insert 1000 items
    for _ in 0..1000 {
        cache::insert_test_value_cache(&client).await?;
    }
    // at this point, we have a snapshot -> insert a new value with TTL and make sure
    // everything is fine after the restart and replication
    let key = "purge_key";
    let value = "after snap value".to_string();
    let ttl = 15u64;
    client
        .put(Cache::One, key, &value, Some(ttl as i64))
        .await?;

    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node_with_cache::<Cache>(build_config(node_id).await).await?;
    time::sleep(Duration::from_millis(100)).await;

    // inside it does a `wait_until_healthy` which may vary, so we check the time left
    // using the instant later one
    check::is_client_db_healthy(&client, Some(node_id)).await?;
    // panic!("############### client id {}", node_id);

    time::sleep(Duration::from_millis(100)).await;
    let v: String = client.get(Cache::One, key).await?.unwrap();
    assert_eq!(v, value);

    // TODO for some reason this node loses the leader and gets stuck after some sleeping
    // -> is this maybe only test-related again because of weird timers handling under the hood
    //    and everything being started from the same test context?
    // -> never saw this in any "real" deployment so far
    // -> somehow, when the sleep is too long, it seems to affect the network answers and the raft
    //    appears a being stopped even when it is not?

    // // wait until the TTL for the values has expired to make sure it's gone after restart + sync
    // let mut v: Option<String> = client.get(Cache::One, key).await?;
    // while v.is_some() {
    //     time::sleep(Duration::from_millis(100)).await;
    //     v = client.get(Cache::One, key).await?;
    // }

    Ok(client)
}

async fn shutdown_lock_sm_db_restart(client: Client, node_id: u64) -> Result<Client, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;

    let path_lock_file = format!("{}/lock", folder_state_machine(node_id));
    log(format!(
        "Mocking crashed instance - lock file not correctly cleaned up {}",
        path_lock_file
    ));
    fs::File::create_new(path_lock_file).await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node_with_cache::<Cache>(build_config(node_id).await).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

async fn shutdown_remove_all_restart(client: Client, node_id: u64) -> Result<Client, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;
    time::sleep(Duration::from_secs(1)).await;

    let folder = folder_base(node_id);
    log(format!("Deleting {}", folder));
    fs::remove_dir_all(folder).await?;

    log(format!(
        "Re-starting client {} after full data deletion",
        node_id
    ));
    let client = start_node_with_cache::<Cache>(build_config(node_id).await).await?;
    time::sleep(Duration::from_millis(150)).await;

    Ok(client)
}

async fn shutdown_remove_sm_db_restart(client: Client, node_id: u64) -> Result<Client, Error> {
    log(format!("Shutting down client {}", node_id));
    client.shutdown().await?;
    // time::sleep(Duration::from_millis(200)).await;

    let folder_sm_db = format!("{}/db", folder_state_machine(node_id));
    log(format!("Deleting {}", folder_sm_db));
    fs::remove_dir_all(folder_sm_db).await?;

    log(format!("Re-starting client {}", node_id));
    let client = start_node_with_cache::<Cache>(build_config(node_id).await).await?;
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
