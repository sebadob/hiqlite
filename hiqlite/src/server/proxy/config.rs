use crate::{Error, Node, RateLimitConfig, tls::ServerTlsConfig};
use cryptr::EncKeys;
use std::env;
use tracing::debug;

#[derive(Debug)]
pub struct Config {
    // TODO we also want the listen socket addr to be configurable
    pub listen_port: u16,
    pub nodes: Vec<String>,
    pub tls_config: Option<ServerTlsConfig>,
    pub secret_api: String,
    pub max_stream_connections: usize,
    pub rate_limit_cache: Option<RateLimitConfig>,
    pub rate_limit_db: Option<RateLimitConfig>,
}

impl Config {
    pub fn parse(filename: String) -> Self {
        if dotenvy::from_filename("config").is_err() {
            debug!("config file './config' not found");
        }
        if dotenvy::from_filename_override(&filename).is_err() {
            debug!("config file '{}' not found", filename);
        }
        dotenvy::dotenv_override().ok();

        let listen_port = env::var("LISTEN_PORT")
            .unwrap_or_else(|_| "8200".to_string())
            .parse::<u16>()
            .expect("Cannot parse LISTEN_PORT to u16");

        let max_stream_connections = env::var("HQL_PROXY_MAX_STREAM_CONNECTIONS")
            .unwrap_or_else(|_| "20".to_string())
            .parse::<usize>()
            .expect("Cannot parse HQL_PROXY_MAX_STREAM_CONNECTIONS to usize");

        // Honor the same rate-limit settings a node would, so a proxy can be limited too.
        let rate_limit_cache = {
            if let Some(rps) = env::var("HQL_RL_CACHE_RPS").as_deref().ok().map(|v| {
                v.parse::<u32>()
                    .expect("Cannot parse HQL_RL_CACHE_RPS as u32")
            }) {
                let burst = env::var("HQL_RL_CACHE_BURST").as_deref().ok().map(|v| {
                    v.parse::<u32>()
                        .expect("Cannot parse HQL_RL_CACHE_BURST as u32")
                });
                Some(RateLimitConfig {
                    rps,
                    burst: burst.unwrap_or(rps),
                })
            } else {
                None
            }
        };

        let rate_limit_db = {
            if let Some(rps) = env::var("HQL_RL_DB_RPS")
                .as_deref()
                .ok()
                .map(|v| v.parse::<u32>().expect("Cannot parse HQL_RL_DB_RPS as u32"))
            {
                let burst = env::var("HQL_RL_DB_BURST").as_deref().ok().map(|v| {
                    v.parse::<u32>()
                        .expect("Cannot parse HQL_RL_DB_BURST as u32")
                });
                Some(RateLimitConfig {
                    rps,
                    burst: burst.unwrap_or(rps),
                })
            } else {
                None
            }
        };

        EncKeys::from_env()
            .expect("ENC_KEYS not configured correctly")
            .init()
            .unwrap();

        Self {
            listen_port,
            nodes: Node::parse_from_env("HQL_NODES")
                .into_iter()
                .map(|n| n.addr_api)
                .collect::<Vec<_>>(),
            tls_config: ServerTlsConfig::from_env("API").expect("Cannot parse TLS config"),
            secret_api: env::var("HQL_SECRET_API").expect("HQL_SECRET_API not found"),
            max_stream_connections,
            rate_limit_cache,
            rate_limit_db,
        }
    }

    pub fn is_valid(&self) -> Result<(), Error> {
        if self.nodes.is_empty() {
            return Err(Error::Config("'nodes' must not be empty".into()));
        }

        if self.secret_api.chars().count() < 16 {
            return Err(Error::Config(
                "'secret_api' should be at least 16 characters long".into(),
            ));
        }

        Ok(())
    }
}
