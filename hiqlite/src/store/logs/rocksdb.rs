use crate::helpers::{deserialize, serialize, set_path_access};
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::store::{logs, StorageResult};
use crate::NodeId;
use bincode::error::DecodeError;
use byteorder::BigEndian;
use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;
use flume::RecvError;
use openraft::storage::Snapshot;
use openraft::storage::{LogFlushed, LogState, RaftLogStorage};
use openraft::Entry;
use openraft::EntryPayload;
use openraft::ErrorVerb;
use openraft::LogId;
use openraft::OptionalSend;
use openraft::RaftLogReader;
use openraft::RaftSnapshotBuilder;
use openraft::RaftTypeConfig;
use openraft::SnapshotMeta;
use openraft::StorageError;
use openraft::StorageIOError;
use openraft::StoredMembership;
use openraft::TokioRuntime;
use openraft::Vote;
use openraft::{AnyError, ErrorSubject};
use openraft::{BasicNode, LeaderId};
use rocksdb::{ColumnFamily, WriteBatchWithTransaction, WriteOptions};
use rocksdb::{ColumnFamilyDescriptor, FlushOptions};
use rocksdb::{DBCompressionType, Direction};
use rocksdb::{LogLevel, Options};
use rocksdb::{WriteBatch, DB};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use std::collections::{BTreeMap, Bound};
use std::env;
use std::error::Error;
use std::fmt::Debug;
use std::future::Future;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};
use tokio::time::Interval;
use tokio::{fs, task, time};
use tracing::{debug, error, info, trace, warn};

// static KEY_COMMITTED: &[u8] = b"committed";
static KEY_LAST_PURGED: &[u8] = b"last_purged";
static KEY_VOTE: &[u8] = b"vote";

pub enum ActionWrite {
    Append(ActionAppend),
    Remove(ActionRemove),
    Vote(ActionVote),
    Sync,
    Shutdown(oneshot::Sender<()>),
}

pub struct ActionAppend {
    rx: flume::Receiver<Option<(Vec<u8>, Vec<u8>)>>,
    // TODO with 0.10 the callback will be async ready
    callback: LogFlushed<TypeConfigSqlite>,
    ack: oneshot::Sender<Result<(), StorageIOError<NodeId>>>,
}

pub struct ActionVote {
    value: Vec<u8>,
    ack: oneshot::Sender<Result<(), StorageIOError<NodeId>>>,
}

pub struct ActionRemove {
    from: Vec<u8>,
    until: Vec<u8>,
    last_log: Option<Vec<u8>>,
    ack: oneshot::Sender<Result<(), StorageError<NodeId>>>,
}

pub struct ActionBackup {
    pub path: String,
}

// TODO we maybe don't need that at all and can use `.to_be_bytes()` -> check + bench
/// converts an id to a byte vector for storing in the database.
/// Note that we're using big endian encoding to ensure correct sorting of keys
#[inline]
fn id_to_bin(id: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.write_u64::<BigEndian>(id).unwrap();
    buf
}

#[inline]
fn bin_to_id(buf: &[u8]) -> u64 {
    (&buf[0..8]).read_u64::<BigEndian>().unwrap()
}

#[derive(Debug)]
struct LogStoreWriter {
    db: Arc<DB>,
}

