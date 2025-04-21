use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::helpers::deserialize;
use crate::network::HEADER_NAME_SECRET;
use crate::{Client, Error};
use openraft::ServerState;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;
use tracing::{debug, info};

#[cfg(feature = "sqlite")]
use crate::store::{logs::rocksdb::ActionWrite, state_machine::sqlite::writer::WriterRequest};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use crate::{Node, NodeId};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use openraft::RaftMetrics;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use std::clone::Clone;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use std::sync::atomic::Ordering;

impl Client {
    /// Get cluster metrics for the database Raft.
    #[cfg(feature = "sqlite")]
    pub async fn metrics_db(&self) -> Result<RaftMetrics<NodeId, Node>, Error> {
        if let Some(state) = &self.inner.state {
            let metrics = state.raft_db.raft.metrics().borrow().clone();
            Ok(metrics)
        } else {
            let url = self
                .build_addr("/cluster/metrics/sqlite", &self.inner.leader_db)
                .await;
            self.get_metrics_remote(url).await
        }
    }

    /// Get cluster metrics for the cache Raft.
    #[cfg(feature = "cache")]
    pub async fn metrics_cache(&self) -> Result<RaftMetrics<NodeId, Node>, Error> {
        if let Some(state) = &self.inner.state {
            let metrics = state.raft_cache.raft.metrics().borrow().clone();
            Ok(metrics)
        } else {
            let url = self
                .build_addr("/cluster/metrics/cache", &self.inner.leader_cache)
                .await;
            self.get_metrics_remote(url).await
        }
    }

    // This is separated from the `self.send_with_retry_db()` to avoid recursion on leader unreachable
    async fn get_metrics_remote(&self, url: String) -> Result<RaftMetrics<NodeId, Node>, Error> {
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
            .header(HEADER_NAME_SECRET, self.inner.api_secret.as_ref().unwrap())
            .send()
            .await?;

        if res.status().is_success() {
            let bytes = res.bytes().await?;
            let resp = deserialize(bytes.as_ref())?;
            Ok(resp)
        } else {
            let err = res.json::<Error>().await?;
            Err(err)
        }
    }

