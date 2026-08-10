use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::state_machine::{CacheResponse, StateMachineData};
use chrono::Utc;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};
use tokio::{task, time};
use tracing::{debug, warn};

#[derive(Debug)]
pub enum TtlRequest {
    Ttl((i64, String)),
    /// Removes a previously registered expiry for a key, e.g. when the value was re-put without
    /// a TTL. Without this, the old expiry would still fire and delete the fresh value early.
    Clear(String),
    SnapshotBuild(oneshot::Sender<BTreeMap<i64, String>>),
    SnapshotInstall((BTreeMap<i64, String>, oneshot::Sender<()>)),
}

pub fn spawn(tx_kv: flume::Sender<CacheRequestHandler>) -> flume::Sender<TtlRequest> {
    let (tx, rx) = flume::unbounded();
    task::spawn(ttl_handler(tx_kv, rx));
    tx
}

async fn ttl_handler(tx_kv: flume::Sender<CacheRequestHandler>, rx: flume::Receiver<TtlRequest>) {
    // expiry timestamp -> keys expiring at that time. A `Vec` is used because multiple keys can
    // expire in the same second. A map keyed by expiry alone would silently drop all but one of
    // them, so the remaining ones would never be expired.
    let mut data: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    // key -> currently registered expiry. Needed to remove an old expiry in O(1) when a value is
    // refreshed, so a stale expiry can never delete a freshly updated value early.
    let mut exp_of: HashMap<String, i64> = HashMap::new();

    loop {
        let sleep_exp = {
            let first_exp = data
                .first_entry()
                .map(|e| *e.key() - Utc::now().timestamp());

            if let Some(exp) = first_exp {
                if exp < 1 {
                    // `exp` is a relative delta; the bucket key is the absolute expiry.
                    let (exp, keys) = data.pop_first().unwrap();
                    for key in keys {
                        // only delete if this expiry is still the current one for the key
                        if exp_of.get(&key) == Some(&exp) {
                            exp_of.remove(&key);
                            tx_kv
                                .send(CacheRequestHandler::Delete(key))
                                .expect("kv handler to always be running");
                        }
                    }
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
                if let Ok(req) = req {
                    match req {
                        TtlRequest::Ttl((ttl, key)) => {
                            // remove a possibly existing earlier expiry for this key
                            if let Some(&old) = exp_of.get(&key)
                                && let Some(keys) = data.get_mut(&old)
                            {
                                keys.retain(|k| k != &key);
                                if keys.is_empty() {
                                    data.remove(&old);
                                }
                            }
                            exp_of.insert(key.clone(), ttl);
                            data.entry(ttl).or_default().push(key);
                        }
                        TtlRequest::Clear(key) => {
                            if let Some(&old) = exp_of.get(&key)
                                && let Some(keys) = data.get_mut(&old)
                            {
                                keys.retain(|k| k != &key);
                                if keys.is_empty() {
                                    data.remove(&old);
                                }
                            }
                            exp_of.remove(&key);
                        }
                        TtlRequest::SnapshotBuild(ack) => {
                            // The snapshot format stores a single key per expiry timestamp. If
                            // two keys expire in the same second, only one of them survives a
                            // snapshot restore. This is a rare edge case and preferable to
                            // dropping TTLs during live operation.
                            let snap = exp_of
                                .iter()
                                .map(|(key, exp)| (*exp, key.clone()))
                                .collect::<BTreeMap<i64, String>>();
                            ack.send(snap).unwrap();
                        }
                        TtlRequest::SnapshotInstall((snap, ack)) => {
                            data.clear();
                            exp_of.clear();
                            for (exp, key) in snap {
                                exp_of.insert(key.clone(), exp);
                                data.entry(exp).or_default().push(key);
                            }
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

    debug!("cache::ttl_handler exiting");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time;

    #[tokio::test]
    async fn same_second_expiry_deletes_all_keys() {
        let (tx_kv, rx_kv) = flume::unbounded();
        let tx = spawn(tx_kv);
        let past = Utc::now().timestamp() - 1;

        tx.send(TtlRequest::Ttl((past, "k1".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((past, "k2".to_string()))).unwrap();

        let d1 = tokio::time::timeout(Duration::from_secs(5), rx_kv.recv_async())
            .await
            .expect("first delete timeout");
        let d2 = tokio::time::timeout(Duration::from_secs(5), rx_kv.recv_async())
            .await
            .expect("second delete timeout");
        match (d1, d2) {
            (Ok(CacheRequestHandler::Delete(a)), Ok(CacheRequestHandler::Delete(b))) => {
                let mut got = [a, b];
                got.sort();
                assert_eq!(got, ["k1".to_string(), "k2".to_string()]);
            }
            other => panic!("expected Delete messages, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn refreshed_key_is_not_deleted_at_old_expiry() {
        let (tx_kv, rx_kv) = flume::unbounded();
        let tx = spawn(tx_kv);
        let now = Utc::now().timestamp();
        // the old expiry fires after ~1s, the refresh moves it to ~2s
        let old = now + 1;
        let new_exp = now + 2;

        tx.send(TtlRequest::Ttl((old, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((new_exp, "k".to_string())))
            .unwrap();

        // wait past the old expiry: the refreshed key must not be deleted early
        time::sleep(Duration::from_millis(1600)).await;
        assert!(rx_kv.is_empty());
    }

    #[tokio::test]
    async fn clear_removes_pending_expiry() {
        let (tx_kv, rx_kv) = flume::unbounded();
        let tx = spawn(tx_kv);
        let exp = Utc::now().timestamp() + 1;

        tx.send(TtlRequest::Ttl((exp, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Clear("k".to_string())).unwrap();

        // the expiry must never fire after Clear
        time::sleep(Duration::from_millis(1600)).await;
        assert!(rx_kv.is_empty());
    }

    #[tokio::test]
    async fn snapshot_roundtrip_preserves_expiries() {
        let (tx_kv, rx_kv) = flume::unbounded();
        let tx = spawn(tx_kv);
        let future = Utc::now().timestamp() + 3600;

        tx.send(TtlRequest::Ttl((future, "k".to_string()))).unwrap();

        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotBuild(ack)).unwrap();
        let snap = rx.await.unwrap();
        assert_eq!(snap.get(&future), Some(&"k".to_string()));

        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotInstall((snap.clone(), ack)))
            .unwrap();
        rx.await.unwrap();

        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotBuild(ack)).unwrap();
        assert_eq!(rx.await.unwrap(), snap);
        drop(rx_kv);
    }
}
