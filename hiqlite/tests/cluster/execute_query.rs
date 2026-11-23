use crate::log;
use chrono::Utc;
use hiqlite::macros::{FromRow, TryFromRow, params};
use hiqlite::{Client, Error, ReadPolicy};
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

// Hand-written counterpart of `NarrowTsRow`, with the non-panicking family written by hand: a
// `TryFrom<&mut Row>` impl reading the names it passes to `row.try_get`.  The derive is what puts
// a type into the registry of hoisted mappings, so this type has no hoisted mapping, and
// `query_try_map*` runs this body on every row, exactly like it did before.
#[derive(Debug, Clone, PartialEq)]
pub struct ManualTryRow {
    pub id: i64,
    pub ts: i32,
}

impl TryFrom<&mut hiqlite::Row<'_>> for ManualTryRow {
    type Error = hiqlite::Error;

    fn try_from(row: &mut hiqlite::Row<'_>) -> Result<Self, Self::Error> {
        // The same narrowing the `#[column(from_i32)]` attribute of the derive does, by hand.
        let ts = row.try_get::<i64>("ts")?;
        Ok(Self {
            id: row.try_get("id")?,
            ts: i32::try_from(ts)
                .map_err(|_| hiqlite::Error::Sqlite("does not fit into i32".into()))?,
        })
    }
}

// Reads the column `id`.  The query below selects `id` twice, so this pins down which of the two
// equally named result columns a mapping resolves to.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct FirstDupId {
    pub id: i64,
}

// Reads the column `idx`.  The query below aliases `id` as `IdX`, which only rusqlite's own,
// case-insensitive name resolution matches.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct AliasCaseRow {
    pub idx: i64,
}

// Derived types are mapped by `query_map*` through the hoisted index path: the names of these
// fields are resolved against the result set once per query, so the queries below feed them a
// permuted, an aliased and a duplicated column order without changing what they read.  A struct
// with a `flatten` field is the exception this suite also covers - it stays on the name-based path.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct IndexedRow {
    pub id: i64,
    pub ts: i64,
    #[column(rename = "description")]
    pub desc: Option<String>,
    #[column(skip)]
    pub extra: u32,
}

/// The nested half of `IndexedOuter`; its field names are part of the parent's shape, and reading
/// it goes through its own `From<&mut Row>`.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct IndexedInner {
    pub ts: i64,
}

/// A flattened field keeps this type on the name-based mapping, which is what the queries of
/// `IndexedOuter` below exercise.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct IndexedOuter {
    pub id: i64,
    #[column(flatten)]
    pub inner: IndexedInner,
}

/// The field `desc` is named in a case that matches `description` and `DESCRIPTION`
/// case-insensitively and neither of them exactly, which is the shape of a name lookup that has to
/// fall back to the case-insensitive scan.  The query in `test_execute_query` answers those two
/// names as a text and an integer column, so a fallback that picks the wrong one is visible in the
/// mapped value.
#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct IndexedCaseRow {
    pub id: i64,
    #[column(rename = "dEscRiPtIoN")]
    pub desc: Option<String>,
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
    let err = format!("{res:?}");
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
        .query_consistent_map(
            "SELECT * FROM test WHERE id = $1",
            params!(3),
            ReadPolicy::ReadIndex,
        )
        .await?;
    assert_eq!(res[0].id, data.id);
    assert_eq!(res[0].ts, data.ts);
    assert_eq!(res[0].description, data.description);

    // client_2 asks for the lease policy, which must land on the exact same result
    let res: Vec<TestData> = client_2
        .query_consistent_map(
            "SELECT * FROM test WHERE id = $1",
            params!(3),
            ReadPolicy::LeaseRead,
        )
        .await?;
    assert_eq!(res[0].id, data.id);
    assert_eq!(res[0].ts, data.ts);
    assert_eq!(res[0].description, data.description);

    let res: Vec<TestData> = client_3
        .query_consistent_map(
            "SELECT * FROM test WHERE id = $1",
            params!(3),
            ReadPolicy::ReadIndex,
        )
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
    test_try_map(client_1, client_2).await?;

    Ok(())
}

