use crate::error::Error;
use crate::metadata::Metadata;
use crate::wal::WalFileSet;
use memmap2::{Advice, MmapOptions};
use openraft::{StorageError, StorageIOError};
use std::collections::VecDeque;
use std::fs;
use std::fs::OpenOptions;
use tokio::sync::oneshot;

pub enum Action {
    Append {
        rx: flume::Receiver<Option<(Vec<u8>, Vec<u8>)>>,
        // TODO with 0.10 the callback will be async ready
        // callback: LogFlushed<TypeConfigSqlite>,
        ack: oneshot::Sender<Result<(), StorageIOError<u64>>>,
    },
    Remove {
        from: Vec<u8>,
        until: Vec<u8>,
        last_log: Option<Vec<u8>>,
        ack: oneshot::Sender<Result<(), StorageError<u64>>>,
    },
    Vote {
        value: Vec<u8>,
        ack: oneshot::Sender<Result<(), StorageIOError<u64>>>,
    },
    Sync,
    Shutdown(oneshot::Sender<()>),
}

pub fn spawn(
    base_path: &str,
    sync_immediate: bool,
    file_size: u32,
) -> Result<flume::Sender<Action>, Error> {
    create_base_path(base_path)?;

    let meta = Metadata::read(base_path)?;
    let mut set = WalFileSet::read(base_path)?;
    set.check_integrity(&meta)?;
    if set.headers.is_empty() {
        set.add_header(file_size)?;
    }

    let header = set.headers.last().unwrap();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        // the file should already exist at this point
        .create(false)
        .open(&base_path)?;

    let mut mmap = unsafe { MmapOptions::new().populate().map_mut(&file)? };
    mmap.advise(Advice::Sequential)?;

    let mut files = VecDeque::with_capacity(2);
    files.push_front((header, mmap));

    let (tx, rx) = flume::bounded::<Action>(1);
    // TODO
    Ok(tx)
}

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

fn run() -> Result<(), Error> {
    todo!()
}
