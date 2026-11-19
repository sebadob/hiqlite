use crate::Node;
use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use bincode_next::error::{DecodeError, EncodeError};
use bincode_next::{Decode, Encode};
use fastwebsockets::WebSocketError;
use openraft::StorageError;
use openraft::errors::{
    ClientWriteError, Fatal, InitializeError, LinearizableReadError, RaftError, StreamingError,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::convert::Infallible;
use thiserror::Error;
use tokio::task::JoinError;
use tracing::trace;

#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::NotifyRequest;

// openraft 0.10 makes its error types generic over the type config, so every payload type here is
// one instantiation per group. The enum's shape must not depend on the feature set - peers are
// built with different ones - so every variant exists in every build that has a Raft group, and a
// group that isn't compiled in resolves to the wire instantiation. That is the one the network
// layer pins its shared payloads to, and it is layout-identical to both groups because no openraft
// error type has a field built from `D` or `R` - only from `NodeId`, `Node` and the macro defaults.
// The alias guards in `network` keep that from drifting.
#[cfg(feature = "sqlite")]
type ConfigDb = crate::store::state_machine::sqlite::TypeConfigSqlite;
#[cfg(not(feature = "sqlite"))]
type ConfigDb = crate::network::TypeConfigWire;

#[cfg(feature = "cache")]
type ConfigCache = crate::store::state_machine::memory::TypeConfigCache;
#[cfg(not(feature = "cache"))]
type ConfigCache = crate::network::TypeConfigWire;

// The serde derives stay for the JSON API path and cross-decode tooling; the native derives make
// this type flow through `helpers::serialize` / `helpers::deserialize`. The openraft payload
// fields we do not own are bridged through the byte-identical serde adapter via `with_serde`.
// Peers are built with different feature sets, so the variant set is fixed and a group that isn't
// compiled in keeps its variants with payloads on the crate-internal wire config - which is what
// this lint is about, and that is the deal.
#[allow(private_interfaces)]
#[derive(Debug, Error, Serialize, Deserialize, Encode, Decode)]
pub enum Error {
    #[error("BadRequest: {0}")]
    BadRequest(#[bincode(with_serde)] Cow<'static, str>),
    /// Serialization / Deserialization errors from `bincode-next`
    #[error("Bincode: {0}")]
    Bincode(String),
    #[error("Cache: {0}")]
    Cache(#[bincode(with_serde)] Cow<'static, str>),
    /// Internal Channel errors from `flume`
    #[error("Channel: {0}")]
    Channel(String),
    /// Internal error when a leader-request is sent to a non-leader node
    #[error("CheckIsLeaderErrorDb: {0}")]
    CheckIsLeaderErrorDb(
        #[bincode(with_serde)] Box<RaftError<ConfigDb, LinearizableReadError<ConfigDb>>>,
    ),
    #[error("CheckIsLeaderErrorCache: {0}")]
    CheckIsLeaderErrorCache(
        #[bincode(with_serde)] Box<RaftError<ConfigCache, LinearizableReadError<ConfigCache>>>,
    ),
    #[error("ClientWriteErrorDb: {0}")]
    ClientWriteErrorDb(#[bincode(with_serde)] Box<RaftError<ConfigDb, ClientWriteError<ConfigDb>>>),
    #[error("ClientWriteErrorCache: {0}")]
    ClientWriteErrorCache(
        #[bincode(with_serde)] Box<RaftError<ConfigCache, ClientWriteError<ConfigCache>>>,
    ),
    #[error("Config: {0}")]
    Config(#[bincode(with_serde)] Cow<'static, str>),
    #[error("Connect: {0}")]
    Connect(String),
    /// Sqlite constraint violation
    #[error("Connect: {0}")]
    ConstraintViolation(String),
    #[error("Cryptr: {0}")]
    Cryptr(String),
    #[error("Error: {0}")]
    Error(#[bincode(with_serde)] Cow<'static, str>),
    #[error("InitializeErrorDb: {0}")]
    InitializeErrorDb(#[bincode(with_serde)] Box<RaftError<ConfigDb, InitializeError<ConfigDb>>>),
    #[error("InitializeErrorCache: {0}")]
    InitializeErrorCache(
        #[bincode(with_serde)] Box<RaftError<ConfigCache, InitializeError<ConfigCache>>>,
    ),
    /// Error informing about a Raft leader change
    #[error("LeaderChange: {0}")]
    LeaderChange(#[bincode(with_serde)] Cow<'static, str>),
    /// Error when the given query parameters could not be bound properly to the prepared statement.
    #[error("QueryParams: {0}")]
    QueryParams(#[bincode(with_serde)] Cow<'static, str>),
    /// Error returned when a query did not return any rows.
    #[error("QueryReturnedNoRows: {0}")]
    QueryReturnedNoRows(#[bincode(with_serde)] Cow<'static, str>),
    /// Error if the prepared statement cannot be built properly.
    #[error("PrepareStatement: {0}")]
    PrepareStatement(#[bincode(with_serde)] Cow<'static, str>),
    /// Internal Raft error
    #[error("RaftErrorDb: {0}")]
    RaftErrorDb(#[bincode(with_serde)] Box<RaftError<ConfigDb>>),
    #[error("RaftErrorCache: {0}")]
    RaftErrorCache(#[bincode(with_serde)] Box<RaftError<ConfigCache>>),
    #[error("RaftErrorFatalDb: {0}")]
    RaftErrorFatalDb(#[bincode(with_serde)] Box<Fatal<ConfigDb>>),
    #[error("RaftErrorFatalCache: {0}")]
    RaftErrorFatalCache(#[bincode(with_serde)] Box<Fatal<ConfigCache>>),
    /// Internal Raft error
    #[error("RateLimit: {0}")]
    RateLimit(#[bincode(with_serde)] Cow<'static, str>),
    #[error("Request: {0}")]
    Request(String),
    #[error("S3: {0}")]
    S3(String),
    #[error("SnapshotErrorDb: {0}")]
    SnapshotErrorDb(#[bincode(with_serde)] Box<StreamingError<ConfigDb>>),
    #[error("SnapshotErrorCache: {0}")]
    SnapshotErrorCache(#[bincode(with_serde)] Box<StreamingError<ConfigCache>>),
    /// All kinds of SQLite database errors, mostly just a wrapper for the `rusqlite` error apart
    /// from `QueryReturnedNoRows`.
    #[error("Sqlite: {0}")]
    Sqlite(#[bincode(with_serde)] Cow<'static, str>),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Token: {0}")]
    Token(#[bincode(with_serde)] Cow<'static, str>),
    #[error("Transaction: {0}")]
    Transaction(#[bincode(with_serde)] Cow<'static, str>),
    #[error("Unauthorized: {0}")]
    Unauthorized(#[bincode(with_serde)] Cow<'static, str>),
    #[error("WAL: {0}")]
    WAL(String),
    #[error("WebSocket: {0}")]
    WebSocket(String),
}

impl Error {
    pub fn new<E: Into<Cow<'static, str>>>(error: E) -> Self {
        Self::Error(error.into())
    }

    /// Checks if the inner wrapped error is a `ForwardToLeader` error
    pub fn is_forward_to_leader(&self) -> Option<(Option<u64>, &Option<Node>)> {
        if let Self::ClientWriteErrorDb(err) = self
            && let Some(err) = err.api_error()
            && let ClientWriteError::ForwardToLeader(err) = err
        {
            return Some((err.leader_id, &err.leader_node));
        }
        if let Self::ClientWriteErrorCache(err) = self
            && let Some(err) = err.api_error()
            && let ClientWriteError::ForwardToLeader(err) = err
        {
            return Some((err.leader_id, &err.leader_node));
        }

        if let Self::CheckIsLeaderErrorDb(err) = self
            && let Some(err) = err.forward_to_leader()
        {
            return Some((err.leader_id, &err.leader_node));
        }
        if let Self::CheckIsLeaderErrorCache(err) = self
            && let Some(err) = err.forward_to_leader()
        {
            return Some((err.leader_id, &err.leader_node));
        }

        None
    }

    #[cfg(feature = "toml")]
    pub(crate) fn config<C: Into<Cow<'static, str>>>(err: C) -> Self {
        Self::Config(err.into())
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Error::BadRequest(_) => StatusCode::BAD_REQUEST,
            Error::Bincode(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Cache(_) => StatusCode::BAD_REQUEST,
            Error::Channel(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::CheckIsLeaderErrorDb(_) => StatusCode::CONFLICT,
            Error::CheckIsLeaderErrorCache(_) => StatusCode::CONFLICT,
            Error::ConstraintViolation(_) => StatusCode::BAD_REQUEST,
            Error::Cryptr(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::LeaderChange(_) => StatusCode::CONFLICT,
            Error::QueryParams(_) => StatusCode::BAD_REQUEST,
            Error::QueryReturnedNoRows(_) => StatusCode::NOT_FOUND,
            Error::PrepareStatement(_) => StatusCode::BAD_REQUEST,
            Error::ClientWriteErrorDb(_) => {
                if self.is_forward_to_leader().is_some() {
                    StatusCode::PERMANENT_REDIRECT
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
            Error::ClientWriteErrorCache(_) => {
                if self.is_forward_to_leader().is_some() {
                    StatusCode::PERMANENT_REDIRECT
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
            Error::Config(_) => StatusCode::BAD_REQUEST,
            Error::Connect(_) => StatusCode::SERVICE_UNAVAILABLE,
            Error::Error(_) => StatusCode::BAD_REQUEST,
            Error::InitializeErrorDb(_) => StatusCode::BAD_REQUEST,
            Error::InitializeErrorCache(_) => StatusCode::BAD_REQUEST,
            Error::RaftErrorDb(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::RaftErrorCache(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::RaftErrorFatalDb(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::RaftErrorFatalCache(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::RateLimit(_) => StatusCode::TOO_MANY_REQUESTS,
            Error::Request(_) => StatusCode::BAD_REQUEST,
            Error::S3(_) => StatusCode::BAD_REQUEST,
            Error::SnapshotErrorDb(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::SnapshotErrorCache(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Sqlite(_) => StatusCode::BAD_REQUEST,
            Error::Timeout(_) => StatusCode::REQUEST_TIMEOUT,
            Error::Token(_) => StatusCode::UNAUTHORIZED,
            Error::Transaction(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Error::WAL(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::WebSocket(_) => StatusCode::BAD_REQUEST,
        };

        (status, Json(self)).into_response()
    }
}

impl From<std::fmt::Error> for Error {
    fn from(value: std::fmt::Error) -> Self {
        Self::Error(value.to_string().into())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Error(value.to_string().into())
    }
}

impl From<Box<EncodeError>> for Error {
    fn from(value: Box<EncodeError>) -> Self {
        trace!("bincode_next::EncodeError: {value}");
        Self::Bincode(value.to_string())
    }
}

impl From<Box<DecodeError>> for Error {
    fn from(value: Box<DecodeError>) -> Self {
        trace!("bincode_next::DecodeError: {value}");
        Self::Bincode(value.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        trace!("reqwest::Error: {value}");
        if value.is_connect() {
            Self::Connect(value.to_string())
        } else if value.is_timeout() {
            Self::Timeout(value.to_string())
        } else {
            Self::Request(value.to_string())
        }
    }
}

#[cfg(feature = "sqlite")]
impl From<RaftError<ConfigDb, ClientWriteError<ConfigDb>>> for Error {
    fn from(value: RaftError<ConfigDb, ClientWriteError<ConfigDb>>) -> Self {
        trace!("ClientWriteError: {value}");
        Self::ClientWriteErrorDb(Box::new(value))
    }
}

#[cfg(feature = "cache")]
impl From<RaftError<ConfigCache, ClientWriteError<ConfigCache>>> for Error {
    fn from(value: RaftError<ConfigCache, ClientWriteError<ConfigCache>>) -> Self {
        trace!("ClientWriteError: {value}");
        Self::ClientWriteErrorCache(Box::new(value))
    }
}

#[cfg(feature = "sqlite")]
impl From<RaftError<ConfigDb, InitializeError<ConfigDb>>> for Error {
    fn from(value: RaftError<ConfigDb, InitializeError<ConfigDb>>) -> Self {
        trace!("InitializeError: {value}");
        Self::InitializeErrorDb(Box::new(value))
    }
}

#[cfg(feature = "cache")]
impl From<RaftError<ConfigCache, InitializeError<ConfigCache>>> for Error {
    fn from(value: RaftError<ConfigCache, InitializeError<ConfigCache>>) -> Self {
        trace!("InitializeError: {value}");
        Self::InitializeErrorCache(Box::new(value))
    }
}

#[cfg(feature = "sqlite")]
impl From<StreamingError<ConfigDb>> for Error {
    fn from(value: StreamingError<ConfigDb>) -> Self {
        trace!("SnapshotError: {value}");
        Self::SnapshotErrorDb(Box::new(value))
    }
}

#[cfg(feature = "cache")]
impl From<StreamingError<ConfigCache>> for Error {
    fn from(value: StreamingError<ConfigCache>) -> Self {
        trace!("SnapshotError: {value}");
        Self::SnapshotErrorCache(Box::new(value))
    }
}

#[cfg(feature = "sqlite")]
impl From<RaftError<ConfigDb>> for Error {
    fn from(value: RaftError<ConfigDb>) -> Self {
        trace!("RaftError: {value}");
        Self::RaftErrorDb(Box::new(value))
    }
}

#[cfg(feature = "cache")]
impl From<RaftError<ConfigCache>> for Error {
    fn from(value: RaftError<ConfigCache>) -> Self {
        trace!("RaftError: {value}");
        Self::RaftErrorCache(Box::new(value))
    }
}

#[cfg(feature = "sqlite")]
impl From<RaftError<ConfigDb, LinearizableReadError<ConfigDb>>> for Error {
    fn from(value: RaftError<ConfigDb, LinearizableReadError<ConfigDb>>) -> Self {
        trace!("CheckIsLeaderError: {value}");
        Self::CheckIsLeaderErrorDb(Box::new(value))
    }
}

#[cfg(feature = "cache")]
impl From<RaftError<ConfigCache, LinearizableReadError<ConfigCache>>> for Error {
    fn from(value: RaftError<ConfigCache, LinearizableReadError<ConfigCache>>) -> Self {
        trace!("CheckIsLeaderError: {value}");
        Self::CheckIsLeaderErrorCache(Box::new(value))
    }
}

#[cfg(feature = "sqlite")]
impl From<Fatal<ConfigDb>> for Error {
    fn from(value: Fatal<ConfigDb>) -> Self {
        trace!("RaftErrorFatal: {value}");
        Self::RaftErrorFatalDb(Box::new(value))
    }
}

#[cfg(feature = "cache")]
impl From<Fatal<ConfigCache>> for Error {
    fn from(value: Fatal<ConfigCache>) -> Self {
        trace!("RaftErrorFatal: {value}");
        Self::RaftErrorFatalCache(Box::new(value))
    }
}

#[cfg(feature = "sqlite")]
impl From<Box<StorageError<ConfigDb>>> for Error {
    fn from(value: Box<StorageError<ConfigDb>>) -> Self {
        trace!("RaftErrorFatal: {value}");
        Self::RaftErrorFatalDb(Box::new(Fatal::StorageError(*value)))
    }
}

#[cfg(feature = "cache")]
impl From<Box<StorageError<ConfigCache>>> for Error {
    fn from(value: Box<StorageError<ConfigCache>>) -> Self {
        trace!("RaftErrorFatal: {value}");
        Self::RaftErrorFatalCache(Box::new(Fatal::StorageError(*value)))
    }
}

#[cfg(feature = "sqlite")]
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        trace!("rusqlite::Error: {value}");

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
                    Self::ConstraintViolation(format!("{err} {ext:?}"))
                }
                // ErrorCode::TypeMismatch => {}
                // ErrorCode::ApiMisuse => {}
                // ErrorCode::NoLargeFileSupport => {}
                // ErrorCode::AuthorizationForStatementDenied => {}
                // ErrorCode::ParameterOutOfRange => {}
                // ErrorCode::NotADatabase => {}
                // ErrorCode::Unknown => {}
                _ => Self::Sqlite(format!("{err} {ext:?}").into()),
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
        trace!("Sqlite: {value}");
        Self::Sqlite(value.to_string().into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        trace!("BadRequest: {value}");
        Self::BadRequest(value.to_string().into())
    }
}

impl From<DecodeError> for Error {
    fn from(value: DecodeError) -> Self {
        trace!("DecodeError: {value}");
        Self::Bincode(value.to_string())
    }
}

impl From<EncodeError> for Error {
    fn from(value: EncodeError) -> Self {
        trace!("EncodeError: {value}");
        Self::Bincode(value.to_string())
    }
}

impl From<fastwebsockets::WebSocketError> for Error {
    fn from(value: WebSocketError) -> Self {
        trace!("WebSocket: {value}");
        Self::WebSocket(value.to_string())
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        trace!("JoinError: {value}");
        Self::Error(value.to_string().into())
    }
}

impl From<flume::RecvError> for Error {
    fn from(value: flume::RecvError) -> Self {
        trace!("flume::RecvError: {value}");
        Self::Channel(value.to_string())
    }
}

#[cfg(feature = "listen_notify")]
impl From<flume::SendError<NotifyRequest>> for Error {
    fn from(value: flume::SendError<NotifyRequest>) -> Self {
        trace!("flume::SendError<NotifyRequest>: {value}");
        Self::Channel(value.to_string())
    }
}

#[cfg(feature = "s3")]
impl From<cryptr::stream::s3::S3Error> for Error {
    fn from(value: cryptr::stream::s3::S3Error) -> Self {
        trace!("cryptr::stream::s3::S3Error: {value}");
        Self::S3(value.to_string())
    }
}

#[cfg(any(feature = "dashboard", feature = "s3"))]
impl From<cryptr::CryptrError> for Error {
    fn from(value: cryptr::CryptrError) -> Self {
        trace!("cryptr::CryptrError: {value}");
        Self::Cryptr(value.to_string())
    }
}

#[cfg(feature = "dashboard")]
impl From<argon2::password_hash::Error> for Error {
    fn from(value: argon2::password_hash::Error) -> Self {
        trace!("argon2::password_hash::Error: {value}");
        Self::Unauthorized("invalid credentials".into())
    }
}

#[cfg(feature = "dashboard")]
impl From<argon2::password_hash::phc::Error> for Error {
    fn from(value: argon2::password_hash::phc::Error) -> Self {
        trace!("argon2::password_hash::phc::Error: {value}");
        Self::Unauthorized("invalid credentials".into())
    }
}

impl From<hiqlite_wal::error::Error> for Error {
    fn from(value: hiqlite_wal::error::Error) -> Self {
        trace!("hiqlite_wal::error::Error: {value}");
        Self::WAL(value.to_string())
    }
}

impl From<Infallible> for Error {
    fn from(value: Infallible) -> Self {
        unreachable!("Infallible: {value}");
    }
}

/// Lets the storage traits of openraft 0.10, which are defined in terms of `io::Error`, carry
/// our errors. The error kind is flattened to "other", openraft adds its own context on top.
impl From<Error> for std::io::Error {
    fn from(value: Error) -> Self {
        Self::other(value.to_string())
    }
}
