use clap::Parser;
use hiqlite::cache_idx::CacheIndex;
use hiqlite::{start_node_with_cache, Error, Node, NodeConfig, ServerTlsConfig};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tokio::fs;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Parser)]
#[clap(author, version, about, long_about = None)]
enum Args {
    /// Start one of the 3 test nodes
    Server(Server),
    /// Start the test with just a single node - works in the same way
    Single,
}

#[derive(Debug, Clone, Parser)]
struct Server {
    #[clap(long)]
    pub node_id: u64,
}

fn test_nodes() -> Vec<Node> {
    vec![
        Node {
            id: 1,
            addr_api: "127.0.0.1:8201".to_string(),
            addr_raft: "127.0.0.1:8101".to_string(),
        },
        Node {
            id: 2,
            addr_api: "127.0.0.1:8202".to_string(),
            addr_raft: "127.0.0.1:8102".to_string(),
        },
        Node {
            id: 3,
            addr_api: "127.0.0.1:8203".to_string(),
            addr_raft: "127.0.0.1:8103".to_string(),
        },
    ]
}

fn node_config(node_id: u64, nodes: Vec<Node>) -> NodeConfig {
    NodeConfig {
        node_id,
        nodes,
        log_statements: true,
        tls_raft: Some(ServerTlsConfig {
            key: "../../tls/key.pem".into(),
            cert: "../../tls/cert-chain.pem".into(),
            danger_tls_no_verify: true,
        }),
        tls_api: Some(ServerTlsConfig {
            key: "../../tls/key.pem".into(),
            cert: "../../tls/cert-chain.pem".into(),
            danger_tls_no_verify: true,
        }),
        secret_raft: "SuperSecureRaftSecret".to_string(),
        secret_api: "SuperSecureApiSecret".to_string(),
        ..Default::default()
    }
}

/// Matches our test table for this example.
/// serde derives are needed if you want to use the `query_as()` fn.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Entity {
    pub id: String,
    pub num: i64,
    pub description: Option<String>,
}

#[derive(Debug, strum::EnumIter)]
enum Cache {
    One,
    Two,
}

// This tiny block of boilerplate is necessary to index concurrent caches properly.
// The result must always return each elements position in the iterator and this simple typecasting
// is the easiest way to do it. It is checked for correctness and compared against the iterator
// during startup.
impl CacheIndex for Cache {
    fn to_usize(self) -> usize {
        self as usize
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        // .with_env_filter(EnvFilter::from("debug"))
        .with_env_filter(EnvFilter::from("info"))
        .init();

    match Args::parse() {
        Args::Server(args) => {
            if args.node_id < 1 || args.node_id > 3 {
                Err(Error::Config("Node ID must be 1-3".into()))
            } else {
                server(Some(args)).await
            }
        }
        Args::Single => server(None).await,
    }
}

async fn server(args: Option<Server>) -> Result<(), Error> {
    let config = if let Some(args) = args {
        let mut config = node_config(args.node_id, test_nodes());

        // to make this example work when starting all nodes on the same host,
        // we need to save into custom folders for each one
        config.data_dir = format!("data/node_{}", args.node_id).into();
        cleanup(config.data_dir.as_ref()).await;
        config
    } else {
        let mut config = node_config(1, vec![test_nodes()[0].clone()]);
        config.data_dir = "data/node_1".into();
        cleanup(config.data_dir.as_ref()).await;
        config
    };

    let client = start_node_with_cache::<Cache>(config).await?;

    let mut shutdown_handle = client.shutdown_handle()?;
    shutdown_handle.wait().await?;

    Ok(())
}

async fn cleanup(path: &str) {
    let _ = fs::remove_dir_all(path).await;
}
