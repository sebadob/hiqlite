# hiqlite-wal

This crate provides an `openraft` compatible WAL implementation that uses memory mapped files. It has been created
to provide a fast and efficient Raft Log Store for [Hiqlite](https://github.com/sebadob/hiqlite), but is being kept
generic and should work with any other implementation based on `openraft`, as long as the `NodeId` is defined as a
`u64`. The `LogStore` can be used directly in combination with `openraft`. 