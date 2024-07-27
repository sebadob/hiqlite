// Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::args::LogLevel;
use args::Args;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod args;
mod config;

#[tokio::main]
async fn main() -> Result<(), hiqlite::Error> {
    match Args::parse() {
        Args::Serve(args_config) => {
            init_logging(&args_config.log_level);

            let node_config = config::build_node_config(args_config)?;
            let client = hiqlite::start_node(node_config).await?;

            let mut shutdown_handle = client.shutdown_handle()?;
            shutdown_handle.wait().await?;
        }
        Args::GenerateConfig => {
            init_logging(&LogLevel::Info);
            config::generate().await?;
        }
    }

    Ok(())
}

fn init_logging(level: &LogLevel) {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::new(level.as_str()))
        .init();
}
