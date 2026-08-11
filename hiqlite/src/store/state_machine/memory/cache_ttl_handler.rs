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
    spawn_with_clock(tx_kv, || Utc::now().timestamp())
}

/// `now` returns the current unix timestamp; tests inject a controllable clock so expiry
/// behaviour can be asserted deterministically without real-time sleeps.
fn spawn_with_clock(
    tx_kv: flume::Sender<CacheRequestHandler>,
    now: impl Fn() -> i64 + Send + Sync + 'static,
) -> flume::Sender<TtlRequest> {
    let (tx, rx) = flume::unbounded();
    task::spawn(ttl_handler(tx_kv, rx, now));
    tx
}

async fn ttl_handler(
    tx_kv: flume::Sender<CacheRequestHandler>,
    rx: flume::Receiver<TtlRequest>,
    now: impl Fn() -> i64 + Send + Sync + 'static,
) {
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
                .map(|e| *e.key() - now());

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

        // Process pending requests before expiring buckets: a same-instant refresh must not
        // let a stale Delete hit the freshly re-put value.
        tokio::select! {
            biased;
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
                            // Snapshot format: one key per expiry second; same-second keys
                            // collide on restore (rare; accepted over dropping TTLs live).
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
    use std::sync::atomic::{AtomicI64, Ordering};

    /// Controllable clock + sync point. `sync` awaits a SnapshotBuild ack, which the handler
    /// only sends after it has processed the next expiry batch for the current clock value, so
    /// expiry behaviour is asserted deterministically without any real-time sleeps.
    fn harness(
    ) -> (
        flume::Sender<TtlRequest>,
        flume::Receiver<CacheRequestHandler>,
        Arc<AtomicI64>,
    ) {
        let (tx_kv, rx_kv) = flume::unbounded();
        let clock = Arc::new(AtomicI64::new(1000));
        let c = clock.clone();
        let tx = spawn_with_clock(tx_kv, move || c.load(Ordering::Relaxed));
        (tx, rx_kv, clock)
    }

    async fn sync(tx: &flume::Sender<TtlRequest>) {
        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotBuild(ack)).unwrap();
        rx.await.unwrap();
    }

    #[tokio::test]
    async fn same_second_expiry_deletes_all_keys() {
        let (tx, rx_kv, _) = harness();
        tx.send(TtlRequest::Ttl((999, "k1".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((999, "k2".to_string()))).unwrap();
        sync(&tx).await;

        let mut got = vec![];
        while let Ok(CacheRequestHandler::Delete(k)) = rx_kv.try_recv() {
            got.push(k);
        }
        got.sort();
        assert_eq!(got, ["k1".to_string(), "k2".to_string()]);
    }

    #[tokio::test]
    async fn refreshed_key_is_not_deleted_at_old_expiry() {
        let (tx, rx_kv, clock) = harness();
        tx.send(TtlRequest::Ttl((1001, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((1060, "k".to_string()))).unwrap();
        sync(&tx).await; // both requests processed at clock=1000
        clock.store(1002, Ordering::Relaxed); // past the old expiry
        sync(&tx).await;
        assert!(rx_kv.is_empty());
    }

    #[tokio::test]
    async fn refreshed_key_expires_at_new_expiry() {
        let (tx, rx_kv, clock) = harness();
        tx.send(TtlRequest::Ttl((1001, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((1060, "k".to_string()))).unwrap();
        sync(&tx).await; // both requests processed at clock=1000
        clock.store(1002, Ordering::Relaxed);
        sync(&tx).await;
        assert!(rx_kv.is_empty()); // not deleted at the old expiry

        clock.store(1061, Ordering::Relaxed);
        sync(&tx).await;
        match rx_kv.try_recv() {
            Ok(CacheRequestHandler::Delete(k)) => assert_eq!(k, "k".to_string()),
            other => panic!("expected Delete, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn clear_removes_pending_expiry() {
        let (tx, rx_kv, clock) = harness();
        tx.send(TtlRequest::Ttl((1001, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Clear("k".to_string())).unwrap();
        sync(&tx).await; // both requests processed at clock=1000
        clock.store(1002, Ordering::Relaxed);
        sync(&tx).await;
        assert!(rx_kv.is_empty());
    }

    #[tokio::test]
    async fn snapshot_roundtrip_preserves_expiries() {
        let (tx, rx_kv, _) = harness();
        tx.send(TtlRequest::Ttl((3600, "k".to_string()))).unwrap();

        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotBuild(ack)).unwrap();
        let snap = rx.await.unwrap();
        assert_eq!(snap.get(&3600), Some(&"k".to_string()));

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
