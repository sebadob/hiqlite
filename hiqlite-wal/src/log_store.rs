use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use crate::{reader, writer, LogSync};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use tokio::task;

#[derive(Debug)]
pub struct LogStore {
    pub id_until: Arc<AtomicU64>,
    pub latest_wal: Arc<AtomicU64>,
    pub meta: Arc<RwLock<Metadata>>,
    pub wal: Arc<RwLock<WalFileSet>>,

    pub writer: flume::Sender<writer::Action>,
    pub reader: flume::Sender<reader::Action>,
}

impl LogStore {
    pub async fn start(base_path: String, sync: LogSync, wal_size: u32) -> Result<Self, Error> {
        let slf = task::spawn_blocking(move || {
            let meta = Metadata::read(&base_path)?;
            let id_until = Arc::new(AtomicU64::new(meta.log_until));
            let meta = Arc::new(RwLock::new(meta));

            let (writer, wal, latest_wal) =
                writer::spawn(base_path, sync, wal_size, meta.clone(), id_until.clone())?;
            let reader = reader::spawn(
                meta.clone(),
                wal.clone(),
                id_until.clone(),
                latest_wal.clone(),
            )?;

            Ok::<Self, Error>(Self {
                meta,
                id_until,
                latest_wal,
                wal,
                writer,
                reader,
            })
        })
        .await??;

        Ok(slf)
    }

    pub async fn stop(self) -> Result<(), Error> {
        let (tx_ack, ack) = oneshot::channel();
        self.writer
            .send_async(writer::Action::Shutdown(tx_ack))
            .await?;
        ack.await?;

        let _ = self.reader.send_async(reader::Action::Shutdown);

        Ok(())
    }

    pub fn spawn_reader(&self) -> Result<LogStoreReader, Error> {
        let tx = reader::spawn(
            self.meta.clone(),
            self.wal.clone(),
            self.id_until.clone(),
            self.latest_wal.clone(),
        )?;

        Ok(LogStoreReader { tx })
    }
}

#[derive(Debug)]
pub struct LogStoreReader {
    pub tx: flume::Sender<reader::Action>,
}
