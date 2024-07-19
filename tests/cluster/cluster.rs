use chrono::Utc;
use hiqlite::{params, start_node, NodeConfig, Param};
use hiqlite::{DbClient, Error, Node};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::time::Duration;
use tokio::{task, time};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_cluster() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_env_filter(EnvFilter::new("info"))
        .init();

    log("Starting Cluster");

    let (client_1, client_2, client_3) = start_test_cluster().await?;
    log("Cluster has been started");

    wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy");

    log("Starting data insertion and query tests");
    test_insert_query(&client_1, &client_2, &client_3).await?;
    log("Basic query tests are fine");

    log("Starting Transaction tests");
    test_transactions(&client_1, &client_2, &client_3).await?;
    log("Transaction tests are fine");

    log("Starting batch tests");
    test_batch(&client_1, &client_2, &client_3).await?;
    log("Batch tests are fine");

    // TODO impl + test
    // - migrations
    // - batch / simple queries
    // - backups to s3
    // - consistent queries on leader

    // TODO test
    // - shutdown / restart
    // - self-heal capabilities after data loss

    Ok(())
}

async fn start_test_cluster() -> Result<(DbClient, DbClient, DbClient), Error> {
    let d1 = tempfile::TempDir::new()?;
    let d2 = tempfile::TempDir::new()?;
    let d3 = tempfile::TempDir::new()?;

    let dir_1 = d1.path().as_os_str().to_str().unwrap();
    let dir_3 = d3.path().as_os_str().to_str().unwrap();
    let dir_2 = d2.path().as_os_str().to_str().unwrap();

    let build_config = |node_id: u64| -> NodeConfig {
        let nodes = vec![
            Node {
                id: 1,
                addr_raft: "127.0.0.1:32001".to_string(),
                addr_api: "127.0.0.1:31001".to_string(),
            },
            Node {
                id: 2,
                addr_raft: "127.0.0.1:32002".to_string(),
                addr_api: "127.0.0.1:31002".to_string(),
            },
            Node {
                id: 3,
                addr_raft: "127.0.0.1:32003".to_string(),
                addr_api: "127.0.0.1:31003".to_string(),
            },
        ];
        let data_dir = match node_id {
            1 => dir_1.to_string().into(),
            2 => dir_2.to_string().into(),
            3 => dir_3.to_string().into(),
            _ => unreachable!(),
        };

        NodeConfig {
            node_id,
            nodes,
            data_dir,
            filename_db: "hiqlite".into(),
            config: NodeConfig::raft_config(1000),
            tls: false,
            secret_raft: "asdasdasdasdasdasd".to_string(),
            secret_api: "qweqweqweqweqweqwe".to_string(),
        }
    };

    let handle_client_1 = task::spawn(start_node(build_config(1), true));
    let client_2 = start_node(build_config(2), true).await?;
    let client_3 = start_node(build_config(3), true).await?;
    let client_1 = handle_client_1.await??;

    Ok((client_1, client_2, client_3))
}

async fn wait_for_healthy_cluster(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    for i in 1..=3 {
        loop {
            time::sleep(Duration::from_secs(1)).await;

            let client = match i {
                1 => client_1,
                2 => client_2,
                3 => client_3,
                _ => unreachable!(),
            };

            match client.is_healthy().await {
                Ok(_) => {
                    log(format!("Node {} is healthy", i));
                    break;
                }
                Err(err) => {
                    log(format!("Waiting for Node {} to become healthy: {}", i, err));
                }
            }
        }
    }

    let metrics = client_1.metrics().await?;
    assert!(metrics.running_state.is_ok());

    let node_count = metrics.membership_config.membership().nodes().count();
    assert_eq!(node_count, 3);

    Ok(())
}

// serde derives are mandatory if we want to use the `query_as()`
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestData {
    pub id: i64,
    pub ts: i64,
    pub description: String,
}

