use clap::Parser;
use hiqlite::macros::{embed::*, CacheVariants};
use hiqlite::{start_node_with_cache, Client, Error, Node, NodeConfig, RateLimitConfig};
use std::fmt::{Debug, Display};
use std::fs::File;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::fs;
use tokio::time;

mod bench;

#[derive(Embed)]
#[folder = "migrations"]
struct Migrations;

/// The node ids of the cluster this example builds.
const NODE_IDS: [u64; 3] = [1, 2, 3];

#[derive(Debug, Clone, PartialEq, Parser)]
#[clap(author, version, about, long_about = None)]
enum Args {
    /// Spawn one OS process per node on this host and run the benchmark against them. This is the
    /// shape that measures replication: the client never shares a thread with a node.
    Cluster(Options),
    /// Start a single node and stay up. `cluster` spawns these processes; you can also run them by
    /// hand to benchmark a cluster you assembled yourself.
    Node(NodeOptions),
    /// Start tests with a Single Node, inside this process
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

    /// Limit the number of cache inflight requests per second
    #[clap(long)]
    pub cache_rps: Option<u32>,

    /// Limit the number of cache inflight burst requests
    #[clap(long)]
    pub cache_burst: Option<u32>,

    /// Limit the number of DB inflight requests per second
    #[clap(long)]
    pub db_rps: Option<u32>,

    /// Limit the number of DB inflight burst requests
    #[clap(long)]
    pub db_burst: Option<u32>,

    /// How many rows should be generated and inserted
    #[clap(short, long)]
    pub rows: usize,

    /// This config value has probably the biggest impact in very write heavy scenarios. Every
    /// 'logs_until_snapshot' log entries, Hiqlite will trigger a snapshot of the whole DB and purge
    /// old logs.
    #[clap(short, long, default_value = "10000")]
    pub logs_until_snapshot: u64,

    /// openraft's `max_payload_entries`: the biggest number of log entries a single AppendEntries
    /// RPC carries. Raft throughput is measured against this value, so the benchmark can sweep it.
    #[clap(long, default_value = "128")]
    pub max_payload_entries: u64,

    /// Only run the cache part of the benchmark. The cache is in-memory, so this measures the Raft
    /// replication of a group that never waits on a disk.
    #[clap(long, default_value = "false")]
    pub cache_only: bool,

    /// The tokio worker threads each node process gets. A node is CPU-bound on its apply path, so
    /// give every node at least one core of its own.
    #[clap(long, default_value = "8")]
    pub node_threads: usize,

    /// The tokio worker threads the benchmark client gets. Keep these well below the core count:
    /// every thread here is a thread that is not available to a node.
    #[clap(long, default_value = "4")]
    pub client_threads: usize,

    /// CPU lists to pin the node processes to, one per node, e.g. `0-7,8-15,16-23`. Linux only:
    /// with this set, each node is started under `taskset -c`, which is the only real core pinning
    /// available. Empty means no pinning, and the budgets above are the only lever.
    #[clap(long, default_value = "")]
    pub node_cpus: String,
}

#[derive(Debug, Clone, PartialEq, Parser)]
pub struct NodeOptions {
    /// The id of this node. It must be one of the ids in the cluster's node list
    #[clap(long)]
    pub id: u64,

    /// The tokio worker threads of this node process
    #[clap(long, default_value = "8")]
    pub threads: usize,

    /// The snapshot interval of this node's Raft
    #[clap(long, default_value = "10000")]
    pub logs_until_snapshot: u64,

    /// openraft's `max_payload_entries` of this node's Raft
    #[clap(long, default_value = "128")]
    pub max_payload_entries: u64,
}

#[derive(Debug, Clone, PartialEq, Parser)]
pub struct OptionsRemote {
    /// How many concurrent threads should be started for inserts
    #[clap(short, long)]
    pub concurrency: usize,

    /// Limit the number of cache inflight requests per second
    #[clap(long)]
    pub cache_rps: Option<u32>,

    /// Limit the number of cache inflight burst requests
    #[clap(long)]
    pub cache_burst: Option<u32>,

    /// Limit the number of DB inflight requests per second
    #[clap(long)]
    pub db_rps: Option<u32>,

    /// Limit the number of DB inflight burst requests
    #[clap(long)]
    pub db_burst: Option<u32>,

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

