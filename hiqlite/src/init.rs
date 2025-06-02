use crate::app_state::{AppState, RaftType};
use crate::helpers::{deserialize, serialize};
use crate::network::management::{ClusterLeaveReq, LearnerReq};
use crate::network::HEADER_NAME_SECRET;
use crate::{helpers, Error, Node, NodeId};
use openraft::{Membership, RaftMetrics};
use std::fmt::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, warn};

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(any(feature = "cache", feature = "sqlite"))]
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use tracing::info;

/// Initializes a fresh node 1, if it has not been set up yet.
#[cfg(feature = "sqlite")]
pub async fn init_pristine_node_1_db(
    raft: &openraft::Raft<TypeConfigSqlite>,
    node_id: u64,
    nodes: &[Node],
    secret_api: &str,
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    if node_id == 1 {
        let this_node = get_this_node(node_id, nodes);

        if is_initialized_timeout_sqlite(node_id, raft).await? {
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
    wal_on_disk: bool,
    node_id: u64,
    nodes: &[Node],
    secret_api: &str,
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    if node_id == 1 {
        let this_node = get_this_node(node_id, nodes);

        if wal_on_disk && is_initialized_timeout_cache(node_id, raft).await? {
            info!("node 1 raft is already initialized");
            return Ok(());
        }

        if should_node_1_skip_init(&RaftType::Cache, nodes, secret_api, tls, tls_no_verify).await? {
            info!("node 1 (cache) should skip its own init - found existing cluster on remotes");
            return Ok(());
        }

        info!("initializing pristine node 1 raft");
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

#[tracing::instrument(skip(nodes, secret_api, tls, tls_no_verify))]
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
                        let membership: Membership<NodeId, Node> = deserialize(body.as_ref())?;

                        if membership.nodes().count() > 0 {
                            return Ok(true);
                        } else {
                            panic!(
                                "The remote node {} is initialized but has no configured members. \
                            This should never happen",
                                node.id
                            );
                        }
                    } else {
                        let body = resp.bytes().await?;
                        let err: Error = serde_json::from_slice(&body)?;
                        error!("{}", err);
                        skip_nodes.push(node.id);
                    }
                }
                Err(err) => {
                    warn!("Error sending membership request: {}", err);
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
#[allow(clippy::too_many_arguments)]
pub async fn become_cluster_member(
    state: Arc<AppState>,
    raft_type: &RaftType,
    this_node: u64,
    nodes: &[Node],
    tls: bool,
    tls_no_verify: bool,
) -> Result<(), Error> {
    if helpers::is_raft_initialized(&state, raft_type).await? {
        info!(
            "Node {}: {} Raft is already initialized - skipping become_cluster_member()",
            state.id,
            raft_type.as_str(),
        );
        set_raft_running(&state, raft_type);
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .danger_accept_invalid_certs(tls_no_verify)
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(30))
        .build()?;
    let scheme = if tls { "https" } else { "http" };

    if is_remote_cluster_member(&state, raft_type, &client, scheme, this_node, nodes).await {
        leave_remote_cluster(
            &state, raft_type, &client, scheme, this_node, nodes, 10, false,
        )
        .await
        .expect("Cannot leave remote cluster");
    }
    set_raft_running(&state, raft_type);

    let this_node = get_this_node(this_node, nodes);
    let payload = serialize(&LearnerReq {
        node_id: this_node.id,
        addr_api: this_node.addr_api,
        addr_raft: this_node.addr_raft,
    })?;

    info!(
        "Node {}: Trying to become {} raft learner",
        state.id,
        raft_type.as_str()
    );
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
    if skip == SkipBecome::Yes {
        info!(
            "Node {}: Became a {:?} Raft member in the meantime - skipping further init",
            state.id, raft_type,
        );
        return Ok(());
    }
    info!(
        "Node {}: Successfully became {} raft learner",
        state.id,
        raft_type.as_str()
    );

    // If we try to become a member too fast and the request arrives on remote directly in between
    // closing and re-opening the socket to us again, and it then also badly overlaps with the raft
    // membership modification, we can get into a deadlock situation on the leader.
    // We want to wait until we are a commited Raft learner.
    {
        let mut metrics = helpers::get_raft_metrics(&state, raft_type).await;
        let mut are_we_learner = metrics
            .membership_config
            .nodes()
            .any(|(id, _)| *id == state.id);
        while !are_we_learner {
            info!("Waiting until we are a commited Raft Learner ...");
            time::sleep(Duration::from_secs(1)).await;
            metrics = helpers::get_raft_metrics(&state, raft_type).await;
            are_we_learner = metrics
                .membership_config
                .nodes()
                .any(|(id, _)| *id == state.id);
        }
    }

    info!(
        "Node {}: Trying to become {:?} raft member",
        state.id, raft_type
    );
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
    info!(
        "Node {}: Successfully became {} raft member",
        state.id,
        raft_type.as_str()
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn try_become(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    client: &reqwest::Client,
    scheme: &str,
    suffix: &str,
    payload: &[u8],
    this_node: u64,
    nodes: &[Node],
    check_init: bool,
) -> Result<SkipBecome, Error> {
    let mut url = String::with_capacity(48);
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

            url.clear();
            write!(
                url,
                "{}://{}/cluster/{}/{}",
                scheme,
                node.addr_api,
                suffix,
                raft_type.as_str()
            )?;
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

                        // We can get into this situation when using the cache layer, because it has
                        // no persistence. This race condition can happen for a rolling release
                        // on K8s for instance. While this node may try to become a remote member,
                        // the raft has decided that this node is the new leader.
                        //
                        // -> We must check this after each error to get smooth rolling releases.
                        if let Some((Some(leader_id), Some(node))) = err.is_forward_to_leader() {
                            info!(
                                "Node {} become '{}' member on remote ({}): Remote Node is not the leader - trying next",
                                this_node,
                                raft_type.as_str(),
                                url,
                            );

                            // should never happen at this point
                            if leader_id == this_node {
                                if !helpers::is_raft_initialized(state, raft_type).await? {
                                    let leader = helpers::get_raft_leader(state, raft_type).await;
                                    let metrics = helpers::get_raft_metrics(state, raft_type).await;

                                    panic!(
                                        r#"
    Raft is not initialized when remote node has 'this' as leader.
    This can only happen for an in-memory cache node and a too fast restart.
    Because the in-memory Raft does not save the state between restarts, you must way at least
    for the duration of a leader heartbeat timeout before trying to re-join the cluster.

    Raft Type: {raft_type:?}
    This node: {this_node}
    Leader:    {leader:?}: {node:?}
    Metrics:   {metrics:?}
"#
                                    );
                                }

                                info!("This node became the raft leader in the meantime - skipping init");

                                return Ok(SkipBecome::Yes);
                            }
                        } else {
                            error!(
                                "Node {} become '{}' member on remote ({}): {}",
                                this_node,
                                raft_type.as_str(),
                                url,
                                err
                            );
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

/// Make sure this function does not return a Result, which could get us into a locked situation,
/// because the Raft is set to stopped while we check this.
#[tracing::instrument(skip(state, client, scheme))]
async fn is_remote_cluster_member(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    client: &reqwest::Client,
    scheme: &str,
    this_node: u64,
    nodes: &[Node],
) -> bool {
    let mut url = String::with_capacity(48);

    // "This" Node is the +1 for quorum
    let quorum = nodes.len() / 2;

    // check remote metrics for our existence first
    let mut not_initialized_remotes = 0;
    for node in nodes {
        if node.id == this_node {
            debug!("Skipping 'this' node");
            continue;
        }
        if not_initialized_remotes >= quorum {
            info!(
                "Found {} remote Nodes that are not initialized - must be a fresh cluster",
                not_initialized_remotes
            );
        }

        url.clear();
        write!(
            url,
            "{}://{}/cluster/metrics/{}",
            scheme,
            node.addr_api,
            raft_type.as_str()
        )
        .expect("Cannot write into String");

        let res = client
            .get(&url)
            .header(HEADER_NAME_SECRET, &state.secret_api)
            .send()
            .await;

        match res {
            Ok(resp) => {
                if resp.status().is_success() {
                    let Ok(bytes) = resp.bytes().await else {
                        error!("Success response from remote without body");
                        time::sleep(Duration::from_secs(1)).await;
                        continue;
                    };
                    let metrics = deserialize::<RaftMetrics<u64, Node>>(bytes.as_ref())
                        .expect("Cannot deserialize remote metrics response");
                    let is_member = metrics
                        .membership_config
                        .nodes()
                        .any(|(id, _)| *id == this_node);
                    if is_member {
                        // if there already is a remote cluster and we are not part of it,
                        // everything should be fine
                        warn!("Found remote metrics and we ({this_node}) are a Raft member");
                        return true;
                    } else {
                        info!("Found remote metrics, but we ({this_node}) are not a Raft member");
                        return false;
                    }
                } else {
                    // We reached the remote node, but it was not possible to get cluster metrics.
                    // This can only mean, that remote is not initialized as well.
                    not_initialized_remotes += 1;

                    let body = resp
                        .bytes()
                        .await
                        .expect("API answer to always have a body");
                    let err: Error =
                        serde_json::from_slice(&body).expect("To always get back a JSON error");
                    error!(
                        "Error retrieving {:?} Raft metrics from remote Node {}: {:?}",
                        raft_type, node.id, err
                    );

                    time::sleep(Duration::from_secs(1)).await;
                }
            }
            Err(err) => {
                error!("Node connection error: {}", err);
                time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    false
}

#[tracing::instrument(skip(state, client, scheme))]
#[allow(clippy::too_many_arguments)]
pub async fn leave_remote_cluster(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    client: &reqwest::Client,
    scheme: &str,
    this_node: u64,
    nodes: &[Node],
    retries: usize,
    stay_as_learner: bool,
) -> Result<(), Error> {
    let mut url = String::with_capacity(48);

    let payload = serialize(&ClusterLeaveReq {
        node_id: this_node,
        stay_as_learner,
    })?;
    let mut left_cluster = false;
    for _ in 0..retries + 1 {
        for node in nodes {
            if node.id == this_node {
                debug!("Skipping 'this' node");
                continue;
            }

            // We will just try to send our request to all nodes in order without looking up
            // the leader via metrics first, as this can change at any time anyway. The request
            // will only succeed, if the remote node is a leader anyway.
            url.clear();
            write!(
                url,
                "{}://{}/cluster/membership/{}",
                scheme,
                node.addr_api,
                raft_type.as_str()
            )?;

            let res = client
                .delete(&url)
                .header(HEADER_NAME_SECRET, &state.secret_api)
                .body(payload.clone())
                .send()
                .await;

            match res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!(
                            "This Node {this_node} left the remote {:?} cluster via {}",
                            raft_type, url
                        );
                        left_cluster = true;
                        break;
                    } else {
                        let body = resp.bytes().await?;
                        let err: Error = serde_json::from_slice(&body)?;
                        error!(
                            "Error removing this Node {} from remote {:?} Raft cluster: {:?}",
                            this_node,
                            raft_type.as_str(),
                            err
                        );
                    }
                }
                Err(err) => {
                    error!("Node {:?} connection error to {}: {}", raft_type, url, err);
                }
            }
        }
    }
    if !left_cluster {
        return Err(Error::Connect(
            "Could not leave the cluster after trying all nodes once".to_string(),
        ));
    }

    // After removal, query metrics again until this node is fully removed.
    // We need to do this on all nodes, because we don't know if any of them leave or join
    // in the meantime.
    for _ in 0..retries + 1 {
        for node in nodes {
            if node.id == this_node {
                debug!("Skipping 'this' node");
                continue;
            }

            url.clear();
            write!(
                url,
                "{}://{}/cluster/metrics/{}",
                scheme,
                node.addr_api,
                raft_type.as_str()
            )?;

            let Ok(res) = client
                .get(&url)
                .header(HEADER_NAME_SECRET, &state.secret_api)
                .send()
                .await
            else {
                error!(
                    "Unable to reach Node {} via {} to confirm cluster leave via metrics",
                    node.id, url
                );
                continue;
            };

            if res.status().is_success() {
                let bytes = res.bytes().await?;
                let metrics = deserialize::<RaftMetrics<u64, Node>>(bytes.as_ref())?;
                let is_member = metrics
                    .membership_config
                    .nodes()
                    .any(|(id, _)| *id == this_node);
                if is_member {
                    info!("This Node ({this_node}) is still a Raft member after removal - waiting ...");
                    time::sleep(Duration::from_secs(1)).await;
                    continue;
                } else {
                    info!("This Node ({this_node}) has been fully removed from the Raft.");
                    return Ok(());
                }
            }
        }
        time::sleep(Duration::from_secs(1)).await;
    }

    error!(
        "Node was removed from the cluster, but was unable to confirm this via metrics - retries exceeded"
    );

    Ok(())
}

// TODO get rid of the duplication here and make it prettier -> figure out generic types properly

#[cfg(feature = "sqlite")]
async fn is_initialized_timeout_sqlite(
    node_id: u64,
    raft: &openraft::Raft<TypeConfigSqlite>,
) -> Result<bool, Error> {
    let has_any_nodes = || {
        raft.metrics()
            .borrow()
            .membership_config
            .membership()
            .nodes()
            .any(|(id, _)| *id == node_id)
    };

    // Do not try to initialize already initialized nodes
    if raft.is_initialized().await? && has_any_nodes() {
        return Ok(true);
    }

    // If it is not initialized, wait long enough to make sure this
    // node is not joined again to an already existing cluster after data loss.
    let heartbeat = raft.config().heartbeat_interval;
    // We will wait for 5 heartbeats to make sure no other cluster is running
    time::sleep(Duration::from_millis(heartbeat * 5)).await;

    // Make sure we are not initialized by now, otherwise go on
    if raft.is_initialized().await? {
        if has_any_nodes() {
            Ok(true)
        } else {
            warn!("Raft is initialized but the membership config is empty");
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

#[cfg(feature = "cache")]
async fn is_initialized_timeout_cache(
    node_id: u64,
    raft: &openraft::Raft<TypeConfigKV>,
) -> Result<bool, Error> {
    let has_any_nodes = || {
        raft.metrics()
            .borrow()
            .membership_config
            .membership()
            .nodes()
            .any(|(id, _)| *id == node_id)
    };

    // Do not try to initialize already initialized nodes
    if raft.is_initialized().await? && has_any_nodes() {
        return Ok(true);
    }

    // If it is not initialized, wait long enough to make sure this
    // node is not joined again to an already existing cluster after data loss.
    let heartbeat = raft.config().heartbeat_interval;
    // We will wait for 5 heartbeats to make sure no other cluster is running
    time::sleep(Duration::from_millis(heartbeat * 5)).await;

    // Make sure we are not initialized by now, otherwise go on
    if raft.is_initialized().await? {
        if has_any_nodes() {
            Ok(true)
        } else {
            warn!("Raft is initialized but the membership config is empty");
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

fn set_raft_running(state: &Arc<AppState>, raft_type: &RaftType) {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            info!("Setting Sqlite Raft to running");
            state
                .raft_db
                .is_raft_stopped
                .store(false, Ordering::Relaxed)
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            info!("Setting Cache Raft to running");
            state
                .raft_cache
                .is_raft_stopped
                .store(false, Ordering::Relaxed)
        }
        RaftType::Unknown => unreachable!(),
    }
}
