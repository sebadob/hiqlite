use crate::{log, Cache, TEST_DATA_DIR};
use hiqlite::{start_node_with_cache, Client, Error, Node, NodeConfig};
use std::time::Duration;
use tokio::{fs, task, time};

pub const SECRET_API: &str = "qweqweqweqweqweqwe";

pub async fn start_test_cluster() -> Result<(Client, Client, Client), Error> {
    let handle_client_1 = task::spawn(start_node_with_cache::<Cache>(build_config(1).await));
    let handle_client_2 = task::spawn(start_node_with_cache::<Cache>(build_config(2).await));
    let handle_client_3 = task::spawn(start_node_with_cache::<Cache>(build_config(3).await));

    let client_1 = handle_client_1.await??;
    let client_2 = handle_client_2.await??;
    let client_3 = handle_client_3.await??;

    Ok((client_1, client_2, client_3))
}

pub fn nodes() -> Vec<Node> {
    vec![
        Node {
            id: 1,
            addr_raft: "127.0.0.1:32001".to_string(),
            addr_api: "127.0.0.1:31001".to_string(),
        },
        Node {
            id: 2,
            addr_raft: "127.0.0.1:32002".to_string(),
            addr_api: "127.0.0.1:31002".to_string(),
        },
        Node {
            id: 3,
            addr_raft: "127.0.0.1:32003".to_string(),
            addr_api: "127.0.0.1:31003".to_string(),
        },
    ]
}

pub async fn build_config(node_id: u64) -> NodeConfig {
    let dir_1 = format!("{}/node_1", TEST_DATA_DIR);
    let dir_2 = format!("{}/node_2", TEST_DATA_DIR);
    let dir_3 = format!("{}/node_3", TEST_DATA_DIR);

    fs::create_dir_all(&dir_1).await.unwrap();
    fs::create_dir_all(&dir_2).await.unwrap();
    fs::create_dir_all(&dir_3).await.unwrap();

    let data_dir = match node_id {
        1 => dir_1.to_string().into(),
        2 => dir_2.to_string().into(),
        3 => dir_3.to_string().into(),
        _ => unreachable!(),
    };

    let mut config = NodeConfig::from_toml("../hiqlite.toml", None, None)
        .await
        .unwrap();
    config.node_id = node_id;
    config.nodes = nodes();
    config.data_dir = data_dir;
    config.log_statements = true;
    // very tiny WAL to make sure log roll-overs will happen during tests
    config.wal_size = 8 * 1024;
    config.raft_config = NodeConfig::default_raft_config(1000);

    // TODO currently we can't test with TLS, because this depends on `axum_server`.
    // This does not support graceful shutdown, which we need for testing from
    // a single process
    config.tls_raft = None;
    config.tls_api = None;

    config.secret_raft = "asdasdasdasdasdasd".to_string();
    config.secret_api = SECRET_API.to_string();

    config.backup_config = Default::default();
    config.cache_storage_disk = false;

    config
}

pub async fn wait_for_healthy_cluster(
    client_1: &Client,
    client_2: &Client,
    client_3: &Client,
) -> Result<(), Error> {
    for i in 1..=3 {
        loop {
            time::sleep(Duration::from_secs(1)).await;

            let client = match i {
                1 => client_1,
                2 => client_2,
                3 => client_3,
                _ => unreachable!(),
            };

            let healthy_db = client.is_healthy_db().await;
            let healthy_cache = client.is_healthy_cache().await;

            if healthy_db.is_ok() && healthy_cache.is_ok() {
                log(format!("Node {} is healthy", i));
                break;
            } else {
                log(format!("Waiting for Node {} to become healthy", i));
            }
        }
    }

    let metrics = client_1.metrics_db().await?;
    assert!(metrics.running_state.is_ok());
    let node_count = metrics.membership_config.membership().nodes().count();
    assert_eq!(node_count, 3);

    let metrics = client_1.metrics_cache().await?;
    assert!(metrics.running_state.is_ok());
    let node_count = metrics.membership_config.membership().nodes().count();
    assert_eq!(node_count, 3);

    Ok(())
}
