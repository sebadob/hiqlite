use crate::error::Error;
use crate::lockfile::LockFile;
use crate::log_store_impl::{deserialize, serialize};
use crate::metadata::Metadata;
use crate::reader::LogReadMemo;
use crate::wal::WalFileSet;
use openraft::{LeaderId, LogId};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use thread_priority::ThreadPriority;
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

impl Debug for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Append { .. } => write!(f, "Action::Append"),
            Action::Remove { .. } => write!(f, "Action::Remove"),
            Action::Vote { .. } => write!(f, "Action::Vote"),
            Action::Sync => write!(f, "Action::Sync"),
            Action::Shutdown(_) => write!(f, "Action::Shutdown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogSync {
    Immediate,
    ImmediateAsync,
    IntervalMillis(u64),
}

impl TryFrom<&str> for LogSync {
    type Error = Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "immediate" => Ok(Self::Immediate),
            "immediate_async" => Ok(Self::ImmediateAsync),
            v => {
                if let Some(ms) = v.strip_prefix("interval_") {
                    let Ok(ms) = ms.parse::<u64>() else {
                        return Err(Error::Generic(
                            format!(
                                "Invalid value for log_sync interval, cannot parse as u64: {v}"
                            )
                            .into(),
                        ));
                    };
                    Ok(Self::IntervalMillis(ms))
                } else {
                    Err(Error::Generic(
                        format!("Cannot parse LogSync - invalid value: {v}").into(),
                    ))
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn spawn(
    base_path: String,
    lockfile: LockFile,
    sync: LogSync,
    wal_size: u32,
    wal_deep_integrity_check: bool,
    meta: Arc<RwLock<Metadata>>,
) -> Result<(flume::Sender<Action>, Arc<RwLock<WalFileSet>>), Error> {
    let mut set = WalFileSet::read(base_path, wal_size)?;
    // TODO emit a warning log in that case and tell the user how to resolve or "force start" in
    // that case, or should be maybe `auto-heal` as much as possible?
    let mut buf = Vec::with_capacity(32);
    set.check_integrity(&mut buf, wal_deep_integrity_check)?;
    if set.files.is_empty() {
        buf.clear();
        set.add_file(wal_size, &mut buf)?;
    }
    let wal_locked = Arc::new(RwLock::new(set.clone_no_map()));

    // TODO remove with version <= 0.13
    // This is a fix for a bug from previous versions. Can be removed in later ones,
    // it would be safe to do probably around version >= 0.13.
    if meta.read()?.last_purged_log_id.is_none()
        && let Some(front) = set.files.front_mut()
        && front.wal_no > 1
        && front.id_from > 2
    {
        warn!("Trying to fix bad LogState for `last_purged_logid`");
        let mut buf = Vec::with_capacity(16);
        let mut memo: Option<LogReadMemo> = None;
        front.mmap()?;
        front.read_logs(front.id_from, front.id_until, &mut memo, &mut buf)?;
        let (_, bytes) = buf.first().unwrap();
        let log: openraft::log_id::LogId<u64> = deserialize(bytes)?;
        let log_id: openraft::log_id::LogId<u64> = LogId {
            leader_id: LeaderId {
                term: log.leader_id.term,
                node_id: log.leader_id.node_id,
            },
            index: log.index - 1,
        };
        front.mmap_drop();

        meta.write()?.last_purged_log_id = Some(serialize(&log_id)?);
        Metadata::write(meta.clone(), &set.base_path)?;
    }

    let (tx, rx) = flume::bounded::<Action>(1);
    let wal = wal_locked.clone();
    let snc = sync.clone();
    thread::spawn(move || run(lockfile, meta, wal, set, rx, snc, wal_size));

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
    lockfile: LockFile,
    meta: Arc<RwLock<Metadata>>,
    wal_locked: Arc<RwLock<WalFileSet>>,
    mut wal: WalFileSet,
    rx: flume::Receiver<Action>,
    sync: LogSync,
    wal_size: u32,
) -> Result<(), Error> {
    let _ = ThreadPriority::Max.set_for_current();

    let mut is_dirty = false;
    let mut shutdown_ack: Option<oneshot::Sender<()>> = None;
    let data_len_limit = wal_size as usize - wal.active().offset_logs() - 2;

    // openraft will read chunks of 64 logs for bigger tasks
    let mut buf: Vec<u8> = Vec::with_capacity(64);
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
                            // A single raft entry cannot span WAL files. By default an
                            // oversized entry is a non-recoverable setup issue (it needs a
                            // config change with a full restart, or code changes), so the
                            // writer panics. With the `oversized-entry-error` feature the
                            // append instead fails with `Error::WalSizeExceeded` and the
                            // writer keeps serving subsequent requests. With the default
                            // `wal_size` of 2MB this is easily reached by a single large
                            // INSERT, transaction or batch.
                            #[cfg(feature = "oversized-entry-error")]
                            {
                                res = Err(Error::WalSizeExceeded(
                                    format!(
                                        "`data` length must not exceed `wal_size` -> data \
                                        length is {} vs wal_size (without header) is \
                                        {data_len_limit}",
                                        bytes.len(),
                                    )
                                    .into(),
                                ));
                                break;
                            }
                            #[cfg(not(feature = "oversized-entry-error"))]
                            {
                                panic!(
                                    "`data` length must not exceed `wal_size` -> data length \
                                    is {} vs wal_size (without header) is {data_len_limit}",
                                    bytes.len(),
                                );
                            }
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
                            "active.id_until and id don't match: {} != {id}",
                            active.id_until
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
                    error!("error sending back ack after logs append: {err:?}");
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
                    "WAL Writer - Action::Remove from {from} until {until} / \
                    last_log: {last_log:?}\n{wal:?}"
                );

                // Before removing any logs, make sure that all in-memory buffers are flushed. If
                // at least headers and metadata are not up to date, and a crash happens in the
                // middle of removing logs, we could end up with a hole between Snapshot and latest
                // existing Raft Log, which must never happen.
                let active = wal.active();
                if is_dirty {
                    buf.clear();
                    active.update_header(&mut buf)?;
                    // async is fine, as long as we trigger it before starting log removal.
                    // If the flush fails, so would the log removal, and we would not have a hole.
                    active.flush_async()?;
                    is_dirty = false;
                }

                // Persist `last_purged_log_id` before deleting WAL files: a stale (too high)
                // value would point into deleted files; a too-low one keeps extra files (safe).
                if last_log.is_some() {
                    meta.write()?.last_purged_log_id = last_log;
                    Metadata::write(meta.clone(), &wal.base_path)?;
                }

                buf.clear();
                buf_logs.clear();
                match wal.shift_delete_logs(from, until, wal_size, &mut buf, &mut buf_logs) {
                    Ok(_) => {
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

                buf.clear();
                wal.active().flush_async()?;
                is_dirty = false;

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
                debug!("Raft logs store writer is being shut down");
                shutdown_ack = Some(ack);
                break;
            }
        }
    }

    debug!("Logs Writer exiting");

    let active = wal.active();
    buf.clear();
    active.update_header(&mut buf)?;
    active.flush()?;
    Metadata::write(meta, &wal.base_path)?;

    // drop the lockfile before trying to remove it to unlock it
    drop(lockfile);
    LockFile::remove(&wal.base_path).expect("LockFile removal failed");

    if let Some(ack) = shutdown_ack {
        ack.send(())
            .expect("Shutdown handler to always wait for ack from logs");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "oversized-entry-error")]
    fn append(tx: &flume::Sender<Action>, id: u64, bytes: Vec<u8>) -> Result<(), Error> {
        let (ack_tx, ack_rx) = oneshot::channel();
        let (entry_tx, entry_rx) = flume::bounded(1);
        tx.send(Action::Append {
            rx: entry_rx,
            callback: Box::new(|| {}),
            ack: ack_tx,
        })
        .unwrap();
        entry_tx.send(Some((id, bytes))).unwrap();
        entry_tx.send(None).unwrap();
        ack_rx.blocking_recv().unwrap()
    }

