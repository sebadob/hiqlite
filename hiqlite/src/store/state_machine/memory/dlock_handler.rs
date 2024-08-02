use chrono::{DateTime, Utc};
use openraft::LogState;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::ops::Add;
use std::sync::atomic::AtomicU64;
use tokio::sync::oneshot;
use tokio::task;
use tracing::{error, warn};

const LOCK_VALID: chrono::Duration = chrono::Duration::seconds(10);

pub enum LockRequest {
    Lock(LockRequestPayload),
    Release(LockReleasePayload),
    // Refresh(LockRequestPayload),
    Await(LockAwaitPayload),
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

#[derive(Debug, Serialize, Deserialize)]
pub enum LockState {
    Locked(u64),
    Queued(u64),
    Released,
}

#[derive(Debug)]
struct LockQueue {
    current_ticket: Option<u64>,
    exp: DateTime<Utc>,
    queue: VecDeque<u64>,
}

pub fn spawn() -> flume::Sender<LockRequest> {
    let (tx, rx) = flume::unbounded();
    task::spawn(lock_handler(rx));
    tx
}

async fn lock_handler(rx: flume::Receiver<LockRequest>) {
    let mut locks: HashMap<String, LockQueue> = HashMap::new();
    let mut queues: HashMap<String, Vec<(u64, oneshot::Sender<LockState>)>> = HashMap::new();

    while let Ok(req) = rx.recv_async().await {
        match req {
            LockRequest::Lock(LockRequestPayload { key, log_id, ack }) => {
                let now = Utc::now();
                if let Some(lock) = locks.get_mut(key.as_ref()) {
                    if lock.exp < now || lock.current_ticket.is_none() {
                        let front = lock.queue.front();
                        if let Some(ticket) = front {
                            if *ticket == log_id {
                                lock.queue.pop_front();
                                lock.current_ticket = Some(log_id);
                                lock.exp = now.add(LOCK_VALID);
                                ack.send(LockState::Locked(log_id)).unwrap();
                            } else {
                                lock.queue.push_back(log_id);
                                ack.send(LockState::Queued(log_id)).unwrap();
                            }
                        } else {
                            lock.queue.pop_front();
                            lock.current_ticket = Some(log_id);
                            lock.exp = now.add(LOCK_VALID);
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
                            exp: now.add(LOCK_VALID),
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
                        if let Some(first) = lock.queue.pop_front() {
                            lock.current_ticket = Some(first);
                            lock.exp = Utc::now().add(LOCK_VALID);

                            if let Some(acks) = queues.get_mut(key.as_ref()) {
                                let pos_opt = acks.iter().position(|(i, _)| *i == id);
                                if let Some(pos) = pos_opt {
                                    if let Err(err) =
                                        acks.swap_remove(pos).1.send(LockState::Locked(id))
                                    {
                                        panic!(
                                            "Error sending lock await response for lock {}: {:?}",
                                            key, err
                                        );
                                    }
                                }
                            }
                        } else {
                            full_remove = true;
                        }
                    } else {
                        // TODO can this ever happen?
                        panic!("Lock for {} / {} as been released already", key, id);
                    }
                }

                if full_remove {
                    locks.remove(key.as_ref());
                }
            }

            LockRequest::Await(LockAwaitPayload { key, id, ack }) => {
                let now = Utc::now();

                if let Some(lock) = locks.get_mut(key.as_ref()) {
                    debug_assert!(lock.current_ticket == Some(id) || lock.queue.contains(&id));

                    if lock.exp < now || lock.current_ticket.is_none() {
                        let front = lock.queue.front();
                        if let Some(ticket) = front {
                            if *ticket == id {
                                lock.queue.pop_front();
                                lock.current_ticket = Some(id);
                                lock.exp = now.add(LOCK_VALID);
                                ack.send(LockState::Locked(id)).unwrap();
                            } else if let Some(queue) = queues.get_mut(key.as_ref()) {
                                queue.push((id, ack));
                            } else {
                                queues.insert(key.to_string(), vec![(id, ack)]);
                            }
                        } else {
                            panic!("for a LockAwait there must always be at least 1 element in the queue when the current_ticket is None");
                        }
                    } else if let Some(queue) = queues.get_mut(key.as_ref()) {
                        queue.push((id, ack));
                    } else {
                        queues.insert(key.to_string(), vec![(id, ack)]);
                    }
                } else {
                    panic!("The lock should always exist when we receive an await");
                }
            }
        }
    }
}
