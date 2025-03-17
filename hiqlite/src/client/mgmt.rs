use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::network::HEADER_NAME_SECRET;
use crate::{Client, Error};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;
use tracing::{debug, info};

use crate::helpers::deserialize;
#[cfg(feature = "sqlite")]
use crate::store::{logs::rocksdb::ActionWrite, state_machine::sqlite::writer::WriterRequest};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use crate::{Node, NodeId};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use openraft::RaftMetrics;
use openraft::ServerState;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use std::clone::Clone;

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
            let metrics = state.raft_cache.raft.metrics().borrow().clone();
            let node_count = metrics.membership_config.nodes().count();
            is_single_instance = node_count == 1;

            info!("Shutting down raft cache layer");
            state.raft_cache.raft.shutdown().await?;
        }

        #[cfg(feature = "sqlite")]
        {
            let metrics = state.raft_db.raft.metrics().borrow().clone();
            let node_count = metrics.membership_config.nodes().count();
            is_single_instance = node_count == 1;

            info!("Shutting down raft sqlite layer");
            match state.raft_db.raft.shutdown().await {
                Ok(_) => {
                    let (tx, rx) = tokio::sync::oneshot::channel();

                    info!("Shutting down sqlite writer");
                    let _ = state
                        .raft_db
                        .sql_writer
                        .send_async(WriterRequest::Shutdown(tx))
                        .await;

                    info!("Shutting down sqlite logs writer");
                    if state
                        .raft_db
                        .logs_writer
                        .send_async(ActionWrite::Shutdown)
                        .await
                        .is_ok()
                    {
                        // this sometimes fails because of race conditions and internal drop handlers
                        // it just depends on which task is faster, but in any case the writer
                        // does a wal flush before exiting
                        rx.await.expect("To always get an answer from SQL writer");
                    }
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
            let _ = tx.send(true);
        }

        // no need to apply the shutdown delay for a single instance (mostly used during dev)
        if !is_single_instance {
            // We need to do a short sleep only to avoid race conditions during rolling releases.
            // This also helps to make re-joins after a restart smoother.
            //
            // TODO for some very weird reason, the process sometimes gets stuck during
            // this sleep await. I only saw this behavior in integration tests though, where alle nodes
            // are started from the same `tokio::test` task.
            //
            // Note: The issue is "something blocking" in `openraft` but only in some conditions that
            // don't make sense to me yet - needs further investigation until this sleep can be removed
            // safely.
            info!("Shutting down in 10 s ...");
            time::sleep(Duration::from_secs(10)).await;
        }

        info!("Shutdown complete");
        Ok(())
    }
}
