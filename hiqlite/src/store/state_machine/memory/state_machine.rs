use crate::helpers::{deserialize, serialize, set_path_access};
use crate::store::StorageResult;
use crate::store::state_machine::memory::kv_handler::{CacheRequestHandler, CacheSnapshot};
use crate::store::state_machine::memory::{TypeConfigKV, kv_handler};
use crate::{CacheVariants, Error, Node, NodeId};
use chrono::Utc;
use cryptr::utils::secure_random_alnum;
use dotenvy::var;
use openraft::storage::RaftStateMachine;
use openraft::{
    EntryPayload, LogId, OptionalSend, RaftSnapshotBuilder, Snapshot, SnapshotMeta, StorageError,
    StorageIOError, StoredMembership,
};
use rust_decimal::prelude::ToPrimitive;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
#[cfg(feature = "in-memory-snapshots")]
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock, oneshot};
use tokio::task;
#[cfg(not(feature = "in-memory-snapshots"))]
use tracing::info;
use tracing::warn;
use uuid::Uuid;

#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::{self, *};
#[cfg(feature = "listen_notify_local")]
use crate::store::state_machine::memory::notify_handler::{self, NotifyRequest};

type Entry = openraft::Entry<TypeConfigKV>;
#[cfg(not(feature = "in-memory-snapshots"))]
type SnapshotData = fs::File;
#[cfg(feature = "in-memory-snapshots")]
type SnapshotData = Cursor<Vec<u8>>;

type SnapshotKVs = Vec<CacheSnapshot>;
type SnapshotLocks = Vec<u8>;
type SnapshotDataContent = (SnapshotMeta<NodeId, Node>, SnapshotKVs, SnapshotLocks);
/// The latest snapshot kept in memory (`meta` + serialized bytes) for memory-only mode.
#[cfg(feature = "in-memory-snapshots")]
type MemSnapshot = (SnapshotMeta<NodeId, Node>, Vec<u8>);

