use crate::error::Error;
use crate::metadata::Metadata;
use crate::utils::{bin_to_id, id_to_bin};
use std::fs;
use std::fs::File;
use std::io::Read;

static MAGIC_NO_WAL: &[u8] = b"HQL_WAL";

#[derive(Debug)]
pub struct WalFile {
    pub version: u8,
    pub wal_no: u64,
    pub path: String,
    pub id_from: u64,
    pub id_until: u64,
}

impl WalFile {
    #[inline]
    pub fn new(wal_no: u64, base_path: &str, id_from: u64, id_until: u64) -> Self {
        debug_assert!(!base_path.is_empty());
        debug_assert!(id_from <= id_until);
        let path = Self::build_full_path(base_path, wal_no);

        Self {
            wal_no,
            path,
            version: 1,
            id_from,
            id_until,
        }
    }

    #[inline]
    pub fn create_file(&self, wal_size: u32) -> Result<(), Error> {
        let file = File::create_new(&self.path)?;
        file.set_len(wal_size as u64)?;
        Ok(())
    }

    #[inline]
    pub fn read_from_file(path_full: String) -> Result<Self, Error> {
        let Some((_, fname)) = path_full.rsplit_once('/') else {
            return Err(Error::InvalidPath("Invalid file path"));
        };
        let Some((num, ending)) = fname.split_once('.') else {
            return Err(Error::InvalidPath("Invalid file path"));
        };

        if ending != "wal" {
            return Err(Error::InvalidFileName);
        }
        let wal_no = num.parse::<u64>()?;

        let mut buf = vec![0; 24];
        let mut file = File::open(&path_full)?;
        file.read_exact(&mut buf)?;

        if buf[..7].iter().as_slice() != MAGIC_NO_WAL {
            return Err(Error::FileCorrupted("Invalid WAL file magic number"));
        }
        if buf[7..8] != [1u8] {
            return Err(Error::FileCorrupted("Invalid WAL file version"));
        }
        let id_from = bin_to_id(&buf[8..16])?;
        let id_until = bin_to_id(&buf[16..24])?;
        debug_assert!(id_from < id_until);

        Ok(Self {
            version: 1,
            wal_no,
            path: path_full,
            id_from,
            id_until,
        })
    }

    #[inline]
    pub fn write_header(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        buf.extend_from_slice(MAGIC_NO_WAL);
        buf.push(self.version);
        id_to_bin(self.id_from, buf)?;
        id_to_bin(self.id_until, buf)?;
        Ok(())
    }

    #[inline]
    pub fn offset_start(&self) -> usize {
        match self.version {
            1 => 24,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn build_full_path(base_path: &str, wal_no: u64) -> String {
        debug_assert!(!base_path.is_empty());

        let wal_no_str = wal_no.to_string();
        let zeros = "000000000000000";
        debug_assert_eq!(zeros.len(), 15);
        let path = format!(
            "{}/{}{}.wal",
            base_path,
            &zeros[..16 - wal_no_str.len()],
            wal_no_str
        );
        #[cfg(debug_assertions)]
        {
            println!("path: {path}");
            let (base, name) = path.rsplit_once('/').unwrap();
            debug_assert_eq!(base, base_path);
            let name = name.strip_suffix(".wal").unwrap();
            debug_assert_eq!(name.len(), 16);
        }
        path
    }
}

#[derive(Debug)]
pub struct WalFileSet<'a> {
    pub base_path: &'a str,
    pub headers: Vec<WalFile>,
}

impl WalFileSet<'_> {
    #[inline]
    pub fn active_file(&self) -> &WalFile {
        debug_assert!(!self.headers.is_empty());
        if self.headers.len() > 1 {
            let last = self.headers.last().unwrap();
            let before = self.headers.get(self.headers.len() - 2).unwrap();

            if before.id_until > 0 && last.id_until == 0 {
                before
            } else {
                last
            }
        } else {
            self.headers.last().unwrap()
        }
    }

    pub fn new(base_path: &str) -> WalFileSet {
        WalFileSet {
            base_path,
            headers: Vec::default(),
        }
    }

    /// Adds a new `Header` at the end and creates a file for it.
    pub fn add_header(&mut self, wal_size: u32) -> Result<&WalFile, Error> {
        let wal_no = if self.headers.is_empty() {
            1
        } else {
            self.headers.last().unwrap().wal_no + 1
        };

        let header = WalFile::new(wal_no, &self.base_path, 0, 0);
        header.create_file(wal_size)?;
        self.headers.push(header);

        Ok(self.headers.last().unwrap())
    }

