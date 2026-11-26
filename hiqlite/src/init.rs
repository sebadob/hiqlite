use crate::app_state::{AppState, RaftType};
use crate::helpers::{deserialize_serde, serialize};
use crate::network::WireMetrics;
use crate::network::management::{ClusterLeaveReq, LearnerReq};
use crate::{Error, Node, NodeConfig, NodeId, helpers};
use openraft::Membership;
use openraft::type_config::async_runtime::watch::WatchReceiver;
use std::fmt::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::{fs, time};
use tracing::{debug, error, warn};

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;
#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::state_machine::StateMachineSqlite;

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigCache;
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::state_machine::StateMachineMemory;

use crate::http_client::build_http_client;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use std::collections::BTreeMap;
use std::env;
use std::sync::atomic::Ordering;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use tracing::info;

/// Checks if a Raft Logs / Metadata reset should be performed and deletes all logs if true.
pub async fn check_execute_reset(base_path: &str) -> Result<bool, Error> {
    let do_reset = env::var("HQL_DANGER_RAFT_STATE_RESET")
        .as_deref()
        .unwrap_or("false")
        .parse::<bool>()
        .expect("Cannot parse HQL_DANGER_RAFT_STATE_RESET as u64");
    if !do_reset {
        return Ok(false);
    }

    warn!(
        r"

    !!! CAUTION !!!

    Performing a full Raft State reset! If used incorrectly, this option
    can destroy your cluster and end up with an inconsistent state!

    Base target directory: {base_path}

    Continuing in 10 seconds ...

    "
    );
    time::sleep(Duration::from_secs(10)).await;

    if let Err(err) = fs::remove_dir_all(format!("{base_path}/logs")).await {
        error!("Error during Raft Logs cleanup: {err:?}");
    }
    if let Err(err) = fs::remove_dir_all(format!("{base_path}/logs_cache")).await {
        error!("Error during Raft Logs cleanup: {err:?}");
    }
    if let Err(err) = fs::remove_dir_all(format!("{base_path}/state_machine/snapshots")).await {
        error!("Error during Raft Logs cleanup: {err:?}");
    }
    if let Err(err) = fs::remove_dir_all(format!("{base_path}/state_machine_cache/snapshots")).await
    {
        error!("Error during Raft Logs cleanup: {err:?}");
    }
    // A manual reset wipes the persisted cache data, so drop the cache-index fingerprint too;
    // otherwise a stale file would be compared against on the next (data-free) startup.
    if let Err(err) =
        fs::remove_file(format!("{base_path}/state_machine_cache/cache_index.meta")).await
    {
        // A missing metadata file is expected on a pristine node; only log real errors.
        if err.kind() != std::io::ErrorKind::NotFound {
            error!("Error during Raft State reset (cache index metadata): {err:?}");
        }
    }

    Ok(true)
}