    /// Check the cluster health state for the database Raft.
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
            Err(Error::LeaderChange(
                "The DB leader voting process has not finished yet".into(),
            ))
        }
    }

    /// Check the cluster health state for the cache Raft.
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
                Ok(_) => {
                    return;
                }
                Err(err) => {
                    debug!("Waiting for healthy Raft DB: {:?}", err);
                    info!("Waiting for healthy Raft DB");
                    time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Wait until the cache Raft is healthy.
    #[cfg(feature = "cache")]
    pub async fn wait_until_healthy_cache(&self) {
        loop {
            match self.is_healthy_cache().await {
                Ok(_) => {
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

    /// Perform a graceful shutdown for this Raft node.
    /// Works on local clients only and can't shut down remote nodes.
    ///
    /// The shutdown adds a 10 delay on purpose for smoothing out Kubernetes rolling releases and
    /// make the whole process more graceful, because a whole new leader election might be necessary.
    ///
    /// In future versions, there will be the possibility to trigger a graceful leader election
    /// upfront, but this has not been stabilized in this version.
    pub async fn shutdown(&self) -> Result<(), Error> {
        if let Some(state) = &self.inner.state {
            Self::shutdown_execute(
                state,
                #[cfg(feature = "cache")]
                &self.inner.tx_client_cache,
                #[cfg(feature = "sqlite")]
                &self.inner.tx_client_db,
                &self.inner.tx_shutdown,
            )
            .await
        } else {
            Err(Error::Error(
                "Shutdown for remote Raft clients is not yet implemented".into(),
            ))
        }
    }

    #[allow(unused_assignments)]
    #[allow(unused_variables)]
    pub(crate) async fn shutdown_execute(
        state: &Arc<AppState>,
        #[cfg(feature = "cache")] tx_client_cache: &flume::Sender<ClientStreamReq>,
        #[cfg(feature = "sqlite")] tx_client_db: &flume::Sender<ClientStreamReq>,
        tx_shutdown: &Option<watch::Sender<bool>>,
    ) -> Result<(), Error> {
        #[allow(unused_mut)]
        let mut is_single_instance: bool;

        #[cfg(feature = "cache")]
        {
            let mut metrics = state.raft_cache.raft.metrics().borrow().clone();
            while metrics.current_leader.is_none() {
                info!("No leader exists for the cache raft - waiting ...");
                time::sleep(Duration::from_millis(500)).await;
                metrics = state.raft_cache.raft.metrics().borrow().clone();
            }
            let node_count = metrics.membership_config.nodes().count();
            is_single_instance = node_count == 1;

            if !is_single_instance && metrics.current_leader.unwrap() == state.id {
                // if we are the leader, we want to try to make a graceful switch
                // before shutting down to cause less stress and hiccups
                info!("We are the cache raft leader - disabling elect and trigger it");
                state.raft_cache.raft.runtime_config().elect(false);
                state.raft_cache.raft.trigger().elect().await?;
            }

            info!("Shutting down raft cache layer");
            state.raft_cache.raft.shutdown().await?;
            state
                .raft_cache
                .is_raft_stopped
                .store(true, Ordering::Relaxed);
            state.raft_cache.raft.runtime_config().heartbeat(false);
            // no need to await any writer shutdowns, since the cache is ephemeral anyway
            // -> no cleanup or flushing necessary
        }

        #[cfg(feature = "sqlite")]
        {
            let mut metrics = state.raft_db.raft.metrics().borrow().clone();
            while metrics.current_leader.is_none() {
                info!("No leader exists for the DB raft - waiting ...");
                time::sleep(Duration::from_millis(500)).await;
                metrics = state.raft_db.raft.metrics().borrow().clone();
            }
            let node_count = metrics.membership_config.nodes().count();
            is_single_instance = node_count == 1;

            if !is_single_instance && metrics.current_leader.unwrap() == state.id {
                // if we are the leader, we want to try to make a graceful switch
                // before shutting down to cause less stress and hiccups
                info!("We are the DB raft leader - disabling elect and trigger it");
                state.raft_db.raft.runtime_config().elect(false);
                state.raft_db.raft.trigger().elect().await?;
            }

            info!("Shutting down raft sqlite layer");
            let tr = state.raft_db.raft.trigger();
            tr.elect().await?;
            match state.raft_db.raft.shutdown().await {
                Ok(_) => {
                    state.raft_db.is_raft_stopped.store(true, Ordering::Relaxed);
                    state.raft_db.raft.runtime_config().heartbeat(false);

                    let (tx_logs, rx_logs) = tokio::sync::oneshot::channel();
                    let (tx_sm, rx_sm) = tokio::sync::oneshot::channel();

                    info!("Shutting down sqlite logs writer");
                    state
                        .raft_db
                        .logs_writer
                        .send_async(ActionWrite::Shutdown(tx_logs))
                        .await
                        .expect("The logs writer to always be listening");
                    rx_logs
                        .await
                        .expect("To always get an answer from Logs writer");

                    info!("Shutting down sqlite writer");
                    state
                        .raft_db
                        .sql_writer
                        .send_async(WriterRequest::Shutdown(tx_sm))
                        .await
                        .expect("The state machine writer to always be listening");
                    rx_sm
                        .await
                        .expect("To always get an answer from SQL writer");
                }
                Err(err) => {
                    return Err(Error::Error(err.to_string().into()));
                }
            }
        }

        #[cfg(feature = "cache")]
        let _ = tx_client_cache.send_async(ClientStreamReq::Shutdown).await;
        #[cfg(feature = "sqlite")]
        let _ = tx_client_db.send_async(ClientStreamReq::Shutdown).await;

        if let Some(tx) = tx_shutdown {
            tx.send(true)
                .expect("The global Hiqlite shutdown handler to always listen");
        }

        // no need to apply the shutdown delay for a single instance (mostly used during dev)
        if !is_single_instance {
            // We need to do a short sleep only to avoid race conditions during rolling releases.
            // This also helps to make re-joins after a restart smoother in case you need to
            // replicate a bigger snapshot for an in-memory cache.
            info!("Shutting down in {} ms ...", state.shutdown_relay_millis);
            time::sleep(Duration::from_millis(state.shutdown_relay_millis as u64)).await;
        }

        info!("Shutdown complete");
        Ok(())
    }
}
