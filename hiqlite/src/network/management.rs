use crate::NodeId;
use crate::app_state::{AppState, RaftType};
use crate::network::{
    AppStateExt, Error, WireMembership, fmt_ok, fmt_ok_serde, get_payload, validate_secret,
};
use crate::{Node, helpers};
use axum::body;
use axum::body::Body;
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::response::Response;
use bincode_next::{Decode, Encode};
use openraft::RaftTypeConfig;
use openraft::ServerState;
use openraft::errors::{ForwardReason, ForwardToLeader, LinearizableReadError, RaftError};
use openraft::type_config::async_runtime::watch::WatchReceiver;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info, warn};

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct LearnerReq {
    pub node_id: u64,
    pub addr_api: String,
    pub addr_raft: String,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct ClusterLeaveReq {
    pub node_id: u64,
    pub stay_as_learner: bool,
}

#[tracing::instrument(skip_all)]
pub(crate) async fn add_learner(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    if helpers::is_raft_stopped(&state, &raft_type)
        || !helpers::is_raft_initialized(&state, &raft_type).await?
    {
        return Err(Error::Error("Raft is not initialized".into()));
    }
    are_we_leader(&state, &raft_type).await?;

    let LearnerReq {
        node_id,
        addr_api,
        addr_raft,
    } = get_payload(&headers, &body)?;
    let node = Node {
        id: node_id,
        addr_raft,
        addr_api,
    };
    info!("{:?} requests to be added as {:?} Learner", node, raft_type);
    let lock = state.raft_lock.lock().await;
    let nid = node.id;
    let res = helpers::add_new_learner(&state, &raft_type, node).await;
    match res {
        Ok(()) => {
            if let Err(err) = wait_for_membership_commit(
                &state,
                &raft_type,
                &format!("node {nid} to become a committed learner"),
                |mc| mc.membership().get_node(&nid).is_some(),
                LeaderLoss::Fails,
            )
            .await
            {
                error!(
                    "Error waiting for node {nid} to become a committed learner: {:?}",
                    err
                );
                return Err(err);
            }

            // give it a second to sync before dropping the lock
            time::sleep(Duration::from_millis(1000)).await;
            drop(lock);
            info!("Added node {nid} as commited {:?} learner", raft_type);
            fmt_ok(&headers, ())
        }
        Err(err) => {
            error!("Error adding node as {:?} learner: {:?}", raft_type, err);
            Err(err)
        }
    }
}

/// Changes specified learners to members, or remove members.
#[tracing::instrument(skip_all)]
pub(crate) async fn become_member(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    if helpers::is_raft_stopped(&state, &raft_type)
        || !helpers::is_raft_initialized(&state, &raft_type).await?
    {
        return Err(Error::Error("Raft is not initialized".into()));
    }
    are_we_leader(&state, &raft_type).await?;

    let lock = state.raft_lock.lock().await;
    let payload = get_payload::<LearnerReq>(&headers, &body)?;
    info!("{:?} Node membership request: {:?}", raft_type, payload);

    let view = helpers::get_raft_view(&state, &raft_type)?;
    debug!("{:?} Members before add: {:?}", raft_type, view);

    let is_voter = view.membership.voter_ids().any(|id| id == payload.node_id);
    if is_voter {
        info!(
            "Node {} is a voter already - nothing left to do",
            payload.node_id
        );
        return fmt_ok(&headers, ());
    }

    let mut nodes_set = view.membership.voter_ids().collect::<BTreeSet<u64>>();
    nodes_set.insert(payload.node_id);

    match helpers::change_membership(&state, &raft_type, nodes_set, true).await {
        Ok(()) => {
            if let Err(err) = wait_for_membership_commit(
                &state,
                &raft_type,
                &format!("node {} to become a committed voter", payload.node_id),
                |mc| mc.voter_ids().any(|id| id == payload.node_id),
                LeaderLoss::Fails,
            )
            .await
            {
                error!(
                    "Error waiting for node {} to become a committed voter: {:?}",
                    payload.node_id, err
                );
                return Err(err);
            }

            // give it a second to sync before dropping the lock
            time::sleep(Duration::from_millis(1000)).await;
            drop(lock);
            info!("Added node {} as {:?} member", payload.node_id, raft_type);
            fmt_ok(&headers, ())
        }
        Err(err) => {
            error!("Error adding node as member: {:?}", err);
            Err(err)
        }
    }
}

async fn are_we_leader(state: &Arc<AppState>, raft_type: &RaftType) -> Result<(), Error> {
    if let Some(leader_id) = helpers::get_raft_leader(state, raft_type).await? {
        if leader_id == state.id {
            Ok(())
        } else {
            let view = helpers::get_raft_view(state, raft_type)?;
            let Some(leader) = view.membership.membership().get_node(&leader_id) else {
                return Err(Error::Error(
                    format!("Leader {leader_id} not found in membership config").into(),
                ));
            };

            #[cfg(feature = "sqlite")]
            if matches!(raft_type, RaftType::Sqlite) {
                return Err(Error::CheckIsLeaderErrorDb(Box::new(leader_redirect::<
                    crate::store::state_machine::sqlite::TypeConfigSqlite,
                >(
                    leader_id, leader
                ))));
            }
            #[cfg(feature = "cache")]
            if matches!(raft_type, RaftType::Cache) {
                return Err(Error::CheckIsLeaderErrorCache(Box::new(leader_redirect::<
                    crate::store::state_machine::memory::TypeConfigCache,
                >(
                    leader_id, leader,
                ))));
            }

            Err(Error::Error(
                format!("Leader is {leader_id}, not us, and the Raft type is unknown").into(),
            ))
        }
    } else {
        Err(Error::LeaderChange("Leader election in progress".into()))
    }
}

fn leader_redirect<C>(
    leader_id: C::NodeId,
    leader: &C::Node,
) -> RaftError<C, LinearizableReadError<C>>
where
    C: RaftTypeConfig,
{
    RaftError::APIError(LinearizableReadError::ForwardToLeader(ForwardToLeader {
        leader_id: Some(leader_id),
        leader_node: Some(leader.clone()),
        reason: ForwardReason::default(),
    }))
}

pub(crate) async fn get_membership(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    // A pristine node 1 decides whether to form a new cluster from this answer, so "not
    // initialized" must only be sent when it is true: an initialized node whose Raft has not
    // been started yet answers differently.
    if !helpers::is_raft_initialized(&state, &raft_type).await? {
        return Err(Error::Config(crate::init::MSG_NOT_INITIALIZED.into()));
    }
    if helpers::is_raft_stopped(&state, &raft_type) {
        return Err(Error::Config(
            "Raft node is initialized but not running".into(),
        ));
    }

    let mut members = helpers::get_raft_view(&state, &raft_type)?.membership;

    // it is possible to end up in a race condition on rolling releases
    if members.nodes().count() == 0 {
        time::sleep(Duration::from_millis(1000)).await;
        members = helpers::get_raft_view(&state, &raft_type)?.membership;
        debug!("Membership after 1000ms timeout: {:?}", members);

        // if we still have no members, return an error
        return Err(Error::Config(
            "Node is initialized but has no members".into(),
        ));
    }

    fmt_ok_serde(&headers, members.membership())
}

/// Changes specified learners to members, or remove members.
pub(crate) async fn post_membership(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    if helpers::is_raft_stopped(&state, &raft_type)
        || !helpers::is_raft_initialized(&state, &raft_type).await?
    {
        return Err(Error::Config("Raft node has not been initialized".into()));
    }

    are_we_leader(&state, &raft_type).await?;

    let payload = get_payload::<BTreeSet<NodeId>>(&headers, &body)?;

    // Take the shared raft_lock like the other membership endpoints so this full-set change
    // cannot race with join/leave flows that poll for their commit.
    let _lock = state.raft_lock.lock().await;
    let membership = helpers::get_raft_view(&state, &raft_type)?.membership;
    let old_voters: BTreeSet<u64> = membership.voter_ids().collect();

    // A full-set change that drops this node (the leader) goes through openraft's two-step voter
    // removal. With `retain=false` the joint config already removes us from the node map, but we
    // only step down on a later heartbeat tick: if the second (uniform) config is processed before
    // that tick, openraft's `append_membership` debug assert panics ("Only leader is allowed to
    // call update_effective_membership()"). Keeping ourselves as a learner through the protocol
    // keeps `is_leader()` true for both steps, so the panic is unreachable. Dropping our own node
    // entry afterwards (RemoveNodes) opens the same zombie-leader window until that tick fires,
    // so we also wait for the demotion before releasing the lock (see wait_for_demotion).
    let self_removal = !payload.contains(&state.id);
    let dropped_voters: BTreeSet<u64> = old_voters.difference(&payload).copied().collect();
    helpers::change_membership(&state, &raft_type, payload, self_removal).await?;

    if self_removal {
        // `retain=true` kept every dropped voter in the node map as a learner. Drop those entries
        // now: RemoveNodes only edits the node map (single-step), so it cannot hit the race above.
        // If we were demoted in the meantime, the removal is rejected with ForwardToLeader and the
        // nodes simply remain committed learners - emit loudly instead of failing the request.
        for id in &dropped_voters {
            if let Err(err) = helpers::remove_learner(&state, &raft_type, *id).await {
                warn!(
                    "Node {id} ({:?}) was dropped from the voter set but its Learner entry could \
                     not be removed (we may have been demoted in the meantime): {:?}",
                    raft_type, err
                );
            }
        }

        if dropped_voters.contains(&state.id) {
            // We just removed ourselves from the node map: wait for openraft to actually step us
            // down before releasing the lock, so no queued membership write can hit the assert.
            wait_for_demotion(&state, &raft_type).await?;
        }
    }

    // retain false removes current cluster members if they do not appear in the new list
    fmt_ok(&headers, ())
}

#[tracing::instrument(skip_all)]
pub async fn leave_cluster(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    if helpers::is_raft_stopped(&state, &raft_type)
        || !helpers::is_raft_initialized(&state, &raft_type).await?
    {
        return Err(Error::Config("Raft node has not been initialized".into()));
    }
    are_we_leader(&state, &raft_type).await?;

    let payload = get_payload::<ClusterLeaveReq>(&headers, &body)?;
    leave_cluster_exec(&state.0, &raft_type, payload).await?;

    Ok(Response::new(Body::empty()))
}

pub async fn leave_cluster_exec(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    payload: ClusterLeaveReq,
) -> Result<(), Error> {
    info!("{:?} Node {:?}", raft_type, payload);

    let lock = state.raft_lock.lock().await;

    let membership = helpers::get_raft_view(state, raft_type)?.membership;
    let is_member = membership.nodes().any(|(id, _)| *id == payload.node_id);

    if is_member {
        warn!(
            "Node {} ({:?}) is a cluster member - removing it",
            payload.node_id, raft_type
        );
        let is_voter = membership.voter_ids().any(|id| id == payload.node_id);

        if is_voter {
            warn!("Node {} ({:?}) is a Voter", payload.node_id, raft_type);

            // We are the leader here (the HTTP path checks it via `are_we_leader`, the shutdown
            // path only calls this when we are the current leader). openraft commits voter removal
            // in two steps: joint config first, then the uniform one. When the leader removes
            // itself with `retain=false`, the joint config already drops it from the node map, but
            // the leader only steps down on a later heartbeat tick: if the second step is processed
            // before that tick, openraft's `append_membership` debug assert panics ("Only leader is
            // allowed to call update_effective_membership()"). Keeping ourselves as a learner
            // through the protocol keeps `is_leader()` true for both steps, so the panic is
            // unreachable.
            let self_removal = payload.node_id == state.id;
            if let Err(err) = helpers::remove_voter(
                state,
                raft_type,
                payload.node_id,
                self_removal || payload.stay_as_learner,
            )
            .await
            {
                error!(
                    "Error removing Node {} ({:?}) from Voters: {:?}",
                    payload.node_id, raft_type, err
                );
                return Err(err);
            }
            if let Err(err) = wait_for_membership_commit(
                state,
                raft_type,
                &format!(
                    "node {} ({:?}) to no longer be a Voter",
                    payload.node_id, raft_type
                ),
                |mc| !mc.voter_ids().any(|id| id == payload.node_id),
                if self_removal {
                    LeaderLoss::IsThePoint
                } else {
                    LeaderLoss::Fails
                },
            )
            .await
            {
                error!(
                    "Error waiting for Node {} ({:?}) to no longer be a Voter: {:?}",
                    payload.node_id, raft_type, err
                );
                return Err(err);
            }

            // The self-removal above kept us in the node map as a learner to make the two-step
            // protocol safe. If we should not stay a learner, drop that entry now: RemoveNodes only
            // edits the node map (single-step), so it cannot hit the race above. If we were demoted
            // in the meantime, the removal is rejected with ForwardToLeader and we simply remain a
            // committed learner - emit loudly instead of failing the leave.
            if self_removal && !payload.stay_as_learner {
                if let Err(err) = helpers::remove_learner(state, raft_type, payload.node_id).await {
                    warn!(
                        "Node {} ({:?}) removed itself as Voter but could not drop its Learner \
                         entry (it may have been demoted in the meantime): {:?}",
                        payload.node_id, raft_type, err
                    );
                }

                // We are out of the membership entirely now. openraft only steps us down on a
                // heartbeat tick, so wait that out before releasing the lock: until then we are a
                // zombie leader (still in Leader state, but no longer in the node map), and any
                // queued membership write would hit the assert above.
                wait_for_demotion(state, raft_type).await?;
            }
        } else if !payload.stay_as_learner {
            warn!(
                "Node {} ({:?}) is a Learner and should not stay one",
                payload.node_id, raft_type
            );
            if let Err(err) = helpers::remove_learner(state, raft_type, payload.node_id).await {
                error!(
                    "Error removing Node {} ({:?}) from Learners: {:?}",
                    payload.node_id, raft_type, err
                );
                return Err(err);
            }
            if let Err(err) = wait_for_membership_commit(
                state,
                raft_type,
                &format!(
                    "node {} ({:?}) to no longer be a Learner",
                    payload.node_id, raft_type
                ),
                |mc| !mc.nodes().any(|(id, _)| *id == payload.node_id),
                LeaderLoss::Fails,
            )
            .await
            {
                error!(
                    "Error waiting for Node {} ({:?}) to no longer be a Learner: {:?}",
                    payload.node_id, raft_type, err
                );
                return Err(err);
            }
        }
    }

    drop(lock);
    let membership = helpers::get_raft_view(state, raft_type)?.membership;
    info!(
        "Node {} ({:?}) has left the cluster: {:?}",
        payload.node_id,
        raft_type,
        membership.membership()
    );

    Ok(())
}

/// Get the latest metrics of the cluster
pub(crate) async fn metrics(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    // Gate like the other management endpoints - for `RaftType::Unknown` (neither feature
    // enabled), this is the only guard before the per-group metrics below would panic.
    if helpers::is_raft_stopped(&state, &raft_type)
        || !helpers::is_raft_initialized(&state, &raft_type).await?
    {
        return Err(Error::Error("Raft is not initialized".into()));
    }

    // The metrics struct is generic over the type config, so each group serializes its own.
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            let metrics = state.raft_db.raft.metrics().borrow_watched().clone();
            fmt_ok_serde(&headers, &metrics)
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            let metrics = state.raft_cache.raft.metrics().borrow_watched().clone();
            fmt_ok_serde(&headers, &metrics)
        }
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
    }
}

