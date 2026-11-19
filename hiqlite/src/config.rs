use crate::tls::ServerTlsConfig;
use crate::{Error, Node, NodeId};
use openraft::SnapshotPolicy;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::env;
use std::time::Duration;
use tracing::{debug, warn};

#[cfg(feature = "backup")]
use crate::backup;

#[cfg(feature = "dashboard")]
use crate::dashboard::DashboardState;

use crate::helpers::parse_duration;
pub use openraft::Config as RaftConfig;

/// This is an embedded reference config TOML file. It contains a few templating placeholders, but
/// is a perfect start if you want to generate a custom config on disk.
///
/// Available placeholders are:
/// - `{{ DATA_DIR }}`
/// - `{{ SECRET_RAFT }}`
/// - `{{ SECRET_API }}`
/// - `{{ ENC_KEY }}`
/// - `{{ ENC_KEY_ACTIVE }}`
/// - `{{ PASSWORD_DASHBOARD }}`
#[cfg(feature = "config-embed")]
pub static REFERENCE_CONFIG: &str = include_str!("../../REFERENCE_CONFIG.toml");

/// The marker that a node line may carry as its fourth value: a node that is marked with it never
/// starts an election on its own. See [`Node::parse_line`] and `NodeConfig::election_disable`.
const ELECTION_DISABLE: &str = "election_disable";

#[derive(Debug)]
pub struct RateLimitConfig {
    pub rps: u32,
    pub burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            rps: 100_000,
            burst: 200_000,
        }
    }
}

