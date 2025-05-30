use crate::app_state::AppState;
use crate::helpers::deserialize;
use crate::network::HEADER_NAME_SECRET;
use crate::{Error, Node};
use openraft::{RaftMetrics, StoredMembership};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::{task, time};
use tracing::{debug, error, warn};

pub fn spawn(state: Arc<AppState>, nodes: Vec<Node>, tls: bool) {
    let handle = task::spawn(check_split_brain(state, nodes, tls));

    // TODO just a safety net until everything runs super smooth and stable
    task::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(600)).await;
            assert!(!handle.is_finished())
        }
    });
}

async fn check_split_brain(state: Arc<AppState>, nodes: Vec<Node>, tls: bool) {
    let interval = env::var("HQL_SPLIT_BRAIN_INTERVAL")
        .as_deref()
        .unwrap_or("60")
        .parse::<u64>()
        .expect("Cannot parse HQL_SPLIT_BRAIN_INTERVAL as u64");

    loop {
        time::sleep(Duration::from_secs(interval)).await;

        #[cfg(feature = "sqlite")]
        match state.raft_db.raft.current_leader().await {
            None => {
                warn!("Node {}: No leader for DB", state.id);
            }
            Some(leader_expected) => {
                debug!("Node {}: Raft DB Leader: {}", state.id, leader_expected);
                let metrics = state.raft_db.raft.metrics().borrow().clone();
                let membership = metrics.membership_config;

                if let Err(err) = check_compare_membership(
                    &state,
                    &nodes,
                    membership,
                    leader_expected,
                    "sqlite",
                    tls,
                )
                .await
                {
                    error!(
                        "Node {}: Error during check_compare_membership: {}",
                        state.id, err
                    );
                }
            }
        };

        #[cfg(feature = "cache")]
        match state.raft_cache.raft.current_leader().await {
            None => {
                warn!("Node {}: No leader for Cache", state.id);
            }
            Some(leader_expected) => {
                debug!("Node {}: Raft Cache Leader: {}", state.id, leader_expected);
                let metrics = state.raft_cache.raft.metrics().borrow().clone();
                let membership = metrics.membership_config;

                if let Err(err) = check_compare_membership(
                    &state,
                    &nodes,
                    membership,
                    leader_expected,
                    "cache",
                    tls,
                )
                .await
                {
                    error!(
                        "Node {}: Error during check_compare_membership: {}",
                        state.id, err
                    );
                }
            }
        };
    }
}

fn check_nodes_in_members(
    node_id: u64,
    typ: &str,
    node_id_remote: u64,
    nodes: &[Node],
    membership: &Arc<StoredMembership<u64, Node>>,
) {
    let members = membership.nodes().map(|(id, _)| *id).collect::<Vec<_>>();

    for node in nodes {
        if !members.contains(&node.id) {
            warn!(
                r#"

Node {}: {} node {} not in membership config from Node {node_id_remote}: {:?}
If the missing node is up and running, this is a split brain and should not happen.
If however the missing node is currently offline or just starting up, you can ignore this message.
"#,
                node_id, typ, node.id, members
            );
        }
    }
}

async fn check_compare_membership(
    state: &Arc<AppState>,
    nodes: &[Node],
    membership: Arc<StoredMembership<u64, Node>>,
    leader_expected: u64,
    path: &str,
    tls: bool,
) -> Result<(), Error> {
    let nodes_to_check = nodes
        .iter()
        .filter(|node| node.id != leader_expected)
        .collect::<Vec<_>>();

    if nodes_to_check.is_empty() {
        return Ok(());
    }

    let scheme = if tls { "https" } else { "http" };

    let client = reqwest::Client::new();
    for node in nodes_to_check {
        let url = format!("{}://{}/cluster/metrics/{}", scheme, node.addr_api, path);
        let res = client
            .get(&url)
            .header(HEADER_NAME_SECRET, &state.secret_api)
            .send()
            .await?;
        if !res.status().is_success() {
            let err = res.json::<Error>().await;
            error!("Error metrics lookup to {}: {:?}", url, err);
            continue;
        }

        let bytes = res.bytes().await?;
        let metrics = deserialize::<RaftMetrics<u64, Node>>(&bytes)?;
        let members = metrics.membership_config;

        check_nodes_in_members(state.id, path, node.id, nodes, &members);

        if members != membership {
            error!(
                "Difference in membership config on Node {} for {}:\n\nlocal:\n{:?}\n\nremote ({}):\n{:?}",
                state.id, path, membership, node.id, members
            );
        }
    }

    Ok(())
}
