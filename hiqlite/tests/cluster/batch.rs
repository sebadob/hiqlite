use crate::execute_query::TestData;
use crate::log;
use chrono::Utc;
use hiqlite::{params, Client, Error, Param};
use std::time::Duration;
use tokio::time;

pub async fn test_batch(
    client_1: &Client,
    client_2: &Client,
    client_3: &Client,
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
            (23, {now}, null);

       -- comments should be ignored and not throw errors
       INSERT INTO test VALUES (21, {now}, "This should error - unique key constraint");
        "#
        ))
        .await?;

    let rows_affected = results.remove(0)?;
    assert_eq!(rows_affected, 3);

    let should_be_err = results.remove(0);
    assert!(should_be_err.is_err());

    log("Make sure the other clients see the batch insertions");
    // race condition when we read too fast
    time::sleep(Duration::from_millis(100)).await;

    let data: Vec<TestData> = client_2
        .query_map("SELECT * FROM test WHERE id > $1", params!(20))
        .await?;
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].id, 21);
    assert_eq!(data[0].ts, now);
    assert_eq!(data[0].description.as_deref(), Some("Batch Data 1"));

    assert_eq!(data[1].id, 22);
    assert_eq!(data[1].ts, now);
    assert_eq!(data[1].description.as_deref(), Some("Batch Data 2"));

    assert_eq!(data[2].id, 23);
    assert_eq!(data[2].ts, now);
    assert_eq!(data[2].description, None);

    let data: Vec<TestData> = client_3
        .query_as("SELECT * FROM test WHERE id > $1", params!(20))
        .await?;
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].id, 21);
    assert_eq!(data[0].ts, now);
    assert_eq!(data[0].description.as_deref(), Some("Batch Data 1"));

    assert_eq!(data[1].id, 22);
    assert_eq!(data[1].ts, now);
    assert_eq!(data[1].description.as_deref(), Some("Batch Data 2"));

    assert_eq!(data[2].id, 23);
    assert_eq!(data[2].ts, now);
    assert_eq!(data[2].description, None);

    Ok(())
}
