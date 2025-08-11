#[cfg(feature = "cache")]
pub mod memory;

#[cfg(feature = "sqlite")]
pub fn logs_dir_db(data_dir: &str) -> String {
    format!("{data_dir}/logs")
}

#[cfg(feature = "cache")]
pub fn logs_dir_cache(data_dir: &str) -> String {
    format!("{data_dir}/logs_cache")
}
