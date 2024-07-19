use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::store::StorageResult;
use crate::NodeId;
use openraft::storage::LogFlushed;
use openraft::storage::LogState;
use openraft::storage::RaftLogStorage;
use openraft::LogId;
use openraft::OptionalSend;
use openraft::RaftLogReader;
use openraft::StorageError;
use openraft::StorageIOError;
use openraft::Vote;
use openraft::{AnyError, Entry, ErrorSubject, ErrorVerb};
use redb::{Database, Durability, ReadableTable, TableDefinition};
use serde_json::error::Category::Data;
use std::collections::Bound;
use std::fmt::Debug;
use std::ops::RangeBounds;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tokio::{fs, task, time};

// !!! CAUTION !!!
// TODO This impl is for internal testing only - still contains blocking code sections!

const TABLE_LOGS: TableDefinition<u64, Vec<u8>> = TableDefinition::new("logs");
const TABLE_STORE: TableDefinition<&str, Vec<u8>> = TableDefinition::new("store");

static IDX_COMMITTED: &str = "committed";
static IDX_LAST_PURGED: &str = "last_purged";
static IDX_VOTE: &str = "vote";

enum Action {
    Append(ActionAppend),
    Remove(ActionRemove),
    Metadata(ActionMetadata),
}

struct ActionAppend {
    rx: flume::Receiver<Option<(u64, Vec<u8>)>>,
    ack: oneshot::Sender<StorageResult<()>>,
}

struct ActionMetadata {
    idx: &'static str,
    data: Vec<u8>,
    ack: oneshot::Sender<StorageResult<()>>,
}

struct ActionRemove {
    from: u64,
    until: u64,
    ack: oneshot::Sender<StorageResult<()>>,
}

struct ActionRead {
    from: u64,
    until: u64,
    // ack: flume::Sender<Option<Result<Entry<TypeConfigSqliteSqlite>, StorageError<NodeId>>>>,
    ack: flume::Sender<Option<Result<Vec<u8>, StorageError<NodeId>>>>,
}

#[derive(Debug, Clone)]
pub struct LogStore {
    db: Arc<Database>,
    writer_tx: flume::Sender<Action>,
    reader_tx: flume::Sender<ActionRead>,
}

impl LogStore {
    pub async fn new(data_dir: &str) -> Self {
        let dir = format!("{}/logs", data_dir);
        let path = format!("{}/raft_logs.redb", dir);
        fs::create_dir_all(&dir)
            .await
            .expect("Cannot create logs path");

        // let db = match Database::open(&path) {
        //     Ok(db) => Arc::new(db),
        //     Err(_) => {
        //         let db = Database::create(path).expect("Cannot create database");
        //
        //         // write txn is necessary to create tables if missing
        //         let txn = db.begin_write().unwrap();
        //         {
        //             let mut table = txn.open_table(TABLE_STORE).unwrap();
        //             table.insert("init", Vec::default()).unwrap();
        //             table.remove("init").unwrap();
        //
        //             let mut table = txn.open_table(TABLE_LOGS).unwrap();
        //             table.insert(0, Vec::default()).unwrap();
        //             table.remove(0).unwrap();
        //         }
        //         txn.commit().unwrap();
        //
        //         Arc::new(db)
        //     }
        // };

        // Database::builder()
        //     .set_cache_size(10 * 1024 * 1024)

        let db = match Database::builder()
            .set_cache_size(64 * 1024 * 1024)
            .open(&path)
        {
            Ok(db) => Arc::new(db),
            Err(_) => {
                let db = Database::builder()
                    .set_cache_size(10 * 1024 * 1024)
                    .create(path)
                    .expect("Cannot create database");

                // write txn is necessary to create tables if missing
                let txn = db.begin_write().unwrap();
                {
                    let mut table = txn.open_table(TABLE_STORE).unwrap();
                    table.insert("init", Vec::default()).unwrap();
                    table.remove("init").unwrap();

                    let mut table = txn.open_table(TABLE_LOGS).unwrap();
                    table.insert(0, Vec::default()).unwrap();
                    table.remove(0).unwrap();
                }
                txn.commit().unwrap();

                Arc::new(db)
            }
        };

        let writer_tx = Self::spawn_writer(db.clone());
        let reader_tx = Self::spawn_reader(db.clone());

        Self {
            db,
            writer_tx,
            reader_tx,
        }
    }

