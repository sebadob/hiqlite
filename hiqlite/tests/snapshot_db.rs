//! `Client::snapshot_db()` builds a snapshot and purges the logs it covers, so that the log does
//! not start at ID 1 anymore.

#![cfg(feature = "sqlite")]

use hiqlite::{Node, NodeConfig, Param};
use std::time::Duration;

const PORT: u16 = 36311;

fn config(dir: &str) -> NodeConfig {
    NodeConfig {
        node_id: 1,
        nodes: vec![Node {
            id: 1,
            addr_raft: format!("127.0.0.1:{}", PORT + 1),
            addr_api: format!("127.0.0.1:{PORT}"),
        }],
        data_dir: dir.to_string().into(),
        secret_raft: "SuperSecureSecret1337".to_string(),
        secret_api: "SuperSecureSecret1337".to_string(),
        ..Default::default()
    }
}

/// The first log ID that is still in the log, which is 1 as long as nothing was purged.
async fn first_log(client: &hiqlite::Client) -> u64 {
    client
        .metrics_db()
        .await
        .unwrap()
        .purged
        .map_or(1, |id| id.index + 1)
}

/// openraft may delay a purge for a moment, so wait until the first log ID is above `above`.
async fn first_log_above(client: &hiqlite::Client, above: u64) -> u64 {
    for _ in 0..100 {
        let first = first_log(client).await;
        if first > above {
            return first;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the log still starts at or below ID {above}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_db_purges_the_first_log_id() {
    let dir = "../target/test_data/snapshot_db";
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();

    let client = hiqlite::start_node(config(dir)).await.unwrap();
    client.wait_until_healthy_db().await;
    client
        .execute("CREATE TABLE rows (id INTEGER PRIMARY KEY)", Vec::new())
        .await
        .unwrap();
    client
        .execute("INSERT INTO rows (id) VALUES ($1)", vec![Param::from(1)])
        .await
        .unwrap();
    assert_eq!(first_log(&client).await, 1);

    tokio::time::timeout(Duration::from_secs(60), client.snapshot_db())
        .await
        .expect("snapshot_db finishes")
        .unwrap();
    let first = first_log_above(&client, 1).await;
    let metrics = client.metrics_db().await.unwrap();
    assert!(
        metrics.snapshot.is_some_and(|id| id.index + 1 >= first),
        "the snapshot does not cover the purged logs: {:?}, first log {first}",
        metrics.snapshot
    );

    // a second call on a node that already has a snapshot moves the first log ID forward again
    tokio::time::timeout(Duration::from_secs(60), client.snapshot_db())
        .await
        .expect("snapshot_db finishes")
        .unwrap();
    first_log_above(&client, first).await;

    client.shutdown().await.unwrap();
}
