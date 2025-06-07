# hiqlite-wal

This crate provides an `openraft` compatible WAL implementation that uses memory-mapped files to avoid too many
syscalls. It has been created to provide a fast and efficient Raft Log Store
for [Hiqlite](https://github.com/sebadob/hiqlite), but is being kept generic and should work with any other
implementation based on `openraft`.

You can decide on the "flush to disk" strategy depending on your needs. You use it with `openraft` via the
`LogStore::start()`. WAL files are not created once and then overwritten to not stress the exact same blocks of your
SSD too much. Instead, a new file is being created on log roll-over and pre-populated before using it, so it does not
need to be relocated on your disk when it grows. This means all WAL files with have the exact same length, no matter
how many logs or how much data they contain.

Each WAL will have its own tiny header with some metadata information:

- 7 bytes MAGIC_NO (`HQL_WAL`)
- 1 byte WAL file version No
- 8 bytes Log ID from
- 8 bytes Log ID until
- 4 bytes Data offset start
- 4 bytes Data offset end

After the header, each log entry has a dynamic length with it's own tiny header, followed by the raw data block:

- 8 bytes u64 Log ID
- 4 bytes 32bit CRC CHKSUM
- 4 bytes Data length
- variable length Log Data

The `auto-heal` feature will try to auto heal corrupted WAL files by reverting the latest Raft Log ID to the highest
healthy one. In case of lost or unrecoverable logs, this will only work, if `openraft/loosen-follower-log-revert` is
set.
Such an integrity check can also be forced at startup (for the latest existing WAL) by setting the ENV var
`HQL_CHECK_WAL_INTEGRITY=true`, but there is usually no need to do this, as long as the lock file has not been
deleted manually.