// The variant order is part of the raft log format and must stay stable and
// feature-independent: adding variants changes the serialized indices of
// everything after them, which silently corrupts logs written with a different
// feature set. New variants therefore go at the end of the enum.
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
    GetRemove {
        cache_idx: usize,
        key: Cow<'static, str>,
    },
    Replace {
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
    #[allow(dead_code)] // only constructed with the `counters` feature
    ClearCounters {
        cache_idx: usize,
    },
    ClearAll,
    #[allow(dead_code)] // only constructed with the `listen_notify_local` feature
    Notify((i64, Vec<u8>)),
    #[allow(dead_code)] // only constructed with the `dlock` feature
    Lock((Cow<'static, str>, Option<u64>)),
    #[allow(dead_code)] // only constructed with the `dlock` feature
    LockAwait((Cow<'static, str>, u64)),
    #[allow(dead_code)] // only constructed with the `dlock` feature
    LockRelease((Cow<'static, str>, u64)),
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterGet {
        cache_idx: usize,
        key: Cow<'static, str>,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterSet {
        cache_idx: usize,
        key: Cow<'static, str>,
        value: i64,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterAdd {
        cache_idx: usize,
        key: Cow<'static, str>,
        value: i64,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
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

    #[cfg(feature = "listen_notify_local")]
    pub(crate) tx_notify: flume::Sender<NotifyRequest>,
    #[cfg(feature = "listen_notify_local")]
    pub(crate) rx_notify: flume::Receiver<(i64, Vec<u8>)>,

    #[cfg(feature = "dlock")]
    pub(crate) tx_dlock: flume::Sender<LockRequest>,
}

impl RaftSnapshotBuilder<TypeConfigKV> for Arc<StateMachineMemory> {
    #[cfg(not(feature = "in-memory-snapshots"))]
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigKV>, StorageError<NodeId>> {
        let (meta, snapshot_bytes) = self.build_snapshot_data().await?;
        let path = self.persist_snapshot(&meta, &snapshot_bytes).await?;

        // Stream the snapshot straight from the persisted file (zero-copy).
        let file = fs::File::open(&path)
            .await
            .map_err(|err| StorageIOError::read_state_machine(&err))?;
        Ok(Snapshot {
            meta,
            snapshot: Box::new(file),
        })
    }

    #[cfg(feature = "in-memory-snapshots")]
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfigKV>, StorageError<NodeId>> {
        let (meta, snapshot_bytes) = self.build_snapshot_data().await?;

        // In memory-only mode we keep the snapshot in memory and never touch `data_dir`,
        // so a pure cache-only node does not require it to exist or be writable.
        if self.in_memory_only {
            *self.snapshot_mem.write().await = Some((meta.clone(), snapshot_bytes.clone()));
            return Ok(Snapshot {
                meta,
                snapshot: Box::new(Cursor::new(snapshot_bytes)),
            });
        }

        // Disk-backed: persist exactly like the default path, but stream from memory.
        self.persist_snapshot(&meta, &snapshot_bytes).await?;
        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(snapshot_bytes)),
        })
    }
}

impl StateMachineMemory {
    pub(crate) async fn new<C>(base_path: &str, in_memory_only: bool) -> Result<Self, Error>
    where
        C: Debug + CacheVariants,
    {
        let path_sm = format!("{base_path}/state_machine_cache");
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

        // Validate the cache indices at setup time so `apply()` can rely on `.get(idx)`
        // never failing: the indices must be exactly 0..len (no gaps, no duplicates),
        // otherwise a request for a missing index would panic the state machine deep
        // inside raft apply.
        {
            let mut seen = vec![false; variants.len()];
            for &(idx, _) in variants {
                if idx >= variants.len() || seen[idx] {
                    panic!(
                        "cache variant index {idx} is out of range or duplicated \
                         (expected exactly 0..{})",
                        variants.len()
                    );
                }
                seen[idx] = true;
            }
        }

        // Cross-restart cache-index compatibility check. The cache index is encoded
        // positionally in persisted snapshots and in Raft log entries, so a re-order,
        // insert-in-between, removal or rename of the enum would silently install data
        // into the wrong cache. We persist the current enum's normalized form to
        // `cache_index.meta` and compare it against any existing file on every startup:
        // only a pure expansion at the end is allowed. This is only meaningful when data
        // is actually persisted to disk (`!in_memory_only`).
        if !in_memory_only {
            let path_meta = format!("{path_sm}/cache_index.meta");
            match fs::read_to_string(&path_meta).await {
                Ok(stored) => {
                    if !C::hiqlite_cache_compatible_with(&stored) {
                        return Err(Error::Cache(
                            format!(
                                "cache index enum is incompatible with previously persisted \
                                 data ({}):\n--- stored ---\n{}--- current ---\n{}",
                                path_meta,
                                stored,
                                C::hiqlite_cache_variants_normalized()
                            )
                            .into(),
                        ));
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(Error::Cache(
                        format!("cannot read cache index metadata {path_meta}: {err}").into(),
                    ))
                }
            }

            // (Re)write the normalized form atomically so the file always reflects the
            // current enum and a torn write can never leave a half-written fingerprint.
            let normalized = C::hiqlite_cache_variants_normalized();
            let path_temp = format!("{path_sm}/cache_index.meta~");
            {
                let mut file = fs::File::create(&path_temp)
                    .await
                    .map_err(|err| Error::Cache(format!("cannot create {path_temp}: {err}").into()))?;
                file.write_all(normalized.as_bytes())
                    .await
                    .map_err(|err| Error::Cache(format!("cannot write {path_temp}: {err}").into()))?;
                file.sync_data()
                    .await
                    .map_err(|err| Error::Cache(format!("cannot sync {path_temp}: {err}").into()))?;
            }
            fs::rename(&path_temp, &path_meta)
                .await
                .map_err(|err| {
                    Error::Cache(format!("cannot rename {path_temp} to {path_meta}: {err}").into())
                })?;
        }

        let mut tx_caches = Vec::with_capacity(variants.len());
        for (_, name) in variants {
            tx_caches.push(kv_handler::spawn(name));
        }

        #[cfg(feature = "dlock")]
        let tx_dlock = dlock_handler::spawn();

        #[cfg(feature = "listen_notify_local")]
        let (tx_notify, rx_notify) = notify_handler::spawn();

        let slf = Self {
            data: RwLock::new(StateMachineData::default()),
            path_snapshots,
            #[cfg(feature = "in-memory-snapshots")]
            in_memory_only,
            #[cfg(feature = "in-memory-snapshots")]
            snapshot_mem: RwLock::new(None),
            tx_caches,
            #[cfg(feature = "listen_notify_local")]
            tx_notify,
            #[cfg(feature = "listen_notify_local")]
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
    ) -> Result<(SnapshotMeta<NodeId, Node>, Vec<u8>), StorageError<NodeId>> {
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
        let mut caches = Vec::with_capacity(self.tx_caches.len());
        for tx in &self.tx_caches {
            let (ack, rx) = oneshot::channel();
            tx.send(CacheRequestHandler::SnapshotBuild { reply: ack })
                .expect("kv handler to always be running");
            caches.push(
                rx.await
                    .expect("to always receive an answer from kv handler"),
            );
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

        let snap: SnapshotDataContent = (meta.clone(), caches, locks_bytes);
        let snapshot_bytes =
            serialize(&snap).map_err(|err| StorageIOError::write_state_machine(&err))?;

        Ok((meta, snapshot_bytes))
    }

    /// Persists a serialized snapshot to `data_dir` and spawns cleanup of older snapshots.
    /// Returns the path of the persisted snapshot file.
    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
    async fn persist_snapshot(
        &self,
        meta: &SnapshotMeta<NodeId, Node>,
        snapshot_bytes: &[u8],
    ) -> Result<String, StorageError<NodeId>> {
        let path = format!("{}/{}", self.path_snapshots, meta.snapshot_id);
        let path_temp = format!("{path}~");
        {
            // truncate any leftover temp from a crashed previous run
            let mut file = fs::File::create(&path_temp)
                .await
                .map_err(|err| StorageIOError::write_state_machine(&err))?;
            file.write_all(snapshot_bytes)
                .await
                .map_err(|err| StorageIOError::write_state_machine(&err))?;
            file.sync_data()
                .await
                .map_err(|err| StorageIOError::write_state_machine(&err))?;
        }

        // atomic move: a crash can never leave a partially written snapshot at the final path
        fs::rename(&path_temp, &path)
            .await
            .map_err(|err| StorageIOError::write_state_machine(&err))?;

        // cleanup task for old snapshots
        let id = meta.snapshot_id.clone();
        let dir = self.path_snapshots.clone();
        task::spawn(async move {
            let mut entries = fs::read_dir(&dir).await.unwrap();
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
                    // skip the in-flight snapshot receive temp file (`begin_receiving_snapshot`)
                    && name != "temp~"
                    && name != id
                    && let Err(err) = fs::remove_file(format!("{dir}/{name}")).await
                {
                    warn!("Error removing old snapshot {name}: {err:?}");
                }
            }
        });

        Ok(path)
    }

    /// Deserializes a snapshot blob and applies it to the in-memory state.
    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
    async fn apply_snapshot_bytes(
        &self,
        meta: &SnapshotMeta<NodeId, Node>,
        bytes: &[u8],
    ) -> Result<(), StorageError<NodeId>> {
        let (meta_snap, kvs, locks) = deserialize::<SnapshotDataContent>(bytes)
            .map_err(|e| StorageIOError::read_snapshot(Some(meta.signature()), &e))?;
        debug_assert_eq!(meta.snapshot_id, meta_snap.snapshot_id);
        debug_assert_eq!(meta.last_log_id, meta_snap.last_log_id);
        debug_assert_eq!(meta.last_membership, meta_snap.last_membership);

        self.update_state_machine((meta_snap, kvs, locks)).await;

        Ok(())
    }

    async fn update_state_machine(&self, content: SnapshotDataContent) {
        let (meta, kvs, locks) = content;

        // make sure to hold the metadata lock the whole time
        let mut data = self.data.write().await;

        // One install per cache: values + counters travel together, mirroring the single
        // roundtrip used in `build_snapshot_data`. Expiries live inside the entries and are
        // rebuilt by the handler on install.
        for (idx, snapshot) in kvs.into_iter().enumerate() {
            let (ack, rx) = oneshot::channel();
            self.tx_caches
                .get(idx)
                .unwrap()
                .send(CacheRequestHandler::SnapshotInstall { snapshot, ack })
                .expect("kv handler to always be running");
            rx.await
                .expect("to always receive an answer from the kv handler");
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

    // The error type is huge, but defined by the openraft trait.
    #[allow(clippy::result_large_err)]
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

            // we are using sync sends -> unbounded channels. Every `cache_idx` below is
            // validated at setup time (see `new()`) and again at the API boundary
            // (`network/api.rs`), so `.get(idx).unwrap()` cannot fail on valid requests.
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
                        cache_idx,
                        key,
                        value,
                        expires,
                    } => {
                        // The per-cache handler owns values and expiries together, so the expiry
                        // registration/drop and the value change happen atomically inside one
                        // task: a stale expiry can never remove the freshly put value.
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Put {
                                key: key.to_string(),
                                value,
                                expires,
                            })
                            .expect("kv handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::GetRemove { cache_idx, key } => {
                        let (ack, rx) = oneshot::channel();
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::GetRemove {
                                key: key.to_string(),
                                reply: ack,
                            })
                            .expect("kv handler to always be running");

                        // The kv handler runs on its own thread per cache and never takes the
                        // state-machine lock, so awaiting its answer while `data` is held is
                        // deadlock-free. The removal is a raft log entry, applied on all nodes.
                        CacheResponse::Value(rx.await.expect("kv handler to always answer"))
                    }

                    CacheRequest::Replace {
                        cache_idx,
                        key,
                        value,
                        expires,
                    } => {
                        // The expiry registration/drop and the value change happen atomically
                        // inside the per-cache handler, so a stale expiry can never remove the
                        // freshly replaced value.
                        let (ack, rx) = oneshot::channel();
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Replace {
                                key: key.to_string(),
                                value,
                                expires,
                                reply: ack,
                            })
                            .expect("kv handler to always be running");

                        CacheResponse::Value(rx.await.expect("kv handler to always answer"))
                    }

                    CacheRequest::Delete { cache_idx, key } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Delete {
                                key: key.to_string(),
                            })
                            .expect("kv handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::Clear { cache_idx } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Clear)
                            .expect("kv handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::ClearCounters { cache_idx } => {
                        #[cfg(feature = "counters")]
                        {
                            self.tx_caches
                                .get(cache_idx)
                                .unwrap()
                                .send(CacheRequestHandler::ClearCounters)
                                .expect("kv handler to always be running");

                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("ClearCounters requires the `counters` feature")
                    }

                    CacheRequest::ClearAll => {
                        for tx in &self.tx_caches {
                            tx.send(CacheRequestHandler::Clear)
                                .expect("kv handler to always be running");
                            #[cfg(feature = "counters")]
                            tx.send(CacheRequestHandler::ClearCounters)
                                .expect("kv handler to always be running");
                        }

                        CacheResponse::Ok
                    }

                    CacheRequest::Notify(payload) => {
                        #[cfg(feature = "listen_notify_local")]
                        {
                            self.tx_notify
                                .send(NotifyRequest::Notify(payload))
                                // this channel can never be closed - we have both sides
                                .unwrap();
                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "listen_notify_local"))]
                        unreachable!("Notify requires the `listen_notify_local` feature")
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

                    CacheRequest::CounterGet { .. } => {
                        unreachable!("a CacheRequest::Get should never come through the Raft")
                    }

                    CacheRequest::CounterSet {
                        cache_idx,
                        key,
                        value,
                    } => {
                        #[cfg(feature = "counters")]
                        {
                            self.tx_caches
                                .get(cache_idx)
                                .unwrap()
                                .send(CacheRequestHandler::CounterSet {
                                    key: key.to_string(),
                                    value,
                                })
                                .expect("kv handler to always be running");

                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("CounterSet requires the `counters` feature")
                    }

                    CacheRequest::CounterAdd {
                        cache_idx,
                        key,
                        value,
                    } => {
                        #[cfg(feature = "counters")]
                        {
                            let (ack, rx) = oneshot::channel();

                            self.tx_caches
                                .get(cache_idx)
                                .unwrap()
                                .send(CacheRequestHandler::CounterAdd {
                                    key: key.to_string(),
                                    delta: value,
                                    reply: ack,
                                })
                                .expect("kv handler to always be running");

                            let v = rx.await.unwrap();
                            CacheResponse::CounterValue(Some(v))
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("CounterAdd requires the `counters` feature")
                    }

                    CacheRequest::CounterDel { cache_idx, key } => {
                        #[cfg(feature = "counters")]
                        {
                            self.tx_caches
                                .get(cache_idx)
                                .unwrap()
                                .send(CacheRequestHandler::CounterDel {
                                    key: key.to_string(),
                                })
                                .expect("kv handler to always be running");

                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("CounterDel requires the `counters` feature")
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

    #[cfg(not(feature = "in-memory-snapshots"))]
    #[tracing::instrument(skip_all)]
    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<SnapshotData>, StorageError<NodeId>> {
        // `~`-suffixed name: both `read_current_snapshot` and the snapshot cleanup skip those,
        // so an in-flight receive is never mistaken for a real (complete) snapshot.
        let path = format!("{}/temp~", self.path_snapshots);
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

    #[cfg(feature = "in-memory-snapshots")]
    #[tracing::instrument(skip_all)]
    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<SnapshotData>, StorageError<NodeId>> {
        // The incoming snapshot is streamed into memory. For disk-backed nodes it is
        // persisted in `install_snapshot`; for memory-only nodes it never hits disk.
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    #[cfg(not(feature = "in-memory-snapshots"))]
    #[tracing::instrument(skip_all)]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, Node>,
        // the streamed data already lives in the temp file created by `begin_receiving_snapshot`
        _snapshot: Box<SnapshotData>,
    ) -> Result<(), StorageError<NodeId>> {
        // the temp file created by `begin_receiving_snapshot` (see its `~` naming comment)
        let src = format!("{}/temp~", self.path_snapshots);
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

        self.apply_snapshot_bytes(meta, &bytes).await
    }

    #[cfg(feature = "in-memory-snapshots")]
    #[tracing::instrument(skip_all)]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, Node>,
        snapshot: Box<SnapshotData>,
    ) -> Result<(), StorageError<NodeId>> {
        let bytes = (*snapshot).into_inner();

        if self.in_memory_only {
            *self.snapshot_mem.write().await = Some((meta.clone(), bytes.clone()));
        } else {
            let dest = format!("{}/{}", self.path_snapshots, meta.snapshot_id);
            fs::write(&dest, &bytes)
                .await
                .map_err(|err| StorageError::IO {
                    source: StorageIOError::write(&err),
                })?;
        }

        self.apply_snapshot_bytes(meta, &bytes).await
    }

    #[cfg(not(feature = "in-memory-snapshots"))]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigKV>>, StorageError<NodeId>> {
        match self.read_current_snapshot().await? {
            None => Ok(None),
            Some((path, (meta, ..))) => {
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

    #[cfg(feature = "in-memory-snapshots")]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfigKV>>, StorageError<NodeId>> {
        if self.in_memory_only {
            let guard = self.snapshot_mem.read().await;
            return Ok(guard.as_ref().map(|(meta, bytes)| Snapshot {
                meta: meta.clone(),
                snapshot: Box::new(Cursor::new(bytes.clone())),
            }));
        }

        match self.read_current_snapshot().await? {
            None => Ok(None),
            Some((path, (meta, ..))) => {
                let bytes = fs::read(&path).await.map_err(|err| StorageError::IO {
                    source: StorageIOError::read(&err),
                })?;

                let snapshot = Snapshot {
                    meta,
                    snapshot: Box::new(Cursor::new(bytes)),
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
        assert_eq!(current.meta.snapshot_id, built.meta.snapshot_id);

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
        let valid = format!("{}/{}", sm.path_snapshots, meta.snapshot_id);
        std::fs::write(&valid, &bytes).unwrap();

        let ts = meta
            .snapshot_id
            .split_once('-')
            .unwrap()
            .0
            .parse::<i64>()
            .unwrap();
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

    /// A disk-backed state machine must refuse to start when the persisted cache-index
    /// fingerprint no longer matches the current enum (a rename here). This is the startup
    /// safeguard that catches re-order / insert-in-between / removal / rename of the
    /// `CacheVariants` enum across restarts; a pure expansion at the end is still accepted.
    #[tokio::test(flavor = "multi_thread")]
    async fn startup_rejects_incompatible_cache_index() {
        let base_dir = std::env::temp_dir().join("hiqlite_cache_index_compat_test");
        let _ = std::fs::remove_dir_all(&base_dir);
        let base = base_dir.to_str().unwrap();

        // TestCache's current enum is a single variant at index 0 named "One". A stored
        // fingerprint claiming index 0 is something else is a rename -> incompatible.
        std::fs::create_dir_all(format!("{base}/state_machine_cache")).unwrap();
        std::fs::write(
            format!("{base}/state_machine_cache/cache_index.meta"),
            "0 SomethingElse\n",
        )
        .unwrap();

        let res = StateMachineMemory::new::<TestCache>(base, false).await;
        assert!(
            matches!(res, Err(Error::Cache(_))),
            "expected an incompatible cache index to be rejected at startup, got {res:?}"
        );

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
                cache_idx: 0,
                key: String::new()
            }),
            0
        );
        assert_eq!(
            idx(&CacheRequest::Put {
                cache_idx: 0,
                key: key(),
                value: vec![],
                expires: None
            }),
            1
        );
        assert_eq!(
            idx(&CacheRequest::GetRemove {
                cache_idx: 0,
                key: key()
            }),
            2
        );
        assert_eq!(
            idx(&CacheRequest::Replace {
                cache_idx: 0,
                key: key(),
                value: vec![],
                expires: None
            }),
            3
        );
        assert_eq!(
            idx(&CacheRequest::Delete {
                cache_idx: 0,
                key: key()
            }),
            4
        );
        assert_eq!(idx(&CacheRequest::Clear { cache_idx: 0 }), 5);
        assert_eq!(idx(&CacheRequest::ClearCounters { cache_idx: 0 }), 6);
        assert_eq!(idx(&CacheRequest::ClearAll), 7);
        assert_eq!(idx(&CacheRequest::Notify((0, vec![]))), 8);
        assert_eq!(idx(&CacheRequest::Lock((key(), None))), 9);
        assert_eq!(idx(&CacheRequest::LockAwait((key(), 0))), 10);
        assert_eq!(idx(&CacheRequest::LockRelease((key(), 0))), 11);
        assert_eq!(
            idx(&CacheRequest::CounterGet {
                cache_idx: 0,
                key: key()
            }),
            12
        );
        assert_eq!(
            idx(&CacheRequest::CounterSet {
                cache_idx: 0,
                key: key(),
                value: 0
            }),
            13
        );
        assert_eq!(
            idx(&CacheRequest::CounterAdd {
                cache_idx: 0,
                key: key(),
                value: 0
            }),
            14
        );
        assert_eq!(
            idx(&CacheRequest::CounterDel {
                cache_idx: 0,
                key: key()
            }),
            15
        );
    }
}

#[cfg(test)]
mod cache_index_compatibility {
    use crate::CacheVariants;

    /// A fixed "current" enum used to exercise the compatibility check against a variety of
    /// previously-persisted (stored) index maps. Only the associated functions are used, so
    /// `hiqlite_cache_index` is never called at runtime here.
    #[derive(Debug)]
    enum Cur {
        App,
        AuthCodes,
        Users,
        MagicLinks,
    }

    impl CacheVariants for Cur {
        fn hiqlite_cache_index(&self) -> usize {
            match self {
                Self::App => 0,
                Self::AuthCodes => 1,
                Self::Users => 2,
                Self::MagicLinks => 3,
            }
        }

        fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
            &[(0, "App"), (1, "AuthCodes"), (2, "Users"), (3, "MagicLinks")]
        }
    }

    #[test]
    fn normalized_form_is_stable_and_self_compatible() {
        let normalized = Cur::hiqlite_cache_variants_normalized();
        assert_eq!(normalized, "0 App\n1 AuthCodes\n2 Users\n3 MagicLinks\n");
        // the on-disk fingerprint is always compatible with itself
        assert!(Cur::hiqlite_cache_compatible_with(&normalized));
    }

    #[test]
    fn expansion_at_the_end_is_allowed() {
        // data was written when only the first three variants existed; `MagicLinks` was
        // added at the end later -> every stored index still maps to the same name.
        assert!(Cur::hiqlite_cache_compatible_with("0 App\n1 AuthCodes\n2 Users\n"));
    }

    #[test]
    fn reorder_is_incompatible() {
        // `Users` moved to index 1, pushing `AuthCodes` down -> data at index 1 would be
        // installed into the wrong cache.
        assert!(!Cur::hiqlite_cache_compatible_with(
            "0 App\n1 Users\n2 AuthCodes\n3 MagicLinks\n"
        ));
    }

    #[test]
    fn insert_in_between_is_incompatible() {
        // an older enum where `Users` sat at index 1 (no `AuthCodes` yet) -> inserting a
        // variant in the middle shifts every following index.
        assert!(!Cur::hiqlite_cache_compatible_with("0 App\n1 Users\n2 MagicLinks\n"));
    }

    #[test]
    fn removal_or_rename_is_incompatible() {
        // a stored index that no longer exists (out of range for the current enum) ...
        assert!(!Cur::hiqlite_cache_compatible_with("0 App\n4 Sessions\n"));
        // ... or the same index now carrying a different variant name.
        assert!(!Cur::hiqlite_cache_compatible_with(
            "0 App\n1 Tokens\n2 Users\n3 MagicLinks\n"
        ));
    }

    #[test]
    fn malformed_stored_lines_are_incompatible() {
        // no space -> cannot split into index + name
        assert!(!Cur::hiqlite_cache_compatible_with("App\n"));
        // non-numeric index
        assert!(!Cur::hiqlite_cache_compatible_with("x App\n"));
    }
}