    fn spawn_writer(db: Arc<Database>) -> flume::Sender<Action> {
        let (tx, rx) = flume::bounded::<Action>(2);

        // thread::spawn(move || {
        task::spawn_blocking(move || {
            // let mut app_start = Instant::now();
            // let mut working_time = 0;
            // let mut latency_median = 0;

            while let Ok(action) = rx.recv() {
                match action {
                    Action::Append(ActionAppend { rx, ack }) => {
                        // if working_time == 0 {
                        //     app_start = Instant::now();
                        // }
                        // let start = Instant::now();

                        let mut txn = match db.begin_write() {
                            Ok(txn) => txn,
                            Err(err) => {
                                ack.send(Err(StorageError::IO {
                                    source: StorageIOError::write_logs(&err),
                                }))
                                .unwrap();
                                continue;
                            }
                        };
                        // txn.set_durability(Durability::None);

                        // let mut count = 0;
                        let mut res = Ok(());
                        {
                            let mut table = match txn.open_table(TABLE_LOGS) {
                                Ok(table) => table,
                                Err(err) => {
                                    ack.send(Err(StorageError::IO {
                                        source: StorageIOError::write_logs(&err),
                                    }))
                                    .unwrap();
                                    continue;
                                }
                            };

                            // TODO after network optimization, check if chunked writing is faster than streaming
                            while let Ok(Some((index, data))) = rx.recv() {
                                if let Err(err) = table.insert(index, &data) {
                                    res = Err(StorageError::IO {
                                        source: StorageIOError::write_logs(&err),
                                        // TODO build the proper LogId in case of an error here
                                        // source: StorageIOError::write_log_entry(&err),
                                    });
                                    break;
                                }
                                // count += 1;
                            }
                        }

                        if res.is_ok() {
                            if let Err(err) = txn.commit() {
                                ack.send(Err(StorageError::IO {
                                    source: StorageIOError::write_logs(&err),
                                }))
                                .unwrap();
                                continue;
                            }
                        } else {
                            txn.abort().unwrap();
                        }
                        ack.send(res).unwrap();

                        // working_time += start.elapsed().as_millis() as u64;

                        // let total = app_start.elapsed().as_millis() as u64;
                        // let idle = total - working_time;
                        // println!(
                        //     "Total: {} ms / Working: {} ms / Idle: {} ms",
                        //     total, working_time, idle
                        // );
                        // latency_median += start.elapsed().as_millis() as u64;
                        // latency_median /= 2;
                        //
                        // if latency_median > 5 {
                        //     println!("\nAPPEND latency: {} ms\n", latency_median);
                        // }
                    }

                    Action::Metadata(ActionMetadata { idx, data, ack }) => {
                        let txn = match db.begin_write() {
                            Ok(txn) => txn,
                            Err(err) => {
                                ack.send(Err(StorageError::IO {
                                    source: StorageIOError::write_vote(&err),
                                }))
                                .unwrap();
                                continue;
                            }
                        };

                        {
                            let mut table = match txn.open_table(TABLE_STORE) {
                                Ok(table) => table,
                                Err(err) => {
                                    ack.send(Err(StorageError::IO {
                                        source: StorageIOError::write_vote(&err),
                                    }))
                                    .unwrap();
                                    continue;
                                }
                            };

                            if let Err(err) = table.insert(idx, data) {
                                ack.send(Err(StorageError::IO {
                                    source: StorageIOError::write_vote(&err),
                                }))
                                .unwrap();
                                continue;
                            };
                        }

                        match txn.commit() {
                            Ok(_) => ack.send(Ok(())).unwrap(),
                            Err(err) => {
                                ack.send(Err(StorageError::IO {
                                    source: StorageIOError::write_vote(&err),
                                }))
                                .unwrap();
                            }
                        }
                    }

                    // TODO maybe split this to return detailed errors just in case
                    Action::Remove(ActionRemove { from, until, ack }) => {
                        let txn = match db.begin_write() {
                            Ok(txn) => txn,
                            Err(err) => {
                                ack.send(Err(StorageError::IO {
                                    source: StorageIOError::write_logs(&err),
                                }))
                                .unwrap();
                                continue;
                            }
                        };

                        let mut res = Ok(());
                        {
                            let mut table = match txn.open_table(TABLE_LOGS) {
                                Ok(table) => table,
                                Err(err) => {
                                    ack.send(Err(StorageError::IO {
                                        source: StorageIOError::write_logs(&err),
                                    }))
                                    .unwrap();
                                    continue;
                                }
                            };

                            for i in from..until {
                                match table.remove(i) {
                                    Ok(res) => {
                                        if res.is_none() {
                                            break;
                                        }
                                    }
                                    Err(err) => {
                                        res = Err(StorageError::IO {
                                            source: StorageIOError::write_logs(&err),
                                        });
                                        break;
                                    }
                                }
                            }
                        }

                        if res.is_ok() {
                            if let Err(err) = txn.commit() {
                                ack.send(Err(StorageError::IO {
                                    source: StorageIOError::write_logs(&err),
                                }))
                                .unwrap();
                                continue;
                            }
                        } else {
                            txn.abort().unwrap();
                        }
                        ack.send(res).unwrap();
                    }
                }
            }
        });

        tx
    }

