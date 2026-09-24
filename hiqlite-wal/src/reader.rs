use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use std::sync::{Arc, RwLock};
use std::thread;
use tokio::sync::oneshot;
use tracing::{debug, error};

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
    // openraft will read chunks of 64 logs for bigger tasks
    let mut buf = Vec::with_capacity(64);
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
                'logs: for log in wal.files.iter_mut() {
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
                                    ack.send(Some(Ok(data))).unwrap()
                                }

                                // If the until goes beyond our current file, we want to remove the `mmap`
                                // to save memory. Only if the system needs a log snapshot to recover
                                // another node, it may need lower log IDs again.
                                log.mmap_drop();
                            }
                            Err(err) => {
                                error!("Error reading logs: {:?}", err);
                                ack.send(Some(Err(err))).unwrap();
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
                                    ack.send(Some(Ok(data))).unwrap();
                                }
                            }
                            Err(err) => {
                                error!("Error reading logs: {:?}", err);
                                ack.send(Some(Err(err))).unwrap();
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
                debug!(
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
        let base_path = format!("{}/reader_logs_error", PATH);
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
        drop(wal);

        // corrupt the first data byte of log 3 (record starts at offset 80, header is 16 bytes)
        {
            let mut file = fs::OpenOptions::new().write(true).open(&path)?;
            file.seek(SeekFrom::Start(96))?;
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

        let tx = spawn(meta, Arc::new(RwLock::new(set)))?;
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
