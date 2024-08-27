use crate::server::args::{Args, LogLevel};
use crate::server::proxy::config::Config;
use crate::{start_node_with_cache, Error};
use clap::Parser;
use tracing::info;

mod args;
mod cache;
pub mod config;
mod logging;
mod password;
mod proxy;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn server() -> Result<(), Error> {
    match Args::parse() {
        Args::Serve(args) => {
            logging::init_logging(&args.log_level);
            info!("Hiqlite Server v{}", APP_VERSION);

            let node_config = config::build_node_config(args)?;
            let client = start_node_with_cache::<cache::Cache>(node_config).await?;

            let mut shutdown_handle = client.shutdown_handle()?;
            shutdown_handle.wait().await?;
        }

        Args::Proxy(args) => {
            logging::init_logging(&args.log_level);
            info!("Hiqlite Proxy v{}", APP_VERSION);

            let config = Config::parse(args.config_file);
            config.is_valid()?;

            proxy::start_proxy(config).await?;
        }

        Args::GenerateConfig(args) => {
            logging::init_logging(&LogLevel::Info);
            config::generate(args).await?;
        }
    }

    Ok(())
}
