use crate::error::Error;
use crate::metadata::{LockFile, Metadata};
use crate::wal::WalFileSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::Interval;
use tokio::{task, time};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct TaskData {
    pub meta: Arc<RwLock<Metadata>>,
    pub latest_wal: Arc<AtomicU64>,
    pub latest_log_id: Arc<AtomicU64>,
    pub wal: Arc<RwLock<WalFileSet>>,
}

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

#[derive(Debug, PartialEq)]
pub enum LogSync {
    Immediate,
    IntervalMillis(u64),
}

#[allow(clippy::type_complexity)]
pub fn spawn(
    base_path: String,
    sync: LogSync,
    wal_size: u32,
    meta: Arc<RwLock<Metadata>>,
) -> Result<(flume::Sender<Action>, TaskData), Error> {
    // TODO emit a warning log in that case and tell the user how to resolve or "force start" in
    // that case
    LockFile::write(&base_path)?;

    let mut set = WalFileSet::read(base_path, wal_size)?;
    // TODO emit a warning log in that case and tell the user how to resolve or "force start" in
    // that case
    set.check_integrity(meta.clone())?;
    // let mut is_new_instance = false;
    if set.files.is_empty() {
        let mut buf = Vec::with_capacity(28);
        set.add_file(wal_size, &mut buf)?;
        // is_new_instance = true;
    }
    debug!("WalFileSet in Writer: {:?}", set);
    let latest_log_id = Arc::new(AtomicU64::new(set.files.back().unwrap().id_until));
    let latest_wal = Arc::new(AtomicU64::new(set.files.back().unwrap().wal_no));
    let set_locked = Arc::new(RwLock::new(set.clone_no_map()));

    let (tx, rx) = flume::bounded::<Action>(1);
    let sync_immediate = sync == LogSync::Immediate;

    let data = TaskData {
        meta: meta.clone(),
        latest_wal,
        latest_log_id: latest_log_id.clone(),
        wal: set_locked,
    };

    thread::spawn(move || {
        run(
            meta,
            latest_log_id,
            set,
            rx,
            sync_immediate,
            wal_size,
            // is_new_instance,
        )
    });

    if !sync_immediate {
        let LogSync::IntervalMillis(millis) = sync else {
            unreachable!();
        };
        let interval = time::interval(Duration::from_millis(millis));
        spawn_syncer(tx.clone(), interval);
    }

    Ok((tx, data))
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

fn run(
    meta: Arc<RwLock<Metadata>>,
    latest_log_id: Arc<AtomicU64>,
    mut wal: WalFileSet,
    rx: flume::Receiver<Action>,
    sync_immediate: bool,
    wal_size: u32,
    // mut is_new_instance: bool,
) -> Result<(), Error> {
    let mut is_dirty = false;
    let mut shutdown_ack: Option<oneshot::Sender<()>> = None;

    let mut buf: Vec<u8> = Vec::with_capacity(8);
    let mut buf_logs: Vec<(u64, Vec<u8>)> = Vec::with_capacity(1);

    wal.active().mmap_mut()?;

    while let Ok(action) = rx.recv() {
        match action {
            Action::Append { rx, callback, ack } => {
                info!("Action::Append");

                let mut res = Ok(());
                let mut last_id = 0;
                {
                    let mut active = wal.active();
                    while let Ok(Some((id, data))) = rx.recv() {
                        // we get here during late cluster joins or WAL rebuilds from remote nodes
                        // if active.wal_no == 1 && active.id_from == 0 {
                        //     active.id_from = id;
                        // }

                        if data.len() > u32::MAX as usize {
                            res = Err(Error::Generic("`data` length must not exceed u32".into()));
                            break;
                        }

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
                        last_id = id;
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
                    callback();
                }

                buf.clear();
                wal.active().update_header(&mut buf)?;
                // meta.write()?.log_until = last_id;
                latest_log_id.store(last_id, Ordering::Relaxed);

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

                let active = wal.active();
                let until = if until > active.id_until {
                    active.id_until
                } else {
                    until
                };

                match wal.shift_delete_logs_until(until, wal_size, &mut buf, &mut buf_logs) {
                    Ok(_) => {
                        {
                            let mut lock = meta.write()?;
                            // lock.log_from = until;
                            lock.last_purged_log_id = last_log;
                        }
                        // Metadata::write(meta.clone(), &wal.base_path)?;
                        ack.send(Ok(())).unwrap();
                    }
                    // Err(err) => ack.send(Err(err)).unwrap(),
                    Err(err) => {
                        panic!("shift delete logs err: {:?}", err);
                        ack.send(Err(err)).unwrap()
                    }
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
                    buf.clear();
                    active.update_header(&mut buf)?;
                    active.flush_async()?;

                    // meta.write()?.log_until = id_until.load(Ordering::Relaxed);
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

    warn!("Logs Writer exiting");

    let active = wal.active();
    buf.clear();
    active.update_header(&mut buf)?;
    active.flush_async()?;
    Metadata::write(meta.clone(), &wal.base_path)?;
    LockFile::remove(&wal.base_path).expect("LockFile removal failed");

    if let Some(ack) = shutdown_ack {
        ack.send(())
            .expect("Shutdown handler to always wait for ack from logs");
    }

    Ok(())
}
