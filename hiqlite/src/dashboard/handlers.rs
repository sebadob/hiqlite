use crate::dashboard::session::{Session, INSECURE_COOKIES};
use crate::dashboard::table::Table;
use crate::dashboard::{query, session};
use crate::network::AppStateExt;
use crate::query::rows::RowOwned;
use crate::{Error, Node};
use axum::body::Body;
use axum::extract::Path;
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, Method};
use axum::response::Response;
use axum::{body, Form, Json};
use hyper::StatusCode;
use openraft::RaftMetrics;
use serde::Deserialize;
use spow::pow::Pow;

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
    password: String,
    // TODO not sure yet how to proceed with the `pow`. This can only be calculated on the frontend
    // when running via TLS it seems -> investigate further
    pow: String,
}

#[tracing::instrument(skip_all)]
pub async fn post_session(
    state: AppStateExt,
    headers: HeaderMap,
    Form(login): Form<LoginRequest>,
) -> Result<Response, Error> {
    // TODO currently, svelte 5 preview produces an error when loading the WASM in production.
    // Request PoW again when this is resolved in the future -> check
    // Pow::validate(&login.pow).map_err(|err| Error::Unauthorized(err.to_string().into()))?;
    session::set_session_verify(&state, Method::POST, &headers, login.password).await
}

#[tracing::instrument(skip_all)]
pub async fn get_pow() -> Result<String, Error> {
    let difficulty = if *INSECURE_COOKIES { 10 } else { 20 };
    let pow =
        Pow::with_difficulty(difficulty, 5).map_err(|err| Error::Config(err.to_string().into()))?;
    Ok(pow.to_string())
}

pub async fn get_tables(state: AppStateExt, _: Session) -> Result<Json<Vec<Table>>, Error> {
    let tables = Table::find_all(&state).await?;
    Ok(Json(tables))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TableFilterRequest {
    Table,
    Index,
    View,
    Trigger,
}

impl TableFilterRequest {
    pub fn as_str(&self) -> &str {
        match self {
            TableFilterRequest::Table => "table",
            TableFilterRequest::Index => "index",
            TableFilterRequest::View => "view",
            TableFilterRequest::Trigger => "trigger",
        }
    }
}

pub async fn get_tables_filtered(
    state: AppStateExt,
    _: Session,
    Path(filter): Path<TableFilterRequest>,
) -> Result<Json<Vec<Table>>, Error> {
    let tables = Table::find_all_filtered(&state, filter).await?;
    Ok(Json(tables))
}

#[tracing::instrument(skip_all)]
pub async fn post_query(
    state: AppStateExt,
    _: Session,
    body: body::Bytes,
) -> Result<Json<Vec<RowOwned>>, Error> {
    let binding = String::from_utf8_lossy(body.as_ref());
    let sql = binding.trim().to_string();
    let res = query::dashboard_query_dynamic(state, sql).await?;
    Ok(Json(res))
}

pub async fn get_metrics(state: AppStateExt, _: Session) -> Json<RaftMetrics<u64, Node>> {
    let metrics = state.raft_db.raft.metrics().borrow().clone();
    Json(metrics)
}
