use crate::error::Error;
use crate::utils::{bin_to_u32, bin_to_u64, crc, u32_to_bin, u64_to_bin};
use memmap2::{Advice, Mmap, MmapMut, MmapOptions};
use std::collections::VecDeque;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use tracing::{debug, info, warn};

static MAGIC_NO_WAL: &[u8] = b"HQL_WAL";
static MIN_WAL_SIZE: u32 = 8 * 1024;

#[derive(Debug)]
pub struct WalRecord<'a> {
    log_id: u64,
    crc: &'a [u8],
    data: &'a [u8],
}

impl WalRecord<'_> {
    #[inline]
    fn len(&self) -> u32 {
        debug_assert!(self.data.len() < u32::MAX as usize);
        // - 8 byte id
        // - 4 byte crc (u32)
        // - 4 byte data length
        // - variable length data
        8 + 4 + 4 + self.data.len() as u32
    }
}

#[derive(Debug)]
pub struct WalFile {
    pub version: u8,
    pub wal_no: u64,
    pub path: String,
    pub id_from: u64,
    pub id_until: u64,
    pub data_start: Option<u32>,
    pub data_end: Option<u32>,
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
    /// Does NOT check the file header's integrity. This is done inside a `WalFileSet`.
    ///
    /// Iterates over the complete data and makes sure, that the expected start and end log IDs
    /// match the existing data set, as well as that any data after the last expected ID is null.
    /// Basically, it makes sure that the information written inside the header matches the actual
    /// data.
    ///
    /// This check can be quite expensive and only needs to be done for the last existing file in a
    /// `WalFileSet`. If the last one is fine, the ones before can only be as well because of their
    /// immutability. CRC checks are done when reading a single log each time anyway.
    ///
    /// If unexpected `WalRecord`s are found and they are valid, the header will be "repaired".
    ///
    /// Note: `self` must have an active `mmap_mut` to be able to execute this check properly.
    #[tracing::instrument(skip_all)]
    fn check_repair_data_integrity(&mut self, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(self.mmap_mut.is_some());
        debug_assert!(self.id_from <= self.id_until);
        debug_assert_eq!(self.data_start.is_some(), self.data_end.is_some());

        info!(
            "Starting detailed WalFile integrity check for {}",
            self.path
        );

        // This check is done in 2 phases:
        // 1. Iterate over all logs and make sure start and end are correct and valid
        // 2. After reaching the end, make sure everything afterward is null and there are no
        //    orphaned logs that might be missing in the header information.

        let mut offset = self.offset_logs() as u32 + 1;
        let mut id_before: Option<u64> = None;

        if let Some(start) = self.data_start {
            offset = start;
            let data_end = self.data_end.unwrap();

            loop {
                let record = self.read_record_unchecked(offset)?;

                if let Some(id_before) = id_before {
                    if record.log_id != id_before + 1 {
                        return Err(Error::Integrity(
                            format!(
                                "WAL record incorrect ordering, missing Log ID {}",
                                id_before + 1
                            )
                            .into(),
                        ));
                    }
                }
                id_before = Some(record.log_id);

                if record.crc != crc!(record.data) {
                    // TODO with `auto-heal` enabled, we could remove everything afterward
                    //  and re-sync from another node in this case
                    return Err(Error::Integrity("Invalid CRC for WAL Record".into()));
                }

                // add 1 because the `len` is inclusive and points at the last byte of data
                offset += record.len() + 1;
                if offset > data_end || record.log_id == self.id_until {
                    debug!("Reached data_end in data integrity check");
                    break;
                }
            }
        }

        // At this point, the `offset` should point to only NULL data.
        // If this is not the case, it means there is a mismatch in the actual data in the file
        // and the saved `id_until` / `data_end` information in the header. This could only happen
        // during a force-killed application or similar situation.
        loop {
            if offset >= self.len_max + 8 + 4 + 4 {
                debug!("Reached the end of the WAL file");
                break;
            }

            // This should always work, but everything should be null.
            if let Ok(record) = self.read_record_unchecked(offset) {
                debug_assert_eq!(record.log_id, 0);
                debug_assert_eq!(record.data.len(), 0);

                if record.log_id > 0 {
                    // We found unexpected data. In case of any errors, we will ignore this
                    // unexpected data and let the next append logs action overwrite it.

                    if let Some(id_before) = id_before {
                        if record.log_id != id_before + 1 {
                            warn!(
                                "Mismatch in log IDs in unexpected data section - ignoring entry"
                            );
                            break;
                        }
                    }
                    id_before = Some(record.log_id);

                    if record.crc != crc!(record.data) {
                        warn!("Mismatch in CRC in unexpected data section - ignoring entry");
                        break;
                    }

                    warn!(
                        "Found unexpected, valid WAL record with Log ID {}",
                        record.log_id
                    );

                    let log_id = record.log_id;
                    let record_len = record.len();
                    // `record_len` is inclusive
                    offset += record_len + 1;

                    self.id_until = log_id;
                    if self.data_start.is_none() {
                        self.data_start = Some(offset);
                    }
                    self.data_end = Some(offset + record_len);
                    self.update_header(buf)?;

                    if offset >= self.len_max {
                        debug!("Reached end of file in unexpected data section");
                        break;
                    }
                } else {
                    debug!("No unexpected WAL records found");
                    break;
                }
            }
        }

        Ok(())
    }