    /// Only run the cache part of the benchmark
    #[clap(long, default_value = "false")]
    pub cache_only: bool,
}

/// The part of the CLI that a node's `NodeConfig` is built from. Both the `cluster` options and the
/// `node` options describe a node, and a node must be configured identically no matter which of the
/// two started it.
struct NodeSetup {
    id: u64,
    logs_until_snapshot: u64,
    max_payload_entries: u64,
    cache_rps: Option<u32>,
    cache_burst: Option<u32>,
    db_rps: Option<u32>,
    db_burst: Option<u32>,
}

impl From<&Options> for NodeSetup {
    fn from(options: &Options) -> Self {
        Self {
            id: 1,
            logs_until_snapshot: options.logs_until_snapshot,
            max_payload_entries: options.max_payload_entries,
            cache_rps: options.cache_rps,
            cache_burst: options.cache_burst,
            db_rps: options.db_rps,
            db_burst: options.db_burst,
        }
    }
}

impl From<&NodeOptions> for NodeSetup {
    fn from(options: &NodeOptions) -> Self {
        Self {
            id: options.id,
            logs_until_snapshot: options.logs_until_snapshot,
            max_payload_entries: options.max_payload_entries,
            cache_rps: None,
            cache_burst: None,
            db_rps: None,
            db_burst: None,
        }
    }
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

async fn node_config(nodes: Vec<Node>, setup: &NodeSetup) -> NodeConfig {
    // If you are doing very write heavy stuff with many operations, you can do a lot with the
    // `raft_config.snapshot_policy` value. Each so many inserts, the Raft will actually do a
    // snapshot of the state machine and purge logs. The more often this is done, the less space
    // on disk is used and the faster a database can be rebuilt, but the more it will have an
    // impact for very high write scenarios, because these snapshots and purging do take time and
    // compute.
    // By default, Hiqlite triggers a snapshot every 10k logs.
    let mut raft_config = NodeConfig::default_raft_config(setup.logs_until_snapshot);

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
    raft_config.max_payload_entries = setup.max_payload_entries;

    let mut config = NodeConfig::from_toml("../../hiqlite.toml", None, None, None)
        .await
        .unwrap();

    config.node_id = setup.id;
    config.nodes = nodes;
    config.raft_config = raft_config;
    config.data_dir = format!("data/node_{}", setup.id).into();
    config.log_statements = false;

    if let Some(rps) = setup.cache_rps {
        config.rate_limit_cache = Some(RateLimitConfig {
            rps,
            burst: setup.cache_burst.unwrap_or(rps),
        });
    }
    if let Some(rps) = setup.db_rps {
        config.rate_limit_db = Some(RateLimitConfig {
            rps,
            burst: setup.db_burst.unwrap_or(rps),
        });
    }

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
    // `cache_storage_disk = false` alone keeps the Cache's log store and state machine in memory,
    // and its Snapshots still go to disk. This example builds with the `in-memory-snapshots`
    // feature, so `false` here really keeps the whole Cache group off the disk, Snapshots included,
    // which is what a write-throughput benchmark needs.
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
    // been written, but with a huge performance penalty. `hiqlite::LogSync::ImmediateAsync` will
    // do the same but not wait for completion, and therefore not block. This will usually have no
    // impact on performance at all.
    // However, both `hiqlite::LogSync::Immediate` + `hiqlite::LogSync::ImmediateAsync` will put
    // a lot of stress on your SSD in case of high traffic. The default is to sync every 200ms.
    //
    config.wal_sync = hiqlite::LogSync::IntervalMillis(200);

    config
}

#[allow(unused)]
#[derive(Debug, CacheVariants)]
enum Cache {
    One,
    Two,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    // A run is only comparable when every thread of it sits on a performance core: the efficiency
    // cores of an Apple chip are around 3.5x slower, and macOS has no thread affinity to pin a
    // process with. The lever there is the thread budget - a busy process at the default QoS is
    // scheduled onto the performance cores, so the budget is sized to the number of those cores,
    // and a QoS clamp like `taskpolicy -c maintenance` does the opposite of what it sounds like: it
    // throttles a process onto the efficiency cores. On Linux, pin the nodes with `--node-cpus` and
    // the budgets are yours to pick.
    let workers = match &args {
        Args::Cluster(opts) => opts.client_threads,
        Args::Node(opts) => opts.threads,
        Args::Single(_) | Args::Remote(_) => default_workers(),
    };
    log(format!("Using {workers} runtime worker threads"));

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .expect("Cannot build the tokio runtime")
        .block_on(run(args))
}

/// The worker budget for the modes that run a node inside their own process: the performance cores
/// of an Apple chip, falling back to the number of logical CPUs.
fn default_workers() -> usize {
    std::env::var("HQL_BENCH_WORKERS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(perf_core_count)
}

/// The number of performance cores on Apple Silicon, read from `sysctl`. Falls back to the number
/// of logical CPUs everywhere else, and never returns 0.
fn perf_core_count() -> usize {
    let perf_cores = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.perflevel0.logicalcpu"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<usize>()
                .ok()
        })
        .filter(|n| *n > 0);

