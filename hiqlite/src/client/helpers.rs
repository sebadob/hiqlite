use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigCache;
#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::{Client, Error, Node, NodeId};
use openraft::RaftMetrics;
use openraft::RaftTypeConfig;
use openraft::type_config::async_runtime::watch::WatchReceiver;
use std::clone::Clone;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};
use tokio::time;
use tracing::{debug, error, warn};

/// Timeout applied to every client request waiting for a response from the stream manager or a
/// local handler. A stalled leader (GC pause, blocked writer, network partition without a
/// disconnect) must not hang the caller forever. Keep it generous: slow transactions and
/// backups legitimately take a while. A timed-out request must be treated as at-least-once: the
/// operation may still have been applied server-side.
const STREAM_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Timeout applied to direct local-leader `client_write` calls (bypassing the stream path). A
/// local leader writing into a partitioned cluster (no quorum) must not hang the caller forever;
/// fail after this budget instead. Keep it generous: slow transactions and backups legitimately
/// take a while. A timed-out write must be treated as at-least-once: the operation may still have
/// been applied server-side.
pub(crate) const LOCAL_CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Waits for a response channel to resolve, surfacing a stall or a dead handler as an error
/// instead of hanging or panicking the calling task.
pub(crate) async fn await_channel_response<T>(rx: oneshot::Receiver<T>) -> Result<T, Error> {
    match time::timeout(STREAM_REQUEST_TIMEOUT, rx).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err(Error::Error("response channel closed".into())),
        Err(_) => Err(Error::Connect("request timed out".into())),
    }
}

impl Client {
    #[inline]
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

    /// Wraps a direct `client_write` on the local leader with [`LOCAL_CLIENT_WRITE_TIMEOUT`] so a
    /// stalled cluster (no quorum) fails loudly instead of hanging the caller forever. The bounds
    /// mirror openraft's own `client_write`, so `C` and `E` are inferred exactly as at a plain call
    /// site.
    pub(crate) async fn client_write_local<C, SM>(
        raft: &openraft::Raft<C, SM>,
        req: C::D,
    ) -> Result<openraft::raft::ClientWriteResponse<C>, Error>
    where
        C: openraft::RaftTypeConfig<NodeId = u64, Node = Node>,
        SM: openraft::storage::RaftStateMachine<C>,
        Error: From<openraft::errors::RaftError<C, openraft::errors::ClientWriteError<C>>>,
    {
        match time::timeout(LOCAL_CLIENT_WRITE_TIMEOUT, raft.client_write(req)).await {
            Ok(res) => res.map_err(Into::into),
            Err(_) => Err(Error::Timeout("local client write timed out".into())),
        }
    }

