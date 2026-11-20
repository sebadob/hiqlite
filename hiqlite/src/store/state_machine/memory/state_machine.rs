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
use crate::store::state_machine::memory::dlock_handler::{self, *};
#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::{self, NotifyRequest};
#[cfg(feature = "in-memory-snapshots")]
use std::io::Cursor;

type Entry = EntryOf<TypeConfigCache>;

type SnapshotKVs = Vec<CacheSnapshot>;
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
        cache_idx: usize,
        key: String,
    },
    Put {
        cache_idx: usize,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: Vec<u8>,
        expires: Option<i64>,
    },
    GetRemove {
        cache_idx: usize,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
    },
    Replace {
        cache_idx: usize,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: Vec<u8>,
        expires: Option<i64>,
    },
    Delete {
        cache_idx: usize,
        #[bincode(with_serde)]
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
        cache_idx: usize,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterSet {
        cache_idx: usize,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: i64,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterAdd {
        cache_idx: usize,
        #[bincode(with_serde)]
        key: Cow<'static, str>,
        value: i64,
    },
    #[allow(dead_code)] // only constructed with the `counters` feature
    CounterDel {
        cache_idx: usize,
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

        // Validate the cache indices at setup time so `apply()` can rely on `.get(idx)`
        // never failing: the indices must be exactly 0..len (no gaps, no duplicates),
        // otherwise a request for a missing index would panic the state machine deep
        // inside raft apply.
        {
            let mut seen = vec![false; variants.len()];
            for &(idx, _) in variants {
                if idx >= variants.len() || seen[idx] {
                    panic!(
                        "cache variant index {} is out of range or duplicated \
                         (expected exactly 0..{})",
                        idx,
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
        // only a pure expansion at the end is allowed.
        #[cfg(not(feature = "in-memory-snapshots"))]
        {
            let path_meta = format!("{path_sm}/cache_index.meta");

            // `HQL_CACHE_META_RESET=true` deletes the stored fingerprint and re-initializes it
            // from the current enum. That is the way out of the startup refusal after a rename.
            // Removing a variant needs more: the app must clear all of its caches at startup,
            // because the persisted entries of the removed index are still in there.
            let meta_reset = std::env::var("HQL_CACHE_META_RESET")
                .as_deref()
                .unwrap_or("false")
                .parse::<bool>()
                .expect("Cannot parse HQL_CACHE_META_RESET as bool");

            let stored = if meta_reset {
                warn!("Resetting the cache index metadata at {path_meta}");
                let _ = fs::remove_file(&path_meta).await;
                String::default()
            } else {
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
                        stored
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::default(),
                    Err(err) => {
                        return Err(Error::Cache(
                            format!("cannot read cache index metadata {path_meta}: {err}").into(),
                        ));
                    }
                }
            };

            // (Re)write the normalized form atomically so the file always reflects the
            // current enum and a torn write can never leave a half-written fingerprint.
            // A missing file is always (re)written, even when the fingerprint itself is
            // empty: an enum with zero variants (the `hiqlite` server binary) produces the
            // same normalized form as a fresh startup, so it must create the file too.
            let normalized = C::hiqlite_cache_variants_normalized();
            if !fs::try_exists(&path_meta).await? || stored.as_bytes() != normalized.as_bytes() {
                let path_temp = format!("{path_meta}~");
                {
                    let mut file = fs::File::create(&path_temp).await.map_err(|err| {
                        Error::Cache(format!("cannot create {path_temp}: {err}").into())
                    })?;
                    file.write_all(normalized.as_bytes()).await.map_err(|err| {
                        Error::Cache(format!("cannot write {path_temp}: {err}").into())
                    })?;
                }
                atomic_file_switch(path_temp, path_meta).await?;
            }
        }

        let mut tx_caches = Vec::with_capacity(variants.len());
        for (_, name) in variants {
            tx_caches.push(kv_handler::spawn(name));
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
        let mut caches = Vec::with_capacity(self.tx_caches.len());
        for tx in &self.tx_caches {
            let (ack, rx) = oneshot::channel();
            tx.send(CacheRequestHandler::SnapshotBuild { reply: ack })
                .expect("cache handler to always be running");
            caches.push(
                rx.await
                    .expect("to always receive an answer from cache handler"),
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
            serialize_serde(&locks).unwrap()
        };
        #[cfg(not(feature = "dlock"))]
        let locks_bytes: Vec<u8> = Vec::default();

        let meta = SnapshotMetaOf::<TypeConfigCache> {
            last_log_id: data.last_applied_log_id,
            last_membership: data.last_membership.clone(),
        };

        let snap: SnapshotDataContent = (meta.clone(), caches, locks_bytes);
        let snapshot_bytes =
            serialize_serde(&snap).map_err(|err| io::Error::other(format!("{err}")))?;

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
                && !(name.ends_with("~") || name.ends_with("tmp") || name.ends_with("temp"))
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

        // Note: We do not need to have a bounds-check on `cache_index` here:
        // - when the request is coming over the network, we have a check in `network/api.rs`
        // - when the request is direct without the network, it can only come from the same
        //   `CacheVariants` enum that this handler is running on, because all client functions are
        //   sealed and the user never provides the Integer manually.

        let mut last_applied_log_id = None;
        while let Some(item) = entries.next().await {
            let (entry, responder) = item?;
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
                            .expect("cache handler to always be running");

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
                            .expect("cache handler to always be running");

                        // The cache handler runs on its own thread per cache and never takes the
                        // state-machine lock, so awaiting its answer while `data` is held is
                        // deadlock-free. The removal is a raft log entry, applied on all nodes.
                        CacheResponse::Value(rx.await.expect("cache handler to always answer"))
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
                            .expect("cache handler to always be running");

                        CacheResponse::Value(rx.await.expect("cache handler to always answer"))
                    }

                    CacheRequest::Delete { cache_idx, key } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Delete {
                                key: key.to_string(),
                            })
                            .expect("cache handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::Clear { cache_idx } => {
                        self.tx_caches
                            .get(cache_idx)
                            .unwrap()
                            .send(CacheRequestHandler::Clear)
                            .expect("cache handler to always be running");

                        CacheResponse::Ok
                    }

                    CacheRequest::ClearCounters { cache_idx } => {
                        #[cfg(feature = "counters")]
                        {
                            self.tx_caches
                                .get(cache_idx)
                                .unwrap()
                                .send(CacheRequestHandler::ClearCounters)
                                .expect("cache handler to always be running");

                            CacheResponse::Ok
                        }
                        #[cfg(not(feature = "counters"))]
                        unreachable!("ClearCounters requires the `counters` feature")
                    }

                    CacheRequest::ClearAll => {
                        for tx in &self.tx_caches {
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
                                .expect("cache handler to always be running");

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
                                .expect("cache handler to always be running");

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
        assert_eq!(idx(&CacheRequest::LockAlive((key(), 0))), 10);
        assert_eq!(idx(&CacheRequest::LockAwait((key(), 0))), 11);
        assert_eq!(idx(&CacheRequest::LockRelease((key(), 0))), 12);
        assert_eq!(
            idx(&CacheRequest::CounterGet {
                cache_idx: 0,
                key: key()
            }),
            13
        );
        assert_eq!(
            idx(&CacheRequest::CounterSet {
                cache_idx: 0,
                key: key(),
                value: 0
            }),
            14
        );
        assert_eq!(
            idx(&CacheRequest::CounterAdd {
                cache_idx: 0,
                key: key(),
                value: 0
            }),
            15
        );
        assert_eq!(
            idx(&CacheRequest::CounterDel {
                cache_idx: 0,
                key: key()
            }),
            16
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
            &[
                (0, "App"),
                (1, "AuthCodes"),
                (2, "Users"),
                (3, "MagicLinks"),
            ]
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
        assert!(Cur::hiqlite_cache_compatible_with(
            "0 App\n1 AuthCodes\n2 Users\n"
        ));
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
        assert!(!Cur::hiqlite_cache_compatible_with(
            "0 App\n1 Users\n2 MagicLinks\n"
        ));
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

    /// A disk-backed state machine must refuse to start when the persisted fingerprint no
    /// longer matches the current enum (a rename here). This is the startup safeguard that
    /// catches re-order / insert-in-between / removal / rename of the `CacheVariants` enum
    /// across restarts; a pure expansion at the end is still accepted.
    #[cfg(not(feature = "in-memory-snapshots"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn startup_rejects_incompatible_cache_index() {
        let base_dir = std::env::temp_dir().join("hiqlite_cache_index_compat_test");
        let _ = std::fs::remove_dir_all(&base_dir);
        let base = base_dir.to_str().unwrap();

        // `Cur` has `App` at index 0, so a stored fingerprint claiming index 0 to be
        // something else is a rename -> incompatible.
        std::fs::create_dir_all(format!("{base}/state_machine_cache")).unwrap();
        std::fs::write(
            format!("{base}/state_machine_cache/cache_index.meta"),
            "0 SomethingElse\n",
        )
        .unwrap();

        let res = super::StateMachineMemory::new::<Cur>(base, false).await;
        assert!(
            matches!(res, Err(crate::Error::Cache(_))),
            "expected an incompatible cache index to be rejected at startup, got {res:?}"
        );

        let _ = std::fs::remove_dir_all(&base_dir);
    }

    /// The fingerprint file is the "cache index enum compatibility" marker that every startup
    /// compares against. A zero-variant enum - the `Empty` type of the `hiqlite` server binary -
    /// normalizes to the same string as a fresh startup, so it must still write the file,
    /// otherwise a startup with such an enum would never create the marker at all.
    #[cfg(not(feature = "in-memory-snapshots"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn empty_enum_still_writes_fingerprint_file() {
        #[derive(Debug)]
        enum NoVariants {}

        impl CacheVariants for NoVariants {
            fn hiqlite_cache_index(&self) -> usize {
                unreachable!()
            }

            fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
                &[]
            }
        }

        let base_dir = std::env::temp_dir().join("hiqlite_cache_index_empty_enum_test");
        let _ = std::fs::remove_dir_all(&base_dir);
        let base = base_dir.to_str().unwrap();
        let path_meta = format!("{base}/state_machine_cache/cache_index.meta");

        super::StateMachineMemory::new::<NoVariants>(base, false)
            .await
            .expect("state machine to start");
        assert!(
            std::path::Path::new(&path_meta).try_exists().unwrap(),
            "a zero-variant enum must write the fingerprint file"
        );
        assert_eq!(std::fs::read_to_string(&path_meta).unwrap(), "");

        // a pure expansion at the end must update the fingerprint and stay compatible
        super::StateMachineMemory::new::<Cur>(base, false)
            .await
            .expect("expanding a zero-variant enum to start");
        assert_eq!(
            std::fs::read_to_string(&path_meta).unwrap(),
            "0 App\n1 AuthCodes\n2 Users\n3 MagicLinks\n"
        );

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}