    #[inline]
    pub fn has_space(&self, data_len: u32) -> bool {
        // wal entries will have:
        // - 1 byte offset -> `data_end` is inclusive
        // - 8 byte id
        // - 4 byte crc
        // - 4 byte data length
        // - variable length data
        self.len_max
            > self.data_end.unwrap_or_else(|| self.offset_logs() as u32) + 1 + 8 + 4 + 4 + data_len
    }

    #[inline]
    pub fn space_left(&self) -> u32 {
        self.len_max - self.data_end.unwrap_or_else(|| self.offset_logs() as u32)
    }

    /// Expects to have enough space left -> check MUST be done upfront
    #[inline]
    #[tracing::instrument(skip_all)]
    pub fn append_log(&mut self, id: u64, data: &[u8], buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(self.mmap_mut.is_some());
        debug_assert!(data.len() <= u32::MAX as usize);
        debug_assert!(self.has_space(data.len() as u32));
        debug_assert_eq!(self.data_start.is_some(), self.data_end.is_some());

        let start = if self.data_start.is_none() {
            self.id_from = id;
            let start = self.offset_logs();
            self.data_start = Some(start as u32);
            start
        } else {
            self.data_end.unwrap() as usize + 1
        };
        self.id_until = id;

        let mmap = self.mmap_mut.as_mut().unwrap();
        u64_to_bin(id, buf)?;
        (&mut mmap[start..]).write_all(buf)?;

        let crc = crc!(&data);
        (&mut mmap[start + 8..]).write_all(crc.as_slice())?;

        let len = data.len() as u32;
        buf.clear();
        u32_to_bin(len, buf)?;
        (&mut mmap[start + 8 + 4..]).write_all(buf)?;

        (&mut mmap[start + 8 + 4 + 4..]).write_all(data)?;

        self.data_end = Some((start + 8 + 4 + 4 + data.len()) as u32);
        debug_assert!(self.data_end.unwrap() <= self.len_max);

        Ok(())
    }

    #[inline]
    pub fn clone_no_mmap(&self) -> Self {
        Self {
            version: self.version,
            wal_no: self.wal_no,
            path: self.path.clone(),
            id_from: self.id_from,
            id_until: self.id_until,
            data_start: self.data_start,
            data_end: self.data_end,
            len_max: self.len_max,
            mmap: None,
            mmap_mut: None,
        }
    }

    #[inline]
    pub fn clone_from_no_mmap(&mut self, other: &Self) {
        debug_assert_eq!(self.path, other.path);
        self.id_from = other.id_from;
        self.id_until = other.id_until;
        self.data_start = other.data_start;
        self.data_end = other.data_end;
    }

    /// Reads the logs into the given buffer. Returns `Ok(offset)` of the first log.
    #[inline]
    #[tracing::instrument(skip_all)]
    pub fn read_logs(
        &self,
        id_from: u64,
        id_until: u64,
        buf: &mut Vec<(u64, Vec<u8>)>,
    ) -> Result<u32, Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(id_until >= id_from);
        debug_assert!(self.data_start.is_some());
        debug_assert!(self.data_end.is_some());

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

        let data_start = self.data_start.unwrap();
        let data_end = self.data_end.unwrap();

        let mut idx = data_start;
        let mut offset = 0;
        loop {
            let record = self.read_record_unchecked(idx)?;

            if record.log_id == id_from {
                offset = idx;
            }
            if record.log_id >= id_from {
                if record.log_id <= id_until {
                    debug_assert!(idx + record.len() <= data_end);
                    if record.crc != crc!(record.data) {
                        return Err(Error::Integrity("Invalid CRC for WAL Record".into()));
                    }
                    buf.push((record.log_id, record.data.to_vec()))
                } else {
                    break;
                }
            }
            if record.log_id >= id_until {
                break;
            }

            // add 1 because the `len` is inclusive and points at the last byte of data
            idx += record.len() + 1;
            debug_assert!(idx - 1 <= data_end);
        }

