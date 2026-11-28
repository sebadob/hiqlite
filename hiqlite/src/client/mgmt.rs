use crate::app_state::{AppState, RaftType};
use crate::helpers::{deserialize, deserialize_serde, serialize};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use crate::helpers::{shutdown_transfer_deadline, transfer_leadership_before_shutdown};
use crate::{Client, Error, NodeId};
use openraft::Instant;
use openraft::RaftTypeConfig;
use openraft::ServerState;
use openraft::type_config::async_runtime::watch::WatchReceiver;
use reqwest::Method;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info};

use crate::network::management::{
    self, ClusterLeaveReq, ElectionReq, ElectionState, MembersState, NodeReq,
};
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigCache;
#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;
#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::writer::WriterRequest;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use openraft::RaftMetrics;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use std::clone::Clone;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use std::sync::atomic::Ordering;

/// Maximum number of log entries a node may lag behind the cluster tip while still counting as
/// "in sync". Followers are normally a bit behind the leader, so we never require an exact index
/// match - instead callers wait until the gap drops to this bound or below.
const CLUSTER_SYNC_ALLOWED_DIFF: u64 = 100;

impl Client {
    /// Get cluster metrics for the database Raft.
    ///
    /// # Errors
    ///
    /// Only the remote path can fail: [`Error::Token`] when the client has no API secret,
    /// [`Error::Connect`] when the node does not answer, [`Error::Bincode`] for an answer that
    /// cannot be deserialized, and the [`Error`] the node reports back. An embedded client
    /// reads its own metrics and never fails.
    #[cfg(feature = "sqlite")]
    pub async fn metrics_db(&self) -> Result<RaftMetrics<TypeConfigSqlite>, Error> {
        if let Some(state) = &self.inner.state {
            let metrics = state.raft_db.raft.metrics().borrow_watched().clone();
            Ok(metrics)
        } else {
            let url = self
                .build_addr("/cluster/metrics/sqlite", &self.inner.leader_db)
                .await;
            self.get_metrics_remote(url).await
        }
    }

    /// Build a snapshot of the database Raft and purge the logs it covers, so that the log does
    /// not start at ID 1 anymore.
    ///
    /// A node that joins later then always receives this snapshot instead of replaying the log
    /// from ID 1. Hiqlite does this on its own after a backup restore and after a Raft state
    /// reset on a single node. It writes a few logs first, so it must run on the current leader,
    /// and only an embedded client can do that. The purge is retried until it succeeds.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] for a remote client that has no local node, [`Error::LeaderChange`]
    /// when this node is not the leader of the database Raft, and [`Error::RaftErrorFatalDb`]
    /// when the Raft is shutting down or has a storage error.
    #[cfg(feature = "sqlite")]
    pub async fn snapshot_db(&self) -> Result<(), Error> {
        let Some(state) = &self.inner.state else {
            return Err(Error::Config(
                "A remote client cannot build a snapshot, use an embedded client".into(),
            ));
        };
        let leader = state.raft_db.raft.metrics().borrow_watched().current_leader;
        if leader != Some(state.id) {
            return Err(Error::LeaderChange(
                format!("This node is not the DB leader, the current leader is {leader:?}").into(),
            ));
        }
        state.raft_db.snapshot_purge_logs().await
    }

    /// Get cluster metrics for the cache Raft.
    ///
    /// # Errors
    ///
    /// Only the remote path can fail: [`Error::Token`] when the client has no API secret,
    /// [`Error::Connect`] when the node does not answer, [`Error::Bincode`] for an answer that
    /// cannot be deserialized, and the [`Error`] the node reports back. An embedded client
    /// reads its own metrics and never fails.
    #[cfg(feature = "cache")]
    pub async fn metrics_cache(&self) -> Result<RaftMetrics<TypeConfigCache>, Error> {
        if let Some(state) = &self.inner.state {
            let metrics = state.raft_cache.raft.metrics().borrow_watched().clone();
            Ok(metrics)
        } else {
            let url = self
                .build_addr("/cluster/metrics/cache", &self.inner.leader_cache)
                .await;
            self.get_metrics_remote(url).await
        }
    }

    // This is separated from the `self.send_with_retry_db()` to avoid recursion on leader unreachable
    async fn get_metrics_remote<C>(&self, url: String) -> Result<RaftMetrics<C>, Error>
    where
        C: RaftTypeConfig,
    {
        // This should never be called if we have a local client with its own replicated data
        debug_assert!(
            self.inner.state.is_none(),
            "get_metrics_remote should never be called with local state"
        );
        debug_assert!(
            self.inner.api_secret.is_some(),
            "api_secret should always exist for remote clients"
        );

        let res = self
            .inner
            .client
            .as_ref()
            .unwrap()
            .get(url)
            .bearer_auth(self.inner.api_secret.as_ref().unwrap())
            .send()
            .await?;

        if res.status().is_success() {
            let bytes = res.bytes().await?;
            Ok(deserialize_serde(bytes.as_ref())?)
        } else {
            let err = res.json::<Error>().await?;
            Err(err)
        }
    }

