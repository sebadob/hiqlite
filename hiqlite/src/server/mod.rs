use crate::empty::Empty;
use crate::server::args::{Args, LogLevel};
use crate::server::proxy::config::Config;
use crate::{APP_VERSION, Error, start_node_with_cache};
use clap::Parser;
use tracing::info;

mod args;
pub mod config;
mod logging;
mod password;
mod proxy;

pub async fn server() -> Result<(), Error> {
    match Args::parse() {
        Args::Serve(args) => {
            logging::init_logging(&args.log_level, args.node_id);
            info!("Hiqlite Server v{}", APP_VERSION);

            let node_config = config::build_node_config(args).await?;
            let client = start_node_with_cache::<Empty>(node_config).await?;

            let mut shutdown_handle = client.shutdown_handle()?;
            shutdown_handle.wait().await?;
        }

        Args::Proxy(args) => {
            logging::init_logging(&args.log_level, None);
            info!("Hiqlite Proxy v{}", APP_VERSION);

            let config_path = if args.config_file == "$HOME/.hiqlite/hiqlite-proxy.toml" {
                config::default_proxy_config_file_path()
            } else {
                args.config_file
            };
            let config = Config::from_toml(&config_path, None, None).await?;
            config.is_valid()?;

            proxy::start_proxy(config).await?;
        }

        Args::GenerateConfig(args) => {
            logging::init_logging(&LogLevel::Info, None);
            config::generate(args).await?;
        }

        Args::GenerateProxyConfig => {
            logging::init_logging(&LogLevel::Info, None);
            config::generate_proxy().await?;
        }
    }

    Ok(())
}
