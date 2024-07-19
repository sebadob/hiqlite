// use crate::store::StorageResult;
// use crate::{typ, typ::SnapshotData, Node, NodeId, TypeConfigSqlite};
// use openraft::storage::RaftStateMachine;
// use openraft::{
//     EntryPayload, LogId, OptionalSend, RaftSnapshotBuilder, Snapshot, SnapshotMeta, StorageError,
//     StorageIOError, StoredMembership,
// };
// use redb::{Database, TableDefinition};
// use serde::{Deserialize, Serialize};
// use std::borrow::Cow;
// use std::collections::BTreeMap;
// use std::io::Cursor;
// use std::sync::Arc;
// use tokio::fs;
// use tokio::sync::RwLock;
// use uuid::Uuid;
//
// const TABLE_SNAPSHOT: TableDefinition<&str, Vec<u8>> = TableDefinition::new("snapshots_memory");
//
// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub enum Request {
//     Put {
//         key: Cow<'static, str>,
//         value: Cow<'static, str>,
//     },
//     Delete {
//         key: Cow<'static, str>,
//     },
// }
//
// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct Response {
//     pub value: Option<Cow<'static, str>>,
// }
//
// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct StoredSnapshot {
//     pub meta: SnapshotMeta<NodeId, Node>,
//
//     /// The data of the state machine at the time of this snapshot.
//     pub data: Vec<u8>,
// }
//
// #[derive(Debug, Clone)]
// pub struct StateMachineMemory {
//     pub data: StateMachineData,
//
//     /// snapshot index is not persisted in this example.
//     ///
//     /// It is only used as a suffix of snapshot id, and should be globally unique.
//     /// In practice, using a timestamp in micro-second would be good enough.
//     snapshot_idx: Uuid,
//
//     /// State machine stores snapshot in db.
//     db: Arc<Database>,
// }
//
// #[derive(Debug, Clone)]
// pub struct StateMachineData {
//     pub last_applied_log_id: Option<LogId<NodeId>>,
//
//     pub last_membership: StoredMembership<NodeId, Node>,
//
//     /// State built from applying the raft logs
//     /// TODO should be converted to a concurrent HashMap if we keep this
//     pub kvs: Arc<RwLock<BTreeMap<String, String>>>,
// }
//
// impl RaftSnapshotBuilder<TypeConfigSqlite> for StateMachineMemory {
//     async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigSqlite>, StorageError<NodeId>> {
//         let (last_log_id, last_membership, kv_bytes) = {
//             let kvs = self.data.kvs.read().await;
//             let kv_bytes = bincode::serialize(&*kvs)
//                 .map_err(|err| StorageIOError::read_state_machine(&err))?;
//
//             let last_applied_log = self.data.last_applied_log_id;
//             let last_membership = self.data.last_membership.clone();
//
//             (last_applied_log, last_membership, kv_bytes)
//         };
//
//         let snapshot_id = if let Some(last) = last_log_id {
//             format!("{}-{}-{}", self.snapshot_idx, last.leader_id, last.index)
//         } else {
//             format!("{}", self.snapshot_idx)
//         };
//
//         let meta = SnapshotMeta {
//             last_log_id,
//             last_membership,
//             snapshot_id,
//         };
//
//         self.persist_snapshot(StoredSnapshot {
//             meta: meta.clone(),
//             data: kv_bytes.clone(),
//         })?;
//
//         Ok(Snapshot {
//             meta,
//             snapshot: Box::new(Cursor::new(kv_bytes)),
//         })
//     }
// }
//
// impl StateMachineMemory {
//     pub(crate) async fn new(db: Arc<Database>) -> Result<StateMachineMemory, StorageError<NodeId>> {
//         let mut sm = Self {
//             data: StateMachineData {
//                 last_applied_log_id: None,
//                 last_membership: Default::default(),
//                 kvs: Arc::new(Default::default()),
//             },
//             snapshot_idx: Uuid::default(),
//             db,
//         };
//
//         if let Some(snapshot) = sm.get_current_snapshot_()? {
//             sm.update_state_machine_(snapshot).await?;
//         }
//
//         Ok(sm)
//     }
//
//     async fn update_state_machine_(
//         &mut self,
//         snapshot: StoredSnapshot,
//     ) -> Result<(), StorageError<NodeId>> {
//         let kvs: BTreeMap<String, String> = bincode::deserialize(&snapshot.data)
//             .map_err(|e| StorageIOError::read_snapshot(Some(snapshot.meta.signature()), &e))?;
//
//         self.data.last_applied_log_id = snapshot.meta.last_log_id;
//         self.data.last_membership = snapshot.meta.last_membership.clone();
//         let mut lock = self.data.kvs.write().await;
//         *lock = kvs;
//
//         Ok(())
//     }
//
//     fn get_current_snapshot_(&self) -> StorageResult<Option<StoredSnapshot>> {
//         let txn = self.db.begin_read().map_err(|err| StorageError::IO {
//             source: StorageIOError::read(&err),
//         })?;
//         let table = txn
//             .open_table(TABLE_SNAPSHOT)
//             .map_err(|err| StorageError::IO {
//                 source: StorageIOError::read_snapshot(None, &err),
//             })?;
//         let value = table.get("snapshot").map_err(|err| StorageError::IO {
//             source: StorageIOError::read_snapshot(None, &err),
//         })?;
//
//         if let Some(guard) = value {
//             let data: StoredSnapshot = bincode::deserialize(&guard.value()).unwrap();
//             Ok(Some(data))
//         } else {
//             Ok(None)
//         }
//     }
//
//     fn persist_snapshot(&self, snapshot: StoredSnapshot) -> StorageResult<()> {
//         let data = bincode::serialize(&snapshot).unwrap();
//
//         let txn = self.db.begin_write().map_err(|err| StorageError::IO {
//             source: StorageIOError::write(&err),
//         })?;
//
//         {
//             let mut table = txn
//                 .open_table(TABLE_SNAPSHOT)
//                 .map_err(|e| StorageError::IO {
//                     source: StorageIOError::write_snapshot(Some(snapshot.meta.signature()), &e),
//                 })?;
//             table
//                 .insert("snapshot", data)
//                 .map_err(|e| StorageError::IO {
//                     source: StorageIOError::write_snapshot(Some(snapshot.meta.signature()), &e),
//                 })?;
//         }
//
//         txn.commit().map_err(|err| StorageError::IO {
//             source: StorageIOError::write(&err),
//         })?;
//
//         Ok(())
//     }
// }
//
// impl RaftStateMachine<TypeConfigSqlite> for StateMachineMemory {
//     type SnapshotBuilder = Self;
//
//     async fn applied_state(
//         &mut self,
//     ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, Node>), StorageError<NodeId>> {
//         Ok((
//             self.data.last_applied_log_id,
//             self.data.last_membership.clone(),
//         ))
//     }
//
//     async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, StorageError<NodeId>>
//     where
//         I: IntoIterator<Item = typ::Entry> + OptionalSend,
//         I::IntoIter: OptionalSend,
//     {
//         let entries = entries.into_iter();
//         let mut replies = Vec::with_capacity(entries.size_hint().0);
//
//         for entry in entries {
//             // TODO probably should be moved after disk IO
//             self.data.last_applied_log_id = Some(entry.log_id);
//
//             let mut resp_value = None;
//
//             match entry.payload {
//                 EntryPayload::Blank => {}
//                 EntryPayload::Normal(req) => match req {
//                     Request::Put { key, value } => {
//                         resp_value = Some(value.clone());
//                         let mut st = self.data.kvs.write().await;
//                         st.insert(key.into(), value.into());
//                     }
//
//                     Request::Delete { key } => {
//                         let mut st = self.data.kvs.write().await;
//                         st.remove(key.as_ref());
//                     }
//                 },
//                 EntryPayload::Membership(mem) => {
//                     self.data.last_membership = StoredMembership::new(Some(entry.log_id), mem);
//                 }
//             }
//
//             replies.push(Response { value: resp_value });
//         }
//         Ok(replies)
//     }
//
//     async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
//         self.snapshot_idx = Uuid::now_v7();
//         self.clone()
//     }
//
//     async fn begin_receiving_snapshot(
//         &mut self,
//     ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
//         Ok(Box::new(Cursor::new(Vec::new())))
//     }
//
//     async fn install_snapshot(
//         &mut self,
//         meta: &SnapshotMeta<NodeId, Node>,
//         snapshot: Box<SnapshotData>,
//     ) -> Result<(), StorageError<NodeId>> {
//         let new_snapshot = StoredSnapshot {
//             meta: meta.clone(),
//             data: snapshot.into_inner(),
//         };
//
//         self.update_state_machine_(new_snapshot.clone()).await?;
//
//         self.persist_snapshot(new_snapshot, *snapshot)?;
//
//         Ok(())
//     }
//
//     async fn get_current_snapshot(
//         &mut self,
//     ) -> Result<Option<Snapshot<TypeConfigSqlite>>, StorageError<NodeId>> {
//         let x = self.get_current_snapshot_()?;
//         Ok(x.map(|s| Snapshot {
//             meta: s.meta.clone(),
//             snapshot: Box::new(Cursor::new(s.data.clone())),
//         }))
//     }
// }
//
// pub(crate) async fn build_state_machine(db_path: &str) -> StateMachineMemory {
//     let dir = format!("{}/sm", db_path);
//     let path = format!("{}/kv_bytes.redb", dir);
//     fs::create_dir_all(&dir)
//         .await
//         .expect("Cannot create state machine path");
//
//     let db = match Database::open(&path) {
//         Ok(db) => db,
//         Err(_) => {
//             let db = Database::create(path).expect("Cannot create database");
//
//             // write txn is necessary to create DBs if missing
//             let txn = db.begin_write().unwrap();
//             {
//                 let mut table = txn.open_table(TABLE_SNAPSHOT).unwrap();
//                 table.insert("init", Vec::default()).unwrap();
//                 table.remove("init").unwrap();
//             }
//             txn.commit().unwrap();
//
//             db
//         }
//     };
//
//     StateMachineMemory::new(Arc::new(db)).await.unwrap()
// }
