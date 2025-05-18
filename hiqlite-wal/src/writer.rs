use crate::error::Error;
use crate::metadata::Metadata;
use crate::utils::id_to_bin;
use crate::wal::{WalFile, WalFileSet, WalRecord};
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
) -> Result<flume::Sender<Action>, Error> {
    create_base_path(&base_path)?;

    let meta = Metadata::read(&base_path)?;
    let mut set = WalFileSet::read(base_path, wal_size)?;
    // TODO emit a warning log in that case and tell the user how to resolve or "force start" in
    // that case
    set.check_integrity(&meta)?;
    if set.files.is_empty() {
        let mut buf = Vec::with_capacity(28);
        set.add_file(wal_size, &mut buf)?;
    }

    let (tx, rx) = flume::bounded::<Action>(1);
    thread::spawn(move || run(meta, set, rx, sync_immediate, wal_size));
    Ok(tx)
}

fn run(
    mut meta: Metadata,
    mut wal: WalFileSet,
    rx: flume::Receiver<Action>,
    sync_immediate: bool,
    wal_size: u32,
) -> Result<(), Error> {
    let mut is_dirty = false;
    let mut shutdown_ack: Option<oneshot::Sender<()>> = None;

    let mut buf = Vec::with_capacity(8);

    while let Ok(action) = rx.recv() {
        match action {
            Action::Append { rx, callback, ack } => {
                let mut res = Ok(());

                while let Ok(Some((id, data))) = rx.recv() {
                    buf.clear();
                    if !wal.active().has_space(data.len()) {
                        todo!("roll WAL");
                    }

                    wal.active().append_log(id, &data, &mut buf)?;
                }

                is_dirty = true;
                let is_ok = res.is_ok();

                if let Err(err) = ack.send(res) {
                    // this should usually not happen, but it may during a shutdown crash
                    error!("error sending back ack after logs append: {:?}", err);
                    // db.flush_wal(true);
                }

                if is_ok {
                    // TODO with the next big openraft release, we can do async callbacks

                    if sync_immediate {
                        wal.active().flush()?;
                        is_dirty = false;
                    }
                    callback();
                }

                todo!()
            }
            Action::Remove {
                from,
                until,
                last_log,
                ack,
            } => todo!(),
            Action::Vote { value, ack } => {
                meta.vote = Some(value);
                let res = meta.write(&wal.base_path);
                ack.send(res).unwrap();
            }
            Action::Sync => {
                if is_dirty {
                    wal.active().flush()?;
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