    fn spawn_reader(db: Arc<Database>) -> flume::Sender<ActionRead> {
        let (tx, rx) = flume::bounded::<ActionRead>(2);

        task::spawn_blocking(move || {
            while let Ok(ActionRead { from, until, ack }) = rx.recv() {
                let txn = match db.begin_read() {
                    Ok(txn) => txn,
                    Err(err) => {
                        ack.send(Some(Err(StorageError::IO {
                            source: StorageIOError::read_logs(&err),
                        })))
                        .unwrap();
                        continue;
                    }
                };
                let table = match txn.open_table(TABLE_LOGS) {
                    Ok(table) => table,
                    Err(err) => {
                        ack.send(Some(Err(StorageError::IO {
                            source: StorageIOError::read_logs(&err),
                        })))
                        .unwrap();
                        continue;
                    }
                };

                // // TODO performance could possibly greatly improved with ranged / batched reading
                // for i in from..=until {
                //     match table.get(i) {
                //         Ok(entry) => match entry {
                //             None => {
                //                 ack.send(Some(Err(StorageError::IO {
                //                     source: StorageIOError::read_logs(AnyError::error(format!(
                //                         "no log id {}",
                //                         i
                //                     ))),
                //                 })))
                //                 .unwrap();
                //                 continue;
                //             }
                //             Some(entry) => {
                //                 let bytes = entry.value();
                //                 // let entry: Entry<_> = bincode::deserialize(&bytes).unwrap();
                //                 ack.send(Some(Ok(bytes))).unwrap();
                //             }
                //         },
                //         Err(err) => {
                //             ack.send(Some(Err(StorageError::IO {
                //                 source: StorageIOError::read_logs(&err),
                //             })))
                //             .unwrap();
                //             continue;
                //         }
                //     }
                // }
                //
                // ack.send(None).unwrap();

                match table.range(from..=until) {
                    Ok(res) => {
                        for entry in res {
                            match entry {
                                Ok((_key, value)) => {
                                    let bytes = value.value();
                                    ack.send(Some(Ok(bytes))).unwrap();
                                }
                                Err(err) => {
                                    ack.send(Some(Err(StorageError::IO {
                                        source: StorageIOError::read_logs(&err),
                                    })))
                                    .unwrap();
                                    break;
                                }
                            };
                        }
                        ack.send(None).unwrap();
                    }
                    Err(err) => {
                        ack.send(Some(Err(StorageError::IO {
                            source: StorageIOError::read_logs(&err),
                        })))
                        .unwrap();
                    }
                }
            }
        });

        tx
    }

    fn get_last_purged_(&self) -> StorageResult<Option<LogId<u64>>> {
        let txn = self.db.begin_read().map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;
        let table = txn
            .open_table(TABLE_STORE)
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;
        match table.get(IDX_LAST_PURGED) {
            Ok(Some(value)) => {
                let res: LogId<u64> = bincode::deserialize(&value.value()).unwrap();
                Ok(Some(res))
            }
            Ok(None) => Ok(None),
            Err(err) => Err(StorageError::IO {
                source: StorageIOError::read_logs(&err),
            }),
        }
    }

    fn set_last_purged_(&self, log_id: LogId<u64>) -> StorageResult<()> {
        let data = bincode::serialize(&log_id).unwrap();
        let txn = self.db.begin_write().map_err(|err| StorageError::IO {
            source: StorageIOError::write_logs(&err),
        })?;
        {
            let mut table = txn
                .open_table(TABLE_STORE)
                .map_err(|err| StorageError::IO {
                    source: StorageIOError::write_logs(&err),
                })?;
            table
                .insert("last_purged_id", data)
                .map_err(|err| StorageError::IO {
                    source: StorageIOError::write_logs(&err),
                })?;
        }

        txn.commit().map_err(|e| StorageError::IO {
            source: StorageIOError::write_logs(&e),
        })?;
        Ok(())
    }
}

impl RaftLogReader<TypeConfigSqlite> for LogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> StorageResult<Vec<Entry<TypeConfigSqlite>>> {
        let start = match range.start_bound() {
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i - 1,
            Bound::Unbounded => panic!("open end log entries get"),
        };
        if end < start {
            return Ok(Vec::default());
        }

        let (ack, rx) = flume::unbounded();
        self.reader_tx
            .send_async(ActionRead {
                from: start,
                until: end,
                ack,
            })
            .await
            .expect("LogReader to always be listening");