    perf_cores.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    })
}

async fn run(args: Args) -> Result<(), Error> {
    #[cfg(feature = "tokio-console")]
    console_subscriber::init();

    #[cfg(not(feature = "tokio-console"))]
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error")),
        )
        .init();

    match args {
        Args::Node(opts) => run_node(&opts).await,
        Args::Cluster(opts) => run_cluster(opts).await,
        Args::Single(opts) => run_single(opts).await,
        Args::Remote(opts) => run_remote(opts).await,
    }
}

/// Start one node and stay up. `cluster` spawns these, and a run ends when its parent kills it.
async fn run_node(options: &NodeOptions) -> Result<(), Error> {
    let config = node_config(test_nodes(), &NodeSetup::from(options)).await;
    let _client = start_node_with_cache::<Cache>(config).await?;
    log(format!("Node {} is up", options.id));

    loop {
        time::sleep(Duration::from_secs(3600)).await;
    }
}

/// Spawn one process per node, then benchmark them through a plain remote client. The client and
/// the nodes are separate processes, so no phase of the run steals a core from replication.
async fn run_cluster(options: Options) -> Result<(), Error> {
    // Clean slate for every run. The nodes create their data dirs after this.
    let _ = fs::remove_dir_all("data").await;
    fs::create_dir_all("data").await?;

    // Nodes are terminated when this guard drops, on a successful run and on a failing one alike.
    let _children = Children(spawn_nodes(&options)?);

    let client = cluster_client().await?;
    wait_for_cluster(&client).await?;

    client.migrate::<Migrations>().await?;
    bench::start_benchmark(client, options, true).await?;

    time::sleep(Duration::from_secs(3)).await;
    Ok(())
}

async fn run_single(options: Options) -> Result<(), Error> {
    let (client, _, _) = start_cluster(&options).await?;

    client.migrate::<Migrations>().await?;
    bench::start_benchmark(client, options, false).await?;

    time::sleep(Duration::from_secs(3)).await;

    // Sleep for longer to have more time to inspect with tokio-console
    #[cfg(feature = "tokio-console")]
    time::sleep(Duration::from_secs(300)).await;

    Ok(())
}

async fn run_remote(options: OptionsRemote) -> Result<(), Error> {
    log(format!("Connecting to remote cluster: {:?}", options.nodes));

    let rate_limit_cache = options.cache_rps.map(|rps| RateLimitConfig {
        rps,
        burst: options.cache_burst.unwrap_or(rps),
    });
    let rate_limit_db = options.db_rps.map(|rps| RateLimitConfig {
        rps,
        burst: options.db_burst.unwrap_or(rps),
    });

    let client = Client::remote(
        options.nodes.clone(),
        options.tls,
        options.tls_no_verify,
        options.api_secret.clone(),
        true,
        rate_limit_cache,
        rate_limit_db,
    )
    .await?;

    // slower systems need a second when they run all 3 nodes at once
    time::sleep(Duration::from_secs(1)).await;

    let bench_options = Options {
        concurrency: options.concurrency,
        cache_rps: options.cache_rps,
        cache_burst: options.cache_burst,
        db_rps: options.db_rps,
        db_burst: options.db_burst,
        rows: options.rows,
        logs_until_snapshot: 10_000,
        max_payload_entries: 128,
        cache_only: options.cache_only,
        // A remote run drives a cluster it does not own, so it has no node processes to size.
        node_threads: 8,
        client_threads: default_workers(),
        node_cpus: String::new(),
    };

    client.migrate::<Migrations>().await?;
    bench::start_benchmark(client, bench_options, true).await?;

    time::sleep(Duration::from_secs(3)).await;
    Ok(())
}

