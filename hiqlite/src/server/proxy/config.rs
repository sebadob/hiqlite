use crate::config_toml::{t_bool, t_str, t_str_secret, t_str_vec, t_table, t_u16, t_u32};
use crate::tls::{ServerTlsConfig, ServerTlsConfigCerts};
use crate::{Error, Node, RateLimitConfig};
use tokio::fs;

#[derive(Debug)]
pub struct Config {
    pub listen_addr: String,
    pub listen_port: u16,
    pub nodes: Vec<String>,
    pub tls_config: Option<ServerTlsConfig>,
    pub secret_api: String,
    pub max_stream_connections: usize,
    pub rate_limit_cache: Option<RateLimitConfig>,
    pub rate_limit_db: Option<RateLimitConfig>,
}

impl Config {
    /// Tries to read the proxy `Config` from the given TOML file path. Will use default values for
    /// all non-existing keys. You can define a custom `table` to read from; if none, expects the
    /// data to be in `[hiqlite]`.
    ///
    /// You can overwrite most values from the file with ENV vars. If this is possible, it is
    /// mentioned in the documentation for each value.
    ///
    /// The secrets source is, in order of precedence:
    /// 1. the `secrets` table passed in here, if `Some`;
    /// 2. otherwise a `secrets_file` config option (or the `HQL_SECRETS_FILE` env var) pointing to
    ///    a TOML file that mirrors the config structure (i.e. holds the same `[{table}]` table).
    pub async fn from_toml(
        path: &str,
        table: Option<&str>,
        secrets: Option<toml::Table>,
    ) -> Result<Self, Error> {
        dotenvy::dotenv().ok();

        let t_name = table.unwrap_or("hiqlite");

        let config = fs::read_to_string(path).await.map_err(|err| {
            Error::config(format!("Cannot read proxy config file from: {path}: {err}"))
        })?;

        let mut root = config
            .parse::<toml::Table>()
            .map_err(|err| Error::config(format!("Cannot parse TOML file: {err}")))?;
        let table = t_table(&mut root, t_name).map_err(|err| {
            Error::config(format!("Cannot find table '{t_name}' in {path}: {err}"))
        })?;

        Self::from_toml_table(table, t_name, secrets).await
    }

