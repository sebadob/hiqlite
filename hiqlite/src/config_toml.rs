use crate::config::RateLimitConfig;
use crate::tls::{ServerTlsConfig, ServerTlsConfigCerts};
use crate::{Error, Node, NodeConfig};
use hiqlite_wal::LogSync;
use std::borrow::Cow;
use std::env;
use tokio::fs;
use toml::Value;

impl NodeConfig {
    /// Tries to read the `NodeConfig` from the given toml file path. Will use default values for
    /// all non-existing keys. You can define a custom `table` to read from. If none, expects the
    /// data to be in `[hiqlite]`.
    ///
    /// You can overwrite most values from the file with ENV vars. If this is possible, it is
    /// mentioned in the documentation for each value.
    ///
    /// ## Secrets
    ///
    /// Secret-bearing values (`secret_raft`, `secret_api`, `s3_key`, `s3_secret`, `enc_keys`,
    /// `enc_key_active`, `password_dashboard`) may be set to the case-sensitive sentinel
    /// `"$SECRETS"`. In that case, the real value is looked up by the same key in a separate
    /// secrets source. This keeps the main config diffable and version-controllable while the
    /// secrets are managed separately (systemd `LoadCredential`, Docker / Kubernetes secrets, ...).
    ///
    /// The secrets source is, in order of precedence:
    /// 1. the `secrets` table passed in here, if `Some`;
    /// 2. otherwise a `secrets_file` config option (or the `HQL_SECRETS_FILE` env var) pointing to
    ///    a TOML file that mirrors the config structure (i.e. holds the same `[{table}]` table).
    pub async fn from_toml(
        path: &str,
        table: Option<&str>,
        secrets: Option<toml::Table>,
        #[cfg(any(feature = "s3", feature = "dashboard"))] enc_keys: Option<cryptr::EncKeys>,
    ) -> Result<Self, Error> {
        dotenvy::dotenv().ok();

        let t_name = table.unwrap_or("hiqlite");

        let config = fs::read_to_string(path)
            .await
            .map_err(|err| Error::config(format!("Cannot read config file from: {path}: {err}")))?;

        // Note: these inner parsers are very verbose, but they allow the upfront memory allocation
        // and memory fragmentation, after the quite big toml has been freed and the config stays
        // in static memory.

        let mut root = config
            .parse::<toml::Table>()
            .map_err(|err| Error::config(format!("Cannot parse TOML file: {err}")))?;
        let table = t_table(&mut root, t_name).map_err(|err| {
            Error::config(format!("Cannot find table '{t_name}' in {path}: {err}"))
        })?;

        Self::from_toml_table(
            table,
            t_name,
            secrets,
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
    ///
    /// See [`NodeConfig::from_toml`] for how the `secrets` table and the `"$SECRETS"` sentinel
    /// work. If `secrets` is `None`, a `secrets_file` config option (or `HQL_SECRETS_FILE`) is
    /// honored as a fallback.
    pub async fn from_toml_table(
        table: toml::Table,
        table_name: &str,
        secrets: Option<toml::Table>,
        #[cfg(any(feature = "s3", feature = "dashboard"))] enc_keys: Option<cryptr::EncKeys>,
    ) -> Result<Self, Error> {
        dotenvy::dotenv().ok();

        let t_name = table_name;
        let mut map = table;

        // Resolve the optional secrets source used by the `$SECRETS` sentinel. An explicitly
        // passed-in `secrets` table wins; otherwise a `secrets_file` path (or `HQL_SECRETS_FILE`)
        // is loaded, which must mirror the config structure (i.e. hold the same `[{t_name}]`
        // table). The `secrets_file` value is only read when no `secrets` table was passed in.
        let secrets_owned = match secrets {
            Some(secrets) => Some(secrets),
            None => match t_str(&mut map, t_name, "secrets_file", "HQL_SECRETS_FILE")? {
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

        let node_id = if let Some(v) = t_str(&mut map, t_name, "node_id_from", "HQL_NODE_ID_FROM")?
        {
            if v == "k8s" {
                Self::node_id_from_hostname()
            } else {
                t_u64(&mut map, t_name, "node_id", "HQL_NODE_ID")?.unwrap_or(0)
            }
        } else {
            t_u64(&mut map, t_name, "node_id", "HQL_NODE_ID")?.unwrap_or(0)
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

        let listen_addr_api = t_str(&mut map, t_name, "listen_addr_api", "HQL_LISTEN_ADDR_API")?
            .map(Cow::from)
            .unwrap_or_else(|| "0.0.0.0".into());
        let listen_addr_raft = t_str(&mut map, t_name, "listen_addr_raft", "HQL_LISTEN_ADDR_RAFT")?
            .map(Cow::from)
            .unwrap_or_else(|| "0.0.0.0".into());

        let data_dir = t_str(&mut map, t_name, "data_dir", "HQL_DATA_DIR")?
            .map(Cow::from)
            .unwrap_or_else(|| "data".into());
        let filename_db = t_str(&mut map, t_name, "filename_db", "HQL_FILENAME_DB")?
            .map(Cow::from)
            .unwrap_or_else(|| "hiqlite.db".into());
        let log_statements =
            t_bool(&mut map, t_name, "log_statements", "HQL_LOG_STATEMENTS")?.unwrap_or(false);
        let prepared_statement_cache_capacity =
            t_u16(&mut map, t_name, "prepared_statement_cache_capacity", "")?.unwrap_or(1000)
                as usize;
        let read_pool_size =
            t_u16(&mut map, t_name, "read_pool_size", "HQL_READ_POOL_SIZE")?.unwrap_or(4) as usize;

        let wal_sync = if let Some(v) = t_str(&mut map, t_name, "log_sync", "HQL_LOG_SYNC")? {
            let Ok(sync) = LogSync::try_from(v.as_str()) else {
                return Err(Error::config(err_t("log_sync", t_name, "LogSync")));
            };
            sync
        } else {
            LogSync::ImmediateAsync
        };
        let wal_size =
            t_u32(&mut map, t_name, "wal_size", "HQL_WAL_SIZE")?.unwrap_or(2 * 1024 * 1024);

        #[cfg(feature = "cache")]
        let cache_storage_disk = t_bool(
            &mut map,
            t_name,
            "cache_storage_disk",
            "HQL_CACHE_STORAGE_DISK",
        )?
        .unwrap_or(true);

        let logs_until_snapshot = t_u64(
            &mut map,
            t_name,
            "logs_until_snapshot",
            "HQL_LOGS_UNTIL_SNAPSHOT",
        )?
        .unwrap_or(10_000);

        let tls_auto_certificates = t_bool(
            &mut map,
            t_name,
            "tls_auto_certificates",
            "HQL_TLS_AUTO_CERTS",
        )?
        .unwrap_or(false);

        let tls_raft_key = t_str(&mut map, t_name, "tls_raft_key", "HQL_TLS_RAFT_KEY")?;
        let tls_raft_cert = t_str(&mut map, t_name, "tls_raft_cert", "HQL_TLS_RAFT_CERT")?;
        let tls_raft_danger_tls_no_verify =
            t_bool(&mut map, t_name, "tls_raft_danger_tls_no_verify", "")?.unwrap_or(false);
        #[allow(clippy::unnecessary_unwrap)]
        let tls_raft = if tls_raft_key.is_some() && tls_raft_cert.is_some() {
            Some(ServerTlsConfig::Specific(ServerTlsConfigCerts {
                key: tls_raft_key.unwrap().into(),
                cert: tls_raft_cert.unwrap().into(),
                danger_tls_no_verify: tls_raft_danger_tls_no_verify,
            }))
        } else if tls_auto_certificates {
            Some(ServerTlsConfig::TlsAutoCertificates)
        } else {
            None
        };

        let tls_api_key = t_str(&mut map, t_name, "tls_api_key", "HQL_TLS_API_KEY")?;
        let tls_api_cert = t_str(&mut map, t_name, "tls_api_cert", "HQL_TLS_API_CERT")?;
        let tls_api_danger_tls_no_verify =
            t_bool(&mut map, t_name, "tls_raft_danger_tls_no_verify", "")?.unwrap_or(false);
        #[allow(clippy::unnecessary_unwrap)]
        let tls_api = if tls_api_key.is_some() && tls_api_cert.is_some() {
            Some(ServerTlsConfig::Specific(ServerTlsConfigCerts {
                key: tls_api_key.unwrap().into(),
                cert: tls_api_cert.unwrap().into(),
                danger_tls_no_verify: tls_api_danger_tls_no_verify,
            }))
        } else if tls_auto_certificates {
            Some(ServerTlsConfig::TlsAutoCertificates)
        } else {
            None
        };

        let Some(secret_raft) =
            t_str_secret(&mut map, t_name, "secret_raft", "HQL_SECRET_RAFT", secrets)?
        else {
            return Err(Error::config(format!(
                "{t_name}.secret_raft is a mandatory value"
            )));
        };
        let Some(secret_api) =
            t_str_secret(&mut map, t_name, "secret_api", "HQL_SECRET_API", secrets)?
        else {
            return Err(Error::config(format!(
                "{t_name}.secret_api is a mandatory value"
            )));
        };

        let health_check_delay_secs =
            t_u32(&mut map, t_name, "health_check_delay_secs", "")?.unwrap_or(30);
        let learner_only =
            t_bool(&mut map, t_name, "learner_only", "HQL_LEARNER_ONLY")?.unwrap_or(false);

        #[cfg(feature = "backup")]
        let (backup_config, backup_keep_days_local) = {
            let backup_cron =
                if let Some(v) = t_str(&mut map, t_name, "backup_cron", "HQL_BACKUP_CRON")? {
                    Cow::from(v)
                } else {
                    Cow::from("0 30 2 * * * *")
                };
            let backup_keep_days =
                t_u16(&mut map, t_name, "backup_keep_days", "HQL_BACKUP_KEEP_DAYS")?.unwrap_or(30);
            let backup_keep_days_local = t_u16(
                &mut map,
                t_name,
                "backup_keep_days_local",
                "HQL_BACKUP_KEEP_DAYS_LOCAL",
            )?
            .unwrap_or(30);

            let backup_config =
                crate::backup::BackupConfig::new(backup_cron.as_ref(), backup_keep_days)
                    .map_err(|err| Error::config(format!("Error building BackupConfig: {err}")))?;
            (backup_config, backup_keep_days_local)
        };

        #[cfg(feature = "s3")]
        let s3_config = if let Some(url) = t_str(&mut map, t_name, "s3_url", "HQL_S3_URL")? {
            // we expect all values to exist when we can read the url successfully

            let bucket = t_str(&mut map, t_name, "s3_bucket", "HQL_S3_BUCKET")?.ok_or(
                Error::config("Missing config variable `s3_bucket`".to_string()),
            )?;
            let region = t_str(&mut map, t_name, "s3_region", "HQL_S3_REGION")?.ok_or(
                Error::config("Missing config variable `s3_region`".to_string()),
            )?;
            let path_style =
                t_bool(&mut map, t_name, "s3_path_style", "HQL_S3_PATH_STYLE")?.unwrap_or(true);

            let key = t_str_secret(&mut map, t_name, "s3_key", "HQL_S3_KEY", secrets)?.ok_or(
                Error::config("Missing config variable `s3_key`".to_string()),
            )?;
            let secret = t_str_secret(&mut map, t_name, "s3_secret", "HQL_S3_SECRET", secrets)?
                .ok_or(Error::config(
                    "Missing config variable `s3_secret`".to_string(),
                ))?;

            let config = crate::s3::S3Config::new(&url, bucket, region, key, secret, path_style)
                .map_err(|err| {
                    Error::config(format!(
                        "Cannot build S3Config from given S3 values in {t_name}: {err:?}"
                    ))
                })?;
            Some(config)
        } else {
            None
        };

        #[cfg(feature = "dashboard")]
        let password_dashboard = match t_str_secret(
            &mut map,
            t_name,
            "password_dashboard",
            "HQL_PASSWORD_DASHBOARD",
            secrets,
        )? {
            Some(password_dashboard_b64) => {
                let password_dashboard_vec_u8 = cryptr::utils::b64_decode(&password_dashboard_b64)
                    .map_err(|_| {
                        Error::config("password_dashboard must be valid base64.".to_string())
                    })?;
                Some(String::from_utf8(password_dashboard_vec_u8).map_err(|_| {
                    Error::config(
                        "password_dashboard must contain String characters only".to_string(),
                    )
                })?)
            }
            None => None,
        };
        #[cfg(feature = "dashboard")]
        let insecure_cookie =
            t_bool(&mut map, t_name, "insecure_cookie", "HQL_INSECURE_COOKIE")?.unwrap_or(false);

        #[cfg(any(feature = "s3", feature = "dashboard"))]
        let enc_keys = if let Some(keys) = enc_keys {
            keys
        } else {
            let enc_key_active = t_str_secret(
                &mut map,
                t_name,
                "enc_key_active",
                "ENC_KEY_ACTIVE",
                secrets,
            )?
            .ok_or(Error::config(format!(
                "{t_name}.enc_key_active is a mandatory value"
            )))?;
            let enc_keys = t_str_vec_secret(&mut map, t_name, "enc_keys", "ENC_KEYS", secrets)?
                .ok_or(Error::config(format!(
                    "{t_name}.enc_keys is a mandatory value"
                )))?;
            cryptr::EncKeys::try_parse(enc_key_active, enc_keys)?
        };

        #[cfg(feature = "cache")]
        let rate_limit_cache = {
            if let Some(rps) = t_u32(&mut map, t_name, "rate_limit_cache_rps", "HQL_RL_CACHE_RPS")?
            {
                Some(RateLimitConfig {
                    rps,
                    burst: t_u32(
                        &mut map,
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

        #[cfg(feature = "sqlite")]
        let rate_limit_db = {
            if let Some(rps) = t_u32(&mut map, t_name, "rate_limit_db_rps", "HQL_RL_DB_RPS")? {
                Some(RateLimitConfig {
                    rps,
                    burst: t_u32(&mut map, t_name, "rate_limit_db_burst", "HQL_RL_DB_BURST")?
                        .unwrap_or(rps),
                })
            } else {
                None
            }
        };

        check_empty(map, table_name)?;

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
            learner_only,
            #[cfg(feature = "cache")]
            rate_limit_cache,
            #[cfg(feature = "sqlite")]
            rate_limit_db,
        })
    }
}

fn check_empty(table: toml::Table, tbl_name: &str) -> Result<(), Error> {
    if table.is_empty() {
        Ok(())
    } else {
        // It may be the case that we found values that belong to not-activated features.
        // We should not error on these.
        let mut err = false;
        for key in table.keys() {
            if ![
                "backup_cron",
                "backup_keep_days",
                "backup_keep_days_local",
                "cache_storage_disk",
                "s3_url",
                "s3_bucket",
                "s3_region",
                "s3_path_style",
                "s3_key",
                "s3_secret",
                "password_dashboard",
                "insecure_cookie",
                "enc_key_active",
                "enc_keys",
                "rate_limit_cache_rps",
                "rate_limit_cache_burst",
                "rate_limit_db_rps",
                "rate_limit_db_burst",
            ]
            .contains(&key.as_str())
            {
                err = true;
                break;
            }
        }

        if err {
            Err(Error::Config(
                format!("Unknown Config data in section: '{tbl_name}': {table:?}").into(),
            ))
        } else {
            Ok(())
        }
    }
}

fn t_bool(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<bool>, Error> {
    let value = map.remove(key);

    if !env_var.is_empty()
        && let Ok(v) = env::var(env_var)
    {
        return match v.parse::<bool>() {
            Ok(b) => Ok(Some(b)),
            Err(_) => Err(Error::config(err_t(key, parent, "bool"))),
        };
    }

    if let Some(v) = value {
        let Value::Boolean(b) = v else {
            return Err(Error::config(err_t(key, parent, "bool")));
        };
        Ok(Some(b))
    } else {
        Ok(None)
    }
}

fn t_i64(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<i64>, Error> {
    let value = map.remove(key);

    if !env_var.is_empty()
        && let Ok(v) = env::var(env_var)
    {
        return match v.parse::<i64>() {
            Ok(i) => Ok(Some(i)),
            Err(_) => Err(Error::config(err_t(key, parent, "i64"))),
        };
    }

    if let Some(v) = value {
        let Value::Integer(i) = v else {
            return Err(Error::config(err_t(key, parent, "i64")));
        };
        Ok(Some(i))
    } else {
        Ok(None)
    }
}

fn t_u64(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<u64>, Error> {
    if let Ok(Some(v)) = t_i64(map, parent, key, env_var) {
        if v < 0 {
            return Err(Error::config(err_t(key, parent, "u64")));
        }
        Ok(Some(v as u64))
    } else {
        Ok(None)
    }
}

fn t_u32(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<u32>, Error> {
    if let Ok(Some(v)) = t_i64(map, parent, key, env_var) {
        if v < 0 || v > u32::MAX as i64 {
            return Err(Error::config(err_t(key, parent, "u32")));
        }
        Ok(Some(v as u32))
    } else {
        Ok(None)
    }
}
fn t_u16(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<u16>, Error> {
    if let Ok(Some(v)) = t_i64(map, parent, key, env_var) {
        if v < 0 || v > u16::MAX as i64 {
            return Err(Error::config(err_t(key, parent, "u16")));
        }
        Ok(Some(v as u16))
    } else {
        Ok(None)
    }
}

fn t_str(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<String>, Error> {
    let value = map.remove(key);

    if !env_var.is_empty()
        && let Ok(v) = env::var(env_var)
    {
        return Ok(Some(v));
    }

    if let Some(v) = value {
        let Value::String(s) = v else {
            return Err(Error::config(err_t(key, parent, "String")));
        };
        Ok(Some(s))
    } else {
        Ok(None)
    }
}

fn t_str_vec(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
) -> Result<Option<Vec<String>>, Error> {
    let value = map.remove(key);

    if !env_var.is_empty()
        && let Ok(arr) = env::var(env_var)
    {
        return Ok(Some(
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
        ));
    }

    let Some(Value::Array(arr)) = value else {
        return Ok(None);
    };
    let mut res = Vec::with_capacity(arr.len());
    for value in arr {
        let Value::String(s) = value else {
            return Err(Error::config(err_t(key, parent, "String")));
        };
        res.push(s);
    }
    Ok(Some(res))
}

/// Case-sensitive sentinel marking a config value whose real content lives in the
/// separate secrets source (see `NodeConfig::from_toml`).
const SECRETS_REF: &str = "$SECRETS";

/// Like `t_str`, but resolves the `$SECRETS` sentinel against the optional `secrets` table,
/// looking the real value up by the same `key`. Per-var error messages are preserved.
fn t_str_secret(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
    secrets: Option<&toml::Table>,
) -> Result<Option<String>, Error> {
    match t_str(map, parent, key, env_var)? {
        Some(v) if v == SECRETS_REF => match secrets {
            None => Err(Error::config(format!(
                "{parent}.{key}: `{SECRETS_REF}` reference but no secrets file configured"
            ))),
            Some(secrets) => match secrets.get(key) {
                Some(Value::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(Error::config(err_t(key, parent, "String"))),
                None => Err(Error::config(format!(
                    "{parent}.{key}: `{SECRETS_REF}` set but `{key}` not found in secrets file"
                ))),
            },
        },
        other => Ok(other),
    }
}

/// Like `t_str_vec`, but resolves the `$SECRETS` sentinel. The sentinel is written as a single
/// string (e.g. `enc_keys = "$SECRETS"`); the real value is then looked up by the same `key` in
/// the `secrets` table, where it must be an array of strings.
#[cfg(any(feature = "s3", feature = "dashboard"))]
fn t_str_vec_secret(
    map: &mut toml::Table,
    parent: &str,
    key: &str,
    env_var: &str,
    secrets: Option<&toml::Table>,
) -> Result<Option<Vec<String>>, Error> {
    let value = map.remove(key);

    if !env_var.is_empty()
        && let Ok(arr) = env::var(env_var)
    {
        if arr == SECRETS_REF {
            return secret_vec_lookup(parent, key, secrets);
        }
        return Ok(Some(
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
        ));
    }

    match value {
        Some(Value::String(s)) if s == SECRETS_REF => secret_vec_lookup(parent, key, secrets),
        Some(Value::Array(arr)) => {
            let mut res = Vec::with_capacity(arr.len());
            for value in arr {
                let Value::String(s) = value else {
                    return Err(Error::config(err_t(key, parent, "String")));
                };
                res.push(s);
            }
            Ok(Some(res))
        }
        _ => Ok(None),
    }
}

/// Looks up a string-array secret by `key` in the `secrets` table for the `$SECRETS` sentinel.
#[cfg(any(feature = "s3", feature = "dashboard"))]
fn secret_vec_lookup(
    parent: &str,
    key: &str,
    secrets: Option<&toml::Table>,
) -> Result<Option<Vec<String>>, Error> {
    let Some(secrets) = secrets else {
        return Err(Error::config(format!(
            "{parent}.{key}: `{SECRETS_REF}` reference but no secrets file configured"
        )));
    };
    match secrets.get(key) {
        Some(Value::Array(arr)) => {
            let mut res = Vec::with_capacity(arr.len());
            for value in arr {
                let Value::String(s) = value else {
                    return Err(Error::config(err_t(key, parent, "String")));
                };
                res.push(s.clone());
            }
            Ok(Some(res))
        }
        Some(_) => Err(Error::config(err_t(key, parent, "String"))),
        None => Err(Error::config(format!(
            "{parent}.{key}: `{SECRETS_REF}` set but `{key}` not found in secrets file"
        ))),
    }
}

fn t_table(map: &mut toml::Table, key: &str) -> Result<toml::Table, Error> {
    let value = map
        .remove(key)
        .ok_or(Error::config(format!("Expected type `Table` for {key}")))?;
    toml::Table::try_from(value)
        .map_err(|err| Error::config(format!("Cannot build toml table from removed value: {err}")))
}

#[inline]
fn err_t(key: &str, parent: &str, typ: &str) -> String {
    let sep = if parent.is_empty() { "" } else { "." };
    format!("Expected type `{typ}` for {parent}{sep}{key}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(s: &str) -> toml::Table {
        s.parse::<toml::Table>().unwrap()
    }

    #[test]
    fn t_str_secret_passes_through_plain_value() {
        let mut map = table(r#"secret_raft = "plain1234""#);
        let out = t_str_secret(&mut map, "hiqlite", "secret_raft", "", None).unwrap();
        assert_eq!(out, Some("plain1234".to_string()));
    }

    #[test]
    fn t_str_secret_resolves_sentinel_from_table() {
        let mut map = table(r#"secret_raft = "$SECRETS""#);
        let secrets = table(r#"secret_raft = "real-raft-secret""#);
        let out = t_str_secret(&mut map, "hiqlite", "secret_raft", "", Some(&secrets)).unwrap();
        assert_eq!(out, Some("real-raft-secret".to_string()));
    }

    #[test]
    fn t_str_secret_sentinel_without_secrets_errors() {
        let mut map = table(r#"secret_raft = "$SECRETS""#);
        let err = t_str_secret(&mut map, "hiqlite", "secret_raft", "", None).unwrap_err();
        assert!(err.to_string().contains("no secrets file"));
    }

    #[test]
    fn t_str_secret_sentinel_missing_key_errors() {
        let mut map = table(r#"secret_raft = "$SECRETS""#);
        let secrets = table(r#"secret_api = "other""#);
        let err = t_str_secret(&mut map, "hiqlite", "secret_raft", "", Some(&secrets)).unwrap_err();
        assert!(err.to_string().contains("not found in secrets file"));
    }

    #[test]
    fn t_str_secret_sentinel_wrong_type_errors() {
        let mut map = table(r#"secret_raft = "$SECRETS""#);
        let secrets = table("secret_raft = 1234");
        let err = t_str_secret(&mut map, "hiqlite", "secret_raft", "", Some(&secrets)).unwrap_err();
        assert!(err.to_string().contains("Expected type `String`"));
    }

    #[cfg(any(feature = "s3", feature = "dashboard"))]
    #[test]
    fn t_str_vec_secret_passes_through_array() {
        let mut map = table(r#"enc_keys = ["a", "b"]"#);
        let out = t_str_vec_secret(&mut map, "hiqlite", "enc_keys", "", None).unwrap();
        assert_eq!(out, Some(vec!["a".to_string(), "b".to_string()]));
    }

    #[cfg(any(feature = "s3", feature = "dashboard"))]
    #[test]
    fn t_str_vec_secret_resolves_sentinel_from_table() {
        let mut map = table(r#"enc_keys = "$SECRETS""#);
        let secrets = table(r#"enc_keys = ["key1/abc", "key2/def"]"#);
        let out = t_str_vec_secret(&mut map, "hiqlite", "enc_keys", "", Some(&secrets)).unwrap();
        assert_eq!(
            out,
            Some(vec!["key1/abc".to_string(), "key2/def".to_string()])
        );
    }

    #[cfg(any(feature = "s3", feature = "dashboard"))]
    #[test]
    fn t_str_vec_secret_sentinel_without_secrets_errors() {
        let mut map = table(r#"enc_keys = "$SECRETS""#);
        let err = t_str_vec_secret(&mut map, "hiqlite", "enc_keys", "", None).unwrap_err();
        assert!(err.to_string().contains("no secrets file"));
    }

    #[cfg(any(feature = "s3", feature = "dashboard"))]
    #[test]
    fn t_str_vec_secret_sentinel_missing_key_errors() {
        let mut map = table(r#"enc_keys = "$SECRETS""#);
        let secrets = table(r#"secret_raft = "x""#);
        let err =
            t_str_vec_secret(&mut map, "hiqlite", "enc_keys", "", Some(&secrets)).unwrap_err();
        assert!(err.to_string().contains("not found in secrets file"));
    }

    // Integration: the `secrets_file` loading path runs before any secret parsing, so these
    // error branches are deterministic regardless of ambient `HQL_*` env vars. The successful
    // sentinel resolution itself is covered by the `t_str*_secret_*` tests above.
    #[cfg(any(feature = "s3", feature = "dashboard"))]
    #[tokio::test]
    async fn from_toml_table_missing_secrets_file_errors() {
        let table = "secrets_file = \"/nonexistent/hiqlite-secrets-does-not-exist.toml\"\n"
            .parse::<toml::Table>()
            .unwrap();
        let err = NodeConfig::from_toml_table(table, "hiqlite", None, None)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Cannot read secrets file"),
            "unexpected error: {err}"
        );
    }

    #[cfg(any(feature = "s3", feature = "dashboard"))]
    #[tokio::test]
    async fn from_toml_table_secrets_file_wrong_structure_errors() {
        // The secrets file must mirror the config structure (a `[hiqlite]` table here).
        let secrets_path = std::env::temp_dir().join("hiqlite_test_secrets_wrong_structure.toml");
        tokio::fs::write(&secrets_path, "secret_raft = \"x\"\n")
            .await
            .unwrap();
        let cfg = format!("secrets_file = \"{}\"\n", secrets_path.display());
        let table = cfg.parse::<toml::Table>().unwrap();

        let err = NodeConfig::from_toml_table(table, "hiqlite", None, None)
            .await
            .unwrap_err();
        let _ = tokio::fs::remove_file(&secrets_path).await;
        assert!(
            err.to_string().contains("Cannot find table 'hiqlite'"),
            "unexpected error: {err}"
        );
    }
}
