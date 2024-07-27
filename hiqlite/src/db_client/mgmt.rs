use crate::app_state::AppState;
use crate::db_client::stream::ClientStreamReq;
use crate::store::logs::rocksdb::ActionWrite;
use crate::store::state_machine::sqlite::writer::WriterRequest;
use crate::{DbClient, Error, Node, NodeId};
use openraft::RaftMetrics;
use std::clone::Clone;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, watch};
use tokio::time;
use tracing::error;

impl DbClient {
    // pub async fn init(&self) -> Result<(), Error> {
    //     let url = self.build_addr("/cluster/init").await;
    //     let res = self
    //         .client
    //         .post(url)
    //         .header(HEADER_NAME_SECRET, self.api_secret())
    //         .send()
    //         .await
    //         .unwrap();
    //
    //     if res.status().is_success() {
    //         Ok(())
    //     } else {
    //         Err(res.json().await.unwrap())
    //     }
    // }

    // pub async fn add_learner(&self, req: LearnerReq) -> Result<RaftWriteResponse, Error> {
    //     self.send_with_retry("/cluster/add_learner", Some(&req))
    //         .await
    // }

    // pub async fn change_membership(
    //     &self,
    //     req: &BTreeSet<NodeId>,
    // ) -> Result<RaftWriteResponse, Error> {
    //     self.send_with_retry("/cluster/membership", Some(req)).await
    // }

    #[cfg(feature = "sqlite")]
    pub async fn metrics_db(&self) -> Result<RaftMetrics<NodeId, Node>, Error> {
        if let Some(state) = &self.state {
            let metrics = state.raft_db.raft.metrics().borrow().clone();
            Ok(metrics)
        } else {
            self.send_with_retry("/cluster/metrics", None::<String>.as_ref())
                .await
        }
    }

    #[cfg(feature = "cache")]
    pub async fn metrics_cache(&self) -> Result<RaftMetrics<NodeId, Node>, Error> {
        if let Some(state) = &self.state {
            let metrics = state.raft_cache.raft.metrics().borrow().clone();
            Ok(metrics)
        } else {
            self.send_with_retry("/cluster/metrics", None::<String>.as_ref())
                .await
        }
    }

    /// Check the Raft health state
    /// TODO different fn's for db / cache ?
    pub async fn is_healthy(&self) -> Result<(), Error> {
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

    /// TODO different fn's for db / cache ?
    pub async fn wait_until_healthy(&self) {
        while let Err(err) = self.is_healthy().await {
            error!("Waiting for cluster to become healthy: {}", err);
            time::sleep(Duration::from_millis(1000)).await;
        }
    }

    // #[must_use]
    /// Perform a graceful shutdown for this Raft node.
    /// Works on local clients only and can't shut down remote nodes.
    pub async fn shutdown(&self) -> Result<(), Error> {
        if let Some(state) = &self.state {
            Self::shutdown_execute(state, &self.tx_client, &self.tx_shutdown).await
        } else {
            Err(Error::Error(
                "Shutdown for remote Raft clients is not yet implemented".into(),
            ))
        }
    }

    pub(crate) async fn shutdown_execute(
        state: &Arc<AppState>,
        tx_client: &flume::Sender<ClientStreamReq>,
        tx_shutdown: &Option<watch::Sender<bool>>,
    ) -> Result<(), Error> {
        #[cfg(feature = "cache")]
        match state.raft_cache.raft.shutdown().await {
            Ok(_) => {}
            Err(err) => {
                return Err(Error::Error(err.to_string().into()));
            }
        }

        #[cfg(feature = "sqlite")]
        match state.raft_db.raft.shutdown().await {
            Ok(_) => {
                let (tx, rx) = oneshot::channel();

                state
                    .raft_db
                    .sql_writer
                    .send_async(WriterRequest::Shutdown(tx))
                    .await
                    .expect("SQL writer to always be running");

                let _ = state
                    .raft_db
                    .logs_writer
                    .send_async(ActionWrite::Shutdown)
                    .await;

                rx.await.expect("To always get an answer from SQL writer");
            }
            Err(err) => {
                return Err(Error::Error(err.to_string().into()));
            }
        }

        let _ = tx_client.send_async(ClientStreamReq::Shutdown).await;

        if let Some(tx) = tx_shutdown {
            tx.send(true).unwrap();
        }

        time::sleep(Duration::from_millis(200)).await;

        Ok(())
    }
}
