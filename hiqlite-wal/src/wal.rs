use crate::error::Error;
use crate::metadata::Metadata;
use crate::utils::{bin_to_id, bin_to_len, crc, id_to_bin, len_to_bin};
use memmap2::{Advice, Mmap, MmapMut, MmapOptions};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

static MAGIC_NO_WAL: &[u8] = b"HQL_WAL";
static MIN_WAL_SIZE: u32 = 2 * 1024 * 1024;

#[derive(Debug)]
pub struct WalFile {
    pub version: u8,
    pub wal_no: u64,
    pub path: String,
    pub id_from: u64,
    pub id_until: u64,
    data_start: Option<u32>,
    data_end: Option<u32>,
    len_max: u32,
    mmap: Option<Mmap>,
    mmap_mut: Option<MmapMut>,
}

impl Drop for WalFile {
    fn drop(&mut self) {
        if self.mmap_mut.is_some() {
            let mut buf = Vec::with_capacity(32);
            self.update_header(&mut buf).unwrap();
            self.flush().unwrap();
        }
    }
}

impl WalFile {
    #[inline]
    pub fn has_space(&self, data_len: usize) -> bool {
        // wal entries will have:
        // - 8 byte id
        // - 4 byte crc
        // - 8 bytes data length
        // - variable length data
        (self.len_max - self.data_end.unwrap_or(0)) as usize > 8 + 4 + 8 + data_len
    }

    #[inline]
    pub fn space_left(&self) -> u32 {
        self.len_max - self.data_end.unwrap_or(0)
    }

    /// Expects to have enough space left -> check MUST be done upfront
    pub fn append_log(&mut self, id: u64, data: &[u8], buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(self.mmap_mut.is_some());
        debug_assert!(self.has_space(data.len()));
        debug_assert!(data.len() < u32::MAX as usize);
        debug_assert_eq!(self.data_start.is_some(), self.data_end.is_some());

        let start = if self.data_start.is_none() {
            debug_assert_eq!(self.id_from, 0);
            self.id_from = id;
            let start = self.offset_logs();
            self.data_start = Some(start as u32);
            start
        } else {
            self.data_end.unwrap() as usize + 1
        };
        self.id_until = id;

        let mmap = self.mmap_mut.as_mut().unwrap();
        id_to_bin(id, buf)?;
        (&mut mmap[start..]).write_all(buf)?;

        let crc = crc!(&data);
        (&mut mmap[start + 8..]).write_all(crc.as_slice())?;

        let len = data.len() as u32;
        buf.clear();
        len_to_bin(len, buf)?;
        (&mut mmap[start + 8 + 4..]).write_all(buf)?;

        (&mut mmap[start + 8 + 4 + 4..]).write_all(data)?;

        self.data_end = Some((start + 8 + 4 + 4 + data.len()) as u32);

        Ok(())
    }

    pub fn read_logs(
        &self,
        id_from: u64,
        id_until: u64,
        buf: &mut Vec<(u64, Vec<u8>)>,
    ) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(self.data_start.is_some());
        debug_assert!(self.data_end.is_some());
        debug_assert!(id_until >= id_from);

        if self.id_from > id_from {
            return Err(Error::Generic("`id_from` is below threshold".into()));
        }
        if self.id_until < id_until {
            return Err(Error::Generic("`id_until` is above threshold".into()));
        }
        if id_until < id_from {
            return Err(Error::Generic(
                "`id_until` cannot be smaller than `id_from`".into(),
            ));
        }

