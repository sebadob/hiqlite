use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::{cache_ttl_handler, kv_handler, TypeConfigKV};
use crate::store::StorageResult;
use crate::{Node, NodeId};
use openraft::storage::RaftStateMachine;
use openraft::{
    EntryPayload, LogId, OptionalSend, RaftSnapshotBuilder, Snapshot, SnapshotMeta, StorageError,
    StorageIOError, StoredMembership,
};
use redb::{Database, TableDefinition};
use rusqlite::types::Type;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::info;
use uuid::Uuid;

// pub(crate) type KvStore = Arc<RwLock<BTreeMap<String, Vec<u8>>>>;

type Entry = openraft::Entry<TypeConfigKV>;
type SnapshotData = Cursor<Vec<u8>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheRequest {
    Put {
        key: Cow<'static, str>,
        value: Vec<u8>,
        expires: Option<i64>,
    },
    Delete {
        key: Cow<'static, str>,
    },
    Notify((i64, Vec<u8>)),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CacheResponse {
    Empty,
    Ok,
}

#[derive(Debug, Default)]
pub struct StateMachineData {
    last_applied_log_id: Option<LogId<NodeId>>,
    last_membership: StoredMembership<NodeId, Node>,
}

/// This is a full in-memory state machine acting as a cache.
/// It does not persist anything at all and losses its data when the whole Raft is being shut
/// down. If just a single node is restarting, it will re-sync in-memory data from other members.
#[derive(Debug)]
pub struct StateMachineMemory {
    data: RwLock<StateMachineData>,
    snapshot_idx: AtomicU64,
    snapshot: Mutex<Option<Snapshot<TypeConfigKV>>>,

    pub(crate) tx_kv: flume::Sender<CacheRequestHandler>,
    tx_ttl: flume::Sender<(i64, String)>,

    tx_notify: flume::Sender<(i64, Vec<u8>)>,
    pub(crate) rx_notify: flume::Receiver<(i64, Vec<u8>)>,
}

impl RaftSnapshotBuilder<TypeConfigKV> for Arc<StateMachineMemory> {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigKV>, StorageError<NodeId>> {
        let (last_log_id, last_membership, kv_bytes) = {
            let data = self.data.read().await;

            let (ack, rx) = oneshot::channel();
            self.tx_kv
                .send(CacheRequestHandler::SnapshotBuild(ack))
                .expect("kv handler to always be running");
            let kvs = rx
                .await
                .expect("to always receive an answer from kv handler");
            let kv_bytes =
                bincode::serialize(&kvs).map_err(|err| StorageIOError::read_state_machine(&err))?;

            let last_applied_log = data.last_applied_log_id;
            let last_membership = data.last_membership.clone();

            (last_applied_log, last_membership, kv_bytes)
        };

        let snapshot_idx = self.snapshot_idx.fetch_add(1, Ordering::Relaxed) + 1;
        let snapshot_id = if let Some(last) = last_log_id {
            format!("{}-{}-{}", last.leader_id, last.index, snapshot_idx)
        } else {
            format!("--{}", snapshot_idx)
        };

        let snapshot = Snapshot {
            meta: SnapshotMeta {
                last_log_id,
                last_membership,
                snapshot_id,
            },
            snapshot: Box::new(Cursor::new(kv_bytes)),
        };

        {
            let mut current_snapshot = self.snapshot.lock().await;
            *current_snapshot = Some(snapshot.clone());
        }

        Ok(snapshot)
    }
}

impl StateMachineMemory {
    pub(crate) async fn new() -> Result<Self, StorageError<NodeId>> {
        let tx_kv = kv_handler::spawn();
        let tx_ttl = cache_ttl_handler::spawn(tx_kv.clone());

        let (tx_notify, rx_notify) = flume::unbounded();

        Ok(Self {
            data: Default::default(),
            snapshot_idx: AtomicU64::new(0),
            snapshot: Default::default(),
            tx_kv,
            tx_ttl,
            tx_notify,
            rx_notify,
        })
    }
}

impl RaftStateMachine<TypeConfigKV> for Arc<StateMachineMemory> {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, Node>), StorageError<NodeId>> {
        let data = self.data.read().await;
        Ok((data.last_applied_log_id, data.last_membership.clone()))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<CacheResponse>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let entries = entries.into_iter();
        let mut replies = Vec::with_capacity(entries.size_hint().0);

        // TODO if this takes `&mut self`, can we assume that there will be no reads in between?
        // TODO -> we could take the lock only once at the start and be much faster with everything!
        let mut data = self.data.write().await;

        let mut last_applied_log_id = None;
        for entry in entries {
            last_applied_log_id = Some(entry.log_id);

            // we are using sync sends -> unbounded channels
            let resp_value = match entry.payload {
                EntryPayload::Blank => CacheResponse::Empty,

                EntryPayload::Normal(req) => match req {
                    CacheRequest::Put {
                        key,
                        value,
                        expires,
                    } => {
                        if let Some(exp) = expires {
                            self.tx_ttl
                                .send((exp, key.to_string()))
                                .expect("cache ttl handler to always be running");
                        }

                        self.tx_kv
                            .send(CacheRequestHandler::Put((key.to_string(), value)))
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                        // data.kvs.insert(key.into(), value);
                    }

                    CacheRequest::Delete { key } => {
                        self.tx_kv
                            .send(CacheRequestHandler::Delete(key.to_string()))
                            .expect("cache ttl handler to always be running");
                        // let mut lock = self.kvs.write().await;
                        // data.kvs.remove(key.as_ref());
                        CacheResponse::Ok
                    }

                    CacheRequest::Notify(payload) => {
                        self.tx_notify
                            .send(payload)
                            // this channel can never be closed - we have both sides
                            .unwrap();
                        CacheResponse::Ok
                    }
                },

                EntryPayload::Membership(mem) => {
                    data.last_membership = StoredMembership::new(Some(entry.log_id), mem);
                    CacheResponse::Empty
                }
            };

            replies.push(resp_value);
        }

        data.last_applied_log_id = last_applied_log_id;

        Ok(replies)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, Node>,
        snapshot: Box<SnapshotData>,
    ) -> Result<(), StorageError<NodeId>> {
        let kvs: BTreeMap<String, Vec<u8>> = bincode::deserialize(snapshot.get_ref())
            .map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;

        let mut data = self.data.write().await;

        let (ack, rx) = oneshot::channel();
        self.tx_kv
            .send(CacheRequestHandler::SnapshotInstall((kvs, ack)))
            .expect("kv handler to always be running");
        rx.await
            .expect("to always receive an answer from the kv handler");

        data.last_applied_log_id = meta.last_log_id;
        data.last_membership = meta.last_membership.clone();

        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigKV>>, StorageError<NodeId>> {
        Ok(self.snapshot.lock().await.clone())
    }
}
