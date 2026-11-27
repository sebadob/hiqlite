use crate::store::state_machine::memory::TypeConfigCache;
use crate::store::state_machine::memory::state_machine::{CacheRequest, StateMachineMemory};
use bincode_next::{Decode, Encode};
use chrono::Utc;
use openraft::type_config::async_runtime::watch::WatchReceiver;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task;
use tracing::{debug, warn};

/// How long a granted (or reserved) ticket is considered alive before its holder is treated as
/// dead. Shorter under `debug_assertions` so integration tests don't have to wait out the full
/// production window.
#[cfg(debug_assertions)]
pub const LOCK_VALID_SECONDS: i64 = 2;
#[cfg(not(debug_assertions))]
pub const LOCK_VALID_SECONDS: i64 = 10;
/// How often a lock holder sends `LockAlive` heartbeats to extend its lease. Must stay well
/// below `LOCK_VALID_SECONDS`. Shorter under `debug_assertions`, matching the shorter lease.
#[cfg(debug_assertions)]
pub const LOCK_ALIVE_INTERVAL: Duration = Duration::from_millis(500);
#[cfg(not(debug_assertions))]
pub const LOCK_ALIVE_INTERVAL: Duration = Duration::from_secs(3);
/// How often the Raft leader checks for holders whose lease expired. This bounds how long a
/// queue waits behind a dead holder to one lease window plus this interval.
const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

pub enum LockRequest {
    /// used for a first try lock without coming from a queue
    Lock(LockRequestPayload),
    /// used after an await to acquire the lock now
    Acquire(LockRequestPayload),
    Release(LockReleasePayload),
    Alive(LockAlivePayload),
    Await(LockAwaitPayload),
    /// The holders whose lease expired by this node's clock, as `(key, ticket)`.
    Expired(oneshot::Sender<Vec<(String, u64)>>),
    SnapshotBuild(oneshot::Sender<HashMap<String, LockQueue>>),
    SnapshotInstall((HashMap<String, LockQueue>, oneshot::Sender<()>)),
}

pub struct LockRequestPayload {
    pub key: Cow<'static, str>,
    pub log_id: u64,
    pub ack: oneshot::Sender<LockState>,
}

pub struct LockReleasePayload {
    pub key: Cow<'static, str>,
    pub id: u64,
}

pub struct LockAwaitPayload {
    pub key: Cow<'static, str>,
    pub id: u64,
    pub ack: oneshot::Sender<LockState>,
}

