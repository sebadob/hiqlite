use crate::server::args::{Args, LogLevel};
use clap::Parser;
use hiqlite::Error;

mod args;
mod cache;
mod config;
mod logging;
mod password;

pub async fn server() -> Result<(), Error> {
    match Args::parse() {
        Args::Serve(args) => {
            logging::init_logging(&args.log_level);

            let node_config = config::build_node_config(args)?;
            let client = hiqlite::start_node::<cache::Cache>(node_config).await?;

            let mut shutdown_handle = client.shutdown_handle()?;
            shutdown_handle.wait().await?;
        }
        Args::GenerateConfig(args) => {
            logging::init_logging(&LogLevel::Info);
            config::generate(args).await?;
        }
    }

    Ok(())
}
