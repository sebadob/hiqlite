use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
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
    wal_locked: Arc<RwLock<WalFileSet>>,
) -> Result<flume::Sender<Action>, Error> {
    let (tx, rx) = flume::bounded::<Action>(1);
    thread::spawn(move || run(meta, wal_locked, rx));
    Ok(tx)
}

fn run(
    meta: Arc<RwLock<Metadata>>,
    wal_locked: Arc<RwLock<WalFileSet>>,
    rx: flume::Receiver<Action>,
) {
    // we keep the local set for faster access inside the loop and lazily update if necessary
    let mut wal = wal_locked.read().unwrap().clone_no_map();
    // TODO a good value would be max payload chunks
    let mut buf = Vec::with_capacity(16);

    while let Ok(action) = rx.recv() {
        match action {
            Action::Logs { from, until, ack } => {
                {
                    let wal_upd = wal_locked.read().unwrap();
                    wal.active = wal_upd.active;
                    wal.clone_files_from_no_mmap(&wal_upd.files);
                }

                let mut from_next = from;
                for log in wal.files.iter_mut() {
                    if log.id_until < from_next {
                        continue;
                    }

                    // if mmap fails, there is probably no way to recover anyway
                    log.mmap().unwrap();
                    buf.clear();

                    if log.id_until < until {
                        // log reading should not fail as well, no way to recover from mmap issues
                        log.read_logs(from_next, log.id_until, &mut buf).unwrap();
                        for (_id, data) in buf.drain(..) {
                            debug_assert!(_id >= from_next && _id <= until);
                            ack.send(Some(Ok(data))).unwrap()
                        }

                        // If the until goes beyond our current file, we want to remove the mmap
                        // to save memory. Only if the system needs a log snapshot to recover
                        // another node, it may need lower log IDs again.
                        log.mmap_drop();

                        from_next = log.id_until + 1;
                    } else {
                        // log reading should not fail as well, no way to recover from mmap issues
                        log.read_logs(from_next, until, &mut buf).unwrap();
                        for (_, data) in buf.drain(..) {
                            ack.send(Some(Ok(data))).unwrap()
                        }
                        break;
                    };

                    if log.id_from <= from && log.id_until >= from {}
                }

                ack.send(None).unwrap();
            }
            Action::LogState(ack) => {
                {
                    let wal_upd = wal_locked.read().unwrap();
                    wal.active = wal_upd.active;
                    wal.clone_files_from_no_mmap(&wal_upd.files);
                }

                let latest_log_id = {
                    let file = &wal.files[wal.files.len() - 1];
                    if file.data_start.is_some() {
                        Some(file.id_until)
                    } else if wal.files.len() > 1 {
                        // In this case we might just be at the edge of a log roll-over
                        Some(wal.files[wal.files.len() - 2].id_until)
                    } else {
                        None
                    }
                };

                let last_log = if let Some(latest_log_id) = latest_log_id {
                    buf.clear();
                    let active = wal.active();
                    active.mmap().unwrap();
                    active
                        .read_logs(latest_log_id, latest_log_id, &mut buf)
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
