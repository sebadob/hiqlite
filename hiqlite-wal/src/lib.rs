// Copyright 2025 Sebastian Dobe <sebastiandobe@mailbox.org>

#![doc = include_str!("../README.md")]

pub use crate::writer::LogSync;
pub use log_store::{LogStore, LogStoreReader};
pub use shutdown::ShutdownHandle;
pub use writer::Action;

pub mod error;
mod log_store;
mod log_store_impl;
mod metadata;
mod reader;
mod shutdown;
mod utils;
mod wal;
// TODO make private after the `rocksdb-migrate` feature was removed in `hiqlite`
pub mod writer;
