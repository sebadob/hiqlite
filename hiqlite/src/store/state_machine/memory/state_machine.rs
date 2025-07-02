use crate::cache_idx::CacheIndex;
use crate::helpers::{deserialize, serialize, set_path_access};
use crate::store::state_machine::memory::cache_ttl_handler::TtlRequest;
use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::{cache_ttl_handler, kv_handler, TypeConfigKV};
use crate::store::StorageResult;
use crate::{Error, Node, NodeId};
use chrono::Utc;
use cryptr::utils::secure_random_alnum;
use dotenvy::var;
use openraft::storage::RaftStateMachine;
use openraft::{
    EntryPayload, LogId, OptionalSend, RaftSnapshotBuilder, Snapshot, SnapshotMeta, StorageError,
    StorageIOError, StoredMembership,
};
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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::task;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::{self, *};
#[cfg(feature = "listen_notify_local")]
use crate::store::state_machine::memory::notify_handler::{self, NotifyRequest};

type Entry = openraft::Entry<TypeConfigKV>;
type SnapshotData = fs::File;

type SnapshotKVs = Vec<(BTreeMap<String, Vec<u8>>, BTreeMap<String, i64>)>;
type SnapshotTTLs = Vec<BTreeMap<i64, String>>;
type SnapshotLocks = Vec<u8>;
type SnapshotDataContent = (
    SnapshotMeta<NodeId, Node>,
    SnapshotKVs,
    SnapshotTTLs,
    SnapshotLocks,
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheRequest {
    Get {
        cache_idx: usize,
        key: String,
    },
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
    Clear {
        cache_idx: usize,
    },
    #[cfg(feature = "counters")]
    ClearCounters {
        cache_idx: usize,
    },
    ClearAll,
    #[cfg(feature = "listen_notify_local")]
    Notify((i64, Vec<u8>)),
    #[cfg(feature = "dlock")]
    Lock((Cow<'static, str>, Option<u64>)),
    #[cfg(feature = "dlock")]
    LockAwait((Cow<'static, str>, u64)),
    #[cfg(feature = "dlock")]
    LockRelease((Cow<'static, str>, u64)),
    #[cfg(feature = "counters")]
    CounterGet {
        cache_idx: usize,
        key: Cow<'static, str>,
    },
    #[cfg(feature = "counters")]
    CounterSet {
        cache_idx: usize,
        key: Cow<'static, str>,
        value: i64,
    },
    #[cfg(feature = "counters")]
    CounterAdd {
        cache_idx: usize,
        key: Cow<'static, str>,
        value: i64,
    },
    #[cfg(feature = "counters")]
    CounterDel {
        cache_idx: usize,
        key: Cow<'static, str>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CacheResponse {
    Empty,
    Ok,
    #[cfg(feature = "dlock")]
    Lock(LockState),
    Value(Option<Vec<u8>>),
    #[cfg(feature = "counters")]
    CounterValue(Option<i64>),
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
    path_snapshots: String,
    in_memory_only: bool,

    pub(crate) tx_caches: Vec<flume::Sender<CacheRequestHandler>>,
    tx_ttls: Vec<flume::Sender<TtlRequest>>,

    #[cfg(feature = "listen_notify_local")]
    pub(crate) tx_notify: flume::Sender<NotifyRequest>,
    #[cfg(feature = "listen_notify_local")]
    pub(crate) rx_notify: flume::Receiver<(i64, Vec<u8>)>,

    #[cfg(feature = "dlock")]
    pub(crate) tx_dlock: flume::Sender<LockRequest>,
}

impl RaftSnapshotBuilder<TypeConfigKV> for Arc<StateMachineMemory> {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigKV>, StorageError<NodeId>> {
        let (meta, snapshot_bytes) = {
            let data = self.data.read().await;

            // TODO should we include notifications in snapshots as well?
            //  -> unsure if it makes sense or not

            let mut ttls = Vec::with_capacity(self.tx_ttls.len());
            for tx in &self.tx_ttls {
                let (ack, rx) = oneshot::channel();
                tx.send(TtlRequest::SnapshotBuild(ack))
                    .expect("ttl handler to always be running");
                let snap = rx
                    .await
                    .expect("to always receive an answer from ttl handler");
                ttls.push(snap);
            }

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
                serialize(&locks).unwrap()
            };
            #[cfg(not(feature = "dlock"))]
            let locks_bytes: Vec<u8> = Vec::default();

            let now = Utc::now().timestamp();
            let snapshot_id = if let Some(last) = data.last_applied_log_id {
                format!("{}-{}-{}", now, last.leader_id, last.index)
            } else {
                format!("{now}--")
            };

            let meta = SnapshotMeta {
                last_log_id: data.last_applied_log_id,
                last_membership: data.last_membership.clone(),
                snapshot_id,
            };

            let snap: SnapshotDataContent = (meta.clone(), caches, ttls, locks_bytes);
            let snapshot_bytes =
                serialize(&snap).map_err(|err| StorageIOError::write_state_machine(&err))?;

            (meta, snapshot_bytes)
        };

        let path = format!("{}/{}", self.path_snapshots, meta.snapshot_id);
        let mut file = fs::File::create_new(&path)
            .await
            .map_err(|err| StorageIOError::read_state_machine(&err))?;
        file.write_all(&snapshot_bytes)
            .await
            .map_err(|err| StorageIOError::read_state_machine(&err))?;

        // cleanup task for old snapshots
        let id = meta.snapshot_id.clone();
        let path = self.path_snapshots.clone();
        task::spawn(async move {
            let mut dir = fs::read_dir(&path).await.unwrap();
            while let Ok(Some(entry)) = dir.next_entry().await {
                let fname = entry.file_name();
                let name = fname.to_str().unwrap_or_default();
                if !name.is_empty() && name != id {
                    fs::remove_file(format!("{path}/{name}")).await.unwrap();
                }
            }
        });

        let snapshot = Snapshot {
            meta,
            snapshot: Box::new(file),
        };

        Ok(snapshot)
    }
}

impl StateMachineMemory {
    pub(crate) async fn new<C>(base_path: &str, in_memory_only: bool) -> Result<Self, Error>
    where
        C: Debug + IntoEnumIterator + CacheIndex,
    {
        let path_sm = format!("{base_path}/state_machine_cache");
        let path_snapshots = format!("{path_sm}/snapshots");

        if in_memory_only {
            // in this case we must always start clean,
            // because otherwise there would be a gap in logs
            let _ = fs::remove_dir_all(&path_snapshots).await;
        }
        fs::create_dir_all(&path_snapshots).await?;
        set_path_access(&path_sm, 0o700)
            .await
            .expect("Cannot set access rights for path_sm");

        // we must make sure that the index is correct and in order
        let mut len = 0;
        for variant in C::iter() {
            let value = variant.to_usize();
            if value != len {
                return Err(Error::Config(
                    format!(
                        "'Cache' enum's `.to_usize()` must return each elements position in the \
                    iterator. Expected {len} for {value:?}"
                    )
                    .into(),
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

        #[cfg(feature = "dlock")]
        let tx_dlock = dlock_handler::spawn();

        #[cfg(feature = "listen_notify_local")]
        let (tx_notify, rx_notify) = notify_handler::spawn();

        let slf = Self {
            data: RwLock::new(StateMachineData::default()),
            path_snapshots,
            in_memory_only,
            tx_caches,
            tx_ttls,
            #[cfg(feature = "listen_notify_local")]
            tx_notify,
            #[cfg(feature = "listen_notify_local")]
            rx_notify,
            #[cfg(feature = "dlock")]
            tx_dlock,
        };

        if let Some((_, content)) = slf
            .read_current_snapshot()
            .await
            .expect("Cannot read current snapshot")
        {
            slf.update_state_machine(content).await;
        }

        Ok(slf)
    }

    async fn update_state_machine(&self, content: SnapshotDataContent) {
        let (meta, kvs, ttls, locks) = content;

        // make sure to hold the metadata lock the whole time
        let mut data = self.data.write().await;

        for (idx, kv_data) in kvs.into_iter().enumerate() {
            let (ack, rx) = oneshot::channel();
            self.tx_caches
                .get(idx)
                .unwrap()
                .send(CacheRequestHandler::SnapshotInstall((kv_data, ack)))
                .expect("kv handler to always be running");
            rx.await
                .expect("to always receive an answer from the kv handler");
        }

        for (idx, kv_data) in ttls.into_iter().enumerate() {
            let (ack, rx) = oneshot::channel();
            self.tx_ttls
                .get(idx)
                .unwrap()
                .send(TtlRequest::SnapshotInstall((kv_data, ack)))
                .expect("ttl handler to always be running");
            rx.await
                .expect("to always receive an answer from the ttl handler");
        }

        #[cfg(feature = "dlock")]
        {
            let locks: HashMap<String, dlock_handler::LockQueue> = deserialize(&locks).unwrap();
            let (ack, rx) = oneshot::channel();
            self.tx_dlock
                .send(LockRequest::SnapshotInstall((locks, ack)))
                .expect("locks handler to always be running");
            rx.await
                .expect("to always get an answer from locks handler");
        }

        data.last_applied_log_id = meta.last_log_id;
        data.last_membership = meta.last_membership;
    }

    pub async fn read_current_snapshot(
        &self,
    ) -> StorageResult<Option<(String, SnapshotDataContent)>> {
        let mut list = tokio::fs::read_dir(&self.path_snapshots)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;

        let mut latest_ts: Option<i64> = None;
        let mut latest_file_name = None;
        while let Ok(Some(entry)) = list.next_entry().await {
            let file_name = entry.file_name();
            let name = file_name.to_str().unwrap_or_default();

            let meta = entry.metadata().await.map_err(|err| StorageError::IO {
                source: StorageIOError::read(&err),
            })?;
            if meta.is_dir() {
                warn!("Invalid folder in snapshots dir: {}", name);
                continue;
            }

            let Some((ts, rest)) = name.split_once('-') else {
                warn!("Invalid filename in snapshots dir: {}", name);
                continue;
            };
            let Ok(ts) = ts.parse::<i64>() else {
                warn!(
                    "Invalid filename in snapshots dir, does not start with TS: {}",
                    name
                );
                continue;
            };

            if let Some(latest) = latest_ts {
                if ts > latest {
                    latest_ts = Some(ts);
                    latest_file_name = Some(name.to_string());
                } else if ts == latest {
                    // may happen if 2 snapshots have been created at the exact same second
                    let Some((rest_, log_id)) = name.rsplit_once('-') else {
                        warn!("Invalid filename in snapshots dir: {}", name);
                        continue;
                    };
                    let Ok(log_id) = log_id.parse::<i64>() else {
                        warn!(
                            "Invalid filename in snapshots dir, invalid log id: {}",
                            name
                        );
                        continue;
                    };

                    let last_name = latest_file_name.as_deref().unwrap_or_default();
                    let Some((rest_, log_id_latest)) = name.rsplit_once('-') else {
                        warn!("Invalid filename in snapshots dir: {}", name);
                        continue;
                    };
                    let Ok(log_id_latest) = log_id_latest.parse::<i64>() else {
                        warn!(
                            "Invalid filename in snapshots dir, invalid log id: {}",
                            name
                        );
                        continue;
                    };

                    if log_id > log_id_latest {
                        latest_ts = Some(ts);
                        latest_file_name = Some(name.to_string());
                    }
                }
            } else {
                latest_ts = Some(ts);
                latest_file_name = Some(name.to_string());
            }
        }
        if latest_ts.is_none() {
            return Ok(None);
        }

        debug_assert!(latest_file_name.is_some());
        let path = format!(
            "{}/{}",
            self.path_snapshots,
            latest_file_name.unwrap_or_default()
        );

        let bytes = fs::read(&path)
            .await
            .map_err(|e| StorageIOError::read_snapshot(None, &e))?;

        Ok(Some((
            path,
            deserialize::<SnapshotDataContent>(&bytes)
                .map_err(|e| StorageIOError::read_snapshot(None, &e))?,
        )))
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
                    CacheRequest::Get { .. } => {
                        unreachable!("a CacheRequest::Get should never come through the Raft")
                    }

                    CacheRequest::Put {
                        cache_idx,
                        key,
                        value,
                        expires,
                    } => {
                        if let Some(exp) = expires {
                            self.tx_ttls
                                .get(cache_idx)
                                .unwrap()
                                .send(TtlRequest::Ttl((exp, key.to_string())))
                                .expect("cache ttl handler to always be running");
                        }

                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Put((key.to_string(), value)))
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::Delete { cache_idx, key } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Delete(key.to_string()))
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::Clear { cache_idx } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Clear)
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                    }

                    #[cfg(feature = "counters")]
                    CacheRequest::ClearCounters { cache_idx } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::ClearCounters)
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::ClearAll => {
                        for tx in &self.tx_caches {
                            tx.send(CacheRequestHandler::Clear)
                                .expect("cache ttl handler to always be running");
                            #[cfg(feature = "counters")]
                            tx.send(CacheRequestHandler::ClearCounters)
                                .expect("cache ttl handler to always be running");
                        }

                        CacheResponse::Ok
                    }

                    #[cfg(feature = "listen_notify_local")]
                    CacheRequest::Notify(payload) => {
                        self.tx_notify
                            .send(NotifyRequest::Notify(payload))
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
                    CacheRequest::LockAwait(..) => {
                        unreachable!("Lock Awaits should never come through the Raft")
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

                    #[cfg(feature = "counters")]
                    CacheRequest::CounterGet { .. } => {
                        unreachable!("a CacheRequest::Get should never come through the Raft")
                    }

                    #[cfg(feature = "counters")]
                    CacheRequest::CounterSet {
                        cache_idx,
                        key,
                        value,
                    } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::CounterSet((key.to_string(), value)))
                            .expect("cache ttl handler to always be running");

                        CacheResponse::Ok
                    }

                    #[cfg(feature = "counters")]
                    CacheRequest::CounterAdd {
                        cache_idx,
                        key,
                        value,
                    } => {
                        let (ack, rx) = oneshot::channel();

                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::CounterAdd((
                                key.to_string(),
                                value,
                                ack,
                            )))
                            .expect("cache ttl handler to always be running");

                        let v = rx.await.unwrap();
                        CacheResponse::CounterValue(Some(v))
                    }

                    #[cfg(feature = "counters")]
                    CacheRequest::CounterDel { cache_idx, key } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::CounterDel(key.to_string()))
                            .expect("cache ttl handler to always be running");

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

    #[tracing::instrument(skip_all)]
    async fn begin_receiving_snapshot(&mut self) -> Result<Box<fs::File>, StorageError<NodeId>> {
        let path = format!("{}/temp", self.path_snapshots);
        info!("Saving incoming snapshot to {}", path);

        // clean up possible existing old data
        let _ = fs::remove_file(&path).await;

        match fs::File::create(path).await {
            Ok(file) => Ok(Box::new(file)),
            Err(err) => Err(StorageError::IO {
                source: StorageIOError::write(&err),
            }),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, Node>,
        snapshot: Box<SnapshotData>,
    ) -> Result<(), StorageError<NodeId>> {
        let src = format!("{}/temp", self.path_snapshots);
        let dest = format!("{}/{}", self.path_snapshots, meta.snapshot_id);
        fs::copy(&src, &dest)
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::write(&err),
            })?;

        fs::remove_file(src).await.map_err(|err| StorageError::IO {
            source: StorageIOError::write(&err),
        })?;

        let bytes = fs::read(dest)
            .await
            .map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;

        let (meta_snap, kvs, ttls, locks) = deserialize::<SnapshotDataContent>(&bytes)
            .map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;
        debug_assert_eq!(meta.snapshot_id, meta_snap.snapshot_id);
        debug_assert_eq!(meta.last_log_id, meta_snap.last_log_id);
        debug_assert_eq!(meta.last_membership, meta_snap.last_membership);

        self.update_state_machine((meta_snap, kvs, ttls, locks))
            .await;

        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigKV>>, StorageError<NodeId>> {
        match self.read_current_snapshot().await? {
            None => Ok(None),
            Some((path, (meta, kvs, ttls, locks))) => {
                let file = fs::File::open(path).await.map_err(|err| StorageError::IO {
                    source: StorageIOError::read(&err),
                })?;

                let snapshot = Snapshot {
                    meta,
                    snapshot: Box::new(file),
                };

                Ok(Some(snapshot))
            }
        }
    }
}
