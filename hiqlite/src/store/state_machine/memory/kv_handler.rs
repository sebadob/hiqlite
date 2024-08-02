use crate::store::state_machine::memory::state_machine::StateMachineData;
use crate::store::state_machine::memory::TypeConfigKV;
use crate::NodeId;
use openraft::{Snapshot, StorageError};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::thread;
use tokio::sync::{oneshot, RwLock};
use tokio::task;
use tracing::{info, warn};

#[derive(Debug)]
pub enum CacheRequestHandler {
    Get((String, oneshot::Sender<Option<Vec<u8>>>)),
    Put((String, Vec<u8>)),
    Delete(String),
    SnapshotBuild(oneshot::Sender<BTreeMap<String, Vec<u8>>>),
    SnapshotInstall((BTreeMap<String, Vec<u8>>, oneshot::Sender<()>)),
}

pub fn spawn<C: Debug>(cache: C) -> flume::Sender<CacheRequestHandler> {
    let (tx, rx) = flume::unbounded();
    let cache_name = format!("{:?}", cache);

    task::spawn(kv_handler(cache_name, rx));

    tx
}

async fn kv_handler(cache_name: String, rx: flume::Receiver<CacheRequestHandler>) {
    info!(
        "Cache {} running on Thread {:?}",
        cache_name,
        thread::current().id()
    );

    let mut data: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    while let Ok(req) = rx.recv_async().await {
        match req {
            CacheRequestHandler::Get((key, ack)) => ack.send(data.get(&key).cloned()).unwrap(),
            CacheRequestHandler::Put((key, value)) => {
                data.insert(key, value);
            }
            CacheRequestHandler::Delete(key) => {
                data.remove(&key);
            }
            CacheRequestHandler::SnapshotBuild(ack) => {
                ack.send(data.clone()).unwrap();
            }
            CacheRequestHandler::SnapshotInstall((kvs, ack)) => {
                data = kvs;
                ack.send(()).unwrap();
            }
        }
    }

    warn!("cache::kv_handler for {} exiting", cache_name);
}
