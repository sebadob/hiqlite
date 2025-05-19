use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::{fs, thread};
use tokio::sync::oneshot;
use tracing::{error, warn};

pub enum Action {
    Append {
        rx: flume::Receiver<Option<(u64, Vec<u8>)>>,
        callback: Box<dyn FnOnce() + Send>,
        ack: oneshot::Sender<Result<(), Error>>,
        // ack: oneshot::Sender<Result<(), StorageIOError<u64>>>,
    },
    Remove {
        from: u64,
        until: u64,
        last_log: Option<Vec<u8>>,
        ack: oneshot::Sender<Result<(), Error>>,
        // ack: oneshot::Sender<Result<(), StorageError<u64>>>,
    },
    Vote {
        value: Vec<u8>,
        ack: oneshot::Sender<Result<(), Error>>,
        // ack: oneshot::Sender<Result<(), StorageIOError<u64>>>,
    },
    Sync,
    Shutdown(oneshot::Sender<()>),
}

#[inline]
fn create_base_path(base_path: &str) -> Result<(), Error> {
    fs::create_dir_all(base_path)?;
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(base_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(base_path, perms)?;
    }
    Ok(())
}

pub fn spawn(
    base_path: String,
    sync_immediate: bool,
    wal_size: u32,
    meta: Arc<RwLock<Metadata>>,
    id_until: AtomicU64,
) -> Result<flume::Sender<Action>, Error> {
    create_base_path(&base_path)?;

    let mut set = WalFileSet::read(base_path, wal_size)?;
    // TODO emit a warning log in that case and tell the user how to resolve or "force start" in
    // that case
    set.check_integrity(meta.clone())?;
    if set.files.is_empty() {
        let mut buf = Vec::with_capacity(28);
        set.add_file(wal_size, &mut buf)?;
    }

    let (tx, rx) = flume::bounded::<Action>(1);
    thread::spawn(move || run(meta, id_until, set, rx, sync_immediate, wal_size));
    Ok(tx)
}

fn run(
    meta: Arc<RwLock<Metadata>>,
    id_until: AtomicU64,
    mut wal: WalFileSet,
    rx: flume::Receiver<Action>,
    sync_immediate: bool,
    wal_size: u32,
) -> Result<(), Error> {
    let mut is_dirty = false;
    let mut shutdown_ack: Option<oneshot::Sender<()>> = None;

    let mut buf: Vec<u8> = Vec::with_capacity(8);
    let mut buf_logs: Vec<(u64, Vec<u8>)> = Vec::with_capacity(1);

    while let Ok(action) = rx.recv() {
        match action {
            Action::Append { rx, callback, ack } => {
                let mut res = Ok(());
                let mut last_id = 0;
                {
                    let mut active = wal.active();
                    while let Ok(Some((id, data))) = rx.recv() {
                        if data.len() > u32::MAX as usize {
                            res = Err(Error::Generic("`data` length must not exceed u32".into()));
                            break;
                        }
                        last_id = id;

                        if !active.has_space(data.len()) {
                            buf.clear();
                            wal.roll_over(wal_size, &mut buf)?;
                            active = wal.active();
                        }
                        // TODO should we handle errors or better panic in case of a storage error?
                        // Crashing is probably the way we want, because you would probably not be able
                        // to recover from general storage errors.
                        //
                        // It could also have a feature like `panic-on-storage-err` to make it optional.
                        buf.clear();
                        if let Err(err) = active.append_log(id, &data, &mut buf) {
                            res = Err(err);
                            break;
                        }
                    }
                }
                is_dirty = true;

                let is_ok = res.is_ok();
                if let Err(err) = ack.send(res) {
                    // this should usually not happen, but it may during an incorrect shutdown
                    error!("error sending back ack after logs append: {:?}", err);
                }
                if is_ok {
                    // TODO with the next big openraft release, we can do async callbacks
                    if sync_immediate {
                        wal.active().flush()?;
                        is_dirty = false;
                    }
                    id_until.store(last_id, Ordering::Relaxed);
                    meta.write()?.log_until = last_id;
                    callback();
                }

                // Roll WAL pre-emptively if only very few space is left at this point, because
                // if we just wrote some chunks, me probably have a very short break now until the
                // next request comes in.
                //
                // TODO fixed 16kB -> make configurable
                if wal.active().space_left() < 16 * 1024 {
                    buf.clear();
                    wal.roll_over(wal_size, &mut buf)?;
                }
            }
            Action::Remove {
                from: _,
                until,
                last_log,
                ack,
            } => {
                // We don't care about the `from` part, since we don't really delete anything yet.
                // We only want to shift the index inside the WALs and only delete complete WAL
                // files, if their last log ID is < `until`. Wasting a bit of disk space is far
                // better than doing real deletions and shift data all the time.
                // In the worst case, there is the size of 1 WAL "wasted", because maybe only the
                // very last log from this WAL should be kept.
                buf.clear();
                buf_logs.clear();
                match wal.shift_delete_logs_until(until, wal_size, &mut buf, &mut buf_logs) {
                    Ok(_) => {
                        {
                            let mut lock = meta.write()?;
                            lock.log_from = until;
                            lock.last_purged = last_log;
                        }
                        ack.send(Ok(())).unwrap();
                    }
                    Err(err) => ack.send(Err(err)).unwrap(),
                }
            }
            Action::Vote { value, ack } => {
                meta.write()?.vote = Some(value);
                let res = Metadata::write(meta.clone(), &wal.base_path);
                ack.send(res).unwrap();
            }
            Action::Sync => {
                if is_dirty {
                    let active = wal.active();
                    active.update_header(&mut buf)?;
                    active.flush_async()?;

                    meta.write()?.log_until = id_until.load(Ordering::Relaxed);
                    Metadata::write(meta.clone(), &wal.base_path)?;

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

    // TODO flush and close all files
    warn!("Logs Writer exiting");

    if let Some(ack) = shutdown_ack {
        ack.send(())
            .expect("Shutdown handler to always wait for ack from logs");
    }

    Ok(())
}
