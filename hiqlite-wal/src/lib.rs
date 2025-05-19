use crate::error::Error;
use crate::metadata::Metadata;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

pub use crate::writer::LogSync;
pub use shutdown::ShutdownSender;
pub use writer::Action;

pub mod error;
mod metadata;
mod reader;
mod shutdown;
mod utils;
mod wal;
mod writer;

pub async fn run_background(
    base_path: String,
    sync: LogSync,
    wal_size: u32,
) -> Result<ShutdownSender, Error> {
    let meta = Metadata::read(&base_path)?;
    let id_until = AtomicU64::new(meta.log_until);
    let meta = Arc::new(RwLock::new(meta));

    let tx = writer::spawn(base_path, sync, wal_size, meta, id_until)?;

    Ok(ShutdownSender::new(tx))
}
