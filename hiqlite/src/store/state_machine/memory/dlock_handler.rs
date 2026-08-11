use chrono::{DateTime, Utc};
use openraft::LogState;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::Add;
use std::sync::atomic::AtomicU64;
use std::thread;
use tokio::sync::oneshot;
use tokio::task;
use tracing::{debug, error, info, warn};

const LOCK_VALID_SECONDS: i64 = 10;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    let mut queues: HashMap<String, Vec<(u64, oneshot::Sender<LockState>)>> = HashMap::new();

    while let Ok(req) = rx.recv_async().await {
        match req {
            LockRequest::Lock(LockRequestPayload { key, log_id, ack }) => {
                let now = Utc::now().timestamp();
                if let Some(lock) = locks.get_mut(key.as_ref()) {
                    // If the lease of the current holder has expired, the holder is considered
                    // dead. Any ticket at the front of the queue that had a full lease window to
                    // reclaim the lock but did not is dead as well: drop it (and wake its client,
                    // if it is still waiting) so it can never block the lock forever.
                    if lock.exp < now {
                        while let Some(&ticket) = lock.queue.front()
                            && ticket != log_id
                        {
                            lock.queue.pop_front();
                            if let Some(acks) = queues.get_mut(key.as_ref())
                                && let Some(pos) = acks.iter().position(|(i, _)| *i == ticket)
                            {
                                let (_, ack) = acks.swap_remove(pos);
                                let _ = ack.send(LockState::Released);
                            }
                        }
                    }

                    if lock.exp < now || lock.current_ticket.is_none() {
                        let front = lock.queue.front();
                        if let Some(ticket) = front {
                            if *ticket == log_id {
                                lock.queue.pop_front();
                                lock.current_ticket = Some(log_id);
                                lock.exp = now + LOCK_VALID_SECONDS;
                                ack.send(LockState::Locked(log_id)).unwrap();
                            } else {
                                lock.queue.push_back(log_id);
                                ack.send(LockState::Queued(log_id)).unwrap();
                            }
                        } else {
                            lock.current_ticket = Some(log_id);
                            lock.exp = now + LOCK_VALID_SECONDS;
                            ack.send(LockState::Locked(log_id)).unwrap();
                        }
                    } else {
                        lock.queue.push_back(log_id);
                        ack.send(LockState::Queued(log_id)).unwrap();
                    }
                } else {
                    locks.insert(
                        key.to_string(),
                        LockQueue {
                            current_ticket: Some(log_id),
                            exp: now + LOCK_VALID_SECONDS,
                            queue: Default::default(),
                        },
                    );
                    ack.send(LockState::Locked(log_id)).unwrap();
                }
            }

            LockRequest::Acquire(LockRequestPayload { key, log_id, ack }) => {
                if let Some(lock) = locks.get_mut(key.as_ref()) {
                    if lock.current_ticket.is_some() {
                        // Someone else holds the lock (e.g. our lease expired and the lock was
                        // re-granted). Re-queue and report back so the client can wait again.
                        lock.queue.push_back(log_id);
                        ack.send(LockState::Queued(log_id)).unwrap();
                    } else if let Some(first) = lock.queue.front() {
                        if *first == log_id {
                            lock.queue.pop_front();
                            lock.current_ticket = Some(log_id);
                            lock.exp = Utc::now().timestamp() + LOCK_VALID_SECONDS;
                            ack.send(LockState::Locked(log_id)).unwrap();
                        } else {
                            // Our ticket is not the promoted one anymore -> re-queue.
                            lock.queue.push_back(log_id);
                            ack.send(LockState::Queued(log_id)).unwrap();
                        }
                    } else {
                        // Nobody is queued and nobody holds the lock -> take it directly.
                        lock.current_ticket = Some(log_id);
                        lock.exp = Utc::now().timestamp() + LOCK_VALID_SECONDS;
                        ack.send(LockState::Locked(log_id)).unwrap();
                    }
                } else {
                    // The lock was fully removed while this request was in flight. Grant a fresh
                    // one so the client never hangs.
                    locks.insert(
                        key.to_string(),
                        LockQueue {
                            current_ticket: Some(log_id),
                            exp: Utc::now().timestamp() + LOCK_VALID_SECONDS,
                            queue: Default::default(),
                        },
                    );
                    ack.send(LockState::Locked(log_id)).unwrap();
                }
            }

            LockRequest::Release(LockReleasePayload { key, id }) => {
                let mut full_remove = false;

                if let Some(lock) = locks.get_mut(key.as_ref()) {
                    if lock.current_ticket == Some(id) {
                        lock.current_ticket = None;

                        if let Some(first) = lock.queue.front() {
                            if let Some(acks) = queues.get_mut(key.as_ref()) {
                                let pos_opt = acks.iter().position(|(i, _)| i == first);
                                if let Some(pos) = pos_opt
                                    && let Err(err) =
                                        acks.swap_remove(pos).1.send(LockState::Released)
                                {
                                    // The client may have disconnected - this is not fatal.
                                    warn!(
                                        "Error sending lock await response for lock {key}: {err:?}"
                                    );
                                }
                            }
                        } else {
                            full_remove = true;
                        }
                    } else {
                        // The lease expired and the lock was granted to another ticket, or this
                        // is a duplicate release. Releasing an already released / re-granted lock
                        // is safe to ignore. Panicking here would kill the whole dlock handler.
                        warn!(
                            "Ignoring release for lock {key} / {id}: current holder is not this \
                            ticket (current_ticket: {:?})",
                            lock.current_ticket
                        );
                    }
                }

                if full_remove {
                    locks.remove(key.as_ref());
                }
            }

            LockRequest::Await(LockAwaitPayload { key, id, ack }) => {
                let now = Utc::now().timestamp();

                if let Some(lock) = locks.get_mut(key.as_ref()) {
                    // Same dead-ticket handling as in LockRequest::Lock.
                    if lock.exp < now {
                        while let Some(&ticket) = lock.queue.front()
                            && ticket != id
                        {
                            lock.queue.pop_front();
                            if let Some(acks) = queues.get_mut(key.as_ref())
                                && let Some(pos) = acks.iter().position(|(i, _)| *i == ticket)
                            {
                                let (_, ack) = acks.swap_remove(pos);
                                let _ = ack.send(LockState::Released);
                            }
                        }
                    }

                    if lock.exp < now || lock.current_ticket.is_none() {
                        let front = lock.queue.front();
                        if let Some(ticket) = front {
                            if *ticket == id {
                                lock.queue.pop_front();
                                lock.current_ticket = Some(id);
                                lock.exp = now + LOCK_VALID_SECONDS;
                                ack.send(LockState::Locked(id)).unwrap();
                            } else if let Some(queue) = queues.get_mut(key.as_ref()) {
                                queue.push((id, ack));
                            } else {
                                queues.insert(key.to_string(), vec![(id, ack)]);
                            }
                        } else {
                            // Nothing to wait for: no current holder and no queued ticket. Let the
                            // client re-request, it will get a fresh grant.
                            ack.send(LockState::Released).unwrap();
                        }
                    } else if let Some(queue) = queues.get_mut(key.as_ref()) {
                        queue.push((id, ack));
                    } else {
                        queues.insert(key.to_string(), vec![(id, ack)]);
                    }
                } else {
                    // The lock was released and fully removed while this await was in flight.
                    // Let the client re-request, it will get a fresh grant.
                    ack.send(LockState::Released).unwrap();
                }
            }

            LockRequest::SnapshotBuild(ack) => ack.send(locks.clone()).unwrap(),

            LockRequest::SnapshotInstall((data, ack)) => {
                locks = data;
                ack.send(()).unwrap()
            }
        }
    }

    debug!("DLock handler exiting");
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

    async fn await_lock(tx: &flume::Sender<LockRequest>, key: &str, id: u64) -> LockState {
        let (ack, rx) = oneshot::channel();
        send(
            tx,
            LockRequest::Await(LockAwaitPayload {
                key: Cow::Owned(key.to_string()),
                id,
                ack,
            }),
        );
        rx.await.unwrap()
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
}