        let mut idx = self.data_start.unwrap();
        loop {
            // id, crc, length
            let head = self.read_bytes(idx, idx + 8 + 4 + 4)?;
            let id = bin_to_id(&head[..8])?;
            let crc = &head[8..12];
            let len = bin_to_len(&head[12..16])?;

            if id >= id_from {
                if id <= id_until {
                    let data_from = idx + 8 + 4 + 4;
                    debug_assert!(data_from + len <= self.data_end.unwrap());
                    let data = self.read_bytes(data_from, data_from + len)?;
                    if crc != crc!(data) {
                        return Err(Error::Integrity("Invalid CRC for WAL Record".into()));
                    }
                    buf.push((id, data.to_vec()))
                } else {
                    break;
                }
            }

            if id >= id_until {
                break;
            }

            // add 1 because the `len` is inclusive and points at the last byte of data
            idx += 8 + 4 + 4 + len + 1;
            debug_assert!(idx - 1 <= self.data_end.unwrap());
        }

        debug_assert_eq!(id_until + 1 - id_from, buf.len() as u64);

        Ok(())
    }

    /// Does NOT do any boundary checks for the given range!
    #[inline(always)]
    fn read_bytes(&self, from: u32, until: u32) -> Result<&[u8], Error> {
        debug_assert!(from < until);
        if let Some(mmap) = &self.mmap {
            Ok(&mmap[from as usize..until as usize])
        } else if let Some(mmap) = &self.mmap_mut {
            Ok(&mmap[from as usize..until as usize])
        } else {
            Err(Error::Generic("No mmap exists".into()))
        }
    }

    #[inline]
    pub fn flush(&mut self) -> Result<(), Error> {
        debug_assert!(self.mmap_mut.is_some());
        self.mmap_mut.as_mut().unwrap().flush()?;
        Ok(())
    }

    #[inline]
    pub fn flush_async(&mut self) -> Result<(), Error> {
        debug_assert!(self.mmap_mut.is_some());
        self.mmap_mut.as_mut().unwrap().flush_async()?;
        Ok(())
    }

    #[inline]
    pub fn update_header(&mut self, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(self.mmap_mut.is_some());
        debug_assert!(buf.is_empty());

        self.build_header(buf)?;
        let mmap = self.mmap_mut.as_mut().unwrap();
        (&mut mmap[..buf.len()]).write_all(buf)?;

        Ok(())
    }

    #[inline]
    pub fn mmap(&mut self) -> Result<(), Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(false)
            // the file should already exist at this point
            .create(false)
            .open(&self.path)?;

        let mmap = unsafe { MmapOptions::new().populate().map(&file)? };
        mmap.advise(Advice::Sequential)?;

        self.mmap = Some(mmap);

        Ok(())
    }

    #[inline]
    pub fn mmap_mut(&mut self) -> Result<(), Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            // the file should already exist at this point
            .create(false)
            .open(&self.path)?;

        let mmap = unsafe { MmapOptions::new().populate().map_mut(&file)? };
        mmap.advise(Advice::Sequential)?;

        self.mmap_mut = Some(mmap);

        Ok(())
    }

    #[inline]
    pub fn new(
        wal_no: u64,
        base_path: &str,
        id_from: u64,
        id_until: u64,
        wal_size: u32,
    ) -> Result<Self, Error> {
        debug_assert!(!base_path.is_empty());
        debug_assert!(id_from <= id_until);
        debug_assert!(wal_size >= MIN_WAL_SIZE);
        if wal_size < MIN_WAL_SIZE {
            return Err(Error::Generic(
                format!("min allowed `wal_size` is {}", MIN_WAL_SIZE).into(),
            ));
        }

        let path = Self::build_full_path(base_path, wal_no);

        Ok(Self {
            wal_no,
            path,
            version: 1,
            id_from,
            id_until,
            data_start: None,
            data_end: None,
            len_max: wal_size,
            mmap: None,
            mmap_mut: None,
        })
    }

    #[inline]
    pub fn create_file(&mut self, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());

        let file = File::create_new(&self.path)?;
        file.set_len(self.len_max as u64)?;

        self.build_header(buf)?;

        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        (&mut mmap[..buf.len()]).write_all(buf)?;
        mmap.flush_async()?;

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

        let mut buf = vec![0; 32];
        let mut file = File::open(&path_full)?;
        // The current file size will be the max size, because new WALs will be initialized with
        // all 0s during creation.
        let len_max = file.metadata()?.len();
        debug_assert!(len_max < u32::MAX as u64);

        file.read_exact(&mut buf)?;

        if buf[..7].iter().as_slice() != MAGIC_NO_WAL {
            return Err(Error::FileCorrupted("Invalid WAL file magic number"));
        }
        if buf[7..8] != [1u8] {
            return Err(Error::FileCorrupted("Invalid WAL file version"));
        }
        let id_from = bin_to_id(&buf[8..16])?;
        let id_until = bin_to_id(&buf[16..24])?;
        println!("id_from: {} / id_until: {}", id_from, id_until);

        let data_start = bin_to_len(&buf[24..28])?;
        let data_end = bin_to_len(&buf[28..32])?;

        Ok(Self {
            version: 1,
            wal_no,
            path: path_full,
            id_from,
            id_until,
            data_start: if data_start == 0 {
                None
            } else {
                Some(data_start)
            },
            data_end: if data_end == 0 { None } else { Some(data_end) },
            len_max: len_max as u32,
            mmap: None,
            mmap_mut: None,
        })
    }

    #[inline]
    fn build_header(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());

        buf.extend_from_slice(MAGIC_NO_WAL);
        match self.version {
            1 => {
                buf.push(self.version);
                id_to_bin(self.id_from, buf)?;
                id_to_bin(self.id_until, buf)?;
                len_to_bin(self.data_start.unwrap_or(0), buf)?;
                len_to_bin(self.data_end.unwrap_or(0), buf)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    #[inline]
    pub fn offset_logs(&self) -> usize {
        match self.version {
            // MAGIC NO, version, id_from, id_until, data_start, data_end
            1 => 7 + 1 + 8 + 8 + 4 + 4,
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
pub struct WalFileSet {
    pub active: Option<usize>,
    pub base_path: String,
    pub files: Vec<WalFile>,
}

impl WalFileSet {
    #[inline]
    pub fn active(&mut self) -> &mut WalFile {
        debug_assert!(self.active.is_some());
        debug_assert!(self.files.len() > self.active.unwrap());
        self.files.get_mut(self.active.unwrap()).unwrap()
    }

    fn active_index(files: &[WalFile]) -> Result<usize, Error> {
        if files.is_empty() {
            return Err(Error::Generic(
                "Cannot find active WAL when files are empty".into(),
            ));
        }

        if files.len() > 1 {
            let last = files.last().unwrap();
            let before = files.get(files.len() - 2).unwrap();

            if before.id_until > 0 && last.id_until == 0 {
                return Ok(files.len() - 2);
            }
        }
        Ok(files.len() - 1)
    }

    pub fn new(base_path: String) -> WalFileSet {
        WalFileSet {
            active: None,
            base_path,
            files: Vec::default(),
        }
    }

    /// Adds a new `Header` at the end and creates a file for it.
    pub fn add_file(&mut self, wal_size: u32, buf: &mut Vec<u8>) -> Result<&WalFile, Error> {
        let wal_no = self.files.last().map(|w| w.wal_no + 1).unwrap_or(1);
        let mut wal = WalFile::new(wal_no, &self.base_path, 0, 0, wal_size)?;
        wal.create_file(buf)?;
        self.files.push(wal);

        Ok(self.files.last().unwrap())
    }

    pub fn read(base_path: String, wal_size: u32) -> Result<WalFileSet, Error> {
        let mut files = Vec::with_capacity(2);

        for entry in fs::read_dir(&base_path)? {
            let entry = entry?.file_name();
            let fname = entry.to_str().unwrap_or_default();
            if fname.ends_with(".wal") {
                let path_full = format!("{}/{}", base_path, fname);
                if let Ok(wal) = WalFile::read_from_file(path_full) {
                    files.push(wal);
                }
            }
        }

        let active = if files.is_empty() {
            let mut wal = WalFile::new(1, &base_path, 0, 0, wal_size)?;
            let mut buf = Vec::with_capacity(28);
            wal.create_file(&mut buf)?;
            files.push(wal);
            1
        } else {
            files.sort_by(|a, b| a.wal_no.cmp(&b.wal_no));
            files.len() - 1
        };

        Ok(Self {
            active: Some(active),
            base_path,
            files,
        })
    }

    /// Checks the integrity of the Headers and makes sure the order is strictly ascending and
    /// there are no missing log IDs.
    pub fn check_integrity(&self, metadata: &Metadata) -> Result<(), Error> {
        if self.files.is_empty() {
            if metadata.log_from != 0 || metadata.log_until != 0 {
                return Err(Error::FileCorrupted(
                    "Expected WAL files from Metadata but none found",
                ));
            }
            return Ok(());
        }

        let mut iter = self.files.iter();

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

        for wal in iter {
            if wal.data_end.unwrap_or(0) > wal.len_max {
                return Err(Error::Integrity(
                    "WAL data offset bigger than file size".into(),
                ));
            }

            if wal_no + 1 != wal.wal_no {
                return Err(Error::Integrity(
                    format!("Missing wal file no {}", wal_no + 1).into(),
                ));
            }
            // if there is already a new prepared header at the end, which is no in use yet,
            // it will have both ids set to 0 until first time use
            if wal.id_from == 0 && wal.id_until == 0 {
                break;
            }
            if wal.data_start.is_none() || wal.data_end.is_none() {
                return Err(Error::Integrity(
                    "Invalid `data_start` / `data_end` values for WAL with data".into(),
                ));
            }
            let data_start = wal.data_start.unwrap();
            let data_end = wal.data_end.unwrap();

            if data_start < wal.offset_logs() as u32 {
                return Err(Error::Integrity(
                    "`data_start` cannot be smaller than header offset".into(),
                ));
            }
            // head for record: id + crc + data_len
            if data_start == data_end || data_start + 8 + 4 + 4 > data_end {
                return Err(Error::Integrity(
                    "`data_start` does not match `data_end` - data cannot fit".into(),
                ));
            }
            if until + 1 != wal.id_from {
                return Err(Error::Integrity(
                    format!("Missing logs between IDs {} and {}", until + 1, wal.id_from).into(),
                ));
            }

            wal_no = wal.wal_no;
            until = wal.id_until;
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

    static PATH: &str = "test_data";
    static MB2: u32 = 2 * 1024 * 1024;

    #[test]
    fn append_read_logs() -> Result<(), Error> {
        let base_path = format!("{}/append_logs", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;
        let mut buf = Vec::with_capacity(32);

        let mut wal = WalFile::new(1, &base_path, 0, 0, MB2).unwrap();
        wal.create_file(&mut buf)?;
        wal.mmap_mut()?;

        let d1 = b"Hello World".as_slice();
        let d2 = b"I am Batman!".as_slice();
        let d3 = b"... and not the Joker!".as_slice();
        println!("length d1: {}", d1.len());
        println!("length d2: {}", d2.len());
        println!("length d3: {}", d3.len());

        assert!(wal.data_start.is_none());
        assert!(wal.data_end.is_none());

        assert!(wal.has_space(d1.len()));
        buf.clear();
        wal.append_log(1, d1, &mut buf)?;
        let start = wal.data_start.unwrap();
        assert_eq!(start, wal.offset_logs() as u32);
        let end = start + 8 + 4 + 4 + d1.len() as u32;
        assert_eq!(wal.data_end.unwrap(), end);
        assert_eq!(wal.id_from, 1);
        assert_eq!(wal.id_until, 1);

        assert!(wal.has_space(d2.len()));
        buf.clear();
        wal.append_log(2, d2, &mut buf)?;
        assert_eq!(wal.data_start.unwrap(), start);
        let end = end + 1 + 8 + 4 + 4 + d2.len() as u32;
        assert_eq!(wal.data_end.unwrap(), end);
        assert_eq!(wal.id_from, 1);
        assert_eq!(wal.id_until, 2);

        assert!(wal.has_space(d3.len()));
        buf.clear();
        wal.append_log(3, d3, &mut buf)?;
        assert_eq!(wal.data_start.unwrap(), start);
        let end = end + 1 + 8 + 4 + 4 + d3.len() as u32;
        assert_eq!(wal.data_end.unwrap(), end);
        assert_eq!(wal.id_from, 1);
        assert_eq!(wal.id_until, 3);

        buf.clear();
        wal.update_header(&mut buf)?;
        wal.flush()?;

        // make sure we can successfully read it
        let mut wal_disk = WalFile::read_from_file(wal.path.clone())?;
        wal_disk.mmap()?;
        assert_eq!(wal_disk.data_start.unwrap(), start);
        assert_eq!(wal_disk.data_end.unwrap(), end);
        assert_eq!(wal_disk.id_from, 1);
        assert_eq!(wal_disk.id_until, 3);

        let mut logs = Vec::with_capacity(3);
        buf.clear();
        wal_disk.read_logs(1, 3, &mut logs)?;
        assert_eq!(logs.len(), 3);

        let (id, data) = logs.get(0).unwrap();
        assert_eq!(id, &1);
        assert_eq!(data, d1);
        let (id, data) = logs.get(1).unwrap();
        assert_eq!(id, &2);
        assert_eq!(data, d2);
        let (id, data) = logs.get(2).unwrap();
        assert_eq!(id, &3);
        assert_eq!(data, d3);

        buf.clear();
        logs.clear();
        wal_disk.read_logs(2, 3, &mut logs)?;
        assert_eq!(logs.len(), 2);
        let (id, data) = logs.get(0).unwrap();
        assert_eq!(id, &2);
        assert_eq!(data, d2);
        let (id, data) = logs.get(1).unwrap();
        assert_eq!(id, &3);
        assert_eq!(data, d3);

        buf.clear();
        logs.clear();
        wal_disk.read_logs(2, 2, &mut logs)?;
        assert_eq!(logs.len(), 1);
        let (id, data) = logs.get(0).unwrap();
        assert_eq!(id, &2);
        assert_eq!(data, d2);

        buf.clear();
        logs.clear();
        assert!(wal_disk.read_logs(1, 4, &mut logs).is_err());
        assert!(wal_disk.read_logs(0, 2, &mut logs).is_err());

        Ok(())
    }

    #[test]
    fn convert_wal_header() -> Result<(), Error> {
        let base_path = format!("{}/convert_wal_header", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        // make sure we are cleaned up
        let path_with_no = format!("{base_path}/0000000000000001.wal");
        let _ = fs::remove_file(&path_with_no);

        let mut wal = WalFile::new(1, &base_path, 23, 1337, MB2).unwrap();
        assert!(wal.data_start.is_none());
        assert!(wal.data_end.is_none());
        let mut buf = Vec::with_capacity(28);
        wal.create_file(&mut buf)?;
        assert!(wal.data_start.is_none());
        assert!(wal.data_end.is_none());

        let wal_disk = WalFile::read_from_file(path_with_no.clone())?;
        assert_eq!(wal.version, wal_disk.version);
        assert_eq!(wal.path, wal_disk.path);
        assert_eq!(wal.wal_no, wal_disk.wal_no);
        assert_eq!(wal.id_from, wal_disk.id_from);
        assert_eq!(wal.id_until, wal_disk.id_until);
        assert_eq!(wal.data_start, wal_disk.data_start);
        assert_eq!(wal.data_end, wal_disk.data_end);
        assert_eq!(wal.len_max, wal_disk.len_max);

        let path_h1 = format!("{}/0000000000000001.wal", base_path);
        let path_h2 = format!("{}/0000000000000002.wal", base_path);
        let _ = fs::remove_file(&path_with_no);
        let _ = fs::remove_file(&path_h1);
        let _ = fs::remove_file(&path_h2);

        let mut set = WalFileSet::new(base_path);
        buf.clear();
        set.add_file(MB2, &mut buf).unwrap();
        assert_eq!(fs::exists(&path_h1)?, true);
        assert_eq!(fs::exists(&path_h2)?, false);

        buf.clear();
        set.add_file(MB2, &mut buf).unwrap();
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
            active: Some(2),
            base_path: base_path.clone(),
            files: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
            ],
        };
        set.check_integrity(&meta).unwrap();
        assert_eq!(set.active().wal_no, 3);

        let mut buf = Vec::with_capacity(28);
        set.add_file(MB2, &mut buf)?;
        set.check_integrity(&meta).unwrap();
        assert_eq!(set.active().wal_no, 3);

        // invalid data_start / data_end
        let set = WalFileSet {
            active: Some(1),
            base_path: base_path.clone(),
            files: vec![WalFile {
                version: 1,
                wal_no: 1,
                path: "".to_string(),
                id_from: 1,
                id_until: 10,
                data_start: Some(32),
                data_end: None,
                len_max: MB2,
                mmap: None,
                mmap_mut: None,
            }],
        };
        assert!(set.check_integrity(&meta).is_err());
        // invalid data_start / data_end
        let set = WalFileSet {
            active: Some(1),
            base_path: base_path.clone(),
            files: vec![WalFile {
                version: 1,
                wal_no: 1,
                path: "".to_string(),
                id_from: 1,
                id_until: 10,
                data_start: Some(32),
                data_end: Some(32),
                len_max: MB2,
                mmap: None,
                mmap_mut: None,
            }],
        };
        assert!(set.check_integrity(&meta).is_err());
        // invalid data_start / data_end
        let set = WalFileSet {
            active: Some(1),
            base_path: base_path.clone(),
            files: vec![WalFile {
                version: 1,
                wal_no: 1,
                path: "".to_string(),
                id_from: 1,
                id_until: 10,
                data_start: Some(32),
                data_end: Some(40),
                len_max: MB2,
                mmap: None,
                mmap_mut: None,
            }],
        };
        assert!(set.check_integrity(&meta).is_err());
        // invalid data_start / data_end
        let set = WalFileSet {
            active: Some(1),
            base_path: base_path.clone(),
            files: vec![WalFile {
                version: 1,
                wal_no: 1,
                path: "".to_string(),
                id_from: 1,
                id_until: 10,
                // make integrity check work
                data_start: None,
                data_end: Some(32),
                len_max: MB2,
                mmap: None,
                mmap_mut: None,
            }],
        };
        assert!(set.check_integrity(&meta).is_err());

        // missing logs - invalid if_until -> next id_from
        let set = WalFileSet {
            active: Some(1),
            base_path: base_path.clone(),
            files: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 4,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 2,
                    id_until: 10,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 33,
                    data_start: None,
                    data_end: None,
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 18,
                    id_until: 32,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        let set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files: vec![
                WalFile {
                    version: 1,
                    wal_no: 1,
                    path: "".to_string(),
                    id_from: 1,
                    id_until: 10,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 2,
                    path: "".to_string(),
                    id_from: 11,
                    id_until: 17,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
                WalFile {
                    version: 1,
                    wal_no: 3,
                    path: "".to_string(),
                    id_from: 19,
                    id_until: 33,
                    // make integrity check work
                    data_start: Some(32),
                    data_end: Some(64),
                    len_max: MB2,
                    mmap: None,
                    mmap_mut: None,
                },
            ],
        };
        assert!(set.check_integrity(&meta).is_err());

        Ok(())
    }
}
