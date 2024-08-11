use crate::app_state::AppState;
use crate::network::HEADER_NAME_SECRET;
use crate::{Error, Node};
use openraft::{RaftMetrics, StoredMembership};
use std::sync::Arc;
use std::time::Duration;
use tokio::{task, time};
use tracing::{error, info, warn};

pub fn spawn(state: Arc<AppState>, nodes: Vec<Node>, tls: bool) {
    let handle = task::spawn(check_split_brain(state, nodes, tls));

    task::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(600)).await;
            assert!(!handle.is_finished())
        }
    });
}

async fn check_split_brain(state: Arc<AppState>, nodes: Vec<Node>, tls: bool) {
    loop {
        time::sleep(Duration::from_secs(30)).await;

        #[cfg(feature = "sqlite")]
        match state.raft_db.raft.current_leader().await {
            None => {
                warn!("No leader for DB");
                continue;
            }
            Some(leader_expected) => {
                let metrics = state.raft_db.raft.metrics().borrow().clone();
                let membership = metrics.membership_config;
                info!("Expected leader for DB: {}", leader_expected);

                check_nodes_in_members(&nodes, &membership);

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
                    error!("Error during check_compare_membership: {}", err);
                }
            }
        };

        #[cfg(feature = "cache")]
        match state.raft_cache.raft.current_leader().await {
            None => {
                warn!("No leader for Cache");
                continue;
            }
            Some(leader_expected) => {
                let metrics = state.raft_cache.raft.metrics().borrow().clone();
                let membership = metrics.membership_config;
                info!("Expected leader for Cache: {}", leader_expected);

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
                    error!("Error during check_compare_membership: {}", err);
                }
            }
        };
    }
}

fn check_nodes_in_members(nodes: &[Node], membership: &Arc<StoredMembership<u64, Node>>) {
    let members = membership.nodes().map(|(id, _)| *id).collect::<Vec<_>>();

    for node in nodes {
        if !members.contains(&node.id) {
            error!("Node {} not in membership config: {:?}", node.id, members);
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
        let metrics = bincode::deserialize::<RaftMetrics<u64, Node>>(&bytes).unwrap();
        // let metrics = res.json::<RaftMetrics<u64, Node>>().await?;
        let members = metrics.membership_config;

        check_nodes_in_members(&nodes, &members);

        if members != membership {
            error!(
                "Difference in membership config for {}:\n\nlocal:\n{:?}\n\nremote:\n{:?}",
                path, membership, members
            );
        }
    }

    Ok(())
}
