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
    /// Removes a key's pending expiry, e.g. after a re-put without a TTL.
    Clear(String),
    SnapshotBuild(oneshot::Sender<BTreeMap<i64, String>>),
    SnapshotInstall((BTreeMap<i64, String>, oneshot::Sender<()>)),
}

pub fn spawn(tx_kv: flume::Sender<CacheRequestHandler>) -> flume::Sender<TtlRequest> {
    spawn_with_clock(tx_kv, || Utc::now().timestamp_micros())
}

/// `now` returns the current unix time in microseconds; tests inject a controllable clock so
/// expiry behaviour is asserted deterministically without real-time sleeps.
fn spawn_with_clock(
    tx_kv: flume::Sender<CacheRequestHandler>,
    now: impl Fn() -> i64 + Send + Sync + 'static,
) -> flume::Sender<TtlRequest> {
    let (tx, rx) = flume::unbounded();
    task::spawn(ttl_handler(tx_kv, rx, now));
    tx
}

/// Older snapshots stored second-precision expiries (~1.7e9); micros values are ~1.7e15.
const SECONDS_THRESHOLD: i64 = 1_000_000_000_000;

/// Maps a possibly old second-precision expiry to micros.
fn normalize(exp: i64) -> i64 {
    if exp < SECONDS_THRESHOLD {
        exp * 1_000_000
    } else {
        exp
    }
}

