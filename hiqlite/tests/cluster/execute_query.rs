use crate::log;
use chrono::Utc;
use hiqlite::macros::{TryFromRow, params};
use hiqlite::{Client, Error};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time;

// serde derives are mandatory if we want to use the `query_as()`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestData {
    pub id: i64,
    pub ts: i64,
    pub description: Option<String>,
}

impl From<&mut hiqlite::Row<'_>> for TestData {
    fn from(row: &mut hiqlite::Row<'_>) -> Self {
        Self {
            id: row.get("id"),
            ts: row.get("ts"),
            description: row.get("description"),
        }
    }
}

// Non-panicking counterpart of `TestData`, built with the `TryFromRow` derive so a row that
// cannot be converted is returned as an `Error` instead of aborting the whole query.
#[derive(Debug, Clone, PartialEq, TryFromRow)]
pub struct TestTryRow {
    pub id: i64,
    pub ts: i64,
    pub description: Option<String>,
}

// Reads `ts` as a narrowing `i32`; a stored value outside the `i32` range makes `TryFrom` fail
// with an `Error::Sqlite(...)` instead of panicking.
#[derive(Debug, Clone, PartialEq, TryFromRow)]
pub struct NarrowTsRow {
    pub id: i64,
    #[column(from_i32)]
    pub ts: i32,
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
    time::sleep(Duration::from_millis(500)).await;

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
    time::sleep(Duration::from_millis(500)).await;

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
    time::sleep(Duration::from_millis(500)).await;

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
    assert!(err.starts_with("Err(ConstraintViolation("));

    log("DELETE query and make sure data is gone");
    let rows_affected = client_1
        .execute("DELETE FROM test WHERE id = $1", params!(1))
        .await?;
    assert_eq!(rows_affected, 1);

    // wait for the delete to apply on the local replica before asserting it is gone
    time::sleep(Duration::from_millis(500)).await;

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
    let mut row = rows.remove(0)?;
    assert_eq!(row.get::<i64>("id"), data.id);
    assert_eq!(row.get::<i64>("ts"), data.ts);
    assert_eq!(
        row.get::<Option<String>>("description").as_deref(),
        Some("Execute Returning Data")
    );

    log("Test Execute Returning Mapped");
    let data = TestData {
        id: 8,
        ts: Utc::now().timestamp(),
        description: None,
    };
    let mut rows: Vec<Result<TestData, Error>> = client_1
        .execute_returning_map(
            "INSERT INTO test VALUES ($1, $2, $3) RETURNING *",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows.len(), 1);
    let row = rows.remove(0)?;
    assert_eq!(row.id, data.id);
    assert_eq!(row.ts, data.ts);
    assert_eq!(row.description, data.description);

    log("Running the non-panicking query_try_map* tests");
    test_try_map(client_1).await?;

    Ok(())
}

