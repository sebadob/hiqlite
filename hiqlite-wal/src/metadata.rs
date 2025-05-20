use crate::error::Error;
use crate::utils::{crc, deserialize, serialize};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::ops::Deref;
use std::sync::{Arc, RwLock};

static MAGIC_NO_META: &[u8] = b"HQLMETA";

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub log_from: u64,
    pub log_until: u64,
    pub last_purged_log_id: Option<Vec<u8>>,
    pub vote: Option<Vec<u8>>,
}

impl Metadata {
    pub fn read(base_path: &str) -> Result<Self, Error> {
        let path = format!("{}/meta.hql", base_path);
        let Ok(bytes) = fs::read(&path) else {
            return Err(Error::InvalidPath("cannot open metadata file"));
        };
        if bytes.len() < 16 {
            return Err(Error::FileCorrupted("invalid metadata file length"));
        }

        debug_assert_eq!(MAGIC_NO_META.len(), 7);
        if bytes[..7].iter().as_slice() != MAGIC_NO_META {
            return Err(Error::FileCorrupted(
                "metadata file is corrupt - magic no does not match",
            ));
        }
        let version = &bytes[7..8];
        match version {
            [1u8] => {
                let crc = &bytes[8..12];
                if crc != crc!(&bytes[12..]) {
                    return Err(Error::FileCorrupted("metadata CRC checksum does not match"));
                }
                Ok(deserialize::<Self>(&bytes[12..])?)
            }
            _ => Err(Error::FileCorrupted("unknown metadata file version")),
        }
    }

    #[inline]
    pub fn write(meta: Arc<RwLock<Self>>, base_path: &str) -> Result<(), Error> {
        let path = format!("{}/meta.hql", base_path);

        let slf_bytes = {
            let lock = meta.read()?;
            serialize(lock.deref())?
        };

        let _ = fs::remove_file(&path);
        let mut file = File::create_new(&path)?;
        // TODO overwriting the file when we started with .seek() did not work when the
        // new meta was smaller than the old one, and therefore the CRC would not match.
        // let mut file = OpenOptions::new()
        //     .read(true)
        //     .write(true)
        //     .create(true)
        //     .open(&path)?;

        debug_assert_eq!(MAGIC_NO_META.len(), 7);
        file.write_all(MAGIC_NO_META)?;
        file.write_all(&[1u8])?;
        file.write_all(crc!(&slf_bytes).as_slice())?;
        file.write_all(&slf_bytes)?;
        file.flush()?;

        Ok(())
    }
}

pub struct LockFile;

impl LockFile {
    pub fn write(base_path: &str) -> Result<(), Error> {
        let path = format!("{base_path}/lock.hql");
        match File::open(&path) {
            Ok(_) => Err(Error::Locked("WAL is locked, cannot create lock file")),
            Err(_) => {
                File::create(path)?;
                Ok(())
            }
        }
    }

    pub fn remove(base_path: &str) -> Result<(), Error> {
        let path = format!("{base_path}/lock.hql");
        fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "test_data";

    #[test]
    fn lockfile() -> Result<(), Error> {
        LockFile::write(&PATH)?;
        assert!(LockFile::write(&PATH).is_err());
        LockFile::remove(&PATH)?;

        LockFile::write(&PATH)?;
        assert!(LockFile::write(&PATH).is_err());
        LockFile::remove(&PATH)?;

        Ok(())
    }

    #[test]
    fn metadata_write_read() -> Result<(), Error> {
        let base_path = format!("{}/metadata_write_read", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let meta = Arc::new(RwLock::new(Metadata {
            log_from: 13,
            log_until: 27,
            last_purged_log_id: Some(vec![13, 17, 43]),
            vote: None,
        }));
        Metadata::write(meta.clone(), &base_path)?;

        let meta_back = Metadata::read(&base_path)?;
        let lock = meta.read()?;
        assert_eq!(lock.log_from, meta_back.log_from);
        assert_eq!(lock.log_until, meta_back.log_until);
        assert_eq!(lock.last_purged_log_id, meta_back.last_purged_log_id);
        assert_eq!(lock.vote, meta_back.vote);

        Ok(())
    }
}