/// The election state of one Raft group on this node.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct NodeElection {
    pub node_id: NodeId,
    /// openraft's server state for this group, formatted: `Leader`, `Follower`, `Learner`, ...
    pub state: String,
    /// The group's current leader, if any.
    pub leader: Option<NodeId>,
    pub is_voter: bool,
    /// The effective runtime value: `false` means this node never starts an election on its own
    /// for this group. It still grants votes and it still accepts a leadership transfer.
    pub enable_elect: bool,
    /// The marker of this node's own node line. A runtime change of `enable_elect` is not
    /// persisted, so a restart applies this value again.
    pub election_disable: bool,
}

/// The election state of every Raft group this node runs.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct ElectionState {
    #[cfg(feature = "sqlite")]
    pub sqlite: NodeElection,
    #[cfg(feature = "cache")]
    pub cache: NodeElection,
}

/// The body of `PUT /cluster/election`.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct ElectionReq {
    pub enable_elect: bool,
}

/// Every Raft group this build runs.
pub(crate) fn raft_types() -> Vec<RaftType> {
    let types = [
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite,
        #[cfg(feature = "cache")]
        RaftType::Cache,
    ];

    types.into_iter().collect()
}

fn node_election(state: &Arc<AppState>, raft_type: &RaftType) -> Result<NodeElection, Error> {
    let view = helpers::get_raft_view(state, raft_type)?;

    Ok(NodeElection {
        node_id: state.id,
        state: format!("{:?}", view.state),
        leader: view.leader_id,
        is_voter: view.membership.voter_ids().any(|id| id == state.id),
        enable_elect: helpers::is_elect_enabled(state, raft_type),
        election_disable: state.election_disable.contains(&state.id),
    })
}

