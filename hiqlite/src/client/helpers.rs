use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::{Client, Error, Node, NodeId};
use openraft::RaftMetrics;
use std::clone::Clone;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, error};

impl Client {
    #[inline(always)]
    pub(crate) async fn build_addr(
        &self,
        path: &str,
        leader: &Arc<RwLock<(NodeId, String)>>,
    ) -> String {
        let scheme = if self.inner.tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        let url = {
            let lock = leader.read().await;
            format!("{}://{}{}", scheme, lock.1, path)
        };
        debug!("request url: {}", url);
        url
    }

    pub(crate) async fn find_set_active_leader(&self) {
        if let Some(state) = &self.inner.state {
            // we never need to do any remote lookups for metrics -> get can never fail
            #[cfg(feature = "sqlite")]
            {
                let mut find_leader = Err(Error::Error("".into()));
                while find_leader.is_err() {
                    time::sleep(Duration::from_millis(100)).await;
                    let metrics = state.raft_db.raft.metrics().borrow().clone();
                    find_leader = Self::find_set_leader(metrics, &self.inner.leader_db).await;
                }
            }

            #[cfg(feature = "cache")]
            {
                let mut find_leader = Err(Error::Error("".into()));
                while find_leader.is_err() {
                    time::sleep(Duration::from_millis(100)).await;
                    let metrics = state.raft_cache.raft.metrics().borrow().clone();
                    find_leader = Self::find_set_leader(metrics, &self.inner.leader_cache).await;
                }
            }
        } else {
            // in this case, we have a remote client
            #[cfg(feature = "sqlite")]
            {
                let mut metrics = self.remote_metrics_loop_db().await;
                loop {
                    match Self::find_set_leader(metrics, &self.inner.leader_db).await {
                        Ok(_) => {
                            break;
                        }
                        Err(_) => {
                            metrics = self.remote_metrics_loop_db().await;
                        }
                    }
                }
            }

            #[cfg(feature = "cache")]
            {
                let mut metrics = self.remote_metrics_loop_cache().await;
                loop {
                    match Self::find_set_leader(metrics, &self.inner.leader_cache).await {
                        Ok(_) => {
                            break;
                        }
                        Err(_) => {
                            metrics = self.remote_metrics_loop_cache().await;
                        }
                    }
                }
            }
        }
    }

    #[cfg(feature = "cache")]
    async fn remote_metrics_loop_cache(&self) -> RaftMetrics<NodeId, Node> {
        loop {
            for addr in &self.inner.nodes {
                {
                    let mut lock = self.inner.leader_cache.write().await;
                    *lock = (lock.0, addr.clone());
                }

                match self.metrics_cache().await {
                    Ok(metrics) => {
                        return metrics;
                    }
                    Err(err) => {
                        error!("Error looking up Cache metrics: {}", err);
                    }
                }
            }
            time::sleep(Duration::from_millis(500)).await;
        }
    }

    #[cfg(feature = "sqlite")]
    async fn remote_metrics_loop_db(&self) -> RaftMetrics<NodeId, Node> {
        loop {
            for addr in &self.inner.nodes {
                {
                    let mut lock = self.inner.leader_db.write().await;
                    *lock = (lock.0, addr.clone());
                }

                match self.metrics_db().await {
                    Ok(metrics) => {
                        return metrics;
                    }
                    Err(err) => {
                        error!("Error looking up DB metrics: {}", err);
                    }
                }
            }
            time::sleep(Duration::from_millis(500)).await;
        }
    }

    async fn find_set_leader(
        metrics: RaftMetrics<NodeId, Node>,
        leader: &Arc<RwLock<(NodeId, String)>>,
    ) -> Result<(), Error> {
        let leader_id = match metrics.current_leader {
            None => {
                return Err(Error::Connect("Leader vote is in progress".to_string()));
            }
            Some(leader_id) => leader_id,
        };

        let leader_filtered = metrics
            .membership_config
            .nodes()
            .filter(|(id, _)| *id == &leader_id)
            .collect::<Vec<_>>();
        assert_eq!(leader_filtered.len(), 1);

        let mut lock = leader.write().await;
        *lock = (*leader_filtered[0].0, leader_filtered[0].1.addr_api.clone());

        Ok(())
    }

    #[cfg(feature = "sqlite")]
    #[inline(always)]
    pub async fn is_leader_db(&self) -> Option<&Arc<AppState>> {
        if let Some(state) = &self.inner.state {
            if state.id == self.inner.leader_db.read().await.0 {
                return Some(state);
            }
        }
        None
    }

    #[cfg(feature = "cache")]
    #[inline(always)]
    pub async fn is_leader_cache(&self) -> Option<&Arc<AppState>> {
        if let Some(state) = &self.inner.state {
            if state.id == self.inner.leader_cache.read().await.0 {
                return Some(state);
            }
        }
        None
    }

    #[cfg(not(feature = "dashboard"))]
    #[inline(always)]
    pub(crate) fn new_request_id(&self) -> usize {
        self.inner.request_id.fetch_add(1, Ordering::Relaxed)
    }

    #[cfg(feature = "dashboard")]
    #[inline(always)]
    pub(crate) fn new_request_id(&self) -> usize {
        if let Some(st) = &self.inner.state {
            st.new_request_id()
        } else {
            self.inner.request_id.fetch_add(1, Ordering::Relaxed)
        }
    }

    #[inline]
    pub(crate) async fn was_leader_update_error(
        &self,
        err: &Error,
        lock: &Arc<RwLock<(NodeId, String)>>,
        tx: &flume::Sender<ClientStreamReq>,
    ) -> bool {
        let mut has_changed = false;

        if let Some((id, node)) = err.is_forward_to_leader() {
            if id.is_some() && node.is_some() {
                let api_addr = node.as_ref().unwrap().addr_api.clone();
                let leader_id = id.unwrap();
                {
                    let mut lock = lock.write().await;
                    // we check additionally to prevent race conditions and multiple
                    // re-connect triggers
                    if lock.0 != leader_id {
                        *lock = (leader_id, api_addr.clone());
                        has_changed = true;
                    }
                }

                if has_changed {
                    tx.send_async(ClientStreamReq::LeaderChange((id, node.clone())))
                        .await
                        .expect("the Client API WebSocket Manager to always be running");
                }
            }
        }

        has_changed
    }
}
