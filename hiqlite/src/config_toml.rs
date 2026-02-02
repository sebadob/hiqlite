use crate::{Error, Node, NodeConfig, ServerTlsConfig};
use hiqlite_wal::LogSync;
use std::any::type_name;
use std::borrow::Cow;
use std::env;
use std::str::FromStr;
use tokio::fs;
use toml::Value;

impl NodeConfig {
    /// Tries to read the `NodeConfig` from the given toml file path. Will use default values for
    /// all non-existing keys. You can define a custom `table` to read from. If none, expects the
    /// data to be in `[hiqlite]`.
    ///
    /// You can overwrite most values from the file with ENV vars. If this is possible, it is
    /// mentioned in the documentation for each value.
    pub async fn from_toml(
        path: &str,
        table: Option<&str>,
        #[cfg(any(feature = "s3", feature = "dashboard"))] enc_keys: Option<cryptr::EncKeys>,
    ) -> Result<Self, Error> {
        dotenvy::dotenv().ok();

        let t_name = table.unwrap_or("hiqlite");

        let config = fs::read_to_string(path)
            .await
            .map_err(|err| Error::String(format!("Cannot read config file from: {path}: {err}")))?;

        // Note: these inner parsers are very verbose, but they allow the upfront memory allocation
        // and memory fragmentation, after the quite big toml has been freed and the config stays
        // in static memory.

        let mut root = config
            .parse::<toml::Table>()
            .map_err(|err| Error::String(format!("Cannot parse TOML file: {err}")))?;
        let table = t_table(&mut root, t_name).map_err(|err| {
            Error::String(format!("Cannot find table '{t_name}' in {path}: {err}"))
        })?;

        Self::from_toml_table(
            table,
            t_name,
            #[cfg(any(feature = "s3", feature = "dashboard"))]
            enc_keys,
        )
        .await
    }