// the  From<&'r hiqlite::Row<'r>> is mandatory if we want to use the more efficient `query_map()`
impl<'r> From<&'r hiqlite::Row<'r>> for TestData {
    fn from(row: &'r hiqlite::Row<'r>) -> Self {
        // the fastest but more error-prone method is to use column index
        // with these, the order matters
        Self {
            id: row.get_unwrap(0),
            ts: row.get_unwrap(1),
            description: row.get_unwrap(2),
        }

        // you could also use the get by return column name, which is a
        // bit more safe but at the same time a tiny bit less fast
        // Self {
        //     id: row.get_unwrap("id"),
        //     ts: row.get_unwrap("ts"),
        //     description: row.get_unwrap("description"),
        // }
    }
}

async fn test_insert_query(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    log("Creating test table");
    client_1
        .execute(
            r#"
    CREATE TABLE test
    (
        id          INTEGER NOT NULL
                     CONSTRAINT test_pk
                         PRIMARY KEY,
        ts          INTEGER NOT NULL,
        description TEXT    NOT NULL
    )
    "#,
            params!(),
        )
        .await?;

    log("Inserting test data");

    let data = TestData {
        id: 1,
        ts: Utc::now().timestamp(),
        description: "My First Row from client 1".to_string(),
    };
    let rows_affected = client_1
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows_affected, 1);

    log("Making sure clients 2 and 3 can read the same data");

    let res: TestData = client_2
        .query_as_one("SELECT * FROM test WHERE id = $1", params!(1))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.ts, data.ts);
    assert_eq!(res.description, data.description);

    let res: TestData = client_3
        .query_map_one("SELECT * FROM test WHERE id = $1", params!(1))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.ts, data.ts);
    assert_eq!(res.description, data.description);

    log("Making sure the same insert and read works on the other nodes as well");

    let data = TestData {
        id: 2,
        ts: Utc::now().timestamp(),
        description: "My First Row from client 2".to_string(),
    };
    let rows_affected = client_2
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows_affected, 1);

    log("Making sure clients 2 and 3 can read the same data");

    let res: TestData = client_1
        .query_as_one("SELECT * FROM test WHERE id = $1", params!(2))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.ts, data.ts);
    assert_eq!(res.description, data.description);

    let res: TestData = client_3
        .query_map_one("SELECT * FROM test WHERE id = $1", params!(2))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.ts, data.ts);
    assert_eq!(res.description, data.description);

    let data = TestData {
        id: 3,
        ts: Utc::now().timestamp(),
        description: "My First Row from client 3".to_string(),
    };
    let rows_affected = client_3
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows_affected, 1);

    log("Making sure clients 2 and 3 can read the same data");

    let res: TestData = client_1
        .query_as_one("SELECT * FROM test WHERE id = $1", params!(3))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.ts, data.ts);
    assert_eq!(res.description, data.description);

    let res: TestData = client_2
        .query_map_one("SELECT * FROM test WHERE id = $1", params!(3))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.ts, data.ts);
    assert_eq!(res.description, data.description);

    log("Expecting unique key constraint error from SQLite");
    let res = client_3
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await;
    assert!(res.is_err());
    let err = format!("{:?}", res);
    assert!(err.starts_with("Err(Sqlite(\"UNIQUE constraint failed"));

    log("DELETE query and make sure data is gone");
    let rows_affected = client_1
        .execute("DELETE FROM test WHERE id = $1", params!(1))
        .await?;
    assert_eq!(rows_affected, 1);

    let res: Result<TestData, Error> = client_1
        .query_as_one("SELECT * FROM test WHERE id = $1", params!(1))
        .await;
    assert!(res.is_err());

    log("Query multiple rows with 'query_as()'");
    let res: Vec<TestData> = client_1.query_as("SELECT * FROM test", params!()).await?;
    assert_eq!(res.len(), 2);

    log("Query multiple rows with 'query_map()'");
    let res: Vec<TestData> = client_1.query_map("SELECT * FROM test", params!()).await?;
    assert_eq!(res.len(), 2);

    Ok(())
}

