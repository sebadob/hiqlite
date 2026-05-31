use crate::{Cache, log};
use hiqlite::{Client, Error, Node, NodeConfig, start_node_with_cache};
use openraft::ServerState;
use std::time::Duration;
use tokio::{fs, task, time};

const TEST_DATA_DIR_LEARNER_ONLY: &str = "tests/data_test_learner_only";

#[tokio::test(flavor = "multi_thread")]
async fn learner_only_node_stays_non_voter_and_becomes_ready() {
    let _ = fs::remove_dir_all(TEST_DATA_DIR_LEARNER_ONLY).await;

    let nodes = learner_only_nodes();
    let handle_client_1 = task::spawn(start_node_with_cache::<Cache>(
        build_learner_only_config(1, nodes.clone()).await,
    ));
    let handle_client_2 = task::spawn(start_node_with_cache::<Cache>(
        build_learner_only_config(2, nodes.clone()).await,
    ));
    let handle_client_3 = task::spawn(start_node_with_cache::<Cache>(
        build_learner_only_config(3, nodes).await,
    ));

    let client_1 = handle_client_1.await.unwrap().unwrap();
    let client_2 = handle_client_2.await.unwrap().unwrap();
    let client_3 = handle_client_3.await.unwrap().unwrap();

    wait_for_node_health(&client_1).await.unwrap();
    wait_for_node_health(&client_2).await.unwrap();
    wait_for_node_health(&client_3).await.unwrap();
    wait_for_ready("127.0.0.1:35003").await.unwrap();

    assert_learner_only_membership(&client_1, 3).await.unwrap();
    assert_learner_only_membership(&client_3, 3).await.unwrap();

    join_shutdown([client_1, client_2, client_3]).await;
    let _ = fs::remove_dir_all(TEST_DATA_DIR_LEARNER_ONLY).await;
}

fn learner_only_nodes() -> Vec<Node> {
    vec![
        Node {
            id: 1,
            addr_raft: "127.0.0.1:36001".to_string(),
            addr_api: "127.0.0.1:35001".to_string(),
        },
        Node {
            id: 2,
            addr_raft: "127.0.0.1:36002".to_string(),
            addr_api: "127.0.0.1:35002".to_string(),
        },
        Node {
            id: 3,
            addr_raft: "127.0.0.1:36003".to_string(),
            addr_api: "127.0.0.1:35003".to_string(),
        },
    ]
}

async fn build_learner_only_config(node_id: u64, nodes: Vec<Node>) -> NodeConfig {
    let mut config =
        crate::start::build_config_with_nodes(node_id, nodes, TEST_DATA_DIR_LEARNER_ONLY).await;
    config.learner_only = node_id == 3;
    config
}

async fn wait_for_node_health(client: &Client) -> Result<(), Error> {
    for _ in 0..30 {
        let healthy_db = client.is_healthy_db().await;
        let healthy_cache = client.is_healthy_cache().await;
        if healthy_db.is_ok() && healthy_cache.is_ok() {
            return Ok(());
        }

        time::sleep(Duration::from_millis(500)).await;
    }

    client.is_healthy_db().await?;
    client.is_healthy_cache().await
}

async fn wait_for_ready(addr_api: &str) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let url = format!("http://{addr_api}/ready");

    for _ in 0..30 {
        if let Ok(resp) = client.get(&url).send().await
            && resp.status().is_success()
        {
            return Ok(());
        }

        time::sleep(Duration::from_millis(500)).await;
    }

    let resp = client.get(url).send().await?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(Error::Error(
            format!("learner-only node did not become ready: {}", resp.status()).into(),
        ))
    }
}

async fn assert_learner_only_membership(client: &Client, node_id: u64) -> Result<(), Error> {
    let metrics_db = client.metrics_db().await?;
    assert!(metrics_db.membership_config.nodes().any(|(id, _)| *id == node_id));
    assert!(!metrics_db.membership_config.voter_ids().any(|id| id == node_id));

    let metrics_cache = client.metrics_cache().await?;
    assert!(metrics_cache.membership_config.nodes().any(|(id, _)| *id == node_id));
    assert!(!metrics_cache.membership_config.voter_ids().any(|id| id == node_id));

    if client.metrics_db().await?.id == node_id {
        assert_eq!(metrics_db.state, ServerState::Learner);
        assert_eq!(metrics_cache.state, ServerState::Learner);
    }

    Ok(())
}

async fn join_shutdown(clients: [Client; 3]) {
    for client in clients {
        if let Err(err) = client.shutdown().await {
            log(format!("learner-only test shutdown error: {err}"));
        }
    }
}