pub(crate) fn election_state(state: &Arc<AppState>) -> Result<ElectionState, Error> {
    Ok(ElectionState {
        #[cfg(feature = "sqlite")]
        sqlite: node_election(state, &RaftType::Sqlite)?,
        #[cfg(feature = "cache")]
        cache: node_election(state, &RaftType::Cache)?,
    })
}

#[cfg(any(feature = "sqlite", feature = "cache"))]
impl ElectionState {
    /// The id of the node this state comes from - every group reports the same one.
    #[must_use]
    pub fn node_id(&self) -> NodeId {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.node_id
        }
        #[cfg(not(feature = "sqlite"))]
        {
            self.cache.node_id
        }
    }
}

/// The local counterpart of `PUT /cluster/election` for an embedded client's own node.
pub(crate) async fn local_set_enable_elect(
    state: &Arc<AppState>,
    enable: bool,
) -> Result<ElectionState, Error> {
    for raft_type in raft_types() {
        if helpers::is_raft_stopped(state, &raft_type)
            || !helpers::is_raft_initialized(state, &raft_type).await?
        {
            return Err(Error::Error("Raft is not initialized".into()));
        }
        helpers::set_enable_elect(state, &raft_type, enable);
    }

    election_state(state)
}

/// The local counterpart of `POST /cluster/election` for an embedded client's own node.
///
/// A learner is refused outright: it can never win an election, it can only inflate the term. A node
/// with its own elections disabled is refused by openraft's gate, and the campaign is a Pre-Vote
/// probe, so a refusal never touches the term.
pub(crate) async fn local_elect(state: &Arc<AppState>) -> Result<ElectionState, Error> {
    for raft_type in raft_types() {
        let view = helpers::get_raft_view(state, &raft_type)?;
        if view.state == ServerState::Learner {
            return Err(Error::Error(
                format!(
                    "Node {} is a learner in the {:?} group and can never become a leader",
                    state.id, raft_type
                )
                .into(),
            ));
        }
    }

    for raft_type in raft_types() {
        if helpers::is_raft_stopped(state, &raft_type)
            || !helpers::is_raft_initialized(state, &raft_type).await?
        {
            return Err(Error::Error("Raft is not initialized".into()));
        }

        helpers::trigger_elect(state, &raft_type).await?;
    }

    election_state(state)
}

