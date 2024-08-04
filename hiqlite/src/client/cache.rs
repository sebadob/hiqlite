use crate::client::stream::{ClientKVPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::state_machine::{CacheRequest, CacheResponse};
use crate::{Client, Error};
use chrono::Utc;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;
use strum::IntoEnumIterator;
use tokio::sync::oneshot;

impl Client {
    pub async fn get<C, K, V>(&self, cache: C, key: K) -> Result<Option<V>, Error>
    where
        C: Debug + Serialize + for<'a> Deserialize<'a> + IntoEnumIterator + ToPrimitive,
        K: Into<String>,
        V: for<'a> Deserialize<'a>,
    {
        if let Some(state) = &self.inner.state {
            let (ack, rx) = oneshot::channel();
            state
                .raft_cache
                .tx_caches
                .get(cache.to_usize().unwrap())
                .unwrap()
                .send(CacheRequestHandler::Get((key.into(), ack)))
                .expect("kv handler to always be running");
            let value = rx
                .await
                .expect("to always get an answer from the kv handler");
            Ok(value.map(|b| bincode::deserialize(&b).unwrap()))
        } else {
            todo!("CacheGet for remote clients")
        }
        // Err(Error::Cache("no value found".into()))
    }

    /// `Put` a value into the cache.
    /// The optional `ttl` is the lifetime of the value in seconds from *now* on.
    pub async fn put<C, K, V>(
        &self,
        cache: C,
        key: K,
        value: &V,
        ttl: Option<i64>,
    ) -> Result<(), Error>
    where
        C: Debug + Serialize + for<'a> Deserialize<'a> + IntoEnumIterator + ToPrimitive,
        K: Into<Cow<'static, str>>,
        V: Serialize,
    {
        self.cache_req_retry(CacheRequest::Put {
            cache_idx: cache
                .to_usize()
                .expect("Invalid ToPrimitive impl on Cache Index"),
            key: key.into(),
            value: bincode::serialize(value).unwrap(),
            expires: ttl.map(|seconds| Utc::now().timestamp().saturating_add(seconds)),
        })
        .await
    }

    /// `Delete` a value from the cache.
    pub async fn delete<C, K>(&self, cache: C, key: K) -> Result<(), Error>
    where
        C: Debug + Serialize + for<'a> Deserialize<'a> + IntoEnumIterator + ToPrimitive,
        K: Into<Cow<'static, str>>,
    {
        self.cache_req_retry(CacheRequest::Delete {
            cache_idx: cache
                .to_usize()
                .expect("Invalid ToPrimitive impl on Cache Index"),
            key: key.into(),
        })
        .await
    }

    async fn cache_req_retry(&self, cache_req: CacheRequest) -> Result<(), Error> {
        match self.cache_req(cache_req.clone()).await {
            Ok(_) => Ok(()),
            Err(err) => {
                if self.was_leader_update_error(&err).await {
                    self.cache_req(cache_req).await?;
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn cache_req(&self, cache_req: CacheRequest) -> Result<CacheResponse, Error> {
        if let Some(state) = self.is_this_local_leader().await {
            let res = state.raft_cache.raft.client_write(cache_req).await?;
            Ok(res.data)
        } else {
            let (ack, rx) = oneshot::channel();
            self.inner
                .tx_client
                .send_async(ClientStreamReq::KV(ClientKVPayload {
                    request_id: self.new_request_id(),
                    cache_req,
                    ack,
                }))
                .await
                .expect("Client Stream Manager to always be running");
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::KV(res) => res,
                #[cfg(feature = "sqlite")]
                _ => unreachable!(),
            }
        }
    }
}
