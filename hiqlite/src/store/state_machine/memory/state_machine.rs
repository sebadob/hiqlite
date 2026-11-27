use crate::helpers::{atomic_file_switch, deserialize_serde, serialize_serde, set_path_access};
use crate::store::StorageResult;
use crate::store::state_machine::memory::kv_handler::{CacheRequestHandler, CacheSnapshot};
use crate::store::state_machine::memory::{TypeConfigCache, kv_handler};
use crate::{CacheVariants, Error};
use bincode_next::{Decode, Encode};
use chrono::Utc;
use futures_util::{Stream, StreamExt};
use openraft::storage::{EntryResponder, RaftStateMachine, Snapshot};
use openraft::type_config::alias::{
    EntryOf, LogIdOf, SnapshotMetaOf, SnapshotOf, StoredMembershipOf,
};
use openraft::{EntryPayload, OptionalSend, RaftSnapshotBuilder};
use rust_decimal::prelude::ToPrimitive;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Display};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock, oneshot};
use tracing::warn;
use uuid::Uuid;

#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::{
    self, LockAlivePayload, LockReleasePayload, LockRequest, LockRequestPayload, LockState,
};
#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::{self, NotifyRequest};
#[cfg(feature = "in-memory-snapshots")]
use std::io::Cursor;

type Entry = EntryOf<TypeConfigCache>;

/// Each cache by its variant name.
type SnapshotKVs = Vec<(String, CacheSnapshot)>;
type SnapshotLocks = Vec<u8>;
type SnapshotDataContent = (SnapshotMetaOf<TypeConfigCache>, SnapshotKVs, SnapshotLocks);
/// The latest snapshot kept in memory (`meta` + serialized bytes) for memory-only mode.
#[cfg(feature = "in-memory-snapshots")]
type MemSnapshot = (SnapshotMetaOf<TypeConfigCache>, Vec<u8>);