/// Get the election state of this node: `GET /cluster/election`.
#[tracing::instrument(skip_all)]
pub(crate) async fn get_election(
    state: AppStateExt,
    headers: HeaderMap,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    fmt_ok(&headers, election_state(&state)?)
}

/// Change this node's own right to start elections: `PUT /cluster/election`.
///
/// Applied to every group the node runs, so a node is never internally inconsistent about it.
/// The change is in-memory only - a restart applies the `election_disable` marker of the node's
/// own line again. Any node accepts this for itself, it does not need to be the leader.
#[tracing::instrument(skip_all)]
pub(crate) async fn put_election(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let req = get_payload::<ElectionReq>(&headers, &body)?;

    fmt_ok(
        &headers,
        local_set_enable_elect(&state, req.enable_elect).await?,
    )
}

/// Make this node campaign for the leadership of all its groups: `POST /cluster/election`.
///
/// openraft runs its Pre-Vote round first, so the term is untouched when this node cannot win - a
/// node whose own leader lease is still valid is refused locally, and so is a node with its own
/// elections disabled. A learner is refused outright: it can never win, it can only inflate the
/// term. There is no way to bypass the Pre-Vote: a leader change that must happen is a leadership
/// transfer on `POST /cluster/transfer_leader`.
#[tracing::instrument(skip_all)]
pub(crate) async fn post_election(
    state: AppStateExt,
    headers: HeaderMap,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    fmt_ok(&headers, local_elect(&state).await?)
}

