//! A single-node deployment must not report healthy while it is still re-applying the log it
//! found at startup.
//!
//! A single node is its own leader, so it re-applies everything its WAL holds shortly after the
//! election. Before that, its state machine lags behind its own log, and after an unclean stop
//! with `auto-heal` the SQLite state machine even starts empty. Reporting healthy in that window
//! lets an application read missing data, e.g. find an empty database and run its first-start
//! setup a second time.
//!
//! The unclean stop is simulated by a clean shutdown followed by recreating the state machine
//! lock file, which is exactly the crash marker a killed process leaves behind. The replay only
//! has to take longer than a single-node election, which a few thousand log entries ensure.

#![cfg(all(feature = "sqlite", feature = "cache"))]

use hiqlite::{CacheVariants, Node, NodeConfig, Param};
use std::time::{Duration, Instant};

/// Enough single-row writes that replaying them takes longer than a single-node election.
const ROWS: i64 = 3000;
/// How long a correct start may take to become healthy.
const HEALTHY_WITHIN: Duration = Duration::from_secs(120);

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

fn fresh(case: &str) -> String {
    let dir = format!("../target/test_data/single_node_replay/{case}");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn config(dir: &str, port_api: u16) -> NodeConfig {
    #[allow(unused_mut)]
    let mut c = NodeConfig {
        node_id: 1,
        nodes: vec![Node {
            id: 1,
            addr_raft: format!("127.0.0.1:{}", port_api + 1),
            addr_api: format!("127.0.0.1:{port_api}"),
        }],
        data_dir: dir.to_string().into(),
        secret_raft: "SuperSecureSecret1337".to_string(),
        secret_api: "SuperSecureSecret1337".to_string(),
        cache_storage_disk: true,
        // Keep the default early health check delay on purpose: on a single node, it must not
        // hide a pending replay either.
        ..Default::default()
    };
    #[cfg(any(feature = "s3", feature = "dashboard"))]
    {
        c.enc_keys.enc_key_active = "k1".into();
        c.enc_keys.enc_keys = vec![("k1".into(), vec![7u8; 32])];
    }
    c
}

async fn start(dir: &str, port_api: u16) -> hiqlite::Client {
    hiqlite::start_node_with_cache::<Cache>(config(dir, port_api))
        .await
        .expect("the node starts")
}

/// `GET /ready` on the node's API, answered with the HTTP status code.
async fn http_ready(port_api: u16) -> Option<u16> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port_api))
        .await
        .ok()?;
    s.write_all(b"GET /ready HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n")
        .await
        .ok()?;
    let mut buf = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(10), s.read_to_end(&mut buf)).await;
    let head = String::from_utf8_lossy(&buf);
    head.split_whitespace().nth(1)?.parse().ok()
}

async fn count(client: &hiqlite::Client) -> Result<i64, hiqlite::Error> {
    let mut row = client
        .query_raw_one("SELECT COUNT(*) AS c FROM rows", Vec::new())
        .await?;
    Ok(row.get::<i64>("c"))
}

/// After an unclean stop, the SQLite state machine is rebuilt from the log. The first time
/// `/ready` or `wait_for_cluster_sync()` says the node is good to go, every row that was
/// acknowledged before the stop must be readable.
#[cfg(feature = "auto-heal")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sqlite_rebuild_after_unclean_stop_is_not_ready_before_replay() {
    let port = 36101;
    let dir = fresh("sqlite_unclean_stop");

    let client = start(&dir, port).await;
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
    assert_eq!(count(&client).await.unwrap(), ROWS);
    client.shutdown().await.unwrap();
    drop(client);

    // the crash marker a killed process leaves behind
    std::fs::write(format!("{dir}/state_machine/lock"), b"").unwrap();

    let client = start(&dir, port).await;

    let deadline = Instant::now() + HEALTHY_WITHIN;
    loop {
        assert!(Instant::now() < deadline, "the node never became ready");

        if http_ready(port).await == Some(200) {
            let seen = count(&client).await;
            assert!(
                matches!(seen, Ok(n) if n == ROWS),
                "/ready answered 200 while the rebuilt state machine held {seen:?} of {ROWS} \
                 acknowledged rows"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    client.shutdown().await.unwrap();
    drop(client);

    // `wait_for_cluster_sync()` must wait for the replay as well
    std::fs::write(format!("{dir}/state_machine/lock"), b"").unwrap();
    let client = start(&dir, port).await;
    client
        .wait_for_cluster_sync_timeout(HEALTHY_WITHIN)
        .await
        .unwrap();
    assert_eq!(count(&client).await.unwrap(), ROWS);
    client.shutdown().await.unwrap();
}

/// A disk-backed cache keeps its state machine in memory, so every start replays its log, clean
/// or not. The first time `/ready` or `wait_for_cluster_sync()` says the node is good to go, every
/// acknowledged key must be readable.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cache_replay_is_not_ready_before_replay() {
    let port = 36111;
    let dir = fresh("cache_replay");

    let client = start(&dir, port).await;
    client.wait_until_healthy_cache().await;
    for i in 0..ROWS {
        client
            .put(Cache::One, format!("k{i}"), &i, None)
            .await
            .unwrap();
    }
    client.shutdown().await.unwrap();
    drop(client);

    let client = start(&dir, port).await;

    let deadline = Instant::now() + HEALTHY_WITHIN;
    loop {
        assert!(Instant::now() < deadline, "the node never became ready");

        if http_ready(port).await == Some(200) {
            let last: Result<Option<i64>, _> =
                client.get(Cache::One, format!("k{}", ROWS - 1)).await;
            assert!(
                matches!(last, Ok(Some(n)) if n == ROWS - 1),
                "/ready answered 200 while the last acknowledged key read as {last:?}"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    client.shutdown().await.unwrap();
    drop(client);

    // `wait_for_cluster_sync()` must wait for the replay as well
    let client = start(&dir, port).await;
    client
        .wait_for_cluster_sync_timeout(HEALTHY_WITHIN)
        .await
        .unwrap();
    let last: Result<Option<i64>, _> = client.get(Cache::One, format!("k{}", ROWS - 1)).await;
    assert!(
        matches!(last, Ok(Some(n)) if n == ROWS - 1),
        "wait_for_cluster_sync() returned while the last acknowledged key read as {last:?}"
    );
    client.shutdown().await.unwrap();
}
