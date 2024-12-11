use crate::app_state::{AppState, RaftType};
use crate::network::management::LearnerReq;
use crate::network::HEADER_NAME_SECRET;
use crate::{helpers, Error, Node, NodeId};
use openraft::Membership;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error};

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(any(feature = "cache", feature = "sqlite"))]
use std::collections::BTreeMap;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use tracing::info;

pub type IsPristineNode1 = bool;

/// Initializes a fresh node 1, if it has not been set up yet.
#[cfg(feature = "sqlite")]
pub async fn init_pristine_node_1_db(
    raft: &openraft::Raft<TypeConfigSqlite>,
    this_node: u64,
    nodes: &[Node],
    secret_api: &str,
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    if this_node == 1 {
        let this_node = get_this_node(this_node, nodes);

        if is_initialized_timeout_sqlite(raft).await? {
            info!("node 1 raft is already initialized");
            return Ok(());
        }

        if should_node_1_skip_init(&RaftType::Sqlite, nodes, secret_api, tls, tls_no_verify).await?
        {
            info!("node 1 (DB) should skip its own init - found existing cluster on remotes");
            return Ok(());
        }

        info!("initializing pristine node 1 raft");
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
    raft: &openraft::Raft<TypeConfigKV>,
    this_node: u64,
    nodes: &[Node],
    secret_api: &str,
    tls: bool,
    tls_no_verify: bool,
) -> Result<IsPristineNode1, Error> {
    if this_node == 1 {
        let this_node = get_this_node(this_node, nodes);

        // in case of cache raft, a node will never be initialized after start up

        if should_node_1_skip_init(&RaftType::Cache, nodes, secret_api, tls, tls_no_verify).await? {
            info!("node 1 (cache) should skip its own init - found existing cluster on remotes");
            return Ok(false);
        }

        let mut nodes_set = BTreeMap::new();
        nodes_set.insert(this_node.id, this_node);
        raft.initialize(nodes_set).await?;

        Ok(true)
    } else {
        Ok(false)
    }
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
    if nodes.len() < 2 {
        return Ok(false);
    }

    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .danger_accept_invalid_certs(tls_no_verify)
        .build()?;

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
                            bincode::deserialize(body.as_ref())?;

                        if membership.nodes().count() > 0 {
                            // We could check if the remote members are at least of size "quorum",
                            // but this could possibly lead to a situation where you would not be
                            // able to recover a cluster with only 1 healthy node left, which is a
                            // possible situation.
                            return Ok(true);
                        } else {
                            panic!(
                                "The remote node {} is initialized but has no configured members. \
                            This should never happen, but it may occur for a cache-only node and \
                            a too fast restart -> wait at least for the leader heartbeat timeout.",
                                node.id
                            );
                        }
                    } else {
                        let body = resp.bytes().await?;
                        let err: Error = serde_json::from_slice(&body)?;
                        error!("{}", err);
                        // TODO should we even track "quorum" nodes or simply join if any configured
                        // remote node is already initialized?
                        skip_nodes.push(node.id);
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

#[derive(Debug, PartialEq)]
enum SkipBecome {
    Yes,
    No,
}

/// If this node is not a cluster member, it will try to become a learner and
/// a voting member afterward.
#[tracing::instrument(skip(state, nodes, tls, tls_no_verify))]
pub async fn become_cluster_member(
    state: Arc<AppState>,
    raft_type: &RaftType,
    this_node: u64,
    nodes: &[Node],
    is_pristine_cache_node_1: IsPristineNode1,
    election_timeout_max: u64,
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    // A cache node may return initialized here if an existing leaders opens the client stream
    // before we can do the check, but this will lead to an inconsistent state, because the cache
    // layer does not keep any state between restarts - cache nodes will always be empty and
    // in-memory. Therefore, we always need to do a new cluster join for cache nodes.
    //
    // However, the situation is different for a pristine node 1 - cache and in-memory only.
    // In this situation, the node will always be initialized but will fail joining its own,
    // not yet existent cluster later on in the client.
    #[cfg(feature = "sqlite")]
    let check_init = raft_type == &RaftType::Sqlite || is_pristine_cache_node_1;
    #[cfg(not(feature = "sqlite"))]
    let check_init = is_pristine_cache_node_1;

    if check_init && is_initialized_timeout(&state, raft_type, election_timeout_max).await? {
        info!(
            "{} raft is already initialized - skipping become_cluster_member()",
            raft_type.as_str()
        );
        return Ok(());
    }

    // TODO if raft type is cache:
    // - pristine node 1 first init will always be initialized here, no matter what
    // - nodes joining an existing cluster must always re-join - even node 1
    // - somehow check here, if this is a pristine node 1 or a node 1 re-joining an existing cluster

    // If this node is neither node 1 nor initialized, we always want to reach
    // out to node 1 and yell at it, that we want to join the party as well.
    // During a normal init, this is not necessary, but it is in case of a node
    // recovery from failure in case the leader does not recognize our issues.
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .danger_accept_invalid_certs(tls_no_verify)
        .build()?;
    let scheme = if tls { "https" } else { "http" };

    let this_node = get_this_node(this_node, nodes);
    let payload = bincode::serialize(&LearnerReq {
        node_id: this_node.id,
        addr_api: this_node.addr_api,
        addr_raft: this_node.addr_raft,
    })?;

    info!("Trying to become {} raft learner", raft_type.as_str());
    let skip = try_become(
        &state,
        raft_type,
        &client,
        scheme,
        "add_learner",
        &payload,
        this_node.id,
        nodes,
        true,
    )
    .await?;
    info!("Successfully became {} raft learner", raft_type.as_str());

    // Again, for the same reason as above, an im-memory cache member must always do a full
    // re-join after restarts.
    #[cfg(feature = "sqlite")]
    if skip == SkipBecome::Yes {
        // can happen in a race condition situation during a rolling release
        info!("Became a Raft member in the meantime - skipping further init");
        return Ok(());
    }

    info!("Trying to become {} raft member", raft_type.as_str());
    try_become(
        &state,
        raft_type,
        &client,
        scheme,
        "become_member",
        &payload,
        this_node.id,
        nodes,
        false,
    )
    .await?;
    info!("Successfully became {} raft member", raft_type.as_str());

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
    check_init: bool,
) -> Result<SkipBecome, Error> {
    loop {
        // maybe we are initialized in the meantime
        if check_init && helpers::is_raft_initialized(state, raft_type).await? {
            info!(
                "Init check at loop start in try_become - this node became the raft leader in \
            the meantime - skipping init"
            );
            return Ok(SkipBecome::Yes);
        }

        for node in nodes {
            if node.id == this_node {
                debug!("Skipping 'this' node");
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

            let res = client
                .post(&url)
                .header(HEADER_NAME_SECRET, &state.secret_api)
                .body(payload.to_vec())
                .send()
                .await;
            debug!("raw request to {}: {:?}", url, res);

            match res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        debug!("becoming a member has been successful");
                        return Ok(SkipBecome::No);
                    } else {
                        let body = resp.bytes().await?;
                        let err: Error = serde_json::from_slice(&body)?;
                        error!(
                            "Node {} become '{}' member on remote ({}): {}",
                            this_node,
                            raft_type.as_str(),
                            url,
                            err
                        );

                        // We can get into this situation when using the cache layer, because it has
                        // no persistence. This race condition can happen for a rolling release
                        // on K8s for instance. While this node may try to become a remote member,
                        // the raft has decided that this node is the new leader.
                        //
                        // -> We must check this after each error to get smooth rolling releases.
                        if let Some((Some(leader_id), Some(node))) = err.is_forward_to_leader() {
                            if leader_id == this_node {
                                info!("This node became the raft leader in the meantime - skipping init");

                                if !helpers::is_raft_initialized(state, raft_type).await? {
                                    let leader = helpers::get_raft_leader(state, raft_type).await;
                                    let metrics = helpers::get_raft_metrics(state, raft_type).await;

                                    panic!(
                                        r#"
    Raft is not initialized when remote node has 'this' as leader.
    This can only happen for an in-memory cache node and a too fast restart.
    Because the in-memory Raft does not save the state between restarts, you must way at least
    for the duration of a leader heartbeat timeout before trying to re-join the cluster.

    This node: {this_node}
    Leader:    {leader:?}: {node:?}
    Metrics:   {metrics:?}
"#
                                    );
                                }

                                return Ok(SkipBecome::Yes);
                            }
                        }

                        time::sleep(Duration::from_millis(500)).await;
                    }
                }
                Err(err) => {
                    error!("Node connection error: {}", err);

                    time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
}

async fn is_initialized_timeout(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    election_timeout_max: u64,
) -> Result<bool, Error> {
    // Do not try to initialize already initialized nodes
    if helpers::is_raft_initialized(state, raft_type).await? {
        return Ok(true);
    }

    // If it is not initialized, wait long enough to make sure this
    // node is not joined again to an already existing cluster after data loss.
    time::sleep(Duration::from_millis(election_timeout_max * 2)).await;

    // Make sure we are not initialized by now, otherwise go on
    if helpers::is_raft_initialized(state, raft_type).await? {
        Ok(true)
    } else {
        Ok(false)
    }
}

// TODO get rid of the duplication here and make it prettier -> figure out generic types properly

#[cfg(feature = "sqlite")]
async fn is_initialized_timeout_sqlite(
    raft: &openraft::Raft<TypeConfigSqlite>,
) -> Result<bool, Error> {
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
