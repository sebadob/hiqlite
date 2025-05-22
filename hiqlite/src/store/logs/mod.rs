#[cfg(all(feature = "sqlite", feature = "rocksdb"))]
pub mod rocksdb;

#[cfg(feature = "cache")]
pub mod memory;
#[cfg(all(feature = "sqlite", feature = "migrate-rocksdb"))]
pub mod migrate;

#[cfg(feature = "sqlite")]
pub fn logs_dir(data_dir: &str) -> String {
    format!("{}/logs", data_dir)
}
