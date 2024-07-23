use crate::app_state::AppState;
use crate::network::management::LearnerReq;
use crate::network::HEADER_NAME_SECRET;
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::{init, Error, Node};
use openraft::Raft;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{error, info};

/// Initializes a fresh node 1, if it has not been set up yet.
pub async fn init_pristine_node_1(
    raft: &Raft<TypeConfigSqlite>,
    this_node: u64,
    nodes: &[Node],
) -> Result<(), Error> {
    // TODO will probably be an issue if node 1 died and needs to join an existing cluster
    // TODO -> add remote lookup when the metrics endpoint is implemented
    if this_node == 1 {
        let this_node = get_this_node(this_node, nodes);

        if is_initialized_timeout(raft).await? {
            return Ok(());
        }

        let mut nodes_set = BTreeMap::new();
        nodes_set.insert(this_node.id, this_node);
        raft.initialize(nodes_set).await?;
    }

    Ok(())
}

fn get_this_node(this_node: u64, nodes: &[Node]) -> Node {
    let filtered = nodes
        .iter()
        .filter(|node| node.id == this_node)
        .collect::<Vec<&Node>>();
    let node = filtered
        .first()
        .expect("this node to always exist in all nodes")
        .clone();
    (*node).clone()
}

/// If this node is a non cluster member, it will try to become a learner and
/// a voting member afterward.
pub async fn become_cluster_member(
    // state: &Arc<AppState>,
    raft: &Raft<TypeConfigSqlite>,
    this_node: u64,
    nodes: &[Node],
    tls: bool,
    tls_no_verify: bool,
    secret_api: &str,
) -> Result<(), Error> {
    if is_initialized_timeout(raft).await? {
        return Ok(());
    }

    // If this node is neither node 1 nor initialized, we always want to reach
    // out to node 1 and yell at it, that we want to join the party as well.
    // During a normal init, this is not necessary, but it is in case of a node
    // recovery from failure in case the leader does not recognize our issues.
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .danger_accept_invalid_certs(tls_no_verify)
        .build()
        .unwrap();
    let scheme = if tls { "https" } else { "http" };

    let this_node = get_this_node(this_node, nodes);
    let payload = bincode::serialize(&LearnerReq {
        node_id: this_node.id,
        addr_api: this_node.addr_api,
        addr_raft: this_node.addr_raft,
    })
    .unwrap();

    try_become(
        raft,
        &client,
        scheme,
        "add_learner",
        &payload,
        this_node.id,
        &nodes,
        secret_api,
        true,
    )
    .await?;

    try_become(
        raft,
        &client,
        scheme,
        "become_member",
        &payload,
        this_node.id,
        &nodes,
        secret_api,
        false,
    )
    .await?;

    Ok(())
}

async fn try_become(
    // state: &Arc<AppState>,
    raft: &Raft<TypeConfigSqlite>,
    client: &reqwest::Client,
    scheme: &str,
    suffix: &str,
    payload: &[u8],
    this_node: u64,
    nodes: &[Node],
    secret_api: &str,
    check_init: bool,
) -> Result<(), Error> {
    loop {
        time::sleep(Duration::from_secs(1)).await;
        // maybe we got initialized in the meantime
        if check_init && raft.is_initialized().await? {
            return Ok(());
        }

        for node in nodes {
            if node.id == this_node {
                continue;
            }

            let url = format!("{}://{}/cluster/{}", scheme, node.addr_api, suffix);
            info!("Sending request to {}", url);

            let res = client
                .post(url)
                .header(HEADER_NAME_SECRET, secret_api)
                .body(payload.to_vec())
                .send()
                .await;

            match res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!("Becoming was successful");
                        return Ok(());
                    } else {
                        let body = resp.bytes().await?;
                        let err = bincode::deserialize::<Error>(&body)?;

                        if let Some((id, _)) = err.is_forward_to_leader() {
                            if id.is_none() {
                                info!("Vote in progress, stepping back");
                                time::sleep(Duration::from_secs(1)).await;
                            }
                        } else {
                            error!("Becoming error: {}", err);
                        }
                    }
                }
                Err(err) => {
                    error!("Node connection error: {}", err);
                }
            }
        }
    }
}

// async fn is_initialized_timeout(state: &AppState) -> Result<bool, Error> {
//     // Do not try to initialize already initialized nodes
//     if state.raft.is_initialized().await? {
//         return Ok(true);
//     }
//
//     // If it is not initialized, wait long enough to make sure this
//     // node is not joined again to an already existing cluster after data loss.
//     let heartbeat = state.raft.config().heartbeat_interval;
//     // We will wait for 5 heartbeats to make sure no other cluster is running
//     time::sleep(Duration::from_millis(heartbeat * 5)).await;
//
//     // Make sure we are not initialized by now, otherwise go on
//     if state.raft.is_initialized().await? {
//         Ok(true)
//     } else {
//         Ok(false)
//     }
// }

async fn is_initialized_timeout(raft: &Raft<TypeConfigSqlite>) -> Result<bool, Error> {
    // Do not try to initialize already initialized nodes
    if raft.is_initialized().await? {
        return Ok(true);
    }

    // If it is not initialized, wait long enough to make sure this
    // node is not joined again to an already existing cluster after data loss.
    let heartbeat = raft.config().heartbeat_interval;
    // We will wait for 5 heartbeats to make sure no other cluster is running
    time::sleep(Duration::from_millis(heartbeat * 5)).await;

    // Make sure we are not initialized by now, otherwise go on
    if raft.is_initialized().await? {
        Ok(true)
    } else {
        Ok(false)
    }
}

// async fn wait_for_nodes_online(state: &AppState, nodes: &[Node], tls: bool, tls_no_verify: bool) {
//     let scheme = if tls { "https" } else { "http" };
//     let remotes = nodes
//         .iter()
//         .filter_map(|node| {
//             (node.id != state.id).then_some(format!("{}://{}/ping", scheme, node.addr_raft))
//         })
//         .collect::<Vec<String>>();
//     let mut remotes_online = 0;
//
//     let client = reqwest::Client::builder()
//         .danger_accept_invalid_certs(tls_no_verify)
//         .http2_prior_knowledge()
//         .build()
//         .unwrap();
//     while remotes_online != remotes.len() {
//         info!("Waiting for remote nodes {:?} to become reachable", remotes);
//
//         remotes_online = 0;
//         time::sleep(Duration::from_secs(1)).await;
//
//         for node in &remotes {
//             if client.get(node).send().await.is_ok() {
//                 remotes_online += 1;
//             }
//         }
//     }
//
//     info!("All remote nodes are reachable");
// }