async fn test_transactions(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    // we re-use the test table from the simple insert / query tests here

    log("Inserting rows with a transaction");

    let sql = "INSERT INTO test VALUES ($1, $2, $3)";
    let now = Utc::now().timestamp();
    let results = client_1
        .txn([
            (sql, params!(11, now, "Transaction Data id 11")),
            (sql, params!(12, now, "Transaction Data id 12")),
            (sql, params!(13, now, "Transaction Data id 13")),
        ])
        // The first result returned is for the whole transaction commit
        .await?;

    for res in results {
        // each result in the returned vector is for
        // the single queries in the exact same order
        assert!(res.is_ok());
    }

    log("Making sure transaction data exists for client 1");
    let select = "SELECT * FROM test WHERE id >= $1";
    let data: Vec<TestData> = client_1.query_map(select, params!(11)).await?;
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].id, 11);
    assert_eq!(data[0].ts, now);
    assert_eq!(data[0].description, "Transaction Data id 11");

    assert_eq!(data[1].id, 12);
    assert_eq!(data[1].ts, now);
    assert_eq!(data[1].description, "Transaction Data id 12");

    assert_eq!(data[2].id, 13);
    assert_eq!(data[2].ts, now);
    assert_eq!(data[2].description, "Transaction Data id 13");

    log("Making sure transaction data exists for client 2");
    let data: Vec<TestData> = client_2.query_map(select, params!(11)).await?;
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].id, 11);
    assert_eq!(data[0].ts, now);
    assert_eq!(data[0].description, "Transaction Data id 11");

    assert_eq!(data[1].id, 12);
    assert_eq!(data[1].ts, now);
    assert_eq!(data[1].description, "Transaction Data id 12");

    assert_eq!(data[2].id, 13);
    assert_eq!(data[2].ts, now);
    assert_eq!(data[2].description, "Transaction Data id 13");

    log("Making sure transaction data exists for client 3");
    let data: Vec<TestData> = client_3.query_map(select, params!(11)).await?;
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].id, 11);
    assert_eq!(data[0].ts, now);
    assert_eq!(data[0].description, "Transaction Data id 11");

    assert_eq!(data[1].id, 12);
    assert_eq!(data[1].ts, now);
    assert_eq!(data[1].description, "Transaction Data id 12");

    assert_eq!(data[2].id, 13);
    assert_eq!(data[2].ts, now);
    assert_eq!(data[2].description, "Transaction Data id 13");

    Ok(())
}

async fn test_batch(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    // we re-use the test table from the simple insert / query tests here
    log("Inserting rows with batching");

    let now = Utc::now().timestamp();
    let mut results = client_1
        .batch(format!(
            r#"
        INSERT INTO test VALUES
            (21, {now}, "Batch Data 1"),
            (22, {now}, "Batch Data 2"),
            (23, {now}, "Batch Data 3");

       INSERT INTO test VALUES (21, {now}, "This should error - unique key constraint");
        "#
        ))
        .await?;

    let rows_affected = results.remove(0)?;
    assert_eq!(rows_affected, 3);

    let should_be_err = results.remove(0);
    assert!(should_be_err.is_err());

    log("Make sure the other clients see the batch insertions");

    let data: Vec<TestData> = client_2
        .query_map("SELECT * FROM test WHERE id > $1", params!(20))
        .await?;
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].id, 21);
    assert_eq!(data[0].ts, now);
    assert_eq!(data[0].description, "Batch Data 1");

    assert_eq!(data[1].id, 22);
    assert_eq!(data[1].ts, now);
    assert_eq!(data[1].description, "Batch Data 2");

    assert_eq!(data[2].id, 23);
    assert_eq!(data[2].ts, now);
    assert_eq!(data[2].description, "Batch Data 3");

    let data: Vec<TestData> = client_3
        .query_as("SELECT * FROM test WHERE id > $1", params!(20))
        .await?;
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].id, 21);
    assert_eq!(data[0].ts, now);
    assert_eq!(data[0].description, "Batch Data 1");

    assert_eq!(data[1].id, 22);
    assert_eq!(data[1].ts, now);
    assert_eq!(data[1].description, "Batch Data 2");

    assert_eq!(data[2].id, 23);
    assert_eq!(data[2].ts, now);
    assert_eq!(data[2].description, "Batch Data 3");

    Ok(())
}

fn log<S: Display>(s: S) {
    info!("\n\n>>> {}\n", s);
}

#[allow(unused)]
fn debug<S: Debug>(s: &S) {
    info!("\n\n>>> {:?}\n", s);
}
