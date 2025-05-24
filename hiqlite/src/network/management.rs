use crate::app_state::RaftType;
use crate::network::{fmt_ok, get_payload, validate_secret, AppStateExt, Error};
use crate::NodeId;
use crate::{helpers, Node};
use axum::body;
use axum::body::Body;
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::response::Response;
use openraft::error::{CheckIsLeaderError, ForwardToLeader, RaftError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct LearnerReq {
    pub node_id: u64,
    pub addr_api: String,
    pub addr_raft: String,
}

#[derive(Debug, Serialize, Deserialize)]
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

    if !helpers::is_raft_initialized(&state, &raft_type).await? {
        return Err(Error::Error("Raft is not initialized".into()));
    }
    are_we_leader(&state, &raft_type).await?;

    let LearnerReq {
        node_id,
        addr_api,
        addr_raft,
    } = get_payload(&headers, body)?;
    let node = Node {
        id: node_id,
        addr_raft,
        addr_api,
    };
    info!(
        "\n\n{:?} requests to be added as {:?} Learner",
        node, raft_type
    );

    // Check if the node is maybe already a member.
    // If this is the case, it might do the request because it tries to recover from volume loss.
    // -> remove the membership and re-add it as a new learner, so it can catch up again.
    let _lock = state.raft_lock.lock().await;

    let mut metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    let is_member_already = metrics
        .membership_config
        .nodes()
        .any(|(id, _)| *id == node.id);

    if is_member_already {
        if raft_type == RaftType::Cache {
            warn!(
                "\n\nNode{:?} is already a cache member - removing it first\n",
                node
            );
            let mut is_voter = metrics
                .membership_config
                .voter_ids()
                .any(|id| id != node.id);
            let members_remove = metrics
                .membership_config
                .nodes()
                .filter_map(|(id, _)| if *id != node.id { Some(*id) } else { None })
                .collect::<BTreeSet<u64>>();

            if is_voter {
                if let Err(err) =
                    helpers::change_membership(&state, &raft_type, members_remove.clone(), true)
                        .await
                {
                    error!(
                        "\n\nError setting existing voter node as cache learner: {:?}\n",
                        err
                    );
                    return Err(err);
                }

                while is_voter {
                    time::sleep(Duration::from_millis(250)).await;
                    metrics = helpers::get_raft_metrics(&state, &raft_type).await;
                    is_voter = metrics
                        .membership_config
                        .voter_ids()
                        .any(|id| id != node.id);
                }
            }

            if let Err(err) =
                helpers::change_membership(&state, &raft_type, members_remove, false).await
            {
                error!(
                    "\n\nError removing existing node from cache members: {:?}\n",
                    err
                );
                return Err(err);
            }
            time::sleep(Duration::from_millis(500)).await;

            metrics = helpers::get_raft_metrics(&state, &raft_type).await;
            let mut is_member = metrics
                .membership_config
                .membership()
                .get_node(&node.id)
                .is_some();
            while is_member {
                time::sleep(Duration::from_millis(100)).await;
                metrics = helpers::get_raft_metrics(&state, &raft_type).await;
                is_member = metrics
                    .membership_config
                    .membership()
                    .get_node(&node.id)
                    .is_some();
            }
        } else {
            info!("\n\nNode is a {:?} Learner already\n", raft_type);
            return fmt_ok(headers, ());
        }
    }

    let res = helpers::add_new_learner(&state, &raft_type, node).await;
    match res {
        Ok(_) => {
            info!("\n\nAdded node as {:?} learner\n", raft_type);
            fmt_ok(headers, ())
        }
        Err(err) => {
            error!(
                "\n\nError adding node as {:?} learner: {:?}\n",
                raft_type, err
            );
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

    if !helpers::is_raft_initialized(&state, &raft_type).await? {
        return Err(Error::Error("Raft is not initialized".into()));
    }
    are_we_leader(&state, &raft_type).await?;

    let payload = get_payload::<LearnerReq>(&headers, body)?;
    info!("{:?} Node membership request: {:?}\n", raft_type, payload);

    let _lock = state.raft_lock.lock().await;

    let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    let members = metrics.membership_config;
    info!("{:?} Members before add: {:?}", raft_type, members);

    let mut nodes_set = BTreeSet::new();
    for (id, _node) in members.nodes() {
        nodes_set.insert(*id);
    }
    nodes_set.insert(payload.node_id);

    let res = helpers::change_membership(&state, &raft_type, nodes_set, true).await;
    match res {
        Ok(_) => {
            info!("Added node as {:?} member", raft_type);
            fmt_ok(headers, ())
        }
        Err(err) => {
            error!("Error adding node as member: {:?}", err);
            Err(err)
        }
    }
}

#[tracing::instrument(skip_all)]
pub async fn leave_cluster(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    if !helpers::is_raft_initialized(&state, &raft_type).await? {
        return Err(Error::Error("Raft is not initialized".into()));
    }
    are_we_leader(&state, &raft_type).await?;

    let payload = get_payload::<ClusterLeaveReq>(&headers, body)?;
    info!("{:?} Node {:?}\n", raft_type, payload);

    let _lock = state.raft_lock.lock().await;

    let mut metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    let is_member_already = metrics
        .membership_config
        .nodes()
        .any(|(id, _)| *id == payload.node_id);

    if is_member_already {
        warn!("\n\nNode{:?} is a member - removing it\n", payload.node_id);
        let mut is_voter = metrics
            .membership_config
            .voter_ids()
            .any(|id| id != payload.node_id);
        let members_remove = metrics
            .membership_config
            .nodes()
            .filter_map(|(id, _)| {
                if *id != payload.node_id {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<u64>>();

        if is_voter {
            if let Err(err) =
                helpers::change_membership(&state, &raft_type, members_remove.clone(), true).await
            {
                error!("\n\nError setting existing voters: {:?}\n", err);
                return Err(err);
            }

            while is_voter {
                info!(
                    "\n\nWaiting until Node {} is not a voter anymore\n",
                    payload.node_id
                );
                time::sleep(Duration::from_millis(250)).await;
                metrics = helpers::get_raft_metrics(&state, &raft_type).await;
                is_voter = metrics
                    .membership_config
                    .voter_ids()
                    .any(|id| id != payload.node_id);
            }
        }

        if !payload.stay_as_learner {
            if let Err(err) =
                helpers::change_membership(&state, &raft_type, members_remove, false).await
            {
                error!("\n\nError updating cluster members: {:?}\n", err);
                return Err(err);
            }
            time::sleep(Duration::from_millis(250)).await;

            metrics = helpers::get_raft_metrics(&state, &raft_type).await;
            let mut is_member = metrics
                .membership_config
                .membership()
                .get_node(&payload.node_id)
                .is_some();
            while is_member {
                info!(
                    "\n\nWaiting until Node {} is not a member anymore\n",
                    payload.node_id
                );
                time::sleep(Duration::from_millis(100)).await;
                metrics = helpers::get_raft_metrics(&state, &raft_type).await;
                is_member = metrics
                    .membership_config
                    .membership()
                    .get_node(&payload.node_id)
                    .is_some();
            }
        }
    }

    info!(
        "\n\nNode {} has left the cluster:\n\n{:?}\n",
        payload.node_id,
        metrics.membership_config.membership()
    );

    Ok(Response::new(Body::empty()))
}

async fn are_we_leader(state: &AppStateExt, raft_type: &RaftType) -> Result<(), Error> {
    if let Some(leader_id) = helpers::get_raft_leader(&state, &raft_type).await {
        if leader_id == state.id {
            Ok(())
        } else {
            let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
            let leader = metrics
                .membership_config
                .membership()
                .get_node(&leader_id)
                .expect("Leader ID to always exist in membership config");

            let err = RaftError::APIError(CheckIsLeaderError::ForwardToLeader(ForwardToLeader {
                leader_id: Some(leader_id),
                leader_node: Some(leader.clone()),
            }));
            Err(Error::CheckIsLeaderError(Box::new(err)))
        }
    } else {
        Err(Error::LeaderChange("Leader election in progress".into()))
    }
}

pub(crate) async fn get_membership(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    if !helpers::is_raft_initialized(&state, &raft_type).await? {
        return Err(Error::Config("Raft node has not been initialized".into()));
    }

    let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    let mut members = metrics.membership_config;

    // it is possible to end up in a race condition on rolling releases
    if members.nodes().count() == 0 {
        time::sleep(Duration::from_millis(1000)).await;
        let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
        members = metrics.membership_config;
        debug!("Membership after 1000ms timeout: {:?}", members);

        // if we still have no members, return an error
        return Err(Error::Config(
            "Node is initialized but has no members".into(),
        ));
    }

    fmt_ok(headers, members.membership())
}

/// Changes specified learners to members, or remove members.
pub(crate) async fn post_membership(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let payload = get_payload::<BTreeSet<NodeId>>(&headers, body)?;
    helpers::change_membership(&state, &raft_type, payload, false).await?;

    // retain false removes current cluster members if they do not appear in the new list
    fmt_ok(headers, ())
}

/// Get the latest metrics of the cluster
pub(crate) async fn metrics(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    fmt_ok(headers, &metrics)
}
