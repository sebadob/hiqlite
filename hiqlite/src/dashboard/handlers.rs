use crate::dashboard::session;
use crate::dashboard::session::Session;
use crate::network::AppStateExt;
use crate::Error;
use axum::http::{HeaderMap, Method};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;

pub async fn check_login(_: Session) -> Result<(), Error> {
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

pub async fn login(
    state: AppStateExt,
    headers: HeaderMap,
    Json(login): Json<LoginRequest>,
) -> Result<Response, Error> {
    session::set_session_verify(&state, Method::POST, &headers, login).await
}