        let mut res = Vec::with_capacity((end + 1 - start) as usize);
        while let Some(payload) = rx.recv_async().await.unwrap() {
            let bytes = payload?;
            let entry: Entry<_> = bincode::deserialize(&bytes).unwrap();
            res.push(entry);
        }

        Ok(res)
    }
}

impl RaftLogStorage<TypeConfigSqlite> for LogStore {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> StorageResult<LogState<TypeConfigSqlite>> {
        let last_purged_log_id = self.get_last_purged_()?;

        let txn = self.db.begin_read().map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;
        let table = txn.open_table(TABLE_LOGS).map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;

        let last = table.last().map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;
        let last_log_id = match last {
            None => last_purged_log_id,
            Some(last) => {
                let bytes = last.1.value();
                let log_id = bincode::deserialize(&bytes).unwrap();
                Some(log_id)
            }
        };

        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogId<NodeId>>,
    ) -> Result<(), StorageError<NodeId>> {
        let data = bincode::serialize(&committed).unwrap();

        let (ack, rx) = oneshot::channel();
        self.writer_tx
            .send_async(Action::Metadata(ActionMetadata {
                idx: IDX_COMMITTED,
                data,
                ack,
            }))
            .await
            .expect("Logs writer to always be running");

        // TODO we could have an additional watch channel or Atomic to make reads in-memory and a lot faster

        rx.await.expect("To always get an answer from logs writer")
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
        let txn = self.db.begin_read().map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;
        let table = txn
            .open_table(TABLE_STORE)
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;
        let value = table.get("committed").map_err(|err| StorageError::IO {
            source: StorageIOError::read(&err),
        })?;

        if let Some(guard) = value {
            let bytes = guard.value();
            let data: Option<LogId<NodeId>> = bincode::deserialize(&bytes).unwrap();
            Ok(data)
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let data = bincode::serialize(vote).unwrap();

        let (ack, rx) = oneshot::channel();
        self.writer_tx
            .send_async(Action::Metadata(ActionMetadata {
                idx: IDX_VOTE,
                data,
                ack,
            }))
            .await
            .expect("Logs writer to always be running");

        rx.await.expect("To always get an answer from logs writer")
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        let txn = self.db.begin_read().map_err(|err| StorageError::IO {
            source: StorageIOError::read_vote(&err),
        })?;
        let table = txn
            .open_table(TABLE_STORE)
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?;

        let value = table.get("vote").map_err(|err| StorageError::IO {
            source: StorageIOError::read_vote(&err),
        })?;

        if let Some(guard) = value {
            let bytes = guard.value();
            let data: Vote<NodeId> = bincode::deserialize(&bytes).unwrap();
            Ok(Some(data))
        } else {
            Ok(None)
        }
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
        let (tx, rx) = flume::bounded(2);
        let (ack_tx, ack_rx) = oneshot::channel();

        self.writer_tx
            .send_async(Action::Append(ActionAppend { rx, ack: ack_tx }))
            .await
            .expect("The appender to always be running");

        for entry in entries {
            let data = bincode::serialize(&entry).unwrap();
            tx.send_async(Some((entry.log_id.index, data)))
                .await
                .unwrap();
        }
        tx.send_async(None).await.unwrap();

        let res = ack_rx
            .await
            .expect("To always receive an answer from the appender");

        if res.is_ok() {
            callback.log_io_completed(Ok(()));
        }

        res
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn truncate(&mut self, log_id: LogId<NodeId>) -> StorageResult<()> {
        tracing::debug!("delete_log: [{:?}, +oo)", log_id);

        let (ack, rx) = oneshot::channel();
        self.writer_tx
            .send_async(Action::Remove(ActionRemove {
                from: log_id.index,
                until: u64::MAX,
                ack,
            }))
            .await
            .expect("Logs writer to always be running");
        rx.await
            .expect("To always receive an answer from logs writer")
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        tracing::debug!("delete_log: [0, {:?}]", log_id);

        let (ack, rx) = oneshot::channel();
        self.writer_tx
            .send_async(Action::Remove(ActionRemove {
                from: 0,
                until: log_id.index + 1,
                ack,
            }))
            .await
            .expect("Logs writer to always be running");
        rx.await
            .expect("To always receive an answer from logs writer")?;

        let (ack, rx) = oneshot::channel();
        let data = bincode::serialize(&log_id).unwrap();
        self.writer_tx
            .send_async(Action::Metadata(ActionMetadata {
                idx: IDX_LAST_PURGED,
                ack,
                data,
            }))
            .await
            .expect("Logs writer to always be running");
        rx.await
            .expect("To always receive an answer from logs writer")
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        let reader_tx = Self::spawn_reader(self.db.clone());
        Self {
            db: self.db.clone(),
            writer_tx: self.writer_tx.clone(),
            reader_tx,
        }
    }
}
