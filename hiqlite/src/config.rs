use crate::tls::ServerTlsConfig;
use crate::{Error, Node, NodeId};
use openraft::SnapshotPolicy;
use std::borrow::Cow;
use std::env;
use tracing::warn;

pub use openraft::Config as RaftConfig;

#[cfg(feature = "backup")]
use crate::backup;

#[cfg(feature = "dashboard")]
use crate::dashboard::DashboardState;

#[cfg(feature = "s3")]
#[derive(Debug, Clone)]
pub enum EncKeysFrom {
    Env,
    File(String),
}

/// The main Node config.
///
/// Most default values are good for internal, fast networks. If you have a slow or unstable
/// network, you might want to tune the `RaftConfig`. However, you should never adjust the
/// `max_in_snapshot_log_to_keep`, because this will play a crucial role if you need to restore
/// from a backup in case of desaster recovery.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// The `node_id` defines which entry from the `nodes` is "this node"
    pub node_id: NodeId,
    /// All Raft member nodes
    pub nodes: Vec<Node>,
    /// The directory where the replication log, database and snapshots should be stored
    pub data_dir: Cow<'static, str>,
    /// If the SQLite should be written to disk, provide a filename here.
    /// It is recommended to leave it set to None if your DB size fits fully into memory, and
    /// you can afford this. No data will be lost with an in-memory DB because Raft logs and
    /// snapshots are always persisted and the in-memory DB can be rebuilt quickly after a restart.
    pub filename_db: Cow<'static, str>,
    /// Enables statement logging or the SQL writer
    pub log_statements: bool,
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
    /// auto-backup configuration
    #[cfg(feature = "backup")]
    pub backup_config: backup::BackupConfig,
    /// From where `ENC_KEYS` should be read for S3 backup encryption. feature `s3`
    #[cfg(feature = "s3")]
    pub enc_keys_from: EncKeysFrom,
    /// If an `S3Config` is given, it will be used to push backups to the S3 bucket. feature `s3`
    #[cfg(feature = "s3")]
    pub s3_config: Option<std::sync::Arc<crate::s3::S3Config>>,
    /// Set the password for the integrated dashboard. Must be given as argon2id hash. feature `dashboard`
    #[cfg(feature = "dashboard")]
    pub password_dashboard: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: 0,
            nodes: vec![],
            data_dir: "hiqlite".into(),
            filename_db: "hiqlite.db".into(),
            log_statements: false,
            raft_config: Self::default_raft_config(10_000),
            tls_raft: None,
            tls_api: None,
            secret_raft: String::default(),
            secret_api: String::default(),
            #[cfg(feature = "backup")]
            backup_config: backup::BackupConfig::default(),
            #[cfg(feature = "s3")]
            enc_keys_from: EncKeysFrom::Env,
            #[cfg(feature = "s3")]
            s3_config: None,
            #[cfg(feature = "dashboard")]
            password_dashboard: String::default(),
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
        if dotenvy::from_filename("config").is_err() {
            warn!("config file './config' not found");
        }
        if dotenvy::from_filename_override(filename).is_err() {
            warn!("config file '{}' not found", filename);
        }
        dotenvy::dotenv_override().ok();
        Self::from_env_parse()
    }

    fn from_env_parse() -> Self {
        let env_from = env::var("HQL_NODE_ID_FROM").unwrap_or_else(|_| String::default());
        let node_id = if env_from == "k8s" {
            let binding = hostname::get().expect("Cannot read hostname");
            let hostname = binding.to_str().expect("Invalid hostname format");
            match hostname.rsplit_once('-') {
                None => {
                    panic!(
                        "Cannot split off the NODE_ID from the hostname {}",
                        hostname
                    );
                }
                Some((_, id)) => {
                    let id_hostname = id.parse::<u64>().expect("Cannot parse HQL_NODE_ID to u64");
                    // the hostnames for k8s sts always start at 0, but we need to start at 1
                    id_hostname + 1
                }
            }
        } else {
            env::var("HQL_NODE_ID")
                .expect("Node ID not found")
                .parse::<u64>()
                .expect("Cannot parse HQL_NODE_ID to u64")
        };

        let logs_keep = env::var("HQL_LOGS_UNTIL_SNAPSHOT")
            .unwrap_or_else(|_| "10000".to_string())
            .parse::<u64>()
            .expect("Cannot parse HQL_LOGS_UNTIL_SNAPSHOT to u64");

        #[cfg(feature = "s3")]
        let enc_keys_from = env::var("HQL_ENC_KEYS_FROM")
            .map(|v| {
                if let Some(path) = v.strip_prefix("file:") {
                    EncKeysFrom::File(path.to_string())
                } else {
                    EncKeysFrom::Env
                }
            })
            .unwrap_or(EncKeysFrom::Env);

        let slf = Self {
            node_id,
            nodes: Node::all_from_env(),
            data_dir: env::var("HQL_DATA_DIR")
                .unwrap_or("data".to_string())
                .into(),
            filename_db: env::var("HQL_FILENAME_DB")
                .unwrap_or("hiqlite.db".to_string())
                .into(),
            log_statements: env::var("HQL_LOG_STATEMENTS")
                .unwrap_or("false".to_string())
                .parse()
                .expect("Cannot parse HQL_LOG_STATEMENTS to u64"),
            raft_config: Self::default_raft_config(logs_keep),
            tls_raft: ServerTlsConfig::from_env("RAFT"),
            tls_api: ServerTlsConfig::from_env("API"),
            secret_raft: env::var("HQL_SECRET_RAFT").expect("HQL_SECRET_RAFT not found"),
            secret_api: env::var("HQL_SECRET_API").expect("HQL_SECRET_API not found"),
            #[cfg(feature = "backup")]
            backup_config: backup::BackupConfig::from_env(),
            #[cfg(feature = "s3")]
            enc_keys_from,
            #[cfg(feature = "s3")]
            s3_config: crate::s3::S3Config::try_from_env(),
            #[cfg(feature = "dashboard")]
            password_dashboard: DashboardState::from_env().password_dashboard,
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
            election_timeout_min: 500,
            election_timeout_max: 1000,
            heartbeat_interval: 100,
            // election_timeout_min: 250,
            // election_timeout_max: 500,
            // heartbeat_interval: 100,
            install_snapshot_timeout: 1000,
            send_snapshot_timeout: 0,
            max_payload_entries: 128,
            // max_payload_entries: 300,
            replication_lag_threshold: 5000,
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

        #[cfg(feature = "dashboard")]
        if self.password_dashboard.len() < 14 {
            return Err(Error::Config(
                "password_dashboard should be at least 14 characters long".into(),
            ));
        }

        Ok(())
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
    fn all_from_env() -> Vec<Self> {
        let mut res = Vec::new();

        let value = env::var("HQL_NODES").expect("HQL_NODES does not exist");

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
        let c = NodeConfig::from_env_file("config");
        println!("{:?}", c);

        assert_eq!(c.node_id, 1);
        assert_eq!(
            c.nodes,
            vec![
                Node {
                    id: 1,
                    addr_raft: "localhost:8100".to_string(),
                    addr_api: "localhost:8200".to_string(),
                },
                Node {
                    id: 2,
                    addr_raft: "localhost:8100".to_string(),
                    addr_api: "localhost:8200".to_string(),
                },
                Node {
                    id: 3,
                    addr_raft: "localhost:8100".to_string(),
                    addr_api: "localhost:8200".to_string(),
                },
            ]
        );
        assert_eq!(c.data_dir, "my_hiqlite");
        assert_eq!(c.filename_db, "my_hiqlite.db");
        assert_eq!(c.log_statements, true);

        let tls_raft = c.tls_raft.unwrap();
        assert_eq!(tls_raft.key, "tls/key.pem");
        assert_eq!(tls_raft.cert, "tls/cert-chain.pem");
        assert_eq!(tls_raft.danger_tls_no_verify, true);

        let tls_api = c.tls_api.unwrap();
        assert_eq!(tls_api.key, "tls/key.pem");
        assert_eq!(tls_api.cert, "tls/cert-chain.pem");
        assert_eq!(tls_api.danger_tls_no_verify, true);

        assert_eq!(c.secret_raft, "asdasdasdasdasdasd");
        assert_eq!(c.secret_api, "qweqweqweqweqweqwe");

        let bucket = &c.s3_config.unwrap().bucket;
        assert_eq!(bucket.host, "https://s3.example.com".parse().unwrap());
        assert_eq!(bucket.name, "my_bucket");
        assert_eq!(bucket.region.0, "example");
    }
}
