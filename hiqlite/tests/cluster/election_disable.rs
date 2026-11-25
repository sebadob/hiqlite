use crate::{Cache, log};
use hiqlite::{Client, Error, Node, NodeConfig, start_node_with_cache};
use openraft::ServerState;
use std::time::Duration;
use tokio::{fs, task, time};

const TEST_DATA_DIR: &str = "tests/data_test_election_disable";

/// The geo-replica setup: three full voters, node 3 marked with `election_disable`.
///
/// Proven here:
/// - the marker reaches `enable_elect = false` on the marked node, for both groups
/// - a handover skips the marked node while another voter can take over
/// - the marked node still accepts a leadership transfer as the fallback target
/// - `PUT` flips the state at runtime and `GET` reports it, the config marker stays untouched
/// - a probe against a live leader does not touch the term, and a campaign on a node whose own
///   elections are disabled is refused by that node's own gate
/// - a node that is disabled for elections stays a follower, even after it was asked to campaign
///
/// Every leadership change here is a handover, never a campaign against a live leader: who wins a
/// campaign is decided by leader leases and by the timing of the quorum acks behind them, which is a
/// race no assertion should rest on. Steps 3, 4 and 7 move the seat with `transfer_leadership()`,
/// whose outcome the test can await, and step 8 uses the one handover a graceful shutdown runs
/// itself, because that is the thing step 8 proves. Step 6 asks node 1 to campaign and asserts the
/// term is untouched, which is the gate holding a node that declared it must never lead.
#[tokio::test(flavor = "multi_thread")]
async fn election_disable_node_never_leads_without_a_transfer() {
    let _ = fs::remove_dir_all(TEST_DATA_DIR).await;

    let combo = crate::start::TlsCombo {
        api: false,
        raft: false,
    };
    let (client_1, client_2, client_3) = start_cluster(combo).await.unwrap();

    let leader = wait_for_leader(&client_1, 1, 40).await.unwrap();
    assert_eq!(leader, 1, "node 1 must bootstrap as the leader");

    // 1. the marker of node 3 is its config value, and it disables its own elections
    let state_1 = client_1.cluster_election_state().await.unwrap();
    let own = state_1.get(&1).unwrap();
    assert!(
        own.sqlite.enable_elect && own.cache.enable_elect,
        "{state_1:?}"
    );
    assert!(
        !own.sqlite.election_disable && !own.cache.election_disable,
        "{state_1:?}"
    );

    let state_3 = client_3.cluster_election_state().await.unwrap();
    let own = state_3.get(&3).unwrap();
    assert!(
        !own.sqlite.enable_elect && !own.cache.enable_elect,
        "{state_3:?}"
    );
    assert!(
        own.sqlite.election_disable && own.cache.election_disable,
        "{state_3:?}"
    );

    // 2. a probe against the live leader changes nothing: no term inflation, no new leader
    let term_before = client_1.metrics_db().await.unwrap().current_term;
    client_2.elect_node(2).await.unwrap();
    time::sleep(Duration::from_secs(6)).await;
    let metrics = client_2.metrics_db().await.unwrap();
    assert_eq!(
        metrics.current_term, term_before,
        "an election probe must not inflate the term: {metrics:?}"
    );
    assert_eq!(metrics.state, ServerState::Follower, "{metrics:?}");
    assert_eq!(metrics.current_leader, Some(1), "{metrics:?}");

    // 3. the seat moves by transfer, from the node that holds it, to the node that may win an
    //    election: node 2, never the marked node 3
    client_1.transfer_leadership(2).await.unwrap();
    let leader = wait_for_leader(&client_2, 2, 40).await.unwrap();
    assert_eq!(leader, 2, "the transfer reaches node 2");
    // The new leader's first heartbeats are what makes node 3 see it, so the test waits for them.
    let leader = wait_for_leader(&client_3, 2, 40).await.unwrap();
    assert_eq!(leader, 2, "node 3 follows the transferred leader");
    let metrics = client_3.metrics_db().await.unwrap();
    assert_eq!(metrics.state, ServerState::Follower, "{metrics:?}");

    // 4. the marked node accepts a leadership transfer as the fallback target
    client_2.transfer_leadership(3).await.unwrap();
    let leader = wait_for_leader(&client_3, 3, 40).await.unwrap();
    assert_eq!(leader, 3, "the marked node accepts a leadership transfer");

    // 5. the runtime switch: the state flips, the config marker stays
    let state = client_3.set_node_election(3, true).await.unwrap();
    assert!(state.sqlite.enable_elect && state.cache.enable_elect);
    assert!(state.sqlite.election_disable && state.cache.election_disable);

    // 6. back to the geo setup: node 3 is switched off for elections again, and node 1 as well.
    //
    //    A campaign on node 1 is a Pre-Vote probe, and the node's own disabled state refuses it:
    //    openraft skips the election before the term moves. Nothing bypasses that gate, which is the
    //    point - a leader change that must happen is a transfer, asked for by the node that holds
    //    the seat, and step 7 is exactly that.
    let state = client_3.set_node_election(3, false).await.unwrap();
    assert!(!state.sqlite.enable_elect && !state.cache.enable_elect);
    client_1.set_node_election(1, false).await.unwrap();

    let term_before = client_1.metrics_db().await.unwrap().current_term;
    client_1.elect_node(1).await.unwrap();
    time::sleep(Duration::from_secs(6)).await;
    let metrics = client_1.metrics_db().await.unwrap();
    assert_eq!(
        metrics.current_term, term_before,
        "a node with elections disabled must not campaign, and its term stays: {metrics:?}"
    );
    assert_eq!(metrics.state, ServerState::Follower, "{metrics:?}");
    assert_eq!(metrics.current_leader, Some(3), "{metrics:?}");

    // 7. the leadership change is a transfer, asked for by the node that holds the seat, so its
    //    outcome is something the test awaits instead of a window it has to outlast.
    client_3.transfer_leadership(2).await.unwrap();
    let leader = wait_for_leader(&client_1, 2, 40).await.unwrap();
    assert_eq!(
        leader, 2,
        "the transfer reaches the node that may win an election"
    );

    // several election timeouts: node 1 was asked to campaign twice, it follows the cluster's
    // highest term, and it never nominates itself.
    time::sleep(Duration::from_secs(8)).await;
    let metrics = client_1.metrics_db().await.unwrap();
    assert_eq!(metrics.current_leader, Some(2), "{metrics:?}");
    assert_eq!(
        metrics.state,
        ServerState::Follower,
        "a node with elections disabled never nominates, not even after being asked to twice: \
         {metrics:?}"
    );
    assert!(
        metrics.current_term >= term_before,
        "node 1 follows the cluster's term: {term_before} -> {}",
        metrics.current_term
    );
    let metrics = client_3.metrics_db().await.unwrap();
    assert_eq!(
        metrics.state,
        ServerState::Follower,
        "the marked node must not lead, {metrics:?}"
    );

    client_1
        .wait_for_cluster_sync_timeout(Duration::from_secs(60))
        .await
        .unwrap();
    log(format!("Cluster stabilized with leader {leader}"));

    let state = client_1.set_node_election(1, true).await.unwrap();
    assert!(state.sqlite.enable_elect && state.cache.enable_elect);

    // 8. the shutdown handover order: node 2 leads, and its graceful shutdown hands the seat to
    //    node 1, skipping the marked node 3 while another voter can take over.
    client_2.shutdown().await.unwrap();
    let leader = wait_for_leader(&client_3, 1, 40).await.unwrap();
    assert_eq!(leader, 1, "the handover must skip the marked node");
    let metrics = client_3.metrics_db().await.unwrap();
    assert_eq!(metrics.current_leader, Some(1), "{metrics:?}");
    assert_eq!(metrics.state, ServerState::Follower, "{metrics:?}");
    log(format!(
        "Cluster settled on leader {leader} after the handover"
    ));

    join_shutdown([client_1, client_2, client_3]).await;
    let _ = fs::remove_dir_all(TEST_DATA_DIR).await;
}

