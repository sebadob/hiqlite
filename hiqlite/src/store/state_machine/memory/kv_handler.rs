use chrono::Utc;
use std::collections::BTreeMap;
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::{task, time};
use tracing::{debug, error, info};

/// Per-cache snapshot payload: values (each with its optional expiry in unix micros) plus
/// counters. The expiry lives inside the entry, so nothing else has to be snapshotted.
pub type CacheSnapshot = (
    BTreeMap<String, (Vec<u8>, Option<i64>)>,
    BTreeMap<String, i64>,
);

#[derive(Debug)]
pub enum CacheRequestHandler {
    Get {
        key: String,
        reply: oneshot::Sender<Option<Vec<u8>>>,
    },
    GetRemove {
        key: String,
        reply: oneshot::Sender<Option<Vec<u8>>>,
    },
    Put {
        key: String,
        value: Vec<u8>,
        expires: Option<i64>,
    },
    Replace {
        key: String,
        value: Vec<u8>,
        expires: Option<i64>,
        reply: oneshot::Sender<Option<Vec<u8>>>,
    },
    Delete {
        key: String,
    },
    Clear,
    #[cfg(feature = "counters")]
    ClearCounters,
    SnapshotBuildCacheOnly {
        reply: oneshot::Sender<BTreeMap<String, Vec<u8>>>,
    },
    SnapshotBuild {
        reply: oneshot::Sender<CacheSnapshot>,
    },
    SnapshotInstall {
        snapshot: CacheSnapshot,
        ack: oneshot::Sender<()>,
    },
    #[cfg(feature = "counters")]
    CounterGet {
        key: String,
        reply: oneshot::Sender<Option<i64>>,
    },
    #[cfg(feature = "counters")]
    CounterSet {
        key: String,
        value: i64,
    },
    #[cfg(feature = "counters")]
    CounterAdd {
        key: String,
        delta: i64,
        reply: oneshot::Sender<i64>,
    },
    #[cfg(feature = "counters")]
    CounterDel {
        key: String,
    },
}

pub fn spawn(cache_name: &'static str) -> flume::Sender<CacheRequestHandler> {
    let (tx, rx) = flume::unbounded();
    task::spawn(kv_handler(cache_name, rx));
    tx
}

/// Removes the key's pending expiry from the index. The index mirrors the `expires` field of
/// `values`, so call this before changing or removing an entry in `values`.
#[inline(always)]
fn drop_expiry(
    values: &mut BTreeMap<String, (Vec<u8>, Option<i64>)>,
    expiries: &mut BTreeMap<i64, Vec<String>>,
    key: &str,
) {
    if let Some(exp) = values.get(key).and_then(|(_, expires)| *expires) {
        remove_expiry(expiries, exp, key);
    }
}

/// Registers the key's expiry in the index. It must run inside the same synchronous message as
/// the matching change of `values` (no await between), so the index mirrors `values` again
/// before the next top-of-loop iteration; it may come before that change - so the key can be
/// moved into the map without a clone - or after it.
#[inline(always)]
fn register_expiry(expiries: &mut BTreeMap<i64, Vec<String>>, key: &str, exp: i64) {
    expiries.entry(exp).or_default().push(key.to_string());
}

