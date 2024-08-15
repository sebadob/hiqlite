use crate::s3::EncKeysFrom;
use crate::{Error, Node, ServerTlsConfig};
use cryptr::EncKeys;
use spow::pow::Pow;
use std::env;
use tracing::debug;

#[derive(Debug)]
pub struct Config {
    pub listen_port: u16,
    pub nodes: Vec<String>,
    pub tls_config: Option<ServerTlsConfig>,
    pub secret_api: String,
    // pub password_dashboard: String,
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

        let enc_keys_from = env::var("HQL_ENC_KEYS_FROM")
            .map(|v| {
                if let Some(path) = v.strip_prefix("file:") {
                    EncKeysFrom::File(path.to_string())
                } else {
                    EncKeysFrom::Env
                }
            })
            .unwrap_or(EncKeysFrom::Env);

        match enc_keys_from {
            EncKeysFrom::Env => EncKeys::from_env(),
            EncKeysFrom::File(path) => EncKeys::read_from_file(&path),
        }
        .expect("ENC_KEYS not configured correctly")
        .init()
        .unwrap();

        let enc_key_active = EncKeys::get_key_active().unwrap();
        Pow::init_bytes(enc_key_active);

        // let b64 =
        //     env::var("HQL_PASSWORD_DASHBOARD").expect("HQL_PASSWORD_DASHBOARD does not exist");
        // let password_dashboard = String::from_utf8(b64_decode(&b64).unwrap()).unwrap();

        Self {
            listen_port,
            nodes: Node::all_from_env()
                .into_iter()
                .map(|n| n.addr_api)
                .collect::<Vec<_>>(),
            tls_config: ServerTlsConfig::from_env("API"),
            secret_api: env::var("HQL_SECRET_API").expect("HQL_SECRET_API not found"),
            // password_dashboard,
        }
    }

    pub fn is_valid(&self) -> Result<(), Error> {
        if self.nodes.is_empty() {
            return Err(Error::Config("'nodes' must not be empty".into()));
        }

        if self.secret_api.len() < 16 {
            return Err(Error::Config(
                "'secret_raft' and 'secret_api' should be at least 16 characters long".into(),
            ));
        }

        // #[cfg(feature = "dashboard")]
        // if self.password_dashboard.len() < 14 {
        //     return Err(Error::Config(
        //         "password_dashboard should be at least 14 characters long".into(),
        //     ));
        // }

        Ok(())
    }
}
