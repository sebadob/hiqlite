use crate::{log, Cache, Options};
use chrono::Utc;
use hiqlite::macros::params;
use hiqlite::{Client, Error, Params, Row};
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::task;
use tokio::time;
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
impl From<&mut Row<'_>> for Entity {
    fn from(row: &mut Row<'_>) -> Self {
        Self {
            id: row.get("id"),
            ts: row.get("ts"),
            name: row.get("name"),
        }
    }
}

pub async fn start_benchmark(client: Client, options: Options, remote: bool) -> Result<(), Error> {
    let data = prepare_data(&options);
    let concurrency = options.concurrency;
    let rows = options.rows;

    // The in-memory cache is the Raft group with no disk in its way, so its numbers are the
    // cleanest signal for replication depth. Run it on its own to keep a run short.
    if options.cache_only {
        let (elapsed, retries) = put_cache(client.clone(), &options, data).await?;
        let per_second = rows * 1000 / max(elapsed as usize, 1);
        log(format!(
            "{rows} single cache PUTs with concurrency {concurrency} took:\n{elapsed} ms \
            -> {per_second} inserts / s{}",
            retries_note(retries)
        ));
        return Ok(());
    }

    // make sure everything is clean
    client.execute("DELETE FROM _bench", params!()).await?;

    let (elapsed, retries) = insert_concurrent(client.clone(), &options, data.clone(), false).await?;
    let per_second = rows * 1000 / max(elapsed as usize, 1);
    log(format!(
        "{rows} single INSERTs with concurrency {concurrency} took:\n{elapsed} ms \
        -> {per_second} inserts / s{}",
        retries_note(retries)
    ));

    // TEMPORARY for the pipeline benchmark: the transactional insert is a showcase of how fast a
    // transaction is, not a signal for replication depth, and it doubles the length of every run.
    // Re-activate this block and `cleanup` before pushing.
    //
    // We only want to run transactional inserts without rate-limiting. This impl is very naive, and
    // it can easily crash everything for very high row counts.
    if options.db_rps.is_none() {
        // The batch sizes here heavily influence the WebSocket buffers. If we send batches of ~10k
        // queries at once, it is very fast, but the internal buffers will grow up to a size that can
        // hold these big messages.
        cleanup(&client).await?;
        let (elapsed, retries) = insert_concurrent(client.clone(), &options, data.clone(), true).await?;
        let per_second = rows * 1000 / max(elapsed as usize, 1);
        log(format!(
            "{rows} transactional / batched INSERTs with concurrency {concurrency} \
            took:\n{elapsed} ms -> {per_second} inserts / s{}",
            retries_note(retries)
        ));
    }

    select_timings(client.clone(), remote).await?;

    let (elapsed, retries) = put_cache(client.clone(), &options, data).await?;
    let per_second = rows * 1000 / max(elapsed as usize, 1);
    log(format!(
        "{rows} single cache PUTs with concurrency {concurrency} took:\n{elapsed} ms \
        -> {per_second} inserts / s{}",
        retries_note(retries)
    ));

    get_timings(client.clone()).await?;

    Ok(())
}

/// A leader change during a run is a normal event in a Raft cluster. The client has no
/// leader-following retry for a write, so a write that lands on a node which is not the leader
/// comes back as `ForwardToLeader`, and this example repeats it a few times instead of dying.
const LEADER_RETRIES: u32 = 5;
const LEADER_RETRY_DELAY: Duration = Duration::from_millis(200);

/// `ForwardToLeader` is the shape of a write that hit a node which is not the leader, and of a node
/// that does not know who the leader is yet. Matched on the message because this example does not
/// depend on openraft directly.
/// The retry count of a measured phase, appended to its throughput line. A retry is time spent
/// inside the measured window, so a run that paid for leader changes is only comparable to another
/// run when the number of them is visible.
fn retries_note(retries: usize) -> String {
    if retries == 0 {
        String::new()
    } else {
        format!(" -> {retries} writes retried after a leader change")
    }
}

fn is_leader_change(err: &Error) -> bool {
    let msg = err.to_string();
    msg.contains("ForwardToLeader") || msg.contains("NotLeader")
}

/// Max statements per transaction.
///
/// A transaction is a single Raft entry, and a replication request carries up to
/// `max_payload_entries` entries, so the payload that travels is this count times the entry size
/// times whatever the leader has queued. If this value is too big, the Raft can run into timeouts.
const TXN_STATEMENTS: usize = 1000;