/// Removes one key from its timestamp bucket; drops the bucket once it is empty.
#[inline(always)]
fn remove_expiry(expiries: &mut BTreeMap<i64, Vec<String>>, exp: i64, key: &str) {
    if let Some(keys) = expiries.get_mut(&exp) {
        keys.retain(|k| k != key);
        if keys.is_empty() {
            expiries.remove(&exp);
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn kv_handler(cache_name: &'static str, rx: flume::Receiver<CacheRequestHandler>) {
    info!(
        "Cache {} running on Thread {:?}",
        cache_name,
        thread::current().id()
    );

    // key -> (value, optional expiry in unix micros). The expiry lives with the value, so a
    // snapshot serializes it for free and an install rebuilds the index below from it.
    //
    // Expiry timing intentionally uses wall clocks. `expires` is an absolute timestamp computed
    // client-side (client/cache.rs) from that client's clock and replicated via the Raft; every
    // node then enforces it against its own clock, so a skewed client or node shifts the
    // effective TTL by the skew amount - a far-future write can land already expired on a
    // faster clock. Keep the clocks close enough that the skew is negligible next to the
    // smallest TTL in use.
    let mut values: BTreeMap<String, (Vec<u8>, Option<i64>)> = BTreeMap::new();
    #[cfg(feature = "counters")]
    let mut counters: BTreeMap<String, i64> = BTreeMap::new();
    // expiry (micros) -> keys due at that instant. In-memory only; several keys may share one
    // timestamp, so no collision bumping is needed anymore.
    let mut expiries: BTreeMap<i64, Vec<String>> = BTreeMap::new();

    loop {
        let sleep_exp = {
            let first_exp = expiries.first_key_value().map(|(k, _)| *k);

            if let Some(exp) = first_exp {
                if exp - Utc::now().timestamp_micros() < 1 {
                    // The earliest expiry is due: remove every key registered at that instant.
                    // The index mirrors `values`, so each of them still carries exactly this
                    // expiry; a refresh already moved it to its new bucket.
                    let (exp, keys) = expiries.pop_first().unwrap();
                    for key in keys {
                        debug_assert!(
                            values.get(&key).and_then(|(_, expires)| *expires) == Some(exp),
                            "expiry index and values out of sync"
                        );
                        values.remove(&key);
                    }
                    continue;
                } else {
                    Duration::from_micros((exp - Utc::now().timestamp_micros()) as u64)
                }
            } else {
                Duration::from_secs(u64::MAX)
            }
        };

        // Process pending requests before expiring entries: a same-instant refresh must not
        // let a stale expiry remove the freshly re-put value.
        tokio::select! {
            biased;
            req = rx.recv_async() => match req {
                Ok(req) => match req {
                    CacheRequestHandler::Get { key, reply } => {
                        // Never serve a past expiry: the loop head above already pops due
                        // entries, this also covers the clock advancing inside one iteration.
                        // Pure read - no mutation on Get.
                        let value = match values.get(&key) {
                            Some((value, Some(exp))) if *exp <= Utc::now().timestamp_micros() => None,
                            Some((value, _)) => Some(value.clone()),
                            None => None,
                        };
                        if reply.send(value).is_err() {
                            error!("Error sending back Cache GET request: channel closed");
                        }
                    }
                    CacheRequestHandler::GetRemove { key, reply } => {
                        // consistent with Get: an already-expired key answers None; the
                        // physical removal happens regardless. Drop the pending expiry first,
                        // while it is still readable from the entry.
                        drop_expiry(&mut values, &mut expiries, &key);
                        let value = match values.remove(&key) {
                            Some((value, Some(exp))) if exp <= Utc::now().timestamp_micros() => None,
                            Some((value, _)) => Some(value),
                            None => None,
                        };
                        if reply.send(value).is_err() {
                            error!("Error sending back Cache GET_REMOVE request: channel closed");
                        }
                    }
                    CacheRequestHandler::Put { key, value, expires } => {
                        // Drop the key's previous expiry (or register a fresh one), then install
                        // the value - all inside this one message, so a stale expiry can never
                        // remove the freshly put value. The new expiry is registered before the
                        // insert so `key` can be moved into `values` without a clone.
                        drop_expiry(&mut values, &mut expiries, &key);
                        if let Some(exp) = expires {
                            register_expiry(&mut expiries, &key, exp);
                        }
                        values.insert(key, (value, expires));
                    }
                    CacheRequestHandler::Replace { key, value, expires, reply } => {
                        drop_expiry(&mut values, &mut expiries, &key);
                        if let Some(exp) = expires {
                            register_expiry(&mut expiries, &key, exp);
                        }
                        // returns the physically stored old value (no lazy expiry check),
                        // keeping raft responses as deterministic as before the merge
                        let old = values
                            .insert(key, (value, expires))
                            .map(|(old_value, _)| old_value);
                        if reply.send(old).is_err() {
                            error!("Error sending back Cache REPLACE request: channel closed");
                        }
                    }
                    CacheRequestHandler::Delete { key } => {
                        drop_expiry(&mut values, &mut expiries, &key);
                        values.remove(&key);
                    }
                    CacheRequestHandler::Clear => {
                        debug!("Clearing all caches for {cache_name}");
                        values.clear();
                        expiries.clear();
                    }
                    #[cfg(feature = "counters")]
                    CacheRequestHandler::ClearCounters => {
                        debug!("Clearing all counters for {cache_name}");
                        counters.clear();
                    }
                    CacheRequestHandler::SnapshotBuildCacheOnly { reply } => {
                        let kvs = values
                            .iter()
                            .map(|(k, (v, _))| (k.clone(), v.clone()))
                            .collect();
                        if reply.send(kvs).is_err() {
                            error!("Error sending back SnapshotBuildCacheOnly response");
                        }
                    }
                    CacheRequestHandler::SnapshotBuild { reply } => {
                        #[cfg(feature = "counters")]
                        let counts = counters.clone();
                        #[cfg(not(feature = "counters"))]
                        let counts: BTreeMap<String, i64> = BTreeMap::new();
                        if reply.send((values.clone(), counts)).is_err() {
                            error!("Error sending back SnapshotBuild response");
                        }
                    }
                    CacheRequestHandler::SnapshotInstall { snapshot, ack } => {
                        #[cfg(feature = "counters")]
                        let (kvs, counts) = snapshot;
                        #[cfg(not(feature = "counters"))]
                        let (kvs, _) = snapshot;
                        values = kvs;
                        #[cfg(feature = "counters")]
                        {
                            counters = counts;
                        }
                        // Rebuild the in-memory expiry index from the entries.
                        expiries.clear();
                        for (key, (_, expires)) in &values {
                            if let Some(exp) = *expires {
                                register_expiry(&mut expiries, key, exp);
                            }
                        }
                        if ack.send(()).is_err() {
                            error!("Error sending back SnapshotInstall response");
                        }
                    }

                    #[cfg(feature = "counters")]
                    CacheRequestHandler::CounterGet { key, reply } => {
                        let v = counters.get(&key).copied();
                        if reply.send(v).is_err() {
                            error!("Error sending back CounterGet response");
                        }
                    }
                    #[cfg(feature = "counters")]
                    CacheRequestHandler::CounterSet { key, value } => {
                        counters.insert(key, value);
                    }
                    #[cfg(feature = "counters")]
                    CacheRequestHandler::CounterAdd { key, delta, reply } => {
                        let v = if let Some(current) = counters.get_mut(&key) {
                            *current = current.saturating_add(delta);
                            *current
                        } else {
                            counters.insert(key, delta);
                            delta
                        };
                        if reply.send(v).is_err() {
                            error!("Error sending back CounterAdd value");
                        }
                    }
                    #[cfg(feature = "counters")]
                    CacheRequestHandler::CounterDel { key } => {
                        counters.remove(&key);
                    }
                },
                Err(_) => break,
            },
            _ = time::sleep(sleep_exp) => {
                debug!("Timeout reached - first entry in map expires");
            }
        }
    }

    debug!("cache::kv_handler for {cache_name} exiting");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> i64 {
        Utc::now().timestamp_micros()
    }

    fn harness() -> flume::Sender<CacheRequestHandler> {
        spawn("test")
    }

    /// A SnapshotBuild roundtrip: guarantees everything sent before it has been processed.
    async fn sync(tx: &flume::Sender<CacheRequestHandler>) {
        let (ack, rx) = oneshot::channel();
        tx.send(CacheRequestHandler::SnapshotBuild { reply: ack })
            .unwrap();
        rx.await.unwrap();
    }

    fn put(key: &str, value: &[u8], expires: Option<i64>) -> CacheRequestHandler {
        CacheRequestHandler::Put {
            key: key.to_string(),
            value: value.to_vec(),
            expires,
        }
    }

    async fn call(
        tx: &flume::Sender<CacheRequestHandler>,
        make: impl Fn(oneshot::Sender<Option<Vec<u8>>>) -> CacheRequestHandler,
    ) -> Option<Vec<u8>> {
        let (ack, rx) = oneshot::channel();
        tx.send(make(ack)).unwrap();
        rx.await.unwrap()
    }

    async fn get(tx: &flume::Sender<CacheRequestHandler>, key: &str) -> Option<Vec<u8>> {
        call(tx, |reply| CacheRequestHandler::Get {
            key: key.to_string(),
            reply,
        })
        .await
    }

    async fn get_remove(tx: &flume::Sender<CacheRequestHandler>, key: &str) -> Option<Vec<u8>> {
        call(tx, |reply| CacheRequestHandler::GetRemove {
            key: key.to_string(),
            reply,
        })
        .await
    }

    #[tokio::test]
    async fn get_remove_and_replace_are_atomic_per_key() {
        let tx = harness();
        tx.send(put("a", b"1", None)).unwrap();
        tx.send(put("b", b"2", None)).unwrap();
        sync(&tx).await;

        // GetRemove serves and removes only its own key
        assert_eq!(get_remove(&tx, "a").await, Some(b"1".to_vec()));
        assert_eq!(get(&tx, "a").await, None);
        assert_eq!(get(&tx, "b").await, Some(b"2".to_vec()));

        // Replace returns the physically stored old value and installs the new one
        let (ack, rx) = oneshot::channel();
        tx.send(CacheRequestHandler::Replace {
            key: "b".into(),
            value: b"3".to_vec(),
            expires: None,
            reply: ack,
        })
        .unwrap();
        assert_eq!(rx.await.unwrap(), Some(b"2".to_vec()));
        assert_eq!(get(&tx, "b").await, Some(b"3".to_vec()));
    }

    #[tokio::test]
    async fn value_expires_at_its_expiry() {
        let tx = harness();
        tx.send(put("k", b"v", Some(now() + 30_000))).unwrap();
        sync(&tx).await;
        assert_eq!(get(&tx, "k").await, Some(b"v".to_vec()));

        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(get(&tx, "k").await, None);
    }

    #[tokio::test]
    async fn put_with_past_expiry_expires_immediately() {
        let tx = harness();
        tx.send(put("k", b"v", Some(now() - 1_000))).unwrap();
        sync(&tx).await;
        assert_eq!(get(&tx, "k").await, None);
    }

    #[tokio::test]
    async fn refreshed_key_expires_at_new_expiry() {
        let tx = harness();
        // The first put expires in 30ms; the refresh extends it to 250ms. The key must
        // survive the old expiry and only disappear after the new one.
        tx.send(put("k", b"v1", Some(now() + 30_000))).unwrap();
        tx.send(put("k", b"v2", Some(now() + 250_000))).unwrap();
        sync(&tx).await;

        tokio::time::sleep(Duration::from_millis(120)).await; // well past the old expiry
        assert_eq!(get(&tx, "k").await, Some(b"v2".to_vec()));

        tokio::time::sleep(Duration::from_millis(200)).await; // ~320ms: past the new expiry
        assert_eq!(get(&tx, "k").await, None);
    }

    #[tokio::test]
    async fn same_expiry_keys_expire_together() {
        let tx = harness();
        // both keys share one exact timestamp and must expire together
        let exp = now() + 60_000;
        tx.send(put("k1", b"v1", Some(exp))).unwrap();
        tx.send(put("k2", b"v2", Some(exp))).unwrap();
        sync(&tx).await;

        assert_eq!(get(&tx, "k1").await, Some(b"v1".to_vec()));
        assert_eq!(get(&tx, "k2").await, Some(b"v2".to_vec()));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(get(&tx, "k1").await, None);
        assert_eq!(get(&tx, "k2").await, None);
    }

    #[tokio::test]
    async fn delete_removes_pending_expiry() {
        let tx = harness();
        tx.send(put("k1", b"v1", Some(now() + 40_000))).unwrap();
        tx.send(put("k2", b"v2", Some(now() + 250_000))).unwrap();
        sync(&tx).await;

        tx.send(CacheRequestHandler::Delete { key: "k1".into() })
            .unwrap();
        sync(&tx).await;

        // past k1's old expiry, still before k2's: the deleted key's stale expiry must not
        // touch k2
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(get(&tx, "k1").await, None);
        assert_eq!(get(&tx, "k2").await, Some(b"v2".to_vec()));
    }

    #[tokio::test]
    async fn clear_removes_all_pending_expiries() {
        let tx = harness();
        tx.send(put("a", b"1", Some(now() + 40_000))).unwrap();
        tx.send(put("b", b"2", Some(now() + 80_000))).unwrap();
        tx.send(put("c", b"3", Some(now() + 120_000))).unwrap();
        sync(&tx).await;

        tx.send(CacheRequestHandler::Clear).unwrap();
        sync(&tx).await;

        tokio::time::sleep(Duration::from_millis(200)).await; // past every cleared expiry
        assert_eq!(get(&tx, "a").await, None);
        assert_eq!(get(&tx, "b").await, None);
        assert_eq!(get(&tx, "c").await, None);
    }

    #[tokio::test]
    async fn snapshot_roundtrip_preserves_expiries() {
        let src = harness();
        let dst = harness();

        // a pending expiry and a plain value must both survive the roundtrip
        src.send(put("k", b"v", Some(now() + 400_000))).unwrap();
        src.send(put("plain", b"p", None)).unwrap();
        sync(&src).await;

        let (ack, rx) = oneshot::channel();
        src.send(CacheRequestHandler::SnapshotBuild { reply: ack })
            .unwrap();
        let snapshot = rx.await.unwrap();

        let (ack, rx) = oneshot::channel();
        dst.send(CacheRequestHandler::SnapshotInstall { snapshot, ack })
            .unwrap();
        rx.await.unwrap();

        assert_eq!(get(&dst, "k").await, Some(b"v".to_vec()));
        assert_eq!(get(&dst, "plain").await, Some(b"p".to_vec()));

        // the expiry is still enforced after install
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(get(&dst, "k").await, None);
        assert_eq!(get(&dst, "plain").await, Some(b"p".to_vec()));
    }
}
