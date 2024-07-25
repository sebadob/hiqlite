use crate::network::{RaftInitError, RaftSnapshotError, RaftWriteError};
use crate::Node;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use bincode::ErrorKind;
use deadpool::managed::{BuildError, PoolError};
use fastwebsockets::WebSocketError;
use openraft::error::{CheckIsLeaderError, ClientWriteError, Fatal, RaftError};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use thiserror::Error;
use tokio::task::JoinError;
use tracing::error;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum Error {
    #[error("BadRequest: {0}")]
    BadRequest(Cow<'static, str>),
    #[error("Bincode: {0}")]
    Bincode(String),
    #[error("Cache: {0}")]
    Cache(Cow<'static, str>),
    #[error("CheckIsLeaderError: {0}")]
    CheckIsLeaderError(RaftError<u64, CheckIsLeaderError<u64, Node>>),
    #[error("ClientWriteError: {0}")]
    ClientWriteError(RaftWriteError),
    #[error("Config: {0}")]
    Config(Cow<'static, str>),
    #[error("Connect: {0}")]
    Connect(String),
    #[error("Error: {0}")]
    Error(Cow<'static, str>),
    #[error("InitializeError: {0}")]
    InitializeError(RaftInitError),
    #[error("LeaderChange: {0}")]
    LeaderChange(Cow<'static, str>),
    #[error("QueryParams: {0}")]
    QueryParams(Cow<'static, str>),
    #[error("PrepareStatement: {0}")]
    PrepareStatement(Cow<'static, str>),
    #[error("RaftError: {0}")]
    RaftError(RaftError<u64>),
    #[error("RaftErrorFatal: {0}")]
    RaftErrorFatal(Fatal<u64>),
    #[error("Request: {0}")]
    Request(String),
    #[error("S3: {0}")]
    S3(String),
    #[error("SnapshotError: {0}")]
    SnapshotError(RaftSnapshotError),
    #[error("Sqlite: {0}")]
    Sqlite(Cow<'static, str>),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Token: {0}")]
    Token(Cow<'static, str>),
    #[error("Transaction: {0}")]
    Transaction(Cow<'static, str>),
    #[error("WebSocket: {0}")]
    WebSocket(String),
}

impl Error {
    pub fn new<E: Into<Cow<'static, str>>>(error: E) -> Self {
        Self::Error(error.into())
    }

    /// Checks if the inner wrapped error is a `ForwardToLeader` error
    pub fn is_forward_to_leader(&self) -> Option<(Option<u64>, &Option<Node>)> {
        if let Self::ClientWriteError(RaftWriteError::APIError(
            ClientWriteError::ForwardToLeader(err),
        )) = self
        {
            return Some((err.leader_id, &err.leader_node));
        }

        if let Self::CheckIsLeaderError(RaftError::APIError(CheckIsLeaderError::ForwardToLeader(
            err,
        ))) = self
        {
            return Some((err.leader_id, &err.leader_node));
        }

        None
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Error::BadRequest(_) => StatusCode::BAD_REQUEST,
            Error::Bincode(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Cache(_) => StatusCode::BAD_REQUEST,
            Error::CheckIsLeaderError(_) => StatusCode::CONFLICT,
            Error::LeaderChange(_) => StatusCode::CONFLICT,
            Error::QueryParams(_) => StatusCode::BAD_REQUEST,
            Error::PrepareStatement(_) => StatusCode::BAD_REQUEST,
            Error::ClientWriteError(_) => {
                if self.is_forward_to_leader().is_some() {
                    StatusCode::PERMANENT_REDIRECT
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
            Error::Config(_) => StatusCode::BAD_REQUEST,
            Error::Connect(_) => StatusCode::SERVICE_UNAVAILABLE,
            Error::Error(_) => StatusCode::BAD_REQUEST,
            Error::InitializeError(_) => StatusCode::BAD_REQUEST,
            Error::RaftError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::RaftErrorFatal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Request(_) => StatusCode::BAD_REQUEST,
            Error::S3(_) => StatusCode::BAD_REQUEST,
            Error::SnapshotError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Sqlite(_) => StatusCode::BAD_REQUEST,
            Error::Timeout(_) => StatusCode::REQUEST_TIMEOUT,
            Error::Token(_) => StatusCode::UNAUTHORIZED,
            Error::Transaction(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::WebSocket(_) => StatusCode::BAD_REQUEST,
        };

        (status, Json(self)).into_response()
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Error(value.to_string().into())
    }
}

impl From<Box<bincode::ErrorKind>> for Error {
    fn from(value: Box<ErrorKind>) -> Self {
        error!("\n\nbincode::ErrorKind: {}\n", value);
        Self::Bincode(value.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        error!("reqwest::Error: {}", value);
        if value.is_connect() {
            Self::Connect(value.to_string())
        } else if value.is_timeout() {
            Self::Timeout(value.to_string())
        } else {
            Self::Request(value.to_string())
        }
    }
}

impl From<RaftWriteError> for Error {
    fn from(value: RaftWriteError) -> Self {
        error!("ClientWriteError: {}", value);
        Self::ClientWriteError(value)
    }
}

impl From<RaftInitError> for Error {
    fn from(value: RaftInitError) -> Self {
        error!("InitializeError: {}", value);
        Self::InitializeError(value)
    }
}

impl From<RaftSnapshotError> for Error {
    fn from(value: RaftSnapshotError) -> Self {
        error!("SnapshotError: {}", value);
        Self::SnapshotError(value)
    }
}

impl From<RaftError<u64>> for Error {
    fn from(value: RaftError<u64>) -> Self {
        error!("RaftError: {}", value);
        Self::RaftError(value)
    }
}

impl From<RaftError<u64, CheckIsLeaderError<u64, Node>>> for Error {
    fn from(value: RaftError<u64, CheckIsLeaderError<u64, Node>>) -> Self {
        error!("CheckIsLeaderError: {}", value);
        Self::CheckIsLeaderError(value)
    }
}

impl From<Fatal<u64>> for Error {
    fn from(value: Fatal<u64>) -> Self {
        error!("RaftErrorFatal: {}", value);
        Self::RaftErrorFatal(value)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        error!("Sqlite: {}", value);
        Self::Sqlite(value.to_string().into())
    }
}

impl From<deadpool::managed::BuildError> for Error {
    fn from(value: BuildError) -> Self {
        error!("Sqlite: {}", value);
        Self::Sqlite(value.to_string().into())
    }
}

impl From<deadpool::managed::PoolError<rusqlite::Error>> for Error {
    fn from(value: PoolError<rusqlite::Error>) -> Self {
        error!("Sqlite: {}", value);
        Self::Sqlite(value.to_string().into())
    }
}

impl From<deadpool::unmanaged::PoolError> for Error {
    fn from(value: deadpool::unmanaged::PoolError) -> Self {
        error!("Sqlite: {}", value);
        Self::Sqlite(value.to_string().into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        error!("BadRequest: {}", value);
        Self::BadRequest(value.to_string().into())
    }
}

impl From<fastwebsockets::WebSocketError> for Error {
    fn from(value: WebSocketError) -> Self {
        error!("WebSocket: {}", value);
        Self::WebSocket(value.to_string())
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        error!("JoinError: {}", value);
        Self::Error(value.to_string().into())
    }
}
