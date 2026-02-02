use clap::Parser;
use hiqlite::{start_node_with_cache, Client, Error, Node, NodeConfig};
use hiqlite_macros::embed::*;
use std::fmt::{Debug, Display};
use std::time::Duration;
use tokio::time;
use tokio::{fs, task};
use tracing_subscriber::EnvFilter;

mod bench;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

#[derive(Debug, Clone, PartialEq, Parser)]
#[clap(author, version, about, long_about = None)]
enum Args {
    /// Start benchmarks where all nodes are spawned on the same host while still using real networking
    Cluster(Options),
    /// Start tests with a Single Node
    Single(Options),
    /// Run the benchmark with a pure remote client on an already running cluster.
    /// CAUTION: This may overwrite existing data, depending on your setup and config!
    Remote(OptionsRemote),
}

#[derive(Debug, Clone, PartialEq, Parser)]
pub struct Options {
    /// How many concurrent threads should be started for inserts
    #[clap(short, long)]
    pub concurrency: usize,

    /// How many rows should be generated and inserted
    #[clap(short, long)]
    pub rows: usize,

    /// This config value has probably the biggest impact in very write heavy scenarios. Every
    /// 'logs_until_snapshot' log entries, Hiqlite will trigger a snapshot of the whole DB and purge
    /// old logs.
    #[clap(short, long, default_value = "10000")]
    pub logs_until_snapshot: u64,
}

#[derive(Debug, Clone, PartialEq, Parser)]
pub struct OptionsRemote {
    /// How many concurrent threads should be started for inserts
    #[clap(short, long)]
    pub concurrency: usize,

    /// How many rows should be generated and inserted
    #[clap(short, long)]
    pub rows: usize,

    /// The remote cluster nodes
    #[clap(short, long)]
    pub nodes: Vec<String>,

    /// If TLS should be used for the connection
    #[clap(short, long, default_value = "false")]
    pub tls: bool,

    /// Disable TLS certificate validation
    #[clap(long = "no-verify", default_value = "false")]
    pub tls_no_verify: bool,

    /// The API secret to access the remote cluster
    #[clap(short = 's', long = "secret")]
    pub api_secret: String,

    /// Set to true to connect to the DB cluster through a Hiqlite proxy
    #[clap(short = 'p', long = "proxy")]
    pub proxy: bool,
}

fn test_nodes() -> Vec<Node> {
    vec![
        Node {
            id: 1,
            addr_api: "127.0.0.1:8101".to_string(),
            addr_raft: "127.0.0.1:8201".to_string(),
        },
        Node {
            id: 2,
            addr_api: "127.0.0.1:8102".to_string(),
            addr_raft: "127.0.0.1:8202".to_string(),
        },
        Node {
            id: 3,
            addr_api: "127.0.0.1:8103".to_string(),
            addr_raft: "127.0.0.1:8203".to_string(),
        },
    ]
}

async fn node_config(nodes: Vec<Node>, logs_until_snapshot: u64) -> NodeConfig {
    // If you are doing very write heavy stuff with many operations, you can do a lot with the
    // `raft_config.snapshot_policy` value. Each so many inserts, the Raft will actually do a
    // snapshot of the state machine and purge logs. The more often this is done, the less space
    // on disk is used and the faster a database can be rebuilt, but the more it will have an
    // impact for very high write scenarios, because these snapshots and purging do take time and
    // compute.
    // By default, Hiqlite triggers a snapshot every 10k logs.
    let mut raft_config = NodeConfig::default_raft_config(logs_until_snapshot);

    // These 3 values have a quite big impact as well.
    // They decide how quickly a leader switch-over will happen, which means lower downtime in
    // case of a node crash, but also higher possibility of unnecessary switches because of short
    // term network issues.
    // Also, if you are writing a huge amount of data, the Raft may start lagging in the current
    // implementation if the `heartbeat_interval` is too short. In the next iteration, we will be
    // able to get rid of this, but this is unstable right now. You may see this especially with
    // the `bench` example when writing lots of data concurrently.
    raft_config.heartbeat_interval = 500;
    raft_config.election_timeout_min = 1500;
    raft_config.election_timeout_max = 2500;

    // This value may be interesting when you are able to execute high amounts of batched writes.
    raft_config.max_payload_entries = 128;

    let mut config = NodeConfig::from_toml("../../hiqlite.toml", None, None)
        .await
        .unwrap();

    config.node_id = 1;
    config.nodes = nodes;
    config.raft_config = raft_config;
    config.data_dir = format!("data/node_{}", 1).into();
    config.log_statements = false;

    // Hiqlite Caches are (by default) disk-backed. This means they provide the consistency of Raft
    // and can rebuild their in-memory data after a restart and never lose it. With
    // `config.cache_storage_disk = true`, the Cache WAL files + Snapshots will be persisted to
    // disk. This is of course quite a bit slower than keeping everything in-memory only, but in
    // return, you get lower memory usage and higher consistency + you never lose state.
    //
    // You can also keep everything in-memory only, which will be a lot faster, but can lead to
    // instabilities in the whole Raft cluster, if a node cannot do a graceful shutdown (and leave
    // the cluster cleanly). This is due to the lost Raft state and coming up in an inconsistent
    // state after restart, because the current Leader expects the node to still have the last known
    // state applied.
    //
    config.cache_storage_disk = false;

    // The only reason we set the WAL size to 8MB here is because of the possibly huge transactional
    // inserts with a high row count and low concurrency. At least for `hiqlite-wal`, the WAL size
    // should not have any negative impact on performance, as long as you don't go crazy low like
    // only a few kB. This value basically just needs tuning to each application specifically.
    // The default value here is `2MB`. If you adjust it, you should do it in combination with
    // `HQL_LOGS_UNTIL_SNAPSHOT` / `logs_until_snapshot`. Try to aim for having max 3-4 WAL files
    // around at all times. Exact values cannot be given, since it depends on the size of DB
    // queries your application uses.
    //
    // The only very important thing to note is, that the `wal_size` should be at least 3x your
    // biggest query size. If a query does not fit inside a single WAL, you will get a panic at
    // runtime. Usually, this should never be any issue at all, as long as you're not writing very
    // huge batch queries for instance.
    config.wal_size = 8 * 1024 * 1024;

    // `hiqlite::LogSync::Immediate` will sync immediately after a chunk of logs to append has
    // been written, but with a huge performance penalty. `hiqlite::LogSync::Immediate` will do the
    // same but not wait for completion, and therefore not block. This will usually have no
    // impact on performance at all.
    // However, both `hiqlite::LogSync::Immediate` + `hiqlite::LogSync::ImmediateAsync` will put
    // a lot of stress on your SSD in case of high traffic. The default is to sync every 200ms.
    //
    //config.wal_sync = hiqlite::LogSync::IntervalMillis(200);

    config
}

