use crate::execute_query::{TestData, TestTryRow};
use crate::log;
use hiqlite::macros::params;
use hiqlite::{Client, Error, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

// Exercises the `query_*_stream` variants on a local client: rows arrive as they are consumed, a
// stream that is dropped early returns its connection to the read pool, a query that cannot run
// fails on its build, before a single row is on its way, and a query that fails on one of its rows
// hands that row over as the `Err` of its item, or ends the stream on `next()`.
pub async fn test_query_streams(client: &Client) -> Result<(), Error> {
    log("Inserting rows for the query_*_stream tests");
    for id in 200..250i64 {
        let rows_affected = client
            .execute(
                "INSERT INTO test VALUES ($1, $2, $3)",
                params!(id, id * 10, Some(format!("stream row {id}"))),
            )
            .await?;
        assert_eq!(rows_affected, 1);
    }

    // The inserts travel through Raft; the local read pool only answers once they are applied.
    time::sleep(Duration::from_millis(500)).await;

    log("Streaming every row of a mapped query");
    let mut stream = client
        .query_map_stream::<TestData, _>(
            "SELECT * FROM test WHERE id >= $1 ORDER BY id",
            params!(200i64),
        )
        .await?;

    let mut ids = Vec::new();
    while let Some(row) = stream.next().await? {
        ids.push(row.id);
    }
    assert_eq!(ids, (200..250).collect::<Vec<i64>>());

    log("Streaming a mapped query with a conversion and a limit in flight");
    // The `StreamExt` shape of the very same stream: the items of a combinator are the `Item` of
    // the stream, and `take(5)` must not read a single row behind it.
    let mut stream = client
        .query_map_stream::<TestData, _>(
            "SELECT * FROM test WHERE id >= $1 ORDER BY id",
            params!(200i64),
        )
        .await?
        .map(|res| res.map(|row| row.id))
        .take(5);
    let mut ids = Vec::new();
    while let Some(res) = stream.next().await {
        ids.push(res?);
    }
    assert_eq!(ids, (200..205).collect::<Vec<i64>>());

    log("Streaming the non-panicking query_try_map_stream");
    let mut stream = client
        .query_try_map_stream::<TestTryRow, _>(
            "SELECT * FROM test WHERE id >= $1 ORDER BY id",
            params!(200i64),
        )
        .await?;
    let mut rows = 0;
    while let Some(res) = stream.next().await? {
        let _row = res?;
        rows += 1;
    }
    assert_eq!(rows, 50);

    log("Streaming the bare rows of query_raw_stream");
    let mut stream = client
        .query_raw_stream(
            "SELECT id, description FROM test WHERE id >= $1 ORDER BY id",
            params!(200i64),
        )
        .await?;
    let mut rows = 0;
    while let Some(mut row) = stream.next().await? {
        // The header of the result set is on every row of a raw stream, shared through its `Arc`.
        assert_eq!(row.columns(), ["id", "description"].map(Arc::from));
        let id = row.get::<i64>("id");
        assert_eq!(id, 200 + rows);
        assert_eq!(
            row.get::<String>("description"),
            format!("stream row {}", 200 + rows)
        );
        rows += 1;
    }
    assert_eq!(rows, 50);

    log("Streaming the bare rows of a raw query into a JSON API response");
    let stream = client
        .query_raw_stream(
            "SELECT id, description FROM test WHERE id >= $1 ORDER BY id",
            params!(200i64),
        )
        .await?;
    let body = axum::body::to_bytes(stream.into_json_response().into_body(), 128 * 1024)
        .await
        .map_err(|err| Error::Config(err.to_string().into()))?;
    let json =
        String::from_utf8(body.to_vec()).map_err(|err| Error::Config(err.to_string().into()))?;
    let lines: Vec<&str> = json.lines().collect();
    // One JSON object per row, keyed by the column names of the query, in result-set order.
    assert_eq!(lines.len(), 50);
    assert_eq!(lines[0], r#"{"id":200,"description":"stream row 200"}"#);
    assert_eq!(lines[49], r#"{"id":249,"description":"stream row 249"}"#);

    log("Inserting a row whose `ts` column holds a text value");
    // SQLite stores what you give it, so this row is the one a mapping into `i64` fails on, and it
    // sits behind every row a stream below reads successfully.
    let rows_affected = client
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(260i64, "not-a-number", Some("bad row".to_string())),
        )
        .await?;
    assert_eq!(rows_affected, 1);
    time::sleep(Duration::from_millis(500)).await;

    log("Making sure a row that cannot be mapped is the Err of its item");
    let mut stream = client
        .query_try_map_stream::<TestTryRow, _>(
            "SELECT * FROM test WHERE id >= $1 ORDER BY id",
            params!(200i64),
        )
        .await?;
    let mut good = 0;
    let mut row_err: Option<Error> = None;
    while let Some(res) = stream.next().await? {
        match res {
            Ok(_row) => good += 1,
            Err(err) => row_err = Some(err),
        }
    }
    assert_eq!(good, 50);
    let err = row_err.expect("the bad row must come back as the Err of its item");
    assert!(
        matches!(err, Error::Sqlite(_)),
        "expected the conversion failure of the row: {err:?}"
    );

    log("Making sure a cursor that fails mid-scan ends the stream on next()");
    // SQLite fails on the step that produces row 205: a `randomblob` over the maximum value size is
    // a real cursor error, and it happens after the stream has delivered the rows before it.
    let mut stream = client
        .query_raw_stream(
            "SELECT id, CASE WHEN id = $1 THEN randomblob($2) ELSE description END \
             FROM test WHERE id >= $3 ORDER BY id",
            params!(205i64, 100_000_000_000i64, 200i64),
        )
        .await?;
    let mut rows = 0;
    let mut stream_err: Option<Error> = None;
    loop {
        match stream.next().await {
            Ok(Some(_row)) => rows += 1,
            Ok(None) => break,
            Err(err) => {
                stream_err = Some(err);
                break;
            }
        }
    }
    assert_eq!(rows, 5, "the rows before the failing one are delivered");
    let err = stream_err.expect("a failing cursor must end the stream with an Err");
    assert!(
        matches!(err, Error::Sqlite(_)),
        "expected the cursor error: {err:?}"
    );
    // A truncated stream stays truncated: it never reads as a result that ran to its end.
    assert!(
        stream.next().await?.is_none(),
        "a failed stream is closed after its Err"
    );

    log("Making sure a stream that is dropped early hands its connection back");
    // More streams dropped mid-query than the read pool has connections: a single leaked
    // connection would leave the query below waiting for one that never comes.
    for _ in 0..20 {
        let mut stream = client
            .query_map_stream::<TestData, _>(
                "SELECT * FROM test WHERE id >= $1 ORDER BY id",
                params!(200i64),
            )
            .await?;
        let first = stream.next().await?.expect("a row");
        assert_eq!(first.id, 200);
        drop(stream);
    }
    // The non-panicking read, because the set below holds the bad row of the section above.
    let rows: Vec<Result<TestTryRow, Error>> = client
        .query_try_map("SELECT * FROM test WHERE id >= $1", params!(200i64))
        .await?;
    assert_eq!(rows.len(), 51);
    assert_eq!(rows.iter().filter(|row| row.is_ok()).count(), 50);

    log("Making sure a query that cannot run fails on its build, not on a row");
    let res = client
        .query_map_stream::<TestData, _>("SELECT * FROM no_such_table", params!())
        .await;
    assert!(res.is_err());

    log("Cleaning up the query_*_stream test rows");
    let rows_affected = client
        .execute("DELETE FROM test WHERE id >= $1", params!(200i64))
        .await?;
    assert_eq!(rows_affected, 51);

    Ok(())
}
