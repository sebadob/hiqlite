use crate::execute_query::TestData;
use crate::log;
use chrono::Utc;
use hiqlite::{params, Client, Error, Param};
use std::time::Duration;
use tokio::time;

pub async fn test_transactions(
    client_1: &Client,
    client_2: &Client,
    client_3: &Client,
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
    time::sleep(Duration::from_millis(10)).await;

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
