use crate::error::Error;
use crate::metadata::Metadata;
use crate::writer::TaskData;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::thread;
use tokio::sync::oneshot;
use tracing::{info, warn};

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
    data: TaskData,
    // wal_set: Arc<RwLock<WalFileSet>>,
    // id_until: Arc<AtomicU64>,
    // latest_wal: Arc<AtomicU64>,
) -> Result<flume::Sender<Action>, Error> {
    let (tx, rx) = flume::bounded::<Action>(1);
    thread::spawn(move || run(data, rx));
    Ok(tx)
}

fn run(data: TaskData, rx: flume::Receiver<Action>) {
    // we keep the local set for faster access inside the loop and lazily update if necessary
    let mut wal = data.wal.read().unwrap().clone_no_map();
    let mut buf = Vec::with_capacity(16);

    while let Ok(action) = rx.recv() {
        match action {
            Action::Logs { from, until, ack } => {
                info!("Action::Logs: from: {from}, until: {until}");

                // let latest = latest_wal.load(Ordering::Relaxed);
                if data.latest_log_id.load(Ordering::Relaxed) == 0 {
                    ack.send(None).unwrap();
                    continue;
                }

                // lazily update our `WalFileSet`
                let active = wal.active();
                if data.latest_wal.load(Ordering::Relaxed) != active.wal_no {
                    wal = data.wal.read().unwrap().clone_no_map();
                } else {
                    active.id_until = data.latest_log_id.load(Ordering::Relaxed);
                }

                let mut next = from;
                for log in wal.files.iter_mut() {
                    warn!("log.id_until < next -> {:?} / {}", log, next);
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
                let latest_log_id = data.latest_wal.load(Ordering::Relaxed);
                let last_log = if latest_log_id > 0 {
                    let active = wal.active();
                    let id_until = if latest_log_id != active.wal_no {
                        wal = data.wal.read().unwrap().clone_no_map();
                        wal.active().id_until
                    } else {
                        let until = data.latest_log_id.load(Ordering::Relaxed);
                        active.id_until = until;
                        until
                    };

                    if id_until != 0 {
                        buf.clear();
                        let active = wal.active();
                        active.mmap().unwrap();
                        active.read_logs(id_until, id_until, &mut buf).unwrap();
                        let (_, data) = buf.swap_remove(0);
                        Some(data)
                    } else {
                        None
                    }
                } else {
                    None
                };

                match data.meta.read() {
                    Ok(lock) => {
                        let st = LogState {
                            last_purged_log_id: lock.last_purged_log_id.clone(),
                            last_log,
                        };
                        info!("Sending {:?}", st);
                        ack.send(Ok(st)).unwrap();
                    }
                    Err(err) => {
                        ack.send(Err(Error::Generic(err.to_string().into())))
                            .unwrap();
                    }
                };
            }
            Action::Vote(ack) => {
                match data.meta.read() {
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
