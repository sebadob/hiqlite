use crate::execute_query::TestData;
use crate::start::SECRET_API;
use crate::{check, log, start, Cache};
use chrono::Utc;
use hiqlite::{params, Client, Error, Lock, Node, Param};
use std::time::Duration;
use tokio::{task, time};

pub async fn test_remote_only_client() -> Result<(), Error> {
    log("Make sure remote clients work fine with any member node, even if none leader");

    let client_1 = build_client(1);
    let client_2 = build_client(2);
    let client_3 = build_client(3);

    check_client(&client_1, 1).await?;
    check_client(&client_2, 2).await?;
    check_client(&client_3, 3).await?;

    Ok(())
}

async fn check_client(client: &Client, id: u64) -> Result<(), Error> {
    check::is_client_db_healthy(&client, Some(id)).await?;

    log(format!("Test remote client {} database", id));

    // single execute / query
    let data = TestData {
        id: 1337,
        ts: Utc::now().timestamp(),
        description: Some("My Remote Row".to_string()),
    };
    let rows_affected = client
        .execute(
            "INSERT INTO test VALUES ($1, $2, $3)",
            params!(data.id, data.ts, data.description.clone()),
        )
        .await?;
    assert_eq!(rows_affected, 1);

    let res: TestData = client
        .query_map_one("SELECT * FROM test WHERE id = $1", params!(data.id))
        .await?;
    assert_eq!(res.id, data.id);
    assert_eq!(res.ts, data.ts);
    assert_eq!(res.description, data.description);

    let rows_affected = client
        .execute("DELETE FROM test WHERE id = $1", params!(data.id))
        .await?;
    assert_eq!(rows_affected, 1);

    let res: Option<TestData> = client
        .query_map_one("SELECT * FROM test WHERE id = $1", params!(data.id))
        .await
        .ok();
    assert!(res.is_none());

    // transaction

    let sql = "INSERT INTO test VALUES ($1, $2, $3)";
    let now = Utc::now().timestamp();
    let results = client
        .txn([
            (sql, params!(1001, now, "Transaction Data id 1001")),
            (sql, params!(1002, now, "Transaction Data id 1002")),
        ])
        // The first result returned is for the whole transaction commit
        .await?;
    assert_eq!(results.iter().len(), 2);

    for res in results {
        // each result in the returned vector is for
        // the single queries in the exact same order
        assert!(res.is_ok());
    }

    let data: Vec<TestData> = client
        .query_map("SELECT * FROM test WHERE id >= $1", params!(1001))
        .await?;
    assert_eq!(data.len(), 2);

    assert_eq!(data[0].id, 1001);
    assert_eq!(data[0].ts, now);
    assert_eq!(
        data[0].description.as_deref(),
        Some("Transaction Data id 1001")
    );

    assert_eq!(data[1].id, 1002);
    assert_eq!(data[1].ts, now);
    assert_eq!(
        data[1].description.as_deref(),
        Some("Transaction Data id 1002")
    );

    // batch
    let results = client
        .batch(
            r#"
        DELETE FROM test WHERE id = 1001;
        DELETE FROM test WHERE id = 1002;
        "#,
        )
        .await?;

    for res in results {
        let rows_affected = res?;
        assert_eq!(rows_affected, 1);
    }

    let data: Vec<TestData> = client
        .query_map("SELECT * FROM test WHERE id >= $1", params!(1001))
        .await?;
    assert_eq!(data.len(), 0);

    log(format!("Test remote client {} cache", id));
    let key = "remote_key";
    let value = "remote Value";
    client
        .put(Cache::One, key, &value.to_string(), None)
        .await?;

    let v: String = client.get(Cache::One, key).await?.unwrap();
    assert_eq!(&v, value);

    client.delete(Cache::One, key).await?;

    let v: Option<String> = client.get(Cache::One, key).await?;
    assert!(v.is_none());

    log(format!("Test remote client {} locks", id));
    let lock = client.lock("remote").await?;

    let cl = client.clone();
    let handle = task::spawn(async move {
        let lock = cl.lock("remote").await?;
        Ok::<Lock, Error>(lock)
    });

    time::sleep(Duration::from_millis(100)).await;
    assert!(!handle.is_finished());

    drop(lock);
    let _lock_handle = handle.await??;

    Ok(())
}

fn build_client(node_id: u64) -> Client {
    let addr = start::nodes()
        .into_iter()
        .filter(|n| n.id == node_id)
        .collect::<Vec<Node>>()
        .swap_remove(0)
        .addr_api;

    Client::remote(node_id, addr, false, false, SECRET_API.to_string())
}
