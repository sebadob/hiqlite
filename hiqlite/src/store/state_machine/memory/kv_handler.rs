use crate::NodeId;
use crate::store::state_machine::memory::TypeConfigKV;
use crate::store::state_machine::memory::state_machine::StateMachineData;
use openraft::{Snapshot, StorageError};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::thread;
use tokio::sync::{RwLock, oneshot};
use tokio::task;
use tracing::{debug, error, info, warn};

#[derive(Debug)]
#[allow(clippy::type_complexity)]
pub enum CacheRequestHandler {
    Get((String, oneshot::Sender<Option<Vec<u8>>>)),
    Put((String, Vec<u8>)),
    Delete(String),
    Clear,
    #[cfg(feature = "counters")]
    ClearCounters,
    SnapshotBuildCacheOnly(oneshot::Sender<BTreeMap<String, Vec<u8>>>),
    SnapshotBuild(oneshot::Sender<(BTreeMap<String, Vec<u8>>, BTreeMap<String, i64>)>),
    SnapshotInstall(
        (
            (BTreeMap<String, Vec<u8>>, BTreeMap<String, i64>),
            oneshot::Sender<()>,
        ),
    ),

    #[cfg(feature = "counters")]
    CounterGet((String, oneshot::Sender<Option<i64>>)),
    #[cfg(feature = "counters")]
    CounterSet((String, i64)),
    #[cfg(feature = "counters")]
    CounterAdd((String, i64, oneshot::Sender<i64>)),
    #[cfg(feature = "counters")]
    CounterDel(String),
}

pub fn spawn(cache_name: &'static str) -> flume::Sender<CacheRequestHandler> {
    let (tx, rx) = flume::unbounded();
    task::spawn(kv_handler(cache_name, rx));
    tx
}

#[tracing::instrument(level = "debug", skip(rx))]
async fn kv_handler(cache_name: &'static str, rx: flume::Receiver<CacheRequestHandler>) {
    info!(
        "Cache {} running on Thread {:?}",
        cache_name,
        thread::current().id()
    );

    let mut data: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    #[cfg(feature = "counters")]
    let mut counters: BTreeMap<String, i64> = BTreeMap::new();

    while let Ok(req) = rx.recv_async().await {
        match req {
            CacheRequestHandler::Get((key, ack)) => {
                if ack.send(data.get(&key).cloned()).is_err() {
                    error!("Error sending back Cache GET request: channel closed");
                }
            }
            CacheRequestHandler::Put((key, value)) => {
                data.insert(key, value);
            }
            CacheRequestHandler::Delete(key) => {
                data.remove(&key);
            }
            CacheRequestHandler::Clear => {
                debug!("Clearing all caches for {cache_name}");
                data.clear();
            }
            #[cfg(feature = "counters")]
            CacheRequestHandler::ClearCounters => {
                debug!("Clearing all counters for {cache_name}");
                counters.clear();
            }
            CacheRequestHandler::SnapshotBuildCacheOnly(ack) => {
                if ack.send(data.clone()).is_err() {
                    error!("Error sending back SnapshotBuildCacheOnly response");
                }
            }
            CacheRequestHandler::SnapshotBuild(ack) => {
                #[cfg(feature = "counters")]
                let data_counter = counters.clone();
                #[cfg(not(feature = "counters"))]
                let data_counter = BTreeMap::new();

                if ack.send((data.clone(), data_counter)).is_err() {
                    error!("Error sending back SnapshotBuild response");
                }
            }
            CacheRequestHandler::SnapshotInstall(((kvs, counts), ack)) => {
                data = kvs;
                #[cfg(feature = "counters")]
                {
                    counters = counts;
                }
                if ack.send(()).is_err() {
                    error!("Error sending back SnapshotInstall response");
                }
            }

            #[cfg(feature = "counters")]
            CacheRequestHandler::CounterGet((k, ack)) => {
                let v = counters.get(&k).cloned();
                if ack.send(v).is_err() {
                    error!("Error sending back CounterGet response");
                }
            }
            #[cfg(feature = "counters")]
            CacheRequestHandler::CounterSet((k, v)) => {
                counters.insert(k, v);
            }
            #[cfg(feature = "counters")]
            CacheRequestHandler::CounterAdd((k, v, ack)) => {
                let v = if let Some(current) = counters.get_mut(&k) {
                    *current = current.saturating_add(v);
                    *current
                } else {
                    counters.insert(k, v);
                    v
                };
                if ack.send(v).is_err() {
                    error!("Error sending back CounterAdd value");
                }
            }
            #[cfg(feature = "counters")]
            CacheRequestHandler::CounterDel(key) => {
                counters.remove(&key);
            }
        }
    }

    debug!("cache::kv_handler for {cache_name} exiting");
}