/// The membership of one Raft group, as this node sees it.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct NodeMembers {
    pub node_id: NodeId,
    /// openraft's server state for this group, formatted: `Leader`, `Follower`, `Learner`, ...
    pub state: String,
    /// The group's current leader, if any.
    pub leader: Option<NodeId>,
    /// Every voter of the group.
    pub voters: Vec<NodeId>,
    /// Every learner of the group.
    pub learners: Vec<NodeId>,
    pub is_voter: bool,
    pub is_learner: bool,
}

/// The membership of every Raft group this node runs.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct MembersState {
    #[cfg(feature = "sqlite")]
    pub sqlite: NodeMembers,
    #[cfg(feature = "cache")]
    pub cache: NodeMembers,
}

/// The body of `POST /cluster/promote` and `POST /cluster/transfer_leader`.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct NodeReq {
    pub node_id: NodeId,
}

#[cfg(any(feature = "sqlite", feature = "cache"))]
impl MembersState {
    /// The id of the node this state comes from - every group reports the same one.
    #[must_use]
    pub fn node_id(&self) -> NodeId {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.node_id
        }
        #[cfg(not(feature = "sqlite"))]
        {
            self.cache.node_id
        }
    }
}

fn node_members(state: &Arc<AppState>, raft_type: &RaftType) -> Result<NodeMembers, Error> {
    let view = helpers::get_raft_view(state, raft_type)?;

    let voters = view.membership.voter_ids().collect::<Vec<_>>();
    let learners = view
        .membership
        .nodes()
        .filter(|(node_id, _)| !voters.contains(node_id))
        .map(|(node_id, _)| *node_id)
        .collect::<Vec<_>>();

    Ok(NodeMembers {
        node_id: state.id,
        state: format!("{:?}", view.state),
        leader: view.leader_id,
        voters,
        learners,
        is_voter: view.membership.voter_ids().any(|id| id == state.id),
        is_learner: view.membership.nodes().any(|(id, _)| *id == state.id)
            && !view.membership.voter_ids().any(|id| id == state.id),
    })
}

