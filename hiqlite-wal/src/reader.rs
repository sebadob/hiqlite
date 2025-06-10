use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use std::sync::{Arc, RwLock};
use std::thread;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

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

/// Memorizes the last read log to speed up future lookups and have a saved starting position.
/// Logs are always read sequential, apart from during app start, when once Logs will be read
/// backwards to find the latest membership config.
/// This saves us from maintaining a complete index in memory, which is not necessary at all.
/// Each reader usually reads each log max once, followed by the next one guaranteed in sequential
/// order. This means (apart from the very first start), this memoized position will always be used.
#[derive(Debug)]
pub struct LogReadMemo {
    pub last_wal_no: u64,
    pub last_log_id: u64,
    pub data_end: u32,
}

pub fn spawn(
    meta: Arc<RwLock<Metadata>>,
    wal_locked: Arc<RwLock<WalFileSet>>,
) -> Result<flume::Sender<Action>, Error> {
    let (tx, rx) = flume::bounded::<Action>(1);
    thread::spawn(move || run(meta, wal_locked, rx));
    Ok(tx)
}

/// There are a lot of `unwrap()`s in this task. The reason is simply, if most of these fail, it can
/// only be because of a non-recoverable error anyway and the application should crash, so that
/// the next health check can restart it.
///
/// Everything related to locking and memory mapping is being `unwrap()`ped. If anything fails in
/// this regard, it's either a physical storage or OS issue and this code an do nothing about it.
fn run(
    meta: Arc<RwLock<Metadata>>,
    wal_locked: Arc<RwLock<WalFileSet>>,
    rx: flume::Receiver<Action>,
) {
    // we keep the local set for faster access inside the loop and lazily update if necessary
    let mut wal = wal_locked.read().unwrap().clone_no_map();
    // TODO a good value would be max payload chunks
    let mut buf = Vec::with_capacity(16);

    let mut memo: Option<LogReadMemo> = None;

    while let Ok(action) = rx.recv() {
        match action {
            Action::Logs { from, until, ack } => {
                debug!("WAL Reader - Action::Logs - read from {from} until {until}");
                {
                    let wal_upd = wal_locked.read().unwrap();
                    wal.active = wal_upd.active;
                    wal.clone_files_from_no_mmap(&wal_upd.files);
                }

                let mut from_next = from;
                for log in wal.files.iter_mut() {
                    if log.id_until < from_next {
                        debug!(
                            "log.id_until < from_next -> {} < {}",
                            log.id_until, from_next
                        );
                        continue;
                    }
                    debug!("Reading from Log {:?}", log);

                    log.mmap().unwrap();
                    buf.clear();

                    if log.id_until < until {
                        debug!("log.id_until < until -> {} < {}", log.id_until, until);
                        log.read_logs(from_next, log.id_until, &mut memo, &mut buf)
                            .unwrap();

                        for (_id, data) in buf.drain(..) {
                            debug_assert!(_id >= from_next && _id <= until);
                            ack.send(Some(Ok(data))).unwrap()
                        }

                        // If the until goes beyond our current file, we want to remove the `mmap`
                        // to save memory. Only if the system needs a log snapshot to recover
                        // another node, it may need lower log IDs again.
                        log.mmap_drop();

                        from_next = log.id_until + 1;
                    } else {
                        debug!("log contains end of read request");

                        match log.read_logs(from_next, until, &mut memo, &mut buf) {
                            Ok(_) => {
                                for (_, data) in buf.drain(..) {
                                    ack.send(Some(Ok(data))).unwrap()
                                }
                            }
                            Err(err) => {
                                error!("Error reading logs: {:?}", err);
                            }
                        }
                        break;
                    };
                }

                ack.send(None).unwrap();
            }
            Action::LogState(ack) => {
                debug!("WAL Reader - Action::LogState");
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
                        // In this case we might just be in the middle of a log roll-over
                        Some(wal.files[wal.files.len() - 2].id_until)
                    } else {
                        None
                    }
                };

                let last_log = if let Some(latest_log_id) = latest_log_id {
                    buf.clear();
                    let active = wal.active();
                    if active.data_start.is_some() {
                        active.mmap().unwrap();
                        active
                            .read_logs(latest_log_id, latest_log_id, &mut memo, &mut buf)
                            .unwrap();
                    } else if wal.files.len() > 1 {
                        // this is an edge case, when we shut down beforehand exactly after rolling
                        // over WAL files but without adding anything to the new file
                        let file = wal.files.get_mut(wal.files.len() - 2).unwrap();
                        file.mmap().unwrap();
                        file.read_logs(latest_log_id, latest_log_id, &mut memo, &mut buf)
                            .unwrap();
                        file.mmap_drop();
                    }
                    let (_, data) = buf.swap_remove(0);
                    Some(data)
                } else {
                    None
                };

                let st = LogState {
                    last_purged_log_id: meta.read().unwrap().last_purged_log_id.clone(),
                    last_log,
                };
                info!(
                    "WAL Reader - Action::LogState -> latest_log_id: {:?}\n{:?}",
                    latest_log_id, st
                );
                ack.send(Ok(st)).unwrap();
            }
            Action::Vote(ack) => {
                debug!("WAL Reader - Action::Vote");
                let vote = meta.read().unwrap().vote.clone();
                ack.send(Ok(vote)).unwrap();
            }
            Action::Shutdown => {
                warn!("Raft logs store reader is being shut down");
                break;
            }
        }
    }

    warn!("Logs Reader exiting");
}