async fn insert_concurrent(
    client: Client,
    options: &Options,
    sets: Vec<Vec<Entity>>,
    with_txn: bool,
) -> Result<(u128, usize), Error> {
    let concurrency = options.concurrency;

    let mut handles = Vec::with_capacity(concurrency);
    let start = Instant::now();
    // A leader change is a normal event in a Raft cluster, and a retry is time spent inside the
    // measured window. Counting them makes a run that spent its time retrying recognisable from the
    // throughput line it prints.
    let retries = Arc::new(AtomicUsize::new(0));
    for set in sets {
        let client = client.clone();
        let retries = retries.clone();

        let handle = if with_txn {
            task::spawn(async move {
                let mapped: Vec<(&str, Params)> = set
                    .into_iter()
                    .map(|t| {
                        (
                            "INSERT INTO _bench (id, ts, name) VALUES ($1, $2, $3)",
                            params!(t.id, t.ts, t.name),
                        )
                    })
                    .collect();

                for chunk in mapped.chunks(TXN_STATEMENTS) {
                    for res in client.txn(chunk.to_vec()).await.unwrap() {
                        assert_eq!(res.unwrap(), 1);
                    }
                }
            })
        } else {
            task::spawn(async move {
                for entity in set {
                    let mut rows_affected = 0;
                    for attempt in 1..=LEADER_RETRIES {
                        match client
                            .execute(
                                "INSERT INTO _bench (id, ts, name) VALUES ($1, $2, $3)",
                                params!(entity.id, entity.ts, entity.name.clone()),
                            )
                            .await
                        {
                            Ok(rows) => {
                                rows_affected = rows;
                                break;
                            }
                            Err(err) if attempt < LEADER_RETRIES && is_leader_change(&err) => {
                                retries.fetch_add(1, Ordering::Relaxed);
                                time::sleep(LEADER_RETRY_DELAY).await;
                            }
                            Err(err) => panic!("{err}"),
                        }
                    }
                    assert_eq!(rows_affected, 1);
                }
            })
        };

        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    Ok((
        start.elapsed().as_millis(),
        retries.load(Ordering::Relaxed),
    ))
}

async fn select_timings(client: Client, remote: bool) -> Result<(), Error> {
    let select = "SELECT * FROM _bench WHERE id = $1";

    let start = Instant::now();
    let _: Entity = client.query_map_one(select, params!(1)).await?;
    log(format!("SELECT a single row with a fresh connection and no cached prepared query using `query_map_one` took:\n{} micros", start.elapsed().as_micros()));

    log("SELECT a single row with a cached prepared statement using `query_map_one`:");
    for _ in 0..10 {
        let start = Instant::now();
        let _: Entity = client.query_map_one(select, params!(1)).await?;
        println!("{} micros", start.elapsed().as_micros());
    }

    if remote {
        log("Skipping `query_as_one` as it does only work with local clients");
    } else {
        log("SELECT a single row with a cached prepared statement using `query_as_one`:");
        for _ in 0..10 {
            let start = Instant::now();
            let _: Entity = client.query_as_one(select, params!(1)).await?;
            println!("{} micros", start.elapsed().as_micros());
        }
    }

    let start = Instant::now();
    let _: Vec<Entity> = client.query_map("SELECT * FROM _bench", params!()).await?;
    log(format!(
        "SELECT all rows using `query_map` took:\n{} micros",
        start.elapsed().as_micros()
    ));

    if remote {
        log("Skipping `query_as` as it does only work with local clients");
    } else {
        let start = Instant::now();
        let _: Vec<Entity> = client.query_as("SELECT * FROM _bench", params!()).await?;
        log(format!(
            "SELECT all rows using `query_as` took:\n{} micros",
            start.elapsed().as_micros()
        ));
    }

    Ok(())
}

async fn put_cache(
    client: Client,
    options: &Options,
    sets: Vec<Vec<Entity>>,
) -> Result<(u128, usize), Error> {
    let concurrency = options.concurrency;

    let mut handles = Vec::with_capacity(concurrency);
    let start = Instant::now();
    let retries = Arc::new(AtomicUsize::new(0));
    for set in sets {
        let client = client.clone();
        let retries = retries.clone();

        let handle = task::spawn(async move {
            for entity in set {
                for attempt in 1..=LEADER_RETRIES {
                    match client
                        .put(Cache::One, entity.name.clone(), &entity, Some(30))
                        .await
                    {
                        Ok(()) => break,
                        Err(err) if attempt < LEADER_RETRIES && is_leader_change(&err) => {
                            retries.fetch_add(1, Ordering::Relaxed);
                            time::sleep(LEADER_RETRY_DELAY).await;
                        }
                        Err(err) => panic!("{err}"),
                    }
                }
            }
        });

        handles.push(handle);
    }
    for handle in handles {
        handle.await?;
    }

    Ok((
        start.elapsed().as_millis(),
        retries.load(Ordering::Relaxed),
    ))
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

// Only used by the commented-out transactional insert block above.
async fn cleanup(client: &Client) -> Result<(), Error> {
    client.execute("DELETE FROM _bench", params!()).await?;
    Ok(())
}

fn prepare_data(options: &Options) -> Vec<Vec<Entity>> {
    let concurrency = options.concurrency;
    let rows = options.rows;

    let mut sets = Vec::with_capacity(concurrency);
    let mut idx = 0;
    for _ in 1..=concurrency {
        let set_size = rows / concurrency;
        let mut data = Vec::with_capacity(set_size);
        for _ in 0..set_size {
            // let mut id = Uuid::now_v7().as_bytes().to_vec();
            // id.push(i as u8);
            data.push(Entity {
                id: idx,
                ts: Utc::now().timestamp(),
                name: format!("Name {idx}"),
            });
            idx += 1;
        }
        sets.push(data);
    }

    sets
}