    /// The election state of every cluster node, keyed by node id.
    ///
    /// An embedded client knows its own node only, so it reports that one. A remote client asks
    /// every node address it was built with and leaves out the nodes that do not answer. It is an
    /// error only when no node answers at all.
    ///
    /// # Errors
    ///
    /// An embedded client reports its own node and never fails. A remote client answers
    /// [`Error::Token`] when the client has no API secret, [`Error::Connect`] when the node
    /// does not answer, [`Error::Bincode`] for an answer that cannot be deserialized, and the
    /// [`Error`] the node reports back. Nodes that do not answer at all are skipped, so only a
    /// cluster where nobody answers is an error.
    pub async fn cluster_election_state(&self) -> Result<BTreeMap<NodeId, ElectionState>, Error> {
        let mut map = BTreeMap::new();
        for (_, state) in self.election_states().await? {
            map.insert(state.node_id(), state);
        }
        Ok(map)
    }

    /// Enable or disable a node's own right to start elections, for every Raft group it runs.
    ///
    /// `openraft` keeps this in its runtime config, so the change is in memory only: a restart
    /// applies the `election_disable` marker of that node's own node line again. An embedded client
    /// can change its own node only.
    ///
    /// # Errors
    ///
    /// [`Error::Error`] when an embedded client asks for a node that is not its own, and the
    /// [`Error`] of the change itself, locally from `openraft` and remotely as [`Error::Token`]
    /// when the client has no API secret, [`Error::Connect`] when the node does not answer,
    /// [`Error::Bincode`] for an answer that cannot be deserialized, and the [`Error`] the node
    /// reports back.
    pub async fn set_node_election(
        &self,
        node_id: NodeId,
        enable: bool,
    ) -> Result<ElectionState, Error> {
        if let Some(state) = &self.inner.state {
            if node_id != state.id {
                return Err(Error::Error(
                    "An embedded client can change the election state of its own node only".into(),
                ));
            }
            return management::local_set_enable_elect(state, enable).await;
        }

        let body = serialize(&ElectionReq {
            enable_elect: enable,
        })?;
        self.node_request(node_id, Method::PUT, "", Some(body))
            .await
    }

    /// Make a node campaign for the leadership of all its Raft groups.
    ///
    /// openraft runs a Pre-Vote round first, so the term is untouched when the node cannot win: a
    /// live leader holding its lease is refused, and so is a node whose own elections are disabled.
    /// The campaign is therefore a request a healthy cluster may refuse, and it is safe as a probe.
    /// A learner is refused, and an embedded client can elect its own node only.
    ///
    /// There is no way to bypass the Pre-Vote. When a leader change must happen, use
    /// [`Self::transfer_leadership`], which asks the node that currently holds the seat to hand it
    /// over; that is the graceful path, and it is the only one.
    ///
    /// # Errors
    ///
    /// [`Error::Error`] when an embedded client asks another node to campaign, and the
    /// [`Error`] of the campaign itself, locally from `openraft` and remotely as
    /// [`Error::Token`] when the client has no API secret, [`Error::Connect`] when the node
    /// does not answer, [`Error::Bincode`] for an answer that cannot be deserialized, and the
    /// [`Error`] the node reports back.
    pub async fn elect_node(&self, node_id: NodeId) -> Result<ElectionState, Error> {
        if let Some(state) = &self.inner.state {
            if node_id != state.id {
                return Err(Error::Error(
                    "An embedded client can elect its own node only".into(),
                ));
            }
            return management::local_elect(state).await;
        }

        self.node_request(node_id, Method::POST, "", None).await
    }

    /// The membership of every cluster node, keyed by node id.
    ///
    /// It answers who is voter and who is learner in each Raft group, and which node each group
    /// follows. Like `cluster_election_state()`: an embedded client reports its own node, a remote
    /// client asks every node address it was built with and leaves out the nodes that do not answer.
    ///
    /// # Errors
    ///
    /// An embedded client reports its own node and never fails. A remote client answers
    /// [`Error::Token`] when the client has no API secret, [`Error::Connect`] when the node
    /// does not answer, [`Error::Bincode`] for an answer that cannot be deserialized, and the
    /// [`Error`] the node reports back. Nodes that do not answer at all are skipped, so only a
    /// cluster where nobody answers is an error.
    pub async fn cluster_membership(&self) -> Result<BTreeMap<NodeId, MembersState>, Error> {
        let mut map = BTreeMap::new();
        for (_, state) in self.members_states().await? {
            map.insert(state.node_id(), state);
        }
        Ok(map)
    }

    /// Make a learner a voter in every Raft group of the cluster.
    ///
    /// The node must already be a learner - this only moves it into the voter set. It runs on the
    /// node that leads the cluster: a remote client sends it to its known leader and follows a
    /// leader redirect, an embedded client runs it locally and fails with the redirect payload when
    /// its own node is not the leader.
    ///
    /// # Errors
    ///
    /// The [`Error`] of the change: on an embedded client a [`Error::ClientWriteErrorDb`] style
    /// redirect when its own node is not the leader, on a remote client [`Error::Token`] when
    /// the client has no API secret, [`Error::Connect`] when the node does not answer,
    /// [`Error::Bincode`] for an answer that cannot be deserialized, and the [`Error`] the node
    /// reports back, and [`Error::Connect`] when the leader keeps changing during all three
    /// attempts.
    pub async fn promote_node(&self, node_id: NodeId) -> Result<MembersState, Error> {
        if let Some(state) = &self.inner.state {
            return management::local_promote(state, node_id).await;
        }

        let body = serialize(&NodeReq { node_id })?;
        self.leader_request(Method::POST, "/cluster/promote", body)
            .await
    }

