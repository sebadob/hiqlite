use crate::error::Error;
use crate::reader::LogReadMemo;
use crate::utils::{bin_to_u32, bin_to_u64, crc, u32_to_bin, u64_to_bin};
use memmap2::{Mmap, MmapMut, MmapOptions};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::{env, fs};
use tracing::{debug, info, warn};

static MAGIC_NO_WAL: &[u8] = b"HQL_WAL";
static MIN_WAL_SIZE: u32 = 8 * 1024;

/// Header version written to every newly created WAL file. Files already on disk keep their own
/// version, which is read from their header, so a directory mixes versions freely.
const WAL_VERSION: u8 = 2;
/// Header length of version 1: magic, version, `id_from`, `id_until`, `data_start`, `data_end`.
const HEADER_V1: usize = 7 + 1 + 8 + 8 + 4 + 4;
/// Header length of version 2: version 1 plus the fixed-size `committed` slot.
const HEADER_V2: usize = HEADER_V1 + 1 + COMMITTED_SLOT;
/// Bytes a serialized `committed` log id may occupy; one more byte in front holds its length.
const COMMITTED_SLOT: usize = 39;
/// Bytes a single WAL record adds around its payload: the gap byte in front of it, the 8 byte log
/// id, the 4 byte CRC, the 4 byte data length, and one more because `data_end` is inclusive.
const RECORD_OVERHEAD: usize = 1 + 8 + 4 + 4 + 1;

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
    /// The last `committed` log id published into this file's header, serialized. Version 2
    /// headers only; `None` for version 1.
    pub committed: Option<Vec<u8>>,
    len_max: u32,
    mmap: Option<Mmap>,
    mmap_mut: Option<MmapMut>,
    // Kept while `mmap_mut` exists: `sync_file_range` in `flush_async` needs an fd,
    // which the mmap alone does not provide.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    file: Option<File>,
}

impl Drop for WalFile {
    fn drop(&mut self) {
        if self.mmap_mut.is_some() {
            let mut buf = Vec::with_capacity(HEADER_V2);
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
    /// This check is expensive. `check_integrity` runs it over every file on an unclean start,
    /// because a persisted `committed` pointer makes openraft re-apply a range that can reach any
    /// of them; after a clean shutdown the newest file alone is enough, because only it can hold
    /// data that never reached the fsync.
    ///
    /// If unexpected `WalRecord`s are found and they are valid, the header will be "repaired".
    ///
    /// Note: `self` must have an active `mmap_mut` to be able to execute this check properly.
    #[tracing::instrument(skip_all)]
    fn check_repair_data_integrity(&mut self, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
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

                if let Some(id_before) = id_before
                    && record.log_id != id_before + 1
                {
                    return Err(Error::Integrity(
                        format!(
                            "WAL record incorrect ordering, missing Log ID {}",
                            id_before + 1
                        )
                        .into(),
                    ));
                }
                if record.crc != crc!(record.data) || record.data.is_empty() {
                    #[cfg(feature = "auto-heal")]
                    {
                        // If we get here, we want to update the header in a way that all logs
                        // beginning with this one are considered lost and should therefore be
                        // re-synced from another cluster member. This will only work, if
                        // `auto-heal` is enabled for the main `hiqlite` crate as well.

                        if let Some(id) = id_before {
                            warn!(
                                "Bad CRC CHKSUM for WAL ID {}. Trying automatic healing. \
                                Rolling back latest log ID to {}.",
                                record.log_id, id
                            );
                            self.id_until = id;
                            // `data_end` is inclusive
                            self.data_end = Some(offset - 1);
                        } else {
                            warn!(
                                "Bad CRC CHKSUM for WAL ID {}. Trying automatic healing. \
                                The first ID in this log is bad - resetting data block.",
                                record.log_id
                            );
                            self.data_start = None;
                            self.data_end = None;
                            self.id_until = self.id_from;
                        }

                        // We don't need to do anything with the already existing data from now on.
                        // It will be automatically overwritten by `data_end` index with upcoming
                        // log appends.
                        // The only thing we want to do, just to be safe, is to overwrite the
                        // 8 byte log id, so this check never needs to be done again, as we would
                        // get NULL for the whole record in that case.
                        let mmap = self.mmap_mut.as_mut().unwrap();
                        u64_to_bin(0, buf)?;
                        let idx = offset as usize;
                        (&mut mmap[idx..idx + 8]).write_all(buf)?;

                        return Ok(());
                    }

                    #[cfg(not(feature = "auto-heal"))]
                    return Err(Error::Integrity("Invalid CRC for WAL Record".into()));
                }
                id_before = Some(record.log_id);

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
        let mut recovered = 0;
        loop {
            // make sure a full record header fits at this offset - use u64 so that a large
            // `offset` cannot wrap around in u32 arithmetic and pass this check
            if offset as u64 + 8 + 4 + 4 > self.len_max as u64 {
                debug!("Reached the end of the WAL file");
                break;
            }

            // This will fail if only null data is read and the `data_len` is 0
            if let Ok(record) = self.read_record_unchecked(offset) {
                if record.log_id > 0 {
                    // We found unexpected data. In case of any errors, we will ignore this
                    // unexpected data and let the next append logs action overwrite it.

                    if let Some(id_before) = id_before
                        && record.log_id != id_before + 1
                    {
                        warn!(
                            "Mismatch in Log ID in unexpected data for Log ID {} after already \
                                recovered {} logs - ignoring entry\nexpected Log ID: {} / found: {}\
                                \nread offset: {}\n{:?}",
                            record.log_id,
                            recovered,
                            id_before + 1,
                            record.log_id,
                            offset,
                            record
                        );
                        break;
                    }
                    id_before = Some(record.log_id);

                    if record.crc != crc!(record.data) {
                        warn!(
                            "Mismatch in CRC in unexpected data section after already recovered {} \
                            logs - ignoring entry with log id: {}, expected crc: {:?}, \
                            actual crc: {:?} - \nread offset: {} / {:?}",
                            recovered,
                            record.log_id,
                            record.crc,
                            crc!(record.data),
                            offset,
                            self
                        );
                        break;
                    }

                    warn!(
                        "Found unexpected, valid WAL record with Log ID {}",
                        record.log_id
                    );

                    let record_len = record.len();
                    self.id_until = record.log_id;
                    if self.data_start.is_none() {
                        self.data_start = Some(offset);
                    }
                    self.data_end = Some(offset + record_len);
                    buf.clear();
                    self.update_header(buf)?;
                    recovered += 1;

                    // `record_len` is inclusive
                    offset += record_len + 1;
                    if offset >= self.len_max {
                        debug!("Reached end of file in unexpected data section");
                        break;
                    }
                } else {
                    info!("No unexpected additional WAL records found");
                    break;
                }
            } else {
                info!("No unexpected additional data found in {}", self.path);
                break;
            }
        }

        if recovered > 0 {
            info!(
                "Successfully recovered {} orphaned WAL file records: {:?}",
                recovered, self
            );
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
        // use u64 so a corrupt or large `data_end` cannot wrap around in u32 arithmetic and
        // report free space that does not exist
        let next = self.data_end.unwrap_or_else(|| self.offset_logs() as u32) as u64;
        (self.len_max as u64) > next + 1 + 8 + 4 + 4 + data_len as u64
    }

    #[inline]
    pub fn space_left(&self) -> u32 {
        self.len_max
            .saturating_sub(self.data_end.unwrap_or_else(|| self.offset_logs() as u32))
    }

    /// The largest single log payload the file that receives it can ever hold, independent of how
    /// full that file already is.
    ///
    /// A single entry cannot span WAL files, so this is the hard cap for one entry and it has to be
    /// exact: an entry that passes it must fit into an empty file of this header version, because
    /// the writer rolls over and appends without a second space check. The cap matches `has_space`
    /// on an empty file, and it follows the receiving file's header on purpose - a log directory
    /// mixes header versions, and a cap taken from an older file would let an entry overflow a
    /// newer one.
    #[inline]
    pub fn max_data_len(&self, wal_size: u32) -> usize {
        // `len_max` is the size the file actually has; a config change can make it differ from
        // `wal_size`, and the file is what the entry has to fit into.
        (wal_size as usize)
            .min(self.len_max as usize)
            .saturating_sub(self.offset_logs())
            .saturating_sub(RECORD_OVERHEAD)
    }

    /// Expects to have enough space left -> check MUST be done upfront
    #[tracing::instrument(level = "debug", skip_all)]
    #[inline]
    pub fn append_log(&mut self, id: u64, data: &[u8], buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(self.mmap_mut.is_some());
        debug_assert!(data.len() <= u32::MAX as usize);
        debug_assert!(self.has_space(data.len() as u32));
        debug_assert_eq!(self.data_start.is_some(), self.data_end.is_some());

        let (start, update_header) = if self.data_start.is_none() {
            self.id_from = id;
            let start = self.offset_logs();
            self.data_start = Some(start as u32);
            (start, true)
        } else {
            (self.data_end.unwrap() as usize + 1, false)
        };

        // TODO this can panic if the cluster is overwhelmed -> why?
        //  Saw this for very huge transactional inserts. Maybe when the queries are too big
        //  to fit inside the WAL? We could also test with a simple `File` handle for the writer
        //  and only mmap the readers. This would get rid of the size limit when it overflows, and
        //  it would only grow the file to a size bigger than expected instead of maybe panicking.
        let mmap = self.mmap_mut.as_mut().unwrap();

        let crc = crc!(&data);
        (&mut mmap[start + 8..start + 8 + 8]).write_all(crc.as_slice())?;

        let len = data.len() as u32;
        u32_to_bin(len, buf)?;
        (&mut mmap[start + 8 + 4..start + 8 + 4 + 4]).write_all(buf)?;

        (&mut mmap[start + 8 + 4 + 4..]).write_all(data)?;

        // Note: We write the id last on purpose and only after the complete data section has been
        // written. This makes automatic integrity checks / repairs after a crash less likely to
        // output false positive warning logs, since it uses a non-Null log ID as an indicator
        // for possibly existing, orphaned data.
        buf.clear();
        u64_to_bin(id, buf)?;
        (&mut mmap[start..start + 8]).write_all(buf)?;

        self.id_until = id;
        self.data_end = Some((start + 8 + 4 + 4 + data.len()) as u32);

        debug_assert!(self.data_start.is_some());
        debug_assert!(self.data_end.is_some());
        debug_assert!(self.data_end.unwrap() <= self.len_max);

        if update_header {
            buf.clear();
            self.update_header(buf)?;
        }

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
            committed: self.committed.clone(),
            len_max: self.len_max,
            mmap: None,
            mmap_mut: None,
            file: None,
        }
    }

    #[inline]
    pub fn clone_from_no_mmap(&mut self, other: &Self) {
        debug_assert_eq!(self.path, other.path);
        self.id_from = other.id_from;
        self.id_until = other.id_until;
        self.data_start = other.data_start;
        self.data_end = other.data_end;
        self.committed.clone_from(&other.committed);
    }

    /// Reads the logs into the given buffer. Returns `Ok(offset)` of the first log.
    #[tracing::instrument(level = "debug", skip_all)]
    #[inline]
    pub fn read_logs(
        &self,
        id_from: u64,
        id_until: u64,
        memo: &mut Option<LogReadMemo>,
        buf: &mut Vec<(u64, Vec<u8>)>,
    ) -> Result<u32, Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(id_until >= id_from);

        // openraft's reference stores answer a range read that starts below the first entry they
        // hold, and its storage test suite asks for one (index 0). Clamp to what we have instead of
        // failing - nothing in the runtime has ever asked for a range below the first id.
        let id_from = id_from.max(self.id_from);
        if id_from > id_until || self.data_start.is_none() {
            // the whole requested range sits below our first entry, or we hold nothing at all -
            // an empty result, like openraft's reference stores give for it
            return Ok(0);
        }

        debug_assert!(self.data_start.is_some());
        debug_assert!(self.data_end.is_some());
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

        if let Some(memo) = memo
            && memo.last_wal_no == self.wal_no
            // the memo is only valid for the exact same WAL file (same `wal_no` AND same
            // `id_from`): after all files are purged and re-created, `wal_no` values wrap
            // around and a stale memo would start scanning at an offset that belongs to
            // different logs
            && memo.id_from == self.id_from
            && memo.last_log_id < id_from
        {
            // we can use the memoized last log as our start position
            // `data_end` is inclusive
            idx = memo.data_end + 1;
            debug!("LogReadMemo match, shifting start idx to: {}", idx);
        }
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
                    buf.push((record.log_id, record.data.to_vec()));
                } else {
                    break;
                }
            }
            if record.log_id >= id_until {
                debug_assert!(record.data.len() < u32::MAX as usize);
                match self.version {
                    1 | 2 => {
                        *memo = Some(LogReadMemo {
                            last_wal_no: self.wal_no,
                            // the file this memo belongs to - `wal_no` alone is not a unique
                            // identifier, because it wraps around after all files are purged
                            id_from: self.id_from,
                            last_log_id: record.log_id,
                            // id, crc, length, data
                            data_end: idx + 8 + 4 + 4 + record.data.len() as u32,
                        });
                    }
                    _ => unreachable!(),
                }
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

    /// Reads a record at the given `offset`. The caller MUST make sure that the offset is valid
    /// and within the bounds of the file - this function additionally checks that the record
    /// header and data fit into the file, so callers can rely on it not reading past the end of
    /// the mapped region (a corrupt `data_len` must not cause an OOB read or a u32 overflow).
    #[inline]
    fn read_record_unchecked(&self, offset: u32) -> Result<WalRecord<'_>, Error> {
        // make sure the record header fits completely into the file - use u64 so that a large
        // `offset` cannot wrap around in u32 arithmetic and pass this check
        if offset as u64 + 8 + 4 + 4 > self.len_max as u64 {
            return Err(Error::Integrity(
                format!("WAL record header at offset {offset} does not fit into file").into(),
            ));
        }

        // id, crc, length
        let head = self.read_bytes(offset, offset + 8 + 4 + 4)?;

        let log_id = bin_to_u64(&head[..8])?;
        let crc = &head[8..12];

        let data_from = offset + 8 + 4 + 4;
        let data_len = bin_to_u32(&head[12..16])?;
        if data_len == 0 {
            return Err(Error::Integrity(
                format!("Attempt to read non-existent data of length 0\n{self:?}").into(),
            ));
        }
        // make sure the record data fits into the file - use u64 so a corrupt `data_len` cannot
        // wrap around in u32 arithmetic and pass this check
        if (offset as u64 + 8 + 4 + 4) + data_len as u64 > self.len_max as u64 {
            return Err(Error::Integrity(
                format!("WAL record data at offset {offset} does not fit into file").into(),
            ));
        }
        let data = self.read_bytes(data_from, data_from + data_len)?;

        Ok(WalRecord { log_id, crc, data })
    }