    /// Tries to parse the `NodeConfig` from the already parsed given `toml::Table`. Will use
    /// default values for all non-existing keys. The `table_name` is only used for potential logs
    /// if any errors are encountered.
    ///
    /// You can overwrite most values from the file with ENV vars. If this is possible, it is
    /// mentioned in the documentation for each value.
    pub async fn from_toml_table(
        table: toml::Table,
        table_name: &str,
        #[cfg(any(feature = "s3", feature = "dashboard"))] enc_keys: Option<cryptr::EncKeys>,
    ) -> Result<Self, Error> {
        dotenvy::dotenv().ok();

        let t_name = table_name;
        let mut map = table;

        let node_id = if let Some(v) =
            t_type::<String>(&mut map, t_name, "node_id_from", "HQL_NODE_ID_FROM")?
        {
            if v == "k8s" {
                Self::node_id_from_hostname()
            } else {
                t_type::<u64>(&mut map, t_name, "node_id", "HQL_NODE_ID")?.unwrap_or(0)
            }
        } else {
            t_type::<u64>(&mut map, t_name, "node_id", "HQL_NODE_ID")?.unwrap_or(0)
        };
        let nodes = if let Some(nodes) = t_str_vec(&mut map, t_name, "nodes", "HQL_NODES")? {
            nodes
                .into_iter()
                .map(|n| Node::from(n.as_str()))
                .collect::<Vec<_>>()
        } else {
            vec![Node {
                id: 1,
                addr_raft: "localhost:8100".to_string(),
                addr_api: "localhost:8200".to_string(),
            }]
        };

        let listen_addr_api =
            t_type::<String>(&mut map, t_name, "listen_addr_api", "HQL_LISTEN_ADDR_API")?
                .map(Cow::from)
                .unwrap_or_else(|| "0.0.0.0".into());
        let listen_addr_raft =
            t_type::<String>(&mut map, t_name, "listen_addr_raft", "HQL_LISTEN_ADDR_RAFT")?
                .map(Cow::from)
                .unwrap_or_else(|| "0.0.0.0".into());

        let data_dir = t_type::<String>(&mut map, t_name, "data_dir", "HQL_DATA_DIR")?
            .map(Cow::from)
            .unwrap_or_else(|| "data".into());
        let filename_db = t_type::<String>(&mut map, t_name, "filename_db", "HQL_FILENAME_DB")?
            .map(Cow::from)
            .unwrap_or_else(|| "hiqlite.db".into());
        let log_statements =
            t_type::<bool>(&mut map, t_name, "log_statements", "HQL_LOG_STATEMENTS")?
                .unwrap_or(false);
        let prepared_statement_cache_capacity =
            t_type::<u16>(&mut map, t_name, "prepared_statement_cache_capacity", "")?
                .unwrap_or(1000) as usize;
        let read_pool_size =
            t_type::<u16>(&mut map, t_name, "read_pool_size", "HQL_READ_POOL_SIZE")?.unwrap_or(4)
                as usize;

        let wal_sync =
            if let Some(v) = t_type::<String>(&mut map, t_name, "log_sync", "HQL_LOG_SYNC")? {
                let Ok(sync) = LogSync::try_from(v.as_str()) else {
                    return Err(Error::String(err_t("log_sync", t_name, "LogSync")));
                };
                sync
            } else {
                LogSync::ImmediateAsync
            };
        let wal_size =
            t_type::<u32>(&mut map, t_name, "wal_size", "HQL_WAL_SIZE")?.unwrap_or(2 * 1024 * 1024);

        #[cfg(feature = "cache")]
        let cache_storage_disk = t_type::<bool>(
            &mut map,
            t_name,
            "cache_storage_disk",
            "HQL_CACHE_STORAGE_DISK",
        )?
        .unwrap_or(true);

        let logs_until_snapshot = t_type::<u64>(
            &mut map,
            t_name,
            "logs_until_snapshot",
            "HQL_LOGS_UNTIL_SNAPSHOT",
        )?
        .unwrap_or(10_000);

        let tls_raft_key = t_type::<String>(&mut map, t_name, "tls_raft_key", "HQL_TLS_RAFT_KEY")?;
        let tls_raft_cert =
            t_type::<String>(&mut map, t_name, "tls_raft_cert", "HQL_TLS_RAFT_CERT")?;
        let tls_raft_danger_tls_no_verify =
            t_type::<bool>(&mut map, t_name, "tls_raft_danger_tls_no_verify", "")?.unwrap_or(false);
        let tls_api_key = t_type::<String>(&mut map, t_name, "tls_api_key", "HQL_TLS_API_KEY")?;
        let tls_api_cert = t_type::<String>(&mut map, t_name, "tls_api_cert", "HQL_TLS_API_CERT")?;
        let tls_api_danger_tls_no_verify =
            t_type::<bool>(&mut map, t_name, "tls_raft_danger_tls_no_verify", "")?.unwrap_or(false);

        #[allow(clippy::unnecessary_unwrap)]
        let tls_raft = if tls_raft_key.is_some() && tls_raft_cert.is_some() {
            Some(ServerTlsConfig {
                key: tls_raft_key.unwrap().into(),
                cert: tls_raft_cert.unwrap().into(),
                danger_tls_no_verify: tls_raft_danger_tls_no_verify,
            })
        } else {
            None
        };
        #[allow(clippy::unnecessary_unwrap)]
        let tls_api = if tls_api_key.is_some() && tls_api_cert.is_some() {
            Some(ServerTlsConfig {
                key: tls_api_key.unwrap().into(),
                cert: tls_api_cert.unwrap().into(),
                danger_tls_no_verify: tls_api_danger_tls_no_verify,
            })
        } else {
            None
        };

        let Some(secret_raft) =
            t_type::<String>(&mut map, t_name, "secret_raft", "HQL_SECRET_RAFT")?
        else {
            return Err(format!("{t_name}.secret_raft is a mandatory value").into());
        };
        let Some(secret_api) = t_type::<String>(&mut map, t_name, "secret_api", "HQL_SECRET_API")?
        else {
            return Err(format!("{t_name}.secret_api is a mandatory value").into());
        };

        let health_check_delay_secs =
            t_type::<u32>(&mut map, t_name, "health_check_delay_secs", "")?.unwrap_or(30);

        #[cfg(feature = "backup")]
        let (backup_config, backup_keep_days_local) = {
            let backup_cron = if let Some(v) =
                t_type::<String>(&mut map, t_name, "backup_config", "HQL_BACKUP_CRON")?
            {
                Cow::from(v)
            } else {
                Cow::from("0 30 2 * * * *")
            };
            let backup_keep_days =
                t_type::<u16>(&mut map, t_name, "backup_keep_days", "HQL_BACKUP_KEEP_DAYS")?
                    .unwrap_or(30);
            let backup_keep_days_local = t_type::<u16>(
                &mut map,
                t_name,
                "backup_keep_days_local",
                "HQL_BACKUP_KEEP_DAYS_LOCAL",
            )?
            .unwrap_or(30);

            let backup_config =
                crate::backup::BackupConfig::new(backup_cron.as_ref(), backup_keep_days)
                    .map_err(|err| Error::String(format!("Error building BackupConfig: {err}")))?;
            (backup_config, backup_keep_days_local)
        };

        #[cfg(feature = "s3")]
        let s3_config = if let Some(url) =
            t_type::<String>(&mut map, t_name, "s3_url", "HQL_S3_URL")?
        {
            // we expect all values to exist when we can read the url successfully

            let bucket = t_type::<String>(&mut map, t_name, "s3_bucket", "HQL_S3_BUCKET")?.ok_or(
                Error::String("Missing config variable `s3_bucket`".to_string()),
            )?;
            let region = t_type::<String>(&mut map, t_name, "s3_region", "HQL_S3_REGION")?.ok_or(
                Error::String("Missing config variable `s3_region`".to_string()),
            )?;
            let path_style =
                t_type::<bool>(&mut map, t_name, "s3_path_style", "HQL_S3_PATH_STYLE")?
                    .unwrap_or(true);

            let key = t_type::<String>(&mut map, t_name, "s3_key", "HQL_S3_KEY")?.ok_or(
                Error::String("Missing config variable `s3_key`".to_string()),
            )?;
            let secret = t_type::<String>(&mut map, t_name, "s3_secret", "HQL_S3_SECRET")?.ok_or(
                Error::String("Missing config variable `s3_secret`".to_string()),
            )?;

            let config = crate::s3::S3Config::new(&url, bucket, region, key, secret, path_style)
                .map_err(|err| {
                    Error::String(format!(
                        "Cannot build S3Config from given S3 values in {t_name}: {err:?}"
                    ))
                })?;
            Some(config)
        } else {
            None
        };

        #[cfg(feature = "dashboard")]
        let password_dashboard = match t_type::<String>(
            &mut map,
            t_name,
            "password_dashboard",
            "HQL_PASSWORD_DASHBOARD",
        )? {
            Some(password_dashboard_b64) => {
                let password_dashboard_vec_u8 = cryptr::utils::b64_decode(&password_dashboard_b64)
                    .map_err(|_| {
                        Error::String("password_dashboard must be valid base64.".to_string())
                    })?;
                Some(String::from_utf8(password_dashboard_vec_u8).map_err(|_| {
                    Error::String(
                        "password_dashboard must contain String characters only".to_string(),
                    )
                })?)
            }
            None => None,
        };
        #[cfg(feature = "dashboard")]
        let insecure_cookie =
            t_type::<bool>(&mut map, t_name, "insecure_cookie", "HQL_INSECURE_COOKIE")?
                .unwrap_or(false);

        #[cfg(any(feature = "s3", feature = "dashboard"))]
        let enc_keys = if let Some(keys) = enc_keys {
            keys
        } else {
            let enc_key_active =
                t_type::<String>(&mut map, t_name, "enc_key_active", "ENC_KEY_ACTIVE")?.ok_or(
                    Error::String(format!("{t_name}.enc_key_active is a mandatory value")),
                )?;
            let enc_keys = t_str_vec(&mut map, t_name, "enc_keys", "ENC_KEYS")?.ok_or(
                Error::String(format!("{t_name}.enc_keys is a mandatory value")),
            )?;
            cryptr::EncKeys::try_parse(enc_key_active, enc_keys)?
        };

        Ok(NodeConfig {
            node_id,
            nodes,
            listen_addr_api,
            listen_addr_raft,
            data_dir,
            filename_db,
            log_statements,
            prepared_statement_cache_capacity,
            read_pool_size,
            wal_sync,
            wal_size,
            #[cfg(feature = "cache")]
            cache_storage_disk,
            raft_config: NodeConfig::default_raft_config(logs_until_snapshot),
            tls_raft,
            tls_api,
            secret_raft,
            secret_api,
            #[cfg(any(feature = "s3", feature = "dashboard"))]
            enc_keys,
            #[cfg(feature = "backup")]
            backup_config,
            #[cfg(feature = "backup")]
            backup_keep_days_local,
            #[cfg(feature = "s3")]
            s3_config,
            #[cfg(feature = "dashboard")]
            password_dashboard,
            #[cfg(feature = "dashboard")]
            insecure_cookie,
            health_check_delay_secs,
        })
    }
}

