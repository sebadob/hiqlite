//! A cluster node must re-apply its own committed log at startup, before it serves anything.
//!
//! openraft reads the persisted `committed` pointer from the log store and re-applies
//! `(last_applied, committed]` synchronously inside `Raft::new`, offline: no leader, no peers, no
//! engine. For a cluster node that pointer is the startup target, because its log tail can hold
//! entries that were never committed, and gating on the tail would need the very peers a startup
//! gate has to survive without.
//!
//! The test writes rows into a healthy 3-node cluster, stops it, leaves node 2 the crash marker a
//! killed process leaves behind, and restarts the cluster. Node 2's state machine is wiped, so it
//! has to come back from its snapshot and its log alone, and `start_node()` must only return once
//! every acknowledged row is readable.
//!
//! Node 2 is the node to crash on purpose. Node 1 runs the pristine-init path, which probes its
//! peers and then awaits a quorum, so node 1 cannot complete a startup on its own at all - that is
//! the peer dependency this gate removes, and it is why the assertions here run on a node that is
//! not the init node.

#![cfg(all(feature = "sqlite", feature = "cache", feature = "auto-heal"))]

use hiqlite::{CacheVariants, Node, NodeConfig};
use std::time::Duration;

/// Rows written into the cluster before the restart.
const ROWS: i64 = 1000;
/// How long a correct start may take to replay its log.
const REPLAY_WITHIN: Duration = Duration::from_secs(120);

const PORT_API: [u16; 3] = [36201, 36202, 36203];
const PORT_RAFT: [u16; 3] = [36211, 36212, 36213];

#[derive(Debug)]
enum Cache {
    One,
}

impl CacheVariants for Cache {
    fn hiqlite_cache_index(&self) -> usize {
        match self {
            Cache::One => 0,
        }
    }

    fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
        &[(0, "One")]
    }
}

fn fresh() -> String {
    let dir = "../target/test_data/cluster_startup_replay".to_string();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn nodes() -> Vec<Node> {
    (0..3)
        .map(|i| Node {
            id: i + 1,
            addr_raft: format!("127.0.0.1:{}", PORT_RAFT[i as usize]),
            addr_api: format!("127.0.0.1:{}", PORT_API[i as usize]),
        })
        .collect()
}

fn config(dir: &str, node_id: u64) -> NodeConfig {
    #[allow(unused_mut)]
    let mut c = NodeConfig {
        node_id,
        nodes: nodes(),
        data_dir: format!("{dir}/node_{node_id}").into(),
        secret_raft: "SuperSecureSecret1337".to_string(),
        secret_api: "SuperSecureSecret1337".to_string(),
        cache_storage_disk: true,
        ..Default::default()
    };
    #[cfg(any(feature = "s3", feature = "dashboard"))]
    {
        c.enc_keys.enc_key_active = "k1".into();
        c.enc_keys.enc_keys = vec![("k1".into(), vec![7u8; 32])];
    }
    c
}

async fn start(dir: String, node_id: u64) -> hiqlite::Client {
    hiqlite::start_node_with_cache::<Cache>(config(&dir, node_id))
        .await
        .expect("the node starts")
}

/// Starts the whole cluster at once. Nodes must come up concurrently: node 1 runs the pristine
/// init that forms the cluster, and the others join it while it does.
async fn start_cluster(dir: String) -> [hiqlite::Client; 3] {
    let handles = [1, 2, 3].map(|id| tokio::task::spawn(start(dir.clone(), id)));
    let mut clients = Vec::new();
    for handle in handles {
        clients.push(handle.await.unwrap());
    }
    [clients.remove(0), clients.remove(0), clients.remove(0)]
}

async fn count(client: &hiqlite::Client) -> Result<i64, hiqlite::Error> {
    let mut row = client
        .query_raw_one("SELECT COUNT(*) AS c FROM rows", Vec::new())
        .await?;
    Ok(row.get::<i64>("c"))
}

/// Waits until the node's local state machine holds every acknowledged row, and fails with the
/// given context if it never does.
async fn await_rows(client: &hiqlite::Client, what: &str) -> i64 {
    let deadline = tokio::time::Instant::now() + REPLAY_WITHIN;
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "{what}: the node never replayed its log"
        );
        match count(client).await {
            Ok(n) if n == ROWS => return n,
            Ok(n) => panic!("{what}: the node's state machine holds {n} of {ROWS} rows"),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

/// A node whose state machine was wiped by a crash comes back from its own log before
/// `start_node()` returns, and every row the cluster had acknowledged is readable right after.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn cluster_node_replays_its_committed_log_at_startup() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .init();

    let dir = fresh();

    let clients = start_cluster(dir.clone()).await;
    for client in &clients {
        client.wait_until_healthy_db().await;
    }

    clients[0]
        .execute("CREATE TABLE rows (id INTEGER PRIMARY KEY)", Vec::new())
        .await
        .unwrap();
    // one batched statement: a single log entry carrying every row, so the write phase stays a
    // few Raft round trips and the replay still has a real log tail to rebuild
    let values = (0..ROWS)
        .map(|i| format!("({i})"))
        .collect::<Vec<_>>()
        .join(",");
    clients[0]
        .execute(format!("INSERT INTO rows (id) VALUES {values}"), Vec::new())
        .await
        .unwrap();
    for i in 0..50i64 {
        clients[0]
            .put(Cache::One, format!("k{i}"), &i, None)
            .await
            .unwrap();
    }
    for client in &clients {
        assert_eq!(count(client).await.unwrap(), ROWS);
    }

    for client in clients {
        client.shutdown().await.unwrap();
    }

    // the crash marker a killed process leaves behind: node 2 wipes and rebuilds its state
    // machine from its snapshot and its log on the next start
    std::fs::write(format!("{dir}/node_2/state_machine/lock"), b"").unwrap();

    let clients = start_cluster(dir.clone()).await;
    let client_2 = &clients[1];

    // `Raft::new` re-applies `(last_applied, committed]` before it returns, so node 2 is already
    // back on its acknowledged state by the time `start_node()` is - it never serves a state
    // machine that is behind its own committed log
    await_rows(client_2, "node 2 after the crash restart").await;
    // the cache group replays from its own log too, so the last acknowledged key is readable on
    // the node whose state machine was wiped
    let last: Result<Option<i64>, _> = client_2.get(Cache::One, "k49").await;
    assert!(
        matches!(last, Ok(Some(49))),
        "node 2 replayed its log but the last acknowledged cache key read as {last:?}"
    );

    for client in &clients {
        client
            .wait_for_cluster_sync_timeout(REPLAY_WITHIN)
            .await
            .expect("the cluster reaches its tip");
        assert_eq!(count(client).await.unwrap(), ROWS);
    }

    for client in clients {
        client.shutdown().await.unwrap();
    }
}
