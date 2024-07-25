use crate::app_state::RaftType;
use crate::network::{fmt_ok, get_payload, validate_secret, AppStateExt, Error};
use crate::NodeId;
use crate::{helpers, Node};
use axum::body;
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::response::Response;
use openraft::error::{CheckIsLeaderError, ForwardToLeader, RaftError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct LearnerReq {
    pub node_id: u64,
    pub addr_api: String,
    pub addr_raft: String,
}

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

    if let Some(leader_id) = helpers::get_raft_leader(&state, &raft_type).await {
        if leader_id != state.id {
            let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
            let members = metrics.membership_config;
            let leader = members
                .nodes()
                .filter(|(id, _)| **id == leader_id)
                .collect::<Vec<(&u64, &Node)>>();
            assert_eq!(leader.len(), 1);
            let (_, node) = leader[0];

            let err = RaftError::APIError(CheckIsLeaderError::ForwardToLeader(ForwardToLeader {
                leader_id: Some(leader_id),
                leader_node: Some(node.clone()),
            }));
            return Err(Error::CheckIsLeaderError(err));
        }
    } else {
        return Err(Error::LeaderChange("Leader election in progress".into()));
    }

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

    // Check if the node is maybe already a member.
    // If this is the case, it might do the request because it tries to recover from volume loss.
    // -> remove the membership and re-add it as a new learner, so it can catch up again.
    {
        // hold this lock the whole time, even over await points, to never have race conditions here ...
        let _lock = helpers::lock_raft(&state, &raft_type).await;

        let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
        let members = metrics.membership_config;
        let is_member_already = members.nodes().any(|(id, _)| *id == node.id);

        if is_member_already {
            let new_voters = members
                .voter_ids()
                .filter(|id| *id != node.id)
                .collect::<Vec<u64>>();

            let new_members = members
                .nodes()
                .filter_map(|(id, _)| new_voters.contains(id).then_some(*id))
                .collect::<BTreeSet<u64>>();

            info!(
                r#"

    Members old: {:?}
    new_voters:  {:?}
    new_members: {:?}

            "#,
                members, new_voters, new_members
            );

            let res = helpers::change_membership(&state, &raft_type, new_members, false).await;
            match res {
                Ok(_) => {
                    info!("Removed already existing member");

                    // TODO do we need this timeout? probably not -> more testing
                    time::sleep(Duration::from_millis(100)).await;

                    info!("Adding removed member as learner");
                    helpers::add_new_learner(&state, &raft_type, node).await?;
                    // state.raft_db.raft.add_learner(node.id, node, true).await?;

                    info!("Membership changed successfully");

                    let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
                    let members = metrics.membership_config;
                    info!(
                        r#"

        New Membership after updates: {:?}

                    "#,
                        members
                    );

                    return fmt_ok(headers, ());
                }
                Err(err) => {
                    error!("Error adding node as learner: {:?}", err);
                    return Err(err);
                }
            }
        }
    }

    let res = helpers::add_new_learner(&state, &raft_type, node).await;
    // let res = state.raft_db.raft.add_learner(node_id, node, true).await;
    match res {
        Ok(_) => {
            info!("Added node as learner");
            fmt_ok(headers, ())
        }
        Err(err) => {
            error!("Error adding node as learner: {:?}", err);
            Err(err)
        }
    }
}

/// Changes specified learners to members, or remove members.
pub(crate) async fn become_member(
    state: AppStateExt,
    headers: HeaderMap,
    Path(raft_type): Path<RaftType>,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let payload = get_payload::<Node>(&headers, body)?;
    info!("Node membership request: {:?}\n", payload);

    // we want to hold the lock until we finished to not end up with race conditions
    let _lock = helpers::lock_raft(&state, &raft_type).await;

    let metrics = helpers::get_raft_metrics(&state, &raft_type).await;
    let members = metrics.membership_config;

    let mut nodes_set = BTreeSet::new();
    for (id, _node) in members.nodes() {
        nodes_set.insert(*id);
    }
    nodes_set.insert(payload.id);

    let res = helpers::change_membership(&state, &raft_type, nodes_set, true).await;
    match res {
        Ok(_) => {
            info!("Added node as member");
            fmt_ok(headers, ())
        }
        Err(err) => {
            error!("Error adding node as member: {:?}", err);
            Err(err)
        }
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
    let members = metrics.membership_config;
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

// /// Initialize a single-node cluster.
// pub(crate) async fn init(state: AppStateExt, headers: HeaderMap) -> Result<(), Error> {
//     validate_secret(&state, &headers)?;
//
//     let mut nodes = BTreeMap::new();
//     let node = Node {
//         id: state.id,
//         addr_api: state.addr_api.clone(),
//         addr_raft: state.addr_raft.clone(),
//     };
//
//     nodes.insert(state.id, node);
//     match state.raft_db.raft.initialize(nodes).await {
//         Ok(_) => Ok(()),
//         Err(err) => Err(Error::from(err)),
//     }
// }

/// Get the latest metrics of the cluster
pub(crate) async fn metrics(state: AppStateExt, headers: HeaderMap) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let metrics = state.raft_db.raft.metrics().borrow().clone();
    fmt_ok(headers, &metrics)
}