impl LogStoreWriter {
    fn spawn(db: Arc<DB>, sync_immediate: bool) -> flume::Sender<ActionWrite> {
        let (tx, rx) = flume::bounded::<ActionWrite>(2);

        std::thread::spawn(move || {
            // task::spawn_blocking(move || {
            // let mut callbacks = Vec::with_capacity(8);

            let mut is_dirty = false;
            let mut shutdown_ack: Option<oneshot::Sender<()>> = None;

            while let Ok(action) = rx.recv() {
                match action {
                    ActionWrite::Append(ActionAppend { rx, callback, ack }) => {
                        let mut res = Ok(());

                        while let Ok(Some((id, data))) = rx.recv() {
                            if let Err(err) = db.put_cf(db.cf_handle("logs").unwrap(), id, data) {
                                res = Err(StorageIOError::write_logs(&err));
                                break;
                            }
                        }

                        is_dirty = true;
                        let is_ok = res.is_ok();

                        if let Err(err) = ack.send(res) {
                            // this should usually not happen, but it may during a shutdown crash
                            error!("error sending back ack after logs append: {:?}", err);
                            // db.flush_wal(true);
                        }

                        if is_ok {
                            // TODO with the next big openraft release, we can do async callbacks

                            if sync_immediate {
                                db.flush_wal(true);
                            }
                            callback.log_io_completed(Ok(()));
                        }
                    }
                    ActionWrite::Remove(ActionRemove {
                        from,
                        until,
                        last_log,
                        ack,
                    }) => {
                        let mut res = db
                            .delete_range_cf(db.cf_handle("logs").unwrap(), &from, &until)
                            .map_err(|err| StorageError::IO {
                                source: StorageIOError::write_logs(&err),
                            });

                        if res.is_ok() {
                            if let Some(value) = last_log {
                                res = db
                                    .put_cf(db.cf_handle("meta").unwrap(), KEY_LAST_PURGED, &value)
                                    .map_err(|err| StorageError::IO {
                                        source: StorageIOError::write_logs(&err),
                                    });
                            }
                        };

                        is_dirty = true;
                        ack.send(res).unwrap();
                    }

                    ActionWrite::Vote(ActionVote { value, ack }) => {
                        let res = db
                            .put_cf(db.cf_handle("meta").unwrap(), KEY_VOTE, &value)
                            .map_err(|err| {
                                StorageIOError::new(
                                    ErrorSubject::Vote,
                                    ErrorVerb::Write,
                                    AnyError::new(&err),
                                )
                            });

                        ack.send(res).unwrap();
                    }

                    ActionWrite::Sync => {
                        // async append callbacks are only available for openraft 0.10+
                        if is_dirty {
                            db.flush_wal(true);
                            is_dirty = false;
                        }
                        // for callback in callbacks.drain(..) {
                        //     callback.log_io_completed(Ok(()));
                        // }
                        //
                        // assert!(callbacks.is_empty());
                    }

                    ActionWrite::Shutdown(ack) => {
                        warn!("Raft logs store writer is being shut down");
                        shutdown_ack = Some(ack);
                        break;
                    }
                }
            }

            db.flush_wal(true);
            warn!("Logs Writer exiting");

            if let Some(ack) = shutdown_ack {
                ack.send(())
                    .expect("Shutdown handler to always wait for ack from logs");
            }
        });

        tx
    }
}

// TODO Should not be started depending on when the log has been start, but instead
// after the very first append message has been received to be more in sync with the master
struct LogsSyncer;

impl LogsSyncer {
    fn spawn(tx_writer: flume::Sender<ActionWrite>, mut interval: Interval) {
        task::spawn(async move {
            loop {
                interval.tick().await;
                if let Err(err) = tx_writer.send_async(ActionWrite::Sync).await {
                    debug!("Error sending ActionWrite::Sync to LogStoreWriter - exiting");
                    break;
                }
            }
        });
    }
}

enum ActionRead {
    Logs(ActionReadLogs),
    LogState(oneshot::Sender<Result<LogState<TypeConfigSqlite>, StorageIOError<NodeId>>>),
    Vote(oneshot::Sender<Result<Option<Vec<u8>>, StorageIOError<NodeId>>>),
    Shutdown,
}

struct ActionReadLogs {
    from: Vec<u8>,
    until: u64,
    ack: flume::Sender<Option<Result<Entry<TypeConfigSqlite>, StorageError<NodeId>>>>,
}

#[derive(Debug)]
struct LogStoreReader {
    db: Arc<DB>,
}