    #[cfg(feature = "oversized-entry-error")]
    #[test]
    fn oversized_entry_errors_without_killing_writer() {
        let base = "test_data/oversized_entry".to_string();
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        let lockfile = LockFile::create(&base).unwrap();
        lockfile.lock().unwrap();
        let meta = Arc::new(RwLock::new(Metadata::read_or_create(&base).unwrap()));

        let (tx, _wal) = spawn(
            base.clone(),
            lockfile,
            LogSync::Immediate,
            64 * 1024,
            false,
            meta.clone(),
        )
        .unwrap();

        // an entry larger than the whole WAL file must error, not panic the writer
        let res = append(&tx, 1, vec![0u8; 70 * 1024]);
        assert!(res.is_err(), "oversized entry must error, got {res:?}");

        // the writer must still be alive and accept a normal append afterwards
        let res = append(&tx, 2, b"ok".to_vec());
        assert!(res.is_ok(), "writer must survive: {res:?}");

        // graceful shutdown (removes the lockfile)
        let (ack_tx, ack_rx) = oneshot::channel();
        tx.send(Action::Shutdown(ack_tx)).unwrap();
        ack_rx.blocking_recv().unwrap();
    }

    #[cfg(not(feature = "oversized-entry-error"))]
    #[test]
    fn oversized_entry_panics_and_kills_writer() {
        let base = "test_data/oversized_entry_panic".to_string();
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        let lockfile = LockFile::create(&base).unwrap();
        lockfile.lock().unwrap();
        let meta = Arc::new(RwLock::new(Metadata::read_or_create(&base).unwrap()));

        let (tx, _wal) = spawn(
            base.clone(),
            lockfile,
            LogSync::Immediate,
            64 * 1024,
            false,
            meta.clone(),
        )
        .unwrap();

        let send_append = |id: u64, bytes: Vec<u8>| {
            let (ack_tx, ack_rx) = oneshot::channel();
            let (entry_tx, entry_rx) = flume::bounded(1);
            // The writer thread may already be dead, so every send can fail and is ignored;
            // the assertion is that the append is never acked.
            let _ = tx.send(Action::Append {
                rx: entry_rx,
                callback: Box::new(|| {}),
                ack: ack_tx,
            });
            let _ = entry_tx.send(Some((id, bytes)));
            let _ = entry_tx.send(None);
            ack_rx
        };

        // an oversized entry panics the writer thread by default; the append is never acked
        let mut ack = send_append(1, vec![0u8; 70 * 1024]);
        assert!(
            !recv_with_timeout(&mut ack),
            "oversized entry must panic the writer (no ack)"
        );

        // the writer thread is dead: a subsequent append is never acked either
        let mut ack = send_append(2, b"ok".to_vec());
        assert!(
            !recv_with_timeout(&mut ack),
            "writer must be dead after the panic"
        );
    }

    #[cfg(not(feature = "oversized-entry-error"))]
    fn recv_with_timeout(ack: &mut tokio::sync::oneshot::Receiver<Result<(), Error>>) -> bool {
        for _ in 0..10 {
            if ack.try_recv().is_ok() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }
}
