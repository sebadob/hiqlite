use crate::log;
use chrono::Utc;
use hiqlite::{params, Client, Error, Param};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time;

// serde derives are mandatory if we want to use the `query_as()`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestData {
    pub id: i64,
    pub ts: i64,
    pub description: Option<String>,
}

impl<'r> From<hiqlite::Row<'r>> for TestData {
    fn from(mut row: hiqlite::Row<'r>) -> Self {
        Self {
            id: row.get("id"),
            ts: row.get("ts"),
            description: row.get("description"),
        }
    }
}

pub async fn test_execute_query(
    client_1: &Client,
    client_2: &Client,
    client_3: &Client,
) -> Result<(), Error> {
    log("Inserting test data");

    let data = TestData {
        id: 1,
        ts: Utc::now().timestamp(),
        description: Some("My First Row from client 1".to_string()),
    };
    let rows_affected = client_1
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows_affected, 1);

    log("Making sure clients 2 and 3 can read the same data");
    time::sleep(Duration::from_millis(100)).await;

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
        description: Some("My First Row from client 2".to_string()),
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
        description: None,
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

    log("Query consistent from all clients");
    let res: Vec<TestData> = client_1
        .query_consistent_map("SELECT * FROM test WHERE id = $1", params!(3))
        .await?;
    assert_eq!(res[0].id, data.id);
    assert_eq!(res[0].ts, data.ts);
    assert_eq!(res[0].description, data.description);

    let res: Vec<TestData> = client_2
        .query_consistent_map("SELECT * FROM test WHERE id = $1", params!(3))
        .await?;
    assert_eq!(res[0].id, data.id);
    assert_eq!(res[0].ts, data.ts);
    assert_eq!(res[0].description, data.description);

    let res: Vec<TestData> = client_3
        .query_consistent_map("SELECT * FROM test WHERE id = $1", params!(3))
        .await?;
    assert_eq!(res[0].id, data.id);
    assert_eq!(res[0].ts, data.ts);
    assert_eq!(res[0].description, data.description);

    log("Test Execute Returning RAW");
    let data = TestData {
        id: 7,
        ts: Utc::now().timestamp(),
        description: Some("Execute Returning Data".to_string()),
    };
    let mut rows = client_1
        .execute_returning(
            "INSERT INTO test VALUES ($1, $2, $3) RETURNING *",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<i64>("id"), data.id);
    assert_eq!(rows[0].get::<i64>("ts"), data.ts);
    assert_eq!(
        rows[0].get::<Option<String>>("description").as_deref(),
        Some("Execute Returning Data")
    );

    log("Test Execute Returning Mapped");
    let data = TestData {
        id: 8,
        ts: Utc::now().timestamp(),
        description: None,
    };
    let rows: Vec<TestData> = client_1
        .execute_returning_map(
            "INSERT INTO test VALUES ($1, $2, $3) RETURNING *",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, data.id);
    assert_eq!(rows[0].ts, data.ts);
    assert_eq!(rows[0].description, data.description);

    Ok(())
}
