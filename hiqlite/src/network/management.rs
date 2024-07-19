use crate::network::{fmt_ok, validate_secret, AppStateExt, Error};
use crate::Node;
use crate::NodeId;
use axum::body;
use axum::http::HeaderMap;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Debug, Serialize, Deserialize)]
pub struct LearnerReq {
    pub node_id: u64,
    pub addr_api: String,
    pub addr_raft: String,
}

/// Add a node as **Learner**.
///
/// A Learner receives log replication from the leader but does not vote.
/// This should be done before adding a node as a member into the cluster
/// (by calling `change-membership`)
pub(crate) async fn add_learner(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let LearnerReq {
        node_id,
        addr_api,
        addr_raft,
    } = bincode::deserialize(body.as_ref())?;
    let node = Node {
        id: node_id,
        addr_raft,
        addr_api,
    };
    fmt_ok(headers, state.raft.add_learner(node_id, node, true).await?)
}

/// Changes specified learners to members, or remove members.
pub(crate) async fn change_membership(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let payload: BTreeSet<NodeId> = bincode::deserialize(body.as_ref())?;
    // retain false removes current cluster members if they do not appear in the new list
    fmt_ok(headers, state.raft.change_membership(payload, false).await?)
}

/// Initialize a single-node cluster.
pub(crate) async fn init(state: AppStateExt, headers: HeaderMap) -> Result<(), Error> {
    validate_secret(&state, &headers)?;

    let mut nodes = BTreeMap::new();
    let node = Node {
        id: state.id,
        addr_api: state.addr_api.clone(),
        addr_raft: state.addr_raft.clone(),
    };

    nodes.insert(state.id, node);
    match state.raft.initialize(nodes).await {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::from(err)),
    }
}

/// Get the latest metrics of the cluster
pub(crate) async fn metrics(state: AppStateExt, headers: HeaderMap) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let metrics = state.raft.metrics().borrow().clone();
    fmt_ok(headers, &metrics)
}
