use crate::app_state::AppState;
use crate::Error;
use axum::http::header::{ACCEPT, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::{body, Json};
use openraft::error::{ClientWriteError, InitializeError, InstallSnapshotError, RaftError};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;

// pub use management::LearnerReq;
pub use raft_client::NetworkStreaming;

pub(crate) mod api;
mod challenge_response;
pub(crate) mod handshake;
pub(crate) mod management;
mod raft_client;
pub(crate) mod raft_server;

pub(crate) type AppStateExt = axum::extract::State<Arc<AppState>>;
// pub(crate) type RaftWriteResponse = ClientWriteResponse<TypeConfigSqlite>;
pub(crate) type RaftInitError = RaftError<u64, InitializeError<u64, crate::Node>>;
pub(crate) type RaftSnapshotError = RaftError<u64, InstallSnapshotError>;
pub(crate) type RaftWriteError = RaftError<u64, ClientWriteError<u64, crate::Node>>;

pub static HEADER_NAME_SECRET: &str = "X-API-SECRET";

#[inline(always)]
fn get_payload<T>(headers: &HeaderMap, body: body::Bytes) -> Result<T, Error>
where
    T: for<'a> Deserialize<'a>,
{
    if let Some(typ) = headers.get(CONTENT_TYPE) {
        if typ == HeaderValue::from_static("application/json") {
            return Ok(serde_json::from_slice(body.as_ref())?);
        }
    }
    Ok(bincode::deserialize(body.as_ref())?)
}

#[inline(always)]
fn fmt_ok<S: Debug + Serialize>(headers: HeaderMap, payload: S) -> Result<Response, Error> {
    if let Some(accept) = headers.get(ACCEPT) {
        if accept == HeaderValue::from_static("application/json") {
            return Ok(Json(payload).into_response());
        }
    }
    Ok(bincode::serialize(&payload).unwrap().into_response())
}

#[inline(always)]
fn validate_secret(state: &AppStateExt, headers: &HeaderMap) -> Result<(), Error> {
    match headers.get(HEADER_NAME_SECRET) {
        None => Err(Error::Token("API Secret missing".into())),
        Some(secret) => {
            if state.secret_api.as_bytes() != secret.as_bytes() {
                Err(Error::Token("Invalid API Secret".into()))
            } else {
                Ok(())
            }
        }
    }
}
