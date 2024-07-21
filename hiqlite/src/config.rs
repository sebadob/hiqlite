use crate::{Error, Node, NodeId};
use openraft::SnapshotPolicy;
use std::borrow::Cow;

use crate::tls::ServerTlsConfig;
pub use openraft::Config as RaftConfig;

#[cfg(feature = "s3")]
#[derive(Debug, Clone)]
pub enum EncKeysFrom {
    Env,
    File(String),
}

/// The config for a Raft Node
///
/// TODO if feature `serde` is set, should maybe be Serialize / Deserialize
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
    // pub mode: NodeMode,
    /// The internal Raft config. This must be the same on each node.
    pub config: RaftConfig,
    /// If RPC and HTTP connections should use TLS
    pub tls_raft: Option<ServerTlsConfig>,
    pub tls_api: Option<ServerTlsConfig>,
    /// Secret for all Raft internal messages - at least 16 characters long
    pub secret_raft: String,
    /// Secret for Raft management and DB API - at least 16 characters long
    pub secret_api: String,
    /// From where `ENC_KEYS` should be read for S3 backup encryption.
    #[cfg(feature = "s3")]
    pub enc_keys_from: EncKeysFrom,
    /// If an `S3Config` is given, it will be used to push backups to the S3 bucket.
    #[cfg(feature = "s3")]
    pub s3_config: Option<crate::S3Config>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: 0,
            nodes: vec![],
            data_dir: "hiqlite".into(),
            filename_db: "hiqlite.db".into(),
            config: Self::raft_config(10_000),
            tls_raft: None,
            tls_api: None,
            secret_raft: String::default(),
            secret_api: String::default(),
            #[cfg(feature = "s3")]
            enc_keys_from: EncKeysFrom::Env,
            #[cfg(feature = "s3")]
            s3_config: None,
        }
    }
}

impl NodeConfig {
    // TODO impl some `from_`s like env, json, toml, cli

    // TODO get rid of the `new()` because it gets messier the more features come.
    // its a far better DX to just init directly and use `..Default::default()`
    pub fn new(
        node_id: NodeId,
        nodes: Vec<Node>,
        tls_raft: Option<ServerTlsConfig>,
        tls_api: Option<ServerTlsConfig>,
        secret_raft: String,
        secret_api: String,
    ) -> Result<Self, Error> {
        let slf = Self {
            node_id,
            nodes,
            data_dir: "hiqlite".into(),
            filename_db: "hiqlite.db".into(),
            config: Self::raft_config(10_000),
            tls_raft,
            tls_api,
            secret_raft,
            secret_api,
            #[cfg(feature = "s3")]
            enc_keys_from: EncKeysFrom::Env,
            #[cfg(feature = "s3")]
            s3_config: None,
        };

        slf.is_valid()?;

        Ok(slf)
    }

    // #[cfg(feature = "s3")]
    // pub fn new(
    //     node_id: NodeId,
    //     nodes: Vec<Node>,
    //     tls_raft: Option<ServerTlsConfig>,
    //     tls_api: Option<ServerTlsConfig>,
    //     secret_raft: String,
    //     secret_api: String,
    //     s3_config: Option<crate::S3Config>,
    // ) -> Result<Self, Error> {
    //     let slf = Self {
    //         node_id,
    //         nodes,
    //         data_dir: "hiqlite".into(),
    //         filename_db: "hiqlite.db".into(),
    //         config: Self::raft_config(10_000),
    //         tls_raft,
    //         tls_api,
    //         secret_raft,
    //         secret_api,
    //         s3_config,
    //     };
    //
    //     slf.is_valid()?;
    //
    //     Ok(slf)
    // }

    /// Provides a good starting point for a `RaftConfig` inside a fast network.
    #[allow(deprecated)] // allow to not need ..Default::default() and miss config updates
    pub fn raft_config(logs_until_snapshot: u64) -> RaftConfig {
        RaftConfig {
            cluster_name: "hiqlite".to_string(),
            election_timeout_min: 750,
            election_timeout_max: 1500,
            heartbeat_interval: 250,
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
            max_in_snapshot_log_to_keep: 1000,
            purge_batch_size: 1,
            enable_tick: true,
            enable_heartbeat: true,
            enable_elect: true,
            // ..Default::default()
        }
    }

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

        Ok(())
    }
}
