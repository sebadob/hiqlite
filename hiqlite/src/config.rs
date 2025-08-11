use crate::tls::ServerTlsConfig;
use crate::{Error, Node, NodeId};
use openraft::SnapshotPolicy;
use std::borrow::Cow;
use std::env;
use tracing::{debug, warn};

#[cfg(feature = "backup")]
use crate::backup;

#[cfg(feature = "dashboard")]
use crate::dashboard::DashboardState;

pub use openraft::Config as RaftConfig;

/// The main Node config.
///
/// Most default values are good for internal, fast networks. If you have a slow or unstable
/// network, you might want to tune the `RaftConfig`. However, you should never adjust the
/// `max_in_snapshot_log_to_keep`, because this will play a crucial role if you need to restore
/// from a backup in case of disaster recovery.
#[derive(Debug)]
pub struct NodeConfig {
    /// The `node_id` defines which entry from the `nodes` is "this node"
    pub node_id: NodeId,
    /// All Raft member nodes
    pub nodes: Vec<Node>,
    /// Listen address for the API Server
    pub listen_addr_api: Cow<'static, str>,
    /// Listen address for the Raft Server
    pub listen_addr_raft: Cow<'static, str>,
    /// The directory where the replication log, database and snapshots should be stored
    pub data_dir: Cow<'static, str>,
    /// If the SQLite should be written to disk, provide a filename here.
    /// It is recommended to leave it set to None if your DB size fits fully into memory, and
    /// you can afford this. No data will be lost with an in-memory DB because Raft logs and
    /// snapshots are always persisted and the in-memory DB can be rebuilt quickly after a restart.
    pub filename_db: Cow<'static, str>,
    /// Enables statement logging or the SQL writer
    pub log_statements: bool,
    /// The internal cache size for prepared statements. The default is `1024` which could be
    /// reduced in very heavily memory-constrained environments.
    pub prepared_statement_cache_capacity: usize,
    /// The size of the pooled connections for local database reads.
    ///
    /// Do not confuse this with a pool size for network databases, as it
    /// is much more efficient. You can't really translate between them,
    /// because it depends on many things, but assuming a factor of 10 is
    /// a good start. This means, if you needed a (read) pool size of 40
    /// connections for something like a postgres before, you should start
    /// at a `read_pool_size` of 4.
    ///
    /// Keep in mind that this pool is only used for reads and writes will
    /// travel through the Raft and have their own dedicated connection.
    ///
    /// default: 4
    pub read_pool_size: usize,
    /// When Raft logs should by synced to disk.
    pub wal_sync: hiqlite_wal::LogSync,
    /// Maximum WAL size in bytes.
    pub wal_size: u32,
    /// Set to `true` to store the cache WAL + Snapshots on disk instead of keeping them in memory.
    /// The Caches themselves will always be in-memory only. The default is `true`, which will
    /// effectively reduce the total memory used, because otherwise the WAL + Snapshot in memory
    /// would be duplicate data. WAL + Snapshots on disk should basically always be used, because
    /// it does not require a complete cluster re-join and snapshot + WAL sync after a restart, and
    /// make the Cache persistent, as then in-memory Caches will be rebuilt from disk.
    ///
    /// Keep in mind that the in-memory WAL storage is roughly 4 times faster than the one on disk,
    /// even with memory mapped WAL files (depending on your disk of course).
    ///
    /// CAUTION: There is a known bug in the Raft that can lead to a Raft cluster lock up after
    /// a pure in-memory member (value set to `false`) crashes or is being force-killed, before it
    /// had a chance to leave the cluster cleanly before shutdown! This bug will be fixed in the
    /// future.
    #[cfg(feature = "cache")]
    pub cache_storage_disk: bool,
    /// The internal Raft config. This must be the same on each node.
    /// You will get good defaults with `NodeConfig::default_raft_config(_)`.
    pub raft_config: RaftConfig,
    /// If RPC and HTTP connections should use TLS
    pub tls_raft: Option<ServerTlsConfig>,
    pub tls_api: Option<ServerTlsConfig>,
    /// Secret for all Raft internal messages - at least 16 characters long
    pub secret_raft: String,
    /// Secret for Raft management and DB API - at least 16 characters long
    pub secret_api: String,
    /// The Encryption Keys used for Backups and Dashboard cookies
    #[cfg(any(feature = "s3", feature = "dashboard"))]
    pub enc_keys: cryptr::EncKeys,
    /// auto-backup configuration
    #[cfg(feature = "backup")]
    pub backup_config: backup::BackupConfig,
    #[cfg(feature = "backup")]
    pub backup_keep_days_local: u16,
    /// If an `S3Config` is given, it will be used to push backups to the S3 bucket. feature `s3`
    #[cfg(feature = "s3")]
    pub s3_config: Option<std::sync::Arc<crate::s3::S3Config>>,
    /// Set the password for the integrated dashboard. Must be given as argon2id hash. feature `dashboard`
    #[cfg(feature = "dashboard")]
    pub password_dashboard: Option<String>,
    /// If `true`, insecure cookies will be created for the Dashboard. NEVER use this in production,
    /// only for local testing!
    #[cfg(feature = "dashboard")]
    pub insecure_cookie: bool,
    /// Artificial shutdown delay for a multi-node deployment. This should be at least:
    /// `raft_config.election_timeout_max + raft_config.heartbeat_interval`
    /// You may want to increase it in case you also use a bigger cache size and need a bit more
    /// headroom for replications during rolling releases.
    pub shutdown_delay_millis: u32,
    /// The initial delay until which the "true" health will be returned. A delay at the start is
    /// necessary to solve a chicken-and-egg problem when cold starting cluster which depend on
    /// health checks.
    pub health_check_delay_secs: u32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: 0,
            nodes: vec![],
            listen_addr_api: "0.0.0.0".into(),
            listen_addr_raft: "0.0.0.0".into(),
            data_dir: "hiqlite".into(),
            filename_db: "hiqlite.db".into(),
            log_statements: false,
            prepared_statement_cache_capacity: 1024,
            read_pool_size: 4,
            wal_sync: hiqlite_wal::LogSync::ImmediateAsync,
            wal_size: 2 * 1024 * 1024,
            #[cfg(feature = "cache")]
            cache_storage_disk: true,
            raft_config: Self::default_raft_config(10_000),
            tls_raft: None,
            tls_api: None,
            secret_raft: String::default(),
            secret_api: String::default(),
            #[cfg(any(feature = "s3", feature = "dashboard"))]
            enc_keys: Default::default(),
            #[cfg(feature = "backup")]
            backup_config: backup::BackupConfig::default(),
            #[cfg(feature = "backup")]
            backup_keep_days_local: 30,
            #[cfg(feature = "s3")]
            s3_config: None,
            #[cfg(feature = "dashboard")]
            password_dashboard: None,
            #[cfg(feature = "dashboard")]
            insecure_cookie: false,
            shutdown_delay_millis: 5000,
            health_check_delay_secs: 30,
            // #[cfg(feature = "dashboard")]
            // insecure_cookie: false,
        }
    }
}

