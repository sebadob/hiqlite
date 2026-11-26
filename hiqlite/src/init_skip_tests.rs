//! Regression tests for the decision a pristine node 1 makes before it forms a new cluster.
//!
//! The remotes are tiny HTTP servers that answer `GET /cluster/membership/sqlite` the way a real
//! node does, plus a closed port for a node that is down.

use super::{InitCheck, should_node_1_skip_init};
use crate::app_state::RaftType;
use crate::{Error, Node, NodeId};
use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use openraft::Membership;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time;

const SECRET: &str = "qwertz123456";

fn node(id: u64, addr_api: String) -> Node {
    Node {
        id,
        addr_raft: format!("127.0.0.1:{}", 1 + id),
        addr_api,
    }
}

/// An address nothing listens on.
async fn closed_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    drop(listener);
    addr
}

/// A remote that is a member of an initialized cluster of nodes 1, 2 and 3.
async fn initialized_remote() -> String {
    let nodes = (1..=3)
        .map(|id| (id, node(id, format!("127.0.0.1:{}", 100 + id))))
        .collect::<BTreeMap<NodeId, Node>>();
    let membership =
        Membership::<NodeId, Node>::new(vec![BTreeSet::from([1, 2, 3])], nodes).unwrap();
    let body = crate::helpers::serialize_serde(&membership);
    serve(Router::new().route(
        "/cluster/membership/sqlite",
        get(move || {
            let body = body.clone();
            async move { body.into_response() }
        }),
    ))
    .await
}

/// A remote whose Raft group has never been initialized.
async fn pristine_remote() -> String {
    serve(Router::new().route(
        "/cluster/membership/sqlite",
        get(|| async {
            let err = Error::Config("Raft node has not been initialized".into());
            (StatusCode::BAD_REQUEST, serde_json::to_vec(&err).unwrap()).into_response()
        }),
    ))
    .await
}

async fn serve(router: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    addr
}

async fn skip_init(nodes: &[Node], within: Duration) -> Option<bool> {
    skip_init_with(nodes, within, false).await
}

async fn skip_init_with(nodes: &[Node], within: Duration, require_peers: bool) -> Option<bool> {
    // `start_node` installs it in a real node; it fails when another test already did
    let _ = rustls::crypto::ring::default_provider().install_default();

    let check = InitCheck {
        nodes,
        secret_api: SECRET,
        tls: false,
        tls_no_verify: false,
        require_peers,
    };
    time::timeout(within, should_node_1_skip_init(&RaftType::Sqlite, &check))
        .await
        .ok()
        .map(|res| res.unwrap())
}

/// F-118: node 2 is down, node 3 holds the existing cluster. Node 1 must join it instead of
/// forming a second cluster on its own because node 2 did not answer.
#[tokio::test]
async fn unreachable_peer_does_not_make_node_1_initialize() {
    let nodes = [
        node(1, closed_addr().await),
        node(2, closed_addr().await),
        node(3, initialized_remote().await),
    ];
    assert_eq!(skip_init(&nodes, Duration::from_secs(10)).await, Some(true));
}

/// F-118: node 2 is pristine, node 3 is down and may hold the existing cluster. One pristine
/// answer out of two remotes is not a quorum of the other nodes, so node 1 must keep waiting.
#[tokio::test]
async fn one_pristine_answer_is_not_enough_to_initialize() {
    let nodes = [
        node(1, closed_addr().await),
        node(2, pristine_remote().await),
        node(3, closed_addr().await),
    ];
    assert_eq!(skip_init(&nodes, Duration::from_secs(4)).await, None);
}

/// A brand new cluster whose nodes all run: node 1 forms it right away.
#[tokio::test]
async fn pristine_peers_let_node_1_initialize() {
    let nodes = [
        node(1, closed_addr().await),
        node(2, pristine_remote().await),
        node(3, pristine_remote().await),
    ];
    assert_eq!(skip_init(&nodes, Duration::from_secs(4)).await, Some(false));
}

/// No other node is reachable: by default node 1 forms the cluster alone after 10 rounds, as a
/// fresh cluster started one node after the other needs it to.
#[tokio::test]
async fn all_unreachable_lets_node_1_initialize_by_default() {
    let nodes = [
        node(1, closed_addr().await),
        node(2, closed_addr().await),
        node(3, closed_addr().await),
    ];
    assert_eq!(
        skip_init(&nodes, Duration::from_secs(15)).await,
        Some(false)
    );
}

/// F-118, full outage: with `init_require_peers` node 1 never forms the cluster alone, however
/// long no other node is reachable, and joins once one of them answers.
#[tokio::test]
async fn require_peers_keeps_node_1_waiting_while_all_are_unreachable() {
    let nodes = [
        node(1, closed_addr().await),
        node(2, closed_addr().await),
        node(3, closed_addr().await),
    ];
    assert_eq!(
        skip_init_with(&nodes, Duration::from_secs(15), true).await,
        None
    );

    let nodes = [
        node(1, closed_addr().await),
        node(2, closed_addr().await),
        node(3, initialized_remote().await),
    ];
    assert_eq!(
        skip_init_with(&nodes, Duration::from_secs(10), true).await,
        Some(true)
    );
}
