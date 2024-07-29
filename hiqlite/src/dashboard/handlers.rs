use crate::dashboard::session;
use crate::dashboard::session::Session;
use crate::network::AppStateExt;
use crate::Error;
use axum::body::Body;
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, Method};
use axum::response::Response;
use axum::Form;
use hyper::StatusCode;
use serde::Deserialize;
use tracing::info;

pub async fn redirect_to_index() -> Response {
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(LOCATION, "/dashboard/index.html")
        .body(Body::empty())
        .unwrap()
}

pub async fn login_check(s: Session) -> Result<(), Error> {
    info!("check login: {:?}", s);
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

pub async fn login(
    state: AppStateExt,
    headers: HeaderMap,
    Form(login): Form<LoginRequest>,
) -> Result<Response, Error> {
    info!("login: {:?}", login);
    session::set_session_verify(&state, Method::POST, &headers, login).await
}
