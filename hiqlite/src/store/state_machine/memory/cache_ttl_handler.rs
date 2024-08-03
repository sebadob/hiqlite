use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::state_machine::{CacheResponse, StateMachineData};
use chrono::Utc;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};
use tokio::{task, time};
use tracing::{debug, warn};

#[derive(Debug)]
pub enum TtlRequest {
    Ttl((i64, String)),
    SnapshotBuild(oneshot::Sender<BTreeMap<i64, String>>),
    SnapshotInstall((BTreeMap<i64, String>, oneshot::Sender<()>)),
}

pub fn spawn(tx_kv: flume::Sender<CacheRequestHandler>) -> flume::Sender<TtlRequest> {
    let (tx, rx) = flume::unbounded();
    task::spawn(ttl_handler(tx_kv, rx));
    tx
}

async fn ttl_handler(tx_kv: flume::Sender<CacheRequestHandler>, rx: flume::Receiver<TtlRequest>) {
    let mut data: BTreeMap<i64, String> = BTreeMap::new();

    loop {
        let sleep_exp = {
            let first_exp = data
                .first_entry()
                .map(|e| *e.key() - Utc::now().timestamp());

            if let Some(exp) = first_exp {
                if exp < 1 {
                    let key = data.pop_first().unwrap().1;
                    tx_kv
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
            // req = rx.recv_async() => {
            //     if let Ok((ttl, key)) = req {
            //         // TODO currently we use microsecond precision and this could overlap
            //         // if very unlucky -> use nano's or da an additional step ech time to check if
            //         // the entry exists already? otherwise a value will not be expired
            //         data.insert(ttl, key);
            //     } else {
            //         break;
            //     }
            // }
            req = rx.recv_async() => {
                if let Ok(req) = req {
                    // TODO currently we use microsecond precision and this could overlap
                    // if very unlucky -> use nano's or da an additional step ech time to check if
                    // the entry exists already? otherwise a value will not be expired
                    match req {
                        TtlRequest::Ttl((ttl, key)) => {
                            data.insert(ttl, key);
                        }
                        TtlRequest::SnapshotBuild(ack) => {
                            ack.send(data.clone()).unwrap();
                        }
                        TtlRequest::SnapshotInstall((snap, ack)) => {
                            data = snap;
                            ack.send(()).unwrap();
                        }
                    }
                } else {
                    break;
                }
            }
            _ = time::sleep(sleep_exp) => {
                debug!("Timeout reached - first entry in map expires");
            }
        }
    }

    warn!("cache::ttl_handler exiting");
}