impl NodeConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self::from_env_parse()
    }

    pub fn from_env_file(filename: &str) -> Self {
        dotenvy::from_filename(filename).expect("env file to parse does not exist");
        Self::from_env_parse()
    }

    /// Tries to build up the config from the following sources in order:
    /// 1. read from `./config`
    /// 2. read from given `filename`
    /// 3. read from env vars
    pub fn from_env_all(filename: &str) -> Self {
        if dotenvy::from_filename("hiqlite.env").is_err() {
            debug!("config file './hiqlite.env' not found");
        }
        if dotenvy::from_filename_override(filename).is_err() {
            debug!("config file '{filename}' not found");
        }
        dotenvy::dotenv_override().ok();
        Self::from_env_parse()
    }

    fn from_env_parse() -> Self {
        let env_from = env::var("HQL_NODE_ID_FROM").unwrap_or_else(|_| String::default());
        let node_id = if env_from == "k8s" {
            Self::node_id_from_hostname()
        } else {
            env::var("HQL_NODE_ID")
                .expect("Node ID not found")
                .parse::<u64>()
                .expect("Cannot parse HQL_NODE_ID to u64")
        };

        let listen_addr_api = Cow::from(
            env::var("HQL_LISTEN_ADDR_API")
                .as_deref()
                .unwrap_or("0.0.0.0")
                .to_string(),
        );
        let listen_addr_raft = Cow::from(
            env::var("HQL_LISTEN_ADDR_RAFT")
                .as_deref()
                .unwrap_or("0.0.0.0")
                .to_string(),
        );

        #[cfg(feature = "cache")]
        let cache_storage_disk = env::var("HQL_CACHE_STORAGE_DISK")
            .as_deref()
            .unwrap_or("true")
            .parse::<bool>()
            .expect("Cannot parse HQL_CACHE_STORAGE_DISK as bool");

        let logs_keep = env::var("HQL_LOGS_UNTIL_SNAPSHOT")
            .as_deref()
            .unwrap_or("10000")
            .parse::<u64>()
            .expect("Cannot parse HQL_LOGS_UNTIL_SNAPSHOT to u64");
        #[cfg(feature = "backup")]
        let backup_keep_days_local = env::var("HQL_BACKUP_KEEP_DAYS_LOCAL")
            .as_deref()
            .unwrap_or("30")
            .parse::<u16>()
            .expect("Cannot parse HQL_BACKUP_KEEP_DAYS_LOCAL as u16");

        #[cfg(feature = "dashboard")]
        let insecure_cookie = env::var("HQL_INSECURE_COOKIE")
            .as_deref()
            .unwrap_or("false")
            .parse::<bool>()
            .expect("Cannot parse HQL_INSECURE_COOKIE as bool");

        let slf = Self {
            node_id,
            nodes: Node::parse_from_env("HQL_NODES"),
            listen_addr_api,
            listen_addr_raft,
            data_dir: env::var("HQL_DATA_DIR")
                .unwrap_or_else(|_| "data".to_string())
                .into(),
            filename_db: env::var("HQL_FILENAME_DB")
                .unwrap_or_else(|_| "hiqlite.db".to_string())
                .into(),
            log_statements: env::var("HQL_LOG_STATEMENTS")
                .as_deref()
                .unwrap_or("false")
                .parse()
                .expect("Cannot parse HQL_LOG_STATEMENTS as u64"),
            prepared_statement_cache_capacity: 1024,
            read_pool_size: env::var("HQL_READ_POOL_SIZE")
                .as_deref()
                .unwrap_or("4")
                .parse()
                .expect("Cannot parse HQL_READ_POOL_SIZE as usize"),
            wal_sync: hiqlite_wal::LogSync::ImmediateAsync,
            wal_size: 2 * 1024 * 1024,
            #[cfg(feature = "cache")]
            cache_storage_disk,
            raft_config: Self::default_raft_config(logs_keep),
            tls_raft: ServerTlsConfig::from_env("RAFT"),
            tls_api: ServerTlsConfig::from_env("API"),
            secret_raft: env::var("HQL_SECRET_RAFT").expect("HQL_SECRET_RAFT not found"),
            secret_api: env::var("HQL_SECRET_API").expect("HQL_SECRET_API not found"),
            #[cfg(any(feature = "s3", feature = "dashboard"))]
            enc_keys: cryptr::EncKeys::from_env().expect("Cannot parse ENC_KEYS from ENV"),
            #[cfg(feature = "backup")]
            backup_config: backup::BackupConfig::from_env(),
            #[cfg(feature = "s3")]
            s3_config: crate::s3::S3Config::try_from_env(),
            #[cfg(feature = "dashboard")]
            password_dashboard: DashboardState::from_env().password_dashboard,
            #[cfg(feature = "dashboard")]
            insecure_cookie,
            shutdown_delay_millis: env::var("HQL_SHUTDOWN_DELAY_MILLS")
                .as_deref()
                .unwrap_or("5000")
                .parse()
                .expect("Cannot parse HQL_SHUTDOWN_DELAY_MILLS as u32"),
            health_check_delay_secs: 30,
            #[cfg(feature = "backup")]
            backup_keep_days_local,
        };

        slf.is_valid()
            .expect("NodeConfig parsed from env is invalid");
        slf
    }

    /// Provides good defaults for a `RaftConfig` inside a fast network.
    #[allow(deprecated)] // allow to not need ..Default::default() and miss config updates
    pub fn default_raft_config(logs_until_snapshot: u64) -> RaftConfig {
        RaftConfig {
            cluster_name: "hiqlite".to_string(),
            election_timeout_min: 1500,
            election_timeout_max: 3000,
            heartbeat_interval: 500,
            install_snapshot_timeout: 10_000,
            send_snapshot_timeout: 0,
            max_payload_entries: 128,
            replication_lag_threshold: logs_until_snapshot * 2,
            snapshot_policy: SnapshotPolicy::LogsSinceLast(logs_until_snapshot),
            snapshot_max_chunk_size: 3 * 1024 * 1024,
            // be careful when adjusting this because of `backup::restore_backup_cleanup_task()`
            max_in_snapshot_log_to_keep: 1,
            purge_batch_size: 1,
            enable_tick: true,
            enable_heartbeat: true,
            enable_elect: true,
            // ..Default::default()
        }
    }

    #[cfg(any(feature = "s3", feature = "dashboard"))]
    pub(crate) fn init_enc_keys(&self) {
        let cloned = cryptr::EncKeys {
            enc_key_active: self.enc_keys.enc_key_active.clone(),
            enc_keys: self.enc_keys.enc_keys.clone(),
        };
        if cloned.init().is_err() {
            debug!("Cannot initialize ENC_KEYS - already initialized");
        }
    }

    /// Validates the config
    pub fn is_valid(&self) -> Result<(), Error> {
        if self.nodes.is_empty() {
            return Err(Error::Config("'nodes' must not be empty".into()));
        }

        if self.node_id < 1 {
            return Err(Error::Config("'node_id' must be >= 1".into()));
        }

        if self.node_id as usize > self.nodes.len() {
            return Err(Error::Config("'node_id' not found in 'nodes'".into()));
        }

        if self.secret_raft.len() < 16 || self.secret_api.len() < 16 {
            return Err(Error::Config(
                "'secret_raft' and 'secret_api' should be at least 16 characters long".into(),
            ));
        }

        #[cfg(any(feature = "dashboard", feature = "s3"))]
        {
            if self.enc_keys.enc_keys.is_empty() {
                return Err(Error::Config(
                    "'enc_keys.enc_keys' must not be empty".into(),
                ));
            }
            let active_valid = self
                .enc_keys
                .enc_keys
                .iter()
                .any(|(id, _)| id == &self.enc_keys.enc_key_active);
            if !active_valid {
                return Err(Error::Config(
                    "Invalid 'enc_keys.enc_key_active', does not point to an existing key".into(),
                ));
            }
        }

        #[cfg(feature = "dashboard")]
        if let Some(pwd) = &self.password_dashboard {
            if pwd.len() < 16 {
                return Err(Error::Config(
                    "password_dashboard should be at least 14 characters long".into(),
                ));
            }
        }

        if self.log_statements {
            warn!(
                r#"

!!! CAUTION !!!
Statement logging is activated - this can leak sensitive information into your logs,
as it will log query parameters as well. Be careful when using this in production and
clean up logs after debugging!
"#
            )
        }

        Ok(())
    }

    pub(crate) fn node_id_from_hostname() -> NodeId {
        let binding = hostname::get().expect("Cannot read hostname");
        let hostname = binding.to_str().expect("Invalid hostname format");
        match hostname.rsplit_once('-') {
            None => {
                panic!("Cannot split off the NODE_ID from the hostname {hostname}");
            }
            Some((_, id)) => {
                let id_hostname = id.parse::<u64>().expect("Cannot parse HQL_NODE_ID to u64");
                // the hostnames for k8s sts always start at 0, but we need to start at 1
                id_hostname + 1
            }
        }
    }
}