    pub(crate) async fn find_set_active_leader(&self) {
        if let Some(state) = &self.inner.state {
            // we never need to do any remote lookups for metrics -> get can never fail
            #[cfg(feature = "sqlite")]
            {
                let metrics = state.raft_db.raft.metrics().borrow_watched().clone();
                let mut find_leader = Self::find_set_leader(metrics, &self.inner.leader_db).await;

                while let Err(err) = find_leader {
                    warn!("Find DB leader error: {}", err);
                    time::sleep(Duration::from_millis(500)).await;
                    let metrics = state.raft_db.raft.metrics().borrow_watched().clone();
                    find_leader = Self::find_set_leader(metrics, &self.inner.leader_db).await;
                }
            }

            #[cfg(feature = "cache")]
            {
                let metrics = state.raft_cache.raft.metrics().borrow_watched().clone();
                let mut find_leader =
                    Self::find_set_leader(metrics, &self.inner.leader_cache).await;

                while let Err(err) = find_leader {
                    warn!("Find cache leader error: {}", err);
                    time::sleep(Duration::from_millis(500)).await;
                    let metrics = state.raft_cache.raft.metrics().borrow_watched().clone();
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
                        Ok(()) => {
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
                        Ok(()) => {
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
    async fn remote_metrics_loop_cache(&self) -> RaftMetrics<TypeConfigCache> {
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
    async fn remote_metrics_loop_db(&self) -> RaftMetrics<TypeConfigSqlite> {
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

    /// Resolve the current leader from cluster metrics and update the cached leader address.
    pub(crate) async fn find_set_leader<C>(
        metrics: RaftMetrics<C>,
        leader: &Arc<RwLock<(NodeId, String)>>,
    ) -> Result<(), Error>
    where
        C: RaftTypeConfig<
                NodeId = u64,
                Node = Node,
                Membership = openraft::impls::Membership<u64, Node>,
            >,
    {
        let Some(leader_id) = metrics.current_leader else {
            return Err(Error::Connect("Leader vote is in progress".to_string()));
        };

        // Stale-metrics race: the reported leader may no longer be part of the membership (e.g. a
        // concurrent membership change). Fail loud so callers re-poll fresh metrics instead of
        // panicking the long-lived stream task.
        let Some((leader_node_id, node)) = metrics
            .membership_config
            .nodes()
            .find(|(id, _)| *id == &leader_id)
        else {
            return Err(Error::Connect(format!(
                "Reported leader {leader_id} is not part of the current membership (stale metrics)"
            )));
        };

        let mut lock = leader.write().await;
        *lock = (*leader_node_id, node.addr_api.clone());

        Ok(())
    }

    /// Check if this instance is the current Raft cluster leader for the database.
    #[cfg(feature = "sqlite")]
    pub async fn is_leader_db(&self) -> bool {
        if let Some(state) = &self.inner.state
            && state.id == self.inner.leader_db.read().await.0
        {
            return true;
        }
        false
    }

    /// Check if this instance is the current Raft cluster leader for the cache.
    #[cfg(feature = "cache")]
    pub async fn is_leader_cache(&self) -> bool {
        if let Some(state) = &self.inner.state
            && state.id == self.inner.leader_cache.read().await.0
        {
            return true;
        }
        false
    }

    #[cfg(feature = "sqlite")]
    #[inline]
    pub(crate) async fn is_leader_db_with_state(&self) -> Option<&Arc<AppState>> {
        if let Some(state) = &self.inner.state
            && state.id == self.inner.leader_db.read().await.0
        {
            return Some(state);
        }
        None
    }

    #[cfg(feature = "cache")]
    #[inline]
    pub(crate) async fn is_leader_cache_with_state(&self) -> Option<&Arc<AppState>> {
        if let Some(state) = &self.inner.state
            && state.id == self.inner.leader_cache.read().await.0
        {
            return Some(state);
        }
        None
    }

    #[cfg(not(feature = "dashboard"))]
    #[inline]
    pub(crate) fn new_request_id(&self) -> usize {
        self.inner.request_id.fetch_add(1, Ordering::Relaxed)
    }

    #[cfg(feature = "dashboard")]
    #[inline]
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
        let mut was_leader_error = false;

        if let Some((id, node)) = err.is_forward_to_leader()
            && let Some(leader_id) = id
            && let Some(node) = node
        {
            was_leader_error = true;

            let api_addr = node.addr_api.clone();
            {
                let mut lock = lock.write().await;
                // we check additionally to prevent race conditions and multiple
                // re-connect triggers
                if lock.0 != leader_id {
                    *lock = (leader_id, api_addr.clone());
                }
            }

            if was_leader_error
                && let Err(err) = tx
                    .send_async(ClientStreamReq::LeaderChange((id, Some(node.clone()))))
                    .await
            {
                error!("Error sending LeaderChange to Client API WebSocket Manager: {err:?}");
            }
        }

        was_leader_error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{TypeConfigWire, WireMetrics};

    #[tokio::test]
    async fn await_channel_response_returns_value() {
        let (tx, rx) = oneshot::channel();
        tx.send(42i32).unwrap();
        assert_eq!(await_channel_response(rx).await.unwrap(), 42);
    }

    #[tokio::test]
    async fn await_channel_response_errors_when_closed() {
        let (tx, rx) = oneshot::channel::<i32>();
        drop(tx);
        let err = await_channel_response(rx).await.unwrap_err();
        assert!(err.to_string().contains("channel closed"));
    }

    #[tokio::test(start_paused = true)]
    async fn await_channel_response_times_out() {
        let (_tx, rx) = oneshot::channel::<i32>();
        tokio::time::advance(STREAM_REQUEST_TIMEOUT + Duration::from_secs(1)).await;
        let err = await_channel_response(rx).await.unwrap_err();
        assert!(err.to_string().contains("timed out"));
    }

    fn metrics_with_leader(leader_id: u64, members: &[u64]) -> WireMetrics {
        use openraft::type_config::alias::{MembershipOf, StoredMembershipOf};
        use std::collections::{BTreeMap, BTreeSet};

        let nodes = members
            .iter()
            .map(|id| {
                (
                    *id,
                    Node {
                        id: *id,
                        addr_raft: format!("localhost:{}", 8100 + id),
                        addr_api: format!("localhost:{}", 8200 + id),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let membership = MembershipOf::<TypeConfigWire>::new(
            vec![BTreeSet::from_iter(members.iter().copied())],
            nodes,
        )
        .unwrap();

        // `find_set_leader` only reads the leader and the membership, everything else can stay
        // at openraft's default
        let mut metrics = RaftMetrics::new_initial(leader_id);
        metrics.current_leader = Some(leader_id);
        metrics.membership_config =
            Arc::new(StoredMembershipOf::<TypeConfigWire>::new(None, membership));
        metrics
    }

    #[tokio::test]
    async fn find_set_leader_updates_leader() {
        let metrics = metrics_with_leader(1, &[1]);
        let leader = Arc::new(RwLock::new((0, String::new())));
        assert!(Client::find_set_leader(metrics, &leader).await.is_ok());
        assert_eq!(*leader.read().await, (1, "localhost:8201".to_string()));
    }

    #[tokio::test]
    async fn find_set_leader_errors_on_stale_metrics() {
        let metrics = metrics_with_leader(99, &[1]);
        let leader = Arc::new(RwLock::new((0, String::new())));
        let err = Client::find_set_leader(metrics, &leader).await.unwrap_err();
        assert!(err.to_string().contains("stale metrics"));
        assert_eq!(*leader.read().await, (0, String::new()));
    }
}