impl LogStoreReader {
    fn spawn(db: Arc<DB>) -> flume::Sender<ActionRead> {
        let (tx, rx) = flume::bounded::<ActionRead>(2);

        #[inline]
        fn read_logs_err(err: impl Error + 'static) -> StorageError<NodeId> {
            StorageError::IO {
                source: StorageIOError::read_logs(&err),
            }
        }

        std::thread::spawn(move || {
            while let Ok(action) = rx.recv() {
                match action {
                    ActionRead::Logs(ActionReadLogs { from, until, ack }) => {
                        let logs = db.iterator_cf(
                            db.cf_handle("logs").unwrap(),
                            rocksdb::IteratorMode::From(&from, Direction::Forward),
                        );

                        for log in logs {
                            match log {
                                Ok((id, value)) => {
                                    if bin_to_id(id.as_ref()) >= until {
                                        trace!("Raft logs store reader: bin_to_id(id) >= until");
                                        break;
                                    }

                                    let entry = deserialize::<Entry<_>>(&value)
                                        .map_err(read_logs_err)
                                        .unwrap();
                                    ack.send(Some(Ok(entry))).unwrap();
                                }
                                Err(err) => {
                                    error!("Raft logs store reader: {}", err);
                                    ack.send(Some(Err(read_logs_err(err)))).unwrap();
                                    break;
                                }
                            }
                        }

                        // we ignore the result because the channel will be closed at this point in case of an error
                        let _ = ack.send(None);
                    }

                    ActionRead::LogState(ack) => {
                        let res = db
                            .iterator_cf(db.cf_handle("logs").unwrap(), rocksdb::IteratorMode::End)
                            .next();

                        let last_log_id = if let Some(res) = res {
                            if let Err(err) = &res {
                                ack.send(Err(StorageIOError::new(
                                    ErrorSubject::Logs,
                                    ErrorVerb::Read,
                                    AnyError::new(err),
                                )))
                                .unwrap();
                                continue;
                            }

                            let (_, bytes) = res.unwrap();
                            let res =
                                deserialize::<Entry<TypeConfigSqlite>>(&bytes).map_err(|err| {
                                    StorageIOError::new(
                                        ErrorSubject::Logs,
                                        ErrorVerb::Read,
                                        AnyError::new(&err),
                                    )
                                });

                            match res {
                                Ok(entry) => Some(entry.log_id),
                                Err(err) => {
                                    ack.send(Err(err)).unwrap();
                                    continue;
                                }
                            }
                        } else {
                            None
                        };

                        let res = db.get_cf(db.cf_handle("meta").unwrap(), KEY_LAST_PURGED);
                        let last_purged_log_id = match res {
                            Ok(Some(bytes)) => match deserialize(&bytes) {
                                Ok(log_id) => Some(log_id),
                                Err(err) => {
                                    ack.send(Err(StorageIOError::read_logs(&err))).unwrap();
                                    continue;
                                }
                            },
                            Ok(None) => None,
                            Err(err) => {
                                ack.send(Err(StorageIOError::read_logs(&err))).unwrap();
                                continue;
                            }
                        };

                        ack.send(Ok(LogState {
                            last_purged_log_id,
                            last_log_id,
                        }))
                        .unwrap()
                    }

                    ActionRead::Vote(ack) => {
                        let res =
                            db.get_cf(db.cf_handle("meta").unwrap(), KEY_VOTE)
                                .map_err(|err| {
                                    StorageIOError::new(
                                        ErrorSubject::Vote,
                                        ErrorVerb::Read,
                                        AnyError::new(&err),
                                    )
                                });

                        ack.send(res).unwrap();
                    }

                    ActionRead::Shutdown => {
                        warn!("Raft logs store reader is being shut down");
                        break;
                    }
                }
            }

            warn!("Logs Reader exiting");
        });

        tx
    }
}

