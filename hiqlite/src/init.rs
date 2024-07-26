use crate::app_state::{AppState, RaftType};
use crate::network::management::LearnerReq;
use crate::network::HEADER_NAME_SECRET;
use crate::{helpers, Error, Node, NodeId};
use openraft::{Membership, Raft};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, warn};

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

/// Initializes a fresh node 1, if it has not been set up yet.
#[cfg(feature = "sqlite")]
pub async fn init_pristine_node_1_db(
    raft: &Raft<TypeConfigSqlite>,
    this_node: u64,
    nodes: &[Node],
    secret_api: &str,
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    if this_node == 1 {
        let this_node = get_this_node(this_node, nodes);

        if is_initialized_timeout_sqlite(raft).await? {
            return Ok(());
        }

        if should_node_1_skip_init(&RaftType::Sqlite, nodes, secret_api, tls, tls_no_verify).await?
        {
            warn!("node 1 (DB) should skip its own init - found existing cluster on remotes");
            return Ok(());
        }

        let mut nodes_set = BTreeMap::new();
        nodes_set.insert(this_node.id, this_node);
        raft.initialize(nodes_set).await?;
    }

    Ok(())
}

// TODO this duplication is not pretty but getting the types correct is pretty hard
/// Initializes a fresh node 1, if it has not been set up yet.
#[cfg(feature = "cache")]
pub async fn init_pristine_node_1_cache(
    raft: &Raft<TypeConfigKV>,
    this_node: u64,
    nodes: &[Node],
    secret_api: &str,
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    if this_node == 1 {
        let this_node = get_this_node(this_node, nodes);

        // in case of cache raft, a node will never be initialized after start up

        if should_node_1_skip_init(&RaftType::Cache, nodes, secret_api, tls, tls_no_verify).await? {
            warn!("node 1 (cache) should skip its own init - found existing cluster on remotes");
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
        .cloned()
        .expect("this node to always exist in all nodes");
    (*node).clone()
}

async fn should_node_1_skip_init(
    raft_type: &RaftType,
    nodes: &[Node],
    secret_api: &str,
    tls: bool,
    tls_no_verify: bool,
) -> Result<bool, Error> {
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .danger_accept_invalid_certs(tls_no_verify)
        .build()
        .unwrap();

    // no need for +1 since this very node is the +1
    let quorum = nodes.len() / 2;

    let scheme = if tls { "https" } else { "http" };
    let mut skip_nodes = vec![1];

    loop {
        for node in nodes {
            if skip_nodes.contains(&node.id) {
                continue;
            }

            let url = format!(
                "{}://{}/cluster/membership/{}",
                scheme,
                node.addr_api,
                raft_type.as_str()
            );
            debug!("checking membership via {}", url);

            let res = client
                .get(url)
                .header(HEADER_NAME_SECRET, secret_api)
                .send()
                .await;
            match res {
                Ok(resp) => {
                    debug!("{} status: {}", node.id, resp.status());
                    if resp.status().is_success() {
                        let body = resp.bytes().await?;
                        let membership: Membership<NodeId, Node> =
                            bincode::deserialize(body.as_ref()).unwrap();

                        // If one of our remote nodes is already initialized, we need to check the total
                        // nodes it is already connected to and if node 1 is in the list.
                        // If it is, it means that this very node has lost its volume and need to
                        // re-join the cluster.
                        let contains_this = membership
                            .nodes()
                            .filter(|(id, _node)| **id == 1)
                            .collect::<Vec<(&u64, &Node)>>();

                        if contains_this.is_empty() {
                            panic!(
                                r#"
        Remote member is already initialized but does not contain this node in its members.
        This can only happen with a bad configuration or if the cluster has been modified manually.
        Please add node 1 as a learner to the cluster to fix this issue.
                            "#
                            );
                        } else {
                            // if this node is already a remote member but is not initialized, it has lost
                            // its volume and needs to join remote -> skip our own init
                            return Ok(true);
                        }
                    } else {
                        let body = resp.bytes().await?;
                        let err: Error = serde_json::from_slice(&body).unwrap();
                        // if let Ok(Error::Config(err)) = bincode::deserialize::<Error>(body.as_ref())
                        // {
                        error!("{}", err);
                        skip_nodes.push(node.id);
                        // }
                    }
                }
                Err(err) => {
                    error!("Error sending membership request: {}", err);
                }
            }

            if skip_nodes.len() >= quorum {
                return Ok(false);
            }
        }

        time::sleep(Duration::from_secs(1)).await;
    }
}

/// If this node is a non cluster member, it will try to become a learner and
/// a voting member afterward.
pub async fn become_cluster_member(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    this_node: u64,
    nodes: &[Node],
    tls: bool,
    tls_no_verify: bool,
    secret_api: &str,
) -> Result<(), Error> {
    tracing::info!("\n\nbecome_cluster_member {}\n\n", raft_type.as_str());

    if is_initialized_timeout(state, raft_type).await? {
        tracing::info!(
            "\n\nbecome_cluster_member {} is initialized\n\n",
            raft_type.as_str()
        );
        return Ok(());
    } else {
        tracing::info!(
            "\n\nbecome_cluster_member {} is NOT initialized\n\n",
            raft_type.as_str()
        );
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
        state,
        raft_type,
        &client,
        scheme,
        "add_learner",
        &payload,
        this_node.id,
        nodes,
        secret_api,
        true,
    )
    .await?;

    try_become(
        state,
        raft_type,
        &client,
        scheme,
        "become_member",
        &payload,
        this_node.id,
        nodes,
        secret_api,
        false,
    )
    .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn try_become(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    // raft: &Raft<TypeConfigSqlite>,
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
        if check_init && helpers::is_raft_initialized(state, raft_type).await? {
            return Ok(());
        }

        for node in nodes {
            if node.id == this_node {
                continue;
            }

            let url = format!(
                "{}://{}/cluster/{}/{}",
                scheme,
                node.addr_api,
                suffix,
                raft_type.as_str()
            );
            debug!("Sending request to {}", url);

            tracing::info!("\n\ntry_become {}: {}\n\n", raft_type.as_str(), url);

            let res = client
                .post(&url)
                .header(HEADER_NAME_SECRET, secret_api)
                .body(payload.to_vec())
                .send()
                .await;

            tracing::info!(
                "\n\ntry_become {} response: {:?}\n\n",
                raft_type.as_str(),
                res
            );

            match res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        tracing::info!(
                            "\n\ntry_become {} successful: {:?}\n\n",
                            raft_type.as_str(),
                            resp
                        );
                        debug!("becoming a member has been successful");
                        return Ok(());
                    } else {
                        let body = resp.bytes().await?;
                        let err: Error = serde_json::from_slice(&body).unwrap();
                        // error!("\n\nNode {} -> {}\n\n{:?}\n\n", this_node, url, err);
                        // if let Some((id, _)) = err.is_forward_to_leader() {
                        //     if id.is_none() {
                        //         info!("Vote in progress, stepping back");
                        //         time::sleep(Duration::from_secs(1)).await;
                        //     }
                        // } else {
                        error!(
                            "Node {} become '{}' member on remote ({}): {}",
                            this_node,
                            raft_type.as_str(),
                            url,
                            err
                        );
                    }
                }
                Err(err) => {
                    error!("Node connection error: {}", err);
                }
            }
        }
    }
}

