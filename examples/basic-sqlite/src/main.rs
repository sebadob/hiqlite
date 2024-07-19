use clap::Parser;
use hiqlite::{params, start_node, Error, Node, NodeConfig, Param, Row};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::time::Duration;
use tokio::fs;
use tokio::time;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Parser)]
#[clap(author, version, about, long_about = None)]
enum CliArgs {
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
            addr_api: "127.0.0.1:10001".to_string(),
            addr_raft: "127.0.0.1:20001".to_string(),
        },
        Node {
            id: 2,
            addr_api: "127.0.0.1:10002".to_string(),
            addr_raft: "127.0.0.1:20002".to_string(),
        },
        Node {
            id: 3,
            addr_api: "127.0.0.1:10003".to_string(),
            addr_raft: "127.0.0.1:20003".to_string(),
        },
    ]
}

/// Matches our test table for this example.
/// serde derive's are needed if you want to use the `query_as()` fn.
#[derive(Debug, Serialize, Deserialize)]
struct Entity {
    pub id: String,
    pub num: i64,
    pub description: Option<String>,
}

// This impl is needed for the more efficient and faster `query_map()`
impl<'r> From<&'r Row<'r>> for Entity {
    fn from(row: &'r Row<'r>) -> Self {
        Self {
            // You can `.get()` either via index or via named parameter.
            // Via index is a bit faster again, especially when many values are returned, but you
            // need to be more careful when building your queries to always return values in the
            // correct order.
            id: row.get_unwrap("id"),
            num: row.get_unwrap("num"),
            description: row.get_unwrap("description"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::from("info"))
        .init();

    match CliArgs::parse() {
        CliArgs::Server(args) => {
            if args.node_id < 1 || args.node_id > 3 {
                Err(Error::Config("Node ID must be 1-3".into()))
            } else {
                server(Some(args)).await
            }
        }
        CliArgs::Single => server(None).await,
    }
}

async fn server(args: Option<Server>) -> Result<(), Error> {
    let config = if let Some(args) = args {
        let mut config = NodeConfig::new(
            args.node_id,
            test_nodes(),
            "SuperSecureRaftSecret".to_string(),
            "SuperSecureApiSecret".to_string(),
        )?;

        // to make this example work when starting all nodes on the same host,
        // we need to save into custom folders for each one
        config.data_dir = format!("data/node_{}", args.node_id).into();
        cleanup(config.data_dir.as_ref()).await;
        config
    } else {
        let config = NodeConfig::new(
            1,
            vec![test_nodes()[0].clone()],
            "SuperSecureRaftSecret".to_string(),
            "SuperSecureApiSecret".to_string(),
        )?;
        cleanup(config.data_dir.as_ref()).await;
        config
    };

    // Start the Raft node itself and get a client
    // the auto_init setting will initialize the Raft cluster automatically and adds
    // all given Nodes as members, as sonn as they are all up and running

    // for simplicity, we will only do the inserts in this example on node 1,
    // the others will go to sleep
    let is_node_i1 = config.node_id == 1;

    let client = start_node(config, true).await?;

    // give the client some time to initliaze everything
    time::sleep(Duration::from_secs(3)).await;

    if is_node_i1 {
        log("Wait until the cluster is fully online");
        while client.is_healthy().await.is_err() {
            log("Waiting for the Cluster to become healthy");
            time::sleep(Duration::from_secs(1)).await;
        }
        log("Cluster is online and all nodes are active members\n\n");

        log("Create a table for some test data");
        client
            .execute(
                r#"
        CREATE TABLE IF NOT EXISTS test
        (
            id          TEXT    NOT NULL
                CONSTRAINT test_pk
                    PRIMARY KEY,
            num         INTEGER NOT NULL,
            description TEXT
        )
        "#,
                params!(),
            )
            .await?;

        log("Make sure table is empty from older example runs");
        client.execute("DELETE FROM test", params!()).await?;

        log("Insert a row");
        client
            .execute(
                "INSERT INTO test (id, num, description) VALUES ($1, $2, $3)",
                params!("id1", 123, "my description for 1. row"),
            )
            .await?;

        log("Let's get the data back from the DB in easy mode");

        // The `.query_as` can be used for types that implement serde::Serialize / ::Deserialze.
        // This is easier and less work to implement, but a bit less efficient and slower than a
        // manual implementation of `From<Row>`.

        let res: Entity = client
            .query_as_one("SELECT * FROM test WHERE id = $1", params!("id1"))
            .await?;
        debug(&res);
        assert_eq!(res.id, "id1");
        assert_eq!(res.num, 123);
        assert_eq!(
            res.description.as_deref(),
            Some("my description for 1. row")
        );

        log("Let's get the data back from the DB in a more efficient and faster way with more manual work");

        // The `.query_one` can be used for types that implement `impl<'r> From<&'r hiqlite::Row<'r>>``.
        // This requires an additional manual step (no derive macro exists so far), but it is a
        // more efficient and faster way to map a query result to a `struct`.

        let res: Entity = client
            .query_map_one("SELECT * FROM test WHERE id = $1", params!("id1"))
            .await?;
        debug(&res);
        assert_eq!(res.id, "id1");
        assert_eq!(res.num, 123);
        assert_eq!(
            res.description.as_deref(),
            Some("my description for 1. row")
        );

        // Instead of the *_one queries, you can leave it out to retrieve multiple rows

        let res: Vec<Entity> = client
            .query_map("SELECT * FROM test WHERE id = $1", params!("id1"))
            .await?;
        debug(&res);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "id1");
        assert_eq!(res[0].num, 123);
        assert_eq!(
            res[0].description.as_deref(),
            Some("my description for 1. row")
        );

        // Transactions do work already as well, but because of the Raft a bit differently how you
        // might be used to them. You don't create a transactions and pass it around in your code,
        // but instead you can provide as many queries as you like. This is kind of like batching, but
        // everything inside a single transaction and each query is prepared and cached, which makes it
        // fast ans safe against SQL injection.
        // If you can make use of this, use it! It is really fast!

        let sql = "INSERT INTO test (id, num, description) VALUES ($1, $2, $3)";
        let res = client
            .txn([
                (sql, params!("id2", 345, "my description for 2. row")),
                (sql, params!("id3", 678, "my description for 3. row")),
                (sql, params!("id4", 999, "my description for 4. row")),
            ])
            .await;

        // From a transaction, you get one result and many smaller ones.
        // The first result is for the transaction commit itself
        assert!(res.is_ok());

        // The inner value is a Vec<Result<_>> contain a result for each single execute in the
        // exact same order as they were provided.
        for inner_res in res? {
            let rows_affected = inner_res?;
            assert_eq!(rows_affected, 1);
        }

        time::sleep(Duration::from_secs(3)).await;

        // This is very important:
        // You MUST do a graceful shtudown when your application exits. This will make sure all
        // lock files are cleaned up and will make your next start faster. If the node starts up
        // without cleanup lock files, it will delete the DB and re-create it from the latest
        // snapshot + logs to really make sure it is 100% consistent.
        // You can set features for `hiqlite` which enable auto-healing (without it will panic on
        // start), but you should always try to do a shutdown.
        // Starting an optional automatic shutdown handler is on the TODO, but not yet implemented.
        log("Shutting down client now");
        client.shutdown().await?;
    } else {
        log("Going to sleep - inserts from node 1 only in this example");
        time::sleep(Duration::from_secs(u64::MAX)).await;
    }

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
    // To keep this example simple and because we can't auto-register a shutdownhandler yet
    // we will simply cleanup the data folder with each startup.
    let _ = fs::remove_dir_all(path).await;
}