#[derive(Debug, strum::EnumIter)]
enum Cache {
    One,
    Two,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(EnvFilter::from("error"))
        .init();

    let args = Args::parse();
    if let Args::Remote(opts) = args {
        log(format!("Connecting to remote cluster: {:?}", opts.nodes));
        let client = Client::remote(
            opts.nodes,
            opts.tls,
            opts.tls_no_verify,
            opts.api_secret,
            true,
        )
        .await?;

        // slower systems need a second when they run all 3 nodes at once
        time::sleep(Duration::from_secs(1)).await;

        let options = Options {
            concurrency: opts.concurrency,
            rows: opts.rows,
            logs_until_snapshot: 10_000,
        };

        client.migrate::<Migrations>().await?;

        bench::start_benchmark(client, options, true).await?;
    } else {
        let (full_cluster, options) = match Args::parse() {
            Args::Cluster(opts) => (true, opts),
            Args::Single(opts) => (false, opts),
            Args::Remote(_) => unreachable!(),
        };

        let (client_1, client_2, _client_3) =
            start_cluster(full_cluster, options.logs_until_snapshot).await?;

        let leader = {
            let metrics = client_1.metrics_db().await?;
            let leader = metrics.current_leader.unwrap();
            if leader == 1 {
                client_1
            } else {
                client_2.unwrap()
            }
        };

        leader.migrate::<Migrations>().await?;

        bench::start_benchmark(leader, options, false).await?;
    }

    time::sleep(Duration::from_secs(3)).await;

    Ok(())
}

/// Start the local cluster and wait for all nodes to have joined and be healthy
async fn start_cluster(
    full_cluster: bool,
    logs_until_snapshot: u64,
) -> Result<(Client, Option<Client>, Option<Client>), Error> {
    // make sure to clean up data from older runs
    let _ = fs::remove_dir_all("data").await;

    let config = node_config(test_nodes(), logs_until_snapshot).await;
    let client_1 = start_node_with_cache::<Cache>(config).await?;
    let mut client_2 = None;
    let mut client_3 = None;

    let expected_nodes = if full_cluster {
        let mut config = node_config(test_nodes(), logs_until_snapshot).await;
        client_2 = task::spawn(async move {
            config.node_id = 2;
            config.data_dir = format!("data/node_{}", 2).into();
            let client = start_node_with_cache::<Cache>(config).await.unwrap();
            Some(client)
        })
        .await?;

        let mut config = node_config(test_nodes(), logs_until_snapshot).await;
        client_3 = task::spawn(async move {
            config.node_id = 3;
            config.data_dir = format!("data/node_{}", 3).into();
            let client = start_node_with_cache::<Cache>(config).await.unwrap();
            Some(client)
        })
        .await?;

        3
    } else {
        1
    };

    client_1.wait_until_healthy_db().await;
    client_1.wait_until_healthy_cache().await;

    let mut members = 1;
    while members != expected_nodes {
        let metrics = client_1.metrics_db().await?;
        members = metrics.membership_config.nodes().count();

        log(format!(
            "Waiting for other nodes to join the cluster. Nodes joined: {members}"
        ));
        time::sleep(Duration::from_secs(1)).await;
    }

    if let Some(client) = &client_2 {
        client.is_healthy_db().await?;
        client.is_healthy_cache().await?;
    }
    if let Some(client) = &client_3 {
        client.is_healthy_db().await?;
        client.is_healthy_cache().await?;
    }

    log("All Cluster Members online");

    Ok((client_1, client_2, client_3))
}

// this way of logging makes our logs easier to see with all the raft logging enabled
fn log<S: Display>(s: S) {
    println!("\n\n>>> {s}\n");
}