    /// Take a voter out of the voter set of every Raft group of the cluster.
    ///
    /// With `stay_as_learner` the node keeps replicating as a learner, which is what a master-slave
    /// setup wants. Without it the node leaves the cluster entirely, and `openraft`'s step-down
    /// hands the leadership to the most up-to-date remaining voter, so demoting the leader this way
    /// is the switchover.
    ///
    /// A leader that would demote itself to a learner is refused: it keeps leading, and openraft
    /// cannot complete the change. Hand the leadership over with
    /// [`transfer_leadership`](Self::transfer_leadership) first and run this call on the node that
    /// took it.
    ///
    /// # Errors
    ///
    /// The [`Error`] of the change: on an embedded client a [`Error::ClientWriteErrorDb`] style
    /// redirect when its own node is not the leader, on a remote client [`Error::Token`] when
    /// the client has no API secret, [`Error::Connect`] when the node does not answer,
    /// [`Error::Bincode`] for an answer that cannot be deserialized, and the [`Error`] the node
    /// reports back, and [`Error::Connect`] when the leader keeps changing during all three
    /// attempts.
    pub async fn demote_node(
        &self,
        node_id: NodeId,
        stay_as_learner: bool,
    ) -> Result<MembersState, Error> {
        if let Some(state) = &self.inner.state {
            return management::local_demote(state, node_id, stay_as_learner).await;
        }

        let body = serialize(&ClusterLeaveReq {
            node_id,
            stay_as_learner,
        })?;
        self.leader_request(Method::DELETE, "/cluster/membership", body)
            .await
    }

    /// Hand the leadership of every Raft group to a specific node.
    ///
    /// A transfer is a request, not a command: a target that is unreachable, or whose log is
    /// behind, ignores it, and the answer then shows the leadership where it was. A learner can
    /// never take over and is refused. It must run on the current leader, like `promote_node()`.
    ///
    /// # Errors
    ///
    /// The [`Error`] of the change: on an embedded client a [`Error::ClientWriteErrorDb`] style
    /// redirect when its own node is not the leader, on a remote client [`Error::Token`] when
    /// the client has no API secret, [`Error::Connect`] when the node does not answer,
    /// [`Error::Bincode`] for an answer that cannot be deserialized, and the [`Error`] the node
    /// reports back, and [`Error::Connect`] when the leader keeps changing during all three
    /// attempts.
    pub async fn transfer_leadership(&self, node_id: NodeId) -> Result<MembersState, Error> {
        if let Some(state) = &self.inner.state {
            return management::local_transfer_leader(state, node_id).await;
        }

        let body = serialize(&NodeReq { node_id })?;
        self.leader_request(Method::POST, "/cluster/transfer_leader", body)
            .await
    }

