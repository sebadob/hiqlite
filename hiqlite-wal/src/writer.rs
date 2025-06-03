use crate::error::Error;
use crate::metadata::{LockFile, Metadata};
use crate::wal::WalFileSet;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::Interval;
use tokio::{task, time};
use tracing::{debug, error, warn};

pub enum Action {
    Append {
        rx: flume::Receiver<Option<(u64, Vec<u8>)>>,
        callback: Box<dyn FnOnce() + Send>,
        ack: oneshot::Sender<Result<(), Error>>,
    },
    Remove {
        from: u64,
        until: u64,
        last_log: Option<Vec<u8>>,
        ack: oneshot::Sender<Result<(), Error>>,
    },
    Vote {
        value: Vec<u8>,
        ack: oneshot::Sender<Result<(), Error>>,
    },
    Sync,
    Shutdown(oneshot::Sender<()>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogSync {
    Immediate,
    ImmediateAsync,
    IntervalMillis(u64),
}

#[allow(clippy::type_complexity)]
pub fn spawn(
    base_path: String,
    sync: LogSync,
    wal_size: u32,
    meta: Arc<RwLock<Metadata>>,
) -> Result<(flume::Sender<Action>, Arc<RwLock<WalFileSet>>), Error> {
    let lock_exists = LockFile::exists(&base_path)?;
    if lock_exists {
        warn!("LockFile in {base_path} exists already - this is not a clean start!");
    }
    LockFile::write(&base_path)?;

    let mut set = WalFileSet::read(base_path, wal_size)?;
    // TODO emit a warning log in that case and tell the user how to resolve or "force start" in
    // that case, or should be maybe `auto-heal` as much as possible?
    let mut buf = Vec::with_capacity(32);
    set.check_integrity(&mut buf, lock_exists)?;
    if set.files.is_empty() {
        buf.clear();
        set.add_file(wal_size, &mut buf)?;
    }
    let wal_locked = Arc::new(RwLock::new(set.clone_no_map()));

    let (tx, rx) = flume::bounded::<Action>(1);

    let wal = wal_locked.clone();
    let snc = sync.clone();
    thread::spawn(move || run(meta, wal, set, rx, snc, wal_size));

    if let LogSync::IntervalMillis(millis) = &sync {
        let interval = time::interval(Duration::from_millis(*millis));
        spawn_syncer(tx.clone(), interval);
    }

    Ok((tx, wal_locked))
}

fn spawn_syncer(tx_writer: flume::Sender<Action>, mut interval: Interval) {
    task::spawn(async move {
        loop {
            interval.tick().await;
            if tx_writer.send_async(Action::Sync).await.is_err() {
                debug!("Error sending ActionWrite::Sync to LogStoreWriter - exiting");
                break;
            }
        }
    });
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
    mut wal: WalFileSet,
    rx: flume::Receiver<Action>,
    sync: LogSync,
    wal_size: u32,
) -> Result<(), Error> {
    let mut is_dirty = false;
    let mut shutdown_ack: Option<oneshot::Sender<()>> = None;
    let data_len_limit = wal_size as usize - wal.active().offset_logs() - 2;

    let mut buf: Vec<u8> = Vec::with_capacity(8);
    let mut buf_logs: Vec<(u64, Vec<u8>)> = Vec::with_capacity(1);

    wal.active().mmap_mut()?;

    while let Ok(action) = rx.recv() {
        match action {
            Action::Append { rx, callback, ack } => {
                debug!("WAL Writer - Action::Append");

                let mut res = Ok(());
                {
                    let mut active = wal.active();
                    while let Ok(Some((id, bytes))) = rx.recv() {
                        if bytes.len() > data_len_limit {
                            panic!("`data` length must not exceed `wal_size` -> data length is {} vs wal_size (without header) is {}", bytes.len(), data_len_limit);
                        }

                        if !active.has_space(bytes.len() as u32) {
                            buf.clear();
                            wal.roll_over(wal_size, &mut buf)?;
                            {
                                let mut lock = wal_locked.write().unwrap();
                                lock.active = wal.active;
                                lock.clone_files_from_no_mmap(&wal.files);
                            }
                            active = wal.active();
                        }

                        buf.clear();
                        if let Err(err) = active.append_log(id, &bytes, &mut buf) {
                            res = Err(err);
                            break;
                        }
                        debug_assert_eq!(
                            active.id_until, id,
                            "active.id_until and id don't match: {} != {}",
                            active.id_until, id
                        );
                    }
                }

                {
                    let mut lock = wal_locked.write().unwrap();
                    debug_assert_eq!(lock.active, wal.active);
                    lock.active().clone_from_no_mmap(wal.active());
                }

                if let Err(err) = ack.send(res) {
                    // this should usually not happen, but it may during an incorrect shutdown
                    error!("error sending back ack after logs append: {:?}", err);
                }

                if sync == LogSync::Immediate {
                    wal.active().flush()?;
                } else if sync == LogSync::ImmediateAsync {
                    wal.active().flush_async()?;
                } else {
                    is_dirty = true;
                }
                // TODO with the next big openraft release, we can do async callbacks
                callback();

                // Roll WAL pre-emptively if only very few space is left at this point, because
                // if we just wrote some chunks, me probably have a very short break now until the
                // next request comes in.
                //
                // TODO fixed 4kB -> make configurable?
                if wal.active().space_left() < 4 * 1024 {
                    buf.clear();
                    wal.roll_over(wal_size, &mut buf)?;
                    {
                        let mut lock = wal_locked.write().unwrap();
                        lock.active = wal.active;
                        lock.clone_files_from_no_mmap(&wal.files);
                    }
                }
            }
            Action::Remove {
                from,
                until,
                last_log,
                ack,
            } => {
                debug!(
                    "WAL Writer - Action::Remove from {from} until {until} / last_log: {:?}\n{:?}",
                    last_log, wal
                );

                // We don't care about the `from` part, since we don't really delete anything yet.
                // We only want to shift the index inside the WALs and only delete complete WAL
                // files, if their last log ID is < `until`. Wasting a bit of disk space is far
                // better than doing real deletions and shift data all the time.
                // In the worst case, there is the size of 1 WAL "wasted", because maybe only the
                // very last log from this WAL should be kept.
                buf.clear();
                buf_logs.clear();

                let active = wal.active();
                let until = if until > active.id_until {
                    active.id_until
                } else {
                    until
                };

                match wal.shift_delete_logs_until(until, wal_size, &mut buf, &mut buf_logs) {
                    Ok(_) => {
                        meta.write()?.last_purged_log_id = last_log;
                        Metadata::write(meta.clone(), &wal.base_path)?;
                        {
                            let mut lock = wal_locked.write().unwrap();
                            lock.active = wal.active;
                            lock.clone_files_from_no_mmap(&wal.files);
                        }
                        ack.send(Ok(())).unwrap();
                    }
                    Err(err) => ack.send(Err(err)).unwrap(),
                }
            }
            Action::Vote { value, ack } => {
                debug!("WAL Writer - Action::Vote");

                meta.write()?.vote = Some(value);
                let res = Metadata::write(meta.clone(), &wal.base_path);
                ack.send(res).unwrap();
            }
            Action::Sync => {
                if is_dirty {
                    let active = wal.active();
                    buf.clear();
                    active.update_header(&mut buf)?;
                    active.flush_async()?;
                    is_dirty = false;
                }
            }
            Action::Shutdown(ack) => {
                warn!("Raft logs store writer is being shut down");
                shutdown_ack = Some(ack);
                break;
            }
        }
    }

    warn!("Logs Writer exiting");

    let active = wal.active();
    buf.clear();
    active.update_header(&mut buf)?;
    active.flush()?;
    Metadata::write(meta, &wal.base_path)?;
    LockFile::remove(&wal.base_path).expect("LockFile removal failed");

    if let Some(ack) = shutdown_ack {
        ack.send(())
            .expect("Shutdown handler to always wait for ack from logs");
    }

    Ok(())
}