    /// Tries to parse the proxy `Config` from the already parsed given `toml::Table`. Will use
    /// default values for all non-existing keys. The `table_name` is only used for potential logs
    /// if any errors are encountered.
    ///
    /// You can overwrite most values from the file with ENV vars. If this is possible, it is
    /// mentioned in the documentation for each value.
    ///
    /// See [`Config::from_toml`] for how the `secrets` table and the `"$SECRETS"` sentinel work.
    /// If `secrets` is `None`, a `secrets_file` config option (or `HQL_SECRETS_FILE`) is honored as
    /// a fallback.
    pub async fn from_toml_table(
        mut table: toml::Table,
        t_name: &str,
        secrets: Option<toml::Table>,
    ) -> Result<Self, Error> {
        dotenvy::dotenv().ok();

        // Resolve the optional secrets source used by the `$SECRETS` sentinel. An explicitly
        // passed-in `secrets` table wins; otherwise a `secrets_file` path (or `HQL_SECRETS_FILE`)
        // is loaded, which must mirror the config structure (i.e. hold the same `[{t_name}]`
        // table). The `secrets_file` value is only read when no `secrets` table was passed in.
        let secrets_owned = match secrets {
            Some(secrets) => Some(secrets),
            None => match t_str(&mut table, t_name, "secrets_file", "HQL_SECRETS_FILE")? {
                Some(path) => {
                    let content = fs::read_to_string(&path).await.map_err(|err| {
                        Error::config(format!("Cannot read secrets file from: {path}: {err}"))
                    })?;
                    let mut root = content.parse::<toml::Table>().map_err(|err| {
                        Error::config(format!("Cannot parse secrets file {path}: {err}"))
                    })?;
                    let secrets = t_table(&mut root, t_name).map_err(|err| {
                        Error::config(format!(
                            "Cannot find table '{t_name}' in secrets file {path}: {err}"
                        ))
                    })?;
                    Some(secrets)
                }
                None => None,
            },
        };
        let secrets = secrets_owned.as_ref();

        // The proxy only needs the API address of each node to load-balance against.
        let nodes = if let Some(nodes) = t_str_vec(&mut table, t_name, "nodes", "HQL_NODES")? {
            nodes
                .into_iter()
                .map(|n| Node::from(n.as_str()).addr_api)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let listen_addr = t_str(&mut table, t_name, "listen_addr", "LISTEN_ADDR")?
            .unwrap_or_else(|| "0.0.0.0".to_string());
        let listen_port = t_u16(&mut table, t_name, "listen_port", "LISTEN_PORT")?.unwrap_or(8200);

        let max_stream_connections = t_u32(
            &mut table,
            t_name,
            "max_stream_connections",
            "HQL_PROXY_MAX_STREAM_CONNECTIONS",
        )?
        .unwrap_or(20) as usize;

        // Honor the same rate-limit settings a node would, so a proxy can be limited too.
        let rate_limit_cache = {
            if let Some(rps) = t_u32(
                &mut table,
                t_name,
                "rate_limit_cache_rps",
                "HQL_RL_CACHE_RPS",
            )? {
                Some(RateLimitConfig {
                    rps,
                    burst: t_u32(
                        &mut table,
                        t_name,
                        "rate_limit_cache_burst",
                        "HQL_RL_CACHE_BURST",
                    )?
                    .unwrap_or(rps),
                })
            } else {
                None
            }
        };

        let rate_limit_db = {
            if let Some(rps) = t_u32(&mut table, t_name, "rate_limit_db_rps", "HQL_RL_DB_RPS")? {
                Some(RateLimitConfig {
                    rps,
                    burst: t_u32(&mut table, t_name, "rate_limit_db_burst", "HQL_RL_DB_BURST")?
                        .unwrap_or(rps),
                })
            } else {
                None
            }
        };

        let tls_auto_certificates = t_bool(
            &mut table,
            t_name,
            "tls_auto_certificates",
            "HQL_TLS_AUTO_CERTS",
        )?
        .unwrap_or(false);

        let tls_api_key = t_str(&mut table, t_name, "tls_api_key", "HQL_TLS_API_KEY")?;
        let tls_api_cert = t_str(&mut table, t_name, "tls_api_cert", "HQL_TLS_API_CERT")?;
        let tls_api_danger_tls_no_verify = t_bool(
            &mut table,
            t_name,
            "tls_api_danger_tls_no_verify",
            "HQL_TLS_API_NO_VERIFY",
        )?
        .unwrap_or(false);

        if tls_api_key.is_some() != tls_api_cert.is_some() {
            return Err(Error::Config("Incomplete API TLS config given".into()));
        }

        let tls_config = if let Some(tls_api_key) = tls_api_key
            && let Some(tls_api_cert) = tls_api_cert
        {
            Some(ServerTlsConfig::Specific(ServerTlsConfigCerts {
                key: tls_api_key.into(),
                cert: tls_api_cert.into(),
                danger_tls_no_verify: tls_api_danger_tls_no_verify,
            }))
        } else if tls_auto_certificates {
            Some(ServerTlsConfig::TlsAutoCertificates)
        } else {
            None
        };

        let Some(secret_api) =
            t_str_secret(&mut table, t_name, "secret_api", "HQL_SECRET_API", secrets)?
        else {
            return Err(Error::config(format!(
                "{t_name}.secret_api is a mandatory value"
            )));
        };

        // The proxy only knows a fixed set of keys. Anything left over is unknown and therefore a
        // config error (e.g. someone pasted node-only keys like `node_id` into the proxy file).
        if !table.is_empty() {
            let unknown = table.keys().cloned().collect::<Vec<_>>();
            return Err(Error::config(format!(
                "Unknown config key(s) in [{t_name}]: {}",
                unknown.join(", ")
            )));
        }

        Ok(Config {
            listen_addr,
            listen_port,
            nodes,
            tls_config,
            secret_api,
            max_stream_connections,
            rate_limit_cache,
            rate_limit_db,
        })
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

        // A zero permit count would make every `/stream` acquire time out at the
        // 10 s budget forever, so reject it up front instead of binding a proxy
        // that can never serve a single stream.
        if self.max_stream_connections == 0 {
            return Err(Error::Config(
                "'max_stream_connections' must be greater than zero".into(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parse_ref_config() {
        // make sure it can be parsed properly. Any keys inside it that are unknown would error,
        // and `is_valid` must hold for the reference values.
        let config = Config::from_toml("../REFERENCE_CONFIG_PROXY.toml", None, None)
            .await
            .unwrap();
        config.is_valid().unwrap();
    }
}
