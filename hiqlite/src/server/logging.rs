use crate::server::args::LogLevel;
use tracing_subscriber::EnvFilter;

pub fn init_logging(level: &LogLevel) {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::new(level.as_str()))
        .init();
}