// Exercises the non-panicking `query_try_map*` counterparts against a live cluster. A row that
// cannot be converted is reported as an `Error` (per row) instead of aborting the whole query,
// which is exactly what these functions exist to guarantee.
async fn test_try_map(client_1: &Client) -> Result<(), Error> {
    log("Inserting rows for the non-panicking query_try_map* tests");
    let rows_affected = client_1
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(100i64, 111i64, Some("try row 100".to_string())),
        )
        .await?;
    assert_eq!(rows_affected, 1);
    let rows_affected = client_1
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(101i64, 222i64, None::<String>),
        )
        .await?;
    assert_eq!(rows_affected, 1);
    // `ts` outside the i32 range: stored fine as INTEGER, but narrowing it to i32 must fail as an
    // error, not a panic.
    let rows_affected = client_1
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(102i64, i64::MAX, Some("overflow".to_string())),
        )
        .await?;
    assert_eq!(rows_affected, 1);

    // wait for the writes to be readable before asserting on them (cluster replication)
    time::sleep(Duration::from_millis(500)).await;

    log("query_try_map: convertible rows map to Ok values");
    let res: Vec<Result<TestTryRow, Error>> = client_1
        .query_try_map(
            "SELECT * FROM test WHERE id IN ($1, $2) ORDER BY id",
            params!(100i64, 101i64),
        )
        .await?;
    assert_eq!(res.len(), 2);
    match &res[0] {
        Ok(row) => {
            assert_eq!(row.id, 100);
            assert_eq!(row.ts, 111);
            assert_eq!(row.description.as_deref(), Some("try row 100"));
        }
        Err(e) => panic!("expected an Ok row for id=100, got {e:?}"),
    }
    match &res[1] {
        Ok(row) => {
            assert_eq!(row.id, 101);
            assert_eq!(row.ts, 222);
            assert_eq!(row.description.as_deref(), None);
        }
        Err(e) => panic!("expected an Ok row for id=101, got {e:?}"),
    }

    log("query_try_map: a single bad row is isolated as an error, not a panic");
    let res: Vec<Result<NarrowTsRow, Error>> = client_1
        .query_try_map(
            "SELECT id, ts FROM test WHERE id IN ($1, $2) ORDER BY id",
            params!(100i64, 102i64),
        )
        .await?;
    assert_eq!(res.len(), 2);
    match &res[0] {
        Ok(row) => assert_eq!(row.ts, 111), // fits i32
        Err(e) => panic!("expected an Ok row for id=100, got {e:?}"),
    }
    match &res[1] {
        Err(Error::Sqlite(msg)) => {
            assert!(
                msg.contains("does not fit into i32"),
                "unexpected message: {msg}"
            )
        }
        other => panic!("expected an Err(Sqlite) for id=102, got {other:?}"),
    }

    log("query_try_map_one: exactly one convertible row maps to Ok(row)");
    let res: Result<TestTryRow, Error> = client_1
        .query_try_map_one("SELECT * FROM test WHERE id = $1", params!(100i64))
        .await?;
    match res {
        Ok(row) => assert_eq!(row.id, 100),
        Err(e) => panic!("expected an Ok row for id=100, got {e:?}"),
    }

    log("query_try_map_one: no rows is an error, not a panic");
    let res: Result<Result<TestTryRow, Error>, Error> = client_1
        .query_try_map_one("SELECT * FROM test WHERE id = $1", params!(999i64))
        .await;
    match res {
        Err(Error::QueryReturnedNoRows(_)) => {}
        other => panic!("expected QueryReturnedNoRows, got {other:?}"),
    }

    log("query_try_map_one: a single bad row is Err(...), not an outer error");
    let res: Result<NarrowTsRow, Error> = client_1
        .query_try_map_one("SELECT id, ts FROM test WHERE id = $1", params!(102i64))
        .await?;
    match res {
        Err(Error::Sqlite(msg)) => {
            assert!(
                msg.contains("does not fit into i32"),
                "unexpected message: {msg}"
            )
        }
        other => panic!("expected an Err(Sqlite) for id=102, got {other:?}"),
    }

    log("query_try_map_optional: no rows is None");
    let res: Option<Result<TestTryRow, Error>> = client_1
        .query_try_map_optional("SELECT * FROM test WHERE id = $1", params!(999i64))
        .await?;
    match res {
        None => {}
        Some(other) => panic!("expected None, got {other:?}"),
    }

    log("query_try_map_optional: one convertible row is Some(Ok(row))");
    let res: Option<Result<TestTryRow, Error>> = client_1
        .query_try_map_optional("SELECT * FROM test WHERE id = $1", params!(100i64))
        .await?;
    match res {
        Some(Ok(row)) => assert_eq!(row.id, 100),
        other => panic!("expected Some(Ok(row)), got {other:?}"),
    }

    log("query_try_map_optional: a single bad row is Some(Err(...))");
    let res: Option<Result<NarrowTsRow, Error>> = client_1
        .query_try_map_optional("SELECT id, ts FROM test WHERE id = $1", params!(102i64))
        .await?;
    match res {
        Some(Err(Error::Sqlite(msg))) => {
            assert!(
                msg.contains("does not fit into i32"),
                "unexpected message: {msg}"
            )
        }
        other => panic!("expected Some(Err(Sqlite)), got {other:?}"),
    }

    log("Cleaning up the non-panicking query_try_map* test rows");
    let rows_affected = client_1
        .execute(
            "DELETE FROM test WHERE id IN ($1, $2, $3)",
            params!(100i64, 101i64, 102i64),
        )
        .await?;
    assert_eq!(rows_affected, 3);

    Ok(())
}