#[derive(Debug)]
pub struct LogStoreRocksdb {
    db: Arc<DB>,
    pub(crate) tx_writer: flume::Sender<ActionWrite>,
    tx_reader: flume::Sender<ActionRead>,
}

impl LogStoreRocksdb {
    pub async fn new(data_dir: &str, sync_immediate: bool) -> Self {
        let dir = logs::logs_dir(data_dir);
        fs::create_dir_all(&dir)
            .await
            .expect("Cannot create logs path");
        set_path_access(&dir, 0o700)
            .await
            .expect("Cannot set proper access rights");

        let mut opts = Options::default();
        opts.create_missing_column_families(true);
        opts.create_if_missing(true);

        // opts.set_use_direct_io_for_flush_and_compaction(true);
        // opts.set_use_direct_reads(true);
        // opts.set_enable_pipelined_write(true);
        // opts.increase_parallelism(2);

        opts.set_log_level(LogLevel::Warn);
        opts.set_max_log_file_size(2 * 1024 * 1024);
        opts.set_keep_log_file_num(2);

        // TODO check if we can have a setup that never writes SST files and only ever uses a WAL
        // with rollover -> maybe manual wal compaction on purge?
        // TODO needs fine-tuning https://github.com/facebook/rocksdb/wiki/Write-Ahead-Log-(WAL)
        // TODO double check that our own LogsSyncer does everything properly or if we can optimize
        // opts.set_manual_wal_flush(true);

        opts.set_compression_type(DBCompressionType::None);
        // TODO maybe disable auto compaction and do it after every purge / truncate manually?
        opts.set_periodic_compaction_seconds(24 * 60 * 60);
        opts.set_max_manifest_file_size(4 * 1024 * 1024);

        opts.set_write_buffer_size(128 * 1024);
        opts.set_max_total_wal_size(1024 * 1024);
        opts.set_wal_size_limit_mb(1);
        let meta = ColumnFamilyDescriptor::new("meta", opts.clone());

        // 2MB is absolutely enough here after testing,
        // as long as SQLite is the SM and the limiting factor
        opts.set_write_buffer_size(2 * 1024 * 1024);

        let wal_mb_logs = 64;
        opts.set_max_total_wal_size(wal_mb_logs * 1024 * 1024);
        opts.set_wal_size_limit_mb(wal_mb_logs);
        let logs = ColumnFamilyDescriptor::new("logs", opts.clone());

        let db = DB::open_cf_descriptors(&opts, dir, vec![meta, logs])
            .expect("Cannot open rocksdb files on disk");
        let db = Arc::new(db);

        Self::maybe_leader_reset(db.clone()).await;

        let tx_writer = LogStoreWriter::spawn(db.clone(), sync_immediate);
        let tx_reader = LogStoreReader::spawn(db.clone());

        if !sync_immediate {
            let sync_interval = time::interval(Duration::from_millis(200));
            LogsSyncer::spawn(tx_writer.clone(), sync_interval);
        }

        LogStoreRocksdb {
            db,
            tx_writer,
            tx_reader,
        }
    }

    async fn maybe_leader_reset(db: Arc<DB>) {
        if env::var("HQL_DANGER_RESET_VOTE").as_deref() == Ok("true") {
            warn!(
                r#"

    HQL_DANGER_RESET_VOTE is set to 'true'.
    Resetting any possibliy existing old leader now.
    This can lead to lost logs! Use with care!

    Proceeding in 10 seconds...
"#
            );

            time::sleep(Duration::from_secs(10)).await;

            task::spawn_blocking(move || {
                db.delete_cf(db.cf_handle("meta").unwrap(), KEY_VOTE)
                    .expect("Cannot execute DANGER_LEADER_RESET");
            })
            .await;
        }
    }
}

