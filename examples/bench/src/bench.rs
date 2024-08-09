use crate::{log, Cache, Options};
use chrono::Utc;
use hiqlite::{params, Client, Error, Param, Params, Row};
use serde::{Deserialize, Serialize};
use tokio::task;
use tokio::time::Instant;

/// Matches our test table for this example.
/// serde derives are needed if you want to use the `query_as()` fn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Entity {
    pub id: i64,
    pub ts: i64,
    pub name: String,
}

// This impl is needed for the more efficient and faster `query_map()`
impl<'r> From<Row<'r>> for Entity {
    fn from(mut row: Row<'r>) -> Self {
        Self {
            id: row.get("id"),
            ts: row.get("ts"),
            name: row.get("name"),
        }
    }
}

pub async fn start_benchmark(client: Client, options: Options) -> Result<(), Error> {
    let data = prepare_data(&options);
    let concurrency = options.concurrency;
    let rows = options.rows;

    let elapsed = insert_concurrent(client.clone(), &options, data.clone(), false).await?;
    let per_second = rows * 1000 / elapsed as usize;
    log(format!(
        "{} single INSERTs with concurrency {} took:\n{} ms -> {} inserts / s",
        rows, concurrency, elapsed, per_second
    ));

    cleanup(&client).await?;

    let elapsed = insert_concurrent(client.clone(), &options, data.clone(), true).await?;
    let per_second = rows * 1000 / elapsed as usize;
    log(format!(
        "{} transactional / batched INSERTs with concurrency {} took:\n{} ms -> {} inserts / s",
        rows, concurrency, elapsed, per_second
    ));

    select_timings(client.clone()).await?;

    let elapsed = put_cache(client.clone(), &options, data).await?;
    let per_second = rows * 1000 / elapsed as usize;
    log(format!(
        "{} single cache PUTs with concurrency {} took:\n{} ms -> {} inserts / s",
        rows, concurrency, elapsed, per_second
    ));

    get_timings(client.clone()).await?;

    Ok(())
}

async fn insert_concurrent(
    client: Client,
    options: &Options,
    sets: Vec<Vec<Entity>>,
    with_txn: bool,
) -> Result<u128, Error> {
    let concurrency = options.concurrency;

    let mut handles = Vec::with_capacity(concurrency);
    let start = Instant::now();
    for set in sets {
        let client = client.clone();

        let handle = if with_txn {
            task::spawn(async move {
                let mapped: Vec<(&str, Params)> = set
                    .into_iter()
                    .map(|t| {
                        (
                            "INSERT INTO test (id, ts, name) VALUES ($1, $2, $3)",
                            params!(t.id, t.ts, t.name),
                        )
                    })
                    .collect();

                for res in client.txn(mapped).await.unwrap() {
                    assert_eq!(res.unwrap(), 1);
                }
            })
        } else {
            task::spawn(async move {
                for entity in set {
                    let rows_affected = client
                        .execute(
                            "INSERT INTO test (id, ts, name) VALUES ($1, $2, $3)",
                            params!(entity.id, entity.ts, entity.name.clone()),
                        )
                        .await
                        .unwrap();
                    assert_eq!(rows_affected, 1);
                }
            })
        };

        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    Ok(start.elapsed().as_millis())
}

async fn select_timings(client: Client) -> Result<(), Error> {
    let select = "SELECT * FROM test WHERE id = $1";

    let start = Instant::now();
    let _: Entity = client.query_map_one(select, params!(1)).await?;
    log(format!("SELECT a single row with a fresh connection and no cached prepared query using `query_map_one` took:\n{} micros", start.elapsed().as_micros()));

    log("SELECT a single row with a cached prepared statement using `query_map_one`:");
    for _ in 0..10 {
        let start = Instant::now();
        let _: Entity = client.query_map_one(select, params!(1)).await?;
        println!("{} micros", start.elapsed().as_micros());
    }

    log("SELECT a single row with a cached prepared statement using `query_as_one`:");
    for _ in 0..10 {
        let start = Instant::now();
        let _: Entity = client.query_as_one(select, params!(1)).await?;
        println!("{} micros", start.elapsed().as_micros());
    }

    let start = Instant::now();
    let _: Vec<Entity> = client.query_map("SELECT * FROM test", params!()).await?;
    log(format!(
        "SELECT all rows using `query_map` took:\n{} micros",
        start.elapsed().as_micros()
    ));

    let start = Instant::now();
    let _: Vec<Entity> = client.query_as("SELECT * FROM test", params!()).await?;
    log(format!(
        "SELECT all rows using `query_as` took:\n{} micros",
        start.elapsed().as_micros()
    ));

    Ok(())
}

async fn put_cache(
    client: Client,
    options: &Options,
    sets: Vec<Vec<Entity>>,
) -> Result<u128, Error> {
    let concurrency = options.concurrency;

    let mut handles = Vec::with_capacity(concurrency);
    let start = Instant::now();
    for set in sets {
        let client = client.clone();

        let handle = task::spawn(async move {
            for entity in set {
                client
                    .put(Cache::One, entity.name.clone(), &entity, None)
                    .await
                    .unwrap();
            }
        });

        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    Ok(start.elapsed().as_millis())
}

async fn get_timings(client: Client) -> Result<(), Error> {
    let key = "Name 1";

    log("Cache GET for a single entry :");
    for _ in 0..10 {
        let start = Instant::now();
        let _: Entity = client.get(Cache::One, key).await?.unwrap();
        println!("{} micros", start.elapsed().as_micros());
    }

    Ok(())
}

async fn cleanup(client: &Client) -> Result<(), Error> {
    client.execute("DELETE FROM test", params!()).await?;
    Ok(())
}

fn prepare_data(options: &Options) -> Vec<Vec<Entity>> {
    let concurrency = options.concurrency;
    let rows = options.rows;

    let mut sets = Vec::with_capacity(concurrency);
    let mut idx = 0;
    for _ in 1..=concurrency {
        let mut data = Vec::with_capacity(rows);
        for _ in 0..(rows / concurrency) {
            // let mut id = Uuid::now_v7().as_bytes().to_vec();
            // id.push(i as u8);
            data.push(Entity {
                id: idx,
                ts: Utc::now().timestamp(),
                name: format!("Name {}", idx),
            });
            idx += 1;
        }
        sets.push(data);
    }

    sets
}
