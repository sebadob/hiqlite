//! Loads a data dir that was written by the last release on openraft 0.9 (hiqlite 0.16) and makes
//! sure the SQLite data and its snapshot are still there. The cache data of that release identifies
//! a cache by its index, which this release cannot read, so the node drops it once on startup and
//! the cache group starts empty.
//!
//! The fixture lives in `tests/data_09/` and was produced by the pre-migration code (a single node,
//! default config, plus a fast snapshot policy so the fixture contains a snapshot per group). The
//! test copies it to `tests/data_09_run/` first, because starting a node writes to the data dir and
//! the fixture itself must stay exactly as 0.9 wrote it.
//!
//! The cache part of the fixture is feature-dependent: the memory state machine's snapshot shape
//! changes with `counters`/`dlock`, so the fixture was written - and this test is gated - with exactly
//! the feature set `just test-no-s3` uses.
#![cfg(all(
    feature = "sqlite",
    feature = "cache",
    feature = "macros",
    feature = "toml",
    feature = "auto-heal",
    feature = "backup",
    feature = "counters",
    feature = "dlock",
    feature = "listen_notify",
    feature = "external-state-machine"
))]

use hiqlite::macros::{CacheVariants, FromRow, params};
use hiqlite::{Client, Node, NodeConfig};
use std::path::Path;
use std::time::Duration;
use tokio::time;

#[derive(Debug, CacheVariants)]
enum Cache {
    Fixture,
}

#[derive(Debug, PartialEq, Eq, FromRow)]
struct Row09 {
    a: i64,
}

const FIXTURE_DIR: &str = "tests/data_09";
/// The copy the node runs on - the fixture itself must stay as 0.9 wrote it.
const FIXTURE_RUN_DIR: &str = "tests/data_09_run";
const SECRET_API: &str = "qweqweqweqweqweqwe";
const SECRET_RAFT: &str = "asdasdasdasdasdasd";

#[tokio::test(flavor = "multi_thread")]
async fn loads_an_openraft_09_data_dir() {
    let data_dir = Path::new(FIXTURE_RUN_DIR);
    if data_dir.exists() {
        std::fs::remove_dir_all(data_dir).unwrap();
    }
    copy_dir(Path::new(FIXTURE_DIR), data_dir);

    let mut config = NodeConfig::from_toml("../hiqlite.toml", None, None, None)
        .await
        .unwrap();
    config.node_id = 1;
    config.nodes = vec![Node {
        id: 1,
        addr_raft: "127.0.0.1:39001".to_string(),
        addr_api: "127.0.0.1:38001".to_string(),
    }];
    config.data_dir = FIXTURE_RUN_DIR.to_string().into();
    config.secret_raft = SECRET_RAFT.to_string();
    config.secret_api = SECRET_API.to_string();
    config.cache_storage_disk = true;

    let client: Client = hiqlite::start_node_with_cache::<Cache>(config)
        .await
        .unwrap();

    client.wait_until_healthy_db().await;
    client.wait_until_healthy_cache().await;

    // the SQLite data must be served again, no matter if it came from the database file, the
    // snapshot that was sitting in `state_machine/snapshots`, or the logs after that snapshot
    let rows: Vec<Row09> = client
        .query_map("SELECT a FROM t ORDER BY a", params!())
        .await
        .unwrap();
    assert_eq!(rows, vec![Row09 { a: 1 }, Row09 { a: 2 }, Row09 { a: 3 }]);

    // the snapshot the old version left behind must have been picked up; its name is a UUIDv7
    let dir = format!("{FIXTURE_RUN_DIR}/state_machine/snapshots");
    assert!(
        count_files(&dir) >= 1,
        "no snapshot file was picked up in {dir}"
    );

    // the WAL segment the old version wrote must still be the one in use
    let dir = format!("{FIXTURE_RUN_DIR}/logs");
    assert!(
        Path::new(&format!("{dir}/0000000000000001.wal")).exists(),
        "the 0.9 WAL segment is gone from {dir}"
    );

    // the index-based cache data was dropped once: the marker is gone, and so is the value, while
    // the cache group itself is up and takes writes again
    assert!(
        !Path::new(&format!(
            "{FIXTURE_RUN_DIR}/state_machine_cache/cache_index.meta"
        ))
        .exists()
    );
    let value: Option<String> = client.get(Cache::Fixture, "fixture-key").await.unwrap();
    assert_eq!(value, None);
    client
        .put(
            Cache::Fixture,
            "fixture-key",
            &"new-value".to_string(),
            None,
        )
        .await
        .unwrap();
    let value: Option<String> = client.get(Cache::Fixture, "fixture-key").await.unwrap();
    assert_eq!(value.as_deref(), Some("new-value"));

    // give a possible new snapshot a moment to land before we shut down
    time::sleep(Duration::from_secs(2)).await;
    client.shutdown().await.unwrap();
}

fn count_files(dir: impl AsRef<Path>) -> usize {
    std::fs::read_dir(dir.as_ref()).map_or(0, |rd| {
        rd.filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .count()
    })
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
