use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use crate::{reader, writer, LogSync, ShutdownHandle};
use std::fs;
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use tokio::task;

#[derive(Debug)]
pub struct LogStore {
    meta: Arc<RwLock<Metadata>>,
    wal: Arc<RwLock<WalFileSet>>,
    pub writer: flume::Sender<writer::Action>,
    pub reader: flume::Sender<reader::Action>,
}

impl LogStore {
    pub async fn start(base_path: String, sync: LogSync, wal_size: u32) -> Result<Self, Error> {
        let slf = task::spawn_blocking(move || {
            fs::create_dir_all(&base_path)?;
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&base_path)?.permissions();
                // TODO why do we need +x here? for memmap?
                perms.set_mode(0o700);
                fs::set_permissions(&base_path, perms)?;
            }

            let meta = Metadata::read_or_create(&base_path)?;
            let meta = Arc::new(RwLock::new(meta));

            let (writer, wal) = writer::spawn(base_path, sync, wal_size, meta.clone())?;
            let reader = reader::spawn(meta.clone(), wal.clone())?;

            Ok::<Self, Error>(Self {
                meta,
                wal,
                writer,
                reader,
            })
        })
        .await??;

        Ok(slf)
    }

    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle::new(self.writer.clone(), self.reader.clone())
    }

    pub async fn stop(self) -> Result<(), Error> {
        let (tx_ack, ack) = oneshot::channel();
        self.writer
            .send_async(writer::Action::Shutdown(tx_ack))
            .await?;
        ack.await?;

        let _ = self.reader.send_async(reader::Action::Shutdown).await;

        Ok(())
    }

    pub fn spawn_reader(&self) -> Result<LogStoreReader, Error> {
        let tx = reader::spawn(self.meta.clone(), self.wal.clone())?;

        Ok(LogStoreReader { tx })
    }
}

#[derive(Debug)]
pub struct LogStoreReader {
    pub tx: flume::Sender<reader::Action>,
}