/// The node processes of a `cluster` run. Dropping this terminates them.
struct Children(Vec<Child>);

impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Start one process per node. Each node writes its own log next to its data, so a node that dies
/// during startup is readable.
fn spawn_nodes(options: &Options) -> Result<Vec<Child>, Error> {
    let exe = std::env::current_exe()
        .map_err(|err| Error::Error(format!("Cannot find own executable: {err}").into()))?;
    let cpus = cpu_lists(&options.node_cpus);

    if !cpus.is_empty() && !cfg!(target_os = "linux") {
        log("--node-cpus is Linux only, the nodes start unpinned");
    }

    let mut children = Vec::with_capacity(NODE_IDS.len());
    for (idx, id) in NODE_IDS.iter().enumerate() {
        let log = File::create(format!("data/node_{id}.log")).map_err(|err| {
            Error::Error(format!("Cannot open the log of node {id}: {err}").into())
        })?;

        let mut cmd = Command::new("taskset");
        if cfg!(target_os = "linux") && !cpus.is_empty() {
            let list = cpus.get(idx).ok_or_else(|| {
                Error::Error(format!("--node-cpus has no entry for node {id}").into())
            })?;
            cmd.arg("-c").arg(list).arg(&exe);
        } else {
            cmd = Command::new(&exe);
        }

        let log_dup = log.try_clone().map_err(|err| {
            Error::Error(format!("Cannot duplicate the log of node {id}: {err}").into())
        })?;

        let child = cmd
            .args([
                "node",
                "--id",
                &id.to_string(),
                "--threads",
                &options.node_threads.to_string(),
                "--logs-until-snapshot",
                &options.logs_until_snapshot.to_string(),
                "--max-payload-entries",
                &options.max_payload_entries.to_string(),
            ])
            .stdout(Stdio::from(log_dup))
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|err| Error::Error(format!("Cannot spawn node {id}: {err}").into()))?;
        children.push(child);
    }

    log(format!("Spawned {} node processes", children.len()));
    Ok(children)
}

/// `--node-cpus` as a list, one entry per node.
fn cpu_lists(node_cpus: &str) -> Vec<String> {
    node_cpus
        .split(',')
        .map(|list| list.trim().to_string())
        .filter(|list| !list.is_empty())
        .collect()
}

/// A plain remote client against the nodes this run spawned.
async fn cluster_client() -> Result<Client, Error> {
    let config = NodeConfig::from_toml("../../hiqlite.toml", None, None, None)
        .await
        .map_err(|err| Error::Error(format!("Cannot read the bench config: {err}").into()))?;
    let nodes = test_nodes().into_iter().map(|node| node.addr_api).collect();

    // The bench's nodes run with `tls_auto_certificates`, so a client speaks TLS to them and cannot
    // verify the self-signed certificates it gets.
    Client::remote(nodes, true, true, config.secret_api, true, None, None).await
}

/// Poll until every node has joined and the cluster answers on both of its groups.
async fn wait_for_cluster(client: &Client) -> Result<(), Error> {
    let mut members = 0;
    while members != NODE_IDS.len() {
        match client.metrics_db().await {
            Ok(metrics) => members = metrics.membership_config.nodes().count(),
            Err(err) => log(format!("Cluster not answering yet: {err}")),
        }
        log(format!(
            "Waiting for nodes to join the cluster. Nodes joined: {members}"
        ));
        time::sleep(Duration::from_secs(1)).await;
    }

    client.wait_until_healthy_db().await;
    client.wait_until_healthy_cache().await;
    log("All Cluster Members online");

    Ok(())
}

/// Start a single node inside this process and wait for it to be healthy.
async fn start_cluster(
    options: &Options,
) -> Result<(Client, Option<Client>, Option<Client>), Error> {
    // make sure to clean up data from older runs
    let _ = fs::remove_dir_all("data").await;

    let config = node_config(test_nodes(), &NodeSetup::from(options)).await;
    let client_1 = start_node_with_cache::<Cache>(config).await?;

    client_1.wait_until_healthy_db().await;
    client_1.wait_until_healthy_cache().await;

    Ok((client_1, None, None))
}

// this way of logging makes our logs easier to see with all the raft logging enabled
fn log<S: Display>(s: S) {
    println!("\n\n>>> {s}\n");
}