// Exercises the non-panicking `query_try_map*` counterparts against a live cluster. A row that
// cannot be converted is reported as an `Error` (per row) instead of aborting the whole query,
// which is exactly what these functions exist to guarantee.
async fn test_try_map(client_1: &Client, client_2: &Client) -> Result<(), Error> {
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
            );
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
            );
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
            );
        }
        other => panic!("expected Some(Err(Sqlite)), got {other:?}"),
    }

    log("query_map: a duplicated result column name maps to the first occurrence");
    let rows: Vec<FirstDupId> = client_1
        .query_map(
            "SELECT id, ts AS id FROM test WHERE id = $1",
            params!(100i64),
        )
        .await?;
    // `ts` is far above any id, so the value tells us which of the two `id` columns was read.
    assert_eq!(rows, vec![FirstDupId { id: 100 }]);

    log("query_map: a column name that only differs in case is resolved as well");
    let rows: Vec<AliasCaseRow> = client_1
        .query_map("SELECT id AS IdX FROM test WHERE id = $1", params!(101i64))
        .await?;
    assert_eq!(rows, vec![AliasCaseRow { idx: 101 }]);

    log("query_map: a derived struct maps by name, in any column order");
    let rows: Vec<IndexedRow> = client_1
        .query_map(
            "SELECT ts, description, id FROM test WHERE id = $1",
            params!(100i64),
        )
        .await?;
    assert_eq!(
        rows,
        vec![IndexedRow {
            id: 100,
            ts: 111,
            desc: Some("try row 100".to_string()),
            extra: 0,
        }]
    );

    log("query_map: a flattened struct reads its parent and its nested fields from one result set");
    let rows: Vec<IndexedOuter> = client_1
        .query_map("SELECT ts, id FROM test WHERE id = $1", params!(101i64))
        .await?;
    assert_eq!(
        rows,
        vec![IndexedOuter {
            id: 101,
            inner: IndexedInner { ts: 222 },
        }]
    );

    log("query_map: names that only differ in case are resolved as well");
    let rows: Vec<IndexedRow> = client_1
        .query_map(
            "SELECT id AS Id, ts AS TS, description AS Description FROM test WHERE id = $1",
            params!(100i64),
        )
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 100);
    assert_eq!(rows[0].ts, 111);

    // A field that no column matches exactly resolves through the case-insensitive scan, and that
    // scan has one answer on the leader's own rows (rusqlite walks the columns in order) and,
    // before, a hash-ordered one on the rows of a remote client.  `description` and `DESCRIPTION`
    // are the same name twice, a text column and an integer column, so a wrong pick lands in the
    // mapped value.
    log("query_map: a case-only name match reads the same column on a local and a remote client");
    let sql = "SELECT id, description, ts AS DESCRIPTION FROM test WHERE id = $1";
    let rows: Vec<IndexedCaseRow> = client_1.query_map(sql, params!(100i64)).await?;
    assert_eq!(
        rows,
        vec![IndexedCaseRow {
            id: 100,
            desc: Some("try row 100".to_string()),
        }],
        "the leader reads the column its scan comes to first"
    );
    let rows: Vec<IndexedCaseRow> = client_2.query_map(sql, params!(100i64)).await?;
    assert_eq!(
        rows,
        vec![IndexedCaseRow {
            id: 100,
            desc: Some("try row 100".to_string()),
        }],
        "a remote client maps the same query onto the same column"
    );

    // A `FromRow` mapping is the panicking family: a field whose column is missing in the result
    // set panics with that column name, on the first row that reads it.  This suite cannot show a
    // panic, because the workspace builds with `panic = "abort"`, so it takes the test binary with
    // it; the panic and its message are asserted in the unit tests of the query module.
    // What it can show is the other half of the same rule: a result set with no rows has no row
    // that reads the missing column, so it answers empty like any other empty result set does.
    log("query_map: a missing column is not reported by a result set that has no rows");
    let rows: Vec<IndexedRow> = client_1
        .query_map("SELECT id FROM test WHERE id = $1", params!(999i64))
        .await?;
    assert!(
        rows.is_empty(),
        "no rows, so nothing reads the missing column"
    );

    // `query_try_map` is the non-panicking family, and it reports the same missing column per row,
    // naming the field it cannot read.
    log("query_try_map: the same missing column is an Error naming it, per row");
    let res: Result<Vec<Result<TestTryRow, Error>>, Error> = client_1
        .query_try_map("SELECT id FROM test WHERE id = $1", params!(100i64))
        .await;
    let rows = res.expect("a query with rows answers its rows");
    assert_eq!(rows.len(), 1);
    match &rows[0] {
        Err(Error::QueryParams(msg)) => assert!(msg.contains("ts"), "unexpected message: {msg}"),
        other => panic!("expected QueryParams about the missing column, got {other:?}"),
    }

    let res: Vec<Result<TestTryRow, Error>> = client_1
        .query_try_map("SELECT id FROM test WHERE id = $1", params!(999i64))
        .await
        .expect("an empty result set answers empty, whatever its fields are named");
    assert!(res.is_empty());

    // A hand-written `impl From<&mut Row>` has no field maps to resolve, so `query_map` reads it
    // through the names of its rows, exactly like before.
    log("query_map: a hand-written From<&mut Row> impl keeps its name-based mapping");
    let row: TestData = client_1
        .query_map_one(
            "SELECT ts, description, id FROM test WHERE id = $1",
            params!(100i64),
        )
        .await?;
    assert_eq!(row.id, 100);
    assert_eq!(row.ts, 111);
    assert_eq!(row.description.as_deref(), Some("try row 100"));

    log("query_map_one and query_map_optional map a single row through the same index");
    let row: IndexedRow = client_1
        .query_map_one(
            "SELECT id, ts, description FROM test WHERE id = $1",
            params!(101i64),
        )
        .await?;
    assert_eq!(
        row,
        IndexedRow {
            id: 101,
            ts: 222,
            desc: None,
            extra: 0,
        }
    );
    // `IndexedOuter` needs exactly `id` and `ts`, so this result set covers all of its fields.
    let row: Option<IndexedOuter> = client_1
        .query_map_optional("SELECT id, ts FROM test WHERE id = $1", params!(999i64))
        .await?;
    assert!(row.is_none());

    log("query_try_map: a row that cannot convert is the Err of its own slot");
    let rows: Vec<Result<NarrowTsRow, Error>> = client_1
        .query_try_map(
            "SELECT id, ts FROM test WHERE id IN ($1, $2) ORDER BY id",
            params!(100i64, 102i64),
        )
        .await?;
    assert_eq!(rows.len(), 2);
    assert!(rows[0].is_ok());
    match &rows[1] {
        Err(Error::Sqlite(msg)) => assert!(msg.contains("does not fit into i32"), "{msg}"),
        other => panic!("expected Sqlite about the overflow, got {other:?}"),
    }

    log(
        "query_consistent_map: the hoisted index path over rows that travelled through the network",
    );
    let rows: Vec<IndexedRow> = client_1
        .query_consistent_map(
            "SELECT ts, description, id FROM test WHERE id = $1",
            params!(100i64),
            ReadPolicy::ReadIndex,
        )
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 100);
    assert_eq!(rows[0].ts, 111);
    assert_eq!(rows[0].desc.as_deref(), Some("try row 100"));

    // The same mappings on a client that is not the leader: the rows arrive as owned values and
    // are mapped on the client side, which is a second implementation of the same two paths.
    log("query_map from a non-leader client: the hoisted index path over network rows");
    let rows: Vec<IndexedRow> = client_2
        .query_map(
            "SELECT ts, description, id FROM test WHERE id = $1",
            params!(100i64),
        )
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 100);
    assert_eq!(rows[0].desc.as_deref(), Some("try row 100"));

    log("query_try_map from a non-leader client: the hoisted index path over network rows");
    let rows: Vec<Result<NarrowTsRow, Error>> = client_2
        .query_try_map("SELECT ts, id FROM test WHERE id = $1", params!(100i64))
        .await?;
    assert_eq!(rows.len(), 1);
    match &rows[0] {
        Ok(row) => assert_eq!((row.id, row.ts), (100, 111)),
        Err(e) => panic!("expected an Ok row for id=100, got {e:?}"),
    }

    log("query_map_one from a non-leader client: a hand-written From<&mut Row> impl by name");
    let row: TestData = client_2
        .query_map_one(
            "SELECT ts, description, id FROM test WHERE id = $1",
            params!(101i64),
        )
        .await?;
    assert_eq!(row.id, 101);
    assert_eq!(row.ts, 222);
    assert!(row.description.is_none());

    // A hand-written `TryFrom<&mut Row>` impl is the non-panicking family without the derive: the
    // errors are the ones its own body produces, and a missing column stays the `Error::Sqlite` of
    // its `row.try_get`, because nothing about this impl is resolved ahead of the rows.
    log("query_try_map: a hand-written TryFrom<&mut Row> impl keeps its own mapping");
    let rows: Vec<Result<ManualTryRow, Error>> = client_1
        .query_try_map(
            "SELECT id, ts FROM test WHERE id IN ($1, $2) ORDER BY id",
            params!(100i64, 102i64),
        )
        .await?;
    assert_eq!(rows.len(), 2);
    match &rows[0] {
        Ok(row) => assert_eq!((row.id, row.ts), (100, 111)),
        Err(e) => panic!("expected an Ok row for id=100, got {e:?}"),
    }
    match &rows[1] {
        Err(Error::Sqlite(msg)) => assert!(msg.contains("does not fit into i32"), "{msg}"),
        other => panic!("expected Sqlite about the overflow, got {other:?}"),
    }

    log("query_try_map_one: a hand-written TryFrom<&mut Row> impl maps one row");
    let row: Result<ManualTryRow, Error> = client_1
        .query_try_map_one("SELECT ts, id FROM test WHERE id = $1", params!(101i64))
        .await?;
    match row {
        Ok(row) => assert_eq!((row.id, row.ts), (101, 222)),
        Err(e) => panic!("expected an Ok row for id=101, got {e:?}"),
    }

    log("query_try_map: a hand-written impl reports a missing column as its own Sqlite error");
    let rows: Vec<Result<ManualTryRow, Error>> = client_2
        .query_try_map("SELECT ts FROM test WHERE id = $1", params!(100i64))
        .await?;
    assert_eq!(rows.len(), 1);
    match &rows[0] {
        Err(Error::Sqlite(msg)) => assert!(msg.contains("id"), "unexpected message: {msg}"),
        other => panic!("expected Sqlite naming the missing column, got {other:?}"),
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
