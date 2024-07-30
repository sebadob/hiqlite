use crate::dashboard::session;
use crate::dashboard::session::Session;
use crate::dashboard::table::Table;
use crate::network::AppStateExt;
use crate::query::rows::{ColumnOwned, RowOwned};
use crate::Error;
use axum::body::Body;
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, Method};
use axum::response::Response;
use axum::{body, Form, Json};
use hyper::StatusCode;
use serde::Deserialize;
use tokio::task;
use tracing::info;

pub async fn redirect_to_index() -> Response {
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(LOCATION, "/dashboard/index.html")
        .body(Body::empty())
        .unwrap()
}

pub async fn get_session(s: Session) -> Result<Json<Session>, Error> {
    Ok(Json(s))
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

pub async fn post_session(
    state: AppStateExt,
    headers: HeaderMap,
    Form(login): Form<LoginRequest>,
) -> Result<Response, Error> {
    session::set_session_verify(&state, Method::POST, &headers, login).await
}

pub async fn get_tables(state: AppStateExt, _: Session) -> Result<Json<Vec<Table>>, Error> {
    let tables = Table::find_all(&state).await?;
    Ok(Json(tables))
}

pub(crate) async fn post_query(
    state: AppStateExt,
    _: Session,
    body: body::Bytes,
) -> Result<Json<Vec<RowOwned>>, Error> {
    let binding = String::from_utf8_lossy(body.as_ref());
    let sql = binding.trim().to_string();

    if sql.len() < 8 {
        return Err(Error::Sqlite("invalid query".into()));
    }

    if state.raft_db.log_statements {
        info!("dashboard query:\n{}", sql)
    }

    // we need to check if we can do a local select query or if it is
    // modifying and needs to go through the raft
    let sql_start = sql[..7].to_lowercase();
    let is_select = sql_start.starts_with("select") || sql_start.starts_with("explain");

    let res = if is_select {
        let conn = state.raft_db.read_pool.get().await?;

        task::spawn_blocking(move || {
            let mut stmt = conn.prepare(&sql)?;

            let columns = ColumnOwned::mapping_cols_from_stmt(stmt.columns())?;

            let mut rows = stmt.raw_query();
            let mut rows_owned = Vec::new();
            while let Ok(Some(row)) = rows.next() {
                rows_owned.push(RowOwned::from_row_column(row, &columns));
            }

            Ok::<Vec<RowOwned>, Error>(rows_owned)
        })
        .await??
    } else {
        todo!()

        // let rows_affected = stmt.raw_execute()?;
        // let affected = if rows_affected > i64::MAX as usize {
        //     i64::MAX
        // } else {
        //     rows_affected as i64
        // };
        // vec![RowOwned {
        //     columns: vec![ColumnOwned {
        //         name: "rows_affected".to_string(),
        //         value: ValueOwned::Integer(affected),
        //     }],
        // }]
    };

    Ok(Json(res))
}
