use crate::error::Error;
use crate::lockfile::LockFile;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use crate::{reader, writer, LogSync, ShutdownHandle};
use openraft::RaftTypeConfig;
use std::fs;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;
use tokio::task;
use tracing::warn;

/// `T::NodeId` MUST be a `u64` for the `LogStore` to work correctly.
#[derive(Debug)]
pub struct LogStore<T>
where
    T: RaftTypeConfig,
{
    meta: Arc<RwLock<Metadata>>,
    wal: Arc<RwLock<WalFileSet>>,
    pub writer: flume::Sender<writer::Action>,
    pub reader: flume::Sender<reader::Action>,
    _marker: PhantomData<T>,
}

impl<T> LogStore<T>
where
    T: RaftTypeConfig,
{
    /// Start the LogStore
    pub async fn start(base_path: String, sync: LogSync, wal_size: u32) -> Result<Self, Error> {
        let slf = task::spawn_blocking(move || {
            fs::create_dir_all(&base_path)?;
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&base_path)?.permissions();
                perms.set_mode(0o700);
                fs::set_permissions(&base_path, perms)?;
            }

            let lock_exists = LockFile::exists(&base_path)?;
            if lock_exists {
                warn!("LockFile {base_path} exists already - this is not a clean start!");
                if LockFile::is_locked(&base_path)? {
                    panic!("LockFile {base_path} is locked and in use by another process");
                }
            }
            let lockfile = LockFile::create(&base_path)?;
            lockfile.lock()?;
            debug_assert!(LockFile::is_locked(&base_path).unwrap());

            let meta = Metadata::read_or_create(&base_path)?;
            let meta = Arc::new(RwLock::new(meta));

            let (writer, wal) = writer::spawn(
                base_path,
                lockfile,
                sync,
                wal_size,
                lock_exists,
                meta.clone(),
            )?;
            let reader = reader::spawn(meta.clone(), wal.clone())?;

            Ok::<Self, Error>(Self {
                meta,
                wal,
                writer,
                reader,
                _marker: Default::default(),
            })
        })
        .await??;

        Ok(slf)
    }

    /// Gives you a raw handle to the writer channel to perform manual migrations. Does not start
    /// a log store and does not do anything on its own.
    #[cfg(feature = "migration")]
    pub async fn start_writer_migration(
        base_path: String,
        wal_size: u32,
    ) -> Result<flume::Sender<writer::Action>, Error> {
        let lock_exists = LockFile::exists(&base_path)?;
        if lock_exists {
            warn!("LockFile in {base_path} exists already - this is not a clean start!");
            if LockFile::is_locked(&base_path)? {
                panic!("LockFile {base_path} is locked and in use by another process");
            }
        }
        let lockfile = LockFile::create(&base_path)?;
        lockfile.lock()?;

        task::spawn_blocking(move || {
            let meta = Metadata::read_or_create(&base_path)?;
            let meta = Arc::new(RwLock::new(meta));
            let (writer, _) = writer::spawn(
                base_path,
                lockfile,
                LogSync::ImmediateAsync,
                wal_size,
                lock_exists,
                meta,
            )?;
            Ok(writer)
        })
        .await?
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

    pub(crate) fn spawn_reader(&self) -> Result<LogStoreReader<T>, Error> {
        let tx = reader::spawn(self.meta.clone(), self.wal.clone())?;

        Ok(LogStoreReader {
            tx,
            _marker: self._marker,
        })
    }
}

#[derive(Debug)]
pub struct LogStoreReader<T>
where
    T: RaftTypeConfig,
{
    pub tx: flume::Sender<reader::Action>,
    _marker: PhantomData<T>,
}
