use crate::error::Error;
use fs4::fs_std::FileExt;
use std::fs::{self, File, OpenOptions};

#[derive(Debug)]
pub struct LockFile {
    file: File,
}

impl LockFile {
    #[inline]
    pub fn exists(base_path: &str) -> Result<bool, Error> {
        Ok(fs::exists(Self::path(base_path))?)
    }

    pub fn create(base_path: &str) -> Result<Self, Error> {
        let path = Self::path(base_path);

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .map_err(|err| {
                Error::Internal(format!("Cannot create WAL lock file {}: {}", path, err).into())
            })?;

        Ok(Self { file })
    }

    pub fn is_locked(base_path: &str) -> Result<bool, Error> {
        let path = Self::path(base_path);
        let file = File::open(path)?;
        let is_locked = Self::lock_file(&file).is_err();
        Ok(is_locked)
    }

    pub fn lock(&self) -> Result<(), Error> {
        Self::lock_file(&self.file)
    }

    fn lock_file(file: &File) -> Result<(), Error> {
        match file.try_lock_exclusive() {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::Internal(
                "WAL lock file is in use by another process".into(),
            )),
            Err(err) => Err(Error::Internal(
                format!("Error locking WAL lock file: {}", err).into(),
            )),
        }
    }

    /// Will not work if the locked `LockFile` was not dropped before.
    pub fn remove(base_path: &str) -> Result<(), Error> {
        fs::remove_file(Self::path(base_path))?;
        Ok(())
    }

    #[inline]
    fn path(base_path: &str) -> String {
        format!("{base_path}/lock.hql")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "test_data";

    #[test]
    fn lockfile() {
        let base_path = format!("{}/lockfile", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path).unwrap();

        assert!(!LockFile::exists(&base_path).unwrap());
        let file = LockFile::create(&base_path).unwrap();
        assert!(LockFile::exists(&base_path).unwrap());

        assert!(!LockFile::is_locked(&base_path).unwrap());
        file.lock().unwrap();
        assert!(LockFile::is_locked(&base_path).unwrap());

        // make sure it's still locked even if we create a new file
        let file_new = LockFile::create(&base_path).unwrap();
        assert!(LockFile::is_locked(&base_path).unwrap());
        drop(file_new);

        LockFile::remove(&base_path).unwrap();

        assert!(LockFile::is_locked(&base_path).is_err());
        // create a new one, it should not be locked anymore
        let _file = LockFile::create(&base_path).unwrap();
        assert!(!LockFile::is_locked(&base_path).unwrap());
    }
}
