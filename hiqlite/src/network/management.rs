use crate::app_state::{AppState, RaftType};
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
use std::sync::Arc;
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
    } = get_payload(&headers, body)?;
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
        Ok(_) => {
            let mut metrics = helpers::get_raft_metrics(&state, &raft_type).await;
            let mut is_member = metrics
                .membership_config
                .membership()
                .get_node(&nid)
                .is_some();
            while !is_member {
                info!("Waiting for node {nid} to become a committed learner");
                time::sleep(Duration::from_millis(500)).await;
                metrics = helpers::get_raft_metrics(&state, &raft_type).await;
                is_member = metrics
                    .membership_config
                    .membership()
                    .get_node(&nid)
                    .is_some();
            }

            drop(lock);
            info!("Added node {nid} as commited {:?} learner", raft_type);
            fmt_ok(headers, ())
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
    let payload = get_payload::<LearnerReq>(&headers, body)?;
    info!("{:?} Node membership request: {:?}", raft_type, payload);

    let mut metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    info!("{:?} Members before add: {:?}", raft_type, metrics);

    let is_voter = metrics
        .membership_config
        .voter_ids()
        .any(|id| id == payload.node_id);
    if is_voter {
        info!(
            "Node {} is a voter already - nothing left to do",
            payload.node_id
        );
        return fmt_ok(headers, ());
    }

    let mut nodes_set = metrics
        .membership_config
        .voter_ids()
        .collect::<BTreeSet<u64>>();
    nodes_set.insert(payload.node_id);

    match helpers::change_membership(&state, &raft_type, nodes_set, true).await {
        Ok(_) => {
            metrics = helpers::get_raft_metrics(&state, &raft_type).await;
            let mut is_voter = metrics
                .membership_config
                .voter_ids()
                .any(|id| id == payload.node_id);
            while !is_voter {
                info!(
                    "Waiting for node {} to become a committed learner",
                    payload.node_id
                );
                time::sleep(Duration::from_millis(500)).await;
                metrics = helpers::get_raft_metrics(&state, &raft_type).await;
                is_voter = metrics
                    .membership_config
                    .voter_ids()
                    .any(|id| id == payload.node_id);
            }

            drop(lock);
            info!("Added node {} as {:?} member", payload.node_id, raft_type);
            fmt_ok(headers, ())
        }
        Err(err) => {
            error!("Error adding node as member: {:?}", err);
            Err(err)
        }
    }
}

async fn are_we_leader(state: &AppStateExt, raft_type: &RaftType) -> Result<(), Error> {
    if let Some(leader_id) = helpers::get_raft_leader(state, raft_type).await {
        if leader_id == state.id {
            Ok(())
        } else {
            let metrics = helpers::get_raft_metrics(state, raft_type).await;
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

    if helpers::is_raft_stopped(&state, &raft_type)
        || !helpers::is_raft_initialized(&state, &raft_type).await?
    {
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

    if helpers::is_raft_stopped(&state, &raft_type)
        || !helpers::is_raft_initialized(&state, &raft_type).await?
    {
        return Err(Error::Config("Raft node has not been initialized".into()));
    }

    let payload = get_payload::<BTreeSet<NodeId>>(&headers, body)?;
    helpers::change_membership(&state, &raft_type, payload, false).await?;

    // retain false removes current cluster members if they do not appear in the new list
    fmt_ok(headers, ())
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

    let payload = get_payload::<ClusterLeaveReq>(&headers, body)?;
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

    let mut metrics = helpers::get_raft_metrics(state, raft_type).await;
    let mut is_member = metrics
        .membership_config
        .nodes()
        .any(|(id, _)| *id == payload.node_id);

    if is_member {
        warn!("Node {} is a cluster member - removing it", payload.node_id);
        let mut is_voter = metrics
            .membership_config
            .voter_ids()
            .any(|id| id == payload.node_id);

        if is_voter {
            warn!("Node {} is a Voter", payload.node_id);
            if let Err(err) =
                helpers::remove_voter(state, raft_type, payload.node_id, payload.stay_as_learner)
                    .await
            {
                error!(
                    "Error removing Node {} from Voters: {:?}",
                    payload.node_id, err
                );
                return Err(err);
            }
            while is_voter {
                info!(
                    "Waiting until Node {} is not a Voter anymore\nVoter IDs: {:?}\nis_voter: {}",
                    payload.node_id,
                    metrics.membership_config.voter_ids().collect::<Vec<_>>(),
                    is_voter
                );
                time::sleep(Duration::from_millis(500)).await;
                metrics = helpers::get_raft_metrics(state, raft_type).await;
                is_voter = metrics
                    .membership_config
                    .voter_ids()
                    .any(|id| id == payload.node_id);
            }
        } else if !payload.stay_as_learner {
            warn!(
                "Node {} is a Learner and should not stay one",
                payload.node_id
            );
            if let Err(err) = helpers::remove_learner(state, raft_type, payload.node_id).await {
                error!(
                    "Error removing Node {} from Learners: {:?}",
                    payload.node_id, err
                );
                return Err(err);
            }
            while is_member {
                info!(
                    "Waiting until Node {} is not a Learner anymore",
                    payload.node_id
                );
                time::sleep(Duration::from_millis(500)).await;
                metrics = helpers::get_raft_metrics(state, raft_type).await;
                is_member = metrics
                    .membership_config
                    .nodes()
                    .any(|(id, _)| *id == payload.node_id);
            }
        }
    }

    drop(lock);
    info!(
        "Node {} has left the cluster: {:?}",
        payload.node_id,
        metrics.membership_config.membership()
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

    let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    fmt_ok(headers, &metrics)
}