pub(crate) fn members_state(state: &Arc<AppState>) -> Result<MembersState, Error> {
    Ok(MembersState {
        #[cfg(feature = "sqlite")]
        sqlite: node_members(state, &RaftType::Sqlite)?,
        #[cfg(feature = "cache")]
        cache: node_members(state, &RaftType::Cache)?,
    })
}

/// Every group must be up and leading by this node before any membership change of ours runs.
/// A change that only some groups committed would leave a node voter in one and learner in the
/// other, which is exactly the half state the master-slave playbooks must never end up in.
async fn require_we_lead_all_groups(state: &Arc<AppState>) -> Result<(), Error> {
    for raft_type in raft_types() {
        if helpers::is_raft_stopped(state, &raft_type)
            || !helpers::is_raft_initialized(state, &raft_type).await?
        {
            return Err(Error::Error("Raft is not initialized".into()));
        }
        are_we_leader(state, &raft_type).await?;
    }

    Ok(())
}

/// The local counterpart of `POST /cluster/promote`: make a learner a voter in every group.
pub(crate) async fn local_promote(
    state: &Arc<AppState>,
    node_id: NodeId,
) -> Result<MembersState, Error> {
    require_we_lead_all_groups(state).await?;

    // All groups are checked before the first change runs, so one group that does not know the
    // node cannot leave the other groups already changed.
    let mut targets = Vec::new();
    for raft_type in raft_types() {
        let view = helpers::get_raft_view(state, &raft_type)?;
        let is_voter = view.membership.voter_ids().any(|id| id == node_id);
        let is_member = view.membership.nodes().any(|(id, _)| *id == node_id);
        if !is_voter && !is_member {
            return Err(Error::Error(
                format!(
                    "Node {node_id} is not a member of the {raft_type:?} group - add it as a \
                     learner first"
                )
                .into(),
            ));
        }
        targets.push((raft_type, is_voter));
    }

    let lock = state.raft_lock.lock().await;
    for (raft_type, is_voter) in targets {
        if is_voter {
            info!("Node {node_id} is a voter in the {raft_type:?} group already");
            continue;
        }

        let membership = helpers::get_raft_view(state, &raft_type)?.membership;
        let mut voters = membership.voter_ids().collect::<BTreeSet<NodeId>>();
        voters.insert(node_id);
        helpers::change_membership(state, &raft_type, voters, true).await?;

        wait_for_membership_commit(
            state,
            &raft_type,
            &format!("node {node_id} to become a committed voter of the {raft_type:?} group"),
            |mc| mc.voter_ids().any(|id| id == node_id),
            LeaderLoss::Fails,
        )
        .await?;
    }

    // Give the new voter a second to replicate its own voter state logs, like `become_member` does.
    time::sleep(Duration::from_millis(1000)).await;
    drop(lock);

    members_state(state)
}

