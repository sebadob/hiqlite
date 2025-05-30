use clap::Parser;
use hiqlite::cache_idx::CacheIndex;
use hiqlite::{start_node_with_cache, Error, Node, NodeConfig, Row, ServerTlsConfig};
use hiqlite_macros::embed::*;
use hiqlite_macros::params;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::time::Duration;
use tokio::fs;
use tokio::time;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

#[derive(Debug, Clone, Parser)]
#[clap(author, version, about, long_about = None)]
enum Args {
    /// Start one of the 3 test nodes
    Server(Server),
    /// Start the test with just a single node - works in the same way
    Single,
}

#[derive(Debug, Clone, Parser)]
struct Server {
    #[clap(long)]
    pub node_id: u64,
}

fn test_nodes() -> Vec<Node> {
    vec![
        Node {
            id: 1,
            addr_api: "127.0.0.1:8101".to_string(),
            addr_raft: "127.0.0.1:8201".to_string(),
        },
        Node {
            id: 2,
            addr_api: "127.0.0.1:8102".to_string(),
            addr_raft: "127.0.0.1:8202".to_string(),
        },
        Node {
            id: 3,
            addr_api: "127.0.0.1:8103".to_string(),
            addr_raft: "127.0.0.1:8203".to_string(),
        },
    ]
}

fn node_config(node_id: u64, nodes: Vec<Node>) -> NodeConfig {
    NodeConfig {
        node_id,
        nodes,
        log_statements: true,
        tls_raft: Some(ServerTlsConfig {
            key: "../../tls/key.pem".into(),
            cert: "../../tls/cert-chain.pem".into(),
            danger_tls_no_verify: true,
        }),
        tls_api: Some(ServerTlsConfig {
            key: "../../tls/key.pem".into(),
            cert: "../../tls/cert-chain.pem".into(),
            danger_tls_no_verify: true,
        }),
        secret_raft: "SuperSecureRaftSecret".to_string(),
        secret_api: "SuperSecureApiSecret".to_string(),
        ..Default::default()
    }
}

/// Matches our test table for this example.
/// serde derives are needed if you want to use the `query_as()` fn.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Entity {
    pub id: String,
    pub num: i64,
    pub description: Option<String>,
}

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

