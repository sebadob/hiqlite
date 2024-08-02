use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::state_machine::{CacheResponse, StateMachineData};
use chrono::Utc;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::{task, time};
use tracing::warn;

pub fn spawn(
    tx_kv: Vec<flume::Sender<CacheRequestHandler>>,
) -> flume::Sender<(i64, (usize, String))> {
    let (tx, rx) = flume::unbounded();
    task::spawn(ttl_handler(tx_kv, rx));
    tx
}

async fn ttl_handler(
    tx_kv: Vec<flume::Sender<CacheRequestHandler>>,
    rx: flume::Receiver<(i64, (usize, String))>,
) {
    let mut data: BTreeMap<i64, (usize, String)> = BTreeMap::new();

    loop {
        let sleep_exp = {
            let first_exp = data
                .first_entry()
                .map(|e| *e.key() - Utc::now().timestamp());

            if let Some(exp) = first_exp {
                if exp < 1 {
                    let (idx, key) = data.pop_first().unwrap().1;
                    tx_kv
                        .get(idx)
                        .unwrap()
                        .send(CacheRequestHandler::Delete(key))
                        .expect("kv handler to always be running");
                    continue;
                } else {
                    Duration::from_secs(exp as u64)
                }
            } else {
                Duration::from_secs(u64::MAX)
            }
        };

        tokio::select! {
            req = rx.recv_async() => {
                if let Ok((ttl, idx_key)) = req {
                    data.insert(ttl, idx_key);
                } else {
                    break;
                }
            }
            _ = time::sleep(sleep_exp) => {
                warn!("Timeout reached - first entry in map expires");
            }
        }
    }

    warn!("cache::ttl_handler exiting");
}