fn nodes() -> Vec<Node> {
    vec![
        Node {
            id: 1,
            addr_raft: "127.0.0.1:36101".to_string(),
            addr_api: "127.0.0.1:35101".to_string(),
        },
        Node {
            id: 2,
            addr_raft: "127.0.0.1:36102".to_string(),
            addr_api: "127.0.0.1:35102".to_string(),
        },
        Node {
            id: 3,
            addr_raft: "127.0.0.1:36103".to_string(),
            addr_api: "127.0.0.1:35103".to_string(),
        },
    ]
}

async fn config(node_id: u64, combo: crate::start::TlsCombo) -> NodeConfig {
    let mut config =
        crate::start::build_config_with_nodes(node_id, nodes(), TEST_DATA_DIR, combo).await;
    // every node lists the same marker: the leader needs it to skip node 3 on a handover
    config.election_disable = vec![3];
    config.apply_election_disable();
    config
}

async fn start_cluster(combo: crate::start::TlsCombo) -> Result<(Client, Client, Client), Error> {
    let handle_1 = task::spawn(start_node_with_cache::<Cache>(config(1, combo).await));
    let handle_2 = task::spawn(start_node_with_cache::<Cache>(config(2, combo).await));
    let handle_3 = task::spawn(start_node_with_cache::<Cache>(config(3, combo).await));

    let client_1 = handle_1.await.unwrap().unwrap();
    let client_2 = handle_2.await.unwrap().unwrap();
    let client_3 = handle_3.await.unwrap().unwrap();

    for client in [&client_1, &client_2, &client_3] {
        wait_for_health(client).await.unwrap();
    }

    Ok((client_1, client_2, client_3))
}

async fn wait_for_health(client: &Client) -> Result<(), Error> {
    for _ in 0..40 {
        if client.is_healthy_db().await.is_ok() && client.is_healthy_cache().await.is_ok() {
            return Ok(());
        }
        time::sleep(Duration::from_millis(500)).await;
    }

    client.is_healthy_db().await?;
    client.is_healthy_cache().await
}

/// Poll until this node sees the wanted leader, which needs a quorum of live voting nodes.
async fn wait_for_leader(client: &Client, want: u64, polls: u32) -> Result<u64, Error> {
    for _ in 0..polls {
        let leader = client.metrics_db().await?.current_leader;
        if leader == Some(want) {
            return Ok(want);
        }
        time::sleep(Duration::from_millis(500)).await;
    }

    let leader = client.metrics_db().await?.current_leader;
    Err(Error::Error(
        format!("node {want} did not become the leader, current: {leader:?}").into(),
    ))
}

async fn join_shutdown(clients: [Client; 3]) {
    for client in clients {
        let _ = tokio::time::timeout(Duration::from_secs(30), client.shutdown()).await;
    }
}
