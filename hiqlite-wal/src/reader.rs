use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use tokio::sync::oneshot;
use tracing::{debug, error};

/// Sentinel for the upper bound of an open-ended range in `Action::Logs`; the reader answers such
/// a read with everything it holds, like openraft's reference stores do.
pub(crate) const NO_END: u64 = u64::MAX;

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
    // the `id_from` of the WAL file this memo belongs to - together with `last_wal_no` it
    // uniquely identifies the file, because `wal_no` values wrap around after all files are
    // purged and re-created
    pub id_from: u64,
    pub last_log_id: u64,
    pub data_end: u32,
}

#[derive(Debug)]
struct ReaderWorker {
    sender: flume::WeakSender<Action>,
    closed: flume::Receiver<()>,
}

/// Tracks the main reader and all independent log readers without keeping idle readers alive.
#[derive(Debug)]
pub(crate) struct Readers(Mutex<Option<Vec<ReaderWorker>>>);

impl Readers {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(Some(Vec::new())))
    }

    pub(crate) fn spawn(
        &self,
        meta: Arc<RwLock<Metadata>>,
        wal: Arc<RwLock<WalFileSet>>,
    ) -> (flume::Sender<Action>, flume::Receiver<()>) {
        let mut registry = self.0.lock().unwrap();
        let Some(workers) = registry.as_mut() else {
            return (flume::bounded(1).0, flume::bounded(0).1);
        };
        workers.retain(|worker| !worker.closed.is_disconnected());
        let (sender, closed) = spawn(meta, wal);
        workers.push(ReaderWorker {
            sender: sender.downgrade(),
            closed: closed.clone(),
        });
        (sender, closed)
    }

    pub(crate) async fn shutdown(&self) {
        let workers = self.0.lock().unwrap().take().unwrap_or_default();
        for worker in workers {
            if let Some(sender) = worker.sender.upgrade() {
                let _ = sender.send_async(Action::Shutdown).await;
            }
            let _ = worker.closed.recv_async().await;
        }
    }
}

fn spawn(
    meta: Arc<RwLock<Metadata>>,
    wal_locked: Arc<RwLock<WalFileSet>>,
) -> (flume::Sender<Action>, flume::Receiver<()>) {
    let (tx, rx) = flume::bounded::<Action>(1);
    let (done, closed) = flume::bounded::<()>(0);
    thread::spawn(move || {
        let _done = done;
        let receiver = rx;
        run(&meta, &wal_locked, &receiver);
    });
    (tx, closed)
}