    pub fn read(base_path: &str) -> Result<WalFileSet, Error> {
        let mut headers = Vec::with_capacity(2);

        for entry in fs::read_dir(&base_path)? {
            let entry = entry?.file_name();
            let fname = entry.to_str().unwrap_or_default();
            if fname.ends_with(".wal") {
                let path_full = format!("{}/{}", base_path, fname);
                if let Ok(wal) = WalFile::read_from_file(path_full) {
                    headers.push(wal);
                }
            }
        }

        headers.sort_by(|a, b| a.wal_no.cmp(&b.wal_no));
        Ok(WalFileSet { base_path, headers })
    }

    /// Checks the integrity of the Headers and makes sure the order is strictly ascending and
    /// there are no missing log IDs.
    pub fn check_integrity(&self, metadata: &Metadata) -> Result<(), Error> {
        if self.headers.is_empty() {
            if metadata.log_from != 0 || metadata.log_until != 0 {
                return Err(Error::FileCorrupted(
                    "Expected WAL files from Metadata but none found",
                ));
            }
            return Ok(());
        }

        let mut iter = self.headers.iter();

        let first = iter.next().unwrap();
        let mut wal_no = first.wal_no;
        let mut until = first.id_until;
        if first.id_from > until {
            return Err(Error::FileCorrupted(
                "`id_from` cannot be greater than `id_until`",
            ));
        }
        if metadata.log_from < first.id_from {
            return Err(Error::FileCorrupted(
                "Metadata expected lower log ID from than found",
            ));
        }

        for header in iter {
            if wal_no + 1 != header.wal_no {
                return Err(Error::Integrity(
                    format!("Missing wal file no {}", wal_no + 1).into(),
                ));
            }
            // if there is already a new prepared header at the end, which is no in use yet,
            // it will have both ids set to 0 until first time use
            if header.id_from == 0 && header.id_until == 0 {
                break;
            }
            if until + 1 != header.id_from {
                return Err(Error::Integrity(
                    format!(
                        "Missing logs between IDs {} and {}",
                        until + 1,
                        header.id_from
                    )
                    .into(),
                ));
            }

            wal_no = header.wal_no;
            until = header.id_until;
        }

        if metadata.log_until > until {
            Err(Error::FileCorrupted(
                "Metadata expected higher log ID until than found",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const PATH: &str = "test_data";

    #[test]
    fn convert_wal_header() -> Result<(), Error> {
        let base_path = format!("{}/convert_wal_header", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let header = WalFile::new(1, &base_path, 23, 1337);

        let mut buf = Vec::with_capacity(24);
        header.write_header(&mut buf)?;

        // make sure we are cleaned up
        let path_with_no = format!("{base_path}/0000000000000001.wal");
        let _ = fs::remove_file(&path_with_no);
        fs::write(&path_with_no, &buf)?;

        let header_read = WalFile::read_from_file(path_with_no.clone())?;

        assert_eq!(header.version, header_read.version);
        assert_eq!(header.path, header_read.path);
        assert_eq!(header.wal_no, header_read.wal_no);
        assert_eq!(header.id_from, header_read.id_from);
        assert_eq!(header.id_until, header_read.id_until);

        let path_h1 = format!("{}/0000000000000001.wal", base_path);
        let path_h2 = format!("{}/0000000000000002.wal", base_path);
        let _ = fs::remove_file(&path_with_no);
        let _ = fs::remove_file(&path_h1);
        let _ = fs::remove_file(&path_h2);

        let mut set = WalFileSet::new(&base_path);
        set.add_header(8).unwrap();
        assert_eq!(fs::exists(&path_h1)?, true);
        assert_eq!(fs::exists(&path_h2)?, false);
        set.add_header(8).unwrap();
        assert_eq!(fs::exists(&path_h2)?, true);

        Ok(())
    }

    #[test]
    fn integrity_check() -> Result<(), Error> {
        let base_path = format!("{}/integrity_check", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let meta = Metadata {
            log_from: 1,
            log_until: 33,
            last_purged: None,
            vote: None,
        };
        let mut set = WalFileSet {
            base_path: &base_path,
            headers: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                },
            ],
        };
        set.check_integrity(&meta).unwrap();
        assert_eq!(set.active_file().wal_no, 3);

        set.add_header(8)?;
        set.check_integrity(&meta).unwrap();
        assert_eq!(set.active_file().wal_no, 3);

        let set = WalFileSet {
            base_path: &base_path,
            headers: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            base_path: &base_path,
            headers: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                },
                WalFile {
                    version: 1,
                    wal_no: 4,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            base_path: &base_path,
            headers: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 2,
                    id_until: 10,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            base_path: &base_path,
            headers: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 32,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            base_path: &base_path,
            headers: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 19,
                    id_until: 33,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        Ok(())
    }
}
