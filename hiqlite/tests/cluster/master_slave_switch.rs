use crate::{Cache, log};
use hiqlite::macros::{TryFromRow, params};
use hiqlite::{Client, Error, Node, NodeConfig, NodeMembers, start_node_with_cache};
use std::time::Duration;
use tokio::{fs, task, time};

const TEST_DATA_DIR: &str = "tests/data_test_master_slave";

/// The full master-slave flip on a live cluster: promote a learner, hand the leadership to a
/// specific voter, demote the old leader to a learner, and flip it all back.
///
/// Proven here:
/// - `GET /cluster/membership` reports voters, learners and leader of every group
/// - a learner is refused as a leadership target, an unknown node is refused as a promote target,
///   and a leader that would demote itself to a learner is refused
/// - the switchover of a master-slave setup is `transfer_leader` to the node that shall lead, then
///   `demote` of the old leader from that node
/// - `promote` moves a learner into the voter set of both groups on every node
/// - `transfer_leader` hands the leadership to exactly the node we ask for
/// - `demote` of the node that led before is the last step of the switchover: it drops to a learner
///   and the leadership stays with the node that was handed the leadership
/// - every flip is visible on every node, in both groups, and the learners keep replicating
/// - a remote client reaches the leader through a follower address, following the redirect
#[tokio::test(flavor = "multi_thread")]
async fn master_slave_switch_promote_transfer_and_demote() {
    let _ = fs::remove_dir_all(TEST_DATA_DIR).await;

    let combo = crate::start::TlsCombo {
        api: false,
        raft: false,
    };
    let (client_1, client_2, client_3) = start_cluster(combo).await.unwrap();

    // a remote client that only knows node 2, which is a follower: every call it makes must be
    // redirected to the leader, which is the path a `Client` built against one fixed address takes
    let remote = Client::remote(
        vec![nodes()[1].addr_api.clone()],
        combo.api,
        false,
        crate::start::SECRET_API.to_string(),
        false,
        None,
        None,
    )
    .await
    .unwrap();

    let leader = wait_for_leader(&client_1, 1, 40).await.unwrap();
    assert_eq!(leader, 1, "node 1 must bootstrap as the leader");

    // 1. the master-slave shape: nodes 1 and 2 are voters, node 3 replicates as a learner
    wait_for_membership([&client_1, &client_2, &client_3], &[1, 2], &[3], 1, 40)
        .await
        .unwrap();

    // 2. a learner can never accept a leadership transfer, so it is refused before anything changes
    let err = client_1.transfer_leadership(3).await.unwrap_err();
    log(format!("learner transfer refused: {err}"));

    // 3. the leaner node becomes a full voting member in both groups
    let members = client_1.promote_node(3).await.unwrap();
    assert_eq!(members.sqlite.voters, vec![1, 2, 3], "{members:?}");
    assert!(members.sqlite.learners.is_empty(), "{members:?}");
    wait_for_membership([&client_1, &client_2, &client_3], &[1, 2, 3], &[], 1, 40)
        .await
        .unwrap();

    // 4. the handover to a specific node: node 1 steps down and node 2 leads both groups
    let members = client_1.transfer_leadership(2).await.unwrap();
    assert_eq!(members.sqlite.leader, Some(2), "{members:?}");
    assert_eq!(members.cache.leader, Some(2), "{members:?}");
    wait_for_leader(&client_1, 2, 40).await.unwrap();
    wait_for_leader(&client_3, 2, 40).await.unwrap();
    let members = client_1.cluster_membership().await.unwrap();
    let own = members.get(&1).unwrap();
    assert_eq!(own.sqlite.state, "Follower", "{own:?}");
    assert_eq!(own.cache.state, "Follower", "{own:?}");

    // 5. the demotion of the old leader is the switchover: node 1 drops to a learner and the
    //    leadership stays with the node we moved it to in step 4
    let members = client_2.demote_node(1, true).await.unwrap();
    assert_eq!(members.sqlite.voters, vec![2, 3], "{members:?}");
    assert_eq!(members.sqlite.learners, vec![1], "{members:?}");
    wait_for_membership([&client_1, &client_2, &client_3], &[2, 3], &[1], 2, 40)
        .await
        .unwrap();
    let members = client_1.cluster_membership().await.unwrap();
    let own = members.get(&1).unwrap();
    assert!(!own.sqlite.is_voter && own.sqlite.is_learner, "{own:?}");
    assert!(!own.cache.is_voter && own.cache.is_learner, "{own:?}");
    assert_eq!(own.sqlite.leader, Some(2), "{own:?}");

    // the demoted node is a healthy learner, and it keeps replicating
    client_1.is_healthy_db().await.unwrap();
    client_1.is_healthy_cache().await.unwrap();

    // 6. a node that is not a member of the cluster cannot be promoted
    let err = client_2.promote_node(9).await.unwrap_err();
    log(format!("promote of an unknown node refused: {err}"));

    // 7. a leader cannot demote itself to a learner: it keeps leading, and openraft cannot
    //    complete such a change, so the call is refused before anything changes
    let err = client_2.demote_node(2, true).await.unwrap_err();
    log(format!("self-demotion of the leader refused: {err}"));

    // 7b. the same self-demotion through the per-group leave endpoint is refused as well: on that
    //    path the leader used to remove itself from the voters, keep leading as a learner, and
    //    go on approving the removal of the remaining voters (F-107)
    let status = leave_group(&nodes()[1].addr_api, 2, true).await;
    assert!(
        !status.is_success(),
        "self-demotion of the leader via /cluster/membership/sqlite must be refused, got {status}"
    );
    let members = client_2.cluster_membership().await.unwrap();
    let own = members.get(&2).unwrap();
    assert!(own.sqlite.is_voter, "{own:?}");
    assert_eq!(own.sqlite.voters, vec![2, 3], "{own:?}");
    assert_eq!(own.sqlite.leader, Some(2), "{own:?}");

    // 8. the switchover in the documented order: name the node that shall lead, then demote the
    //    old leader from that node - the answer carries the new shape and the new leader
    let members = client_2.transfer_leadership(3).await.unwrap();
    assert_eq!(members.sqlite.leader, Some(3), "{members:?}");
    assert_eq!(members.cache.leader, Some(3), "{members:?}");
    wait_for_leader(&client_1, 3, 40).await.unwrap();

    let members = client_3.demote_node(2, true).await.unwrap();
    assert_eq!(members.sqlite.voters, vec![3], "{members:?}");
    assert_eq!(members.sqlite.learners, vec![1, 2], "{members:?}");
    assert_eq!(members.cache.voters, vec![3], "{members:?}");
    assert_eq!(members.cache.learners, vec![1, 2], "{members:?}");
    // `is_voter` / `is_learner` describe the node that answered - node 3 leads and is a voter
    assert!(members.sqlite.is_voter, "{members:?}");
    assert_eq!(members.sqlite.leader, Some(3), "{members:?}");
    assert_eq!(members.cache.leader, Some(3), "{members:?}");
    wait_for_membership([&client_1, &client_2, &client_3], &[3], &[1, 2], 3, 40)
        .await
        .unwrap();
    let own = client_2.cluster_membership().await.unwrap();
    let own = own.get(&2).unwrap();
    assert_eq!(own.sqlite.state, "Learner", "{own:?}");
    assert_eq!(own.cache.state, "Learner", "{own:?}");

    // 9. the data of the switchover is on every node, including both demoted leaders
    write_and_read(&client_3, 1).await.unwrap();
    wait_for_sync(&client_1, 1, 1).await.unwrap();

    // 10. the flip back: node 1 is a voter again, node 3 hands the leadership to it, and the
    //    demotion of node 2 is a no-op that answers the shape, because node 2 is a learner already
    let members = client_3.promote_node(1).await.unwrap();
    assert_eq!(members.sqlite.voters, vec![1, 3], "{members:?}");
    assert_eq!(members.sqlite.learners, vec![2], "{members:?}");
    wait_for_membership([&client_1, &client_2, &client_3], &[1, 3], &[2], 3, 40)
        .await
        .unwrap();

    let members = client_3.transfer_leadership(1).await.unwrap();
    assert_eq!(members.sqlite.leader, Some(1), "{members:?}");
    wait_for_leader(&client_2, 1, 40).await.unwrap();

    let members = client_1.demote_node(2, true).await.unwrap();
    assert_eq!(members.sqlite.voters, vec![1, 3], "{members:?}");
    assert_eq!(members.sqlite.learners, vec![2], "{members:?}");
    wait_for_membership([&client_1, &client_2, &client_3], &[1, 3], &[2], 1, 40)
        .await
        .unwrap();

    // 11. the remote client drives the same endpoints through its follower address: node 2 is a
    //    learner, it is promoted to a voter and demoted again, all against the leader of the cluster
    let members = remote.promote_node(2).await.unwrap();
    assert!(members.sqlite.voters.contains(&2), "{members:?}");
    wait_for_membership([&client_1, &client_2, &client_3], &[1, 2, 3], &[], 1, 40)
        .await
        .unwrap();

    let members = remote.demote_node(2, true).await.unwrap();
    assert_eq!(members.sqlite.voters, vec![1, 3], "{members:?}");
    wait_for_membership([&client_1, &client_2, &client_3], &[1, 3], &[2], 1, 40)
        .await
        .unwrap();

    // the leader of the cluster must have answered the remote calls, and its own view is intact
    write_and_read(&client_1, 2).await.unwrap();
    wait_for_sync(&client_3, 3, 2).await.unwrap();

    join_shutdown([client_1, client_2, client_3]).await;
    let _ = fs::remove_dir_all(TEST_DATA_DIR).await;
}