    /// Does NOT do any boundary checks for the given range!
    #[inline]
    fn read_bytes(&self, from: u32, until: u32) -> Result<&[u8], Error> {
        debug_assert!(from < until, "from < until -> {from} < {until}");
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

    /// Starts writeback of all dirty pages without waiting for completion.
    ///
    /// On Linux this uses `sync_file_range(SYNC_FILE_RANGE_WRITE)`, because
    /// `msync(MS_ASYNC)` starts no writeback there at all. This is not a durability
    /// barrier: neither the device write cache nor the extent allocations of the
    /// sparse WAL file are flushed, so data flushed this way must not be treated as
    /// on disk and `is_dirty` must stay set.
    #[inline]
    pub fn flush_async(&mut self) -> Result<(), Error> {
        debug_assert!(self.mmap_mut.is_some());

        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;

            let file = self.file.as_ref().expect("file kept while mmap_mut");
            // SAFETY: fd is valid, offset + nbytes = 0 means "whole file", flags are type-safe enum
            let res = unsafe {
                libc::sync_file_range(file.as_raw_fd(), 0, 0, libc::SYNC_FILE_RANGE_WRITE)
            };
            if res != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        #[cfg(not(target_os = "linux"))]
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

        #[cfg(unix)]
        mmap.advise(memmap2::Advice::Sequential)?;

        self.mmap = Some(mmap);

        Ok(())
    }

    #[inline]
    pub fn mmap_drop(&mut self) {
        self.mmap = None;
    }

    #[inline]
    pub fn mmap_mut(&mut self) -> Result<(), Error> {
        if self.mmap_mut.is_some() {
            return Ok(());
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            // the file should already exist at this point
            .create(false)
            .open(&self.path)?;

        let mmap = unsafe { MmapOptions::new().populate().map_mut(&file)? };

        #[cfg(unix)]
        mmap.advise(memmap2::Advice::Sequential)?;

        self.mmap_mut = Some(mmap);
        self.file = Some(file);

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
                format!("min allowed `wal_size` is {MIN_WAL_SIZE}").into(),
            ));
        }

        let path = Self::build_full_path(base_path, wal_no);

