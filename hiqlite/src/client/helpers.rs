use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::network::HEADER_NAME_SECRET;
use crate::{Client, Error, NodeId};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

impl Client {
    #[inline(always)]
    pub(crate) fn api_secret(&self) -> &str {
        if let Some(st) = &self.inner.state {
            st.secret_api.as_str()
        } else {
            self.inner.api_secret.as_ref().unwrap()
        }
    }

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

    #[cfg(feature = "sqlite")]
    #[inline(always)]
    pub(crate) async fn is_leader_db(&self) -> Option<&Arc<AppState>> {
        if let Some(state) = &self.inner.state {
            if state.id == self.inner.leader_db.read().await.0 {
                return Some(state);
            }
        }
        None
    }

    #[cfg(feature = "cache")]
    #[inline(always)]
    pub(crate) async fn is_leader_cache(&self) -> Option<&Arc<AppState>> {
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

    #[cfg(feature = "cache")]
    pub(crate) async fn send_with_retry_cache<B: Serialize, Resp: for<'a> Deserialize<'a>>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> Result<Resp, Error> {
        let url = self.build_addr(path, &self.inner.leader_cache).await;
        self.send_with_retry(
            url,
            &self.inner.leader_cache,
            &self.inner.tx_client_cache,
            body,
        )
        .await
    }

    #[cfg(feature = "sqlite")]
    pub(crate) async fn send_with_retry_db<B: Serialize, Resp: for<'a> Deserialize<'a>>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> Result<Resp, Error> {
        let url = self.build_addr(path, &self.inner.leader_db).await;
        self.send_with_retry(url, &self.inner.leader_db, &self.inner.tx_client_db, body)
            .await
    }

    #[inline]
    async fn send_with_retry<B: Serialize, Resp: for<'a> Deserialize<'a>>(
        &self,
        url: String,
        leader: &Arc<RwLock<(NodeId, String)>>,
        tx_client: &flume::Sender<ClientStreamReq>,
        body: Option<&B>,
    ) -> Result<Resp, Error> {
        let mut i = 0;
        loop {
            let res = if let Some(body) = body {
                let body = bincode::serialize(body).unwrap();
                self.inner.client.post(url.clone()).body(body)
            } else {
                self.inner.client.get(url.clone())
            }
            .header(HEADER_NAME_SECRET, self.api_secret())
            .send()
            .await?;
            debug!("request status: {}", res.status());

            // let content_type = res.headers().get(CONTENT_TYPE);
            // let is_json =
            //     content_type.map(|v| v.to_str().unwrap_or_default()) == Some("application/json");

            if res.status().is_success() {
                let bytes = res.bytes().await?;
                // let resp = if is_json {
                //     serde_json::from_slice(&bytes)?
                // } else {
                //     bincode::deserialize(bytes.as_ref())?
                // };
                let resp = bincode::deserialize(bytes.as_ref())?;
                return Ok(resp);
            } else {
                let err = res.json::<Error>().await?;
                // TODO do we ever use this with cache? if not - feature gate!
                self.was_leader_update_error(&err, leader, tx_client).await;

                if i >= 2 {
                    return Err(err);
                }

                i += 1;
                continue;
            }
        }
    }
}
