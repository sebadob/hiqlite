// // Copyright 2024 Sebastian Dobe <sebastiandobe@mailbox.org>
//
// use clap::Parser;
// use hiqlite::{start_node, Error, Node, NodeConfig, RaftConfig};
// use tracing_subscriber::EnvFilter;
//
// #[derive(Parser, Clone, Debug)]
// #[clap(author, version, about, long_about = None)]
// pub struct CliArgs {
//     /// this node's ID - must exist in `nodes`
//     #[clap(long)]
//     pub id: u64,
//
//     /// All Raft member nodes given as JSON
//     #[clap(long)]
//     pub nodes: Vec<serde_json::Value>,
// }
//
#[tokio::main]
async fn main() {}
// async fn main() -> Result<(), Error> {
//     // Setup the logger
//     tracing_subscriber::fmt()
//         .with_target(true)
//         .with_level(true)
//         .with_env_filter(EnvFilter::from_default_env())
//         .init();
//
//     // Parse the parameters passed by arguments.
//     let args = CliArgs::parse();
//
//     let nodes = {
//         let mut res: Vec<Node> = Vec::with_capacity(3);
//         for node in args.nodes {
//             res.push(serde_json::from_value(node).unwrap());
//         }
//         res
//     };
//
//     // TODO update all of this to make it actually usable when installed via cargo
//     // TODO separate Cli Args into server and client via shell
//     let raft_config = NodeConfig {
//         node_id: args.id,
//         nodes,
//         data_dir: format!("data/{}", args.id).into(),
//         filename_db: "mydb".into(),
//         config: RaftConfig {
//             // heartbeat_interval: 250,
//             // election_timeout_min: 299,
//             ..Default::default()
//         },
//         tls: false,
//         secret_raft: "aaaaaaaaaaaaaaaaaaaaa".to_string(),
//         secret_api: "aaaaaaaaaaaaaaaaaaaaa".to_string(),
//     };
//
//     let _client = start_node(raft_config, true).await?;
//     Ok(())
// }
