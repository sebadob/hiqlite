//! Streaming queries out of a real HTTP API.
//!
//! This example starts a single Hiqlite node with 2.000 seeded users, and then it starts an axum
//! API on `127.0.0.1:8080` whose endpoints answer with the rows of a streaming query.
//! Point `curl -N` at an endpoint and watch the lines arrive while the query is still running, or
//! open the page on `/` and watch a browser fill a table row by row.
//! Check the `README.md` for the endpoints and the `curl` commands.

use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use hiqlite::macros::{embed::*, params, sql, sql_map, FromRow};
use hiqlite::{Client, Error, JsonLinesExt, NodeConfig, StreamExt};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::time::Duration;
use tokio::fs;
use tokio::net::TcpListener;
use tokio::time;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// The migrations of this example, applied with `client.migrate()` below.
///
/// The `sql!()` and `sql_map!()` macros in this example check every query against exactly this
/// directory while the crate compiles, so a query that names a table or a column that is not in
/// these files, and a query whose columns are not the fields of the type it maps, never reaches the
/// runtime.
#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

/// A row of the `users` table, mapped by the `FromRow` derive.
///
/// `Serialize` is what makes a stream of these rows an API response: the streaming endpoints below
/// write one JSON document per row, and each row is one serialized `User`.
#[derive(Debug, Serialize, Deserialize, FromRow)]
struct User {
    id: i64,
    email: String,
    name: Option<String>,
    role: String,
    logins: i64,
}

/// The shape the `/users/mails` endpoint answers with: built from a `User` in flight, on the way
/// out of the stream, one row at a time.
#[derive(Serialize, FromRow)]
struct Mail {
    id: i64,
    email: String,
}

/// The users this example seeds, and the rows a slow endpoint streams.
const USERS: i64 = 2_000;
const SLOW_ROWS: usize = 40;

/// The pause a slow endpoint puts between two rows: big enough to see one line after the other in
/// `curl -N`, small enough that `/users/slow` still ends in ten seconds.
const SLEEP: Duration = Duration::from_millis(250);

#[tokio::main]
async fn main() -> Result<(), Error> {
    // make sure we always start clean
    let _ = fs::remove_dir_all("./data").await;

    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::from("info"))
        .init();

    let mut config = NodeConfig::from_toml("../../hiqlite.toml", None, None, None).await?;
    // The dev config of the repo logs every statement with its params; the seeding of this example
    // is a single statement over 2.000 rows, and its log would drown the API.
    config.log_statements = false;
    let client = hiqlite::start_node(config).await?;

    log(format!("Applying migrations and seeding {USERS} users"));
    client.migrate::<Migrations>().await?;
    client.execute(sql!("DELETE FROM users"), params!()).await?;
    seed(&client).await?;

    // The header of a result set is on every row of a raw stream, shared through the `Arc` of the
    // row, so reading it allocates nothing.
    let mut stream = client
        .query_raw_stream("SELECT * FROM users LIMIT 1", params!())
        .await?;
    if let Some(row) = stream.next().await? {
        log(format!("The header of the result set: {:?}", row.columns()));
    }
    // Dropped right here on purpose: a stream you hold on to without consuming it parks the thread
    // that runs its query, and with it one of the node's read pool connections, for as long as you
    // hold it.  This stream has served its purpose with its first row.
    drop(stream);

    let app = Router::new()
        .route("/", get(page))
        .route("/users", get(users))
        .route("/users/slow", get(users_slow))
        .route("/users/raw", get(users_raw))
        .route("/users/custom", get(users_custom))
        .route("/users/mails", get(users_mails))
        .with_state(client.clone());

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    log("The API answers on http://127.0.0.1:8080 - the page on / reads it in the browser");
    println!("    open http://127.0.0.1:8080/ in a browser");
    println!("    curl -N http://127.0.0.1:8080/users/slow");
    println!("    curl -N http://127.0.0.1:8080/users/raw");
    println!("    curl -i  http://127.0.0.1:8080/users/custom");
    println!("    time curl -s http://127.0.0.1:8080/users | wc -l");
    println!("    curl -s http://127.0.0.1:8080/users/mails | head");
    log("Exit with CTRL + C, which shuts the API and the node down gracefully");

    // Ctrl-C unblocks the shutdown handle: it shuts the Raft node down and the API stops with it.
    let mut shutdown_handle = client.shutdown_handle()?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_handle.wait().await;
        })
        .await?;

    Ok(())
}

/// Every user, streamed out of the query as one JSON object per line, with nothing collected: this
/// endpoint answers a query over every row of the table and its memory stays a single row.
async fn users(State(client): State<Client>) -> Result<Response, Error> {
    let stream = client
        .query_map_stream::<User, _>(sql_map!(User, "SELECT * FROM users ORDER BY id"), params!())
        .await?;

    Ok(stream.into_json_response())
}

