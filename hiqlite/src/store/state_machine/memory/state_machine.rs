use crate::store::state_machine::memory::TypeConfigKV;
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
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

// pub(crate) type KvStore = Arc<RwLock<BTreeMap<String, Vec<u8>>>>;

type Entry = openraft::Entry<TypeConfigKV>;
type SnapshotData = Cursor<Vec<u8>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheRequest {
    Put {
        key: Cow<'static, str>,
        value: Vec<u8>,
    },
    Delete {
        key: Cow<'static, str>,
    },
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
    /// TODO should be converted to a concurrent map if we keep this
    pub(crate) kvs: BTreeMap<String, Vec<u8>>,
}

/// This is a full in-memory state machine acting as a cache.
/// It does not persist anything at all and losses its data when the whole Raft is being shut
/// down. If just a single node is restarting, it will re-sync in-memory data from other members.
#[derive(Debug, Default)]
pub struct StateMachineMemory {
    pub(crate) data: Arc<RwLock<StateMachineData>>,
    snapshot_idx: AtomicU64,
    snapshot: Mutex<Option<Snapshot<TypeConfigKV>>>,
}

impl RaftSnapshotBuilder<TypeConfigKV> for Arc<StateMachineMemory> {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigKV>, StorageError<NodeId>> {
        let (last_log_id, last_membership, kv_bytes) = {
            let data = self.data.read().await;
            let kv_bytes = bincode::serialize(&data.kvs)
                .map_err(|err| StorageIOError::read_state_machine(&err))?;

            let last_applied_log = data.last_applied_log_id.clone();
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
        let mut sm = Self::default();
        // let mut sm = Self {
        //     last_applied_log_id: None,
        //     last_membership: Default::default(),
        //     kvs: Arc::new(Default::default()),
        //     snapshot_idx: Uuid::default(),
        //     snapshot: None,
        // };

        // TODO maybe persist snapshots in a temp file on disk to save memory?

        // if let Some(snapshot) = sm.get_current_snapshot_()? {
        //     sm.update_state_machine_(snapshot).await?;
        // }

        Ok(sm)
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

        for entry in entries {
            data.last_applied_log_id = Some(entry.log_id);

            let resp_value = match entry.payload {
                EntryPayload::Blank => CacheResponse::Empty,
                EntryPayload::Normal(req) => match req {
                    CacheRequest::Put { key, value } => {
                        // resp_value = Some(value.clone());
                        // let mut lock = self.kvs.write().await;
                        data.kvs.insert(key.into(), value);
                        CacheResponse::Ok
                    }

                    CacheRequest::Delete { key } => {
                        // let mut lock = self.kvs.write().await;
                        data.kvs.remove(key.as_ref());
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
        data.last_applied_log_id = meta.last_log_id;
        data.last_membership = meta.last_membership.clone();
        data.kvs = kvs;

        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigKV>>, StorageError<NodeId>> {
        Ok(self.snapshot.lock().await.clone())
    }
}
