use bincode_next::{Decode, Encode};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task;
use tracing::{debug, warn};

/// How long a granted (or reserved) ticket is considered alive before its holder is treated as
/// dead.
const LOCK_VALID_SECONDS: i64 = 10;
/// How often the handler wakes itself up to promote queued tickets. Promotion is also triggered
/// by every lock request and release for that key, so this only bounds how long a queue waits
/// behind an expired (dead) holder when no further requests arrive for that key.
const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

pub enum LockRequest {
    /// used for a first try lock without coming from a queue
    Lock(LockRequestPayload),
    /// used after an await to acquire the lock now
    Acquire(LockRequestPayload),
    Release(LockReleasePayload),
    Await(LockAwaitPayload),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum LockState {
    Locked(u64),
    Queued(u64),
    Released,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockQueue {
    current_ticket: Option<u64>,
    exp: i64,
    queue: VecDeque<u64>,
}

/// Local registry of in-flight `Await` calls per lock key. It is not part of the snapshot:
/// waiters are ephemeral, and every registered entry is answered exactly once — with
/// `Locked(id)` when its ticket is promoted, or with `Released` when the key is fully removed
/// or a snapshot is installed. A waiter's ticket is always in that key's queue while it is
/// un-answered, so no waiter can block forever.
type Waiters = HashMap<String, Vec<(u64, oneshot::Sender<LockState>)>>;

pub fn spawn() -> flume::Sender<LockRequest> {
    let (tx, rx) = flume::unbounded();
    task::spawn(handler(rx));
    tx
}

async fn handler(rx: flume::Receiver<LockRequest>) {
    // Lease timing (`exp`) intentionally uses this node's wall clock. All lock decisions are made
    // by the Raft leader's handler, so they are always consistent with the leader's clock. The
    // per-node `exp` copies in the state machine diverge by clock skew, but that is benign: Raft
    // never verifies state machine equality, and the new leader evaluates leases with its own
    // clock. A deterministic timestamp inside the raft entry would require changing the entry
    // format, which would break log compatibility for rolling upgrades. Keep the clocks within
    // ~1s of each other so a 10s lease survives failover comfortably.
    let mut locks: HashMap<String, LockQueue> = HashMap::new();
    let mut waiters: Waiters = HashMap::new();

    let mut ticker =
        tokio::time::interval_at(tokio::time::Instant::now() + SWEEP_INTERVAL, SWEEP_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            req = rx.recv_async() => match req {
                Ok(req) => handle_request(req, &mut locks, &mut waiters),
                Err(_) => break,
            },
            _ = ticker.tick() => sweep_expired(&mut locks, &mut waiters),
        }
    }

    debug!("DLock handler exiting");
}

fn handle_request(req: LockRequest, locks: &mut HashMap<String, LockQueue>, waiters: &mut Waiters) {
    match req {
        LockRequest::Lock(p) => handle_lock(locks, waiters, p),
        LockRequest::Acquire(p) => handle_acquire(locks, waiters, p),
        LockRequest::Release(p) => handle_release(locks, waiters, p),
        LockRequest::Await(p) => handle_await(locks, waiters, p),
        LockRequest::SnapshotBuild(ack) => ack.send(locks.clone()).unwrap(),
        LockRequest::SnapshotInstall((data, ack)) => {
            *locks = data;
            // Waiters from the pre-snapshot state are stale: wake them all so their clients
            // re-request against the installed state instead of hanging.
            for (_, acks) in waiters.drain() {
                for (_, ack) in acks {
                    let _ = ack.send(LockState::Released);
                }
            }
            debug!("DLock snapshot installed");
            ack.send(()).unwrap()
        }
    }
}

/// Can this lock be handed out right now: no holder, or the holder's lease has expired (dead
/// client).
fn is_free(lock: &LockQueue, now: i64) -> bool {
    lock.current_ticket.is_none() || lock.exp < now
}

/// Promote the front ticket of `lock`'s queue to holder. No-op if the lock is still held or the
/// queue is empty.
///
/// The promoted ticket keeps its position in line — it is never skipped, re-queued, or dropped.
/// Every waiter registered for it receives `Locked` directly on its pending `Await`, so no extra
/// round trip is needed. If no waiter is registered yet (the client's `Await` may still be in
/// flight, or the client is dead), the lock is reserved silently: the client can claim it via
/// `Acquire`/`Await` within the lease window, and if it never shows up the lease expires and the
/// next promotion moves on. That bounds how long a dead client can block its queue position to
/// one lease window, without ever dropping tickets or waking clients that are not at the front.
fn promote(lock: &mut LockQueue, waiters: &mut Waiters, key: &str) {
    let now = Utc::now().timestamp();
    if !is_free(lock, now) || lock.queue.is_empty() {
        return;
    }

    let front = *lock.queue.front().unwrap();
    lock.queue.pop_front();
    lock.current_ticket = Some(front);
    lock.exp = now + LOCK_VALID_SECONDS;

    // Wake every waiter registered for the promoted ticket, not just the first one: a duplicate
    // registration (at-least-once delivery) must not hang forever. Waiters for other tickets are
    // kept, in order.
    if let Some(list) = waiters.get_mut(key) {
        let mut granted: Vec<(u64, oneshot::Sender<LockState>)> = Vec::new();
        let mut rest: Vec<(u64, oneshot::Sender<LockState>)> = Vec::new();
        for entry in list.drain(..) {
            if entry.0 == front {
                granted.push(entry);
            } else {
                rest.push(entry);
            }
        }
        list.extend(rest);
        for (_, ack) in granted.drain(..) {
            if ack.send(LockState::Locked(front)).is_err() {
                debug!("DLock: await receiver for {key}/{front} dropped (client gone)");
            }
        }
    }
}

fn promote_key(locks: &mut HashMap<String, LockQueue>, waiters: &mut Waiters, key: &str) {
    if let Some(lock) = locks.get_mut(key) {
        promote(lock, waiters, key);
    }
}

/// Wake-up path for when no request arrives for a key: promote the front of every queue whose
/// holder is gone (no holder, or expired lease). This bounds how long a dead client can hold its
/// queue position to one lease window plus this interval, so waiters never block forever on
/// silence.
fn sweep_expired(locks: &mut HashMap<String, LockQueue>, waiters: &mut Waiters) {
    let now = Utc::now().timestamp();
    let keys: Vec<String> = locks
        .iter()
        .filter(|(_, lock)| is_free(lock, now) && !lock.queue.is_empty())
        .map(|(key, _)| key.clone())
        .collect();

    for key in &keys {
        promote_key(locks, waiters, key);
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
    match locks.get_mut(key) {
        Some(lock) => {
            if lock.current_ticket.is_none() || is_free(lock, now) {
                // The queue was empty: grant directly.
                lock.current_ticket = Some(p.log_id);
                lock.exp = now + LOCK_VALID_SECONDS;
                p.ack.send(LockState::Locked(p.log_id)).unwrap();
            } else {
                lock.queue.push_back(p.log_id);
                p.ack.send(LockState::Queued(p.log_id)).unwrap();
            }
        }
        None => {
            locks.insert(
                key.to_string(),
                LockQueue {
                    current_ticket: Some(p.log_id),
                    exp: now + LOCK_VALID_SECONDS,
                    queue: Default::default(),
                },
            );
            p.ack.send(LockState::Locked(p.log_id)).unwrap();
        }
    }
}

fn handle_acquire(
    locks: &mut HashMap<String, LockQueue>,
    waiters: &mut Waiters,
    p: LockRequestPayload,
) {
    let key = p.key.as_ref();

    // The client re-claims a ticket it was told to wait for (or that it already holds). If the
    // lock is free and clients are queued, the front waiter is promoted first.
    promote_key(locks, waiters, key);

    let now = Utc::now().timestamp();
    match locks.get_mut(key) {
        Some(lock) => {
            if lock.current_ticket == Some(p.log_id) && lock.exp >= now {
                // This ticket already holds the lock (granted directly, or reserved for us):
                // claim it.
                p.ack.send(LockState::Locked(p.log_id)).unwrap();
            } else if lock.current_ticket.is_none() || is_free(lock, now) {
                // The lock is free and nobody ahead of us was promoted: grant directly.
                lock.current_ticket = Some(p.log_id);
                lock.exp = now + LOCK_VALID_SECONDS;
                p.ack.send(LockState::Locked(p.log_id)).unwrap();
            } else {
                // Behind another ticket (held, or just promoted): keep (or restore) our position
                // at the back of the queue.
                if !lock.queue.contains(&p.log_id) {
                    lock.queue.push_back(p.log_id);
                }
                p.ack.send(LockState::Queued(p.log_id)).unwrap();
            }
        }
        None => {
            // The lock was fully removed while this request was in flight. Grant a fresh one so
            // the client never hangs.
            locks.insert(
                key.to_string(),
                LockQueue {
                    current_ticket: Some(p.log_id),
                    exp: now + LOCK_VALID_SECONDS,
                    queue: Default::default(),
                },
            );
            p.ack.send(LockState::Locked(p.log_id)).unwrap();
        }
    }
}

fn handle_release(
    locks: &mut HashMap<String, LockQueue>,
    waiters: &mut Waiters,
    p: LockReleasePayload,
) {
    let key = p.key.as_ref();
    let mut full_remove = false;

    if let Some(lock) = locks.get_mut(key) {
        if lock.current_ticket == Some(p.id) {
            lock.current_ticket = None;
            // Promote the front waiter now: it either gets `Locked` directly on its pending
            // await, or reserves the lock. Nobody else is woken.
            promote(lock, waiters, key);
            // Only remove the key when nothing is left to hold it: no holder and no queue.
            // (After a promotion there is always a holder, even if the queue drained.)
            if lock.current_ticket.is_none() && lock.queue.is_empty() {
                full_remove = true;
            }
        } else {
            // The lease expired and the lock was granted to another ticket, or this is a
            // duplicate release. Releasing an already released / re-granted lock is safe to
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
        // hanging forever (defensive — a waiter's ticket should always be in the queue).
        if let Some(acks) = waiters.remove(key) {
            for (_, ack) in acks {
                let _ = ack.send(LockState::Released);
            }
        }
        locks.remove(key);
        debug!("DLock: lock key {key} fully released and removed");
    }
}

fn handle_await(
    locks: &mut HashMap<String, LockQueue>,
    waiters: &mut Waiters,
    p: LockAwaitPayload,
) {
    let key = p.key.as_ref();

    // If the lock is free and clients are queued, the front waiter is promoted first (it gets
    // `Locked` directly on its own pending await).
    promote_key(locks, waiters, key);

    let now = Utc::now().timestamp();
    match locks.get_mut(key) {
        Some(lock) => {
            if lock.current_ticket == Some(p.id) && lock.exp >= now {
                // The lock was reserved for us (or granted directly): claim it.
                p.ack.send(LockState::Locked(p.id)).unwrap();
            } else if is_free(lock, now) {
                // Free with nobody queued (the front was promoted above): let the client
                // re-request via Acquire, it will get a fresh grant.
                p.ack.send(LockState::Released).unwrap();
            } else if lock.queue.contains(&p.id) {
                // Still waiting in line: register; we are woken with `Locked` when our ticket
                // is promoted.
                waiters
                    .entry(key.to_string())
                    .or_default()
                    .push((p.id, p.ack));
            } else {
                // Our ticket left the queue (e.g. a reservation expired): let the client
                // re-request via Acquire, it will be queued at the back again.
                p.ack.send(LockState::Released).unwrap();
            }
        }
        None => {
            // The lock was released and fully removed while this await was in flight. Let the
            // client re-request, it will get a fresh grant.
            p.ack.send(LockState::Released).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use tokio::sync::oneshot;

    fn send(tx: &flume::Sender<LockRequest>, req: LockRequest) {
        tx.send(req).expect("handler to be running");
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
        rx.await.unwrap()
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
        rx.await.unwrap()
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
        await_pending(tx, key, id).await.unwrap()
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

    async fn snapshot_build(tx: &flume::Sender<LockRequest>) -> HashMap<String, LockQueue> {
        let (ack, rx) = oneshot::channel();
        send(tx, LockRequest::SnapshotBuild(ack));
        rx.await.unwrap()
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
        assert_eq!(pending.await.unwrap(), LockState::Locked(2));
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
        assert_eq!(pending.await.unwrap(), LockState::Locked(3));
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
        assert_eq!(w2.await.unwrap(), LockState::Locked(2));
        // w2 being answered means the handler has finished processing the release; ticket 3's
        // waiter must still be pending — it stays queued behind ticket 2, not woken early.
        assert!(matches!(
            w3.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        release(&tx, "k", 2);
        assert_eq!(w3.await.unwrap(), LockState::Locked(3));
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
        assert_eq!(a.await.unwrap(), LockState::Locked(2));
        assert_eq!(b.await.unwrap(), LockState::Locked(2));
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
        // its position (the old behavior evicted it) and be promoted once the lease expires.
        tokio::time::sleep(Duration::from_secs(LOCK_VALID_SECONDS as u64 + 2)).await;
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
        // The sweep promoted ticket 2 and granted it directly on its pending await: no release
        // from the dead holder was ever needed.
        assert_eq!(pending.await.unwrap(), LockState::Locked(2));
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
        rx.await.unwrap();

        // The installed handler sees the same queue.
        let snap2 = snapshot_build(&tx2).await;
        assert_eq!(snap2["k"].current_ticket, Some(1));
        assert_eq!(snap2["k"].queue, VecDeque::from([2u64]));

        // A waiter registered against the installed state is woken when a second install
        // replaces the state (pre-snapshot waiters are stale).
        let pending = await_pending(&tx2, "k", 2);
        let (ack2, rx2) = oneshot::channel();
        send(&tx2, LockRequest::SnapshotInstall((snap, ack2)));
        rx2.await.unwrap();
        assert_eq!(pending.await.unwrap(), LockState::Released);

        // The client re-requests and is queued again against the installed state.
        assert_eq!(acquire(&tx2, "k", 2).await, LockState::Queued(2));
    }
}
