use crate::dashboard::session::Session;
use crate::dashboard::table::Table;
use crate::dashboard::{query, session};
use crate::network::AppStateExt;
use crate::query::rows::RowOwned;
use crate::Error;
use axum::body::Body;
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, Method};
use axum::response::Response;
use axum::{body, Form, Json};
use hyper::StatusCode;
use serde::Deserialize;

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
    let res = query::dashboard_query_dynamic(state, sql).await?;
    Ok(Json(res))
}