    /// Send a `/cluster` request to the node that leads the cluster, following one leader redirect
    /// per attempt, because a leader change between our look and our call is normal.
    async fn leader_request(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<MembersState, Error> {
        let scheme = if self.inner.tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        let secret = self
            .inner
            .api_secret
            .as_ref()
            .ok_or_else(|| Error::Token("API Secret missing".into()))?;
        let client = self
            .inner
            .client
            .as_ref()
            .ok_or_else(|| Error::Connect("Client has no http client".into()))?;

        #[cfg(feature = "sqlite")]
        let (leader, lookup, tx) = (
            &self.inner.leader_db,
            &self.inner.leader_lookup_db,
            &self.inner.tx_client_db,
        );
        #[cfg(not(feature = "sqlite"))]
        let (leader, lookup, tx) = (
            &self.inner.leader_cache,
            &self.inner.leader_lookup_cache,
            &self.inner.tx_client_cache,
        );

        for attempt in 0..3 {
            let addr = leader.read().await.1.clone();
            let url = format!("{scheme}://{addr}{path}");
            let res = client
                .request(method.clone(), &url)
                .bearer_auth(secret)
                .body(body.clone())
                .send()
                .await?;

            if res.status().is_success() {
                let bytes = res.bytes().await?;
                return Ok(deserialize(bytes.as_ref())?);
            }

            let err = res.json::<Error>().await?;
            if !self.was_leader_update_error(&err, leader, lookup, tx).await {
                return Err(err);
            }
            debug!("Node {addr} is not the leader any more - retrying, attempt {attempt}");
        }

        Err(Error::Connect(
            "The cluster leader kept changing during the membership request".into(),
        ))
    }

    /// The election state of every node we know about, as `(api address, state)` pairs.
    async fn election_states(&self) -> Result<Vec<(String, ElectionState)>, Error> {
        self.get_from_every_node("/cluster/election", |state| {
            management::election_state(state)
        })
        .await
    }

    /// The membership of every node we know about, as `(api address, state)` pairs.
    async fn members_states(&self) -> Result<Vec<(String, MembersState)>, Error> {
        self.get_from_every_node("/cluster/membership", |state| {
            management::members_state(state)
        })
        .await
    }

    /// `GET` one `/cluster` path on every node we know, answering `(api address, payload)` pairs.
    /// A node that does not answer is skipped with an `error` log; the call is an error only when
    /// no node answers at all, so a half-reachable cluster gives a partial answer.
    async fn get_from_every_node<T>(
        &self,
        path: &str,
        local: impl Fn(&Arc<AppState>) -> Result<T, Error>,
    ) -> Result<Vec<(String, T)>, Error>
    where
        T: bincode_next::Decode<()>,
    {
        if let Some(state) = &self.inner.state {
            return Ok(vec![(String::new(), local(state)?)]);
        }

        let scheme = if self.inner.tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        let secret = self
            .inner
            .api_secret
            .as_ref()
            .ok_or_else(|| Error::Token("API Secret missing".into()))?;
        let client = self
            .inner
            .client
            .as_ref()
            .ok_or_else(|| Error::Connect("Client has no http client".into()))?;

        let mut out = Vec::new();
        for addr in &self.inner.nodes {
            let url = format!("{scheme}://{addr}{path}");
            let res = client.get(&url).bearer_auth(secret).send().await;
            match res {
                Ok(res) if res.status().is_success() => {
                    let bytes = res.bytes().await?;
                    out.push((addr.clone(), deserialize(bytes.as_ref())?));
                }
                Ok(res) => {
                    let err = res.json::<Error>().await?;
                    error!("Error getting {path} from node {addr}: {err}");
                }
                Err(err) => {
                    error!("Error getting {path} from node {addr}: {err}");
                }
            }
        }

        if out.is_empty() {
            return Err(Error::Connect(format!(
                "No cluster node answered its {path} state"
            )));
        }
        Ok(out)
    }

    /// Send one of the `/cluster/election` requests to a single node, found by its node id.
    async fn node_request(
        &self,
        node_id: NodeId,
        method: Method,
        query: &str,
        body: Option<Vec<u8>>,
    ) -> Result<ElectionState, Error> {
        let mut addr = None;
        for (node_addr, state) in self.election_states().await? {
            if state.node_id() == node_id {
                addr = Some(node_addr);
                break;
            }
        }
        let addr = addr.ok_or_else(|| {
            Error::Connect(format!(
                "Node {node_id} did not answer with its election state"
            ))
        })?;

        let scheme = if self.inner.tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        let secret = self
            .inner
            .api_secret
            .as_ref()
            .ok_or_else(|| Error::Token("API Secret missing".into()))?;
        let client = self
            .inner
            .client
            .as_ref()
            .ok_or_else(|| Error::Connect("Client has no http client".into()))?;

        let url = format!("{scheme}://{addr}/cluster/election{query}");
        let mut req = client.request(method, &url).bearer_auth(secret);
        if let Some(body) = body {
            req = req.body(body);
        }

        let res = req.send().await?;
        if res.status().is_success() {
            let bytes = res.bytes().await?;
            Ok(deserialize(bytes.as_ref())?)
        } else {
            Err(res.json::<Error>().await?)
        }
    }

    /// Check the cluster health state for the database Raft.
    ///
    /// # Errors
    ///
    /// Everything [`Self::metrics_db`] can return, plus the [`Error::RaftErrorDb`] of a running
    /// state that is not ready, [`Error::Connect`] while the leader voting of the group has not
    /// finished, and [`Error::LeaderChange`] when it has no leader at all.
    #[cfg(feature = "sqlite")]
    pub async fn is_healthy_db(&self) -> Result<(), Error> {
        let metrics = self.metrics_db().await?;
        metrics.running_state?;
        if metrics.current_leader.is_some() {
            if metrics.state == ServerState::Learner
                || metrics.state == ServerState::Follower
                || metrics.state == ServerState::Leader
            {
                Ok(())
            } else {
                Err(Error::Connect(format!(
                    "The DB leader voting process has not finished yet - server state: {:?}",
                    metrics.state
                )))
            }
        } else {
            // tracing::error!("Unhealthy DB");
            Err(Error::LeaderChange(
                "The DB leader voting process has not finished yet".into(),
            ))
        }
    }

    /// Check the cluster health state for the cache Raft.
    ///
    /// # Errors
    ///
    /// Everything [`Self::metrics_cache`] can return, plus the [`Error::RaftErrorCache`] of a
    /// running state that is not ready, [`Error::Connect`] while the leader voting of the group
    /// has not finished, and [`Error::LeaderChange`] when it has no leader at all.
    #[cfg(feature = "cache")]
    pub async fn is_healthy_cache(&self) -> Result<(), Error> {
        let metrics = self.metrics_cache().await?;
        metrics.running_state?;
        if metrics.current_leader.is_some() {
            if metrics.state == ServerState::Learner
                || metrics.state == ServerState::Follower
                || metrics.state == ServerState::Leader
            {
                Ok(())
            } else {
                Err(Error::Connect(format!(
                    "The cache leader voting process has not finished yet - server state: {:?}",
                    metrics.state
                )))
            }
        } else {
            // tracing::error!("Unhealthy cache");
            Err(Error::LeaderChange(
                "The cache leader voting process has not finished yet".into(),
            ))
        }
    }

    /// Wait until the database Raft is healthy.
    #[cfg(feature = "sqlite")]
    pub async fn wait_until_healthy_db(&self) {
        loop {
            match self.is_healthy_db().await {
                Ok(()) => {
                    return;
                }
                Err(err) => {
                    debug!("Waiting for healthy Raft DB: {:?}", err);
                    // tracing::warn!("Waiting for healthy Raft DB");
                    info!("Waiting for healthy Raft DB");
                    time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Wait until the database Raft is healthy.
    ///
    /// # Errors
    ///
    /// [`Error::Timeout`] only, when the database Raft is not healthy within `timeout`. The
    /// health errors in between are logged and retried, never returned.
    #[cfg(feature = "sqlite")]
    pub async fn wait_until_healthy_db_timeout(&self, timeout: Duration) -> Result<(), Error> {
        tokio::time::timeout(timeout, self.wait_until_healthy_db())
            .await
            .map_err(|_| {
                Error::Timeout(format!(
                    "Waiting for DB to become healthy timed out after {timeout:?}"
                ))
            })
    }

    /// Wait until the cache Raft is healthy.
    #[cfg(feature = "cache")]
    pub async fn wait_until_healthy_cache(&self) {
        loop {
            match self.is_healthy_cache().await {
                Ok(()) => {
                    return;
                }
                Err(err) => {
                    debug!("Waiting for healthy Raft cache: {:?}", err);
                    info!("Waiting for healthy Raft cache");
                    time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Wait until the cache Raft is healthy.
    ///
    /// # Errors
    ///
    /// [`Error::Timeout`] only, when the cache Raft is not healthy within `timeout`. The health
    /// errors in between are logged and retried, never returned.
    #[cfg(feature = "cache")]
    pub async fn wait_until_healthy_cache_timeout(&self, timeout: Duration) -> Result<(), Error> {
        tokio::time::timeout(timeout, self.wait_until_healthy_cache())
            .await
            .map_err(|_| {
                Error::Timeout(format!(
                    "Waiting for Cache to become healthy timed out after {timeout:?}"
                ))
            })
    }

    /// Wait until this node is in sync with the rest of the cluster.
    /// Works on local clients only; for remote clients it waits forever, because a remote client
    /// cannot know how far its server-side node has caught up. A single instance returns right
    /// away, as it is always in sync with itself.
    ///
    /// This is meant to be called after `start_node()` returned and before serving traffic. It
    /// runs in two phases:
    ///
    /// 1. Re-apply the log this node found on its own disk, up to the startup replay target it
    ///    computed from its own log store at startup. This phase needs no peer at all, so a node
    ///    whose cluster is unreachable completes it and only blocks on phase two.
    /// 2. Follow the cluster tip. "Close enough" means within `CLUSTER_SYNC_ALLOWED_DIFF` log
    ///    entries of the leader's last log index - followers are normally a bit behind the
    ///    leader, so we never require an exact index match.
    pub async fn wait_for_cluster_sync(&self) {
        // Fast path: a single instance will always be in sync with itself.
        // The node list only belongs to a remote client, so a local client takes the member count
        // from its own Raft membership, like `shutdown_execute` does.
        #[allow(unused_mut)]
        let mut single_instance = true;

        if let Some(state) = &self.inner.state {
            #[cfg(feature = "sqlite")]
            {
                let members = state
                    .raft_db
                    .raft
                    .metrics()
                    .borrow_watched()
                    .membership_config
                    .nodes()
                    .count();
                single_instance &= members == 1;
            }

            #[cfg(feature = "cache")]
            {
                let members = state
                    .raft_cache
                    .raft
                    .metrics()
                    .borrow_watched()
                    .membership_config
                    .nodes()
                    .count();
                single_instance &= members == 1;
            }
        } else {
            single_instance = self.inner.nodes.len() == 1;
        }

        // Phase one on every topology: re-applying the log on this node's own disk needs no peer,
        // so a node whose cluster is unreachable finishes this and only blocks on phase two.
        self.wait_for_startup_replay().await;

        if single_instance {
            return;
        }

        loop {
            let mut in_sync = true;

            #[cfg(feature = "sqlite")]
            if let Err(err) = self.is_in_sync_db().await {
                in_sync = false;
                debug!("Waiting for cluster sync (DB): {err:?}");
                info!("Waiting for cluster sync (DB)");
            }

            #[cfg(feature = "cache")]
            if let Err(err) = self.is_in_sync_cache().await {
                in_sync = false;
                debug!("Waiting for cluster sync (Cache): {err:?}");
                info!("Waiting for cluster sync (Cache)");
            }

            if in_sync {
                info!("This node is now in sync with the cluster");
                return;
            }

            time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Waits until this node has re-applied the log it found on its own disk at startup.
    ///
    /// The target is read from the local log store, so this never talks to a peer and never
    /// needs a leader.
    async fn wait_for_startup_replay(&self) {
        let Some(state) = &self.inner.state else {
            return;
        };

        loop {
            #[allow(unused_mut)]
            let mut pending = false;

            #[cfg(feature = "sqlite")]
            if let Err(err) = state.raft_db.check_replay() {
                pending = true;
                debug!("{err}");
            }

            #[cfg(feature = "cache")]
            if let Err(err) = state.raft_cache.check_replay() {
                pending = true;
                debug!("{err}");
            }

            if !pending {
                return;
            }

            info!("Waiting for this node to re-apply its log after startup");
            time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Same as `wait_for_cluster_sync()`, but returns a timeout error after `timeout`.
    ///
    /// # Errors
    ///
    /// [`Error::Error`] for a remote client - only a node that runs its own Raft can measure
    /// its sync gap - and [`Error::Timeout`] when the node is not within the allowed log gap of
    /// the cluster tip after `timeout`.
    pub async fn wait_for_cluster_sync_timeout(&self, timeout: Duration) -> Result<(), Error> {
        if self.inner.state.is_none() {
            return Err(Error::Error(
                "Cluster sync check works on local clients only".into(),
            ));
        }

        tokio::time::timeout(timeout, self.wait_for_cluster_sync())
            .await
            .map_err(|_| {
                Error::Timeout(format!(
                    "Waiting for the cluster to be in sync timed out after {timeout:?}"
                ))
            })
    }

    /// Check that this node's own metrics show it leading a group with a live quorum.
    ///
    /// A node that believes it leads is not enough: a leader that is cut off from a quorum keeps
    /// believing it, and it answers a write with a `ForwardToLeader` that names no target at all.
    /// The proof is a quorum acknowledgement inside one election timeout, which is the lease the
    /// leader holds its leadership by.
    ///
    /// This check only works on the node that recorded the acknowledgement. openraft's
    /// `last_quorum_acked` is an instant on that node's own monotonic clock, and an instant read on
    /// a different node has no meaning, so the age can only be taken here.
    fn assert_leading_here<C>(
        rtype: &RaftType,
        our_id: NodeId,
        metrics: &RaftMetrics<C>,
        lease: Duration,
    ) -> Result<(), Error>
    where
        C: RaftTypeConfig<NodeId = u64>,
    {
        if metrics.state != ServerState::Leader || metrics.current_leader != Some(our_id) {
            return Err(Error::LeaderChange(
                format!(
                    "Node {our_id} is not the {} leader it believes it is - it reports state \
                    {:?} and leader {:?}",
                    rtype.as_str(),
                    metrics.state,
                    metrics.current_leader
                )
                .into(),
            ));
        }

        match metrics.last_quorum_acked.as_ref() {
            None => Err(Error::LeaderChange(
                format!(
                    "Node {our_id} leads the {} but no quorum has acknowledged it yet",
                    rtype.as_str()
                )
                .into(),
            )),
            Some(acked) => {
                let age = acked.elapsed();
                if age > lease {
                    Err(Error::LeaderChange(
                        format!(
                            "Node {our_id} leads the {} but its last quorum acknowledgement is \
                            {age:?} old, past its lease of {lease:?}",
                            rtype.as_str()
                        )
                        .into(),
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Check that the node a client would write to confirms that it leads the group.
    ///
    /// A node inside an election reports a leader it does not know, and a demoted node still has
    /// the old leadership in its logs, so the answer has to name itself as the leader of the group
    /// it is answering about.
    ///
    /// The liveness of a remote leader is proven by the request that fetched this answer: it is a
    /// live round trip to that node, taken now. Its quorum acknowledgement cannot be checked here,
    /// because `last_quorum_acked` is an instant on the leader's own monotonic clock.
    fn assert_leader_confirms<C>(
        rtype: &RaftType,
        leader_id: NodeId,
        metrics: &RaftMetrics<C>,
    ) -> Result<(), Error>
    where
        C: RaftTypeConfig<NodeId = u64>,
    {
        if metrics.state != ServerState::Leader || metrics.current_leader != Some(leader_id) {
            return Err(Error::LeaderChange(
                format!(
                    "The {} leader {leader_id} does not confirm its leadership - it reports \
                    state {:?} and leader {:?}",
                    rtype.as_str(),
                    metrics.state,
                    metrics.current_leader
                )
                .into(),
            ));
        }
        Ok(())
    }

    /// Check if this node's database state machine is in sync with the cluster.
    /// Works on local clients only.
    #[cfg(feature = "sqlite")]
    async fn is_in_sync_db(&self) -> Result<(), Error> {
        let state =
            self.inner.state.as_ref().ok_or_else(|| {
                Error::Error("Cluster sync check works on local clients only".into())
            })?;

        let our_metrics = state.raft_db.raft.metrics().borrow_watched().clone();
        our_metrics.running_state.clone()?;

        // Our own state machine progress - what is actually servable to queries right now.
        let applied = our_metrics.last_applied.map_or(0, |l| l.index);
        let own_last_log_index = our_metrics.last_log_index.unwrap_or(0);

        // A leader holds its leadership for one election timeout without a quorum acknowledgement.
        let lease = Duration::from_millis(state.raft_db.raft.config().election_timeout_min);

        // `find_set_leader` takes the metrics, so the check on our own view of the leadership runs
        // before it. A node that sees itself as the leader has to actually be one, or nothing in
        // front of it is writable.
        let our_leader = our_metrics.current_leader;
        if our_leader == Some(state.id) {
            Self::assert_leading_here(&RaftType::Sqlite, state.id, &our_metrics, lease)?;
        }

        // Keep the cached leader address up to date and fail loud on stale metrics, like
        // `find_set_active_leader` does at startup.
        Self::find_set_leader(our_metrics, &self.inner.leader_db).await?;
        let leader_id = self.inner.leader_db.read().await.0;

        // The cluster tip is the leader's last log index. Followers are normally a bit behind
        // it, so we compare against `CLUSTER_SYNC_ALLOWED_DIFF` instead of requiring an exact
        // match.
        let tip = if state.id == leader_id {
            // We are the leader: our own log is the cluster tip.
            own_last_log_index
        } else {
            let url = self
                .build_addr("/cluster/metrics/sqlite", &self.inner.leader_db)
                .await;
            let leader_metrics = self.get_metrics_local::<TypeConfigSqlite>(url).await?;
            Self::assert_leader_confirms(&RaftType::Sqlite, leader_id, &leader_metrics)?;
            leader_metrics.last_log_index.unwrap_or(0)
        };

        if tip.saturating_sub(applied) <= CLUSTER_SYNC_ALLOWED_DIFF {
            Ok(())
        } else {
            Err(Error::Connect(format!(
                "DB state machine is {} log entries behind the cluster tip (allowed: {CLUSTER_SYNC_ALLOWED_DIFF})",
                tip - applied
            )))
        }
    }

    /// Check if this node's cache state machine is in sync with the cluster.
    /// Works on local clients only.
    #[cfg(feature = "cache")]
    async fn is_in_sync_cache(&self) -> Result<(), Error> {
        let state =
            self.inner.state.as_ref().ok_or_else(|| {
                Error::Error("Cluster sync check works on local clients only".into())
            })?;

        let our_metrics = state.raft_cache.raft.metrics().borrow_watched().clone();
        our_metrics.running_state.clone()?;

        // Our own state machine progress - what is actually servable to queries right now.
        let applied = our_metrics.last_applied.map_or(0, |l| l.index);
        let own_last_log_index = our_metrics.last_log_index.unwrap_or(0);

        // A leader holds its leadership for one election timeout without a quorum acknowledgement.
        let lease = Duration::from_millis(state.raft_cache.raft.config().election_timeout_min);

        // `find_set_leader` takes the metrics, so the check on our own view of the leadership runs
        // before it. A node that sees itself as the leader has to actually be one, or nothing in
        // front of it is writable.
        let our_leader = our_metrics.current_leader;
        if our_leader == Some(state.id) {
            Self::assert_leading_here(&RaftType::Cache, state.id, &our_metrics, lease)?;
        }

        // Keep the cached leader address up to date and fail loud on stale metrics, like
        // `find_set_active_leader` does at startup.
        Self::find_set_leader(our_metrics, &self.inner.leader_cache).await?;
        let leader_id = self.inner.leader_cache.read().await.0;

        // The cluster tip is the leader's last log index. Followers are normally a bit behind
        // it, so we compare against `CLUSTER_SYNC_ALLOWED_DIFF` instead of requiring an exact
        // match.
        let tip = if state.id == leader_id {
            // We are the leader: our own log is the cluster tip.
            own_last_log_index
        } else {
            let url = self
                .build_addr("/cluster/metrics/cache", &self.inner.leader_cache)
                .await;
            let leader_metrics = self.get_metrics_local::<TypeConfigCache>(url).await?;
            Self::assert_leader_confirms(&RaftType::Cache, leader_id, &leader_metrics)?;
            leader_metrics.last_log_index.unwrap_or(0)
        };

        if tip.saturating_sub(applied) <= CLUSTER_SYNC_ALLOWED_DIFF {
            Ok(())
        } else {
            Err(Error::Connect(format!(
                "Cache state machine is {} log entries behind the cluster tip (allowed: {CLUSTER_SYNC_ALLOWED_DIFF})",
                tip - applied
            )))
        }
    }

    // Fetch metrics from a node's API server over HTTP. Local clients use this to query the
    // current leader's own metrics (they have no stored HTTP client or API secret, so we build
    // them on demand here).
    async fn get_metrics_local<C>(&self, url: String) -> Result<RaftMetrics<C>, Error>
    where
        C: RaftTypeConfig,
    {
        let state = self
            .inner
            .state
            .as_ref()
            .expect("get_metrics_local should only be called with local state");

        let client = crate::http_client::build_http_client(self.inner.tls_no_verify);
        let res = client
            .get(url)
            .bearer_auth(&state.secret_api)
            .send()
            .await?;

        if res.status().is_success() {
            let bytes = res.bytes().await?;
            Ok(deserialize_serde(bytes.as_ref())?)
        } else {
            let err = res.json::<Error>().await?;
            Err(err)
        }
    }

    /// Perform a graceful shutdown for this Raft node.
    /// Works on local clients only and can't shut down remote nodes.
    ///
    /// The shutdown adds a 10 delay on purpose for smoothing out Kubernetes rolling releases and
    /// make the whole process more graceful, because a whole new leader election might be necessary.
    ///
    /// In future versions, there will be the possibility to trigger a graceful leader election
    /// upfront, but this has not been stabilized in this version.
    ///
    /// # Errors
    ///
    /// [`Error::Error`] for a remote client, because only a node that runs its own Raft can be
    /// shut down this way, and [`Error::Error`] when the shutdown does not finish within
    /// `NodeConfig::shutdown_delay` plus [`SHUTDOWN_TEARDOWN_BUDGET`](crate::SHUTDOWN_TEARDOWN_BUDGET)
    /// (20 s with the default delay). Finish or drop any open query streams before shutting down.
    /// Teardown continues if this call times out or is cancelled.
    pub async fn shutdown(&self) -> Result<(), Error> {
        let (Some(state), Some(tx)) = (&self.inner.state, &self.inner.tx_shutdown) else {
            return Err(Error::Error(
                "Shutdown for remote Raft clients is not yet implemented".into(),
            ));
        };

        if !state.is_shutting_down.swap(true, Ordering::AcqRel) {
            let state = state.clone();
            let tx = tx.clone();
            #[cfg(feature = "cache")]
            let with_tls = self.inner.tls_config.is_some();
            #[cfg(feature = "cache")]
            let tls_no_verify = self.inner.tls_no_verify;
            let shutdown = tokio::spawn(async move {
                Self::shutdown_execute(
                    &state,
                    #[cfg(feature = "cache")]
                    with_tls,
                    #[cfg(feature = "cache")]
                    tls_no_verify,
                )
                .await
            });
            tokio::spawn(async move {
                let result = shutdown
                    .await
                    .map_err(|err| format!("Raft shutdown task did not finish: {err}"))
                    .and_then(|result| result.map_err(|err| err.to_string()));
                let _ = tx.send_replace(Some(result));
            });
        }

        let mut finished = tx.subscribe();
        time::timeout(
            state.shutdown_delay + crate::config::SHUTDOWN_TEARDOWN_BUDGET,
            async {
                if let Some(Err(err)) = finished
                    .wait_for(Option::is_some)
                    .await
                    .map_err(|_| Error::new("Raft shutdown status channel closed"))?
                    .as_ref()
                {
                    return Err(Error::new(err.clone()));
                }
                // The shutdown signal starts graceful server shutdown. Wait for both serving
                // tasks to leave so an in-process restart can bind their addresses again.
                let _ = state.servers_closed.recv_async().await;
                Ok(())
            },
        )
        .await
        .map_err(|_| Error::new("Timeout reached while shutting down Raft"))?
    }

    #[allow(unused_assignments)]
    async fn shutdown_execute(
        state: &Arc<AppState>,
        #[cfg(feature = "cache")] with_tls: bool,
        #[cfg(feature = "cache")] tls_no_verify: bool,
    ) -> Result<(), Error> {
        info!("Starting Node shutdown");

        #[allow(unused_mut)]
        let mut is_single_instance: bool;
        #[cfg(feature = "cache")]
        {
            let node_count = state
                .raft_cache
                .raft
                .metrics()
                .borrow_watched()
                .membership_config
                .nodes()
                .count();
            is_single_instance = node_count == 1;
        }
        #[cfg(feature = "sqlite")]
        {
            let node_count = state
                .raft_db
                .raft
                .metrics()
                .borrow_watched()
                .membership_config
                .nodes()
                .count();
            is_single_instance = node_count == 1;
        }

        // This pre-shutdown delay is not strictly necessary, but it makes rolling releases
        // smoother, especially with ephemeral storage. `/ready` answers 503 from here on, so with
        // the default of 9.5 s a ready check interval of 3 seconds still catches it before the
        // shutdown actually starts, and services stop sending requests to this node.
        if !is_single_instance && !state.shutdown_delay.is_zero() {
            time::sleep(state.shutdown_delay).await;
        }

        // The handovers of this shutdown share one budget, so a build with both groups doesn't stall
        // for two election timeouts. The group that goes first still gets its whole timeout.
        #[cfg(any(feature = "sqlite", feature = "cache"))]
        let transfer_deadline = shutdown_transfer_deadline(state);
        let mut result = Ok(());

        #[cfg(feature = "cache")]
        {
            let mut metrics = state.raft_cache.raft.metrics().borrow_watched().clone();

            for _ in 0..5 {
                if metrics.current_leader.is_some() {
                    break;
                }
                info!("Delaying cache cluster leave because of no existing leader");
                time::sleep(Duration::from_secs(1)).await;
                metrics = state.raft_cache.raft.metrics().borrow_watched().clone();
            }

            if !state.raft_cache.cache_storage_disk {
                // If we run an entirely in-memory cache and therefore lose the Raft state
                // and membership between restarts, we should always leave the cluster cleanly
                // before doing a shutdown.
                info!("Leaving in-memory-only cache cluster");

                let client = crate::http_client::build_http_client(tls_no_verify);
                let scheme = if with_tls { "https" } else { "http" };

                if metrics.current_leader == Some(state.id) {
                    if let Err(err) = management::leave_cluster_exec(
                        state,
                        &crate::app_state::RaftType::Cache,
                        ClusterLeaveReq {
                            node_id: state.id,
                            stay_as_learner: false,
                        },
                    )
                    .await
                    {
                        tracing::error!("Error leaving the Cache cluster: {:?}", err);
                    }
                } else if let Err(err) = crate::init::leave_remote_cluster(
                    state,
                    &crate::app_state::RaftType::Cache,
                    &client,
                    scheme,
                    state.id,
                    &state.nodes,
                    0,
                    false,
                )
                .await
                {
                    tracing::error!("Error leaving the Cache cluster: {:?}", err);
                }

                info!("Left in-memory-only cache cluster successfully");
            }

            info!("Shutting down raft cache layer");

            transfer_leadership_before_shutdown(
                state,
                &crate::app_state::RaftType::Cache,
                transfer_deadline,
            )
            .await;

            state
                .raft_cache
                .is_raft_stopped
                .store(true, Ordering::Relaxed);
            result = result.and(state.raft_cache.raft.shutdown().await.map_err(Error::from));
            if let Some(handle) = &state.raft_cache.shutdown_handle {
                result = result.and(handle.shutdown().await.map_err(Error::from));
            }
        };

        #[cfg(feature = "sqlite")]
        {
            info!("Shutting down raft sqlite layer");

            for _ in 0..5 {
                if state
                    .raft_db
                    .raft
                    .metrics()
                    .borrow_watched()
                    .current_leader
                    .is_some()
                {
                    break;
                }
                info!("Delaying sqlite raft shutdown because of no existing leader");
                time::sleep(Duration::from_secs(1)).await;
            }

            transfer_leadership_before_shutdown(
                state,
                &crate::app_state::RaftType::Sqlite,
                transfer_deadline,
            )
            .await;

            state.raft_db.is_raft_stopped.store(true, Ordering::Relaxed);

            result = result.and(state.raft_db.raft.shutdown().await.map_err(Error::from));
            info!("Shutting down sqlite logs writer");
            result = result.and(
                state
                    .raft_db
                    .shutdown_handle
                    .shutdown()
                    .await
                    .map_err(Error::from),
            );

            info!("Shutting down sqlite writer");
            let writer = async {
                let (tx_sm, rx_sm) = tokio::sync::oneshot::channel();
                state
                    .raft_db
                    .sql_writer
                    .send_async(WriterRequest::Shutdown(tx_sm))
                    .await
                    .map_err(|_| Error::Sqlite("SQLite writer stopped before shutdown".into()))?;
                rx_sm
                    .await
                    .map_err(|_| Error::Sqlite("SQLite writer did not complete shutdown".into()))?
            }
            .await;
            result = result.and(writer);

            state.raft_db.read_actors.shutdown().await;
        }

        if result.is_ok() {
            info!("Shutdown complete");
        }
        result
    }
}
