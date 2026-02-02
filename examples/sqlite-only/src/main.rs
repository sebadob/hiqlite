use hiqlite::{Error, NodeConfig, StmtIndex};
use hiqlite_macros::embed::*;
use hiqlite_macros::{params, FromRow};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use tokio::fs;
use tracing_subscriber::EnvFilter;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

/// Matches our test table for this example.
///
/// serde derives are needed if you want to use the `query_as()` fn.
///
/// The `FromRow` macro will `impl From<&mut Row<'_>> for Entity` for us. For complex types,
/// you  have additional attributes available, or you can do a manual impl. Check out the
/// `derive-complex-types` in that case.
#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Entity {
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

    let config = NodeConfig::from_toml("../../hiqlite.toml", None, None).await?;
    let client = hiqlite::start_node(config).await?;

    // Let's register our shutdown handle to always perform a graceful shutdown and remove lock files.
    // You can do this manually by calling `.shutdown()` at the end as well, if you already have
    // something like that.
    let mut shutdown_handle = client.shutdown_handle()?;

    log("Apply our database migrations");
    client.migrate::<Migrations>().await?;

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

    // The `.query_as` can be used for types that implement serde::Serialize / ::Deserialize.
    // This is easier and less work to implement, but you have less control compared to a manual
    // `impl From<&mut Row<'_>>`

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
    // fast and safe against SQL injection.
    // If you can make use of this, use it! It is really fast!

    log("Testing multiple executes in a transaction");

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

    // An example of a transaction using variables as parameters.
    // These parameter types, based on StmtIndex, are variables that refer to output columns
    // produced earlier in the transaction.
    // This way, we can express a clean multi-statement insertion using SQL-generated (e.g. autoincrement) primary keys,
    // and use those later as foreign keys.

    log("Testing transaction with variable parameters");

    let insert_parent = "INSERT INTO parent (description) VALUES ($1) RETURNING id";
    let insert_child = "INSERT INTO child (parent_id, description) VALUES ($1, $2)";
    let res = client
        .txn([
            (insert_parent, params!("parent A")), // StmtIndex(0)
            (insert_parent, params!("parent B")), // StmtIndex(1)
            // Pick the first statement's first output column as the children's `parent_id`:
            (insert_child, params!(StmtIndex(0).column(0), "child A.1")),
            (insert_child, params!(StmtIndex(0).column(0), "child A.2")),
            // We can also refer to a column by name. Now using "parent B":
            (
                insert_child,
                params!(StmtIndex(1).column("id"), "child B.1"),
            ),
        ])
        .await;

    for inner_res in res? {
        let rows_affected = inner_res?;
        assert_eq!(rows_affected, 1);
    }

    // We can also do simple `String` based batch executes
    log("Testing simple query batching");

    let mut results = client
        .batch(
            r#"
            INSERT INTO test (id, num, description) VALUES
                ('batch1', 1, "Batch desc 1"),
                ('batch2', 2, "Batch desc 2"),
                ('batch3', 3, "Batch desc 3");

            DELETE FROM test WHERE id = 'id4';
            "#,
        )
        .await?;

    // we will receive a Vec with all the results again, just like for the transaction above
    let rows_affected = results.remove(0)?;
    assert_eq!(rows_affected, 3);

    let rows_affected = results.remove(0)?;
    assert_eq!(rows_affected, 1);

    log("All tests successful");
    log("You can exit with CTRL + C now and the shutdown handler will clean up");

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

    shutdown_handle.wait().await?;

    Ok(())
}

// this way of logging makes our logs easier to see with all the raft logging enabled
fn log<S: Display>(s: S) {
    println!("\n\n>>> {s}\n");
}

fn debug<S: Debug>(s: &S) {
    println!("\n\n>>> {s:?}\n",);
}