// The variant order is part of the raft log format and must stay stable and
// feature-independent: adding variants changes the serialized indices of
// everything after them, which silently corrupts logs written with a different
// feature set. New variants therefore go at the end of the enum.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum CacheRequest {
    Get {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        key: String,
    },
    Put {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: Vec<u8>,
        expires: Option<i64>,
    },
    GetRemove {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
    },
    Replace {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: Vec<u8>,
        expires: Option<i64>,
    },
    Delete {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
    },
    Clear {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    ClearCounters {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
    },
    ClearAll,
    #[allow(dead_code)] // only constructed with the `listen_notify` feature
    Notify((i64, Vec<u8>)),
    #[allow(dead_code)] // only constructed with the `dlock` feature
    Lock(#[bincode(with_serde)] (Cow<'static, str>, Option<u64>)),
    #[allow(dead_code)] // only constructed with the `dlock` feature
    LockAlive(#[bincode(with_serde)] (Cow<'static, str>, u64)),
    #[allow(dead_code)] // only constructed with the `dlock` feature
    LockAwait(#[bincode(with_serde)] (Cow<'static, str>, u64)),
    #[allow(dead_code)] // only constructed with the `dlock` feature
    LockRelease(#[bincode(with_serde)] (Cow<'static, str>, u64)),
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterGet {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterSet {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: i64,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterAdd {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: i64,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterDel {
        #[bincode(with_serde)]
        cache: Cow<'static, str>,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
    },
}

impl Display for CacheRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
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
    last_applied_log_id: Option<LogIdOf<TypeConfigCache>>,
    last_membership: StoredMembershipOf<TypeConfigCache>,
    /// The caches the log names that the local `CacheVariants` enum does not have.
    caches_unknown: BTreeMap<String, flume::Sender<CacheRequestHandler>>,
}

/// This is a full in-memory state machine acting as a cache.
/// It does not persist anything at all and losses its data when the whole Raft is being shut
/// down. If just a single node is restarting, it will re-sync in-memory data from other members.
#[derive(Debug)]
pub struct StateMachineMemory {
    data: RwLock<StateMachineData>,
    path_snapshots: String,
    /// Whether the cache runs fully in-memory (`cache_storage_disk = false`). Only relevant
    /// with the `in-memory-snapshots` feature, which is the only mode that skips `data_dir`.
    #[cfg(feature = "in-memory-snapshots")]
    in_memory_only: bool,
    /// Holds the latest snapshot when running purely in-memory (`in_memory_only == true`).
    /// In that mode nothing is ever written to `data_dir`, so a cache-only node does not
    /// require it to exist or be writable. Unused (always `None`) when persisting to disk.
    #[cfg(feature = "in-memory-snapshots")]
    snapshot_mem: RwLock<Option<MemSnapshot>>,

    pub(crate) tx_caches: Vec<flume::Sender<CacheRequestHandler>>,
    pub(crate) cache_map: &'static phf::Map<&'static str, usize>,

    #[cfg(feature = "listen_notify")]
    pub(crate) tx_notify: flume::Sender<NotifyRequest>,
    #[cfg(feature = "listen_notify")]
    pub(crate) rx_notify: flume::Receiver<(i64, Vec<u8>)>,

    #[cfg(feature = "dlock")]
    pub(crate) tx_dlock: flume::Sender<LockRequest>,
}

impl RaftSnapshotBuilder<TypeConfigCache> for Arc<StateMachineMemory> {
    #[cfg(not(feature = "in-memory-snapshots"))]
    type SnapshotData = fs::File;
    #[cfg(feature = "in-memory-snapshots")]
    type SnapshotData = Cursor<Vec<u8>>;

    #[cfg(not(feature = "in-memory-snapshots"))]
    async fn build_snapshot(
        &mut self,
    ) -> Result<SnapshotOf<TypeConfigCache, Self::SnapshotData>, io::Error> {
        let (meta, snapshot_bytes) = self.build_snapshot_data().await?;
        let path = self.persist_snapshot(&meta, &snapshot_bytes).await?;

        // Stream the snapshot straight from the persisted file (zero-copy).
        let file = fs::File::open(&path).await?;
        Ok(Snapshot {
            meta,
            snapshot: file,
        })
    }

    #[cfg(feature = "in-memory-snapshots")]
    async fn build_snapshot(
        &mut self,
    ) -> Result<SnapshotOf<TypeConfigCache, Self::SnapshotData>, io::Error> {
        let (meta, snapshot_bytes) = self.build_snapshot_data().await?;

        // In memory-only mode we keep the snapshot in memory and never touch `data_dir`,
        // so a pure cache-only node does not require it to exist or be writable.
        if self.in_memory_only {
            *self.snapshot_mem.write().await = Some((meta.clone(), snapshot_bytes.clone()));
            return Ok(Snapshot {
                meta,
                snapshot: Cursor::new(snapshot_bytes),
            });
        }

        // Disk-backed: persist exactly like the default path, but stream from memory.
        self.persist_snapshot(&meta, &snapshot_bytes).await?;
        Ok(Snapshot {
            meta,
            snapshot: Cursor::new(snapshot_bytes),
        })
    }
}

impl StateMachineMemory {
    pub(crate) fn path_sm(data_dir: &str) -> String {
        format!("{data_dir}/state_machine_cache")
    }

    /// File name of a snapshot inside the snapshots dir. openraft 0.10 removed the id from
    /// `SnapshotMeta`, so we keep our own: a timestamp plus the last applied log id. Both the
    /// snapshot reads and the cleanup pick the "newest" one by name, so the timestamp must stay
    /// the leading, fixed-width part.
    fn snapshot_file_name(meta: &SnapshotMetaOf<TypeConfigCache>) -> String {
        let now = Utc::now().timestamp();
        match &meta.last_log_id {
            Some(last) => format!("{}-{}-{}", now, last.leader_id, last.index),
            None => format!("{now}--"),
        }
    }

    pub(crate) async fn new<C>(base_path: &str, in_memory_only: bool) -> Result<Self, Error>
    where
        C: Debug + CacheVariants,
    {
        let path_sm = Self::path_sm(base_path);
        let path_snapshots = format!("{path_sm}/snapshots");

        // Default: snapshots are always persisted, so `data_dir` is always required.
        #[cfg(not(feature = "in-memory-snapshots"))]
        {
            if in_memory_only {
                // in this case we must always start clean,
                // because otherwise there would be a gap in logs
                let _ = fs::remove_dir_all(&path_snapshots).await;
            }
            fs::create_dir_all(&path_snapshots).await?;
            set_path_access(&path_sm, 0o700)
                .await
                .expect("Cannot set access rights for path_sm");
        }
        // With `in-memory-snapshots`, a memory-only node never persists snapshots and must
        // not touch `data_dir` at all. Disk-backed nodes still create (and keep) the dir.
        #[cfg(feature = "in-memory-snapshots")]
        if !in_memory_only {
            fs::create_dir_all(&path_snapshots).await?;
            set_path_access(&path_sm, 0o700)
                .await
                .expect("Cannot set access rights for path_sm");
        }

        // we will start a separate task for each given cache index
        let variants = C::hiqlite_cache_variants();
        let mut tx_caches = Vec::with_capacity(variants.len());
        for (_, name) in variants {
            tx_caches.push(kv_handler::spawn(Cow::Borrowed(name)));
        }

        #[cfg(feature = "dlock")]
        let tx_dlock = dlock_handler::spawn();

        #[cfg(feature = "listen_notify")]
        let (tx_notify, rx_notify) = notify_handler::spawn();

        let slf = Self {
            data: RwLock::new(StateMachineData::default()),
            path_snapshots,
            #[cfg(feature = "in-memory-snapshots")]
            in_memory_only,
            #[cfg(feature = "in-memory-snapshots")]
            snapshot_mem: RwLock::new(None),
            tx_caches,
            cache_map: C::hiqlite_cache_map(),
            #[cfg(feature = "listen_notify")]
            tx_notify,
            #[cfg(feature = "listen_notify")]
            rx_notify,
            #[cfg(feature = "dlock")]
            tx_dlock,
        };

        // Restore the latest persisted snapshot on startup.
        #[cfg(not(feature = "in-memory-snapshots"))]
        if let Some((_, content)) = slf
            .read_current_snapshot()
            .await
            .expect("Cannot read current snapshot")
        {
            slf.update_state_machine(content).await;
        }
        // In memory-only mode the snapshot lives in `snapshot_mem` and starts empty, so there
        // is nothing on disk to read; disk-backed nodes still restore from `data_dir`.
        #[cfg(feature = "in-memory-snapshots")]
        if !in_memory_only
            && let Some((_, content)) = slf
                .read_current_snapshot()
                .await
                .expect("Cannot read current snapshot")
        {
            slf.update_state_machine(content).await;
        }

        Ok(slf)
    }

    /// Serializes the current cache state (caches, locks) into a snapshot blob.
    /// Shared by the disk-backed (default) and in-memory (`in-memory-snapshots`) paths.
    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
    async fn build_snapshot_data(
        &self,
    ) -> Result<(SnapshotMetaOf<TypeConfigCache>, Vec<u8>), io::Error> {
        let data = self.data.read().await;

        // Snapshot consistency: this read lock is held across the whole capture below, while
        // `apply()` takes the write lock. No entry can therefore be applied between reading
        // `last_applied_log_id` and round-tripping the caches/ttls/locks, so the snapshot
        // contains exactly the effects of entries up to that log id; recovery re-applies only
        // later ones (no gap, no double-apply). Do not drop the lock early when extending
        // this function.

        // TODO should we include notifications in snapshots as well?
        //  -> unsure if it makes sense or not

        // One roundtrip per cache: the handler owns values and counters together, so a single
        // SnapshotBuild returns both. Expiries live inside the entries and are rebuilt by the
        // handler on install.
        let named = self
            .cache_map
            .entries()
            .map(|(name, idx)| ((*name).to_string(), &self.tx_caches[*idx]));
        let unknown = data
            .caches_unknown
            .iter()
            .map(|(name, tx)| (name.clone(), tx));
        let mut caches = Vec::with_capacity(self.tx_caches.len() + data.caches_unknown.len());
        for (name, tx) in named.chain(unknown) {
            let (ack, rx) = oneshot::channel();
            tx.send(CacheRequestHandler::SnapshotBuild { reply: ack })
                .expect("cache handler to always be running");
            caches.push((
                name,
                rx.await
                    .expect("to always receive an answer from cache handler"),
            ));
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
            serialize_serde(&locks)
        };
        #[cfg(not(feature = "dlock"))]
        let locks_bytes: Vec<u8> = Vec::default();

        let meta = SnapshotMetaOf::<TypeConfigCache> {
            last_log_id: data.last_applied_log_id,
            last_membership: data.last_membership.clone(),
        };

        let snap: SnapshotDataContent = (meta.clone(), caches, locks_bytes);
        let snapshot_bytes = serialize_serde(&snap);

        Ok((meta, snapshot_bytes))
    }

    /// Persists a serialized snapshot to `data_dir` and cleans up older snapshots.
    /// Returns the path of the persisted snapshot file.
    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
    async fn persist_snapshot(
        &self,
        meta: &SnapshotMetaOf<TypeConfigCache>,
        snapshot_bytes: &[u8],
    ) -> Result<String, io::Error> {
        let name = Self::snapshot_file_name(meta);
        let path = format!("{}/{}", self.path_snapshots, name);
        let path_temp = format!("{path}~");
        {
            // truncate any leftover temp from a crashed previous run
            let mut file = fs::File::create(&path_temp).await?;
            file.write_all(snapshot_bytes).await?;
            file.sync_data().await?;
        }

        atomic_file_switch(path_temp, &path).await?;

        // Cleanup of the old snapshots, kept inline on purpose: it picks files from a directory
        // listing, and `read_current_snapshot` does the same listing before it reads the file it
        // picked. A detached cleaner can delete exactly that file in between, which turns the
        // read into a `NotFound`.
        let id = name;
        let dir = self.path_snapshots.clone();
        let mut entries = fs::read_dir(&dir).await?;
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(err) => {
                    warn!("Error reading directory entries: {err:?}");
                    break;
                }
            };
            let fname = entry.file_name();
            let name = fname.to_str().unwrap_or_default();
            if !name.is_empty()
                && name != id
                // skip the in-flight snapshot receive temp file (`begin_receiving_snapshot`)
                && !(name.ends_with('~') || name.ends_with("tmp") || name.ends_with("temp"))
                && let Err(err) = fs::remove_file(format!("{dir}/{name}")).await
            {
                warn!("Error removing old snapshot {name}: {err:?}");
            }
        }

        Ok(path)
    }

    /// Deserializes a snapshot blob and applies it to the in-memory state.
    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
    async fn apply_snapshot_bytes(
        &self,
        meta: &SnapshotMetaOf<TypeConfigCache>,
        bytes: &[u8],
    ) -> Result<(), io::Error> {
        let (meta_snap, kvs, locks) = deserialize_serde::<SnapshotDataContent>(bytes)
            .map_err(|err| io::Error::other(format!("{err}")))?;
        debug_assert_eq!(meta.last_log_id, meta_snap.last_log_id);
        debug_assert_eq!(meta.last_membership, meta_snap.last_membership);

        self.update_state_machine((meta_snap, kvs, locks)).await;

        Ok(())
    }

    /// Returns the handler of the cache `name`. A name the local enum does not have comes from a
    /// node with a newer enum, during a rolling deploy. Its entries go into a cache without a
    /// variant, which the snapshots carry as well, so this node keeps the same state as the others.
    fn cache<'a>(
        &'a self,
        data: &'a mut StateMachineData,
        name: &str,
    ) -> &'a flume::Sender<CacheRequestHandler> {
        if let Some(idx) = self.cache_map.get(name) {
            return &self.tx_caches[*idx];
        }
        data.caches_unknown
            .entry(name.to_string())
            .or_insert_with(|| {
                warn!(
                    "Cache {name} is not part of this node's CacheVariants enum, keeping its \
                     entries without a variant"
                );
                kv_handler::spawn(Cow::Owned(name.to_string()))
            })
    }

    async fn update_state_machine(&self, content: SnapshotDataContent) {
        let (meta, kvs, locks) = content;

        // make sure to hold the metadata lock the whole time
        let mut data = self.data.write().await;

        // One install per cache: values + counters travel together, mirroring the single
        // roundtrip used in `build_snapshot_data`. Expiries live inside the entries and are
        // rebuilt by the handler on install.
        // A snapshot replaces the whole state, so a local cache it does not carry ends up empty.
        let mut kvs: BTreeMap<String, CacheSnapshot> = kvs.into_iter().collect();
        let local: Vec<String> = self
            .cache_map
            .keys()
            .map(|name| (*name).to_string())
            .chain(data.caches_unknown.keys().cloned())
            .collect();
        for name in local {
            kvs.entry(name).or_default();
        }
        for (name, snapshot) in kvs {
            let (ack, rx) = oneshot::channel();
            self.cache(&mut data, &name)
                .send(CacheRequestHandler::SnapshotInstall { snapshot, ack })
                .expect("cache handler to always be running");
            rx.await
                .expect("to always receive an answer from the cache handler");
        }

        #[cfg(feature = "dlock")]
        {
            let locks: HashMap<String, dlock_handler::LockQueue> =
                deserialize_serde(&locks).unwrap();
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

    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
    pub async fn read_current_snapshot(
        &self,
    ) -> StorageResult<Option<(String, SnapshotDataContent)>> {
        let mut list = tokio::fs::read_dir(&self.path_snapshots).await?;

        let mut latest_ts: Option<i64> = None;
        let mut latest_file_name = None;
        loop {
            let entry = match list.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(err) => {
                    warn!("Error reading directory entries: {err:?}");
                    break;
                }
            };
            let file_name = entry.file_name();
            let name = file_name.to_str().unwrap_or_default();
            if name.ends_with('~') || name.ends_with(".temp") {
                // unfinished snapshots during creation get a trailing `~`;
                // also keep skipping the legacy `.temp` suffix from older versions
                continue;
            }

            let meta = entry.metadata().await?;
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
                    let Some((rest_, log_id_latest)) = last_name.rsplit_once('-') else {
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

        let bytes = fs::read(&path).await?;

        Ok(Some((
            path,
            deserialize_serde::<SnapshotDataContent>(&bytes)
                .map_err(|err| io::Error::other(format!("{err}")))?,
        )))
    }
}

impl RaftStateMachine<TypeConfigCache> for Arc<StateMachineMemory> {
    #[cfg(not(feature = "in-memory-snapshots"))]
    type SnapshotData = fs::File;
    #[cfg(feature = "in-memory-snapshots")]
    type SnapshotData = Cursor<Vec<u8>>;

    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogIdOf<TypeConfigCache>>,
            StoredMembershipOf<TypeConfigCache>,
        ),
        io::Error,
    > {
        let data = self.data.read().await;
        Ok((data.last_applied_log_id, data.last_membership.clone()))
    }

    async fn apply<Strm>(&mut self, mut entries: Strm) -> Result<(), io::Error>
    where
        Strm: Stream<Item = Result<EntryResponder<TypeConfigCache>, io::Error>>
            + Unpin
            + OptionalSend,
    {
        let mut data = self.data.write().await;

        let mut last_applied_log_id = None;
        while let Some(item) = entries.next().await {
            let (entry, responder) = item?;
            last_applied_log_id = Some(entry.log_id);

            // we are using sync sends -> unbounded channels
            let resp_value = match entry.payload {
                EntryPayload::Blank => CacheResponse::Empty,

                EntryPayload::Normal(req) => match req {
                    CacheRequest::Get { .. } => {
                        // `Get` is served locally by the client and never enters the raft log; if it
                        // ever does (hand-crafted wire bytes), we want to fail loudly here rather
                        // than silently drop the entry in favor of data consistency.
                        unreachable!("a CacheRequest::Get should never come through the Raft")
                    }

                    CacheRequest::Put {
                        cache,
                        key,
                        value,
                        expires,
                    } => {
                        // The per-cache handler owns values and expiries together, so the expiry
                        // registration/drop and the value change happen atomically inside one
                        // task: a stale expiry can never remove the freshly put value.
                        self.cache(&mut data, &cache)
                            .send(CacheRequestHandler::Put {
                                key: key.to_string(),
                                value,
                                expires,
                            })
                            .expect("cache handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::GetRemove { cache, key } => {
                        let (ack, rx) = oneshot::channel();
                        self.cache(&mut data, &cache)
                            .send(CacheRequestHandler::GetRemove {
                                key: key.to_string(),
                                reply: ack,
                            })
                            .expect("cache handler to always be running");

                        // The cache handler runs on its own thread per cache and never takes the
                        // state-machine lock, so awaiting its answer while `data` is held is
                        // deadlock-free. The removal is a raft log entry, applied on all nodes.
                        CacheResponse::Value(rx.await.expect("cache handler to always answer"))
                    }

                    CacheRequest::Replace {
                        cache,
                        key,
                        value,
                        expires,
                    } => {
                        // The expiry registration/drop and the value change happen atomically
                        // inside the per-cache handler, so a stale expiry can never remove the
                        // freshly replaced value.
                        let (ack, rx) = oneshot::channel();
                        self.cache(&mut data, &cache)
                            .send(CacheRequestHandler::Replace {
                                key: key.to_string(),
                                value,
                                expires,
                                reply: ack,
                            })
                            .expect("cache handler to always be running");

                        CacheResponse::Value(rx.await.expect("cache handler to always answer"))
                    }

                    CacheRequest::Delete { cache, key } => {
                        self.cache(&mut data, &cache)
                            .send(CacheRequestHandler::Delete {
                                key: key.to_string(),
                            })
                            .expect("cache handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::Clear { cache } => {
                        self.cache(&mut data, &cache)
                            .send(CacheRequestHandler::Clear)
                            .expect("cache handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::ClearCounters { cache } => {
                        #[cfg(feature = "counters")]
                        {
                            self.cache(&mut data, &cache)
                                .send(CacheRequestHandler::ClearCounters)
                                .expect("cache handler to always be running");

                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("ClearCounters requires the `counters` feature")
                    }

                    CacheRequest::ClearAll => {
                        for tx in self.tx_caches.iter().chain(data.caches_unknown.values()) {
                            tx.send(CacheRequestHandler::Clear)
                                .expect("cache handler to always be running");
                            #[cfg(feature = "counters")]
                            tx.send(CacheRequestHandler::ClearCounters)
                                .expect("cache handler to always be running");
                        }

                        CacheResponse::Ok
                    }

                    CacheRequest::Notify(payload) => {
                        #[cfg(feature = "listen_notify")]
                        {
                            self.tx_notify
                                .send(NotifyRequest::Notify(payload))
                                // this channel can never be closed - we have both sides
                                .unwrap();
                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "listen_notify"))]
                        unreachable!("Notify requires the `listen_notify` feature")
                    }

                    CacheRequest::Lock((key, id)) => {
                        #[cfg(feature = "dlock")]
                        {
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
                                    .send(LockRequest::Lock(LockRequestPayload {
                                        key,
                                        log_id,
                                        ack,
                                    }))
                                    // this channel can never be closed - we have both sides
                                    .unwrap();
                            }

                            let state = rx
                                .await
                                .expect("To always get a response from dlock handler");

                            CacheResponse::Lock(state)
                        }
                        #[cfg(not(feature = "dlock"))]
                        unreachable!("Lock requires the `dlock` feature")
                    }

                    CacheRequest::LockAwait(..) => {
                        unreachable!("Lock Awaits should never come through the Raft")
                    }

                    CacheRequest::LockRelease((key, id)) => {
                        #[cfg(feature = "dlock")]
                        {
                            self.tx_dlock
                                .send(LockRequest::Release(LockReleasePayload { key, id }))
                                // this channel can never be closed - we have both sides
                                .unwrap();

                            // we can return early without waiting for answer, release should never fail anyway
                            CacheResponse::Lock(LockState::Released)
                        }
                        #[cfg(not(feature = "dlock"))]
                        unreachable!("LockRelease requires the `dlock` feature")
                    }

                    CacheRequest::LockAlive((key, id)) => {
                        #[cfg(feature = "dlock")]
                        {
                            let (ack, rx) = oneshot::channel();

                            self.tx_dlock
                                .send(LockRequest::Alive(LockAlivePayload { key, id, ack }))
                                // this channel can never be closed - we have both sides
                                .unwrap();

                            let state = rx
                                .await
                                .expect("To always get a response from dlock handler");

                            CacheResponse::Lock(state)
                        }
                        #[cfg(not(feature = "dlock"))]
                        unreachable!("LockAlive requires the `dlock` feature")
                    }

                    CacheRequest::CounterGet { .. } => {
                        unreachable!("a CacheRequest::Get should never come through the Raft")
                    }

                    CacheRequest::CounterSet { cache, key, value } => {
                        #[cfg(feature = "counters")]
                        {
                            self.cache(&mut data, &cache)
                                .send(CacheRequestHandler::CounterSet {
                                    key: key.to_string(),
                                    value,
                                })
                                .expect("cache handler to always be running");

                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("CounterSet requires the `counters` feature")
                    }

                    CacheRequest::CounterAdd { cache, key, value } => {
                        #[cfg(feature = "counters")]
                        {
                            let (ack, rx) = oneshot::channel();

                            self.cache(&mut data, &cache)
                                .send(CacheRequestHandler::CounterAdd {
                                    key: key.to_string(),
                                    delta: value,
                                    reply: ack,
                                })
                                .expect("cache handler to always be running");

                            let v = rx.await.unwrap();
                            CacheResponse::CounterValue(Some(v))
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("CounterAdd requires the `counters` feature")
                    }

                    CacheRequest::CounterDel { cache, key } => {
                        #[cfg(feature = "counters")]
                        {
                            self.cache(&mut data, &cache)
                                .send(CacheRequestHandler::CounterDel {
                                    key: key.to_string(),
                                })
                                .expect("cache handler to always be running");

                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("CounterDel requires the `counters` feature")
                    }
                },

                EntryPayload::Membership(mem) => {
                    data.last_membership =
                        StoredMembershipOf::<TypeConfigCache>::new(Some(entry.log_id), mem);
                    CacheResponse::Empty
                }
            };

            if let Some(responder) = responder {
                responder.send(resp_value);
            }
        }

        data.last_applied_log_id = last_applied_log_id;

        Ok(())
    }

    // openraft declares this trait method `async`, so the impl has to keep `async`; building a
    // snapshot of the in-memory state is a plain `Clone` with nothing to await.
    #[allow(clippy::unused_async_trait_impl)]
    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    #[cfg(not(feature = "in-memory-snapshots"))]
    #[tracing::instrument(skip_all)]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMetaOf<TypeConfigCache>,
        // the received data already lives in the file that got handed to us
        mut snapshot: Self::SnapshotData,
    ) -> Result<(), io::Error> {
        // the receive file (the `~` suffix keeps reads and the cleanup from picking it up)
        let src = format!("{}/temp~", self.path_snapshots);
        let dest = format!(
            "{}/{}",
            self.path_snapshots,
            StateMachineMemory::snapshot_file_name(meta)
        );

        let bytes = if fs::try_exists(&src).await? {
            atomic_file_switch(src, &dest).await?;
            fs::read(&dest).await?
        } else {
            // nothing got streamed into the receive file, so the data is the file we were handed -
            // openraft's storage test suite hands it inline, the runtime never does
            let mut bytes = Vec::new();
            snapshot.read_to_end(&mut bytes).await?;
            fs::write(&dest, &bytes).await?;
            bytes
        };

        self.apply_snapshot_bytes(meta, &bytes).await
    }

    #[cfg(feature = "in-memory-snapshots")]
    #[tracing::instrument(skip_all)]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMetaOf<TypeConfigCache>,
        snapshot: Self::SnapshotData,
    ) -> Result<(), io::Error> {
        let bytes = snapshot.into_inner();

        if self.in_memory_only {
            *self.snapshot_mem.write().await = Some((meta.clone(), bytes.clone()));
        } else {
            let dest = format!(
                "{}/{}",
                self.path_snapshots,
                StateMachineMemory::snapshot_file_name(meta)
            );
            fs::write(&dest, &bytes).await?;
        }

        self.apply_snapshot_bytes(meta, &bytes).await
    }

    #[cfg(not(feature = "in-memory-snapshots"))]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<SnapshotOf<TypeConfigCache, Self::SnapshotData>>, io::Error> {
        match self.read_current_snapshot().await? {
            None => Ok(None),
            Some((path, (meta, ..))) => {
                let file = fs::File::open(path).await?;

                let snapshot = Snapshot {
                    meta,
                    snapshot: file,
                };

                Ok(Some(snapshot))
            }
        }
    }

    #[cfg(feature = "in-memory-snapshots")]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<SnapshotOf<TypeConfigCache, Self::SnapshotData>>, io::Error> {
        if self.in_memory_only {
            let guard = self.snapshot_mem.read().await;
            return Ok(guard.as_ref().map(|(meta, bytes)| Snapshot {
                meta: meta.clone(),
                snapshot: Cursor::new(bytes.clone()),
            }));
        }

        match self.read_current_snapshot().await? {
            None => Ok(None),
            Some((path, (meta, ..))) => {
                let bytes = fs::read(&path).await?;

                let snapshot = Snapshot {
                    meta,
                    snapshot: Cursor::new(bytes),
                };

                Ok(Some(snapshot))
            }
        }
    }
}

#[cfg(all(test, feature = "in-memory-snapshots"))]
mod tests {
    use super::*;
    use crate::CacheVariants;
    use openraft::RaftSnapshotBuilder;
    use openraft::storage::RaftStateMachine;
    use std::sync::Arc;

    #[derive(Debug)]
    enum TestCache {
        One,
    }

    impl CacheVariants for TestCache {
        fn hiqlite_cache_index(&self) -> usize {
            0
        }

        fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
            &[(0, "One")]
        }

        fn hiqlite_cache_map() -> &'static phf::Map<&'static str, usize> {
            static MAP: phf::Map<&'static str, usize> = phf::phf_map! { "One" => 0 };
            &MAP
        }
    }

    /// A pure cache-only node running in-memory (`cache_storage_disk = false`) must never
    /// touch `data_dir`: it does not need to exist or be writable. Snapshots are kept in
    /// memory and are still retrievable for the Raft to stream to other members.
    #[tokio::test(flavor = "multi_thread")]
    async fn in_memory_only_does_not_require_data_dir() {
        let base_dir = std::env::temp_dir().join("hiqlite_inmem_only_no_datadir_test");
        // make sure the path does not exist up-front
        let _ = std::fs::remove_dir_all(&base_dir);
        let base = base_dir.to_str().unwrap();

        let mut sm = Arc::new(
            StateMachineMemory::new::<TestCache>(base, true)
                .await
                .expect("in-memory state machine to start without a data_dir"),
        );

        // nothing may be created on disk in memory-only mode
        assert!(
            !base_dir.exists(),
            "memory-only mode must not create the data_dir"
        );

        // building a snapshot keeps it in memory and still must not write to disk
        let built = sm
            .build_snapshot()
            .await
            .expect("snapshot build to succeed");
        assert!(
            !base_dir.exists(),
            "building a snapshot must not create the data_dir in memory-only mode"
        );

        // the in-memory snapshot is retrievable (what the Raft streams to other members)
        let current = sm
            .get_current_snapshot()
            .await
            .expect("get_current_snapshot to succeed")
            .expect("an in-memory snapshot to be present after building one");
        assert_eq!(current.meta.last_log_id, built.meta.last_log_id);
        assert_eq!(current.meta.last_membership, built.meta.last_membership);

        let _ = std::fs::remove_dir_all(&base_dir);
    }

    /// A leftover unfinished snapshot (a newer `~` temp or a legacy `.temp` from an
    /// older version) must never be picked as the latest snapshot on restore.
    #[tokio::test(flavor = "multi_thread")]
    async fn read_current_snapshot_skips_temp_files() {
        let base_dir = std::env::temp_dir().join("hiqlite_restore_skips_temp_test");
        let _ = std::fs::remove_dir_all(&base_dir);
        let base = base_dir.to_str().unwrap();

        let sm = Arc::new(
            StateMachineMemory::new::<TestCache>(base, false)
                .await
                .expect("state machine to start"),
        );

        // a valid snapshot, then two unfinished ones that would sort as newer
        let (meta, bytes) = sm
            .build_snapshot_data()
            .await
            .expect("snapshot build to succeed");
        let file_name = StateMachineMemory::snapshot_file_name(&meta);
        let valid = format!("{}/{}", sm.path_snapshots, file_name);
        std::fs::write(&valid, &bytes).unwrap();

        let ts = file_name.split_once('-').unwrap().0.parse::<i64>().unwrap();
        let legacy = format!("{}/{}--.temp", sm.path_snapshots, ts + 1);
        let current = format!("{}/{}--~", sm.path_snapshots, ts + 2);
        std::fs::write(&legacy, b"partial snapshot").unwrap();
        std::fs::write(&current, b"partial snapshot").unwrap();

        let restored = sm
            .read_current_snapshot()
            .await
            .expect("restore must not fail on leftover temp files")
            .expect("the valid snapshot to be restored");
        assert_eq!(restored.0, valid);

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}

#[cfg(test)]
mod serialized_enum_order {
    use super::*;

    /// The serialized variant index is part of the raft log format: a reorder
    /// would silently corrupt logs written by older builds with a different
    /// feature set. Pin the current order so a reorder fails this test instead.
    #[test]
    fn cache_request_variant_order_is_stable() {
        let idx = |req: &CacheRequest| crate::helpers::serialize(req).unwrap()[0];
        let key = || Cow::Owned(String::new());

        assert_eq!(
            idx(&CacheRequest::Get {
                cache: "App".into(),
                key: String::new()
            }),
            0
        );
        assert_eq!(
            idx(&CacheRequest::Put {
                cache: "App".into(),
                key: key(),
                value: vec![],
                expires: None
            }),
            1
        );
        assert_eq!(
            idx(&CacheRequest::GetRemove {
                cache: "App".into(),
                key: key()
            }),
            2
        );
        assert_eq!(
            idx(&CacheRequest::Replace {
                cache: "App".into(),
                key: key(),
                value: vec![],
                expires: None
            }),
            3
        );
        assert_eq!(
            idx(&CacheRequest::Delete {
                cache: "App".into(),
                key: key()
            }),
            4
        );
        assert_eq!(
            idx(&CacheRequest::Clear {
                cache: "App".into()
            }),
            5
        );
        assert_eq!(
            idx(&CacheRequest::ClearCounters {
                cache: "App".into()
            }),
            6
        );
        assert_eq!(idx(&CacheRequest::ClearAll), 7);
        assert_eq!(idx(&CacheRequest::Notify((0, vec![]))), 8);
        assert_eq!(idx(&CacheRequest::Lock((key(), None))), 9);
        assert_eq!(idx(&CacheRequest::LockAlive((key(), 0))), 10);
        assert_eq!(idx(&CacheRequest::LockAwait((key(), 0))), 11);
        assert_eq!(idx(&CacheRequest::LockRelease((key(), 0))), 12);
        assert_eq!(
            idx(&CacheRequest::CounterGet {
                cache: "App".into(),
                key: key()
            }),
            13
        );
        assert_eq!(
            idx(&CacheRequest::CounterSet {
                cache: "App".into(),
                key: key(),
                value: 0
            }),
            14
        );
        assert_eq!(
            idx(&CacheRequest::CounterAdd {
                cache: "App".into(),
                key: key(),
                value: 0
            }),
            15
        );
        assert_eq!(
            idx(&CacheRequest::CounterDel {
                cache: "App".into(),
                key: key()
            }),
            16
        );
    }
}

#[cfg(test)]
mod cache_names {
    use super::*;
    use openraft::entry::RaftEntry;
    use openraft::impls::leader_id_adv::LeaderId;

    /// A hand-written `CacheVariants` enum, the way the derive builds it.
    macro_rules! cache_enum {
        ($name:ident { $($variant:ident = $key:literal => $idx:literal),* }) => {
            #[derive(Debug)]
            enum $name {
                $($variant),*
            }

            impl CacheVariants for $name {
                fn hiqlite_cache_index(&self) -> usize {
                    match self {
                        $(Self::$variant => $idx),*
                    }
                }

                fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
                    &[$(($idx, $key)),*]
                }

                fn hiqlite_cache_map() -> &'static phf::Map<&'static str, usize> {
                    static MAP: phf::Map<&'static str, usize> = phf::phf_map! {
                        $($key => $idx),*
                    };
                    &MAP
                }
            }
        };
    }

    cache_enum!(Old { App = "App" => 0 });
    cache_enum!(New { App = "App" => 0, Sessions = "Sessions" => 1, Tokens = "Tokens" => 2 });
    cache_enum!(Reordered { Tokens = "Tokens" => 0, App = "App" => 1, Sessions = "Sessions" => 2 });

    async fn start<C: Debug + CacheVariants>(name: &str) -> Arc<StateMachineMemory> {
        let base_dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&base_dir);
        Arc::new(
            StateMachineMemory::new::<C>(base_dir.to_str().unwrap(), false)
                .await
                .unwrap(),
        )
    }

    fn put(cache: &'static str, key: &'static str, value: &[u8]) -> CacheRequest {
        CacheRequest::Put {
            cache: cache.into(),
            key: key.into(),
            value: value.to_vec(),
            expires: None,
        }
    }

    /// Applies `reqs` as one batch with the log indexes `first..`.
    async fn apply(sm: &mut Arc<StateMachineMemory>, first: u64, reqs: Vec<CacheRequest>) {
        let entries = reqs.into_iter().zip(first..).map(|(req, index)| {
            let log_id = LogIdOf::<TypeConfigCache> {
                leader_id: LeaderId {
                    term: 1,
                    node_id: 1,
                },
                index,
            };
            Ok((Entry::new_normal(log_id, req), None))
        });
        sm.apply(futures_util::stream::iter(entries)).await.unwrap();
    }

    async fn get<C: CacheVariants>(
        sm: &StateMachineMemory,
        cache: C,
        key: &str,
    ) -> Option<Vec<u8>> {
        let (reply, rx) = oneshot::channel();
        sm.tx_caches[cache.hiqlite_cache_index()]
            .send(CacheRequestHandler::Get {
                key: key.to_string(),
                reply,
            })
            .unwrap();
        rx.await.unwrap()
    }

    async fn snapshot(sm: &StateMachineMemory) -> (SnapshotMetaOf<TypeConfigCache>, Vec<u8>) {
        sm.build_snapshot_data().await.unwrap()
    }

    fn snapshot_caches(bytes: &[u8]) -> BTreeMap<String, CacheSnapshot> {
        deserialize_serde::<SnapshotDataContent>(bytes)
            .unwrap()
            .1
            .into_iter()
            .collect()
    }

    #[test]
    fn derived_map_finds_every_variant() {
        assert_eq!(New::hiqlite_cache_map().get("Tokens"), Some(&2));
        assert_eq!(New::Sessions.hiqlite_cache_name(), "Sessions");
        assert_eq!(Old::hiqlite_cache_map().get("Tokens"), None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_keeps_entries_of_a_cache_this_node_does_not_name() {
        let mut old = start::<Old>("hiqlite_cache_names_apply").await;
        apply(
            &mut old,
            1,
            vec![
                put("App", "a", b"0"),
                put("Tokens", "t", b"2"),
                put("Tokens", "gone", b"x"),
            ],
        )
        .await;
        apply(
            &mut old,
            4,
            vec![CacheRequest::Delete {
                cache: "Tokens".into(),
                key: "gone".into(),
            }],
        )
        .await;
        assert_eq!(
            get(&old, Old::App, "a").await.as_deref(),
            Some(b"0".as_slice())
        );

        let (meta, bytes) = snapshot(&old).await;
        assert_eq!(meta.last_log_id.unwrap().index, 4);
        let caches = snapshot_caches(&bytes);
        assert_eq!(caches.keys().collect::<Vec<_>>(), vec!["App", "Tokens"]);
        assert_eq!(caches["Tokens"].0.keys().collect::<Vec<_>>(), vec!["t"]);

        // A node with the newer enum installs that snapshot and finds the value under its
        // variant.
        let new = start::<New>("hiqlite_cache_names_apply_new").await;
        new.apply_snapshot_bytes(&meta, &bytes).await.unwrap();
        assert_eq!(
            get(&new, New::Tokens, "t").await.as_deref(),
            Some(b"2".as_slice())
        );
        assert_eq!(
            get(&new, New::App, "a").await.as_deref(),
            Some(b"0".as_slice())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_with_unknown_caches_installs_and_is_handed_on() {
        let mut new = start::<New>("hiqlite_cache_names_install_new").await;
        apply(
            &mut new,
            1,
            vec![put("App", "a", b"0"), put("Sessions", "s", b"1")],
        )
        .await;
        let (meta, bytes) = snapshot(&new).await;

        let mut old = start::<Old>("hiqlite_cache_names_install_old").await;
        old.apply_snapshot_bytes(&meta, &bytes).await.unwrap();
        assert_eq!(
            get(&old, Old::App, "a").await.as_deref(),
            Some(b"0".as_slice())
        );
        let (_, rebuilt) = snapshot(&old).await;
        assert_eq!(snapshot_caches(&rebuilt), snapshot_caches(&bytes));

        // `ClearAll` empties the unknown caches too.
        apply(&mut old, 3, vec![CacheRequest::ClearAll]).await;
        let (_, cleared) = snapshot(&old).await;
        assert!(
            snapshot_caches(&cleared)
                .values()
                .all(|(v, c)| v.is_empty() && c.is_empty())
        );
    }

    /// The variant order is local to a node: a snapshot of a node with a different order lands
    /// every cache under the same name.
    #[tokio::test(flavor = "multi_thread")]
    async fn reordered_enum_installs_by_name() {
        let mut new = start::<New>("hiqlite_cache_names_reorder_new").await;
        apply(
            &mut new,
            1,
            vec![put("App", "a", b"0"), put("Tokens", "t", b"2")],
        )
        .await;
        let (meta, bytes) = snapshot(&new).await;

        let reordered = start::<Reordered>("hiqlite_cache_names_reorder").await;
        reordered.apply_snapshot_bytes(&meta, &bytes).await.unwrap();
        assert_eq!(
            get(&reordered, Reordered::App, "a").await.as_deref(),
            Some(b"0".as_slice())
        );
        assert_eq!(
            get(&reordered, Reordered::Tokens, "t").await.as_deref(),
            Some(b"2".as_slice())
        );
        assert_eq!(get(&reordered, Reordered::Sessions, "a").await, None);
    }

    /// Installing a snapshot replaces the whole state: a cache the snapshot does not carry is
    /// empty afterwards.
    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_with_fewer_caches_empties_the_missing_ones() {
        let old = start::<Old>("hiqlite_cache_names_fewer_old").await;
        let (meta, bytes) = snapshot(&old).await;

        let mut new = start::<New>("hiqlite_cache_names_fewer_new").await;
        apply(
            &mut new,
            1,
            vec![put("App", "a", b"0"), put("Tokens", "t", b"2")],
        )
        .await;
        new.apply_snapshot_bytes(&meta, &bytes).await.unwrap();
        assert_eq!(get(&new, New::App, "a").await, None);
        assert_eq!(get(&new, New::Tokens, "t").await, None);
    }
}
