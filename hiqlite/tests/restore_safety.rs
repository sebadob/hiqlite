//! A backup restore must never destroy state it does not own, or state it still needs.
//!
//! - A node other than 1 prepares for the restore before node 1 has even looked at the backup. If
//!   node 1 then fails to apply it, the state of the other nodes must still be there.
//! - A restore started against the `data_dir` of a live node must refuse to touch it.

#![cfg(all(feature = "sqlite", feature = "backup"))]

use hiqlite::{BackupSource, Node, NodeConfig};
use std::time::Duration;

fn fresh(case: &str) -> String {
    let dir = format!("../target/test_data/restore_safety/{case}");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A node of a cluster whose node `n` listens on `port_api + 10 * (n - 1)` for the API, and on the
/// port after that for the Raft.
fn config(node_id: u64, nodes: u64, dir: &str, port_api: u16) -> NodeConfig {
    let nodes = (1..=nodes)
        .map(|id| {
            let port = port_api + 10 * (id as u16 - 1);
            Node {
                id,
                addr_raft: format!("127.0.0.1:{}", port + 1),
                addr_api: format!("127.0.0.1:{port}"),
            }
        })
        .collect();
    let c = NodeConfig {
        node_id,
        nodes,
        data_dir: dir.to_string().into(),
        secret_raft: "SuperSecureSecret1337".to_string(),
        secret_api: "SuperSecureSecret1337".to_string(),
        ..Default::default()
    };
    #[cfg(any(feature = "s3", feature = "dashboard"))]
    let c = {
        let mut c = c;
        c.enc_keys.enc_key_active = "k1".into();
        c.enc_keys.enc_keys = vec![("k1".into(), vec![7u8; 32])];
        c
    };
    c
}

/// The state a node had before the restore, planted by hand: the restore must only move it, never
/// look into it.
fn plant_state(dir: &str) {
    std::fs::create_dir_all(format!("{dir}/state_machine/db")).unwrap();
    std::fs::write(format!("{dir}/state_machine/db/hiqlite.db"), b"old db").unwrap();
    std::fs::create_dir_all(format!("{dir}/logs")).unwrap();
    std::fs::write(format!("{dir}/logs/wal"), b"old log").unwrap();
}

/// Node 2 is started for a restore whose source node 1 will never be able to apply. Node 2 cannot
/// know that: it prepares and then waits for node 1 to form the restored cluster. Its previous
/// state must still exist, byte for byte.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_follower_keeps_its_state_until_it_joined_the_restored_cluster() {
    let dir = fresh("follower_keeps_state");
    plant_state(&dir);

    let source = BackupSource::File(format!("{dir}/does_not_exist.sqlite"));
    let start = hiqlite::start_node_from_backup(config(2, 3, &dir, 37201), source);
    // Node 1 never comes up, so node 2 waits to join forever.
    let res = tokio::time::timeout(Duration::from_secs(5), start).await;
    if let Ok(res) = res {
        panic!("node 2 cannot start without node 1: {:?}", res.err());
    }

    let aside = format!("{dir}/pre_restore");
    assert_eq!(
        std::fs::read(format!("{aside}/state_machine/db/hiqlite.db")).unwrap(),
        b"old db"
    );
    assert_eq!(
        std::fs::read(format!("{aside}/logs/wal")).unwrap(),
        b"old log"
    );
    assert!(
        !std::fs::exists(format!("{dir}/logs/wal")).unwrap(),
        "the node joins with an empty state"
    );
}

/// A node that is restarted for the same restore before it joined keeps the state it moved aside
/// the first time: the state it has now is only what it built while waiting.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_repeated_restore_start_keeps_the_first_state_that_was_moved_aside() {
    let dir = fresh("follower_repeated_start");
    plant_state(&dir);

    let aside = format!("{dir}/pre_restore");
    std::fs::create_dir_all(format!("{aside}/state_machine/db")).unwrap();
    std::fs::write(
        format!("{aside}/state_machine/db/hiqlite.db"),
        b"state from before the first start",
    )
    .unwrap();

    let source = BackupSource::File(format!("{dir}/does_not_exist.sqlite"));
    let start = hiqlite::start_node_from_backup(config(2, 3, &dir, 37231), source);
    let res = tokio::time::timeout(Duration::from_secs(5), start).await;
    if let Ok(res) = res {
        panic!("node 2 cannot start without node 1: {:?}", res.err());
    }

    assert_eq!(
        std::fs::read(format!("{aside}/state_machine/db/hiqlite.db")).unwrap(),
        b"state from before the first start"
    );
    assert!(!std::fs::exists(format!("{aside}/logs")).unwrap());
    assert!(!std::fs::exists(format!("{dir}/logs/wal")).unwrap());
}

/// A second process started with a restore against the `data_dir` of a live node must refuse to
/// touch it, whether it would apply the backup (node 1) or clear its state for the join (any other
/// node).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_restore_never_touches_the_data_dir_of_a_live_node() {
    let dir = fresh("live_node");
    let port = 37261;

    let client = hiqlite::start_node(config(1, 1, &dir, port)).await.unwrap();
    client.wait_until_healthy_db().await;
    client
        .execute("CREATE TABLE rows (id INTEGER PRIMARY KEY)", Vec::new())
        .await
        .unwrap();
    client
        .execute("INSERT INTO rows (id) VALUES (1)", Vec::new())
        .await
        .unwrap();

    // a valid backup of this very node, so node 1 would have no reason to stop on its own
    client.backup().await.unwrap();
    let backups = format!("{dir}/state_machine/backups");
    let mut backup = None;
    for _ in 0..100 {
        backup = std::fs::read_dir(&backups)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|ext| ext == "sqlite"));
        if backup.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let backup = backup.expect("the backup has been written");
    let source = BackupSource::File(backup.to_str().unwrap().to_string());

    let db = format!("{dir}/state_machine/db/hiqlite.db");
    let logs = format!("{dir}/logs");
    // node 1 would apply the backup, node 2 would move the state aside
    for (node_id, port_other) in [(1, port + 100), (2, port + 200)] {
        let other = config(node_id, 2, &dir, port_other);
        let Err(err) = hiqlite::start_node_from_backup(other, source.clone()).await else {
            panic!("node {node_id}: the restore must refuse the data dir of a live node");
        };
        assert!(
            err.to_string().contains("held by another live process"),
            "node {node_id}: {err}"
        );

        assert!(std::fs::exists(&db).unwrap(), "node {node_id}");
        assert!(std::fs::exists(&logs).unwrap(), "node {node_id}");
        assert!(!std::fs::exists(format!("{dir}/pre_restore")).unwrap());
    }

    let mut row = client
        .query_raw_one("SELECT COUNT(*) AS c FROM rows", Vec::new())
        .await
        .unwrap();
    assert_eq!(row.get::<i64>("c"), 1);
    client.shutdown().await.unwrap();
}