async fn ttl_handler(
    tx_kv: flume::Sender<CacheRequestHandler>,
    rx: flume::Receiver<TtlRequest>,
    now: impl Fn() -> i64 + Send + Sync + 'static,
) {
    // expiry (micros) -> key. One key per expiry: `Ttl` bumps collisions by 1 microsecond.
    let mut data: BTreeMap<i64, String> = BTreeMap::new();
    // key -> current expiry, so a refresh can drop the old one in O(1) and a stale expiry
    // can never delete the freshly updated value.
    let mut exp_of: HashMap<String, i64> = HashMap::new();

    loop {
        let sleep_exp = {
            let first_exp = data
                .first_entry()
                .map(|e| normalize(*e.key()) - now());

            if let Some(exp) = first_exp {
                if exp < 1 {
                    let (exp, key) = data.pop_first().unwrap();
                    // only delete if this expiry is still the current one for the key
                    if exp_of.get(&key) == Some(&exp) {
                        exp_of.remove(&key);
                        tx_kv
                            .send(CacheRequestHandler::Delete(key))
                            .expect("kv handler to always be running");
                    }
                    continue;
                } else {
                    Duration::from_micros(exp as u64)
                }
            } else {
                Duration::from_secs(u64::MAX)
            }
        };

        // Process pending requests before expiring entries: a same-instant refresh must not
        // let a stale Delete hit the freshly re-put value.
        tokio::select! {
            biased;
            req = rx.recv_async() => {
                if let Ok(req) = req {
                    match req {
                        TtlRequest::Ttl((mut ttl, key)) => {
                            // drop the key's previous expiry, then resolve same-micros
                            // collisions by bumping to the next free slot
                            if let Some(&old) = exp_of.get(&key) {
                                data.remove(&old);
                            }
                            while data.contains_key(&ttl) {
                                ttl += 1;
                            }
                            exp_of.insert(key.clone(), ttl);
                            data.insert(ttl, key);
                        }
                        TtlRequest::Clear(key) => {
                            if let Some(&old) = exp_of.get(&key) {
                                data.remove(&old);
                            }
                            exp_of.remove(&key);
                        }
                        TtlRequest::SnapshotBuild(ack) => {
                            ack.send(data.clone()).unwrap();
                        }
                        TtlRequest::SnapshotInstall((snap, ack)) => {
                            data.clear();
                            exp_of.clear();
                            for (exp, key) in snap {
                                let exp = normalize(exp);
                                exp_of.insert(key.clone(), exp);
                                data.insert(exp, key);
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

    /// Clock base: a plausible 2026 unix timestamp in microseconds.
    const T0: i64 = 1_786_000_000_000_000;

    /// Controllable clock + sync point: `sync` awaits a SnapshotBuild ack, so the handler has
    /// processed the current clock value before the assertions run.
    fn harness(
    ) -> (
        flume::Sender<TtlRequest>,
        flume::Receiver<CacheRequestHandler>,
        Arc<AtomicI64>,
    ) {
        let (tx_kv, rx_kv) = flume::unbounded();
        let clock = Arc::new(AtomicI64::new(T0));
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
    async fn collision_bump_keeps_both_keys() {
        let (tx, rx_kv, _) = harness();
        // same expiry micros: the second key is bumped by 1us, so both expire
        tx.send(TtlRequest::Ttl((T0 - 1, "k1".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((T0 - 1, "k2".to_string()))).unwrap();
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
        tx.send(TtlRequest::Ttl((T0 + 1, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((T0 + 60, "k".to_string()))).unwrap();
        sync(&tx).await; // both requests processed at clock=T0
        clock.store(T0 + 2, Ordering::Relaxed); // past the old expiry
        sync(&tx).await;
        assert!(rx_kv.is_empty());
    }

    #[tokio::test]
    async fn refreshed_key_expires_at_new_expiry() {
        let (tx, rx_kv, clock) = harness();
        tx.send(TtlRequest::Ttl((T0 + 1, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Ttl((T0 + 60, "k".to_string()))).unwrap();
        sync(&tx).await; // both requests processed at clock=T0
        clock.store(T0 + 2, Ordering::Relaxed);
        sync(&tx).await;
        assert!(rx_kv.is_empty()); // not deleted at the old expiry

        clock.store(T0 + 61, Ordering::Relaxed);
        sync(&tx).await;
        match rx_kv.try_recv() {
            Ok(CacheRequestHandler::Delete(k)) => assert_eq!(k, "k".to_string()),
            other => panic!("expected Delete, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn clear_removes_pending_expiry() {
        let (tx, rx_kv, clock) = harness();
        tx.send(TtlRequest::Ttl((T0 + 1, "k".to_string()))).unwrap();
        tx.send(TtlRequest::Clear("k".to_string())).unwrap();
        sync(&tx).await; // both requests processed at clock=T0
        clock.store(T0 + 2, Ordering::Relaxed);
        sync(&tx).await;
        assert!(rx_kv.is_empty());
    }

    #[tokio::test]
    async fn snapshot_roundtrip_preserves_expiries() {
        let (tx, rx_kv, _) = harness();
        tx.send(TtlRequest::Ttl((T0 + 3600, "k".to_string()))).unwrap();

        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotBuild(ack)).unwrap();
        let snap = rx.await.unwrap();
        assert_eq!(snap.get(&(T0 + 3600)), Some(&"k".to_string()));

        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotInstall((snap.clone(), ack)))
            .unwrap();
        rx.await.unwrap();

        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotBuild(ack)).unwrap();
        assert_eq!(rx.await.unwrap(), snap);
        drop(rx_kv);
    }

    #[tokio::test]
    async fn old_seconds_expiries_are_normalized_on_install() {
        let (tx, rx_kv, clock) = harness();
        // an old-format snapshot with a second-precision expiry (2026 + 1s)
        let snap = BTreeMap::from([(T0 / 1_000_000 + 1, "k".to_string())]);
        let (ack, rx) = oneshot::channel();
        tx.send(TtlRequest::SnapshotInstall((snap, ack))).unwrap();
        rx.await.unwrap();

        // the seconds value is normalized to micros, so it still fires on time (1s later)
        clock.store(T0 + 1_000_001, Ordering::Relaxed);
        sync(&tx).await;
        match rx_kv.try_recv() {
            Ok(CacheRequestHandler::Delete(k)) => assert_eq!(k, "k".to_string()),
            other => panic!("expected Delete, got {other:?}"),
        }
    }
}
