use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::{cache_ttl_handler, kv_handler, TypeConfigKV};
use crate::store::StorageResult;
use crate::{Error, Node, NodeId};
use dotenvy::var;
use num_traits::ToPrimitive;
use openraft::storage::RaftStateMachine;
use openraft::{
    EntryPayload, LogId, OptionalSend, RaftSnapshotBuilder, Snapshot, SnapshotMeta, StorageError,
    StorageIOError, StoredMembership,
};
use rusqlite::types::Type;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use strum::IntoEnumIterator;
use tokio::fs;
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::info;
use uuid::Uuid;

#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::{self, *};

type Entry = openraft::Entry<TypeConfigKV>;
type SnapshotData = Cursor<Vec<u8>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheRequest {
    Put {
        cache_idx: usize,
        key: Cow<'static, str>,
        value: Vec<u8>,
        expires: Option<i64>,
    },
    Delete {
        cache_idx: usize,
        key: Cow<'static, str>,
    },
    Notify((i64, Vec<u8>)),
    #[cfg(feature = "dlock")]
    Lock((Cow<'static, str>, Option<u64>)),
    #[cfg(feature = "dlock")]
    LockRelease((Cow<'static, str>, u64)),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CacheResponse {
    Empty,
    Ok,
    #[cfg(feature = "dlock")]
    Lock(LockState),
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

    pub(crate) tx_caches: Vec<flume::Sender<CacheRequestHandler>>,
    tx_ttls: Vec<flume::Sender<(i64, String)>>,

    tx_notify: flume::Sender<(i64, Vec<u8>)>,
    pub(crate) rx_notify: flume::Receiver<(i64, Vec<u8>)>,

    #[cfg(feature = "dlock")]
    pub(crate) tx_dlock: flume::Sender<LockRequest>,
}

impl RaftSnapshotBuilder<TypeConfigKV> for Arc<StateMachineMemory> {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigKV>, StorageError<NodeId>> {
        let (last_log_id, last_membership, kv_bytes) = {
            let data = self.data.read().await;

            // TODO should we include notifications in snapshots as well? -> unsure if it makes sense or not

            let mut caches = Vec::with_capacity(self.tx_caches.len());
            for tx in &self.tx_caches {
                let (ack, rx) = oneshot::channel();
                tx.send(CacheRequestHandler::SnapshotBuild(ack))
                    .expect("kv handler to always be running");
                let snap = rx
                    .await
                    .expect("to always receive an answer from kv handler");
                caches.push(snap);
            }

            #[cfg(feature = "dlock")]
            let locks_bytes = {
                let (ack, rx) = oneshot::channel();
                self.tx_dlock
                    .send(LockRequest::SnapshotBuild(ack))
                    .expect("locks handler to always be running");
                let locks = rx
                    .await
                    .expect("to always receive an answer from locks handler");
                bincode::serialize(&locks).unwrap()
            };
            #[cfg(not(feature = "dlock"))]
            let locks_bytes: Vec<u8> = Vec::default();

            let snap = (caches, locks_bytes);
            let snapshot_bytes = bincode::serialize(&snap)
                .map_err(|err| StorageIOError::read_state_machine(&err))?;

            let last_applied_log = data.last_applied_log_id;
            let last_membership = data.last_membership.clone();

            (last_applied_log, last_membership, snapshot_bytes)
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
    pub(crate) async fn new<C>() -> Result<Self, Error>
    where
        C: Debug + IntoEnumIterator + ToPrimitive,
    {
        // we must make sure that the index is correct and in order
        let mut len = 0;
        for variant in C::iter() {
            if variant.to_usize().unwrap() != len {
                return Err(Error::Config(
                    "Cache Index enum must start at '0' and have no gaps in iter()".into(),
                ));
            }
            len += 1;
        }
        if len == 0 {
            return Err(Error::Config("Cache Index enum is empty".into()));
        }

        // we will start a separate task for each given cache index
        let mut tx_caches = Vec::with_capacity(len);
        let mut tx_ttls = Vec::with_capacity(len);
        for variant in C::iter() {
            let tx_cache = kv_handler::spawn(variant);
            tx_caches.push(tx_cache.clone());
            tx_ttls.push(cache_ttl_handler::spawn(tx_cache));
        }
        // let tx_ttl = cache_ttl_handler::spawn(tx_caches.clone());

        #[cfg(feature = "dlock")]
        let tx_dlock = dlock_handler::spawn();

        let (tx_notify, rx_notify) = flume::unbounded();

        Ok(Self {
            data: Default::default(),
            snapshot_idx: AtomicU64::new(0),
            snapshot: Default::default(),
            tx_caches,
            tx_ttls,
            tx_notify,
            rx_notify,
            #[cfg(feature = "dlock")]
            tx_dlock,
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
                        cache_idx,
                        key,
                        value,
                        expires,
                    } => {
                        let idx = cache_idx.to_usize().unwrap();

                        if let Some(exp) = expires {
                            self.tx_ttls
                                .get(idx)
                                .unwrap()
                                .send((exp, key.to_string()))
                                .expect("cache ttl handler to always be running");
                        }

                        self.tx_caches
                            .get(idx)
                            .unwrap()
                            .send(CacheRequestHandler::Put((key.to_string(), value)))
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::Delete { cache_idx, key } => {
                        let idx = cache_idx.to_usize().unwrap();

                        self.tx_caches
                            .get(idx)
                            .unwrap()
                            .send(CacheRequestHandler::Delete(key.to_string()))
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::Notify(payload) => {
                        self.tx_notify
                            .send(payload)
                            // this channel can never be closed - we have both sides
                            .unwrap();
                        CacheResponse::Ok
                    }

                    #[cfg(feature = "dlock")]
                    CacheRequest::Lock((key, id)) => {
                        let (ack, rx) = oneshot::channel();

                        // the id will be Some(_) in case this request is coming in after awaiting a queue
                        if let Some(log_id) = id {
                            self.tx_dlock
                                .send(LockRequest::Acquire(LockRequestPayload {
                                    key,
                                    log_id,
                                    ack,
                                }))
                                // this channel can never be closed - we have both sides
                                .unwrap();
                        } else {
                            let log_id = id.unwrap_or(last_applied_log_id.unwrap().index);
                            self.tx_dlock
                                .send(LockRequest::Lock(LockRequestPayload { key, log_id, ack }))
                                // this channel can never be closed - we have both sides
                                .unwrap();
                        }

                        let state = rx
                            .await
                            .expect("To always get a response from dlock handler");

                        CacheResponse::Lock(state)
                    }

                    #[cfg(feature = "dlock")]
                    CacheRequest::LockRelease((key, id)) => {
                        self.tx_dlock
                            .send(LockRequest::Release(LockReleasePayload { key, id }))
                            // this channel can never be closed - we have both sides
                            .unwrap();

                        // we can return early without waiting for answer, release should never fail anyway
                        CacheResponse::Lock(LockState::Released)
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
        let snap: (Vec<BTreeMap<String, Vec<u8>>>, Vec<u8>) =
            bincode::deserialize(snapshot.get_ref())
                .map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;
        // let caches: Vec<BTreeMap<String, Vec<u8>>> = bincode::deserialize(snapshot.get_ref())
        //     .map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;

        // make sure to hold the metadata lock the whole time
        let mut data = self.data.write().await;

        for (idx, kv_data) in snap.0.into_iter().enumerate() {
            let (ack, rx) = oneshot::channel();
            self.tx_caches
                .get(idx)
                .unwrap()
                .send(CacheRequestHandler::SnapshotInstall((kv_data, ack)))
                .expect("kv handler to always be running");
            rx.await
                .expect("to always receive an answer from the kv handler");
        }

        #[cfg(feature = "dlock")]
        {
            let locks: HashMap<String, dlock_handler::LockQueue> =
                bincode::deserialize(&snap.1).unwrap();
            let (ack, rx) = oneshot::channel();
            self.tx_dlock
                .send(LockRequest::SnapshotInstall((locks, ack)))
                .expect("locks handler to always be running");
            rx.await
                .expect("to always get an answer from locks handler");
        }

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
