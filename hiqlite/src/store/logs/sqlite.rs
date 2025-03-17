// use crate::store::state_machine::sqlite::TypeConfigSqlite;
// use crate::store::{connect_sqlite, StorageResult};
// use crate::{Error, NodeId};
// use openraft::storage::LogFlushed;
// use openraft::storage::LogState;
// use openraft::storage::RaftLogStorage;
// use openraft::Entry;
// use openraft::LogId;
// use openraft::OptionalSend;
// use openraft::RaftLogReader;
// use openraft::StorageError;
// use openraft::StorageIOError;
// use openraft::Vote;
// use rusqlite::{PrepFlags, ToSql, Transaction, TransactionBehavior};
// use std::collections::Bound;
// use std::fmt::Debug;
// use std::fmt::Write;
// use std::ops::RangeBounds;
// use std::sync::Arc;
// use tokio::sync::oneshot;
// use tokio::time::Instant;
// use tokio::{fs, task};
// use tracing::error;
//
// // TODO maybe even remove this as soon as redb + async commit is implemented to have
// // at least a fallback for non-gnu builds (rocksdb does not compile to musl)
//
// static IDX_COMMITTED: &str = "committed";
// static IDX_LAST_PURGED: &str = "last_purged";
// static IDX_VOTE: &str = "vote";
//
// enum ActionWrite {
//     Append(Append),
//     Truncate(ActionTrunce),
//     Purge(ActionPurge),
//     Metadata(WriteMeta),
// }
//
// struct Append {
//     rx: flume::Receiver<Option<(u64, Vec<u8>)>>,
//     ack: oneshot::Sender<StorageResult<()>>,
// }
//
// struct WriteMeta {
//     idx: &'static str,
//     data: Vec<u8>,
//     ack: oneshot::Sender<StorageResult<()>>,
// }
//
// struct ActionTrunce {
//     from: u64,
//     ack: oneshot::Sender<StorageResult<()>>,
// }
//
// struct ActionPurge {
//     log_id: LogId<NodeId>,
//     ack: oneshot::Sender<StorageResult<()>>,
// }
//
// enum ActionRead {
//     Logs(ReadLogs),
//     LogState(ReadLogState),
//     Meta(ReadMeta),
// }
//
// struct ReadLogs {
//     from: u64,
//     until: u64,
//     // ack: flume::Sender<Option<Result<Entry<TypeConfigSqliteSqlite>, StorageError<NodeId>>>>,
//     ack: flume::Sender<Option<Result<Vec<u8>, StorageError<NodeId>>>>,
// }
//
// struct ReadLogState {
//     ack: oneshot::Sender<LogStateResponse>,
// }
//
// #[derive(Debug)]
// struct LogStateResponse {
//     last_log: Option<Vec<u8>>,
//     last_purged: Vec<u8>,
// }
//
// struct ReadMeta {
//     idx: &'static str,
//     // ack: flume::Sender<Option<Result<Entry<TypeConfigSqlite>, StorageError<NodeId>>>>,
//     ack: oneshot::Sender<Result<Vec<u8>, StorageError<NodeId>>>,
// }
//
// #[derive(Debug, Clone)]
// pub struct LogStore {
//     db_path: Option<String>,
//     writer_tx: flume::Sender<ActionWrite>,
//     reader_tx: flume::Sender<ActionRead>,
// }
//
// impl LogStore {
//     pub async fn new(data_dir: &str, filename_db: Option<&str>) -> Result<Self, Error> {
//         let dir = format!("{}/logs", data_dir);
//         fs::create_dir_all(&dir).await?;
//         fn_access(&dir, 0o600).await?;
//         let db_path = filename_db.map(|name| format!("{}/{}", dir, name));
//
//         let conn = connect_sqlite(db_path.as_deref(), false)?;
//         let conn_txn = connect_sqlite(db_path.as_deref(), false)?;
//
//         conn.execute_batch(
//             r#"
//             CREATE TABLE IF NOT EXISTS logs
//             (
//                 id   INTEGER NOT NULL
//                     CONSTRAINT logs_pk
//                         PRIMARY KEY,
//                 data BLOB    NOT NULL
//             );
//
//             CREATE TABLE IF NOT EXISTS metadata
//             (
//                 key   TEXT    NOT NULL
//                     CONSTRAINT metadata_pk
//                         PRIMARY KEY,
//                 value BLOB    NOT NULL
//             );
//             "#,
//         )
//         .expect("logs table creation to succeed");
//
//         let writer_tx = Self::spawn_writer(conn, conn_txn);
//
//         let conn = connect_sqlite(db_path.as_deref(), true)?;
//         let reader_tx = Self::spawn_reader(conn);
//
//         Ok(Self {
//             db_path,
//             writer_tx,
//             reader_tx,
//         })
//     }
//
//     fn spawn_writer(
//         mut conn: rusqlite::Connection,
//         mut conn_txn: rusqlite::Connection,
//     ) -> flume::Sender<ActionWrite> {
//         let (tx, rx) = flume::bounded::<ActionWrite>(2);
//
//         thread::spawn(move || {
//             // task::spawn_blocking(move || {
//             let mut append = conn
//                 .prepare_cached("INSERT INTO logs (id, data) VALUES ($1, $2)")
//                 .expect("Prepare statement to succeed");
//
//             let mut remove_until = conn
//                 .prepare_cached("DELETE FROM logs WHERE id <= $1")
//                 .expect("Prepare statement to succeed");
//
//             let mut remove_from = conn
//                 .prepare_cached("DELETE FROM logs WHERE id >= $1")
//                 .expect("Prepare statement to succeed");
//
//             let mut meta = conn
//                 .prepare_cached(
//                     r#"INSERT INTO metadata (key, value) VALUES ($1, $2)
//                     ON CONFLICT(key) DO UPDATE SET value = $2"#,
//                 )
//                 .expect("Prepare statement to succeed");
//
//             // make sure the metadata has at least empty values
//             {
//                 let mut stmt = conn
//                     .prepare(
//                         r#"INSERT INTO metadata (key, value) VALUES ($1, $2)
//                             ON CONFLICT(key) DO NOTHING"#,
//                     )
//                     .expect("Prepare statement to succeed");
//                 let meta_init = serialize(&None::<Option<Vec<u8>>>).unwrap();
//
//                 for key in [IDX_COMMITTED, IDX_LAST_PURGED, IDX_VOTE] {
//                     stmt.execute((key, &meta_init))
//                         .expect("Meta init to succeed");
//                 }
//             }
//
//             while let Ok(action) = rx.recv() {
//                 match action {
//                     ActionWrite::Append(Append { rx, ack }) => {
//                         let txn = conn_txn.transaction().unwrap();
//
//                         let mut error = None;
//                         while let Ok(Some((index, data))) = rx.recv() {
//                             let mut stmt = txn
//                                 .prepare_cached("INSERT INTO logs (id, data) VALUES ($1, $2)")
//                                 .unwrap();
//
//                             if let Err(err) = stmt.execute((index, data)) {
//                                 error = Some(Err(StorageError::IO {
//                                     source: StorageIOError::write_logs(&err),
//                                     // TODO build the proper LogId in case of an error here
//                                     // source: StorageIOError::write_log_entry(&err),
//                                 }));
//                                 break;
//                             }
//                         }
//
//                         if let Some(err) = error {
//                             ack.send(err).unwrap();
//                         } else if let Err(err) = txn.commit() {
//                             ack.send(Err(StorageError::IO {
//                                 source: StorageIOError::write_logs(&err),
//                             }))
//                             .unwrap()
//                         } else {
//                             ack.send(Ok(())).unwrap();
//                         }
//                     }
//
//                     // batching
//                     // ActionWrite::Append(Append { rx, ack }) => {
//                     //     let mut query = String::with_capacity(256);
//                     //     write!(query, "INSERT INTO logs (id, data) VALUES").unwrap();
//                     //
//                     //     while let Ok(Some((index, data))) = rx.recv() {
//                     //         let bytes = hex::encode(&data);
//                     //         // println!("\n\nbytes data default:\n{:?}\n", data);
//                     //         //
//                     //         // let mut bytes = String::with_capacity(data.len());
//                     //         // data.into_iter()
//                     //         //     .for_each(|v| write!(bytes, "{}", v).unwrap());
//                     //
//                     //         // println!("\n\nbytes data formatted:\n{}\n", bytes);
//                     //
//                     //         query.push_str(&format!("\n({}, x'{}'),", index, bytes));
//                     //     }
//                     //     query.insert(query.len() - 1, ';');
//                     //     // query.remove(query.len() - 1);
//                     //     // query.push(';');
//                     //
//                     //     match conn_txn.execute(&query, ()) {
//                     //         // match conn_txn.execute_batch(&query) {
//                     //         Ok(_) => ack.send(Ok(())).unwrap(),
//                     //         Err(err) => {
//                     //             ack.send(Err(StorageError::IO {
//                     //                 source: StorageIOError::write_logs(&err),
//                     //                 // TODO build the proper LogId in case of an error here
//                     //                 // source: StorageIOError::write_log_entry(&err),
//                     //             }))
//                     //             .unwrap();
//                     //         }
//                     //     }
//                     // }
//                     ActionWrite::Metadata(WriteMeta { idx, data, ack }) => {
//                         match meta.execute((idx, data)) {
//                             Ok(_) => ack.send(Ok(())).unwrap(),
//                             Err(err) => {
//                                 ack.send(Err(StorageError::IO {
//                                     source: StorageIOError::write_vote(&err),
//                                 }))
//                                 .unwrap();
//                             }
//                         }
//                     }
//
//                     ActionWrite::Truncate(ActionTrunce { from, ack }) => {
//                         match remove_from.execute([from]) {
//                             Ok(_) => {
//                                 ack.send(Ok(())).unwrap();
//                             }
//                             Err(err) => {
//                                 ack.send(Err(StorageError::IO {
//                                     source: StorageIOError::write_logs(&err),
//                                 }))
//                                 .unwrap();
//                             }
//                         }
//                     }
//
//                     ActionWrite::Purge(ActionPurge { log_id, ack }) => {
//                         if let Err(err) = remove_until.execute([log_id.index]) {
//                             ack.send(Err(StorageError::IO {
//                                 source: StorageIOError::write_logs(&err),
//                             }))
//                             .unwrap();
//                             continue;
//                         }
//
//                         let data = serialize(&log_id).unwrap();
//                         match meta.execute(((IDX_LAST_PURGED), data)) {
//                             Ok(_) => ack.send(Ok(())).unwrap(),
//                             Err(err) => {
//                                 ack.send(Err(StorageError::IO {
//                                     source: StorageIOError::write_vote(&err),
//                                 }))
//                                 .unwrap();
//                             }
//                         }
//                     }
//                 }
//             }
//         });
//
//         tx
//     }
//
//     fn spawn_reader(conn: rusqlite::Connection) -> flume::Sender<ActionRead> {
//         let (tx, rx) = flume::bounded::<ActionRead>(2);
//
//         std::thread::spawn(move || {
//             // task::spawn_blocking(move || {
//             let mut read_logs = conn
//                 .prepare_cached("SELECT data FROM logs WHERE id >= $1 AND id <= $2")
//                 .expect("Statement prepare to succeed");
//
//             let mut read_last_log = conn
//                 .prepare_cached("SELECT data FROM logs ORDER BY id DESC LIMIT 1")
//                 .expect("Statement prepare to succeed");
//
//             let mut read_meta = conn
//                 .prepare_cached("SELECT value FROM metadata WHERE key = $1")
//                 .expect("Prepare statement to succeed");
//
//             while let Ok(action) = rx.recv() {
//                 match action {
//                     ActionRead::Logs(ReadLogs { from, until, ack }) => {
//                         match read_logs.query((from, until)) {
//                             Ok(mut rows) => {
//                                 while let Ok(Some(row)) = rows.next() {
//                                     let data = row.get(0).unwrap();
//                                     ack.send(Some(Ok(data))).unwrap();
//                                 }
//                             }
//                             Err(err) => {
//                                 let msg = err.to_string();
//                                 if !msg.contains("returned no rows") {
//                                     ack.send(Some(Err(StorageError::IO {
//                                         source: StorageIOError::read_logs(&err),
//                                     })))
//                                     .unwrap();
//                                     continue;
//                                 }
//                             }
//                         }
//
//                         // if let Ok(mut rows) = read_logs.query((from, until)) {
//                         //     while let Ok(Some(row)) = rows.next() {
//                         //         let data = row.get(0).unwrap();
//                         //         ack.send(Some(Ok(data))).unwrap();
//                         //     }
//                         // }
//
//                         ack.send(None).unwrap();
//                     }
//
//                     ActionRead::LogState(ReadLogState { ack }) => {
//                         let last_log = read_last_log
//                             .query_row((), |row| {
//                                 let data: Vec<u8> = row.get(0)?;
//                                 Ok(data)
//                             })
//                             .ok();
//
//                         let last_purged = read_meta
//                             .query_row([IDX_LAST_PURGED], |row| {
//                                 let bytes: Vec<u8> = row.get(0)?;
//                                 Ok(bytes)
//                             })
//                             // TODO check error
//                             .unwrap();
//
//                         ack.send(LogStateResponse {
//                             last_log,
//                             last_purged,
//                         })
//                         .unwrap()
//                     }
//
//                     ActionRead::Meta(ReadMeta { idx: key, ack }) => {
//                         #[allow(clippy::blocks_in_conditions)]
//                         match read_meta.query_row([key], |row| {
//                             let value: Vec<u8> = row.get(0)?;
//                             Ok(value)
//                         }) {
//                             Ok(value) => {
//                                 ack.send(Ok(value)).unwrap();
//                             }
//                             Err(err) => {
//                                 ack.send(Err(StorageError::IO {
//                                     source: StorageIOError::read_logs(&err),
//                                 }))
//                                 .unwrap();
//                             }
//                         }
//                     }
//                 }
//             }
//         });
//
//         tx
//     }
// }
//
// impl RaftLogReader<TypeConfigSqlite> for LogStore {
//     async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
//         &mut self,
//         range: RB,
//     ) -> StorageResult<Vec<Entry<TypeConfigSqlite>>> {
//         let start = match range.start_bound() {
//             Bound::Included(i) => *i,
//             Bound::Excluded(i) => *i + 1,
//             Bound::Unbounded => 0,
//         };
//         let end = match range.end_bound() {
//             Bound::Included(i) => *i,
//             Bound::Excluded(i) => *i - 1,
//             Bound::Unbounded => panic!("open end log entries get"),
//         };
//         if end < start {
//             return Ok(Vec::default());
//         }
//
//         let (ack, rx) = flume::unbounded();
//         self.reader_tx
//             .send_async(ActionRead::Logs(ReadLogs {
//                 from: start,
//                 until: end,
//                 ack,
//             }))
//             .await
//             .expect("LogReader to always be listening");
//
//         let mut res = Vec::with_capacity((end + 1 - start) as usize);
//         while let Some(payload) = rx.recv_async().await.unwrap() {
//             let bytes = payload?;
//             let entry: Entry<_> = deserialize(&bytes).unwrap();
//             res.push(entry);
//         }
//
//         Ok(res)
//     }
// }
//
// impl RaftLogStorage<TypeConfigSqlite> for LogStore {
//     type LogReader = Self;
//
//     async fn get_log_state(&mut self) -> StorageResult<LogState<TypeConfigSqlite>> {
//         let (ack, rx) = oneshot::channel();
//         self.reader_tx
//             .send_async(ActionRead::LogState(ReadLogState { ack }))
//             .await
//             .expect("Logs reader to always be listening");
//         let res = rx
//             .await
//             .expect("To always receive an answer from Logs reader");
//
//         // let last_log_id =  bincode::deserialize(&res.last_log).unwrap();
//         let last_log_id = res.last_log.map(|id| deserialize(&id).unwrap());
//         let last_purged_log_id = deserialize(&res.last_purged).unwrap();
//
//         Ok(LogState {
//             last_purged_log_id,
//             last_log_id,
//         })
//     }
//
//     async fn save_committed(
//         &mut self,
//         committed: Option<LogId<NodeId>>,
//     ) -> Result<(), StorageError<NodeId>> {
//         let data = serialize(&committed).unwrap();
//
//         let (ack, rx) = oneshot::channel();
//         self.writer_tx
//             .send_async(ActionWrite::Metadata(WriteMeta {
//                 idx: IDX_COMMITTED,
//                 data,
//                 ack,
//             }))
//             .await
//             .expect("Logs writer to always be running");
//
//         // TODO we could have an additional watch channel or Atomic to make reads in-memory and a lot faster
//
//         rx.await.expect("To always get an answer from logs writer")
//     }
//
//     async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
//         let (ack, rx) = oneshot::channel::<Result<Vec<u8>, StorageError<NodeId>>>();
//         self.reader_tx
//             .send_async(ActionRead::Meta(ReadMeta {
//                 idx: IDX_COMMITTED,
//                 ack,
//             }))
//             .await
//             .expect("Logs reader to always be running");
//
//         let bytes = rx.await.unwrap()?;
//         let data: Option<LogId<NodeId>> = deserialize(&bytes).unwrap();
//         Ok(data)
//     }
//
//     #[tracing::instrument(level = "trace", skip(self))]
//     async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
//         let data = serialize(&Some(vote)).unwrap();
//
//         let (ack, rx) = oneshot::channel();
//         self.writer_tx
//             .send_async(ActionWrite::Metadata(WriteMeta {
//                 idx: IDX_VOTE,
//                 data,
//                 ack,
//             }))
//             .await
//             .expect("Logs writer to always be running");
//
//         rx.await.expect("To always get an answer from logs writer")
//     }
//
//     async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
//         let (ack, rx) = oneshot::channel::<Result<Vec<u8>, StorageError<NodeId>>>();
//         self.reader_tx
//             .send_async(ActionRead::Meta(ReadMeta { idx: IDX_VOTE, ack }))
//             .await
//             .expect("Logs reader to always be running");
//
//         let bytes = rx.await.unwrap()?;
//         // {
//         //     None | Some(Err(_)) => Ok(None),
//         //     Some(Ok(bytes)) => {
//         let data: Option<Vote<NodeId>> = deserialize(&bytes).unwrap();
//         Ok(data)
//         // }
//         // }
//     }
//
//     #[tracing::instrument(level = "trace", skip_all)]
//     async fn append<I>(
//         &mut self,
//         entries: I,
//         callback: LogFlushed<TypeConfigSqlite>,
//     ) -> StorageResult<()>
//     where
//         I: IntoIterator<Item = Entry<TypeConfigSqlite>> + Send,
//         I::IntoIter: Send,
//     {
//         let (tx, rx) = flume::bounded(2);
//         let (ack_tx, ack_rx) = oneshot::channel();
//
//         self.writer_tx
//             .send_async(ActionWrite::Append(Append { rx, ack: ack_tx }))
//             .await
//             .expect("The appender to always be running");
//
//         for entry in entries {
//             let data = serialize(&entry).unwrap();
//             tx.send_async(Some((entry.log_id.index, data)))
//                 .await
//                 .unwrap();
//         }
//         tx.send_async(None).await.unwrap();
//
//         let res = ack_rx
//             .await
//             .expect("To always receive an answer from the appender");
//
//         if res.is_ok() {
//             callback.log_io_completed(Ok(()));
//         }
//
//         res
//     }
//
//     #[tracing::instrument(level = "debug", skip(self))]
//     async fn truncate(&mut self, log_id: LogId<NodeId>) -> StorageResult<()> {
//         tracing::debug!("delete_log: [{:?}, +oo)", log_id);
//
//         let (ack, rx) = oneshot::channel();
//         self.writer_tx
//             .send_async(ActionWrite::Truncate(ActionTrunce {
//                 from: log_id.index,
//                 ack,
//             }))
//             .await
//             .expect("Logs writer to always be running");
//         rx.await
//             .expect("To always receive an answer from logs writer")
//     }
//
//     #[tracing::instrument(level = "debug", skip(self))]
//     async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
//         tracing::debug!("delete_log: [0, {:?}]", log_id);
//
//         let (ack, rx) = oneshot::channel();
//         self.writer_tx
//             .send_async(ActionWrite::Purge(ActionPurge { log_id, ack }))
//             .await
//             .expect("Logs writer to always be running");
//         rx.await
//             .expect("To always receive an answer from logs writer")
//     }
//
//     async fn get_log_reader(&mut self) -> Self::LogReader {
//         let conn = connect_sqlite(self.db_path.as_deref(), true)
//             .expect("Additional read connection to open just fine");
//         let reader_tx = Self::spawn_reader(conn);
//         Self {
//             db_path: self.db_path.clone(),
//             writer_tx: self.writer_tx.clone(),
//             reader_tx,
//         }
//     }
// }