async fn is_initialized_timeout(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> Result<bool, Error> {
    // Do not try to initialize already initialized nodes
    if helpers::is_raft_initialized(state, raft_type).await? {
        return Ok(true);
    }

    // If it is not initialized, wait long enough to make sure this
    // node is not joined again to an already existing cluster after data loss.
    time::sleep(Duration::from_secs(1)).await;

    // Make sure we are not initialized by now, otherwise go on
    if helpers::is_raft_initialized(state, raft_type).await? {
        Ok(true)
    } else {
        Ok(false)
    }
}

// TODO get rid of the duplication here and make it prettier -> figure out generic types properly

#[cfg(feature = "sqlite")]
async fn is_initialized_timeout_sqlite(raft: &Raft<TypeConfigSqlite>) -> Result<bool, Error> {
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

// #[cfg(feature = "cache")]
// async fn is_initialized_timeout_kv(raft: &Raft<TypeConfigKV>) -> Result<bool, Error> {
//     // Do not try to initialize already initialized nodes
//     if raft.is_initialized().await? {
//         return Ok(true);
//     }
//
//     // If it is not initialized, wait long enough to make sure this
//     // node is not joined again to an already existing cluster after data loss.
//     let heartbeat = raft.config().heartbeat_interval;
//     // We will wait for 5 heartbeats to make sure no other cluster is running
//     time::sleep(Duration::from_millis(heartbeat * 5)).await;
//
//     // Make sure we are not initialized by now, otherwise go on
//     if raft.is_initialized().await? {
//         Ok(true)
//     } else {
//         Ok(false)
//     }
// }

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