/// The `enc_keys` argument of `NodeConfig::from_toml` and `NodeConfig::from_toml_table`, in every
/// feature set: the real keys where the features compile encryption keys in, and a placeholder that
/// only ever accepts `None` where they do not. A caller passes the same four arguments everywhere,
/// no matter which features its build has.
#[cfg(any(feature = "s3", feature = "dashboard"))]
pub type EncKeysArg = Option<cryptr::EncKeys>;
#[cfg(not(any(feature = "s3", feature = "dashboard")))]
pub type EncKeysArg = Option<()>;

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
    /// The directory where the replication log, database and snapshots should be stored.
    /// It must exist and be writable, unless you build with the `in-memory-snapshots`
    /// feature and run a pure cache-only node (no SQLite) with `cache_storage_disk = false`:
    /// that combination keeps everything in-memory and never touches `data_dir`.
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
    /// When Raft logs should be synced to disk.
    ///
    /// - `Immediate` fsyncs the WAL before the Raft is told the append finished, so an
    ///   acknowledged write is on disk. Very huge impact on performance, but highest degree of
    ///   consistency, especially important for a single node "cluster".
    /// - `ImmediateAsync` tells the Raft the append finished after starting the writeback without
    ///   waiting for it (`sync_file_range` on Linux, `msync(MS_ASYNC)` elsewhere). Entries are
    ///   typically on disk within milliseconds, but there is no hard bound, because the device
    ///   write cache is not flushed. As long as the kernel does not crash, your data is safe, even
    ///   if the app crashes.
    /// - `IntervalMillis(ms)` (default with 200) acknowledges the same way and flushes the WAL from
    ///   a ticker every X `ms` milliseconds. This is the best compromise for HA clusters between
    ///   performance and consistency. Data is fsync'ed every 200ms (by default) and kept in kernel
    ///   buffers in between. If your app crashes, your data is safe. If your full kernel crashes,
    ///   it might not. However, when you run a HA cluster, anything that got lost in the last 200ms
    ///   can be synced from another node on the next startup.
    ///
    /// The two async levels trade durability for throughput: a power loss or a kernel panic can
    /// drop entries the cluster already acknowledged, and that loss is not confined to the node
    /// that crashed. If a leader and one follower hold an entry in the page cache, the leader
    /// commits it and answers the client, and the leader then power-cycles without it, a third
    /// node whose log ends before that entry can win the next election and truncate it from the
    /// follower. The acknowledged write is gone cluster-wide, and no member can re-sync it.
    /// Pick `Immediate` when acknowledged writes have to survive a power loss.
    ///
    /// default: `IntervalMillis(200)`
    pub wal_sync: hiqlite_wal::LogSync,
    /// Maximum WAL size in bytes.
    ///
    /// Do not set too low, because entries that do not fit inside a single WAL file will create a
    /// panic by default. WAL files do not grow. They will have this fixed size all the time. Set
    /// to a reasonable size that matches queries. Optimal is if you have 2-4 WAL files around all
    /// the time (after log roll-over).
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
    /// With the opt-in `in-memory-snapshots` feature, a pure cache-only deployment (no SQLite)
    /// using `false` additionally keeps Cache Snapshots in-memory, so `data_dir` is never used
    /// and does not need to exist. Without that feature, Snapshots are still written to
    /// `data_dir` (the default), which streams them efficiently but requires the directory.
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
    /// Specific TLS certificates for the Raft traffic. Overwrites `tls_auto_certificates`.
    pub tls_raft: Option<ServerTlsConfig>,
    /// Specific TLS certificates for the API traffic. Overwrites `tls_auto_certificates`.
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
    pub backup_keep_for_local: Duration,
    /// The prefix of every backup file name. A backup is always written as
    /// `{prefix}_{node_id}_{unix_ts}.sqlite`, on the local disk and in the S3 bucket.
    ///
    /// Set one prefix per application when several Hiqlite clusters share a single bucket: the
    /// cleanup jobs only ever delete files matching this exact shape, so clusters with different
    /// prefixes can never delete each other's backups, and objects that are not a Hiqlite backup
    /// at all are never touched.
    ///
    /// default: `backup_node`
    #[cfg(feature = "backup")]
    pub backup_file_prefix: Cow<'static, str>,
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
    /// The initial delay until which the "true" health will be returned. A delay at the start is
    /// necessary to solve a chicken-and-egg problem when cold starting cluster which depend on
    /// health checks.
    pub health_check_delay: Duration,
    /// If true, keep a newly joining node as a learner instead of promoting it to a voting member
    /// during startup reconciliation. This does not demote an existing voter.
    pub learner_only: bool,
    /// The node ids whose node line carries the `election_disable` marker.
    ///
    /// When this node's own id is in this list, `raft_config.enable_elect` is set to `false`
    /// during config parsing: the node never starts an election on its own, for both of its Raft
    /// groups. It still votes for other nodes, and it still accepts a leadership transfer, so a
    /// leader can hand over to it at any time.
    ///
    /// The marker is a node line value only - it is deliberately not a field of [`Node`], because
    /// that type is serialized into the Raft logs and its bytes must stay compatible.
    ///
    /// A node that changes this at runtime reverts to this config value when it restarts.
    pub election_disable: Vec<NodeId>,
    /// To guarantee the stability of your Raft cluster no matter how high requests might spike,
    /// you can set rate-limits.
    #[cfg(feature = "cache")]
    pub rate_limit_cache: Option<RateLimitConfig>,
    /// To guarantee the stability of your Raft cluster no matter how high requests might spike,
    /// you can set rate-limits.
    #[cfg(feature = "sqlite")]
    pub rate_limit_db: Option<RateLimitConfig>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: 0,
            nodes: vec![],
            listen_addr_api: "0.0.0.0".into(),
            listen_addr_raft: "0.0.0.0".into(),
            data_dir: "data".into(),
            filename_db: "hiqlite.db".into(),
            log_statements: false,
            prepared_statement_cache_capacity: 1024,
            read_pool_size: 4,
            wal_sync: hiqlite_wal::LogSync::IntervalMillis(200),
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
            backup_keep_for_local: Duration::from_secs(3 * 24 * 3600),
            #[cfg(feature = "backup")]
            backup_file_prefix: backup::DEFAULT_BACKUP_FILE_PREFIX.into(),
            #[cfg(feature = "s3")]
            s3_config: None,
            #[cfg(feature = "dashboard")]
            password_dashboard: None,
            #[cfg(feature = "dashboard")]
            insecure_cookie: false,
            health_check_delay: Duration::from_secs(30),
            learner_only: false,
            election_disable: vec![],
            #[cfg(feature = "cache")]
            rate_limit_cache: None,
            #[cfg(feature = "sqlite")]
            rate_limit_db: None,
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
        let backup_keep_for_local = env::var("HQL_BACKUP_KEEP_FOR_LOCAL")
            .ok()
            .as_deref()
            .map(|v| parse_duration(v).expect("Cannot parse HQL_BACKUP_KEEP_FOR_LOCAL as Duration"))
            .unwrap_or(Duration::from_secs(3 * 24 * 3600));

        #[cfg(feature = "dashboard")]
        let insecure_cookie = env::var("HQL_INSECURE_COOKIE")
            .as_deref()
            .unwrap_or("false")
            .parse::<bool>()
            .expect("Cannot parse HQL_INSECURE_COOKIE as bool");

        #[cfg(feature = "cache")]
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

        #[cfg(feature = "sqlite")]
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

        let (nodes, election_disable) = Node::parse_from_env_with_flags("HQL_NODES");

        let mut slf = Self {
            node_id,
            nodes,
            election_disable,
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
                .expect("Cannot parse HQL_LOG_STATEMENTS as bool"),
            prepared_statement_cache_capacity: 1024,
            read_pool_size: env::var("HQL_READ_POOL_SIZE")
                .as_deref()
                .unwrap_or("4")
                .parse()
                .expect("Cannot parse HQL_READ_POOL_SIZE as usize"),
            wal_sync: if let Ok(v) = env::var("HQL_LOG_SYNC") {
                hiqlite_wal::LogSync::try_from(v.as_str())
                    .expect("Cannot parse HQL_LOG_SYNC as LogSync")
            } else {
                hiqlite_wal::LogSync::IntervalMillis(200)
            },
            wal_size: env::var("HQL_WAL_SIZE")
                .ok()
                .as_deref()
                .map(|v| v.parse::<u32>().expect("Cannot parse HQL_WAL_SIZE as u32"))
                .unwrap_or(2 * 1024 * 1024),
            #[cfg(feature = "cache")]
            cache_storage_disk,
            raft_config: Self::default_raft_config(logs_keep),
            tls_raft: ServerTlsConfig::from_env("RAFT").expect("Error building Raft TLS config"),
            tls_api: ServerTlsConfig::from_env("API").expect("Error building API TLS config"),
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
            health_check_delay: env::var("HQL_HEALTH_CHECK_DELAY")
                .ok()
                .as_deref()
                .map(|v| {
                    parse_duration(v).expect("Cannot parse HQL_HEALTH_CHECK_DELAY as Duration")
                })
                .unwrap_or(Duration::from_secs(30)),
            learner_only: env::var("HQL_LEARNER_ONLY")
                .as_deref()
                .unwrap_or("false")
                .parse::<bool>()
                .expect("Cannot parse HQL_LEARNER_ONLY as bool"),
            #[cfg(feature = "backup")]
            backup_keep_for_local,
            #[cfg(feature = "backup")]
            backup_file_prefix: env::var("HQL_BACKUP_FILE_PREFIX")
                .unwrap_or_else(|_| backup::DEFAULT_BACKUP_FILE_PREFIX.to_string())
                .into(),
            #[cfg(feature = "cache")]
            rate_limit_cache,
            #[cfg(feature = "sqlite")]
            rate_limit_db,
        };

        slf.apply_election_disable();
        slf.is_valid()
            .expect("NodeConfig parsed from env is invalid");
        slf
    }

    /// Provides good defaults for a `RaftConfig` inside a fast network.
    pub fn default_raft_config(logs_until_snapshot: u64) -> RaftConfig {
        RaftConfig {
            cluster_name: "hiqlite".to_string(),
            election_timeout_min: 1500,
            election_timeout_max: 3000,
            heartbeat_interval: 500,
            install_snapshot_timeout: 10_000,
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
            // A wiped or corrupted volume must be able to re-sync on its own: a reset only ever
            // overwrites a follower's log with the leader's, and the leader's log holds every
            // committed entry, so a reverted log can only ever drop entries that were never
            // committed on that node. Without this, a follower that reverts under the same leader
            // panics that leader in debug builds and stalls its replication in release builds.
            // Set it to `Some(false)` on a node to keep the strict behavior.
            allow_log_reversion: Some(true),
            ..Default::default()
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

    /// Apply the `election_disable` marker of this node's own node line to the Raft config.
    ///
    /// `enable_elect = false` means the node never starts an election on its own, for both of its
    /// Raft groups. It still grants votes and it still accepts a leadership transfer, so it stays a
    /// fully voting member. A node that flips this at runtime with the `PUT /cluster/election`
    /// endpoint reverts to this config value when it restarts.
    ///
    /// The config parsers from env and TOML apply this on their own. Call it after you set
    /// `election_disable` on a config you built by hand.
    pub fn apply_election_disable(&mut self) {
        if !self.election_disable.contains(&self.node_id) {
            return;
        }
        self.raft_config.enable_elect = false;
        warn!(
            "Node {} has 'election_disable' set: it will never start an election on its own \
             (both Raft groups). A restart reverts any runtime change back to this value.",
            self.node_id
        );
    }

    /// Validates the config
    pub fn is_valid(&self) -> Result<(), Error> {
        if self.nodes.is_empty() {
            return Err(Error::Config("'nodes' must not be empty".into()));
        }

        if self.node_id < 1 {
            return Err(Error::Config("'node_id' must be >= 1".into()));
        }

        if !self.nodes.iter().any(|node| node.id == self.node_id) {
            return Err(Error::Config(
                format!("'node_id' {} not found in 'nodes'", self.node_id).into(),
            ));
        }

        let mut ids = BTreeSet::new();
        for node in &self.nodes {
            ids.insert(node.id);
        }
        if ids.len() != self.nodes.len() {
            return Err(Error::Config(
                format!(
                    "Invalid nodes configured: must not contain duplicate IDs: {:?}",
                    self.nodes
                )
                .into(),
            ));
        }

        for node_id in &self.election_disable {
            if !self.nodes.iter().any(|node| node.id == *node_id) {
                return Err(Error::Config(
                    format!("'election_disable' node id {node_id} not found in 'nodes'").into(),
                ));
            }
        }

        #[cfg(feature = "backup")]
        if !backup::is_valid_file_prefix(&self.backup_file_prefix) {
            return Err(Error::Config(
                "'backup_file_prefix' must be a plain file name prefix: non-empty and only letters, \
                 digits, '.', '-' and '_'"
                    .into(),
            ));
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
        if let Some(pwd) = &self.password_dashboard
            && pwd.len() < 16
        {
            return Err(Error::Config(
                "password_dashboard should be at least 14 characters long".into(),
            ));
        }

        self.raft_config
            .clone()
            .validate()
            .map_err(|err| Error::Config(format!("'raft_config' is invalid: {err}").into()))?;

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
        Self::parse_line(s).0
    }
}

impl Node {
    /// Parse a single node line out of `nodes` / `HQL_NODES`.
    ///
    /// The format is `id addr_raft addr_api`, and it accepts the optional fourth value
    /// `election_disable`, which marks a node that never starts an election on its own. The second
    /// return value is `true` for such a line. Anything else is a config error and panics loudly.
    pub fn parse_line(s: &str) -> (Self, bool) {
        let mut tokens = s.split_whitespace();
        let (id, addr_raft, addr_api, flag, extra) = (
            tokens.next(),
            tokens.next(),
            tokens.next(),
            tokens.next(),
            tokens.next(),
        );

        let Some((id, addr_raft, addr_api)) =
            id.zip(addr_raft).zip(addr_api).map(|((i, r), a)| (i, r, a))
        else {
            panic!("invalid format for HQL_NODES, expected 'id addr_raft addr_api': {s}");
        };

        let election_disable = match flag {
            None => false,
            Some(flag) if flag == ELECTION_DISABLE => true,
            Some(flag) => panic!("invalid format for HQL_NODES, unknown node flag '{flag}': {s}"),
        };
        if extra.is_some() {
            panic!("invalid format for HQL_NODES, too many values: {s}");
        }

        let id = id
            .parse::<u64>()
            .expect("Cannot parse Node ID from HQL_NODES to u64");

        (
            Self {
                id,
                addr_raft: addr_raft.to_string(),
                addr_api: addr_api.to_string(),
            },
            election_disable,
        )
    }

    /// Parse a full node list, given as one line per node, and collect the ids of all nodes that
    /// carry the `election_disable` marker. See [`Node::parse_line`].
    pub fn parse_lines<S: AsRef<str>>(
        lines: impl IntoIterator<Item = S>,
    ) -> (Vec<Self>, Vec<NodeId>) {
        let mut nodes = Vec::with_capacity(3);
        let mut election_disable = Vec::new();

        for line in lines {
            let line = line.as_ref();
            if line.trim().is_empty() {
                continue;
            }
            let (node, disable) = Self::parse_line(line);
            if disable {
                election_disable.push(node.id);
            }
            nodes.push(node);
        }

        (nodes, election_disable)
    }

    pub fn parse_from_env(env_var: &str) -> Vec<Self> {
        Self::parse_from_env_with_flags(env_var).0
    }

    /// Parse the node list from the given env var, where every node is one line, and collect the
    /// ids of all nodes that carry the `election_disable` marker. See [`Node::parse_line`].
    pub fn parse_from_env_with_flags(env_var: &str) -> (Vec<Self>, Vec<NodeId>) {
        let value = env::var(env_var).unwrap_or_else(|_| panic!("{env_var} does not exist"));
        Self::parse_lines(value.lines())
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
        assert!(c.log_statements);

        assert_eq!(c.secret_raft, "SuperSecureSecret1337");
        assert_eq!(c.secret_api, "SuperSecureSecret1337");
        assert!(!c.learner_only);
        assert!(c.election_disable.is_empty());
        assert!(c.raft_config.enable_elect);
    }

    #[test]
    fn node_line_parses_plain_and_marked() {
        let (node, disable) = Node::parse_line("1 localhost:8100 localhost:8200");
        assert_eq!(node.id, 1);
        assert_eq!(node.addr_raft, "localhost:8100");
        assert_eq!(node.addr_api, "localhost:8200");
        assert!(!disable);

        let (node, disable) =
            Node::parse_line("3 198.51.100.9:8100 198.51.100.9:8200 election_disable");
        assert_eq!(node.id, 3);
        assert_eq!(node.addr_api, "198.51.100.9:8200");
        assert!(disable);
    }

    #[test]
    fn node_lines_collect_the_marked_ids() {
        let (nodes, election_disable) = Node::parse_lines([
            "1 10.0.0.1:8100 10.0.0.1:8200",
            "2 10.0.0.2:8100 10.0.0.2:8200",
            "3 198.51.100.9:8100 198.51.100.9:8200 election_disable",
        ]);

        assert_eq!(nodes.len(), 3);
        assert_eq!(election_disable, vec![3]);
    }

    #[test]
    #[should_panic(expected = "unknown node flag 'nope'")]
    fn node_line_rejects_an_unknown_flag() {
        Node::parse_line("1 10.0.0.1:8100 10.0.0.1:8200 nope");
    }

    #[test]
    #[should_panic(expected = "too many values")]
    fn node_line_rejects_extra_values() {
        Node::parse_line("1 10.0.0.1:8100 10.0.0.1:8200 election_disable extra");
    }

    #[test]
    #[should_panic(expected = "expected 'id addr_raft addr_api'")]
    fn node_line_rejects_missing_values() {
        Node::parse_line("1 10.0.0.1:8100");
    }

    #[test]
    fn a_marked_node_disables_its_own_election() {
        let mut config = NodeConfig {
            node_id: 3,
            nodes: vec![node(1), node(2), node(3)],
            election_disable: vec![3],
            ..Default::default()
        };

        assert!(config.raft_config.enable_elect);
        config.apply_election_disable();
        assert!(!config.raft_config.enable_elect);
    }

    #[test]
    fn an_unmarked_node_keeps_its_election_enabled() {
        let mut config = NodeConfig {
            node_id: 1,
            nodes: vec![node(1), node(2), node(3)],
            election_disable: vec![3],
            ..Default::default()
        };

        config.apply_election_disable();
        assert!(config.raft_config.enable_elect);
    }

    #[test]
    fn config_rejects_an_election_disable_id_that_is_not_a_node() {
        let config = NodeConfig {
            node_id: 1,
            nodes: vec![node(1)],
            secret_raft: "0123456789abcdef".into(),
            secret_api: "0123456789abcdef".into(),
            election_disable: vec![7],
            ..Default::default()
        };

        let err = config.is_valid().unwrap_err().to_string();
        assert!(err.contains("election_disable"), "{err}");
    }

    fn node(id: u64) -> Node {
        Node {
            id,
            addr_raft: format!("10.0.0.{id}:8100"),
            addr_api: format!("10.0.0.{id}:8200"),
        }
    }
}
