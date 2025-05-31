use crate::server::args::LogLevel;
// use tracing_subscriber::prelude::*;

pub fn init_logging(level: &LogLevel, _node_id: Option<u64>) {
    // if node_id == Some(1) {
    //     let console_layer = console_subscriber::spawn();
    //     tracing_subscriber::registry()
    //         .with(console_layer)
    //         .with(tracing_subscriber::fmt::layer())
    //         .init();
    // } else {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(level.as_str())
        .init();
    // }
}
