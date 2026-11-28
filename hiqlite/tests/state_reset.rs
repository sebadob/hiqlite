//! A node that joins after a Raft state reset must receive the existing data.
//!
//! `HQL_DANGER_RAFT_STATE_RESET` deletes the Raft logs, snapshots and metadata, but keeps the
//! database, and starts a new Raft from log ID 1. This is the way to shrink a cluster to a
//! single node without losing data. A node that joins later replays the leader's log from ID 1,
//! which does not contain the data that was already in the database before the reset. It only
//! receives that data through a snapshot.
//!
//! This is a separate test binary, because the env var applies to the whole process.

#![cfg(all(feature = "sqlite", feature = "backup"))]

use hiqlite::{Node, NodeConfig, Param};
use std::env;
use std::time::Duration;

const ROWS: i64 = 20;
const PORT_1: u16 = 36301;
const PORT_2: u16 = 36303;

fn fresh(case: &str) -> String {
    let dir = format!("../target/test_data/state_reset/{case}");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn node(id: u64, port_api: u16) -> Node {
    Node {
        id,
        addr_raft: format!("127.0.0.1:{}", port_api + 1),
        addr_api: format!("127.0.0.1:{port_api}"),
    }
}

fn config(node_id: u64, nodes: Vec<Node>, dir: &str) -> NodeConfig {
    NodeConfig {
        node_id,
        nodes,
        data_dir: dir.to_string().into(),
        secret_raft: "SuperSecureSecret1337".to_string(),
        secret_api: "SuperSecureSecret1337".to_string(),
        ..Default::default()
    }
}

async fn count(client: &hiqlite::Client) -> i64 {
    let mut row = client
        .query_raw_one("SELECT COUNT(*) AS c FROM rows", Vec::new())
        .await
        .unwrap();
    row.get::<i64>("c")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn node_joining_after_state_reset_receives_existing_data() {
    let dir_1 = fresh("node_1");
    let dir_2 = fresh("node_2");
    let single = || vec![node(1, PORT_1)];
    let both = || vec![node(1, PORT_1), node(2, PORT_2)];

    // a single node with some data
    let client = hiqlite::start_node(config(1, single(), &dir_1))
        .await
        .unwrap();
    client.wait_until_healthy_db().await;
    client
        .execute("CREATE TABLE rows (id INTEGER PRIMARY KEY)", Vec::new())
        .await
        .unwrap();
    for i in 0..ROWS {
        client
            .execute("INSERT INTO rows (id) VALUES ($1)", vec![Param::from(i)])
            .await
            .unwrap();
    }
    client.shutdown().await.unwrap();
    drop(client);

    // Raft state reset: the data must survive
    // SAFETY: nothing else in this test binary reads or writes the env concurrently
    unsafe { env::set_var("HQL_DANGER_RAFT_STATE_RESET", "true") };
    let client = hiqlite::start_node(config(1, single(), &dir_1))
        .await
        .unwrap();
    // SAFETY: see above
    unsafe { env::remove_var("HQL_DANGER_RAFT_STATE_RESET") };
    client.wait_until_healthy_db().await;
    assert_eq!(count(&client).await, ROWS);
    client.shutdown().await.unwrap();
    drop(client);

    // grow to 2 nodes: the new one must end up with the same data
    let client_1 = hiqlite::start_node(config(1, both(), &dir_1))
        .await
        .unwrap();
    client_1.wait_until_healthy_db().await;
    let client_2 = hiqlite::start_node(config(2, both(), &dir_2))
        .await
        .unwrap();
    client_2
        .wait_for_cluster_sync_timeout(Duration::from_secs(60))
        .await
        .unwrap();

    let mut seen = None;
    for _ in 0..100 {
        let res = client_2
            .query_raw_one("SELECT COUNT(*) AS c FROM rows", Vec::new())
            .await;
        if let Ok(mut row) = res {
            let n = row.get::<i64>("c");
            seen = Some(n);
            if n == ROWS {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        seen,
        Some(ROWS),
        "the node that joined after the reset does not have the existing data"
    );

    client_2.shutdown().await.unwrap();
    client_1.shutdown().await.unwrap();
}