        Ok(Self {
            wal_no,
            path,
            version: WAL_VERSION,
            id_from,
            id_until,
            data_start: None,
            data_end: None,
            committed: None,
            len_max: wal_size,
            mmap: None,
            mmap_mut: None,
            file: None,
        })
    }

    #[inline]
    #[tracing::instrument(skip_all)]
    pub fn create_file(&mut self, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());

        self.build_header(buf)?;

        let mut file = File::create_new(&self.path)?;
        file.set_len(self.len_max as u64)?;
        file.write_all(buf)?;
        file.sync_data()?;
        // `sync_data()` makes the file's content durable, but not its name: the new directory
        // entry lives in the parent directory. Without syncing it, a power loss right after a
        // WAL rollover can lose the new file, including every log appended and acknowledged to
        // it in the meantime. This only runs once per new WAL file, never per append.
        sync_parent_dir(&self.path)?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
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

        let mut buf = vec![0; HEADER_V2];
        let mut file = File::open(&path_full)?;
        // The current file size will be the max size, because new WALs will be initialized with
        // all 0s during creation.
        let len_max = file.metadata()?.len();
        debug_assert!(len_max < u32::MAX as u64);

        file.read_exact(&mut buf)?;

        if buf[..7].iter().as_slice() != MAGIC_NO_WAL {
            return Err(Error::FileCorrupted("Invalid WAL file magic number".into()));
        }
        let version = buf[7];
        if version != 1 && version != 2 {
            return Err(Error::FileCorrupted("Invalid WAL file version".into()));
        }
        let id_from = bin_to_u64(&buf[8..16])?;
        let id_until = bin_to_u64(&buf[16..24])?;

        let data_start = bin_to_u32(&buf[24..28])?;
        let data_end = bin_to_u32(&buf[28..32])?;
        // version 1 has no `committed` slot at all, and a version 2 slot of length zero is `None`
        let committed = if version == 2 {
            Self::read_committed_slot(&buf[32..])?
        } else {
            None
        };

        Ok(Self {
            version,
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
            committed,
            len_max: len_max as u32,
            mmap: None,
            mmap_mut: None,
            file: None,
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
            2 => {
                buf.push(self.version);
                u64_to_bin(self.id_from, buf)?;
                u64_to_bin(self.id_until, buf)?;
                u32_to_bin(self.data_start.unwrap_or(0), buf)?;
                u32_to_bin(self.data_end.unwrap_or(0), buf)?;
                Self::write_committed_slot(self.committed.as_deref(), buf)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// Writes the `committed` log id into its fixed-size header slot: a one byte length prefix,
    /// the serialized id, then zeroes up to [`COMMITTED_SLOT`]. A zero length means `None`.
    ///
    /// The slot is fixed so `offset_logs` stays a constant: log data must start at the same byte
    /// in every file of a version, whatever the pointer happens to serialize to.
    fn write_committed_slot(committed: Option<&[u8]>, buf: &mut Vec<u8>) -> Result<(), Error> {
        let bytes = committed.unwrap_or_default();
        if bytes.len() > COMMITTED_SLOT {
            return Err(Error::Generic(
                format!(
                    "serialized `committed` log id is {} bytes, the WAL header slot holds \
                     {COMMITTED_SLOT}",
                    bytes.len(),
                )
                .into(),
            ));
        }
        buf.push(bytes.len() as u8);
        buf.extend_from_slice(bytes);
        buf.resize(buf.len() + (COMMITTED_SLOT - bytes.len()), 0);
        Ok(())
    }

    /// Counterpart of [`Self::write_committed_slot`]; `buf` starts at the slot's length byte.
    fn read_committed_slot(buf: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let len = buf[0] as usize;
        if len == 0 {
            return Ok(None);
        }
        if len > COMMITTED_SLOT {
            return Err(Error::FileCorrupted(
                "`committed` log id length out of range".into(),
            ));
        }
        Ok(Some(buf[1..=len].to_vec()))
    }

    #[inline]
    pub fn offset_logs(&self) -> usize {
        match self.version {
            // MAGIC NO, version, id_from, id_until, data_start, data_end
            1 => HEADER_V1,
            // the above, plus the length byte and the `committed` slot
            2 => HEADER_V2,
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
    ///
    /// `committed` seeds the new file's header slot so the pointer survives a roll-over; pass the
    /// value the previous file carried.
    #[tracing::instrument(level = "debug", skip_all)]
    #[inline]
    pub fn add_file(
        &mut self,
        wal_size: u32,
        buf: &mut Vec<u8>,
        committed: Option<Vec<u8>>,
    ) -> Result<&WalFile, Error> {
        let wal_no = self.files.back().map_or(1, |w| w.wal_no + 1);
        let mut wal = WalFile::new(wal_no, &self.base_path, 0, 0, wal_size)?;
        wal.committed = committed;
        wal.create_file(buf)?;
        self.files.push_back(wal);

        if wal_no == 1 {
            self.active = Some(0);
        }

        Ok(self.files.back().unwrap())
    }

    /// Checks the integrity of the Headers and makes sure the order is strictly ascending and
    /// there are no missing log IDs.
    ///
    /// With the deep check on, every file is scanned, oldest first, and the log is rolled back
    /// behind the first corrupt record. `last_purged_log_id` bounds that rollback: the log store
    /// must never end up with a tail below the purge frontier.
    #[tracing::instrument(skip_all)]
    pub fn check_integrity(
        &mut self,
        buf: &mut Vec<u8>,
        wal_deep_integrity_check: bool,
        last_purged_index: Option<u64>,
    ) -> Result<(), Error> {
        if self.files.is_empty() {
            return Ok(());
        }

        let mut iter = self.files.iter();

        let first = iter.next().unwrap();
        // the `data_end` bounds check below is skipped for the last file, so it must be done
        // here for the first one - otherwise a corrupt header could pass startup checks
        if first.data_end.unwrap_or(0) > first.len_max {
            return Err(Error::Integrity(
                "WAL data offset bigger than file size".into(),
            ));
        }
        let mut wal_no = first.wal_no;
        let mut until = first.id_until;
        if first.id_from > until {
            return Err(Error::FileCorrupted(
                "`id_from` cannot be greater than `id_until`".into(),
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
        let check = env::var("HQL_CHECK_WAL_INTEGRITY")
            .as_deref()
            .unwrap_or("false")
            .parse::<bool>()
            .expect("Cannot parse HQL_CHECK_WAL_INTEGRITY as bool");
        if wal_deep_integrity_check || check {
            self.check_repair_all_files(buf, last_purged_index)?;
        }

        Ok(())
    }

    /// Deep-checks every file, oldest first, and truncates everything behind the first corrupt
    /// record.
    ///
    /// Only the active file can lose data to a crash before its fsync, so for the sealed files this
    /// is a safety barrier: a bad record anywhere in the log is rolled back to the last good one and
    /// every file behind it is dropped. The log stays a valid prefix, `read_committed` clamps to the
    /// new tail, and openraft re-syncs the missing entries from the leader. A persisted `committed`
    /// pointer makes openraft re-apply `(last_applied, committed]` at startup, and that read fails
    /// hard on a bad record, so the check has to cover the whole range a startup replay can ask for
    /// and not just the newest file.
    ///
    /// The rollback is bounded by `last_purged_index`, and that bound is checked before any file is
    /// deleted and before any header is written. The bound is on the log tail the store is left
    /// with: a repair that empties a file leaves the newest log at the id before that file's first,
    /// not at the `id_until` its header keeps. A refusal leaves every file and every header as it
    /// found them; only the corrupt record's own log id is zeroed, so the same check cannot fire
    /// twice.
    ///
    /// A sealed file whose header lags its own data is the other outcome, and it is not a rollback:
    /// its header is extended to cover the records the file really holds, and every newer file
    /// stays. Only a rollback ever drops a file, and a repair that shrinks the file's data range is
    /// always the rollback, never the extension.
    ///
    /// # Errors
    ///
    /// [`Error::Integrity`] when the only rollback that heals the corrupt record would drop the log
    /// tail below `last_purged_log_id`. openraft treats a log store whose last log id is behind the
    /// purge frontier as corrupted, and no replay can bring the dropped entries back.
    ///
    /// [`Error::Integrity`]: crate::error::Error::Integrity
    fn check_repair_all_files(
        &mut self,
        buf: &mut Vec<u8>,
        last_purged_index: Option<u64>,
    ) -> Result<(), Error> {
        let last_idx = self.files.len().saturating_sub(1);

        for idx in 0..self.files.len() {
            let file = &mut self.files[idx];
            if file.mmap_mut.is_none() {
                file.mmap_mut()?;
            }
            let before = (file.id_until, file.data_end);
            buf.clear();
            file.check_repair_data_integrity(buf)?;

            if idx == last_idx {
                // The active file keeps its `mmap_mut`: the writer thread takes over from here.
                return Ok(());
            }

            // A rollback moves this file's data backwards, the unexpected-data repair moves it
            // forwards, and only a rollback may drop the files behind it. `data_end` is what
            // decides: auto-heal shrinks it, down to `None`, while a file that holds exactly one
            // log id shrinks `id_until` not at all, so `id_until` alone cannot tell the two apart.
            if file.id_until < before.0 || file.data_end.unwrap_or(0) < before.1.unwrap_or(0) {
                // The rollback deletes every file behind this one, so the purge frontier has to be
                // settled first. Deleting the newer files and only then refusing would destroy the
                // intact entries the operator is left trying to salvage.
                //
                // The bound is on the tail the store is left with, not on the `id_until` the
                // header keeps. A repair that empties a file leaves it holding nothing, and the
                // newest log this node then has is the one before its first id.
                let tail = if file.data_end.is_some() {
                    file.id_until
                } else {
                    file.id_from.saturating_sub(1)
                };
                if let Some(purged) = last_purged_index
                    && tail < purged
                {
                    // Dropping `mmap_mut` without a header write leaves this file untouched. The
                    // zeroed log id of the corrupt record is the only byte that reaches disk.
                    file.mmap_mut = None;
                    file.file = None;
                    return Err(Error::Integrity(
                        format!(
                            "healing WAL file {} would drop the log tail to {}, behind the purged \
                             log {purged} - the log store cannot be repaired, no file was deleted \
                             and no header was written, restore it from a backup",
                            file.path, tail,
                        )
                        .into(),
                    ));
                }

                warn!(
                    "WAL file {} rolled back to Log ID {} during the integrity check - every log \
                     after it is dropped and re-synced from the cluster",
                    file.path, file.id_until,
                );
                buf.clear();
                file.update_header(buf)?;
                file.flush()?;
                file.mmap_mut = None;
                file.file = None;
                self.truncate_files_after(idx)?;
                return Ok(());
            }

            if (file.id_until, file.data_end) != before {
                warn!(
                    "WAL file {} header extended to Log ID {} during the integrity check - the \
                     file held records its header had not reached",
                    file.path, file.id_until,
                );
                buf.clear();
                file.update_header(buf)?;
                file.flush()?;
            }

            file.mmap_mut = None;
            file.file = None;
        }

        Ok(())
    }

    /// Removes every WAL file behind `idx` and makes it the active one.
    ///
    /// The purge frontier is already settled by [`Self::check_repair_all_files`] before this runs,
    /// so it never removes a file the log store still needs.
    fn truncate_files_after(&mut self, idx: usize) -> Result<(), Error> {
        while self.files.len() > idx + 1 {
            let file = self.files.pop_back().unwrap();
            fs::remove_file(&file.path)?;
        }
        self.active = Some(idx);

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

    #[tracing::instrument(level = "debug", skip_all)]
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

        let mut set = Self {
            active: Some(0),
            base_path,
            files: VecDeque::with_capacity(file_names.len()),
        };

        let mut buf = Vec::with_capacity(HEADER_V2);
        for (idx, name) in file_names.iter().enumerate() {
            let path_full = format!("{}/{}", set.base_path, name);
            let Some(wal_no) = name
                .strip_suffix(".wal")
                .and_then(|num| num.parse::<u64>().ok())
            else {
                return Err(Error::InvalidFileName);
            };

            match WalFile::read_from_file(path_full.clone()) {
                Ok(wal) => set.files.push_back(wal),
                // The newest file is the one a crash can leave headerless: a roll-over creates it
                // and writes its header before anything is appended to it, so a process dying
                // inside that window leaves a file that holds no log entries at all. Every older
                // file was sealed and fsynced before a newer one ever existed, so an unreadable
                // file behind the newest one is a real hole in the log.
                //
                // Only a file with no header at all is healable. A file that does have one and
                // still cannot be read is a disk problem, and deleting it would throw away log
                // entries that exist.
                Err(err)
                    if idx + 1 == file_names.len()
                        && cfg!(feature = "auto-heal")
                        && (matches!(&err, Error::FileCorrupted(_))
                            || matches!(&err, Error::IO(io) if io.kind() == ErrorKind::UnexpectedEof)) =>
                {
                    warn!(
                        "Auto-healing the newest WAL file {path_full}: {err} - the file is \
                         re-created empty; entries it held are re-synced from the cluster"
                    );
                    set.heal_trailing_file(wal_no, wal_size, &mut buf)?;
                }
                // A WAL file we cannot read is not something the store may start without: with
                // `max_in_snapshot_log_to_keep` at 1, the newest file often holds the whole log
                // tail after the last snapshot, so skipping it would silently start the node with
                // a state machine behind its own committed log.
                Err(err) => {
                    return Err(Error::Integrity(
                        format!(
                            "cannot read WAL file {path_full}: {err} - refusing to start with a \
                             log store that is missing this file; restore it from a backup or \
                             start the node with an empty log directory"
                        )
                        .into(),
                    ));
                }
            }
        }

        if set.files.is_empty() {
            let mut wal = WalFile::new(1, &set.base_path, 0, 0, wal_size)?;
            wal.create_file(&mut buf)?;
            set.files.push_back(wal);
        }
        set.active = Some(set.files.len() - 1);

        Ok(set)
    }

    /// Re-creates the newest WAL file when it has no readable header at all.
    ///
    /// The file is removed and rebuilt empty under the same `wal_no`, so the file sequence stays
    /// unbroken and `check_integrity` sees exactly what a clean roll-over would have produced. The
    /// `committed` pointer of the sealed file is carried into the new header, because this file is
    /// the one the writer publishes into from now on.
    fn heal_trailing_file(
        &mut self,
        wal_no: u64,
        wal_size: u32,
        buf: &mut Vec<u8>,
    ) -> Result<(), Error> {
        let carried = self
            .files
            .back()
            .map(|f| f.committed.clone())
            .unwrap_or_default();

        fs::remove_file(WalFile::build_full_path(&self.base_path, wal_no))?;
        let mut wal = WalFile::new(wal_no, &self.base_path, 0, 0, wal_size)?;
        wal.committed = carried;
        wal.create_file(buf)?;
        self.files.push_back(wal);

        Ok(())
    }

    /// Rolls a new WAL file and "closes" the current one. Removes any memory-mapping and flushes
    /// the current file to disk. Creates a new WAL with `mmap_mut`
    #[tracing::instrument(level = "debug", skip_all)]
    #[inline]
    pub fn roll_over(&mut self, wal_size: u32, buf: &mut Vec<u8>) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(!self.files.is_empty());
        debug_assert!(self.files.back().unwrap().mmap_mut.is_some());

        let last_id = {
            let active = self.active();
            active.update_header(buf)?;
            // The seal must flush blocking: entries appended since the last flush would
            // otherwise be acknowledged while only the new, empty file is flushed later.
            // This costs one fsync per WAL file, and a sealed file is never dirty again.
            active.flush()?;
            active.mmap_mut = None;
            active.file = None;
            active.id_until
        };
        // the sealed file holds the newest published pointer - carry it into the new file so a
        // roll-over never resets the durable `committed` back to `None`
        let carried = self
            .files
            .back()
            .map(|f| f.committed.clone())
            .unwrap_or_default();

        buf.clear();
        self.add_file(wal_size, buf, carried)?;
        self.active = Some(self.files.len() - 1);

        let active = self.active();
        active.mmap_mut()?;
        active.id_from = last_id + 1;
        active.id_until = last_id + 1;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    #[inline]
    pub fn shift_delete_logs(
        &mut self,
        id_from: u64,
        id_until: u64,
        wal_size: u32,
        buf: &mut Vec<u8>,
        buf_logs: &mut Vec<(u64, Vec<u8>)>,
    ) -> Result<(), Error> {
        debug_assert!(buf.is_empty());
        debug_assert!(buf_logs.is_empty());
        debug_assert!(!self.files.is_empty());

        let purge_front = Some(id_from) <= self.files.front().map(|f| f.id_from);
        let truncate_back = Some(id_until) >= self.files.back().map(|f| f.id_until);
        let mut memo: Option<LogReadMemo> = None;

        if purge_front {
            while self.files.len() > 1 && self.files.front().unwrap().id_until < id_until {
                let file = self.files.pop_front().unwrap();
                fs::remove_file(&file.path)?;
            }
            // the last file is never deleted, it is emptied in place (like the truncate case
            // below) - deleting and re-creating it recycles its `wal_no`, and the reader task keeps
            // reading the deleted file through the mmap it cached for it
            if self.files.len() == 1 && self.files.front().unwrap().id_until < id_until {
                let front = self.files.front_mut().unwrap();
                front.id_from = id_until;
                front.id_until = id_until;
                front.data_start = None;
                front.data_end = None;
            }

            let front = self.files.front_mut().unwrap();
            if front.id_from < id_until && front.data_end.is_some() {
                debug_assert!(
                    front.id_until >= id_until,
                    "id_until: {id_until}, front: {front:?}"
                );
                if front.mmap_mut.is_none() {
                    front.mmap_mut()?;
                }
                let offset = front.read_logs(id_until, id_until, &mut memo, buf_logs)?;
                front.id_from = id_until;
                front.data_start = Some(offset);
            }
        } else if truncate_back {
            while !self.files.is_empty() && self.files.back().unwrap().id_from > id_from {
                let file = self.files.pop_back().unwrap();
                fs::remove_file(&file.path)?;
            }
            if self.files.is_empty() {
                // the whole log is gone, so the node holds no committed pointer any more
                self.add_file(wal_size, buf, None)?;
                self.files.front_mut().unwrap().mmap_mut()?;
            }

            let back = self.files.back_mut().unwrap();
            if back.id_from == id_from {
                back.id_until = id_from;
                back.data_start = None;
                back.data_end = None;
            } else if back.id_until >= id_from {
                if back.mmap_mut.is_none() {
                    back.mmap_mut()?;
                }
                // offset always goes forwards
                let offset = back.read_logs(id_from, id_from, &mut memo, buf_logs)?;
                back.id_until = id_from - 1;
                // data_end is inclusive
                back.data_end = Some(offset - 1);
            }
        }

        self.active = Some(self.files.len() - 1);

        Ok(())
    }
}

/// Syncs the directory containing `path`, so that a file just created inside it survives a
/// power loss.
fn sync_parent_dir(path: &str) -> Result<(), Error> {
    let dir = std::path::Path::new(path)
        .parent()
        .ok_or(Error::InvalidPath("WAL file path has no parent directory"))?;
    // Directories cannot be opened as files on Windows, and NTFS journals the entry anyway.
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(test)]
    tests::synced_dirs::record(dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    pub(super) mod synced_dirs {
        use std::path::{Path, PathBuf};
        use std::sync::Mutex;

        static SYNCED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

        pub(crate) fn record(dir: &Path) {
            SYNCED.lock().unwrap().push(dir.to_path_buf());
        }

        pub(crate) fn count(dir: &Path) -> usize {
            SYNCED
                .lock()
                .unwrap()
                .iter()
                .filter(|d| d.as_path() == dir)
                .count()
        }
    }

    #[test]
    fn create_file_syncs_its_directory() -> Result<(), Error> {
        let base_path = format!("{}/create_file_syncs_dir", PATH);
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;
        let dir = std::path::Path::new(&base_path);
        let before = synced_dirs::count(dir);

        let mut buf = Vec::with_capacity(32);
        let mut wal = WalFile::new(1, &base_path, 0, 0, MB2).unwrap();
        wal.create_file(&mut buf)?;
        assert_eq!(synced_dirs::count(dir), before + 1);

        buf.clear();
        let mut wal = WalFile::new(2, &base_path, 0, 0, MB2).unwrap();
        wal.create_file(&mut buf)?;
        assert_eq!(synced_dirs::count(dir), before + 2);

        Ok(())
    }

    static PATH: &str = "test_data";
    static MB2: u32 = 2 * 1024 * 1024;

    #[test]
    fn append_read_logs() -> Result<(), Error> {
        let base_path = format!("{PATH}/append_logs");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;
        let mut memo: Option<LogReadMemo> = None;
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
        wal_disk.read_logs(1, 3, &mut memo, &mut logs)?;
        assert_eq!(logs.len(), 3);

        let (id, data) = logs.first().unwrap();
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
        wal_disk.read_logs(2, 3, &mut memo, &mut logs)?;
        assert_eq!(logs.len(), 2);
        let (id, data) = logs.first().unwrap();
        assert_eq!(id, &2);
        assert_eq!(data, d2);
        let (id, data) = logs.get(1).unwrap();
        assert_eq!(id, &3);
        assert_eq!(data, d3);

        buf.clear();
        logs.clear();
        wal_disk.read_logs(2, 2, &mut memo, &mut logs)?;
        assert_eq!(logs.len(), 1);
        let (id, data) = logs.first().unwrap();
        assert_eq!(id, &2);
        assert_eq!(data, d2);

        buf.clear();
        logs.clear();
        assert!(wal_disk.read_logs(1, 4, &mut memo, &mut logs).is_err());

        // a range that starts below the first id is clamped to it, like openraft's reference stores
        wal_disk.read_logs(0, 2, &mut memo, &mut logs)?;
        assert_eq!(logs.len(), 2);
        let (id, data) = logs.first().unwrap();
        assert_eq!(id, &1);
        assert_eq!(data, d1);
        let (id, data) = logs.get(1).unwrap();
        assert_eq!(id, &2);
        assert_eq!(data, d2);

        Ok(())
    }

    #[test]
    fn convert_wal_header() -> Result<(), Error> {
        let base_path = format!("{PATH}/convert_wal_header");
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

        let path_h1 = format!("{base_path}/0000000000000001.wal");
        let path_h2 = format!("{base_path}/0000000000000002.wal");
        let _ = fs::remove_file(&path_with_no);
        let _ = fs::remove_file(&path_h1);
        let _ = fs::remove_file(&path_h2);

        let mut set = WalFileSet {
            active: None,
            base_path,
            files: VecDeque::default(),
        };
        buf.clear();
        set.add_file(MB2, &mut buf, None).unwrap();
        assert!(fs::exists(&path_h1)?);
        assert!(!fs::exists(&path_h2)?);

        buf.clear();
        set.add_file(MB2, &mut buf, None).unwrap();
        assert!(fs::exists(&path_h2)?);

        Ok(())
    }

    #[test]
    fn committed_survives_header_and_roll_over() -> Result<(), Error> {
        // The `committed` pointer is durable only through the header, so a file flushed with a
        // pointer must read back exactly that pointer, and a roll-over must carry it into the new
        // file instead of resetting it to `None`.
        let base_path = format!("{PATH}/committed_header");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet {
            active: None,
            base_path: base_path.clone(),
            files: VecDeque::default(),
        };
        set.add_file(MB2, &mut buf, None)?;
        assert_eq!(set.active().committed, None);

        let committed = vec![7u8; 24];
        set.active().mmap_mut()?;
        set.active().committed = Some(committed.clone());
        buf.clear();
        set.active().update_header(&mut buf)?;
        set.active().flush()?;

        let read = WalFileSet::read(base_path.clone(), MB2)?;
        assert_eq!(
            read.files.back().unwrap().committed,
            Some(committed.clone())
        );

        // the roll-over seals the file that holds the pointer and must carry it forward
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        assert_eq!(set.active().committed, Some(committed.clone()));

        set.active().mmap_mut()?;
        buf.clear();
        set.active().update_header(&mut buf)?;
        set.active().flush()?;
        let read = WalFileSet::read(base_path, MB2)?;
        assert_eq!(read.files.back().unwrap().committed, Some(committed));

        Ok(())
    }

    #[test]
    fn version_one_header_stays_readable() -> Result<(), Error> {
        // A node upgraded from a version whose headers had no `committed` slot must start with its
        // existing files untouched: version 1 keeps its shorter header and reports no pointer.
        let base_path = format!("{PATH}/header_v1");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let path = format!("{base_path}/0000000000000001.wal");
        let _ = fs::remove_file(&path);
        let mut file = fs::File::create(&path)?;
        file.set_len(MB2 as u64)?;
        let mut header = Vec::with_capacity(HEADER_V1);
        header.extend_from_slice(MAGIC_NO_WAL);
        header.push(1);
        u64_to_bin(3, &mut header)?;
        u64_to_bin(9, &mut header)?;
        u32_to_bin(32, &mut header)?;
        u32_to_bin(64, &mut header)?;
        file.write_all(&header)?;
        file.flush()?;
        drop(file);

        let wal = WalFile::read_from_file(path)?;
        assert_eq!(wal.version, 1);
        assert_eq!(wal.offset_logs(), HEADER_V1);
        assert_eq!(wal.committed, None);
        assert_eq!(wal.id_from, 3);
        assert_eq!(wal.id_until, 9);

        Ok(())
    }

    #[test]
    fn version_one_file_rolls_over_to_version_two() -> Result<(), Error> {
        // An upgraded node keeps the version 1 files it wrote before, and only writes version 2
        // headers from the next file on. Log data must stay readable across that boundary, and the
        // new file must start its data right behind its own, longer, header.
        let base_path = format!("{PATH}/header_v1_roll_over");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let path = format!("{base_path}/0000000000000001.wal");
        let _ = fs::remove_file(&path);
        let mut file = fs::File::create(&path)?;
        file.set_len(MB2 as u64)?;
        let mut header = Vec::with_capacity(HEADER_V1);
        header.extend_from_slice(MAGIC_NO_WAL);
        header.push(1);
        u64_to_bin(0, &mut header)?;
        u64_to_bin(0, &mut header)?;
        u32_to_bin(0, &mut header)?;
        u32_to_bin(0, &mut header)?;
        file.write_all(&header)?;
        file.flush()?;
        drop(file);

        let mut wal = WalFile::read_from_file(path)?;
        assert_eq!(wal.version, 1);
        wal.mmap_mut()?;
        let mut buf = Vec::with_capacity(HEADER_V2);
        for (id, data) in [(1, b"one".as_slice()), (2, b"two")] {
            buf.clear();
            wal.append_log(id, data, &mut buf)?;
        }
        buf.clear();
        wal.update_header(&mut buf)?;
        wal.flush()?;
        drop(wal);

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        assert_eq!(set.files.len(), 1);
        assert_eq!(set.files[0].version, 1);
        // two records of 3 bytes each: header, then 8 id + 4 crc + 4 len + payload, plus a byte
        // of gap between the records
        let record = 8 + 4 + 4 + 3;
        assert_eq!(
            set.files[0].data_end,
            Some(HEADER_V1 as u32 + record + 1 + record)
        );

        // the file a roll-over creates is version 2, and it carries the pointer of the sealed file
        set.active().mmap_mut()?;
        set.active().committed = Some(vec![7u8; 24]);
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        assert_eq!(set.active().version, 2);
        assert_eq!(set.active().offset_logs(), HEADER_V2);
        assert_eq!(set.active().committed, Some(vec![7u8; 24]));

        set.active().mmap_mut()?;
        buf.clear();
        set.active().append_log(3, b"three", &mut buf)?;
        assert_eq!(set.active().data_start, Some(HEADER_V2 as u32));
        buf.clear();
        set.active().update_header(&mut buf)?;
        set.active().flush()?;
        drop(set);

        let mut set = WalFileSet::read(base_path, MB2)?;
        assert_eq!(set.files.len(), 2);
        assert_eq!(set.files[0].version, 1);
        assert_eq!(set.files[1].version, 2);
        assert_eq!(set.files[1].committed, Some(vec![7u8; 24]));

        // logs from both files are readable, each at its own version's offsets
        let mut memo = None;
        let mut logs = Vec::new();
        set.files[0].mmap()?;
        set.files[0].read_logs(1, 2, &mut memo, &mut logs)?;
        assert_eq!(logs, vec![(1, b"one".to_vec()), (2, b"two".to_vec())]);

        let mut logs = Vec::new();
        set.files[1].mmap()?;
        set.files[1].read_logs(3, 3, &mut memo, &mut logs)?;
        assert_eq!(logs, vec![(3, b"three".to_vec())]);

        Ok(())
    }

    #[test]
    #[cfg(feature = "auto-heal")]
    fn unreadable_trailing_wal_file_is_healed() -> Result<(), Error> {
        // Regression test: a roll-over creates the new file and writes its header before anything
        // is appended to it, so a process dying inside that window leaves a newest file with no
        // readable header. Skipping it started the node behind its own committed log; refusing to
        // start bricked it. `auto-heal` re-creates it empty and keeps everything the sealed files
        // hold, so a hard reset during a roll-over stays the recoverable event it always was.
        let base_path = format!("{PATH}/heal_trailing");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        set.active().append_log(1, b"one", &mut buf)?;
        buf.clear();
        set.active().append_log(2, b"two", &mut buf)?;
        buf.clear();
        set.active().committed = Some(vec![7u8; 24]);
        set.roll_over(MB2, &mut buf)?;
        buf.clear();
        set.active().append_log(3, b"three", &mut buf)?;
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        // the newest file is now exactly what a crash inside `create_file` leaves behind
        let path2 = format!("{base_path}/0000000000000002.wal");
        fs::write(&path2, b"")?;

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        assert_eq!(
            set.files.len(),
            2,
            "the healed file must keep the wal_no sequence intact"
        );
        assert_eq!(set.active().wal_no, 2);
        assert_eq!(
            set.active().data_start,
            None,
            "the healed file must be empty"
        );
        assert_eq!(set.active().data_end, None, "the healed file must be empty");
        assert_eq!(
            set.files[0].id_until, 2,
            "the sealed file must keep the logs it held"
        );
        assert_eq!(
            set.active().committed,
            Some(vec![7u8; 24]),
            "the sealed file's committed pointer must survive the heal"
        );

        // the log tail is the sealed file's, which is what `read_committed` clamps to
        let mut memo = None;
        let mut logs = Vec::new();
        set.files[0].mmap()?;
        set.files[0].read_logs(1, 2, &mut memo, &mut logs)?;
        assert_eq!(logs, vec![(1, b"one".to_vec()), (2, b"two".to_vec())]);
        drop(set);

        // the power-loss shape of the same event: a file of full size whose header never landed,
        // so it carries no magic at all
        let mut file = OpenOptions::new().write(true).open(&path2)?;
        file.write_all(&[0u8; 8])?;
        file.sync_all()?;
        drop(file);

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        assert_eq!(
            set.files.len(),
            2,
            "a headerless file of full size must be healed as well"
        );
        assert_eq!(set.active().wal_no, 2);
        assert_eq!(set.active().data_start, None);
        assert_eq!(
            set.active().committed,
            Some(vec![7u8; 24]),
            "the heal must carry the pointer on both shapes"
        );

        Ok(())
    }

    #[test]
    #[cfg(feature = "auto-heal")]
    fn unreadable_wal_file_behind_the_newest_refuses_to_start() -> Result<(), Error> {
        // The heal covers the newest file only. A headerless file behind it is a real hole: every
        // older file was sealed and fsynced before a newer one ever existed, so starting without it
        // would serve reads from a state machine behind its own committed log.
        let base_path = format!("{PATH}/hole_behind_newest");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        set.active().append_log(1, b"one", &mut buf)?;
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        buf.clear();
        set.active().append_log(2, b"two", &mut buf)?;
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        fs::write(format!("{base_path}/0000000000000001.wal"), b"")?;

        let err = WalFileSet::read(base_path, MB2).unwrap_err();
        assert!(
            matches!(err, Error::Integrity(_)),
            "a hole behind the newest file must refuse to start, got {err}"
        );

        Ok(())
    }

    #[test]
    #[cfg(feature = "auto-heal")]
    fn deep_integrity_check_truncates_behind_a_corrupt_record() -> Result<(), Error> {
        // Regression test: only the newest file used to be deep-checked, so a bad record in a
        // sealed file stayed in the log. With a persisted `committed` pointer openraft re-applies
        // `(last_applied, committed]` at startup and that read fails hard, so the check has to
        // cover every file and roll the log back behind the corrupt record.
        let base_path = format!("{PATH}/truncate_behind_corrupt");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        set.active().append_log(1, b"one", &mut buf)?;
        buf.clear();
        set.active().append_log(2, b"two", &mut buf)?;
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        buf.clear();
        set.active().append_log(3, b"three", &mut buf)?;
        buf.clear();
        set.active().append_log(4, b"four", &mut buf)?;
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        // flip a payload byte of log 2, inside the sealed file, so its CRC no longer matches
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("{base_path}/0000000000000001.wal"))?;
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        let record = 8 + 4 + 4 + 3;
        let log_two_data = HEADER_V2 + record + 1 + 8 + 4 + 4;
        mmap[log_two_data] = 0xFF;
        mmap.flush()?;
        drop(mmap);
        drop(file);

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        let mut buf = Vec::with_capacity(HEADER_V2);
        set.check_integrity(&mut buf, true, Some(0))?;

        assert_eq!(
            set.files.len(),
            1,
            "every file behind the corrupt record must be dropped"
        );
        assert_eq!(set.active().wal_no, 1);
        assert_eq!(
            set.active().id_until,
            1,
            "the log must roll back to the last good record"
        );
        assert!(
            fs::metadata(format!("{base_path}/0000000000000002.wal")).is_err(),
            "the dropped file must be removed"
        );

        // the log is a valid prefix again: log 1 reads, log 2 is gone and gets re-synced
        let mut memo = None;
        let mut logs = Vec::new();
        set.active().mmap()?;
        set.active().read_logs(1, 1, &mut memo, &mut logs)?;
        assert_eq!(logs, vec![(1, b"one".to_vec())]);

        Ok(())
    }

    #[cfg(feature = "auto-heal")]
    #[test]
    fn deep_integrity_check_refuses_below_purged_without_touching_the_dir() -> Result<(), Error> {
        // Regression test: the rollback deletes every file behind the corrupt one, so the purge
        // frontier has to be settled before anything reaches disk. Refusing after the deletion
        // destroyed the intact entries the operator is left trying to salvage.
        let base_path = format!("{PATH}/refuse_below_purged");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        for (id, data) in [(1, b"one".as_slice()), (2, b"two")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        for (id, data) in [(3, b"three".as_slice()), (4, b"four")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        for (id, data) in [(5, b"five".as_slice()), (6, b"six")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        // flip a payload byte of log 2, inside the oldest file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("{base_path}/0000000000000001.wal"))?;
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        let record = 8 + 4 + 4 + 3;
        let log_two_data = HEADER_V2 + record + 1 + 8 + 4 + 4;
        mmap[log_two_data] = 0xFF;
        mmap.flush()?;
        drop(mmap);
        drop(file);

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        let mut buf = Vec::with_capacity(HEADER_V2);
        // the frontier sits above the only rollback that heals log 2, so the node cannot start
        let err = set.check_integrity(&mut buf, true, Some(3)).unwrap_err();
        assert!(
            matches!(err, Error::Integrity(_)),
            "expected Integrity, got {err:?}"
        );

        for wal_no in [2, 3] {
            let path = format!("{base_path}/000000000000000{wal_no}.wal");
            assert!(fs::exists(&path)?, "{path} must survive the refusal");
        }

        // the bad file keeps its own header: the refusal must not shorten it
        let healed = WalFile::read_from_file(format!("{base_path}/0000000000000001.wal"))?;
        assert_eq!(healed.id_until, 2, "the header must be left as it was");
        assert_eq!(
            healed.data_end,
            Some((HEADER_V2 + record + 1 + record) as u32)
        );

        // the newer files are still readable, which is the whole point of a non-destructive refusal
        let mut memo = None;
        let mut logs = Vec::new();
        set.files[1].mmap()?;
        set.files[1].read_logs(3, 4, &mut memo, &mut logs)?;
        assert_eq!(logs, vec![(3, b"three".to_vec()), (4, b"four".to_vec())]);

        Ok(())
    }

    #[cfg(feature = "auto-heal")]
    #[test]
    fn deep_integrity_check_keeps_newer_files_when_a_sealed_header_lags() -> Result<(), Error> {
        // Regression test: the repair for a sealed file whose header never reached its last record
        // EXTENDS `id_until`. Treating any header change as a rollback dropped every newer file and
        // destroyed log entries that existed nowhere else.
        let base_path = format!("{PATH}/sealed_header_lags");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        for (id, data) in [(1, b"one".as_slice()), (2, b"two")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.active().update_header(&mut buf)?;
        set.active().flush()?;

        // roll over seals file 1, and file 2 gets logs 3 and 4
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        for (id, data) in [(3, b"three".as_slice()), (4, b"four")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        // append into file 1 and drop the mapping WITHOUT writing the header, the way a process
        // killed between the append and its header flush leaves it: the record reaches disk and the
        // header keeps saying `id_until = 2`
        let mut wal = WalFile::read_from_file(format!("{base_path}/0000000000000001.wal"))?;
        wal.mmap_mut()?;
        buf.clear();
        wal.append_log(3, b"three", &mut buf)?;
        wal.mmap_mut = None;
        wal.file = None;
        drop(wal);

        let lagging = WalFile::read_from_file(format!("{base_path}/0000000000000001.wal"))?;
        assert_eq!(
            lagging.id_until, 2,
            "the header must start out behind the data"
        );

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        let mut buf = Vec::with_capacity(HEADER_V2);
        set.check_integrity(&mut buf, true, Some(0))?;

        assert_eq!(
            set.files.len(),
            2,
            "an extended header is a repair, it must not drop the files behind it"
        );
        assert!(
            fs::exists(format!("{base_path}/0000000000000002.wal"))?,
            "the newer file must survive"
        );

        let healed = WalFile::read_from_file(format!("{base_path}/0000000000000001.wal"))?;
        assert_eq!(
            healed.id_until, 3,
            "the repaired header must cover the record the file holds"
        );

        // log 4 is the entry the old behaviour destroyed
        let mut memo = None;
        let mut logs = Vec::new();
        set.files[1].mmap()?;
        set.files[1].read_logs(3, 4, &mut memo, &mut logs)?;
        assert_eq!(logs, vec![(3, b"three".to_vec()), (4, b"four".to_vec())]);

        Ok(())
    }

    #[cfg(feature = "auto-heal")]
    #[test]
    fn deep_integrity_check_rolls_back_a_file_that_holds_one_record() -> Result<(), Error> {
        // Regression test: auto-heal of a corrupt first record resets `data_end` to `None` and
        // leaves `id_until` at `id_from`. In a file that holds exactly one log id nothing moves
        // backwards, so a classifier that compares `id_until` alone reads the shrink as a header
        // extension: the newer files stay and the log keeps a hole where the corrupt record was,
        // which a range read then answers as a short read with no error at all.
        let base_path = format!("{PATH}/single_record_file_rolls_back");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        for (id, data) in [(1, b"one".as_slice()), (2, b"two")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        // the middle file holds EXACTLY one record, so `id_from == id_until`
        buf.clear();
        set.active().append_log(3, b"three", &mut buf)?;
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        for (id, data) in [(4, b"four".as_slice()), (5, b"five")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        // flip a payload byte of log 3, inside the middle file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("{base_path}/0000000000000002.wal"))?;
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        let log_three_data = HEADER_V2 + 8 + 4 + 4;
        mmap[log_three_data] = 0xFF;
        mmap.flush()?;
        drop(mmap);
        drop(file);

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        let mut buf = Vec::with_capacity(HEADER_V2);
        set.check_integrity(&mut buf, true, Some(0))?;

        // the shrink is a rollback, so everything behind the corrupt record is gone
        assert_eq!(set.files.len(), 2, "the rollback must drop the newer files");
        assert_eq!(set.active, Some(1));
        assert!(
            !fs::exists(format!("{base_path}/0000000000000003.wal"))?,
            "the file behind a corrupt record must be dropped"
        );

        // and the log is a valid prefix again, with no hole inside the range it claims
        let mut memo = None;
        let mut logs = Vec::new();
        set.files[0].mmap()?;
        set.files[0].read_logs(1, 2, &mut memo, &mut logs)?;
        assert_eq!(logs, vec![(1, b"one".to_vec()), (2, b"two".to_vec())]);

        Ok(())
    }

    #[cfg(feature = "auto-heal")]
    #[test]
    fn deep_integrity_check_refuses_a_single_record_rollback_below_purged() -> Result<(), Error> {
        // The same shape as above, with the purge frontier above the only rollback that heals the
        // record. The shrink has to run through the purge gate like any other rollback: refusing
        // after the newer files were deleted would destroy the entries the operator is left trying
        // to salvage.
        let base_path = format!("{PATH}/single_record_refuse_below_purged");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        for (id, data) in [(1, b"one".as_slice()), (2, b"two")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        buf.clear();
        set.active().append_log(3, b"three", &mut buf)?;
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        for (id, data) in [(4, b"four".as_slice()), (5, b"five")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("{base_path}/0000000000000002.wal"))?;
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        let log_three_data = HEADER_V2 + 8 + 4 + 4;
        mmap[log_three_data] = 0xFF;
        mmap.flush()?;
        drop(mmap);
        drop(file);

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        let mut buf = Vec::with_capacity(HEADER_V2);
        let err = set.check_integrity(&mut buf, true, Some(4)).unwrap_err();
        assert!(
            matches!(err, Error::Integrity(_)),
            "expected Integrity, got {err:?}"
        );

        for wal_no in [2, 3] {
            let path = format!("{base_path}/000000000000000{wal_no}.wal");
            assert!(fs::exists(&path)?, "{path} must survive the refusal");
        }

        // the refusal must not shrink the header of the file it refuses to roll back
        let healed = WalFile::read_from_file(format!("{base_path}/0000000000000002.wal"))?;
        assert_eq!(healed.id_until, 3, "the header must be left as it was");
        assert_eq!(
            healed.data_end,
            Some((HEADER_V2 + 8 + 4 + 4 + b"three".len()) as u32)
        );

        Ok(())
    }

    #[cfg(feature = "auto-heal")]
    #[test]
    fn deep_integrity_check_refuses_a_corrupt_first_record_at_the_frontier() -> Result<(), Error> {
        // Regression test: after a purge up to P the oldest file starts at `id_from == P`, and a
        // corrupt FIRST record empties that file - `data_start` and `data_end` go to `None` and
        // `id_until` stays at `id_from`. The store is then left with a tail of P - 1, so a gate
        // that compares `id_until` (P) against the frontier (P) lets the rollback through and
        // every newer file is deleted for a record nobody needs any more than the frontier does.
        let base_path = format!("{PATH}/first_record_at_purged");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(HEADER_V2);
        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        set.active().mmap_mut()?;
        for (id, data) in [(1, b"one".as_slice()), (2, b"two")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        for (id, data) in [(3, b"three".as_slice()), (4, b"four")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.roll_over(MB2, &mut buf)?;
        for (id, data) in [(5, b"five".as_slice()), (6, b"six")] {
            buf.clear();
            set.active().append_log(id, data, &mut buf)?;
        }
        buf.clear();
        set.active().flush()?;
        set.active().mmap_mut = None;
        set.active().file = None;
        drop(set);

        // flip a payload byte of log 1, the FIRST record of the oldest file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("{base_path}/0000000000000001.wal"))?;
        let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        let log_one_data = HEADER_V2 + 8 + 4 + 4;
        mmap[log_one_data] = 0xFF;
        mmap.flush()?;
        drop(mmap);
        drop(file);

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        let mut buf = Vec::with_capacity(HEADER_V2);
        // the frontier is log 1, and logs 2..=6 sit behind it
        let err = set.check_integrity(&mut buf, true, Some(1)).unwrap_err();
        assert!(
            matches!(err, Error::Integrity(_)),
            "expected Integrity, got {err:?}"
        );

        // the rollback that the refusal prevents is the one that deletes logs 2..=6
        for wal_no in [2, 3] {
            let path = format!("{base_path}/000000000000000{wal_no}.wal");
            assert!(fs::exists(&path)?, "{path} must survive the refusal");
        }

        let healed = WalFile::read_from_file(format!("{base_path}/0000000000000001.wal"))?;
        assert_eq!(healed.id_until, 2, "the header must be left as it was");
        assert_eq!(
            healed.data_end,
            Some((HEADER_V2 + 19 + 1 + 19) as u32),
            "an emptied header is the damage the refusal must not do"
        );

        Ok(())
    }

    #[test]
    fn max_data_len_is_the_exact_capacity_of_the_receiving_file() -> Result<(), Error> {
        // A single entry cannot span WAL files, and the writer rolls over and appends without
        // checking the space a second time, so an entry above the receiving file's capacity would
        // be written past the end of the mapping. A log directory mixes header versions, so the cap
        // follows the header of the file that receives the entry.
        let base_path = format!("{PATH}/max_data_len");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut set = WalFileSet::read(base_path.clone(), MB2)?;
        assert_eq!(set.active().version, 2);
        let v2 = set.active().max_data_len(MB2);
        assert_eq!(v2, MB2 as usize - HEADER_V2 - RECORD_OVERHEAD);
        // whatever the gate lets through has to fit an empty file of this version
        assert!(set.active().has_space(v2 as u32));
        assert!(!set.active().has_space(v2 as u32 + 1));

        set.active().mmap_mut()?;
        let mut buf = Vec::new();
        set.active().append_log(1, b"one", &mut buf)?;
        buf.clear();
        set.roll_over(MB2, &mut buf)?;

        // a version 1 header on the same `wal_size` has exactly the committed slot more room, so a
        // cap taken from the older file would overflow the newer one
        let mut v1 = set.active().clone_no_mmap();
        v1.version = 1;
        assert_eq!(v1.max_data_len(MB2) - v2, HEADER_V2 - HEADER_V1);

        // the cap never exceeds the size the file actually has, which a `wal_size` change can move
        let mut smaller = set.active().clone_no_mmap();
        smaller.len_max = MB2 / 2;
        assert_eq!(smaller.max_data_len(MB2), smaller.max_data_len(MB2 / 2));

        drop(set);
        let _ = fs::remove_dir_all(&base_path);
        Ok(())
    }

    #[test]
    fn integrity_check() -> Result<(), Error> {
        let mut buf = Vec::new();
        let base_path = format!("{PATH}/integrity_check");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
            version: 1,
            wal_no: 1,
            path: String::new(),
            id_from: 1,
            id_until: 10,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        files.push_back(WalFile {
            version: 1,
            wal_no: 2,
            path: String::new(),
            id_from: 11,
            id_until: 17,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        files.push_back(WalFile {
            version: 1,
            wal_no: 3,
            path: String::new(),
            id_from: 18,
            id_until: 33,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        let mut set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files,
        };
        set.check_integrity(&mut buf, false, None).unwrap();
        assert_eq!(set.active().wal_no, 3);

        let mut buf = Vec::with_capacity(28);
        set.add_file(MB2, &mut buf, None)?;
        set.check_integrity(&mut buf, false, None).unwrap();
        assert_eq!(set.active().wal_no, 3);

        // missing logs - invalid if_until -> next id_from
        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
            version: 1,
            wal_no: 1,
            path: String::new(),
            id_from: 1,
            id_until: 10,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        files.push_back(WalFile {
            version: 1,
            wal_no: 3,
            path: String::new(),
            id_from: 18,
            id_until: 33,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        let mut set = WalFileSet {
            active: Some(1),
            base_path: base_path.clone(),
            files,
        };
        assert!(set.check_integrity(&mut buf, false, None).is_err());

        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
            version: 1,
            wal_no: 1,
            path: String::new(),
            id_from: 1,
            id_until: 10,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        files.push_back(WalFile {
            version: 1,
            wal_no: 2,
            path: String::new(),
            id_from: 11,
            id_until: 17,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        files.push_back(WalFile {
            version: 1,
            wal_no: 4,
            path: String::new(),
            id_from: 18,
            id_until: 33,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        let mut set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files,
        };
        assert!(set.check_integrity(&mut buf, false, None).is_err());

        let mut files = VecDeque::with_capacity(4);
        files.push_back(WalFile {
            version: 1,
            wal_no: 1,
            path: String::new(),
            id_from: 1,
            id_until: 10,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        files.push_back(WalFile {
            version: 1,
            wal_no: 2,
            path: String::new(),
            id_from: 11,
            id_until: 17,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        files.push_back(WalFile {
            version: 1,
            wal_no: 3,
            path: String::new(),
            id_from: 19,
            id_until: 33,
            // make integrity check work
            data_start: Some(32),
            data_end: Some(64),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        let mut set = WalFileSet {
            active: Some(2),
            base_path: base_path.clone(),
            files,
        };
        assert!(set.check_integrity(&mut buf, false, None).is_err());

        Ok(())
    }

    #[test]
    fn roll_over_purge_front() -> Result<(), Error> {
        let base_path = format!("{PATH}/roll_over_purge_front");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;
        let mut buf = Vec::with_capacity(8);

        let mut wal = WalFileSet {
            active: None,
            base_path,
            files: VecDeque::default(),
        };
        wal.add_file(MB2, &mut buf, None)?;
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
        wal.shift_delete_logs(0, 1, MB2, &mut buf, &mut buf_logs)?;
        let front = wal.files.front().unwrap();
        assert_eq!(front.id_from, 1);
        assert_eq!(front.data_start.unwrap() as usize, front.offset_logs());

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(0, 2, MB2, &mut buf, &mut buf_logs)?;
        let front = wal.files.front().unwrap();
        assert_eq!(front.id_from, 2);
        let start = 8 + 4 + 4 + d1.len() + front.offset_logs() + 1;
        assert_eq!(front.data_start.unwrap() as usize, start);

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(0, 3, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.len(), 1);
        let front = wal.files.front().unwrap();
        assert_eq!(front.id_from, 3);
        assert_eq!(front.data_start.unwrap() as usize, front.offset_logs());

        Ok(())
    }

    /// An inclusive purge moves a file's front forward; reading the remaining range must still work
    /// - the reader task does exactly that on `get_log_state`.
    #[test]
    fn purge_then_read_shifted_front() -> Result<(), Error> {
        let base_path = format!("{PATH}/purge_then_read_shifted_front");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;
        let mut buf = Vec::with_capacity(8);

        let mut wal = WalFileSet {
            active: None,
            base_path,
            files: VecDeque::default(),
        };
        wal.add_file(MB2, &mut buf, None)?;
        wal.active().mmap_mut()?;

        for id in 0..=10u64 {
            buf.clear();
            wal.active().append_log(id, b"data".as_slice(), &mut buf)?;
        }

        let mut memo: Option<LogReadMemo> = None;
        let mut buf_logs = Vec::with_capacity(8);

        // an inclusive purge of id 3 moves the front to 4
        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(0, 4, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.active().id_from, 4);

        buf.clear();
        buf_logs.clear();
        wal.active().read_logs(10, 10, &mut memo, &mut buf_logs)?;
        assert_eq!(buf_logs.len(), 1);
        assert_eq!(buf_logs[0].0, 10);

        buf.clear();
        buf_logs.clear();
        wal.active().read_logs(4, 10, &mut memo, &mut buf_logs)?;
        assert_eq!(buf_logs.len(), 7);

        Ok(())
    }

    #[test]
    fn roll_over_truncate_end() -> Result<(), Error> {
        let base_path = format!("{PATH}/roll_over_truncate_end");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;
        let mut buf = Vec::with_capacity(8);

        let mut wal = WalFileSet {
            active: None,
            base_path,
            files: VecDeque::default(),
        };
        wal.add_file(MB2, &mut buf, None)?;
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
        wal.shift_delete_logs(6, u64::MAX, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.front().unwrap().id_from, 1);
        let back = wal.files.back().unwrap();
        assert_eq!(back.id_from, 3);
        assert_eq!(back.id_until, 5);
        let data_end = back.offset_logs() + 16 + d3.len() + 1 + 16 + d4.len() + 1 + 16 + d5.len();
        assert_eq!(back.data_end, Some(data_end as u32));

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(5, u64::MAX, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.front().unwrap().id_from, 1);
        let back = wal.files.back().unwrap();
        assert_eq!(back.id_from, 3);
        assert_eq!(back.id_until, 4);
        let data_end = back.offset_logs() + 16 + d3.len() + 1 + 16 + d4.len();
        assert_eq!(back.data_end, Some(data_end as u32));

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(4, u64::MAX, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.front().unwrap().id_from, 1);
        let back = wal.files.back().unwrap();
        assert_eq!(back.id_from, 3);
        assert_eq!(back.id_until, 3);
        let data_end = back.offset_logs() + 16 + d3.len();
        assert_eq!(back.data_end, Some(data_end as u32));

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(3, u64::MAX, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.front().unwrap().id_from, 1);
        let back = wal.files.back().unwrap();
        assert_eq!(back.id_from, 3);
        assert_eq!(back.id_until, 3);
        assert_eq!(back.data_start, None);
        assert_eq!(back.data_end, None);

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(2, u64::MAX, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.len(), 1);
        assert_eq!(wal.files.front().unwrap().id_from, 1);
        let back = wal.files.back().unwrap();
        assert_eq!(back.id_from, 1);
        assert_eq!(back.id_until, 1);
        let data_end = back.offset_logs() + 16 + d1.len();
        assert_eq!(back.data_end, Some(data_end as u32));

        buf.clear();
        buf_logs.clear();
        wal.shift_delete_logs(1, u64::MAX, MB2, &mut buf, &mut buf_logs)?;
        assert_eq!(wal.files.len(), 1);
        let back = wal.files.back().unwrap();
        assert_eq!(back.data_start, None);
        assert_eq!(back.data_end, None);

        Ok(())
    }

    #[test]
    fn deep_integrity_check_stops_before_eof() -> Result<(), Error> {
        // Regression test: when the last record ends close to the end of the file, the phase-2
        // scan must not read a partial record header past the end of the mapped region (which
        // used to be a u32 overflow / out-of-bounds mmap slice).
        let base_path = format!("{PATH}/deep_check_eof");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut buf = Vec::with_capacity(32);
        let mut wal = WalFile::new(1, &base_path, 0, 0, MB2).unwrap();
        wal.create_file(&mut buf)?;
        wal.mmap_mut()?;

        // one record whose `data_end` lands two bytes before the end of the file: the phase-2
        // scan starts at `data_end + 1`, where a full 16 byte header does not fit anymore
        let data_len = MB2 as usize - wal.offset_logs() - 16 - 2;
        buf.clear();
        wal.append_log(1, &vec![7u8; data_len], &mut buf)?;
        assert_eq!(wal.data_end, Some(MB2 - 2));

        // must not panic and must report the file as consistent
        buf.clear();
        wal.check_repair_data_integrity(&mut buf)?;
        assert_eq!(wal.id_from, 1);
        assert_eq!(wal.id_until, 1);

        Ok(())
    }

    #[test]
    fn read_logs_memo_survives_wal_no_reuse() -> Result<(), Error> {
        // Regression test: after all WAL files are purged and a new file re-uses `wal_no` 1, an
        // old `LogReadMemo` must not be applied - it would start scanning at the wrong offset.
        let base_old = format!("{PATH}/memo_wraparound_old");
        let base_new = format!("{PATH}/memo_wraparound_new");
        for path in [&base_old, &base_new] {
            let _ = fs::remove_dir_all(path);
            fs::create_dir_all(path)?;
        }

        let mut buf = Vec::with_capacity(32);
        let mut memo: Option<LogReadMemo> = None;

        // old file #1 with logs 1..=3
        let mut wal_old = WalFile::new(1, &base_old, 0, 0, MB2).unwrap();
        wal_old.create_file(&mut buf)?;
        wal_old.mmap_mut()?;
        for id in 1..=3 {
            buf.clear();
            wal_old.append_log(id, b"payload", &mut buf)?;
        }

        // read a range that sets the memo (last log ID 2)
        let mut logs = Vec::new();
        buf.clear();
        wal_old.read_logs(1, 2, &mut memo, &mut logs)?;
        assert_eq!(logs.len(), 2);

        // new file #1 (same `wal_no` after wraparound) with logs 6..=8 at the same offsets
        let mut wal_new = WalFile::new(1, &base_new, 0, 0, MB2).unwrap();
        wal_new.create_file(&mut buf)?;
        wal_new.mmap_mut()?;
        for id in 6..=8 {
            buf.clear();
            wal_new.append_log(id, b"payload", &mut buf)?;
        }

        // with the stale memo, `read_logs` would start at the old offset and miss log 6
        let mut logs = Vec::new();
        buf.clear();
        wal_new.read_logs(6, 7, &mut memo, &mut logs)?;
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].0, 6);
        assert_eq!(logs[1].0, 7);

        Ok(())
    }

    #[test]
    fn has_space_does_not_overflow() {
        // Regression test: with a large `data_end` the old u32 arithmetic wrapped around and
        // reported free space that does not exist.
        let mut wal = WalFile::new(1, "some/path", 0, 0, MB2).unwrap();

        wal.data_end = Some(u32::MAX - 10);
        assert!(!wal.has_space(5)); // (u32::MAX - 10) + 17 + 5 wraps in u32 arithmetic
        assert_eq!(wal.space_left(), 0);

        wal.data_end = None;
        // the largest payload that still fits: file size, minus the header, minus the record head
        // (1 offset + 8 id + 4 crc + 4 len), minus one so the comparison stays strict
        let max_len = MB2 - wal.offset_logs() as u32 - (1 + 8 + 4 + 4) - 1;
        assert!(wal.has_space(max_len));
        assert!(!wal.has_space(max_len + 1)); // exactly full: no space left

        // `max_data_len` is the writer's oversized-entry gate and it must be exactly this
        // boundary: an entry that passes it has to fit into an empty file, because the writer
        // rolls over and appends without checking the space again.
        assert_eq!(
            wal.max_data_len(MB2),
            max_len as usize,
            "max_data_len must match the has_space boundary of an empty file"
        );
    }

    #[test]
    fn integrity_check_rejects_first_file_data_end_overflow() -> Result<(), Error> {
        // Regression test: the `data_end` bounds check used to be skipped for the first file,
        // so a corrupt header could pass startup integrity checks.
        let base_path = format!("{PATH}/integrity_first_eof");
        let _ = fs::remove_dir_all(&base_path);
        fs::create_dir_all(&base_path)?;

        let mut files = VecDeque::with_capacity(1);
        files.push_back(WalFile {
            version: 1,
            wal_no: 1,
            path: String::new(),
            id_from: 1,
            id_until: 5,
            data_start: Some(32),
            // bigger than the file size - must be rejected
            data_end: Some(MB2 + 4096),
            committed: None,
            len_max: MB2,
            mmap: None,
            mmap_mut: None,
            file: None,
        });
        let mut set = WalFileSet {
            active: Some(0),
            base_path,
            files,
        };

        let mut buf = Vec::with_capacity(28);
        assert!(matches!(
            set.check_integrity(&mut buf, false, None).unwrap_err(),
            Error::Integrity(_)
        ));

        Ok(())
    }
}
