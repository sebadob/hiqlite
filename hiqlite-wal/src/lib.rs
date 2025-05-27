// Copyright 2025 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../README.md")]

pub use crate::writer::LogSync;
pub use shutdown::ShutdownHandle;
pub use writer::Action;

pub use log_store::{LogStore, LogStoreReader};

pub mod error;
mod log_store;
mod log_store_impl;
mod metadata;
pub mod reader;
mod shutdown;
mod utils;
mod wal;
pub mod writer;
