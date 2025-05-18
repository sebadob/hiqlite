use std::borrow::Cow;
use std::io;
use std::num::ParseIntError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("DecodeError: {0}")]
    Decode(String),
    #[error("Generic: {0}")]
    Generic(Cow<'static, str>),
    #[error("EncodeError: {0}")]
    Encode(String),
    #[error("FileCorrupted: {0}")]
    FileCorrupted(&'static str),
    #[error("Integrity: {0}")]
    Integrity(Cow<'static, str>),
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