pub struct LockAlivePayload {
    pub key: Cow<'static, str>,
    pub id: u64,
    pub ack: oneshot::Sender<LockState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum LockState {
    Locked(u64),
    Queued(u64),
    Released,
}

impl LockState {
    /// Answer a request. The receiver may be gone (a cancelled client future, or a waiter whose
    /// client went away), which is harmless: an answer never grants anything by itself, every
    /// grant is already part of the applied log, and an unclaimed grant expires with its lease.
    fn send(self, ack: oneshot::Sender<LockState>, key: &str) {
        if let Err(state) = ack.send(self) {
            debug!("DLock: receiver for {key} / {state:?} dropped (client gone)");
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockQueue {
    current_ticket: Option<u64>,
    exp: i64,
    queue: VecDeque<u64>,
}

/// Local registry of in-flight `Await` calls per lock key. It is not part of the snapshot:
/// waiters are ephemeral, and every registered entry is answered exactly once - with
/// `Locked(id)` when its ticket is promoted, or with `Released` when the key is fully removed
/// or a snapshot is installed. A waiter's ticket is always in that key's queue while it is
/// un-answered, so no waiter can block forever.
type Waiters = HashMap<String, Vec<(u64, oneshot::Sender<LockState>)>>;

pub fn spawn() -> flume::Sender<LockRequest> {
    let (tx, rx) = flume::unbounded();
    task::spawn(handler(rx));
    tx
}

/// Expire dead holders through the log. Only the Raft leader acts: every `SWEEP_INTERVAL` it
/// asks its handler for the holders whose lease expired and writes a `LockRelease` for each one
/// through Raft, which promotes the next waiter on every node at the same log position. The task
/// exits once Raft has shut down.
pub fn spawn_expiry(
    raft: openraft::Raft<TypeConfigCache, Arc<StateMachineMemory>>,
    node_id: u64,
    tx_dlock: flume::Sender<LockRequest>,
) {
    task::spawn(async move {
        let mut ticker =
            tokio::time::interval_at(tokio::time::Instant::now() + SWEEP_INTERVAL, SWEEP_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            let (is_leader, is_running) = {
                let metrics = raft.metrics();
                let metrics = metrics.borrow_watched();
                (
                    metrics.current_leader == Some(node_id),
                    metrics.running_state.is_ok(),
                )
            };
            if !is_running {
                break;
            }
            if !is_leader {
                continue;
            }

            let (ack, rx) = oneshot::channel();
            if tx_dlock.send(LockRequest::Expired(ack)).is_err() {
                break;
            }
            let Ok(expired) = rx.await else {
                break;
            };

            for (key, id) in expired {
                debug!("DLock: lease of {key} / {id} expired, releasing it");
                if let Err(err) = raft
                    .client_write(CacheRequest::LockRelease((key.clone().into(), id)))
                    .await
                {
                    // The next tick retries, on whichever node is the leader by then.
                    warn!("DLock: cannot release expired lock {key} / {id}: {err}");
                    break;
                }
            }
        }

        debug!("DLock expiry task exiting");
    });
}

async fn handler(rx: flume::Receiver<LockRequest>) {
    // Every lock state transition - grant, queue, promotion, release - happens only while
    // applying a Raft log entry and depends only on the log, so all nodes hold the same holder
    // and queue at the same log position. No node changes lock state on its own clock: a dead
    // holder is expired by the leader writing a `LockRelease` through Raft (`spawn_expiry`).
    //
    // `exp` is the only clock-based value. Each node keeps it with its own wall clock, and it is
    // only read to find expired holders on the leader, so per-node copies diverging by clock skew
    // or apply delay never changes who holds a lock.
    let mut locks: HashMap<String, LockQueue> = HashMap::new();
    let mut waiters: Waiters = HashMap::new();

    while let Ok(req) = rx.recv_async().await {
        handle_request(req, &mut locks, &mut waiters);
    }

    debug!("DLock handler exiting");
}

fn handle_request(req: LockRequest, locks: &mut HashMap<String, LockQueue>, waiters: &mut Waiters) {
    match req {
        LockRequest::Lock(p) => handle_lock(locks, waiters, p),
        LockRequest::Acquire(p) => handle_acquire(locks, waiters, p),
        LockRequest::Release(p) => handle_release(locks, waiters, &p),
        LockRequest::Alive(p) => handle_alive(locks, p),
        LockRequest::Await(p) => handle_await(locks, waiters, p),
        LockRequest::Expired(ack) => {
            let now = Utc::now().timestamp();
            let expired = locks
                .iter()
                .filter_map(|(key, lock)| match lock.current_ticket {
                    Some(id) if lock.exp < now => Some((key.clone(), id)),
                    _ => None,
                })
                .collect();
            if ack.send(expired).is_err() {
                debug!("DLock: receiver for expired locks dropped");
            }
        }
        LockRequest::SnapshotBuild(ack) => {
            if ack.send(locks.clone()).is_err() {
                debug!("DLock: receiver for snapshot build dropped");
            }
        }
        LockRequest::SnapshotInstall((data, ack)) => {
            *locks = data;
            // Waiters from the pre-snapshot state are stale: wake them all so their clients
            // re-request against the installed state instead of hanging.
            for (key, acks) in waiters.drain() {
                for (_, ack) in acks {
                    LockState::Released.send(ack, &key);
                }
            }
            debug!("DLock snapshot installed");
            if ack.send(()).is_err() {
                debug!("DLock: receiver for snapshot install dropped");
            }
        }
    }
}

/// Promote the front ticket of `lock`'s queue to holder. No-op if the lock is still held or the
/// queue is empty. Only ever called while applying a log entry.
///
/// The promoted ticket keeps its position in line - it is never skipped, re-queued, or dropped.
/// Every waiter registered for it receives `Locked` on its pending `Await`. If no waiter is
/// registered yet (the client's `Await` may still be in flight, or the client is dead), the lock
/// is reserved silently: the client can claim it via `Acquire` within the lease window, and if it
/// never shows up the leader expires the lease and the next promotion moves on. That bounds how
/// long a dead client can block its queue position to one lease window, without ever dropping
/// tickets or waking clients that are not at the front.
fn promote(lock: &mut LockQueue, waiters: &mut Waiters, key: &str) {
    if lock.current_ticket.is_some() {
        return;
    }
    let Some(front) = lock.queue.pop_front() else {
        return;
    };
    lock.current_ticket = Some(front);
    lock.exp = Utc::now().timestamp() + LOCK_VALID_SECONDS;

    // Wake every waiter registered for the promoted ticket, not just the first one: a duplicate
    // registration (at-least-once delivery) must not hang forever. Waiters for other tickets are
    // kept, in order.
    if let Some(list) = waiters.get_mut(key) {
        let (granted, rest): (Vec<_>, Vec<_>) = list.drain(..).partition(|(id, _)| *id == front);
        *list = rest;
        for (_, ack) in granted {
            LockState::Locked(front).send(ack, key);
        }
    }
}

fn promote_key(locks: &mut HashMap<String, LockQueue>, waiters: &mut Waiters, key: &str) {
    if let Some(lock) = locks.get_mut(key) {
        promote(lock, waiters, key);
    }
}

fn handle_lock(
    locks: &mut HashMap<String, LockQueue>,
    waiters: &mut Waiters,
    p: LockRequestPayload,
) {
    let key = p.key.as_ref();

    // A first try always carries a fresh ticket (the raft log index of this entry), which is
    // larger than every existing ticket. It never cuts in line: if the lock is free and clients
    // are queued, the front waiter is promoted first and this request goes to the back of the
    // queue.
    promote_key(locks, waiters, key);

    let now = Utc::now().timestamp();
    let state = if let Some(lock) = locks.get_mut(key) {
        if lock.current_ticket.is_none() {
            // The queue was empty: grant directly.
            lock.current_ticket = Some(p.log_id);
            lock.exp = now + LOCK_VALID_SECONDS;
            LockState::Locked(p.log_id)
        } else {
            lock.queue.push_back(p.log_id);
            LockState::Queued(p.log_id)
        }
    } else {
        locks.insert(
            key.to_string(),
            LockQueue {
                current_ticket: Some(p.log_id),
                exp: now + LOCK_VALID_SECONDS,
                queue: VecDeque::default(),
            },
        );
        LockState::Locked(p.log_id)
    };
    state.send(p.ack, key);
}

fn handle_acquire(
    locks: &mut HashMap<String, LockQueue>,
    waiters: &mut Waiters,
    p: LockRequestPayload,
) {
    let key = p.key.as_ref();

    // The client claims a ticket it was told to wait for (or that it already holds). If the
    // lock is free and clients are queued, the front waiter is promoted first.
    promote_key(locks, waiters, key);

    let now = Utc::now().timestamp();
    let state = if let Some(lock) = locks.get_mut(key) {
        if lock.current_ticket == Some(p.log_id) {
            // This ticket already holds the lock (granted directly, or reserved for us): claim
            // it. The claim starts a fresh lease, the heartbeats keep it alive from here.
            lock.exp = now + LOCK_VALID_SECONDS;
            LockState::Locked(p.log_id)
        } else if lock.current_ticket.is_none() {
            // The lock is free and nobody ahead of us was promoted: grant directly.
            lock.current_ticket = Some(p.log_id);
            lock.exp = now + LOCK_VALID_SECONDS;
            LockState::Locked(p.log_id)
        } else {
            // Behind another ticket (held, or just promoted): keep (or restore) our position
            // at the back of the queue.
            if !lock.queue.contains(&p.log_id) {
                lock.queue.push_back(p.log_id);
            }
            LockState::Queued(p.log_id)
        }
    } else {
        // The lock was fully removed while this request was in flight. Grant a fresh one so
        // the client never hangs.
        locks.insert(
            key.to_string(),
            LockQueue {
                current_ticket: Some(p.log_id),
                exp: now + LOCK_VALID_SECONDS,
                queue: VecDeque::default(),
            },
        );
        LockState::Locked(p.log_id)
    };
    state.send(p.ack, key);
}

fn handle_release(
    locks: &mut HashMap<String, LockQueue>,
    waiters: &mut Waiters,
    p: &LockReleasePayload,
) {
    let key = p.key.as_ref();
    let mut full_remove = false;

    if let Some(lock) = locks.get_mut(key) {
        if lock.current_ticket == Some(p.id) {
            lock.current_ticket = None;
            // Promote the front waiter now: it either gets `Locked` on its pending await, or
            // reserves the lock. Nobody else is woken.
            promote(lock, waiters, key);
            // Only remove the key when nothing is left to hold it: no holder and no queue.
            // (After a promotion there is always a holder, even if the queue drained.)
            if lock.current_ticket.is_none() && lock.queue.is_empty() {
                full_remove = true;
            }
        } else {
            // The leader expired the lease and the lock was granted to another ticket, or this
            // is a duplicate release. Releasing an already released / re-granted lock is safe to
            // ignore. Panicking here would kill the whole dlock handler.
            warn!(
                "Ignoring release for lock {key} / {}: current holder is not this \
                ticket (current_ticket: {:?})",
                p.id, lock.current_ticket
            );
        }
    }

    if full_remove {
        // The queue is empty and no holder remains: drop the key entirely. Wake any waiters
        // still registered for the removed lock so their clients can re-request instead of
        // hanging forever (defensive - a waiter's ticket should always be in the queue).
        if let Some(acks) = waiters.remove(key) {
            for (_, ack) in acks {
                LockState::Released.send(ack, key);
            }
        }
        locks.remove(key);
        debug!("DLock: lock key {key} fully released and removed");
    }
}

/// Extend the lease of a live holder. Sent by clients as a heartbeat (`LockAlive`) while they
/// hold a lock, so held locks are not capped at one lease window.
///
/// The extension is unconditional on purpose: a node whose local copy already expired self-heals
/// on the next heartbeat instead of being stuck with a stale lease. A late heartbeat from a dead
/// client can add at most one extra lease window, and only if the leader did not expire it in
/// between (that release changes `current_ticket`, making this a no-op anyway). Alive never
/// touches the queue or waiters: it promotes nothing and wakes nothing.
fn handle_alive(locks: &mut HashMap<String, LockQueue>, p: LockAlivePayload) {
    let key = p.key.as_ref();

    let state = match locks.get_mut(key) {
        Some(lock) if lock.current_ticket == Some(p.id) => {
            lock.exp = Utc::now().timestamp() + LOCK_VALID_SECONDS;
            LockState::Locked(p.id)
        }
        _ => LockState::Released,
    };
    state.send(p.ack, key);
}

/// Wait for a queued ticket's turn. `Await` is served by the node the client talks to, which may
/// be a follower that lags behind the log, and it never changes lock state. Its answer is only a
/// wake-up: the client claims the lock with an `Acquire` through Raft afterwards, and only that
/// answer grants it.
fn handle_await(
    locks: &mut HashMap<String, LockQueue>,
    waiters: &mut Waiters,
    p: LockAwaitPayload,
) {
    let key = p.key.as_ref();

    let state = match locks.get(key) {
        // The lock was promoted to us (or granted directly): go claim it.
        Some(lock) if lock.current_ticket == Some(p.id) => LockState::Locked(p.id),
        Some(lock) if lock.queue.contains(&p.id) => {
            // Still waiting in line: register; we are woken with `Locked` when our ticket is
            // promoted.
            waiters
                .entry(key.to_string())
                .or_default()
                .push((p.id, p.ack));
            return;
        }
        // Our ticket is neither holder nor queued (e.g. the leader expired our reservation), or
        // the lock was released and fully removed while this await was in flight: let the
        // client re-request via Acquire, it gets a fresh grant or is queued at the back again.
        Some(_) | None => LockState::Released,
    };
    state.send(p.ack, key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use tokio::sync::oneshot;

    fn send(tx: &flume::Sender<LockRequest>, req: LockRequest) {
        tx.send(req).expect("handler to be running");
    }

    /// Wait for an answer, failing instead of hanging when the handler never sends one.
    async fn answer<T>(rx: oneshot::Receiver<T>) -> T {
        tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .expect("handler to answer in time")
            .expect("handler to be running")
    }

    async fn lock(tx: &flume::Sender<LockRequest>, key: &str, log_id: u64) -> LockState {
        let (ack, rx) = oneshot::channel();
        send(
            tx,
            LockRequest::Lock(LockRequestPayload {
                key: Cow::Owned(key.to_string()),
                log_id,
                ack,
            }),
        );
        answer(rx).await
    }

    async fn acquire(tx: &flume::Sender<LockRequest>, key: &str, log_id: u64) -> LockState {
        let (ack, rx) = oneshot::channel();
        send(
            tx,
            LockRequest::Acquire(LockRequestPayload {
                key: Cow::Owned(key.to_string()),
                log_id,
                ack,
            }),
        );
        answer(rx).await
    }

    /// Like `await_lock`, but returns the pending receiver so a test can assert *when* (or
    /// whether) it is answered.
    fn await_pending(
        tx: &flume::Sender<LockRequest>,
        key: &str,
        id: u64,
    ) -> oneshot::Receiver<LockState> {
        let (ack, rx) = oneshot::channel();
        send(
            tx,
            LockRequest::Await(LockAwaitPayload {
                key: Cow::Owned(key.to_string()),
                id,
                ack,
            }),
        );
        rx
    }

    async fn await_lock(tx: &flume::Sender<LockRequest>, key: &str, id: u64) -> LockState {
        answer(await_pending(tx, key, id)).await
    }

    fn release(tx: &flume::Sender<LockRequest>, key: &str, id: u64) {
        send(
            tx,
            LockRequest::Release(LockReleasePayload {
                key: Cow::Owned(key.to_string()),
                id,
            }),
        );
    }

    fn alive(tx: &flume::Sender<LockRequest>, key: &str, id: u64) -> oneshot::Receiver<LockState> {
        let (ack, rx) = oneshot::channel();
        send(
            tx,
            LockRequest::Alive(LockAlivePayload {
                key: Cow::Owned(key.to_string()),
                id,
                ack,
            }),
        );
        rx
    }

    async fn alive_lock(tx: &flume::Sender<LockRequest>, key: &str, id: u64) -> LockState {
        answer(alive(tx, key, id)).await
    }

    async fn expired(tx: &flume::Sender<LockRequest>) -> Vec<(String, u64)> {
        let (ack, rx) = oneshot::channel();
        send(tx, LockRequest::Expired(ack));
        answer(rx).await
    }

    async fn snapshot_build(tx: &flume::Sender<LockRequest>) -> HashMap<String, LockQueue> {
        let (ack, rx) = oneshot::channel();
        send(tx, LockRequest::SnapshotBuild(ack));
        answer(rx).await
    }

    #[tokio::test]
    async fn lock_release_roundtrip() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        release(&tx, "k", 1);
        // no current holder anymore: the queued ticket is granted directly while waiting
        assert_eq!(await_lock(&tx, "k", 2).await, LockState::Locked(2));
        release(&tx, "k", 2);
    }

    #[tokio::test]
    async fn duplicate_release_is_ignored() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        release(&tx, "k", 1);
        // second release of the same ticket must not panic the handler
        release(&tx, "k", 1);
        // handler is still alive
        assert_eq!(lock(&tx, "k", 2).await, LockState::Locked(2));
    }

    #[tokio::test]
    async fn release_after_lock_was_removed_is_ignored() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        release(&tx, "k", 1); // no waiters -> lock removed entirely
        release(&tx, "k", 1); // stale release must be a no-op
        assert_eq!(lock(&tx, "k", 2).await, LockState::Locked(2));
    }

    #[tokio::test]
    async fn acquire_after_lock_was_removed_grants_fresh() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        release(&tx, "k", 1); // removed
        // a client re-claiming with an old ticket must not hang or panic
        assert_eq!(acquire(&tx, "k", 1).await, LockState::Locked(1));
        release(&tx, "k", 1);
    }

