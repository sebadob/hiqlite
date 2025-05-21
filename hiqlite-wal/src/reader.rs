use crate::error::Error;
use crate::writer::TaskData;
use std::sync::atomic::Ordering;
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

pub fn spawn(data: TaskData) -> Result<flume::Sender<Action>, Error> {
    let (tx, rx) = flume::bounded::<Action>(1);
    thread::spawn(move || run(data, rx));
    Ok(tx)
}

fn run(data: TaskData, rx: flume::Receiver<Action>) {
    // we keep the local set for faster access inside the loop and lazily update if necessary
    let mut wal = data.wal.read().unwrap().clone_no_map();
    // TODO a good value would be max payload chunks
    let mut buf = Vec::with_capacity(16);

    while let Ok(action) = rx.recv() {
        match action {
            Action::Logs { from, until, ack } => {
                // println!("Action::Logs: from: {from}, until: {until}");

                // if data.latest_log_id.load(Ordering::Relaxed) == 0 {
                //     ack.send(None).unwrap();
                //     continue;
                // }

                // lazily update our `WalFileSet`
                let last = wal.active();
                // let last = wal.files.get_mut(wal.files.len() - 1).unwrap();
                if data.latest_wal.load(Ordering::Relaxed) != last.wal_no {
                    // println!(">>> Found difference in WAL No - Updating");
                    let wal_upd = data.wal.read().unwrap();
                    wal.active = wal_upd.active;

                    for file_upd in &wal_upd.files {
                        if let Some(file) =
                            wal.files.iter_mut().find(|f| f.wal_no == file_upd.wal_no)
                        {
                            file.clone_from_no_mmap(file_upd);
                        } else {
                            wal.files.push_back(file_upd.clone_no_mmap());
                        }
                    }
                    // println!("\nReader WALs after update:\n{:?}\n", wal);
                } else {
                    last.id_until = data.latest_log_id.load(Ordering::Relaxed);
                }
                // debug_assert!(active.id_until <= until);

                let mut from_next = from;
                for log in wal.files.iter_mut() {
                    // println!("log.id_until < next -> {:?} / {}", log, next);
                    if log.id_until < from_next {
                        continue;
                    }

                    // if mmap fails, there is probably no way to recover anyway
                    log.mmap().unwrap();
                    // if log.is_empty().unwrap() {
                    //     continue;
                    // }
                    buf.clear();

                    if log.id_until < until {
                        // log reading should not fail as well, no way to recover from mmap issues
                        log.read_logs(from_next, log.id_until, &mut buf).unwrap();
                        for (_id, data) in buf.drain(..) {
                            debug_assert!(_id >= from_next && _id <= until);
                            ack.send(Some(Ok(data))).unwrap()
                        }
                        // #[cfg(not(debug_assertions))]
                        // for (_, data) in buf.drain(..) {
                        //     ack.send(Some(Ok(data))).unwrap()
                        // }

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
