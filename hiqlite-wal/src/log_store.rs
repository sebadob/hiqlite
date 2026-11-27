use crate::error::Error;
use crate::lockfile::LockFile;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use crate::{LogSync, ShutdownHandle, reader, writer};
use openraft::RaftTypeConfig;
use std::fs;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
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
    /// Disconnects once the WAL writer thread has exited, for any reason.
    pub(crate) writer_closed: flume::Receiver<()>,
    pub reader: flume::Sender<reader::Action>,
    pub(crate) reader_closed: flume::Receiver<()>,
    readers: Arc<reader::Readers>,
    shutdown: ShutdownHandle,
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

            let (writer, writer_closed, wal) = writer::spawn(
                base_path,
                lockfile,
                sync,
                wal_size,
                lock_exists,
                meta.clone(),
                committed.clone(),
            )?;
            let readers = Arc::new(reader::Readers::new());
            let (reader, reader_closed) = readers.spawn(meta.clone(), wal.clone());
            let shutdown =
                ShutdownHandle::new(writer.clone(), writer_closed.clone(), readers.clone());

            Ok::<Self, Error>(Self {
                meta,
                committed,
                wal,
                writer,
                writer_closed,
                reader,
                reader_closed,
                readers,
                shutdown,
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
            let (writer, _, _) = writer::spawn(
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
        self.shutdown.clone()
    }

    ///
    /// # Errors
    ///
    /// [`Error::Internal`] when the WAL writer task is no longer reachable, and
    /// [`Error::Generic`] when a shutdown acknowledgement is never sent.
    pub async fn stop(self) -> Result<(), Error> {
        self.shutdown_handle().shutdown().await
    }

    pub(crate) fn spawn_reader(&self) -> LogStoreReader<T> {
        let (tx, closed) = self.readers.spawn(self.meta.clone(), self.wal.clone());

        LogStoreReader {
            tx,
            closed,
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
    pub(crate) closed: flume::Receiver<()>,
    _marker: PhantomData<T>,
}
