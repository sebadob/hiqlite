use crate::metadata::Metadata;
use crate::{reader, writer};
use std::borrow::Cow;
use std::io;
use std::num::ParseIntError;
use std::sync::{PoisonError, RwLockReadGuard, RwLockWriteGuard};
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::task;

#[derive(Debug, Error)]
pub enum Error {
    #[error("DecodeError: {0}")]
    Decode(String),
    #[error("Generic: {0}")]
    Generic(Cow<'static, str>),
    #[error("EncodeError: {0}")]
    Encode(String),
    #[error("FileCorrupted: {0}")]
    FileCorrupted(Cow<'static, str>),
    #[error("Integrity: {0}")]
    Integrity(Cow<'static, str>),
    #[error("Internal: {0}")]
    Internal(Cow<'static, str>),
    #[error("InvalidPath: {0}")]
    InvalidPath(&'static str),
    #[error("InvalidFileName")]
    InvalidFileName,
    #[error("IOError: {0}")]
    IO(#[from] io::Error),
    #[error("Locked: {0}")]
    Locked(&'static str),
    #[error("ParseError: {0}")]
    Parse(&'static str),
}

impl From<task::JoinError> for Error {
    fn from(err: task::JoinError) -> Self {
        Self::Generic(err.to_string().into())
    }
}

impl From<bincode::error::DecodeError> for Error {
    fn from(err: bincode::error::DecodeError) -> Self {
        Self::Decode(err.to_string())
    }
}

impl From<bincode::error::EncodeError> for Error {
    fn from(err: bincode::error::EncodeError) -> Self {
        Self::Encode(err.to_string())
    }
}

impl From<ParseIntError> for Error {
    fn from(_: ParseIntError) -> Self {
        Self::Parse("Cannot parse value as integer")
    }
}

impl From<PoisonError<RwLockReadGuard<'_, Metadata>>> for Error {
    fn from(err: PoisonError<RwLockReadGuard<Metadata>>) -> Self {
        Self::Generic(err.to_string().into())
    }
}

impl From<PoisonError<RwLockWriteGuard<'_, Metadata>>> for Error {
    fn from(err: PoisonError<RwLockWriteGuard<Metadata>>) -> Self {
        Self::Generic(err.to_string().into())
    }
}

impl From<flume::SendError<writer::Action>> for Error {
    fn from(err: flume::SendError<writer::Action>) -> Self {
        Self::Internal(err.to_string().into())
    }
}

impl From<flume::SendError<reader::Action>> for Error {
    fn from(err: flume::SendError<reader::Action>) -> Self {
        Self::Internal(err.to_string().into())
    }
}

impl From<oneshot::error::RecvError> for Error {
    fn from(err: oneshot::error::RecvError) -> Self {
        Self::Generic(err.to_string().into())
    }
}