/// The local counterpart of `DELETE /cluster/membership`: take a voter out of the voter set of
/// every group, optionally keeping it as a learner.
///
/// Demoting the leader with `stay_as_learner = false` is the switchover: openraft's step-down
/// watcher hands the leadership to the most up-to-date remaining voter. A leader that would
/// demote itself to a learner is refused, because it keeps leading in that shape and the
/// membership change cannot complete.
pub(crate) async fn local_demote(
    state: &Arc<AppState>,
    node_id: NodeId,
    stay_as_learner: bool,
) -> Result<MembersState, Error> {
    require_we_lead_all_groups(state).await?;

    let self_removal = node_id == state.id;

    // A leader that removes itself from the Voters and stays in the node map as a Learner keeps
    // leading: openraft's step-down watcher steps a Leader down only when the committed
    // membership no longer contains it at all, and a Leader that is not in the joint config's
    // quota cannot finalize the two-step change. The cluster would sit on the joint config
    // forever. Moving the leadership first and demoting from the new leader is the way to do it.
    if self_removal && stay_as_learner {
        return Err(Error::Error(
            format!(
                "Node {} is the leader: demoting itself to a Learner is refused, because it would \
                 keep leading and the membership change could not complete. Move the leadership \
                 to another voter first (`POST /cluster/transfer_leader`) and then demote node \
                 {node_id} from that node.",
                state.id
            )
            .into(),
        ));
    }

    let lock = state.raft_lock.lock().await;

    for raft_type in raft_types() {
        let view = helpers::get_raft_view(state, &raft_type)?;
        let is_voter = view.membership.voter_ids().any(|id| id == node_id);
        let is_member = view.membership.nodes().any(|(id, _)| *id == node_id);

        if is_voter {
            // The two-step voter removal must keep a leader that removes itself in the node map,
            // for the reason spelled out in `leave_cluster_exec`.
            let retain = self_removal || stay_as_learner;
            helpers::remove_voter(state, &raft_type, node_id, retain).await?;
            wait_for_membership_commit(
                state,
                &raft_type,
                &format!("node {node_id} to no longer be a Voter of the {raft_type:?} group"),
                |mc| !mc.voter_ids().any(|id| id == node_id),
                if self_removal {
                    LeaderLoss::IsThePoint
                } else {
                    LeaderLoss::Fails
                },
            )
            .await?;

            if !stay_as_learner {
                // RemoveNodes only edits the node map, so it cannot hit the race above. If we were
                // demoted in the meantime it is rejected, and the node stays a committed learner.
                if let Err(err) = helpers::remove_learner(state, &raft_type, node_id).await {
                    warn!(
                        "Node {node_id} was dropped from the Voters of the {raft_type:?} group, \
                         but its Learner entry could not be removed (we may have been demoted in \
                         the meantime): {err:?}",
                    );
                }
            }

            if self_removal && !stay_as_learner {
                wait_for_demotion(state, &raft_type).await?;
            }
        } else if is_member && !stay_as_learner {
            helpers::remove_learner(state, &raft_type, node_id).await?;
            wait_for_membership_commit(
                state,
                &raft_type,
                &format!("node {node_id} to no longer be a Learner of the {raft_type:?} group"),
                |mc| !mc.nodes().any(|(id, _)| *id == node_id),
                LeaderLoss::Fails,
            )
            .await?;
        } else {
            info!(
                "Node {node_id} is not a voter of the {raft_type:?} group - nothing to demote \
                 there"
            );
        }
    }

    drop(lock);
    members_state(state)
}

/// The local counterpart of `POST /cluster/transfer_leader`: hand the leadership of every group to
/// one specific node.
pub(crate) async fn local_transfer_leader(
    state: &Arc<AppState>,
    node_id: NodeId,
) -> Result<MembersState, Error> {
    require_we_lead_all_groups(state).await?;

    for raft_type in raft_types() {
        if node_id == state.id {
            continue;
        }
        let view = helpers::get_raft_view(state, &raft_type)?;
        if !view.membership.voter_ids().any(|id| id == node_id) {
            return Err(Error::Error(
                format!(
                    "Node {node_id} is not a voter of the {raft_type:?} group - only a voter can \
                     accept a leadership transfer"
                )
                .into(),
            ));
        }
    }

    // openraft gives up on a transfer after one election timeout, which is the election we would
    // be facing without the transfer at all, so that is our budget too.
    let deadline = helpers::shutdown_transfer_deadline(state);

    for raft_type in raft_types() {
        if node_id == state.id {
            continue;
        }
        let moved = helpers::transfer_leadership_to(state, &raft_type, node_id, deadline).await?;
        if !moved {
            warn!(
                "Leadership of the {raft_type:?} group did not move to node {node_id} - its log is \
                 most likely behind, or it is unreachable"
            );
        }
    }

    members_state(state)
}

/// Get the membership of every group of this node: `GET /cluster/membership`.
#[tracing::instrument(skip_all)]
pub(crate) async fn get_members(state: AppStateExt, headers: HeaderMap) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    fmt_ok(&headers, members_state(&state)?)
}

/// Make a learner a voter in every group: `POST /cluster/promote`.
#[tracing::instrument(skip_all)]
pub(crate) async fn post_promote(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let req = get_payload::<NodeReq>(&headers, &body)?;

    fmt_ok(&headers, local_promote(&state, req.node_id).await?)
}

/// Take a voter out of the voter set of every group: `DELETE /cluster/membership`.
#[tracing::instrument(skip_all)]
pub(crate) async fn delete_members(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let req = get_payload::<ClusterLeaveReq>(&headers, &body)?;

    fmt_ok(
        &headers,
        local_demote(&state, req.node_id, req.stay_as_learner).await?,
    )
}