fn t_type<T>(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<T>, Error>
where
    T: FromStr,
{
    if env_var.is_empty() {
        let value: T = map
            .remove(key)
            .and_then(|v| v.as_str().and_then(|v| v.parse::<T>().ok()))
            .ok_or(Error::String(err_t(key, parent, type_name::<T>())))?;
        return Ok(Some(value));
    }

    let value: T = env::var(env_var)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .ok_or(Error::String(err_env(env_var, type_name::<T>())))?;
    Ok(Some(value))
}

fn t_str_vec(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<Vec<String>>, Error> {
    if env_var.is_empty() {
        let Value::Array(arr) = map
            .remove(key)
            .ok_or(Error::String(err_t(key, parent, "Array")))?
        else {
            return Ok(None);
        };
        let mut res = Vec::with_capacity(arr.len());
        for value in arr {
            let Value::String(s) = value else {
                return Err(err_t(key, parent, "String").into());
            };
            res.push(s);
        }
        return Ok(Some(res));
    }

    let arr: String = env::var(env_var).map_err(|_| Error::String(err_env(env_var, "String")))?;
    Ok(Some(
        arr.lines()
            .filter_map(|l| {
                let trimmed = l.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
            .collect(),
    ))
}

fn t_table(map: &mut toml::Table, key: &str) -> Result<toml::Table, Error> {
    let value = map
        .remove(key)
        .ok_or(Error::String(format!("Expected type `Table` for {key}")))?;
    toml::Table::try_from(value)
        .map_err(|err| Error::String(format!("Cannot build toml table from removed value: {err}")))
}

#[inline]
fn err_env(var_name: &str, typ: &str) -> String {
    format!("Cannot parse {var_name} as `{typ}`")
}

#[inline]
fn err_t(key: &str, parent: &str, typ: &str) -> String {
    let sep = if parent.is_empty() { "" } else { "." };
    format!("Expected type `{typ}` for {parent}{sep}{key}")
}
