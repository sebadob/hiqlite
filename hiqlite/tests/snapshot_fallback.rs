//! A single node whose newest snapshot is unreadable must either start from an older snapshot
//! that its log still covers, or refuse to start.
//!
//! The snapshot is the only copy of every log entry the WAL has purged. Falling back to an older
//! snapshot is only correct when the WAL still holds every entry after it; otherwise the node
//! would start with those entries silently missing. A single node has no peer to fetch them from.
//!
//! The corruption is deterministic: the newest published snapshot gets its SQLite header
//! overwritten, which is what bit-rot or a torn write on that file leaves behind.

#![cfg(feature = "sqlite")]

use hiqlite::{Node, NodeConfig, Param};
use std::time::{Duration, Instant};

/// Entries between two snapshots, small enough to build a few of them quickly.
const LOGS_UNTIL_SNAPSHOT: u64 = 100;
/// Enough single-row writes for several snapshots.
const ROWS: i64 = 450;

fn fresh(case: &str) -> String {
    let dir = format!("../target/test_data/snapshot_fallback/{case}");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn config(dir: &str, port_api: u16, max_in_snapshot_log_to_keep: u64) -> NodeConfig {
    let mut raft_config = NodeConfig::default_raft_config(LOGS_UNTIL_SNAPSHOT);
    raft_config.max_in_snapshot_log_to_keep = max_in_snapshot_log_to_keep;

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
        raft_config,
        ..Default::default()
    };
    #[cfg(any(feature = "s3", feature = "dashboard"))]
    {
        c.enc_keys.enc_key_active = "k1".into();
        c.enc_keys.enc_keys = vec![("k1".into(), vec![7u8; 32])];
    }
    c
}

fn snapshots_dir(dir: &str) -> String {
    format!("{dir}/state_machine/snapshots")
}

/// The published snapshots, newest first.
fn published_snapshots(dir: &str) -> Vec<uuid::Uuid> {
    let mut ids = std::fs::read_dir(snapshots_dir(dir))
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name();
            uuid::Uuid::parse_str(name.to_str()?).ok()
        })
        .collect::<Vec<_>>();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids
}

async fn count(client: &hiqlite::Client) -> Result<i64, hiqlite::Error> {
    let mut row = client
        .query_raw_one("SELECT COUNT(*) AS c FROM rows", Vec::new())
        .await?;
    Ok(row.get::<i64>("c"))
}

/// Writes `ROWS` rows, waits until two snapshots are published and shuts the node down cleanly.
/// Returns the newest snapshot id at shutdown.
async fn write_and_stop(dir: &str, port: u16, max_in_snapshot_log_to_keep: u64) -> uuid::Uuid {
    let client = hiqlite::start_node(config(dir, port, max_in_snapshot_log_to_keep))
        .await
        .expect("the node starts");
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

    // The builds run in the background: wait until the one triggered by the last batch of writes
    // is published, and the build before it is still there to fall back to.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let ids = published_snapshots(dir);
        let staging = std::fs::read_dir(snapshots_dir(dir))
            .unwrap()
            .any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".tmp"));
        if ids.len() >= 2 && !staging {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "expected the previous snapshot to be kept next to the newest one, found {ids:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // the cleanup after a build runs in the background as well
    tokio::time::sleep(Duration::from_millis(500)).await;
    let ids = published_snapshots(dir);
    assert_eq!(ids.len(), 2, "exactly two snapshots are kept: {ids:?}");

    client.shutdown().await.unwrap();
    drop(client);
    ids[0]
}

/// Overwrites the SQLite header, so the file is no longer readable as a database at all.
fn corrupt(dir: &str, id: uuid::Uuid) {
    use std::io::{Seek, SeekFrom, Write};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .open(format!("{}/{id}", snapshots_dir(dir)))
        .unwrap();
    f.seek(SeekFrom::Start(0)).unwrap();
    f.write_all(&[0xAB; 4096]).unwrap();
    f.sync_all().unwrap();
}

/// The crash marker a killed process leaves behind. With `auto-heal`, the next start deletes the
/// database and rebuilds it from the newest usable snapshot plus the log.
fn mark_unclean(dir: &str) {
    std::fs::write(format!("{dir}/state_machine/lock"), b"").unwrap();
}

/// After a clean stop, the database is intact and no snapshot is restored, but the newest one is
/// still read at startup. An unreadable one must not stop the node from starting.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unreadable_newest_snapshot_after_clean_stop_starts() {
    let port = 36401;
    let dir = fresh("clean_stop");

    let newest = write_and_stop(&dir, port, 1).await;
    corrupt(&dir, newest);

    let client = hiqlite::start_node(config(&dir, port, 1))
        .await
        .expect("an intact database starts despite an unreadable newest snapshot");
    client.wait_until_healthy_db().await;
    assert_eq!(count(&client).await.unwrap(), ROWS);
    client.shutdown().await.unwrap();
}

/// When the log still holds every entry after the previous snapshot, a rebuild after an unclean
/// stop falls back to it and replays the rest.
#[cfg(feature = "auto-heal")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unclean_stop_falls_back_to_previous_snapshot_covered_by_log() {
    let port = 36411;
    let dir = fresh("fallback_covered");

    // keep enough log behind each snapshot to cover the one before it
    let keep = LOGS_UNTIL_SNAPSHOT * 3;
    let newest = write_and_stop(&dir, port, keep).await;
    corrupt(&dir, newest);
    mark_unclean(&dir);

    let client = hiqlite::start_node(config(&dir, port, keep))
        .await
        .expect("the node falls back to the previous snapshot");
    client.wait_until_healthy_db().await;
    assert_eq!(count(&client).await.unwrap(), ROWS);
    assert!(
        !published_snapshots(&dir).contains(&newest),
        "the unreadable snapshot must not stay selectable"
    );
    client.shutdown().await.unwrap();
}

/// With the default log retention, the log behind the newest snapshot is purged right after it is
/// built, so the previous snapshot misses entries nobody holds anymore. Starting from it would
/// lose them silently: the node must refuse instead.
#[cfg(feature = "auto-heal")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unclean_stop_refuses_fallback_past_purged_log() {
    let port = 36421;
    let dir = fresh("fallback_purged");

    let newest = write_and_stop(&dir, port, 1).await;
    corrupt(&dir, newest);
    mark_unclean(&dir);

    let before = published_snapshots(&dir);
    let res = hiqlite::start_node(config(&dir, port, 1)).await;
    let err = match res {
        Ok(client) => {
            let seen = count(&client).await;
            let _ = client.shutdown().await;
            panic!("the node started from a snapshot its log does not cover, rows: {seen:?}");
        }
        Err(err) => err.to_string(),
    };
    assert!(err.contains("purged"), "unexpected error: {err}");
    assert_eq!(
        published_snapshots(&dir),
        before,
        "a refused start must leave the snapshots for manual recovery"
    );
}