// This impl is needed for `query_map()` which gives you more control
impl<'r> From<Row<'r>> for Entity {
    fn from(mut row: Row<'r>) -> Self {
        Self {
            id: row.get("id"),
            num: row.get("num"),
            description: row.get("description"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::from("debug"))
        // .with_env_filter(EnvFilter::from("info"))
        .init();

    match Args::parse() {
        Args::Server(args) => {
            if args.node_id < 1 || args.node_id > 3 {
                Err(Error::Config("Node ID must be 1-3".into()))
            } else {
                server(Some(args)).await
            }
        }
        Args::Single => server(None).await,
    }
}

async fn server(args: Option<Server>) -> Result<(), Error> {
    let is_cluster = args.is_some();
    let config = if let Some(args) = args {
        let mut config = node_config(args.node_id, test_nodes());

        // to make this example work when starting all nodes on the same host,
        // we need to save into custom folders for each one
        config.data_dir = format!("data/node_{}", args.node_id).into();
        cleanup(config.data_dir.as_ref()).await;
        config
    } else {
        let mut config = node_config(1, vec![test_nodes()[0].clone()]);
        config.data_dir = "data/node_1".into();
        cleanup(config.data_dir.as_ref()).await;
        config
    };

    // Start the Raft node itself and get a client
    // the auto_init setting will initialize the Raft cluster automatically and adds
    // all given Nodes as members, as soon as they are all up and running

    // for simplicity, we will only do the inserts in this example on node 1,
    // the others will go to sleep
    let is_node_1 = config.node_id == 1;
    let nodes_len = config.nodes.len();

    let client = start_node_with_cache::<Cache>(config).await?;

    // // give the client some time to initialize everything
    // time::sleep(Duration::from_secs(3)).await;
    //
    // if is_node_1 {
    //     log("Wait until the cluster is fully online");
    //     while client.is_healthy_db().await.is_err() {
    //         log("Waiting for the Cluster to become healthy");
    //         time::sleep(Duration::from_secs(1)).await;
    //     }
    //
    //     // Usually, the "cluster" is healthy with a single node, but for this example, let's wait
    //     // until remotes are online as well
    //     if is_cluster {
    //         loop {
    //             let metrics = client.metrics_db().await?;
    //             let membership = metrics.membership_config.membership();
    //             if membership.nodes().count() != nodes_len {
    //                 info!(
    //                     "Waiting for other nodes to join the cluster - online: {}/{}",
    //                     membership.nodes().count(),
    //                     nodes_len
    //                 );
    //                 time::sleep(Duration::from_secs(1)).await;
    //                 continue;
    //             }
    //
    //             if membership.voter_ids().count() != nodes_len {
    //                 info!(
    //                     "Waiting for other nodes to join the cluster - voters: {}/{}",
    //                     membership.voter_ids().count(),
    //                     nodes_len
    //                 );
    //                 time::sleep(Duration::from_secs(1)).await;
    //                 continue;
    //             }
    //
    //             break;
    //         }
    //     }
    //
    //     log("Cluster is online and all nodes are active members");
    //
    //     log("Apply our database migrations");
    //     client.migrate::<Migrations>().await?;
    //
    //     log("Make sure table is empty from older example runs");
    //     client.execute("DELETE FROM test", params!()).await?;
    //
    //     log("Insert a row");
    //     client
    //         .execute(
    //             "INSERT INTO test (id, num, description) VALUES ($1, $2, $3)",
    //             params!("id1", 123, "my description for 1. row"),
    //         )
    //         .await?;
    //
    //     log("Let's get the data back from the DB in easy mode");
    //
    //     // The `.query_as` can be used for types that implement serde::Serialize / ::Deserialize.
    //     // This is easier and less work to implement, but a bit less efficient and slower than a
    //     // manual implementation of `From<Row>`.
    //
    //     let res: Entity = client
    //         .query_as_one("SELECT * FROM test WHERE id = $1", params!("id1"))
    //         .await?;
    //     debug(&res);
    //     assert_eq!(res.id, "id1");
    //     assert_eq!(res.num, 123);
    //     assert_eq!(
    //         res.description.as_deref(),
    //         Some("my description for 1. row")
    //     );
    //
    //     log("Let's get the data back from the DB in a more efficient and faster way with more manual work");
    //
    //     let res: Entity = client
    //         .query_map_one("SELECT * FROM test WHERE id = $1", params!("id1"))
    //         .await?;
    //     debug(&res);
    //     assert_eq!(res.id, "id1");
    //     assert_eq!(res.num, 123);
    //     assert_eq!(
    //         res.description.as_deref(),
    //         Some("my description for 1. row")
    //     );
    //
    //     // Instead of the *_one queries, you can leave it out to retrieve multiple rows
    //     let res: Vec<Entity> = client
    //         .query_map("SELECT * FROM test WHERE id = $1", params!("id1"))
    //         .await?;
    //     debug(&res);
    //     assert_eq!(res.len(), 1);
    //     assert_eq!(res[0].id, "id1");
    //     assert_eq!(res[0].num, 123);
    //     assert_eq!(
    //         res[0].description.as_deref(),
    //         Some("my description for 1. row")
    //     );
    //
    //     // Transactions do work already as well, but because of the Raft a bit differently how you
    //     // might be used to them. You don't create a transactions and pass it around in your code,
    //     // but instead you can provide as many queries as you like. This is kind of like batching, but
    //     // everything inside a single transaction and each query is prepared and cached, which makes it
    //     // fast and safe against SQL injection.
    //     // If you can make use of this, use it! It is really fast!
    //
    //     log("Testing multiple executes in a transaction");
    //
    //     let sql = "INSERT INTO test (id, num, description) VALUES ($1, $2, $3)";
    //     let res = client
    //         .txn([
    //             (sql, params!("id2", 345, "my description for 2. row")),
    //             (sql, params!("id3", 678, "my description for 3. row")),
    //             (sql, params!("id4", 999, "my description for 4. row")),
    //         ])
    //         .await;
    //
    //     // From a transaction, you get one result and many smaller ones.
    //     // The first result is for the transaction commit itself
    //     assert!(res.is_ok());
    //
    //     // The inner value is a Vec<Result<_>> contain a result for each single execute in the
    //     // exact same order as they were provided.
    //     for inner_res in res? {
    //         let rows_affected = inner_res?;
    //         assert_eq!(rows_affected, 1);
    //     }
    //
    //     // We can also do simple `String` based batch executes
    //     log("Testing simple query batching");
    //
    //     let mut results = client
    //         .batch(
    //             r#"
    //         INSERT INTO test (id, num, description) VALUES
    //             ('batch1', 1, "Batch desc 1"),
    //             ('batch2', 2, "Batch desc 2"),
    //             ('batch3', 3, "Batch desc 3");
    //
    //         DELETE FROM test WHERE id = 'id4';
    //         "#,
    //         )
    //         .await?;
    //
    //     // we will receive a Vec with all the results again, just like for the transaction above
    //     let rows_affected = results.remove(0)?;
    //     assert_eq!(rows_affected, 3);
    //
    //     let rows_affected = results.remove(0)?;
    //     assert_eq!(rows_affected, 1);
    //
    //     log("Inserting a value into the in-memory cache");
    //     let key = "my cache key";
    //     let value = "Some Cache Value".to_string();
    //     client.put(Cache::One, key, &value, None).await?;
    //     let value_ret: String = client
    //         .get(Cache::One, key)
    //         .await?
    //         .expect("It will be Some(_) for sure");
    //     assert_eq!(value_ret, value);
    //
    //     log("Testing listen / notify");
    //     // If you have a client with local data replication, you get a guaranteed once delivery
    //     // for listen / notify, as long as your node is online. A remote-only client may lose
    //     // some messages during network issues, and it behaves the same as PG listen / notify.
    //     let msg = Entity {
    //         id: "id_1337".to_string(),
    //         num: 23,
    //         description: Some(
    //             "You can send anything which implementes Serialize / Deserialize".to_string(),
    //         ),
    //     };
    //     client.notify(&msg).await?;
    //
    //     // Let's listen for messages. These will be replicated to all cluster members.
    //     let msg_ret = client.listen::<Entity>().await?;
    //     debug(&msg_ret);
    //     assert_eq!(msg, msg_ret);
    //
    //     log("Testing distributed locking");
    //     // In some cases, you need to make sure you get some lock for either longer running actions
    //     // or ones that need retrieving data, manipulating it and then sending it back to the DB.
    //     // In these cases you might not be able to do all at once in a SQL query.
    //     // Hiqlite has distributed locks (feature `dlock`) to achieve this.
    //     let lock = client.lock("my lock key").await?;
    //
    //     // A lock key can be any String to provide the most flexibility.
    //     // It behaves the same as any other lock - it will be released on drop and as long as it
    //     // exists, other locks will have to wait.
    //     //
    //     // In the current implementation, distributed locks have an internal timeout of 10 seconds.
    //     // When this time expires, a lock will be considered "dead" because of network issues, just
    //     // in case it has not been possible to release the lock properly. This prevents deadlocks
    //     // just because some client or server crashed.
    //     drop(lock);
    //
    //     log("All tests successful");
    //     log("You can exit with CTRL + C now and the shutdown handler will clean up");
    // }

    // This is very important:
    // You MUST do a graceful shutdown when your application exits. This will make sure all
    // lock files are cleaned up and will make your next start faster. If the node starts up
    // without cleanup lock files, it will delete the DB and re-create it from the latest
    // snapshot + logs to really make sure it is 100% consistent.
    // You can set features for `hiqlite` which enable auto-healing (without it will panic on
    // start), but you should always try to do a shutdown.
    //
    // You have 2 options:
    // - register an automatic shutdown handle with the DbClient like shown above
    // - trigger the shutdown manually at the end of your application
    //   This makes sense when you already have structures implemented that catch shutdown signals,
    //   for instance if you `.await` and API being terminated.
    //   Then oyu can do a `client.shutdown().await?`
    let mut shutdown_handle = client.shutdown_handle()?;
    shutdown_handle.wait().await?;

    Ok(())
}

// this way of logging makes our logs easier to see with all the raft logging enabled
fn log<S: Display>(s: S) {
    println!("\n\n>>> {}\n", s);
}

fn debug<S: Debug>(s: &S) {
    println!("\n\n>>> {:?}\n", s);
}

async fn cleanup(path: &str) {
    let _ = fs::remove_dir_all(path).await;
}