/// There are a lot of `unwrap()`s in this task. The reason is simply, if most of these fail, it can
/// only be because of a non-recoverable error anyway and the application should crash, so that
/// the next health check can restart it.
///
/// Everything related to locking and memory mapping is being `unwrap()`ped. If anything fails in
/// this regard, it's either a physical storage or OS issue and this code an do nothing about it.
fn run(
    meta: &Arc<RwLock<Metadata>>,
    wal_locked: &Arc<RwLock<WalFileSet>>,
    rx: &flume::Receiver<Action>,
) {
    // we keep the local set for faster access inside the loop and lazily update if necessary
    let mut wal = wal_locked.read().unwrap().clone_no_map();
    // openraft will read chunks of 64 logs for bigger tasks
    let mut buf = Vec::with_capacity(64);
    let mut memo: Option<LogReadMemo> = None;

    while let Ok(action) = rx.recv() {
        match action {
            Action::Logs {
                from,
                mut until,
                ack,
            } => {
                debug!("WAL Reader - Action::Logs - read from {from} until {until}");
                {
                    let wal_upd = wal_locked.read().unwrap();
                    wal.active = wal_upd.active;
                    wal.clone_files_from_no_mmap(&wal_upd.files);
                }

                if until == NO_END {
                    // an open-ended request is bounded by the end of what we actually hold
                    until = wal.files.back().map_or(until, |f| f.id_until);
                }

                let mut from_next = from;
                'logs: for log in &mut wal.files {
                    if log.id_until < from_next {
                        debug!(
                            "log.id_until < from_next -> {} < {}",
                            log.id_until, from_next
                        );
                        continue;
                    }

                    log.mmap().unwrap();
                    buf.clear();

                    if log.id_until < until {
                        // this file is entirely before `until` - read the whole overlap
                        debug!("log.id_until < until -> {} < {}", log.id_until, until);
                        match log.read_logs(from_next, log.id_until, &mut memo, &mut buf) {
                            Ok(_) => {
                                for (_id, data) in buf.drain(..) {
                                    debug_assert!(_id >= from_next && _id <= until);
                                    if ack.send(Some(Ok(data))).is_err() {
                                        break 'logs;
                                    }
                                }

                                // If the until goes beyond our current file, we want to remove the `mmap`
                                // to save memory. Only if the system needs a log snapshot to recover
                                // another node, it may need lower log IDs again.
                                log.mmap_drop();
                            }
                            Err(err) => {
                                error!("Error reading logs: {:?}", err);
                                let _ = ack.send(Some(Err(err)));
                                break 'logs;
                            }
                        }

                        from_next = log.id_until + 1;
                    } else {
                        // this file contains the end of the read request
                        debug!("log contains end of read request");

                        match log.read_logs(from_next, until, &mut memo, &mut buf) {
                            Ok(_) => {
                                for (_, data) in buf.drain(..) {
                                    if ack.send(Some(Ok(data))).is_err() {
                                        break 'logs;
                                    }
                                }
                            }
                            Err(err) => {
                                error!("Error reading logs: {:?}", err);
                                let _ = ack.send(Some(Err(err)));
                            }
                        }
                        break;
                    }
                }

                let _ = ack.send(None);
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

                let last_purged = meta.read().unwrap().last_purged_log_id.clone();
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
                    // no entries left in the store; openraft's convention is that the last log id
                    // is the purge frontier, and its startup check requires as much
                    last_purged.clone()
                };

                let st = LogState {
                    last_purged_log_id: last_purged,
                    last_log,
                };
                debug!(
                    "WAL Reader - Action::LogState -> latest_log_id: {:?}\n{:?}",
                    latest_log_id, st
                );
                let _ = ack.send(Ok(st));
            }
            Action::Vote(ack) => {
                debug!("WAL Reader - Action::Vote");
                let vote = meta.read().unwrap().vote.clone();
                let _ = ack.send(Ok(vote));
            }
            Action::Shutdown => {
                debug!("Raft logs store reader is being shut down");
                break;
            }
        }
    }

    debug!("Logs Reader exiting");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wal::WalFile;
    use std::collections::VecDeque;
    use std::fs;
    use std::io::{Seek, SeekFrom, Write};

    static PATH: &str = "test_data";
    static MB2: u32 = 2 * 1024 * 1024;

    #[test]
    fn logs_action_reports_read_errors() -> Result<(), Error> {
        // Regression test: a read error while serving `Action::Logs` used to be swallowed (or,
        // for files entirely before `until`, even panicked the reader thread), so callers got a
        // silent short read instead of an error.
        let base_path = format!("{PATH}/reader_logs_error");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(32);
        let mut wal = WalFile::new(1, &base_path, 0, 0, MB2).unwrap();
        wal.create_file(&mut buf)?;
        wal.mmap_mut()?;
        for id in 1..=3 {
            buf.clear();
            wal.append_log(id, b"payload", &mut buf)?;
        }
        let path = wal.path.clone();
        // a record is 8 id + 4 crc + 4 len + payload, plus one byte of gap to the next one
        let stride = 8 + 4 + 4 + b"payload".len() + 1;
        let corrupt_at = wal.offset_logs() + 2 * stride + 8 + 4 + 4;
        drop(wal);

        // corrupt the first data byte of log 3
        {
            let mut file = fs::OpenOptions::new().write(true).open(&path)?;
            file.seek(SeekFrom::Start(corrupt_at as u64))?;
            file.write_all(b"X")?;
        }

        let set = WalFileSet {
            active: Some(0),
            base_path,
            files: VecDeque::from([WalFile::read_from_file(path)?]),
        };
        let meta = Arc::new(RwLock::new(Metadata {
            last_purged_log_id: None,
            vote: None,
        }));

        let (tx, _) = spawn(meta, Arc::new(RwLock::new(set)));
        let (ack, rx) = flume::bounded(2);
        tx.send(Action::Logs {
            from: 2,
            until: 3,
            ack,
        })
        .expect("reader to always be listening");

        let mut got_err = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                Some(Ok(_)) => panic!("unexpected data"),
                Some(Err(err)) => {
                    assert!(matches!(&err, Error::Integrity(_)));
                    got_err = true;
                }
                None => break,
            }
        }
        assert!(got_err);

        Ok(())
    }
}
