use crate::helpers::deserialize;
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::{Error, NodeId};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use hiqlite_wal::{writer, Action, LogStore};
use openraft::{
    AnyError, Entry, ErrorSubject, ErrorVerb, LogId, RaftTypeConfig, StorageIOError, Vote,
};
use rocksdb::{ColumnFamilyDescriptor, DBWithThreadMode, Direction, Options, SingleThreaded, DB};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::{fs, task, time};
use tracing::log::__private_api::loc;
use tracing::{debug, error, info, trace, warn};

static KEY_LAST_PURGED: &[u8] = b"last_purged";
static KEY_VOTE: &[u8] = b"vote";

/// Checks if `rocksdb` files are in the target folder and tries to perform a migration from
/// rocksdb to `hiqlite-wal` in that case.
#[tracing::instrument]
pub async fn check_migrate_rocksdb(
    logs_dir: String,
    wal_size: u32,
    wal_ignore_lock: bool,
) -> Result<(), Error> {
    #[cfg(feature = "rocksdb")]
    panic!("Feature `migrate-rocksdb` only makes sense when `rocksdb` is not used as logs store");

    // the bare minimum of files that must be there for a possibly existing
    // rocksdb is a `LOG` file
    if !fs::try_exists(format!("{}/LOG", logs_dir)).await? {
        info!(
            "No rocksdb LOG file found in {} - nothing to migrate",
            logs_dir
        );
        return Ok(());
    }

    if let Some(db) = try_open_db(&logs_dir) {
        info!("Found existing rocksdb - starting migration to `hiqlite-wal`");
        // cleanup possibly existing hiqlite-wal files
        let mut files = Vec::with_capacity(4);
        let mut dir = fs::read_dir(&logs_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name();
            let fname = name.to_str().unwrap_or_default();
            if fname.ends_with(".hql") || fname.ends_with(".wal") {
                files.push(fname.to_string());
            }
        }
        info!("Cleaning up existing hiqlite-wal files: {:?}", files);
        for file in files.drain(..) {
            fs::remove_file(format!("{}/{}", logs_dir, file)).await?;
        }

        info!("starting hiqlite-wal writer for migration");
        let writer = LogStore::<TypeConfigSqlite>::start_writer_migration(
            logs_dir.clone(),
            wal_size,
            wal_ignore_lock,
        )
        .await?;
        task::spawn_blocking(move || async {
            if let Err(err) = migrate(db, writer).await {
                panic!("Cannot migrate from rocksdb: {:?}", err);
            }
        })
        .await?;

        // cleanup old rocksdb files
        let mut dirs = Vec::with_capacity(2);
        let mut dir = fs::read_dir(&logs_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name();
            let fname = name.to_str().unwrap_or_default();
            if !fname.ends_with(".hql") && !fname.ends_with(".wal") {
                let meta = entry.metadata().await?;
                if meta.is_dir() {
                    dirs.push(fname.to_string());
                } else {
                    files.push(fname.to_string());
                }
            }
        }
        info!("Cleaning up old rocksdb files: {:?}", files);
        for file in files {
            fs::remove_file(format!("{}/{}", logs_dir, file)).await?;
        }
        info!("Cleaning up old rocksdb dirs: {:?}", dirs);
        for dir in dirs {
            fs::remove_dir_all(format!("{}/{}", logs_dir, dir)).await?;
        }

        // wait for writer shutdown
        let lock_file = format!("{}/lock.hql", logs_dir);
        while fs::try_exists(&lock_file).await? {
            info!("Writer is still shutting down - waiting ...");
            time::sleep(Duration::from_millis(500)).await;
        }
    } else {
        info!("No existing rocksdb Log Store found - nothing to migrate");
    }

    Ok(())
}

fn try_open_db(dir: &str) -> Option<DBWithThreadMode<SingleThreaded>> {
    let mut opts = Options::default();
    opts.create_missing_column_families(false);
    opts.create_if_missing(false);

    let meta = ColumnFamilyDescriptor::new("meta", opts.clone());
    let logs = ColumnFamilyDescriptor::new("logs", opts.clone());
    DB::open_cf_descriptors(&opts, dir, vec![meta, logs]).ok()
}

async fn migrate(
    db: DBWithThreadMode<SingleThreaded>,
    writer: flume::Sender<Action>,
) -> Result<(), Error> {
    // VOTE
    let vote = db.get_cf(db.cf_handle("meta").unwrap(), KEY_VOTE).unwrap();
    if let Some(value) = vote {
        let (ack, rx) = oneshot::channel();
        writer.send(Action::Vote { value, ack }).unwrap();
        rx.await.unwrap();
    }

    let res = db.get_cf(db.cf_handle("meta").unwrap(), KEY_LAST_PURGED);
    let last_purged_log_id = if let Some(bytes) = res.unwrap() {
        Some(deserialize::<
            LogId<<TypeConfigSqlite as RaftTypeConfig>::NodeId>,
        >(&bytes)?)
    } else {
        None
    };

    let (tx, rx) = flume::bounded(1);
    let (ack, rx_ack) = oneshot::channel();
    writer
        .send(Action::Append {
            rx,
            callback: Box::new(|| {}),
            ack,
        })
        .unwrap();

    let logs = db.iterator_cf(db.cf_handle("logs").unwrap(), rocksdb::IteratorMode::Start);
    for log in logs {
        match log {
            Ok((id, value)) => {
                let id = (&id[0..8]).read_u64::<BigEndian>()?;
                tx.send(Some((id, value.to_vec()))).unwrap();
            }
            Err(_) => {
                tx.send(None).unwrap();
                break;
            }
        }
    }
    rx_ack.await.unwrap()?;

    let (tx, rx) = oneshot::channel();
    writer.send(Action::Shutdown(tx)).unwrap();
    rx.await.unwrap();

    info!("Migration finished");

    Ok(())
}
