use crate::error::Error;
use crate::lockfile::LockFile;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use crate::{LogSync, ShutdownHandle, reader, writer};
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
    /// The in-memory `committed` log id, serialized. Written by [`RaftLogStorage::save_committed`]
    /// and read back by [`RaftLogStorage::read_committed`]; the WAL writer publishes it into the
    /// active file's header on the flush that is already running, which is the only place it is
    /// ever durable.
    ///
    /// [`RaftLogStorage::save_committed`]: openraft::storage::RaftLogStorage::save_committed
    /// [`RaftLogStorage::read_committed`]: openraft::storage::RaftLogStorage::read_committed
    pub(crate) committed: Arc<RwLock<Option<Vec<u8>>>>,
    wal: Arc<RwLock<WalFileSet>>,
    pub writer: flume::Sender<writer::Action>,
    pub reader: flume::Sender<reader::Action>,
    _marker: PhantomData<T>,
}

impl<T> LogStore<T>
where
    T: RaftTypeConfig,
{
    /// Start the `LogStore`
    ///
    /// # Errors
    ///
    /// Any [`Error`] from the log directory setup: [`Error::IO`] for the directory itself, its
    /// lock file and the meta file, [`Error::Decode`] and [`Error::Integrity`] for a meta file
    /// that cannot be decoded, and [`Error::Generic`] when the blocking setup task fails.
    ///
    /// # Panics
    ///
    /// If `base_path` already holds a lock file that is locked by another process.
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
            let committed = Arc::new(RwLock::new(None));

            let (writer, wal) = writer::spawn(
                base_path,
                lockfile,
                sync,
                wal_size,
                lock_exists,
                meta.clone(),
                committed.clone(),
            )?;
            let reader = reader::spawn(meta.clone(), wal.clone());

            Ok::<Self, Error>(Self {
                meta,
                committed,
                wal,
                writer,
                reader,
                _marker: PhantomData,
            })
        })
        .await??;

        Ok(slf)
    }

    /// Gives you a raw handle to the writer channel to perform manual migrations. Does not start
    /// a log store and does not do anything on its own.
    ///
    /// # Errors
    ///
    /// Any [`Error`] from the log directory setup: [`Error::IO`] for the directory itself, its
    /// lock file and the meta file, [`Error::Decode`] and [`Error::Integrity`] for a meta file
    /// that cannot be decoded, and [`Error::Generic`] when the blocking setup task fails.
    ///
    /// # Panics
    ///
    /// If `base_path` already holds a lock file that is locked by another process.
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
                LogSync::IntervalMillis(200),
                wal_size,
                lock_exists,
                meta,
                Arc::new(RwLock::new(None)),
            )?;
            Ok(writer)
        })
        .await?
    }

    #[must_use]
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle::new(self.writer.clone(), self.reader.clone())
    }

    ///
    /// # Errors
    ///
    /// [`Error::Internal`] when the WAL writer task is no longer reachable, and
    /// [`Error::Generic`] when its shutdown acknowledgement is never sent.
    pub async fn stop(self) -> Result<(), Error> {
        let (tx_ack, ack) = oneshot::channel();
        self.writer
            .send_async(writer::Action::Shutdown(tx_ack))
            .await?;
        ack.await?;

        let _ = self.reader.send_async(reader::Action::Shutdown).await;

        Ok(())
    }

    pub(crate) fn spawn_reader(&self) -> LogStoreReader<T> {
        let tx = reader::spawn(self.meta.clone(), self.wal.clone());

        LogStoreReader {
            tx,
            _marker: self._marker,
        }
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