    #[tokio::test]
    async fn await_when_lock_was_removed_returns_released() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        release(&tx, "k", 1); // removed
        // an in-flight await must be answered, not hang
        assert_eq!(await_lock(&tx, "k", 1).await, LockState::Released);
        assert_eq!(acquire(&tx, "k", 1).await, LockState::Locked(1));
    }

    #[tokio::test]
    async fn late_release_after_takeover_is_ignored() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        release(&tx, "k", 1);
        // lock is free again, ticket 2 takes it
        assert_eq!(lock(&tx, "k", 2).await, LockState::Locked(2));
        // the old holder (ticket 1) releases late -> must be ignored, not panic
        release(&tx, "k", 1);
        // ticket 2 still holds the lock and can release it normally
        release(&tx, "k", 2);
        assert_eq!(lock(&tx, "k", 3).await, LockState::Locked(3));
    }

    #[tokio::test]
    async fn release_promotes_front_waiter_directly() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        // The client for ticket 2 registers its await (as the real client does after Queued).
        let pending = await_pending(&tx, "k", 2);
        release(&tx, "k", 1);
        // The front waiter is granted directly on its pending await: no extra round trip.
        assert_eq!(answer(pending).await, LockState::Locked(2));
        release(&tx, "k", 2);
    }

    #[tokio::test]
    async fn new_arrival_does_not_cut_in_line() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        // A new arrival while the queue is non-empty goes to the back, not the front.
        assert_eq!(lock(&tx, "k", 3).await, LockState::Queued(3));
        release(&tx, "k", 1);
        let snap = snapshot_build(&tx).await;
        // The front ticket was promoted to holder; the newcomer is still behind it.
        assert_eq!(snap["k"].current_ticket, Some(2));
        assert_eq!(snap["k"].queue, VecDeque::from([3u64]));
        // Ticket 3 must not be granted while ticket 2 holds the lock.
        let pending = await_pending(&tx, "k", 3);
        release(&tx, "k", 2);
        assert_eq!(answer(pending).await, LockState::Locked(3));
        release(&tx, "k", 3);
    }

    #[tokio::test]
    async fn release_wakes_only_the_front_waiter() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        assert_eq!(lock(&tx, "k", 3).await, LockState::Queued(3));
        // Both waiters register (as real clients do after Queued).
        let w2 = await_pending(&tx, "k", 2);
        let mut w3 = await_pending(&tx, "k", 3);
        release(&tx, "k", 1);
        // Ticket 2's waiter is granted directly on its pending await.
        assert_eq!(answer(w2).await, LockState::Locked(2));
        // w2 being answered means the handler has finished processing the release; ticket 3's
        // waiter must still be pending — it stays queued behind ticket 2, not woken early.
        assert!(matches!(
            w3.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        release(&tx, "k", 2);
        assert_eq!(answer(w3).await, LockState::Locked(3));
    }

    #[tokio::test]
    async fn await_claims_silent_reservation() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        release(&tx, "k", 1); // ticket 2 is reserved silently (no waiter registered yet)
        // The client's await arrives after the reservation: it claims the lock directly.
        assert_eq!(await_lock(&tx, "k", 2).await, LockState::Locked(2));
        release(&tx, "k", 2);
    }

    #[tokio::test]
    async fn duplicate_await_is_answered_at_promotion() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        // At-least-once delivery: two awaits registered for the same ticket.
        let a = await_pending(&tx, "k", 2);
        let b = await_pending(&tx, "k", 2);
        release(&tx, "k", 1);
        assert_eq!(answer(a).await, LockState::Locked(2));
        assert_eq!(answer(b).await, LockState::Locked(2));
        release(&tx, "k", 2);
    }

    #[tokio::test]
    async fn await_for_unqueued_ticket_returns_released() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        // Ticket 99 was never queued: the client re-requests via Acquire and is queued at the
        // back.
        assert_eq!(await_lock(&tx, "k", 99).await, LockState::Released);
        assert_eq!(acquire(&tx, "k", 99).await, LockState::Queued(99));
        release(&tx, "k", 1);
        // Ticket 99 is now the front: it claims its silent reservation.
        assert_eq!(acquire(&tx, "k", 99).await, LockState::Locked(99));
        release(&tx, "k", 99);
    }

    #[tokio::test]
    async fn expired_holder_waits_are_kept_and_promoted() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        // Ticket 1 never releases and never awaits: a dead holder. The queued ticket must keep
        // its position (the old behavior evicted it) and be promoted once the leader expires the
        // lease through the log.
        assert!(expired(&tx).await.is_empty());
        tokio::time::sleep(Duration::from_secs(LOCK_VALID_SECONDS as u64 + 2)).await;
        // Time alone changes nothing: the holder stays until the release is applied.
        assert_eq!(snapshot_build(&tx).await["k"].current_ticket, Some(1));
        assert_eq!(expired(&tx).await, vec![("k".to_string(), 1)]);
        release(&tx, "k", 1);
        let snap = snapshot_build(&tx).await;
        assert_eq!(snap["k"].current_ticket, Some(2));
        // Ticket 2 can claim its reservation.
        assert_eq!(acquire(&tx, "k", 2).await, LockState::Locked(2));
        release(&tx, "k", 2);
    }

    #[tokio::test]
    async fn dead_holder_does_not_block_waiter_forever() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        // The client for ticket 2 registers its await; ticket 1 then dies (no release, no
        // await).
        let pending = await_pending(&tx, "k", 2);
        tokio::time::sleep(Duration::from_secs(LOCK_VALID_SECONDS as u64 + 2)).await;
        // The leader finds the expired holder and writes its release through the log: no release
        // from the dead holder itself is ever needed. Applying it wakes the waiter.
        for (key, id) in expired(&tx).await {
            release(&tx, &key, id);
        }
        assert_eq!(answer(pending).await, LockState::Locked(2));
        release(&tx, "k", 2);
    }

    #[tokio::test]
    async fn snapshot_install_preserves_state_and_wakes_waiters() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        let snap = snapshot_build(&tx).await;

        // Install into a fresh handler.
        let tx2 = spawn();
        let (ack, rx) = oneshot::channel();
        send(&tx2, LockRequest::SnapshotInstall((snap.clone(), ack)));
        answer(rx).await;

        // The installed handler sees the same queue.
        let snap2 = snapshot_build(&tx2).await;
        assert_eq!(snap2["k"].current_ticket, Some(1));
        assert_eq!(snap2["k"].queue, VecDeque::from([2u64]));

        // A waiter registered against the installed state is woken when a second install
        // replaces the state (pre-snapshot waiters are stale).
        let pending = await_pending(&tx2, "k", 2);
        let (ack2, rx2) = oneshot::channel();
        send(&tx2, LockRequest::SnapshotInstall((snap, ack2)));
        answer(rx2).await;
        assert_eq!(answer(pending).await, LockState::Released);

        // The client re-requests and is queued again against the installed state.
        assert_eq!(acquire(&tx2, "k", 2).await, LockState::Queued(2));
    }

    #[tokio::test]
    async fn alive_extends_lease_past_expiry() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        // Heartbeat just before the original lease would expire.
        tokio::time::sleep(Duration::from_secs(LOCK_VALID_SECONDS as u64 - 1)).await;
        assert_eq!(alive_lock(&tx, "k", 1).await, LockState::Locked(1));
        // Past the original expiry: without the heartbeat the leader would expire ticket 1 by
        // now; with it, ticket 1 still holds.
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(expired(&tx).await.is_empty());
        let snap = snapshot_build(&tx).await;
        assert_eq!(snap["k"].current_ticket, Some(1));
        release(&tx, "k", 1);
    }

    #[tokio::test]
    async fn alive_for_non_holder_is_noop() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        // A heartbeat for a ticket that does not hold the lock answers Released.
        assert_eq!(alive_lock(&tx, "k", 99).await, LockState::Released);
        let snap = snapshot_build(&tx).await;
        assert_eq!(snap["k"].current_ticket, Some(1));
        release(&tx, "k", 1);
    }

    #[tokio::test]
    async fn alive_after_release_returns_released() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        release(&tx, "k", 1); // key fully removed
        assert_eq!(alive_lock(&tx, "k", 1).await, LockState::Released);
    }

    #[tokio::test]
    async fn same_log_gives_same_state_regardless_of_apply_timing() {
        // Two nodes apply the same log. The leader applies the holder's heartbeats in time, the
        // follower lags behind and applies them only later. Lock state must only ever follow the
        // log, so both end up with the same holder, and the follower must not grant the lock to a
        // waiter on its own while the holder keeps its lease alive through the log.
        let leader = spawn();
        let follower = spawn();
        for tx in [&leader, &follower] {
            assert_eq!(lock(tx, "k", 1).await, LockState::Locked(1));
            assert_eq!(lock(tx, "k", 2).await, LockState::Queued(2));
        }
        // An embedded client on the follower waits for ticket 2 on its local handler.
        let mut pending = await_pending(&follower, "k", 2);

        let heartbeats = LOCK_VALID_SECONDS.cast_unsigned() + 2;
        for _ in 0..heartbeats {
            tokio::time::sleep(Duration::from_secs(1)).await;
            assert_eq!(alive_lock(&leader, "k", 1).await, LockState::Locked(1));
        }
        // The follower catches up and applies the same heartbeats.
        for _ in 0..heartbeats {
            let _ = alive_lock(&follower, "k", 1).await;
        }

        let snap_leader = snapshot_build(&leader).await;
        let snap_follower = snapshot_build(&follower).await;
        assert_eq!(snap_leader["k"].current_ticket, Some(1));
        assert_eq!(
            snap_follower["k"].current_ticket,
            snap_leader["k"].current_ticket
        );
        assert_eq!(snap_follower["k"].queue, snap_leader["k"].queue);
        assert!(matches!(
            pending.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn dropped_await_receiver_does_not_kill_the_handler() {
        let tx = spawn();
        assert_eq!(lock(&tx, "k", 1).await, LockState::Locked(1));
        // A cancelled `lock()` future drops its await receiver before the answer is sent.
        // Ticket 99 is not queued, so the answer is sent immediately.
        drop(await_pending(&tx, "k", 99));
        // The holder is answered the same way, immediately, and is gone as well.
        drop(await_pending(&tx, "k", 1));
        // A registered waiter that goes away before its promotion.
        assert_eq!(lock(&tx, "j", 1).await, LockState::Locked(1));
        assert_eq!(lock(&tx, "j", 2).await, LockState::Queued(2));
        drop(await_pending(&tx, "j", 2));
        release(&tx, "j", 1);
        // The undelivered grant is a silent reservation: it is claimable, or expires with its
        // lease.
        assert_eq!(snapshot_build(&tx).await["j"].current_ticket, Some(2));
        // The handler keeps serving every later request.
        assert_eq!(lock(&tx, "k", 2).await, LockState::Queued(2));
        assert_eq!(alive_lock(&tx, "k", 1).await, LockState::Locked(1));
    }
}