impl From<&str> for Node {
    fn from(s: &str) -> Self {
        let (id, rest) = s
            .trim()
            .split_once(' ')
            .expect("invalid format for HQL_NODES");
        let (addr_raft, addr_api) = rest.split_once(' ').expect("invalid format for HQL_NODES");

        let id = id
            .parse::<u64>()
            .expect("Cannot parse Node ID from HQL_NODES to u64");

        Self {
            id,
            addr_raft: addr_raft.trim().to_string(),
            addr_api: addr_api.trim().to_string(),
        }
    }
}

impl Node {
    pub fn parse_from_env(env_var: &str) -> Vec<Self> {
        let mut res = Vec::with_capacity(3);
        let value = env::var(env_var).unwrap_or_else(|_| panic!("{env_var} does not exist"));

        for line in value.lines() {
            if !line.is_empty() {
                res.push(Self::from(line))
            }
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use crate::{Node, NodeConfig};

    #[test]
    fn test_config_from_env() {
        let c = NodeConfig::from_env_file("hiqlite.env");
        println!("{:?}", c);

        assert_eq!(c.node_id, 1);
        assert_eq!(
            c.nodes,
            vec![Node {
                id: 1,
                addr_raft: "localhost:8100".to_string(),
                addr_api: "localhost:8200".to_string(),
            },]
        );
        assert_eq!(c.data_dir, "data");
        assert_eq!(c.filename_db, "hiqlite.db");
        assert_eq!(c.log_statements, true);

        assert_eq!(c.secret_raft, "SuperSecureSecret1337");
        assert_eq!(c.secret_api, "SuperSecureSecret1337");
    }
}