impl RaftLogReader<TypeConfigSqlite> for LogStoreRocksdb {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> StorageResult<Vec<Entry<TypeConfigSqlite>>> {
        let start = match range.start_bound() {
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i + 1,
            Bound::Unbounded => 0,
        };
        let until = match range.end_bound() {
            Bound::Included(i) => *i + 1,
            Bound::Excluded(i) => *i,
            Bound::Unbounded => unreachable!(),
        };

        let mut res = Vec::with_capacity((until - start) as usize);

        let from = id_to_bin(start);
        // let until = id_to_bin(end);

        let (ack, rx) = flume::bounded(1);
        self.tx_reader
            .send_async(ActionRead::Logs(ActionReadLogs { from, until, ack }))
            .await
            .expect("LogsReader to always be listening");

        while let Some(entry) = rx.recv_async().await.unwrap() {
            res.push(entry?);
        }

        Ok(res)
    }
}

impl RaftLogStorage<TypeConfigSqlite> for LogStoreRocksdb {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> StorageResult<LogState<TypeConfigSqlite>> {
        let (ack, rx) = oneshot::channel();
        self.tx_reader
            .send_async(ActionRead::LogState(ack))
            .await
            .map_err(|err| {
                StorageIOError::new(ErrorSubject::Logs, ErrorVerb::Read, AnyError::new(&err))
            })?;

        let log_state = rx.await.map_err(|err| {
            StorageIOError::new(ErrorSubject::Logs, ErrorVerb::Read, AnyError::new(&err))
        })??;

        Ok(log_state)
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        let tx_reader = LogStoreReader::spawn(self.db.clone());
        Self {
            db: self.db.clone(),
            tx_writer: self.tx_writer.clone(),
            tx_reader,
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let (ack, rx) = oneshot::channel();
        self.tx_writer
            .send_async(ActionWrite::Vote(ActionVote {
                value: serialize(vote).unwrap(),
                ack,
            }))
            .await
            .expect("Writer to always be running");

        rx.await.unwrap()?;
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        let (ack, rx) = oneshot::channel();

        self.tx_reader
            .send_async(ActionRead::Vote(ack))
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?;

        let vote = rx
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })??
            .map(|b| deserialize(&b).unwrap());

        Ok(vote)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<TypeConfigSqlite>,
    ) -> StorageResult<()>
    where
        I: IntoIterator<Item = Entry<TypeConfigSqlite>> + Send,
        I::IntoIter: Send,
    {
        let (tx, rx) = flume::bounded(1);
        let (ack, ack_rx) = oneshot::channel();

        self.tx_writer
            .send_async(ActionWrite::Append(ActionAppend { rx, callback, ack }))
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        for entry in entries {
            let id = id_to_bin(entry.log_id.index);
            let data = serialize(&entry).unwrap();

            tx.send_async(Some((id, data)))
                .await
                .map_err(|err| StorageIOError::write_logs(&err))?;
        }
        tx.send_async(None)
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        ack_rx
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn truncate(&mut self, log_id: LogId<NodeId>) -> StorageResult<()> {
        debug!("delete_log: [{:?}, +oo)", log_id);

        let from = id_to_bin(log_id.index);
        let until = id_to_bin(u64::MAX);

        let (ack, rx) = oneshot::channel();
        self.tx_writer
            .send_async(ActionWrite::Remove(ActionRemove {
                from,
                until,
                last_log: None,
                ack,
            }))
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?;

        rx.await.unwrap().map_err(|err| StorageError::IO {
            source: StorageIOError::read_vote(&err),
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        debug!("delete_log: [0, {:?}]", log_id);

        let from = id_to_bin(0);
        let until = id_to_bin(log_id.index + 1);
        let last_log = Some(serialize(&log_id).unwrap());

        let (ack, rx) = oneshot::channel();
        self.tx_writer
            .send_async(ActionWrite::Remove(ActionRemove {
                from,
                until,
                last_log,
                ack,
            }))
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?;

        rx.await.unwrap().map_err(|err| StorageError::IO {
            source: StorageIOError::read_vote(&err),
        })
    }
}
