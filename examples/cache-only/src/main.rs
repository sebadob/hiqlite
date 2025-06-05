use hiqlite::cache_idx::CacheIndex;
use hiqlite::{Error, NodeConfig};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::time::Duration;
use tokio::fs;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// This enum is used as our cache identifier.
#[derive(Debug, strum::EnumIter)]
enum Cache {
    One,
    Two,
}

// This tiny block of boilerplate is necessary to index concurrent caches properly.
// The result must always return each elements position in the iterator and this simple typecasting
// is the easiest way to do it. It is checked for correctness and compared against the iterator
// during startup.
impl CacheIndex for Cache {
    fn to_usize(self) -> usize {
        self as usize
    }
}

/// We will use this as our test value for the cache
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Value {
    pub id: String,
    pub num: i64,
    pub description: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // make sure we always start clean
    let _ = fs::remove_dir_all("./data").await;

    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::from("info"))
        .init();

    let config = NodeConfig::from_toml("../../hiqlite.toml", None).await?;
    // Hiqlite Caches are (by default) disk-backed. This means they provide the consistency of Raft
    // and can rebuild their in-memory data after a restart and never lose it. With
    // `config.cache_storage_disk = true`, the Cache WAL files + Snapshots will be persisted to
    // disk. This is of course quite a bit slower than keeping everything in-memory only, but in
    // return, you get lower memory usage and higher consistency + you never lose state.
    //
    // You can also keep everything in-memory only, which will be a lot faster, but can lead to
    // instabilities in the whole Raft cluster, if a node cannot do a graceful shutdown (and leave
    // the cluster cleanly). This is due to the lost Raft state and coming up in an inconsistent
    // state after restart, because the current Leader expects the node to still have the last known
    // state applied.
    //
    //config.cache_storage_disk = true;

    let client = hiqlite::start_node_with_cache::<Cache>(config).await?;

    let key = "my key 1";
    let value = Value {
        id: "some id".to_string(),
        num: 1337,
        description: Some("My Example Description".to_string()),
    };

    // Insert a value that will expire 1 second later.
    // Each value has its own custom expiry.
    client.put(Cache::One, key, &value, Some(1)).await?;

    // make sure it still exists
    tokio::time::sleep(Duration::from_millis(800)).await;
    let v: Value = client.get(Cache::One, key).await?.unwrap();
    assert_eq!(v, value);
    info!("{:?}", v);

    // after 1 second, it will be gone
    tokio::time::sleep(Duration::from_millis(250)).await;
    let v: Option<Value> = client.get(Cache::One, key).await?;
    assert!(v.is_none());
    info!("{:?}", v);

    // Each enum variant will start a different cache in the background
    let value_2 = Value {
        id: "some other id".to_string(),
        num: 999,
        description: None,
    };

    client.put(Cache::One, key, &value, None).await?;
    client.put(Cache::Two, key, &value_2, None).await?;

    let v1: Value = client.get(Cache::One, key).await?.unwrap();
    let v2: Value = client.get(Cache::Two, key).await?.unwrap();
    info!("{:?}", v1);
    info!("{:?}", v2);

    assert_eq!(v1, value);
    assert_eq!(v2, value_2);
    assert_ne!(v1, v2);

    info!("All tests successful");

    client.shutdown().await?;

    // In case of cache-only, we don't care about a graceful shutdown, since all data
    // exists in-memory only anyway.

    Ok(())
}