/// Initializes a fresh node 1, if it has not been set up yet.
#[cfg(feature = "sqlite")]
pub async fn init_pristine_node_1_db(
    raft: &openraft::Raft<TypeConfigSqlite, StateMachineSqlite>,
    node_config: &NodeConfig,
) -> Result<(), Error> {
    let node_id = node_config.node_id;
    if node_id == 1 {
        let this_node = get_this_node(node_id, &node_config.nodes)?;

        if is_initialized_timeout_sqlite(node_id, raft).await? {
            info!("node 1 raft is already initialized");
            return Ok(());
        }

        if should_node_1_skip_init(&RaftType::Sqlite, &InitCheck::new(node_config)).await? {
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
    raft: &openraft::Raft<TypeConfigCache, Arc<StateMachineMemory>>,
    node_config: &NodeConfig,
) -> Result<(), Error> {
    let node_id = node_config.node_id;
    if node_id == 1 {
        let this_node = get_this_node(node_id, &node_config.nodes)?;

        if node_config.cache_storage_disk && is_initialized_timeout_cache(node_id, raft).await? {
            info!("node 1 raft is already initialized");
            return Ok(());
        }

        if should_node_1_skip_init(&RaftType::Cache, &InitCheck::new(node_config)).await? {
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

fn get_this_node(this_node: u64, nodes: &[Node]) -> Result<Node, Error> {
    nodes
        .iter()
        .find(|node| node.id == this_node)
        .cloned()
        .ok_or_else(|| {
            Error::Config(
                format!("This node ({this_node}) is not part of the configured node list").into(),
            )
        })
}

/// How many consecutive rounds every remote must be unreachable before a pristine node 1 forms
/// the cluster on its own. With a 1 second pause between rounds this is the same 10 seconds after
/// which `/ready` reports a pristine node 1 as ready.
const ALL_UNREACHABLE_ROUNDS: u32 = 10;

/// What [`should_node_1_skip_init`] needs from the [`NodeConfig`].
pub(crate) struct InitCheck<'a> {
    pub nodes: &'a [Node],
    pub secret_api: &'a str,
    pub tls: bool,
    pub tls_no_verify: bool,
    /// [`NodeConfig::init_require_peers`]
    pub require_peers: bool,
}

#[cfg(any(feature = "cache", feature = "sqlite"))]
impl<'a> InitCheck<'a> {
    fn new(node_config: &'a NodeConfig) -> Self {
        Self {
            nodes: &node_config.nodes,
            secret_api: &node_config.secret_api,
            tls: node_config.tls_api.is_some(),
            tls_no_verify: node_config
                .tls_api
                .as_ref()
                .is_some_and(|c| c.danger_tls_no_verify()),
            require_peers: node_config.init_require_peers,
        }
    }
}

/// What one remote answered to the membership check.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PeerAnswer {
    /// The remote is part of an initialized cluster.
    Initialized,
    /// The remote said its Raft group has never been initialized.
    NotInitialized,
    /// The remote could not be reached at all.
    Unreachable,
    /// The remote answered, but not with one of the above, e.g. a wrong secret or an initialized
    /// node whose Raft is not running yet. It proves nothing either way.
    Inconclusive,
}

/// The outcome of one round of asking every remote.
#[derive(Debug, PartialEq)]
enum InitRound {
    /// A remote belongs to an initialized cluster: node 1 must join it.
    Skip,
    /// A quorum of the other nodes is pristine: node 1 forms the cluster.
    Init,
    /// No remote answered at all.
    AllUnreachable,
    /// Not enough evidence yet: ask again.
    Wait,
}

/// Decides one round from the answers of all remotes (node 1 itself excluded).
///
/// A connection error never counts towards initializing. An unreachable remote may well be a
/// member of an existing cluster, and node 1 forming a second one next to it is a split brain
/// that Raft cannot detect afterwards: a fresh cluster's first leader is always node 1 in term 1,
/// so both clusters write the same log ids with different payloads and the log matching check
/// never fires.
fn decide_init_round(answers: &[PeerAnswer]) -> InitRound {
    if answers.contains(&PeerAnswer::Initialized) {
        return InitRound::Skip;
    }
    if answers.iter().all(|a| *a == PeerAnswer::Unreachable) {
        return InitRound::AllUnreachable;
    }

    // a majority of the other nodes, not of the whole cluster: node 1 has no data, so it can not
    // vouch for anything itself
    let quorum = answers.len() / 2 + 1;
    let not_initialized = answers
        .iter()
        .filter(|a| **a == PeerAnswer::NotInitialized)
        .count();
    if not_initialized >= quorum {
        InitRound::Init
    } else {
        InitRound::Wait
    }
}

/// Asks one remote for its membership.
async fn ask_peer_membership(client: &reqwest::Client, url: &str, secret_api: &str) -> PeerAnswer {
    let resp = match client.get(url).bearer_auth(secret_api).send().await {
        Ok(resp) => resp,
        Err(err) => {
            debug!("Error sending membership request to {url}: {err}");
            return PeerAnswer::Unreachable;
        }
    };

    let status = resp.status();
    let Ok(body) = resp.bytes().await else {
        return PeerAnswer::Inconclusive;
    };

    if status.is_success() {
        return match deserialize_serde::<Membership<NodeId, Node>>(body.as_ref()) {
            Ok(membership) if membership.nodes().count() > 0 => PeerAnswer::Initialized,
            Ok(_) => {
                error!("{url} is initialized but has no configured members");
                PeerAnswer::Inconclusive
            }
            Err(err) => {
                error!("Cannot deserialize the membership from {url}: {err}");
                PeerAnswer::Inconclusive
            }
        };
    }

    match serde_json::from_slice::<Error>(&body) {
        Ok(Error::RaftNotInitialized) => PeerAnswer::NotInitialized,
        Ok(err) => {
            warn!("Membership check on {url} answered {status}: {err}");
            PeerAnswer::Inconclusive
        }
        Err(_) => {
            warn!("Membership check on {url} answered {status} with an unknown body");
            PeerAnswer::Inconclusive
        }
    }
}

/// Decides whether a node 1 without any Raft state must skip its own `initialize()`, because the
/// cluster already exists on the other nodes.
///
/// Every remote is asked in each round. Node 1 skips as soon as one of them belongs to an
/// initialized cluster, and initializes once a quorum of the other nodes positively answered
/// that they were never initialized. While neither is the case, it keeps asking.
///
/// The one exception is a node 1 that cannot reach any remote at all for
/// [`ALL_UNREACHABLE_ROUNDS`] rounds. That is how a brand new cluster looks when its nodes are
/// started one after the other, e.g. a Kubernetes `StatefulSet` with
/// `podManagementPolicy: OrderedReady`, and it is also the documented disaster recovery path, so
/// node 1 still forms the cluster alone in that case, unless `require_peers` is set. Then it keeps
/// asking until the other nodes answer.
#[tracing::instrument(skip(check), fields(require_peers = check.require_peers))]
async fn should_node_1_skip_init(
    raft_type: &RaftType,
    check: &InitCheck<'_>,
) -> Result<bool, Error> {
    let InitCheck {
        nodes,
        secret_api,
        tls,
        tls_no_verify,
        require_peers,
    } = *check;
    if nodes.len() < 2 {
        return Ok(false);
    }

    let client = build_http_client(tls_no_verify);
    let scheme = if tls { "https" } else { "http" };
    let remotes = nodes
        .iter()
        .filter(|node| node.id != 1)
        .map(|node| {
            (
                node.id,
                format!(
                    "{}://{}/cluster/membership/{}",
                    scheme,
                    node.addr_api,
                    raft_type.as_str()
                ),
            )
        })
        .collect::<Vec<_>>();

    let mut answers = Vec::with_capacity(remotes.len());
    let mut all_unreachable_rounds = 0;

    loop {
        answers.clear();
        for (id, url) in &remotes {
            debug!("checking membership of node {id} via {url}");
            answers.push(ask_peer_membership(&client, url, secret_api).await);
        }

        match decide_init_round(&answers) {
            InitRound::Skip => return Ok(true),
            InitRound::Init => return Ok(false),
            InitRound::AllUnreachable => {
                all_unreachable_rounds += 1;
                if require_peers {
                    if all_unreachable_rounds % ALL_UNREACHABLE_ROUNDS == 0 {
                        warn!(
                            "No other node was reachable for {all_unreachable_rounds} rounds - \
                             `init_require_peers` is set, so node 1 keeps waiting for them \
                             instead of forming a new {} cluster alone.",
                            raft_type.as_str()
                        );
                    }
                } else if all_unreachable_rounds >= ALL_UNREACHABLE_ROUNDS {
                    warn!(
                        "No other node was reachable for {ALL_UNREACHABLE_ROUNDS} rounds - \
                         forming a new {} cluster from node 1 alone. If node 1 lost its data \
                         while the other nodes still hold a cluster, they must be wiped before \
                         they rejoin.",
                        raft_type.as_str()
                    );
                    return Ok(false);
                }
            }
            InitRound::Wait => {
                all_unreachable_rounds = 0;
                info!(
                    "Waiting for a quorum of the other nodes to answer before initializing node \
                     1 - answers: {answers:?}"
                );
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

        // Wait until we have a leader before returning to the main application.
        // Only makes sense for actual HA deployments.
        if nodes.len() > 1 {
            time::sleep(Duration::from_secs(1)).await;
            let mut metrics = helpers::get_raft_view(&state, raft_type)?;
            info!("Waiting for Raft Leader");
            for _ in 0..5 {
                // Make sure that this node is not the current leader,
                // which can happen after too quick restarts.
                if let Some(id) = metrics.leader_id
                    && id != this_node
                {
                    info!("Current Raft Leader: {id}");
                    break;
                }
                time::sleep(Duration::from_millis(1000)).await;
                metrics = helpers::get_raft_view(&state, raft_type)?;
            }
        }

        return Ok(());
    }

    let client = build_http_client(tls_no_verify);
    let scheme = if tls { "https" } else { "http" };

    // It is possible that this node is un-initialized while still being a member on a remote
    // cluster. This can happen, if e.g. the volume got lost and the node was a member before
    // already.
    // We will only get here, if a Raft membership state does not exist! Therefore, we want to
    // make sure we start clean again and that the remote cluster is clean as well.
    if is_remote_cluster_member(&state, raft_type, &client, scheme, this_node, nodes).await {
        leave_remote_cluster(
            &state, raft_type, &client, scheme, this_node, nodes, 10, false,
        )
        .await?;
    }
    set_raft_running(&state, raft_type);

    let this_node = get_this_node(this_node, nodes)?;
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
    // TODO check if this is still possible with the addition of "leave before proceed"
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

    // If we try to become a member too fast and the request arrives at remote directly in between
    // closing and re-opening the socket to us again, and it then also badly overlaps with the raft
    // membership modification, we can get into a deadlock situation on the leader.
    // We want to wait until we are a commited Raft learner.
    {
        let mut metrics = helpers::get_raft_view(&state, raft_type)?;

        let mut are_we_learner = metrics.membership.nodes().any(|(id, _)| *id == state.id);
        while !are_we_learner {
            info!("Waiting until we are a replicated Raft Learner ...",);
            time::sleep(Duration::from_secs(1)).await;
            metrics = helpers::get_raft_view(&state, raft_type)?;
            are_we_learner = metrics.membership.nodes().any(|(id, _)| *id == state.id);
        }
        info!(
            "Node {}: Successfully became replicated {} raft learner",
            state.id,
            raft_type.as_str(),
        );
    }

    if state.learner_only {
        info!(
            "Node {}: learner_only=true - skipping {} raft voter promotion",
            state.id,
            raft_type.as_str(),
        );
        return Ok(());
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

    {
        let mut metrics = helpers::get_raft_view(&state, raft_type)?;

        // To smooth out startups, wait until this node has replicated its
        // own voter state logs.
        let mut are_we_voter = metrics.membership.voter_ids().any(|id| id == state.id);
        while !are_we_voter {
            info!("Waiting until we are a replicated Raft Voter ...",);
            time::sleep(Duration::from_secs(1)).await;
            metrics = helpers::get_raft_view(&state, raft_type)?;
            are_we_voter = metrics.membership.voter_ids().any(|id| id == state.id);
        }
        info!(
            "Node {}: Successfully became a replicated {} raft member",
            state.id,
            raft_type.as_str(),
        );
    }

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
                .bearer_auth(&state.secret_api)
                .body(payload.to_vec())
                .send()
                .await;
            debug!("raw request to {}: {:?}", url, res);

            match res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        debug!("becoming a member via /{suffix} was successful");
                        return Ok(SkipBecome::No);
                    }

                    let body = match resp.bytes().await {
                        Ok(body) => body,
                        Err(err) => {
                            error!(
                                "Cannot read response body from remote Node {}: {}",
                                node.id, err
                            );
                            time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                    };

                    // A load balancer or proxy in front of the API port may answer with a
                    // non-JSON error body (e.g. an HTML 502) - log it and try the next node
                    // instead of aborting the whole join.
                    let Ok(err_body) = serde_json::from_slice::<Error>(&body) else {
                        error!(
                            "Cannot deserialize JSON error from remote Node {} (is a load \
                             balancer or proxy in front of the API port?): {}",
                            node.id,
                            String::from_utf8_lossy(&body)
                        );
                        time::sleep(Duration::from_millis(500)).await;
                        continue;
                    };

                    // TODO can this still happen after we added the "leave before proceed"?
                    // We can get into this situation when using the cache layer, because it has
                    // no persistence. This race condition can happen for a rolling release
                    // on K8s for instance. While this node may try to become a remote member,
                    // the raft has decided that this node is the new leader.
                    //
                    // -> We must check this after each error to get smooth rolling releases.
                    if let Some((Some(leader_id), Some(node))) = err_body.is_forward_to_leader() {
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
                                let metrics = helpers::get_raft_view(state, raft_type)?;

                                panic!(
                                    r"
    Raft is not initialized when remote node has 'this' as leader.
    This can only happen for an in-memory cache node and a too fast restart.
    Because the in-memory Raft does not save the state between restarts, you must way at least
    for the duration of a leader heartbeat timeout before trying to re-join the cluster.

    Raft Type: {raft_type:?}
    This node: {this_node}
    Leader:    {leader:?}: {node:?}
    Metrics:   {metrics:?}
"
                                );
                            }

                            info!(
                                "This node became the raft leader in the meantime - skipping init"
                            );

                            return Ok(SkipBecome::Yes);
                        }
                    } else {
                        error!(
                            "Node {} become '{}' member on remote ({}): {}",
                            this_node,
                            raft_type.as_str(),
                            url,
                            err_body
                        );
                    }

                    time::sleep(Duration::from_millis(500)).await;
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

    // we want to re-try twice to really make sure there was no network hiccup
    for _ in 0..2 {
        for node in nodes {
            if node.id == this_node {
                debug!("Skipping 'this' node");
                continue;
            }
            if not_initialized_remotes >= quorum {
                // A quorum of remotes is not initialized, so this must be a fresh cluster, and we
                // cannot be a member of it yet - no need to keep checking the remaining nodes.
                info!(
                    "Found {} remote Nodes that are not initialized - must be a fresh cluster",
                    not_initialized_remotes
                );
                return false;
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

            let res = client.get(&url).bearer_auth(&state.secret_api).send().await;

            match res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let Ok(bytes) = resp.bytes().await else {
                            error!("Success response from remote without body");
                            time::sleep(Duration::from_secs(1)).await;
                            continue;
                        };
                        let Ok(metrics) = deserialize_serde::<WireMetrics>(bytes.as_ref()) else {
                            error!(
                                "Cannot deserialize remote metrics response from Node {}",
                                node.id
                            );
                            time::sleep(Duration::from_secs(1)).await;
                            continue;
                        };

                        let is_member = metrics
                            .membership_config
                            .nodes()
                            .any(|(id, _)| *id == this_node);
                        if is_member {
                            // if there already is a remote cluster, and we are not part of it,
                            // everything should be fine
                            warn!("Found remote metrics and we ({this_node}) are a Raft member");
                            return true;
                        }
                        info!("Found remote metrics, but we ({this_node}) are not a Raft member");
                        return false;
                    }
                    // We reached the remote node, but it was not possible to get cluster
                    // metrics. This can only mean, that remote is not initialized as well.
                    not_initialized_remotes += 1;

                    match resp.bytes().await {
                        Ok(body) => match serde_json::from_slice::<Error>(&body) {
                            Ok(err) => error!(
                                "Error retrieving {:?} Raft metrics from remote Node {}: {:?}",
                                raft_type, node.id, err
                            ),
                            Err(err) => error!(
                                "Cannot deserialize JSON error from remote Node {} (is a load \
                                 balancer or proxy in front of the API port?): {}",
                                node.id, err
                            ),
                        },
                        Err(err) => error!(
                            "Cannot read response body from remote Node {}: {}",
                            node.id, err
                        ),
                    }

                    time::sleep(Duration::from_secs(1)).await;
                }
                Err(err) => {
                    error!("Node connection error: {}", err);
                    time::sleep(Duration::from_millis(500)).await;
                }
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
    'outer: for _ in 0..=retries {
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
                .bearer_auth(&state.secret_api)
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
                        break 'outer;
                    }

                    let body = match resp.bytes().await {
                        Ok(body) => body,
                        Err(err) => {
                            error!(
                                "Cannot read response body from remote Node {}: {}",
                                node.id, err
                            );
                            continue;
                        }
                    };

                    // A load balancer or proxy in front of the API port may answer with a
                    // non-JSON error body (e.g. an HTML 502) - log it and try the next node
                    // instead of aborting the whole leave.
                    let Ok(err_body) = serde_json::from_slice::<Error>(&body) else {
                        error!(
                            "Cannot deserialize JSON error from remote Node {} (is a load \
                             balancer or proxy in front of the API port?): {}",
                            node.id,
                            String::from_utf8_lossy(&body)
                        );
                        continue;
                    };

                    error!(
                        "Error removing this Node {} from remote {:?} Raft cluster: {:?}",
                        this_node,
                        raft_type.as_str(),
                        err_body
                    );
                }
                Err(err) => {
                    error!("Node {:?} connection error to {}: {}", raft_type, url, err);
                }
            }
        }
        time::sleep(Duration::from_secs(3)).await;
    }
    if !left_cluster {
        return Err(Error::Connect(
            "Could not leave the cluster after trying all nodes once".to_string(),
        ));
    }

    // After removal, query metrics again until this node is fully removed.
    // We need to do this on all nodes, because we don't know if any of them leave or join
    // in the meantime.
    for _ in 0..=retries {
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

            let Ok(res) = client.get(&url).bearer_auth(&state.secret_api).send().await else {
                error!(
                    "Unable to reach Node {} via {} to confirm cluster leave via metrics",
                    node.id, url
                );
                continue;
            };

            if res.status().is_success() {
                let bytes = res.bytes().await?;
                let metrics = deserialize_serde::<WireMetrics>(bytes.as_ref())?;
                let is_member = metrics
                    .membership_config
                    .nodes()
                    .any(|(id, _)| *id == this_node);
                if is_member {
                    info!(
                        "This Node ({this_node}) is still a Raft member after removal - waiting ..."
                    );
                    time::sleep(Duration::from_secs(1)).await;
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
    raft: &openraft::Raft<TypeConfigSqlite, StateMachineSqlite>,
) -> Result<bool, Error> {
    let has_any_nodes = || {
        raft.metrics()
            .borrow_watched()
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
            log_no_membership_error();
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

#[cfg(feature = "cache")]
async fn is_initialized_timeout_cache(
    node_id: u64,
    raft: &openraft::Raft<TypeConfigCache, Arc<StateMachineMemory>>,
) -> Result<bool, Error> {
    let has_any_nodes = || {
        raft.metrics()
            .borrow_watched()
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
            log_no_membership_error();
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
                .store(false, Ordering::Relaxed);
            state
                .raft_db
                .is_startup_finished
                .store(true, Ordering::Relaxed);
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            info!("Setting Cache Raft to running");
            state
                .raft_cache
                .is_raft_stopped
                .store(false, Ordering::Relaxed);
            state
                .raft_cache
                .is_startup_finished
                .store(true, Ordering::Relaxed);
        }
        RaftType::Unknown => unreachable!(),
    }
}

fn log_no_membership_error() {
    error!(
        r"

    Raft is initialized but the membership config is empty.
    This can usually only happen during the initialization of a fresh cluster, if your
    application crashed or is being force-killed in the middle of a cluster join.

    If this is a single instance, this Node cann probably not recover from this state on its own.
    You can fix this by starting with: `HQL_DANGER_RAFT_STATE_RESET=true`

    If this happens on a cluster Node and the other members are healthy, it may be able to recover
    when the remote leader tries to re-initialize it. If this failes, the easiest and safest fix
    is to delete the volume and let the Node re-join and sync the cluster data cleanly to not end
    up in an inconsistent state.
"
    );
}

#[cfg(all(test, feature = "sqlite"))]
#[path = "init_skip_tests.rs"]
mod skip_tests;

#[cfg(test)]
mod decide_tests {
    use super::{InitRound, PeerAnswer, decide_init_round};
    use PeerAnswer::{Inconclusive, Initialized, NotInitialized, Unreachable};

    #[test]
    fn any_initialized_remote_means_join() {
        assert_eq!(
            decide_init_round(&[Unreachable, Initialized]),
            InitRound::Skip
        );
        assert_eq!(
            decide_init_round(&[NotInitialized, Initialized]),
            InitRound::Skip
        );
    }

    #[test]
    fn connection_errors_never_count_towards_init() {
        // N=3: the case that formed a second cluster before
        assert_eq!(
            decide_init_round(&[Unreachable, NotInitialized]),
            InitRound::Wait
        );
        assert_eq!(
            decide_init_round(&[Inconclusive, NotInitialized]),
            InitRound::Wait
        );
        // N=5: two pristine answers out of four remotes are not a quorum of them
        assert_eq!(
            decide_init_round(&[NotInitialized, NotInitialized, Unreachable, Unreachable]),
            InitRound::Wait
        );
    }

    #[test]
    fn quorum_of_the_other_nodes_initializes() {
        assert_eq!(decide_init_round(&[NotInitialized]), InitRound::Init);
        assert_eq!(
            decide_init_round(&[NotInitialized, NotInitialized]),
            InitRound::Init
        );
        assert_eq!(
            decide_init_round(&[NotInitialized, NotInitialized, NotInitialized, Unreachable]),
            InitRound::Init
        );
    }

    #[test]
    fn nobody_reachable_is_reported_as_such() {
        assert_eq!(
            decide_init_round(&[Unreachable, Unreachable]),
            InitRound::AllUnreachable
        );
        assert_eq!(
            decide_init_round(&[Inconclusive, Unreachable]),
            InitRound::Wait
        );
    }
}