        debug_assert_eq!(
            id_until + 1 - id_from,
            buf.len() as u64,
            "expected id_from {id_from} until {id_until}: {:?}",
            self
        );

        Ok(offset)
    }

    /// Reads a record at the given `offset`. Does NOT do any boundary checking or any other
    /// validation. Only extracts the data itself.
    #[inline(always)]
    fn read_record_unchecked(&self, offset: u32) -> Result<WalRecord, Error> {
        debug_assert!(offset + 8 + 4 + 4 < self.len_max);

        // id, crc, length
        let head = self.read_bytes(offset, offset + 8 + 4 + 4)?;

        let log_id = bin_to_u64(&head[..8])?;
        let crc = &head[8..12];

        let data_len = bin_to_u32(&head[12..16])?;
        let data_from = offset + 8 + 4 + 4;
        let data = self.read_bytes(data_from, data_from + data_len)?;

        Ok(WalRecord { log_id, crc, data })
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
        if self.mmap.is_some() {
            return Ok(());
        }

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
    pub fn mmap_drop(&mut self) {
        self.mmap = None;
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
        // mmap.advise(Advice::Sequential)?;

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
    #[tracing::instrument(skip_all)]
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
    #[tracing::instrument(skip_all)]
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
        let id_from = bin_to_u64(&buf[8..16])?;
        let id_until = bin_to_u64(&buf[16..24])?;

        let data_start = bin_to_u32(&buf[24..28])?;
        let data_end = bin_to_u32(&buf[28..32])?;

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
                u64_to_bin(self.id_from, buf)?;
                u64_to_bin(self.id_until, buf)?;
                u32_to_bin(self.data_start.unwrap_or(0), buf)?;
                u32_to_bin(self.data_end.unwrap_or(0), buf)?;
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
    pub files: VecDeque<WalFile>,
}

impl WalFileSet {
    #[inline]
    pub fn active(&mut self) -> &mut WalFile {
        debug_assert!(self.active.is_some());
        debug_assert!(
            self.files.len() > self.active.unwrap(),
            "self.files.len(): {}, self.active: {}",
            self.files.len(),
            self.active.unwrap()
        );
        self.files.get_mut(self.active.unwrap()).unwrap()
    }

    /// Adds a new `Header` at the end and creates a file for it.
    /// If `self.files` was empty before, `self.active` will be set to `0` and left untouched
    /// otherwise. If files existed already, `self.roll_over()` will handle `active` switching.
    #[tracing::instrument(skip_all)]
    #[inline]
    pub fn add_file(&mut self, wal_size: u32, buf: &mut Vec<u8>) -> Result<&WalFile, Error> {
        let wal_no = self.files.back().map(|w| w.wal_no + 1).unwrap_or(1);
        let mut wal = WalFile::new(wal_no, &self.base_path, 0, 0, wal_size)?;
        wal.create_file(buf)?;
        self.files.push_back(wal);

        if wal_no == 1 {
            self.active = Some(0);
        }

        Ok(self.files.back().unwrap())
    }

    /// Checks the integrity of the Headers and makes sure the order is strictly ascending and
    /// there are no missing log IDs.
    #[tracing::instrument(skip_all)]
    pub fn check_integrity(
        &mut self,
        buf: &mut Vec<u8>,
        is_clean_start: bool,
    ) -> Result<(), Error> {
        if self.files.is_empty() {
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
            if wal.data_start.is_some() != wal.data_end.is_some() {
                return Err(Error::Integrity(
                    "Invalid `data_start` / `data_end` values for WAL with data".into(),
                ));
            }
            if let Some(data_start) = wal.data_start {
                debug_assert!(wal.data_end.is_some());
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
            }

            if until + 1 != wal.id_from {
                return Err(Error::Integrity(
                    format!("Missing logs between IDs {} and {}", until + 1, wal.id_from).into(),
                ));
            }

            wal_no = wal.wal_no;
            until = wal.id_until;
        }

        // We only need to do the way more expensive check if the startup is not clean, e.g.
        // when an existing `LockFile` has been ignored. This check is unnecessary after a
        // graceful shutdown.
        if !is_clean_start {
            let active = self.active();
            if active.mmap_mut.is_none() {
                active.mmap_mut()?;
            }
            active.check_repair_data_integrity(buf)?;
        }

        Ok(())
    }

    /// Creates a clone of the `WalFileSet` without any active `mmap`s.
    #[inline]
    pub fn clone_no_map(&self) -> Self {
        let mut slf = Self {
            active: self.active,
            base_path: self.base_path.clone(),
            files: VecDeque::with_capacity(self.files.len()),
        };
        for file in &self.files {
            slf.files.push_back(file.clone_no_mmap());
        }
        slf
    }

    #[inline]
    pub fn clone_files_from_no_mmap(&mut self, other: &VecDeque<WalFile>) {
        // in case old WAL files have been cleaned up
        self.files
            .retain(|f| other.iter().any(|upd| upd.wal_no == f.wal_no));

        self.files.iter_mut().enumerate().for_each(|(i, f)| {
            // this has top work, the files MUST always be in order and we have cleanup up
            // old files in the step before
            let upd = &other[i];
            debug_assert_eq!(f.wal_no, upd.wal_no);
            f.id_from = upd.id_from;
            f.id_until = upd.id_until;
            f.data_start = upd.data_start;
            f.data_end = upd.data_end;
        });

        // the last case is that `other` contains new files because of log roll-overs
        for upd in other.iter().skip(self.files.len()) {
            self.files.push_back(upd.clone_no_mmap());
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn read(base_path: String, wal_size: u32) -> Result<WalFileSet, Error> {
        let mut file_names = Vec::with_capacity(2);
        for entry in fs::read_dir(&base_path)? {
            let entry = entry?.file_name();
            let fname = entry.to_str().unwrap_or_default();
            if fname.ends_with(".wal") {
                file_names.push(fname.to_string());
            }
        }
        file_names.sort();

        let mut files = VecDeque::with_capacity(file_names.len());
        for name in file_names {
            let path_full = format!("{}/{}", base_path, name);
            if let Ok(wal) = WalFile::read_from_file(path_full) {
                files.push_back(wal);
            }
        }

        let active = if files.is_empty() {
            let mut wal = WalFile::new(1, &base_path, 0, 0, wal_size)?;
            let mut buf = Vec::with_capacity(28);
            wal.create_file(&mut buf)?;
            files.push_back(wal);
            0
        } else {
            files.len() - 1
        };

        Ok(Self {
            active: Some(active),
            base_path,
            files,
        })
    }

    /// Rolls a new WAL file and "closes" the current one. Removes any memory-mapping and flushes
    /// the current file to disk. Creates a new WAL with `mmap_mut`
    #[tracing::instrument(skip_all)]
    #[inline]
    pub fn roll_over(&mut self, wal_size: u32, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(!self.files.is_empty());
        debug_assert!(self.files.back().unwrap().mmap_mut.is_some());

        let last_id = {
            let active = self.active();
            active.update_header(buf)?;
            active.flush_async()?;
            active.mmap_mut = None;
            active.id_until
        };

        buf.clear();
        self.add_file(wal_size, buf)?;
        self.active = Some(self.files.len() - 1);

        let active = self.active();
        active.mmap_mut()?;
        active.id_from = last_id + 1;
        active.id_until = last_id + 1;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    #[inline]
    pub fn shift_delete_logs_until(
        &mut self,
        id_until: u64,
        wal_size: u32,
        buf: &mut Vec<u8>,
        buf_logs: &mut Vec<(u64, Vec<u8>)>,
    ) -> Result<(), Error> {
        while !self.files.is_empty() && self.files.front().unwrap().id_until < id_until {
            let file = self.files.pop_front().unwrap();
            fs::remove_file(&file.path)?;
        }
        if self.files.is_empty() {
            self.add_file(wal_size, buf)?;
        }

        let first = self.files.get_mut(0).unwrap();
        debug_assert!(first.id_until >= id_until);
        if first.id_from < id_until {
            first.id_from = id_until;
            // find the offset for `id_until` -> via in-memory index in the future?
            if first.mmap_mut.is_none() {
                first.mmap_mut()?;
            }
            debug_assert!(buf_logs.is_empty());
            let offset = first.read_logs(id_until, id_until, buf_logs)?;
            first.id_from = id_until;
            first.data_start = Some(offset);
        }
        self.active = Some(self.files.len() - 1);

        Ok(())
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

        assert!(wal.data_start.is_none());
        assert!(wal.data_end.is_none());

        assert!(wal.has_space(d1.len() as u32));
        buf.clear();
        wal.append_log(1, d1, &mut buf)?;
        let start = wal.data_start.unwrap();
        assert_eq!(start, wal.offset_logs() as u32);
        let end = start + 8 + 4 + 4 + d1.len() as u32;
        assert_eq!(wal.data_end.unwrap(), end);
        assert_eq!(wal.id_from, 1);
        assert_eq!(wal.id_until, 1);

        assert!(wal.has_space(d2.len() as u32));
        buf.clear();
        wal.append_log(2, d2, &mut buf)?;
        assert_eq!(wal.data_start.unwrap(), start);
        let end = end + 1 + 8 + 4 + 4 + d2.len() as u32;
        assert_eq!(wal.data_end.unwrap(), end);
        assert_eq!(wal.id_from, 1);
        assert_eq!(wal.id_until, 2);

        assert!(wal.has_space(d3.len() as u32));
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

        let mut set = WalFileSet {
            active: None,
            base_path,
            files: Default::default(),
        };
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
        let mut buf = Vec::new();
        let base_path = format!("{}/integrity_check", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
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
        });
        files.push_back(WalFile {
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
        });
        files.push_back(WalFile {
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
        });
        let mut set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files,
        };
        set.check_integrity(&mut buf, true).unwrap();
        assert_eq!(set.active().wal_no, 3);

        let mut buf = Vec::with_capacity(28);
        set.add_file(MB2, &mut buf)?;
        set.check_integrity(&mut buf, true).unwrap();
        assert_eq!(set.active().wal_no, 3);

        // missing logs - invalid if_until -> next id_from
        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
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
        });
        files.push_back(WalFile {
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
        });
        let mut set = WalFileSet {
            active: Some(1),
            base_path: base_path.clone(),
            files,
        };
        assert!(set.check_integrity(&mut buf, true).is_err());

        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
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
        });
        files.push_back(WalFile {
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
        });
        files.push_back(WalFile {
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
        });
        let mut set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files,
        };
        assert!(set.check_integrity(&mut buf, true).is_err());

        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
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
        });
        files.push_back(WalFile {
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
        });
        files.push_back(WalFile {
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
        });
        let mut set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files,
        };
        assert!(set.check_integrity(&mut buf, true).is_err());

        Ok(())
    }

    #[test]
    fn roll_over_shift_delete() -> Result<(), Error> {
        let base_path = format!("{}/roll_over_shift_delete", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;
        let mut buf = Vec::with_capacity(8);

        let mut wal = WalFileSet {
            active: None,
            base_path,
            files: Default::default(),
        };
        wal.add_file(MB2, &mut buf)?;
        wal.active().mmap_mut()?;

        let d1 = b"Hello World".as_slice();
        let d2 = b"I am Batman!".as_slice();
        let d3 = b"... and not the Joker!".as_slice();
        let d4 = b"I like Harley Quinn".as_slice();
        let d5 = b"... a lot".as_slice();

        buf.clear();
        wal.active().append_log(1, d1, &mut buf)?;
        buf.clear();
        wal.active().append_log(2, d2, &mut buf)?;
        assert_eq!(wal.files.len(), 1);
        assert_eq!(wal.active().wal_no, 1);

        buf.clear();
        wal.roll_over(MB2, &mut buf)?;
        assert_eq!(wal.files.len(), 2);
        // active file should be shifted, old mmap be removed and new active have an mmap_mut
        assert_eq!(wal.active().wal_no, 2);
        assert!(wal.files.front().unwrap().mmap_mut.is_none());
        assert!(wal.active().mmap_mut.is_some());

        buf.clear();
        wal.active().append_log(3, d3, &mut buf)?;
        buf.clear();
        wal.active().append_log(4, d4, &mut buf)?;
        buf.clear();
        wal.active().append_log(5, d5, &mut buf)?;

        let mut buf_logs = Vec::with_capacity(1);
        buf.clear();
        // this should be a noop
        wal.shift_delete_logs_until(1, MB2, &mut buf, &mut buf_logs)?;
        let front = wal.files.front().unwrap();
        assert_eq!(front.id_from, 1);
        assert_eq!(front.data_start.unwrap() as usize, front.offset_logs());

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs_until(2, MB2, &mut buf, &mut buf_logs)?;
        let front = wal.files.front().unwrap();
        assert_eq!(front.id_from, 2);
        let start = 8 + 4 + 4 + d1.len() + front.offset_logs() + 1;
        assert_eq!(front.data_start.unwrap() as usize, start);

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs_until(3, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.len(), 1);
        let front = wal.files.front().unwrap();
        assert_eq!(front.id_from, 3);
        assert_eq!(front.data_start.unwrap() as usize, front.offset_logs());

        Ok(())
    }
}
