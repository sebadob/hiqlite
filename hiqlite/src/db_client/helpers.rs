use crate::app_state::AppState;
use crate::db_client::stream::ClientStreamReq;
use crate::network::HEADER_NAME_SECRET;
use crate::{DbClient, Error};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::debug;

impl DbClient {
    #[inline(always)]
    pub(crate) fn api_secret(&self) -> &str {
        if let Some(st) = &self.state {
            st.secret_api.as_str()
        } else {
            self.api_secret.as_ref().unwrap()
        }
    }

    #[inline(always)]
    pub(crate) async fn build_addr(&self, path: &str) -> String {
        let scheme = if self.tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        let url = {
            let lock = self.leader.read().await;
            format!("{}://{}{}", scheme, lock.1, path)
        };
        debug!("request url: {}", url);
        url
    }

    #[inline(always)]
    pub(crate) async fn is_this_local_leader(&self) -> Option<&Arc<AppState>> {
        if let Some(state) = &self.state {
            if state.id == self.leader.read().await.0 {
                return Some(state);
            }
        }
        None
    }

    #[inline(always)]
    pub(crate) fn new_request_id(&self) -> usize {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    #[inline]
    pub(crate) async fn was_leader_update_error(&self, err: &Error) -> bool {
        let mut has_changed = false;

        if let Some((id, node)) = err.is_forward_to_leader() {
            if id.is_some() && node.is_some() {
                let api_addr = node.as_ref().unwrap().addr_api.clone();
                let leader_id = id.unwrap();
                {
                    let mut lock = self.leader.write().await;
                    // we check additionally to prevent race conditions and multiple
                    // re-connect triggers
                    if lock.0 != leader_id {
                        *lock = (leader_id, api_addr.clone());
                        has_changed = true;
                    }
                }

                if has_changed {
                    self.tx_client
                        .send_async(ClientStreamReq::LeaderChange((id, node.clone())))
                        .await
                        .expect("the Client API WebSocket Manager to always be running");
                }
            }
        }

        has_changed
    }

    pub(crate) async fn send_with_retry<B: Serialize, Resp: for<'a> Deserialize<'a>>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> Result<Resp, Error> {
        let mut i = 0;
        loop {
            let url = self.build_addr(path).await;
            let res = if let Some(body) = body {
                let body = bincode::serialize(body).unwrap();
                self.client.post(url).body(body)
            } else {
                self.client.get(url)
            }
            .header(HEADER_NAME_SECRET, self.api_secret())
            .send()
            .await?;
            debug!("request status: {}", res.status());

            if res.status().is_success() {
                let bytes = res.bytes().await?;
                let resp = bincode::deserialize(bytes.as_ref())?;
                return Ok(resp);
            } else {
                let err = res.json::<Error>().await?;
                self.was_leader_update_error(&err).await;

                if i >= 2 {
                    return Err(err);
                }

                i += 1;
                continue;
            }
        }
    }
}