/// The very same query with a conversion, a limit and a pause per row applied in flight.
///
/// The conversion and the `take` are `StreamExt` combinators, and a combinator chain is not a
/// `QueryStream` anymore, so it answers with `into_json_response` from `JsonLinesExt`.
/// Every row is logged the moment it goes out, so the server side shows the streaming as well.
async fn users_slow(State(client): State<Client>) -> Result<Response, Error> {
    let stream = client
        .query_map_stream::<User, _>(sql_map!(User, "SELECT * FROM users ORDER BY id"), params!())
        .await?
        .map(|res| {
            res.map(|mut user| {
                user.email = user.email.to_uppercase();
                user
            })
        })
        .take(SLOW_ROWS)
        .inspect(|res| info!("sent row: {:?}", res.as_ref().map(|user| user.id)))
        .then(|res| async move {
            time::sleep(SLEEP).await;
            res
        });

    Ok(stream.into_json_response())
}

/// The bare rows of `query_raw_stream`, one JSON object per line, keyed by the column names of the
/// query: a `RowOwned` serializes itself with exactly its own header.  The rows of this endpoint
/// are paused one by one as well, for `curl -N`.
async fn users_raw(State(client): State<Client>) -> Result<Response, Error> {
    let stream = client
        .query_raw_stream(
            "SELECT id, email, role FROM users ORDER BY id LIMIT 20",
            params!(),
        )
        .await?;

    let stream = stream.then(|res| async move {
        time::sleep(SLEEP).await;
        res
    });

    Ok(stream.into_json_response())
}

/// The response of a streaming query built by hand: the very same streaming JSON body, wrapped
/// into a status code and headers of your own.
///
/// `into_json_body` is the body of `into_json_response` on its own, on a `QueryStream` and on a
/// combinator chain over it alike.
async fn users_custom(State(client): State<Client>) -> Result<Response, Error> {
    let stream = client
        .query_map_stream::<User, _>(
            sql_map!(User, "SELECT * FROM users ORDER BY id LIMIT 50"),
            params!(),
        )
        .await?;

    let response = Response::builder()
        // a status of your own, and any header you want, around the streaming rows
        .status(StatusCode::PARTIAL_CONTENT)
        .header(CONTENT_TYPE, "application/x-ndjson")
        .header("x-query-limit", "50")
        .body(stream.into_json_body())
        .map_err(|err| Error::Config(err.to_string().into()))?;

    Ok(response)
}

/// Rows mapped into another type in flight: the stream carries `User` rows, the response answers
/// with `Mail` objects, and only the rows the query produces are ever built.
async fn users_mails(State(client): State<Client>) -> Result<Response, Error> {
    let stream = client
        .query_map_stream::<User, _>(
            sql_map!(User, "SELECT * FROM users WHERE role = 'admin' ORDER BY id"),
            params!(),
        )
        .await?
        .map(|res| {
            res.map(|user| Mail {
                id: user.id,
                email: user.email,
            })
        });

    Ok(stream.into_json_response())
}

/// The 2.000 users of this example, seeded with ten multi-row `INSERT`s, which is ten Raft log
/// entries.  This SQL is built at runtime, so no macro can check it; every query that reads it
/// below is checked at compile time.
async fn seed(client: &Client) -> Result<(), Error> {
    let rows_per_stmt = (USERS / 10) as usize;
    let mut sql = String::with_capacity(64 * USERS as usize);
    for start in (1..=USERS).step_by(rows_per_stmt) {
        let end = (start + rows_per_stmt as i64 - 1).min(USERS);
        sql.push_str("INSERT INTO users (id, email, name, role, logins) VALUES ");
        for id in start..=end {
            // Every 7. user has no name, to show a JSON `null`, and every 5. is an admin.
            let name = if id % 7 == 0 {
                String::from("NULL")
            } else {
                format!("'User {id}'")
            };
            let role = if id % 5 == 0 { "admin" } else { "member" };
            sql.push_str(&format!(
                "({id}, 'user{id}@example.com', {name}, '{role}', {}),",
                id % 40
            ));
        }
        // drop the trailing comma of the VALUES list
        sql.truncate(sql.len() - 1);
        sql.push_str(";\n");
    }

    for rows_affected in client.batch(sql).await? {
        rows_affected?;
    }
    Ok(())
}

/// The page on `/`: the same endpoints, read in the browser.
///
/// It fetches a streaming response with `fetch` and `response.body.getReader()`, and it appends a
/// table row for every JSON line that arrives, so the streaming is visible line by line.  Plain
/// HTML and plain JavaScript in a single file, served straight from the source.
async fn page() -> Html<&'static str> {
    Html(include_str!("../static/stream.html"))
}

// this way of logging makes our logs easier to see with all the raft logging enabled
fn log<S: Display>(s: S) {
    println!("\n\n>>> {s}\n");
}
