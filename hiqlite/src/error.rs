use crate::network::{RaftInitError, RaftSnapshotError, RaftWriteError};
use crate::Node;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use bincode::ErrorKind;
use fastwebsockets::WebSocketError;
use openraft::error::{CheckIsLeaderError, ClientWriteError, Fatal, RaftError};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use thiserror::Error;
use tokio::task::JoinError;
use tracing::trace;

#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::NotifyRequest;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum Error {
    #[error("BadRequest: {0}")]
    BadRequest(Cow<'static, str>),
    /// Serialization / Deserialization errors from `bincode`
    #[error("Bincode: {0}")]
    Bincode(String),
    #[error("Cache: {0}")]
    Cache(Cow<'static, str>),
    /// Internal Channel errors from `flume`
    #[error("Channel: {0}")]
    Channel(String),
    /// Internal error when a leader-request is sent to a non-leader node
    #[error("CheckIsLeaderError: {0}")]
    CheckIsLeaderError(RaftError<u64, CheckIsLeaderError<u64, Node>>),
    #[error("ClientWriteError: {0}")]
    ClientWriteError(RaftWriteError),
    #[error("Config: {0}")]
    Config(Cow<'static, str>),
    #[error("Connect: {0}")]
    Connect(String),
    /// Sqlite constraint violation
    #[error("Connect: {0}")]
    ConstraintViolation(String),
    #[cfg(any(feature = "dashboard", feature = "s3"))]
    #[error("Cryptr: {0}")]
    Cryptr(String),
    #[error("Error: {0}")]
    Error(Cow<'static, str>),
    #[error("InitializeError: {0}")]
    InitializeError(RaftInitError),
    /// Error informing about a Raft leader change
    #[error("LeaderChange: {0}")]
    LeaderChange(Cow<'static, str>),
    /// Error when the given query parameters could not be bound properly to the prepared statement.
    #[error("QueryParams: {0}")]
    QueryParams(Cow<'static, str>),
    /// Error returned when a query did not return any rows.
    #[error("QueryReturnedNoRows: {0}")]
    QueryReturnedNoRows(Cow<'static, str>),
    /// Error if the prepared statement cannot be built properly.
    #[error("PrepareStatement: {0}")]
    PrepareStatement(Cow<'static, str>),
    /// Internal Raft error
    #[error("RaftError: {0}")]
    RaftError(RaftError<u64>),
    #[error("RaftErrorFatal: {0}")]
    /// Internal Raft error
    RaftErrorFatal(Fatal<u64>),
    #[error("Request: {0}")]
    Request(String),
    #[cfg(feature = "s3")]
    #[error("S3: {0}")]
    S3(String),
    #[error("SnapshotError: {0}")]
    SnapshotError(RaftSnapshotError),
    /// All kinds of SQLite database errors, mostly just a wrapper for the `rusqlite` error apart
    /// from `QueryReturnedNoRows`.
    #[cfg(feature = "sqlite")]
    #[error("Sqlite: {0}")]
    Sqlite(Cow<'static, str>),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Token: {0}")]
    Token(Cow<'static, str>),
    #[error("Transaction: {0}")]
    Transaction(Cow<'static, str>),
    #[error("Unauthorized: {0}")]
    Unauthorized(Cow<'static, str>),
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
            Error::Channel(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::CheckIsLeaderError(_) => StatusCode::CONFLICT,
            Error::ConstraintViolation(_) => StatusCode::BAD_REQUEST,
            #[cfg(any(feature = "dashboard", feature = "s3"))]
            Error::Cryptr(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::LeaderChange(_) => StatusCode::CONFLICT,
            Error::QueryParams(_) => StatusCode::BAD_REQUEST,
            Error::QueryReturnedNoRows(_) => StatusCode::NOT_FOUND,
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
            #[cfg(feature = "s3")]
            Error::S3(_) => StatusCode::BAD_REQUEST,
            Error::SnapshotError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "sqlite")]
            Error::Sqlite(_) => StatusCode::BAD_REQUEST,
            Error::Timeout(_) => StatusCode::REQUEST_TIMEOUT,
            Error::Token(_) => StatusCode::UNAUTHORIZED,
            Error::Transaction(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Unauthorized(_) => StatusCode::UNAUTHORIZED,
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
        trace!("bincode::ErrorKind: {}", value);
        Self::Bincode(value.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        trace!("reqwest::Error: {}", value);
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
        trace!("ClientWriteError: {}", value);
        Self::ClientWriteError(value)
    }
}

impl From<RaftInitError> for Error {
    fn from(value: RaftInitError) -> Self {
        trace!("InitializeError: {}", value);
        Self::InitializeError(value)
    }
}

impl From<RaftSnapshotError> for Error {
    fn from(value: RaftSnapshotError) -> Self {
        trace!("SnapshotError: {}", value);
        Self::SnapshotError(value)
    }
}

impl From<RaftError<u64>> for Error {
    fn from(value: RaftError<u64>) -> Self {
        trace!("RaftError: {}", value);
        Self::RaftError(value)
    }
}

impl From<RaftError<u64, CheckIsLeaderError<u64, Node>>> for Error {
    fn from(value: RaftError<u64, CheckIsLeaderError<u64, Node>>) -> Self {
        trace!("CheckIsLeaderError: {}", value);
        Self::CheckIsLeaderError(value)
    }
}

impl From<Fatal<u64>> for Error {
    fn from(value: Fatal<u64>) -> Self {
        trace!("RaftErrorFatal: {}", value);
        Self::RaftErrorFatal(value)
    }
}

#[cfg(feature = "sqlite")]
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        trace!("rusqlite::Error: {}", value);

        match value {
            rusqlite::Error::QueryReturnedNoRows => {
                Self::QueryReturnedNoRows("no rows returned".into())
            }
            rusqlite::Error::SqliteFailure(err, ext) => match err.code {
                // ErrorCode::InternalMalfunction => {}
                // ErrorCode::PermissionDenied => {}
                // ErrorCode::OperationAborted => {}
                // ErrorCode::DatabaseBusy => {}
                // ErrorCode::DatabaseLocked => {}
                // ErrorCode::OutOfMemory => {}
                // ErrorCode::ReadOnly => {}
                // ErrorCode::OperationInterrupted => {}
                // ErrorCode::SystemIoFailure => {}
                // ErrorCode::DatabaseCorrupt => {}
                // ErrorCode::NotFound => {}
                // ErrorCode::DiskFull => {}
                // ErrorCode::CannotOpen => {}
                // ErrorCode::FileLockingProtocolFailed => {}
                // ErrorCode::SchemaChanged => {}
                // ErrorCode::TooBig => {}
                rusqlite::ErrorCode::ConstraintViolation => {
                    Self::ConstraintViolation(format!("{} {:?}", err, ext))
                }
                // ErrorCode::TypeMismatch => {}
                // ErrorCode::ApiMisuse => {}
                // ErrorCode::NoLargeFileSupport => {}
                // ErrorCode::AuthorizationForStatementDenied => {}
                // ErrorCode::ParameterOutOfRange => {}
                // ErrorCode::NotADatabase => {}
                // ErrorCode::Unknown => {}
                _ => Self::Sqlite(format!("{} {:?}", err, ext).into()),
            },
            v => Self::Sqlite(v.to_string().into()),
        }
    }
}

// impl From<deadpool::managed::BuildError> for Error {
//     fn from(value: BuildError) -> Self {
//         trace!("Sqlite: {}", value);
//         Self::Sqlite(value.to_string().into())
//     }
// }

// impl From<deadpool::managed::PoolError<rusqlite::Error>> for Error {
//     fn from(value: PoolError<rusqlite::Error>) -> Self {
//         trace!("Sqlite: {}", value);
//         Self::Sqlite(value.to_string().into())
//     }
// }

#[cfg(feature = "sqlite")]
impl From<deadpool::unmanaged::PoolError> for Error {
    fn from(value: deadpool::unmanaged::PoolError) -> Self {
        trace!("Sqlite: {}", value);
        Self::Sqlite(value.to_string().into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        trace!("BadRequest: {}", value);
        Self::BadRequest(value.to_string().into())
    }
}

impl From<fastwebsockets::WebSocketError> for Error {
    fn from(value: WebSocketError) -> Self {
        trace!("WebSocket: {}", value);
        Self::WebSocket(value.to_string())
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        trace!("JoinError: {}", value);
        Self::Error(value.to_string().into())
    }
}

impl From<flume::RecvError> for Error {
    fn from(value: flume::RecvError) -> Self {
        trace!("flume::RecvError: {}", value);
        Self::Channel(value.to_string())
    }
}

#[cfg(feature = "listen_notify")]
impl From<flume::SendError<NotifyRequest>> for Error {
    fn from(value: flume::SendError<NotifyRequest>) -> Self {
        trace!("flume::SendError<NotifyRequest>: {}", value);
        Self::Channel(value.to_string())
    }
}

#[cfg(any(feature = "backup", feature = "s3"))]
impl From<cryptr::stream::s3::S3Error> for Error {
    fn from(value: cryptr::stream::s3::S3Error) -> Self {
        trace!("cryptr::stream::s3::S3Error: {}", value);
        Self::S3(value.to_string())
    }
}

#[cfg(any(feature = "dashboard", feature = "s3"))]
impl From<cryptr::CryptrError> for Error {
    fn from(value: cryptr::CryptrError) -> Self {
        trace!("cryptr::CryptrError: {}", value);
        Self::Cryptr(value.to_string())
    }
}

#[cfg(feature = "dashboard")]
impl From<argon2::password_hash::Error> for Error {
    fn from(value: argon2::password_hash::Error) -> Self {
        trace!("argon2::password_hash::Error: {}", value);
        Self::Unauthorized("invalid credentials".into())
    }
}