/// `DELETE /cluster/membership/sqlite` against one node, the per-group leave endpoint.
async fn leave_group(addr_api: &str, node_id: u64, stay_as_learner: bool) -> reqwest::StatusCode {
    reqwest::Client::new()
        .delete(format!("http://{addr_api}/cluster/membership/sqlite"))
        .bearer_auth(crate::start::SECRET_API)
        .header("content-type", "application/json")
        .body(format!(
            r#"{{"node_id":{node_id},"stay_as_learner":{stay_as_learner}}}"#
        ))
        .send()
        .await
        .unwrap()
        .status()
}

fn nodes() -> Vec<Node> {
    vec![
        Node {
            id: 1,
            addr_raft: "127.0.0.1:36201".to_string(),
            addr_api: "127.0.0.1:35201".to_string(),
        },
        Node {
            id: 2,
            addr_raft: "127.0.0.1:36202".to_string(),
            addr_api: "127.0.0.1:35202".to_string(),
        },
        Node {
            id: 3,
            addr_raft: "127.0.0.1:36203".to_string(),
            addr_api: "127.0.0.1:35203".to_string(),
        },
    ]
}

async fn config(node_id: u64, combo: crate::start::TlsCombo) -> NodeConfig {
    let mut config =
        crate::start::build_config_with_nodes(node_id, nodes(), TEST_DATA_DIR, combo).await;
    // the leaner node: it joins as a Learner and never votes in an election
    config.learner_only = node_id == 3;
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

/// Poll until this node sees the wanted leader.
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

/// Poll until every node reports the same voters, learners and leader, in both groups.
async fn wait_for_membership(
    clients: [&Client; 3],
    want_voters: &[u64],
    want_learners: &[u64],
    want_leader: u64,
    polls: u32,
) -> Result<(), Error> {
    for _ in 0..polls {
        let mut is_complete = true;

        for client in clients {
            let members = match client.cluster_membership().await {
                Ok(members) => members,
                Err(err) => {
                    log(format!("membership poll failed: {err}"));
                    is_complete = false;
                    break;
                }
            };

            for (_, state) in members {
                is_complete &= same_group(&state.sqlite, want_voters, want_learners, want_leader);
                is_complete &= same_group(&state.cache, want_voters, want_learners, want_leader);
            }
        }

        if is_complete {
            return Ok(());
        }
        time::sleep(Duration::from_millis(500)).await;
    }

    Err(Error::Error(
        format!(
            "cluster did not reach voters {want_voters:?}, learners {want_learners:?}, leader \
             {want_leader}"
        )
        .into(),
    ))
}

fn same_group(
    members: &NodeMembers,
    want_voters: &[u64],
    want_learners: &[u64],
    want_leader: u64,
) -> bool {
    members.voters == want_voters
        && members.learners == want_learners
        && members.leader == Some(want_leader)
}

#[derive(Debug, TryFromRow)]
struct Flip {
    pub id: i64,
    pub v: String,
}

/// Write a row on the node we pass in and read it back there.
async fn write_and_read(client: &Client, id: i64) -> Result<(), Error> {
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS flip (id INTEGER PRIMARY KEY, v TEXT)",
            params!(),
        )
        .await?;
    let rows = client
        .execute(
            "INSERT INTO flip (id, v) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            params!(id, "flip"),
        )
        .await?;
    assert_eq!(rows, 1);

    let row = client
        .query_try_map_optional::<Flip, _>("SELECT id, v FROM flip WHERE id = $1", params!(id))
        .await?
        .expect("the row we just inserted must be there")
        .expect("the row must convert");
    assert_eq!((row.id, row.v.as_str()), (id, "flip"));
    Ok(())
}

/// Poll until a follower or learner has replicated a row.
async fn wait_for_sync(client: &Client, node_id: u64, id: i64) -> Result<(), Error> {
    for _ in 0..80 {
        let row = client
            .query_try_map_optional::<Flip, _>("SELECT id, v FROM flip WHERE id = $1", params!(id))
            .await
            .unwrap_or_default();
        if matches!(row, Some(Ok(_))) {
            return Ok(());
        }
        time::sleep(Duration::from_millis(500)).await;
    }

    Err(Error::Error(
        format!("row {id} was not replicated to node {node_id}").into(),
    ))
}

async fn join_shutdown(clients: [Client; 3]) {
    for client in clients {
        let _ = tokio::time::timeout(Duration::from_secs(30), client.shutdown()).await;
    }
}
