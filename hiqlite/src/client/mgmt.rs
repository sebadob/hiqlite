use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::network::HEADER_NAME_SECRET;
use crate::{Client, Error};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;
use tracing::info;

#[cfg(feature = "sqlite")]
use crate::store::{logs::rocksdb::ActionWrite, state_machine::sqlite::writer::WriterRequest};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use crate::{Node, NodeId};
#[cfg(any(feature = "sqlite", feature = "cache"))]
use openraft::RaftMetrics;
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
            let resp = bincode::deserialize(bytes.as_ref())?;
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
            Ok(())
        } else {
            Err(Error::LeaderChange(
                "The leader voting process has not finished yet".into(),
            ))
        }
    }

    /// Check the cluster health state for the cache Raft.
    #[cfg(feature = "cache")]
    pub async fn is_healthy_cache(&self) -> Result<(), Error> {
        let metrics = self.metrics_cache().await?;
        metrics.running_state?;
        if metrics.current_leader.is_some() {
            Ok(())
        } else {
            Err(Error::LeaderChange(
                "The leader voting process has not finished yet".into(),
            ))
        }
    }

    /// Wait until the database Raft is healthy.
    #[cfg(feature = "sqlite")]
    pub async fn wait_until_healthy_db(&self) {
        while self.is_healthy_db().await.is_err() {
            info!("Waiting for healthy Raft DB");
            time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Wait until the cache Raft is healthy.
    #[cfg(feature = "cache")]
    pub async fn wait_until_healthy_cache(&self) {
        while self.is_healthy_cache().await.is_err() {
            info!("Waiting for healthy Raft DB");
            time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Perform a graceful shutdown for this Raft node.
    /// Works on local clients only and can't shut down remote nodes.
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

    pub(crate) async fn shutdown_execute(
        state: &Arc<AppState>,
        #[cfg(feature = "cache")] tx_client_cache: &flume::Sender<ClientStreamReq>,
        #[cfg(feature = "sqlite")] tx_client_db: &flume::Sender<ClientStreamReq>,
        tx_shutdown: &Option<watch::Sender<bool>>,
    ) -> Result<(), Error> {
        #[cfg(feature = "cache")]
        {
            info!("Shutting down raft cache layer");
            match state.raft_cache.raft.shutdown().await {
                Ok(_) => {}
                Err(err) => {
                    return Err(Error::Error(err.to_string().into()));
                }
            }
        }

        #[cfg(feature = "sqlite")]
        {
            info!("Shutting down raft sqlite layer");
            match state.raft_db.raft.shutdown().await {
                Ok(_) => {
                    let (tx, rx) = tokio::sync::oneshot::channel();

                    info!("Shutting down sqlite writer");
                    state
                        .raft_db
                        .sql_writer
                        .send_async(WriterRequest::Shutdown(tx))
                        .await
                        .expect("SQL writer to always be running");

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

        #[cfg(feature = "sqlite")]
        let _ = tx_client_db.send_async(ClientStreamReq::Shutdown).await;
        #[cfg(feature = "cache")]
        let _ = tx_client_cache.send_async(ClientStreamReq::Shutdown).await;

        if let Some(tx) = tx_shutdown {
            tx.send(true).unwrap();
        }

        info!("Waiting 5 additional seconds before shutting down");

        // We need to do a short sleep only to avoid race conditions during rolling releases.
        // This also helps to make re-joins after a restart smoother.
        time::sleep(Duration::from_secs(5)).await;

        info!("Shutdown complete");
        Ok(())
    }
}
