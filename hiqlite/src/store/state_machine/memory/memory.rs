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
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use uuid::Uuid;

type Entry = openraft::Entry<TypeConfigKV>;
type SnapshotData = Cursor<Vec<u8>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CacheRequest {
    Put {
        key: Cow<'static, str>,
        value: Cow<'static, str>,
    },
    Delete {
        key: Cow<'static, str>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheResponse {
    pub value: Option<Cow<'static, str>>,
}

/// This is a full in-memory state machine acting as a cache.
/// It does not persist anything at all and losses its data when the whole Raft is being shut
/// down. If just a single node is restarting, it will re-sync in-memory data from other members.
#[derive(Debug, Clone)]
pub struct StateMachineMemory {
    // data: StateMachineData,
    last_applied_log_id: Option<LogId<NodeId>>,
    last_membership: StoredMembership<NodeId, Node>,

    /// TODO should be converted to a concurrent map if we keep this
    kvs: Arc<RwLock<BTreeMap<String, String>>>,

    snapshot_idx: Uuid,
    snapshot: Option<Snapshot<TypeConfigKV>>,
}

impl RaftSnapshotBuilder<TypeConfigKV> for StateMachineMemory {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigKV>, StorageError<NodeId>> {
        let (last_log_id, last_membership, kv_bytes) = {
            let kvs = self.kvs.read().await;
            let kv_bytes = bincode::serialize(&*kvs)
                .map_err(|err| StorageIOError::read_state_machine(&err))?;

            let last_applied_log = self.last_applied_log_id;
            let last_membership = self.last_membership.clone();

            (last_applied_log, last_membership, kv_bytes)
        };

        let snapshot = Snapshot {
            meta: SnapshotMeta {
                last_log_id,
                last_membership,
                snapshot_id: self.snapshot_idx.to_string(),
            },
            snapshot: Box::new(Cursor::new(kv_bytes)),
        };
        self.snapshot = Some(snapshot.clone());

        Ok(snapshot)
    }
}

impl StateMachineMemory {
    pub(crate) async fn new() -> Result<Self, StorageError<NodeId>> {
        let mut sm = Self {
            last_applied_log_id: None,
            last_membership: Default::default(),
            kvs: Arc::new(Default::default()),
            snapshot_idx: Uuid::default(),
            snapshot: None,
        };

        // TODO maybe persist snapshots in a temp file on disk to save memory?

        // if let Some(snapshot) = sm.get_current_snapshot_()? {
        //     sm.update_state_machine_(snapshot).await?;
        // }

        Ok(sm)
    }
}

impl RaftStateMachine<TypeConfigKV> for StateMachineMemory {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, Node>), StorageError<NodeId>> {
        Ok((self.last_applied_log_id, self.last_membership.clone()))
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
        let mut lock = self.kvs.write().await;

        for entry in entries {
            self.last_applied_log_id = Some(entry.log_id);

            let mut resp_value = None;

            match entry.payload {
                EntryPayload::Blank => {}
                EntryPayload::Normal(req) => match req {
                    CacheRequest::Put { key, value } => {
                        resp_value = Some(value.clone());
                        // let mut lock = self.kvs.write().await;
                        lock.insert(key.into(), value.into());
                    }

                    CacheRequest::Delete { key } => {
                        // let mut lock = self.kvs.write().await;
                        lock.remove(key.as_ref());
                    }
                },
                EntryPayload::Membership(mem) => {
                    self.last_membership = StoredMembership::new(Some(entry.log_id), mem);
                }
            }

            replies.push(CacheResponse { value: resp_value });
        }
        Ok(replies)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.snapshot_idx = Uuid::now_v7();
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
        let kvs: BTreeMap<String, String> = bincode::deserialize(snapshot.get_ref())
            .map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;

        self.last_applied_log_id = meta.last_log_id;
        self.last_membership = meta.last_membership.clone();
        let mut lock = self.kvs.write().await;
        *lock = kvs;

        // self.persist_snapshot(new_snapshot, *snapshot)?;

        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigKV>>, StorageError<NodeId>> {
        Ok(self.snapshot.clone())
    }
}
