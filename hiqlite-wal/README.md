# hiqlite-wal

This crate provides an `openraft` compatible WAL implementation that uses memory mapped files. It has been created
to provide a fast and efficient Raft Log Store for [Hiqlite](https://github.com/sebadob/hiqlite), but is being kept
generic and should work with any other implementation based on `openraft`.

The `auto-heal` feature will try to auto heal corrupted WAL files by reverting the latest Raft Log ID to the highest
healthy one. This will only work, if `openraft/loosen-follower-log-revert` is set.