/// Hand the leadership of every group to one specific node: `POST /cluster/transfer_leader`.
#[tracing::instrument(skip_all)]
pub(crate) async fn post_transfer_leader(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let req = get_payload::<NodeReq>(&headers, &body)?;

    fmt_ok(&headers, local_transfer_leader(&state, req.node_id).await?)
}

/// Maximum time to wait for a membership change to be committed while holding `state.raft_lock`.
/// A leader change mid-operation can lose the uncommitted entry, so this wait is bounded and
/// re-checks leadership on every poll - otherwise the shared lock could be held forever (see
/// `SECURITY_ANALYSIS_PLAN.md`, L4). 40 s exceeds the ~30 s HTTP client timeout, so API callers
/// get their own timeout first; for shutdown paths it acts as a loud backstop.
const MEMBERSHIP_COMMIT_TIMEOUT: Duration = Duration::from_secs(40);

/// Poll metrics until `is_done` reports the membership change as committed.
///
/// Bounded by `MEMBERSHIP_COMMIT_TIMEOUT` and re-checks leadership on every poll, because a
/// leader change mid-operation can lose the uncommitted entry. Callers must hold
/// `state.raft_lock`.
/// How a leader change while we wait for a membership commit is judged.
enum LeaderLoss {
    /// The change is ours to drive: a leader change loses the uncommitted entry, so it fails.
    Fails,
    /// Losing the leadership is the point of the change (we removed ourselves from the Voters):
    /// the joint config is committed by us, the new leader completes the exit from there.
    IsThePoint,
}

async fn wait_for_membership_commit(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    what: &str,
    mut is_done: impl FnMut(&WireMembership) -> bool,
    leader_loss: LeaderLoss,
) -> Result<(), Error> {
    let start = time::Instant::now();

    loop {
        let membership = helpers::get_raft_view(state, raft_type)?.membership;
        if is_done(&membership) {
            return Ok(());
        }

        info!("Waiting for {what}");
        time::sleep(Duration::from_millis(500)).await;

        // A leader change mid-operation can lose the uncommitted entry, so bail out loudly. For a
        // self-removal the demotion is the point of the change, and the new leader finalizes it.
        if let Err(err) = are_we_leader(state, raft_type).await {
            if matches!(leader_loss, LeaderLoss::IsThePoint) {
                warn!(
                    "Node {} ({:?}) is no longer the Leader while waiting for {what}, which is the                      point of a self-removal: {err:?}",
                    state.id, raft_type
                );
                return Ok(());
            }
            return Err(err);
        }

        if start.elapsed() > MEMBERSHIP_COMMIT_TIMEOUT {
            return Err(Error::Error(
                format!("Timeout after {MEMBERSHIP_COMMIT_TIMEOUT:?} waiting for {what}").into(),
            ));
        }
    }
}

/// Maximum time to wait for this node to actually step down after it removed itself from the
/// membership (see `wait_for_demotion`). The demotion happens on the next heartbeat tick once
/// the removal is applied, so this is a generous backstop, not an expected duration.
const DEMOTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Wait until this node has stepped down out of `ServerState::Leader`.
///
/// openraft only steps a leader down on a heartbeat tick, and only once the removal has been
/// applied (`io_applied() >= effective().log_id()`). Until that tick, `server_state` stays
/// `Leader` while the membership no longer contains us: any membership entry appended in that
/// window hits openraft's `append_membership` debug assert ("Only leader is allowed to call
/// `update_effective_membership()`") and panics. Callers must hold `state.raft_lock`, which
/// serializes every membership mutation on this node - so once we return as Follower the window
/// is closed and queued writes are rejected with a graceful `ForwardToLeader` instead.
///
/// On timeout we emit loudly and continue: the leave itself is already committed, and the node
/// steps down on the very next tick regardless.
async fn wait_for_demotion(state: &Arc<AppState>, raft_type: &RaftType) -> Result<(), Error> {
    let start = time::Instant::now();

    loop {
        if helpers::get_raft_view(state, raft_type)?.state != ServerState::Leader {
            return Ok(());
        }

        debug!(
            "Waiting for node {} ({:?}) to step down from Leader",
            state.id, raft_type
        );
        time::sleep(Duration::from_millis(250)).await;

        if start.elapsed() > DEMOTION_TIMEOUT {
            warn!(
                "Node {} ({:?}) removed itself from the membership but is still in Leader state \
                 after {:?}; a membership write in this window may hit openraft's append_membership \
                 assert",
                state.id, raft_type, DEMOTION_TIMEOUT
            );
            return Ok(());
        }
    }
}
