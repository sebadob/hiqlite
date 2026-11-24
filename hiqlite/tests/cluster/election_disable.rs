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
/// - a probe against a live leader does not touch the term, and a `force` election inflates it
/// - a node that is disabled for elections stays a follower, even as the node with the highest term
///
/// FLAKY - make it robust before anything is added to it.  Its step 7 waits for node 2 to take the
/// leadership after the forced election on node 1 through `wait_for_leader(.., 120)`: 120 polls of
/// 500 ms, a wall-clock deadline of 60 s.  On a loaded machine node 2 does not always get there
/// inside that window, and the whole cluster suite then dies with
/// `node 2 did not become the leader, current: Some(1)`.  That step asserts a race it only half
/// controls: node 1's inflated term, its own election timeout and node 2's self-nomination are all
/// driven by timers, so raising the poll count only moves the deadline.  The robust shape is a
/// verdict that does not depend on who won inside a window - prove the forced election inflated the
/// term and that node 1 stays a follower, and take the leadership change itself from a transfer,
/// whose outcome the test can await.  Seen failing on 2026-11-24; it passes on an idle machine.
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

    // 2. the leader hands over to the node that can win an election: node 2, not the marked node 3
    client_1.shutdown().await.unwrap();
    let leader = wait_for_leader(&client_2, 2, 40).await.unwrap();
    assert_eq!(leader, 2, "the handover must skip the marked node");
    let metrics = client_3.metrics_db().await.unwrap();
    assert_eq!(metrics.current_leader, Some(2));
    assert_eq!(metrics.state, ServerState::Follower);

    // 3. node 2 has only the marked node left to hand over to, and the transfer reaches it
    client_2.shutdown().await.unwrap();
    let leader = wait_for_leader(&client_3, 3, 40).await.unwrap();
    assert_eq!(leader, 3, "the marked node accepts a leadership transfer");

    // 4. the runtime switch: the state flips, the config marker stays
    let state = client_3.set_node_election(3, true).await.unwrap();
    assert!(state.sqlite.enable_elect && state.cache.enable_elect);
    assert!(state.sqlite.election_disable && state.cache.election_disable);

    // 5. node 1 and 2 come back as followers of node 3
    let client_1 = restart(1, combo).await.unwrap();
    let client_2 = restart(2, combo).await.unwrap();
    let leader = wait_for_leader(&client_1, 3, 40).await.unwrap();
    assert_eq!(leader, 3);
    wait_for_leader(&client_2, 3, 40).await.unwrap();

    // 6. a probe against a live leader changes nothing: no term inflation, no new leader
    let term_before = client_1.metrics_db().await.unwrap().current_term;
    client_1.elect_node(1, false).await.unwrap();
    time::sleep(Duration::from_secs(6)).await;
    let metrics = client_1.metrics_db().await.unwrap();
    assert_eq!(
        metrics.current_term, term_before,
        "an election probe must not inflate the term"
    );
    assert_eq!(metrics.state, ServerState::Follower);
    assert_eq!(metrics.current_leader, Some(3));

    // 7. back to the geo setup: node 3 is switched off for elections again, and node 1 as well. A
    //    forced election on node 1 inflates its term, which is the proof that `force` bypasses both
    //    openraft's Pre-Vote and the node's own disabled state. It cannot win, it has no votes, and
    //    as a follower with `enable_elect = false` it never nominates itself again, even though it
    //    is the node with the highest term. The leader loses its position over the higher vote of
    //    node 1, node 3 is switched off too, so node 2 takes the leadership, and the term-inflated
    //    follower catches up again.
    let state = client_3.set_node_election(3, false).await.unwrap();
    assert!(!state.sqlite.enable_elect && !state.cache.enable_elect);
    client_1.set_node_election(1, false).await.unwrap();

    let term_before = client_1.metrics_db().await.unwrap().current_term;
    client_1.elect_node(1, true).await.unwrap();

    // node 1 cannot win and never nominates itself again, node 3 is switched off as well, so node 2
    // is the only one left that can take the leadership, and it does
    //
    // FLAKY: this 60 s wall-clock poll is the flake of the test - see its doc comment.
    let leader = wait_for_leader(&client_1, 2, 120).await.unwrap();
    assert_eq!(
        leader, 2,
        "the only node that may self-nominate takes the leadership"
    );

    // several election timeouts: node 1 has the highest term of the cluster and must stay quiet
    time::sleep(Duration::from_secs(8)).await;
    let metrics = client_1.metrics_db().await.unwrap();
    assert_eq!(metrics.current_leader, Some(2), "{metrics:?}");
    assert_eq!(metrics.state, ServerState::Follower, "{metrics:?}");
    assert!(
        metrics.current_term > term_before,
        "a forced election must inflate the term: {term_before} -> {}",
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
    log(format!("Cluster re-stabilized with leader {leader}"));

    let state = client_1.set_node_election(1, true).await.unwrap();
    assert!(state.sqlite.enable_elect && state.cache.enable_elect);

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

async fn restart(node_id: u64, combo: crate::start::TlsCombo) -> Result<Client, Error> {
    let handle = task::spawn(start_node_with_cache::<Cache>(config(node_id, combo).await));
    let client = handle.await.unwrap().unwrap();
    wait_for_health(&client).await.unwrap();
    Ok(client)
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
