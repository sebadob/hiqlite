#[cfg(all(feature = "sqlite", feature = "rocksdb"))]
pub mod rocksdb;

#[cfg(feature = "cache")]
pub mod memory;
#[cfg(all(feature = "sqlite", feature = "migrate-rocksdb"))]
pub mod migrate;

#[cfg(feature = "sqlite")]
pub fn logs_dir_db(data_dir: &str) -> String {
    format!("{}/logs", data_dir)
}

#[cfg(feature = "cache")]
pub fn logs_dir_cache(data_dir: &str) -> String {
    format!("{}/logs_cache", data_dir)
}
