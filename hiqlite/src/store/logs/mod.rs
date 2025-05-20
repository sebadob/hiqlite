// There are a `redb` and `sqlite` logs store implementations around. These are not used currently but kept
// around until more tests with async logs commit can be done. The final solution will be decided upon before
// the first release... probably.

// pub mod redb;

#[cfg(feature = "sqlite")]
pub mod rocksdb;

mod hiqlite_wal;
#[cfg(feature = "cache")]
pub mod memory;
// #[cfg(feature = "sqlite")]
// pub mod sqlite;

#[cfg(feature = "sqlite")]
pub fn logs_dir(data_dir: &str) -> String {
    format!("{}/logs", data_dir)
}
