use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use tokio::sync::oneshot;
use tracing::warn;

#[allow(clippy::type_complexity)]
pub enum Action {
    Logs {
        from: u64,
        until: u64,
        ack: flume::Sender<Option<Result<Vec<u8>, Error>>>,
    },
    LogState(oneshot::Sender<Result<LogState, Error>>),
    Vote(oneshot::Sender<Result<Option<Vec<u8>>, Error>>),
    Shutdown,
}

#[derive(Debug)]
pub struct LogState {
    pub last_purged_log_id: Option<Vec<u8>>,
    pub last_log: Option<Vec<u8>>,
}

pub fn spawn(
    meta: Arc<RwLock<Metadata>>,
    wal_set: Arc<RwLock<WalFileSet>>,
    id_until: Arc<AtomicU64>,
    latest_wal: Arc<AtomicU64>,
) -> Result<flume::Sender<Action>, Error> {
    let (tx, rx) = flume::bounded::<Action>(1);
    thread::spawn(move || run(meta, wal_set, id_until, latest_wal, rx));
    Ok(tx)
}

fn run(
    meta: Arc<RwLock<Metadata>>,
    wal_set: Arc<RwLock<WalFileSet>>,
    id_until: Arc<AtomicU64>,
    latest_wal: Arc<AtomicU64>,
    rx: flume::Receiver<Action>,
) {
    // we keep the local set for faster access inside the loop and lazily update if necessary
    let mut wal = wal_set.read().unwrap().clone_no_map();
    let mut buf = Vec::with_capacity(16);

    while let Ok(action) = rx.recv() {
        match action {
            Action::Logs { from, until, ack } => {
                // lazily update our `WalFileSet`
                let active = wal.active();
                if latest_wal.load(Ordering::Relaxed) != active.wal_no {
                    wal = wal_set.read().unwrap().clone_no_map();
                } else {
                    active.id_until = id_until.load(Ordering::Relaxed);
                }

                let mut next = from;
                for log in wal.files.iter_mut() {
                    if log.id_until < next {
                        continue;
                    }

                    // if mmap fails, there is probably no way to recover anyway
                    log.mmap().unwrap();
                    buf.clear();

                    if log.id_until < until {
                        // log reading should not fail as well, no way to recover from mmap issues
                        log.read_logs(next, log.id_until, &mut buf).unwrap();
                        for (_, data) in buf.drain(..) {
                            ack.send(Some(Ok(data))).unwrap()
                        }

                        // If the until goes beyond our current file, we want to remove the mmap
                        // to save memory. Only if the system needs a log snapshot to recover
                        // another node, it may need lower log IDs again.
                        log.mmap_drop();

                        next = log.id_until + 1;
                    } else {
                        // log reading should not fail as well, no way to recover from mmap issues
                        log.read_logs(next, until, &mut buf).unwrap();
                        for (_, data) in buf.drain(..) {
                            ack.send(Some(Ok(data))).unwrap()
                        }

                        next = until;
                    };

                    if log.id_from <= from && log.id_until >= from {}
                }

                ack.send(None).unwrap();
            }
            Action::LogState(ack) => {
                let latest_id = latest_wal.load(Ordering::Relaxed);
                let last_log = if latest_id > 0 {
                    let active = wal.active();
                    if latest_id != active.wal_no {
                        wal = wal_set.read().unwrap().clone_no_map();
                    } else {
                        active.id_until = id_until.load(Ordering::Relaxed);
                    }
                    buf.clear();
                    wal.active()
                        .read_logs(latest_id, latest_id, &mut buf)
                        .unwrap();
                    let (_, data) = buf.swap_remove(0);
                    Some(data)
                } else {
                    None
                };

                match meta.read() {
                    Ok(lock) => {
                        let st = LogState {
                            last_purged_log_id: lock.last_purged_log_id.clone(),
                            last_log,
                        };
                        ack.send(Ok(st)).unwrap();
                    }
                    Err(err) => {
                        ack.send(Err(Error::Generic(err.to_string().into())))
                            .unwrap();
                    }
                };
            }
            Action::Vote(ack) => {
                match meta.read() {
                    Ok(lock) => {
                        ack.send(Ok(lock.vote.clone())).unwrap();
                    }
                    Err(err) => {
                        ack.send(Err(Error::Generic(err.to_string().into())))
                            .unwrap();
                    }
                };
            }
            Action::Shutdown => {
                warn!("Raft logs store reader is being shut down");
                break;
            }
        }
    }

    warn!("Logs Reader exiting");
}
