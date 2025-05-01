use crate::cache_idx::CacheIndex;
use crate::client::stream::{ClientKVPayload, ClientStreamReq};
use crate::helpers::deserialize;
use crate::network::api::ApiStreamResponsePayload;
use crate::network::serialize_network;
use crate::store::state_machine::memory::kv_handler::CacheRequestHandler;
use crate::store::state_machine::memory::state_machine::{CacheRequest, CacheResponse};
use crate::{Client, Error};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use tokio::sync::oneshot;

impl Client {
    /// Clears a single cache.
    pub async fn clear_cache<C>(&self, cache: C) -> Result<(), Error>
    where
        C: CacheIndex,
    {
        self.cache_req_retry(
            CacheRequest::Clear {
                cache_idx: cache.to_usize(),
            },
            false,
        )
        .await?;

        Ok(())
    }

    /// Clears all available caches.
    pub async fn clear_cache_all(&self) -> Result<(), Error> {
        self.cache_req_retry(CacheRequest::ClearAll, false).await?;
        Ok(())
    }

    /// GET a value from the cache.
    ///
    /// ```rust, notest
    /// let key = "my key 1";
    /// let value = Value {
    ///     id: "some id".to_string(),
    ///     num: 1337,
    ///     description: Some("My Example Description".to_string()),
    /// };
    ///
    /// // Insert a value that will expire 1 second later. Each value has its own custom expiry.
    /// client.put(Cache::One, key, &value, Some(1)).await?;
    ///
    /// let v: Value = client.get(Cache::One, key).await?.unwrap();
    //  assert_eq!(v, value);
    /// ```
    pub async fn get<C, K, V>(&self, cache: C, key: K) -> Result<Option<V>, Error>
    where
        C: CacheIndex,
        K: Into<String>,
        V: for<'a> Deserialize<'a>,
    {
        match self.get_bytes(cache, key).await {
            Ok(value) => {
                if let Some(v) = value {
                    Ok(Some(deserialize(&v)?))
                } else {
                    Ok(None)
                }
            }
            Err(err) => Err(err),
        }
    }

    /// GET a raw bytes value from the cache.
    ///
    /// Works in the same way as `.get()` without any value mapping.
    pub async fn get_bytes<C, K>(&self, cache: C, key: K) -> Result<Option<Vec<u8>>, Error>
    where
        C: CacheIndex,
        K: Into<String>,
    {
        if let Some(state) = &self.inner.state {
            let (ack, rx) = oneshot::channel();
            state
                .raft_cache
                .tx_caches
                .get(cache.to_usize())
                .unwrap()
                .send(CacheRequestHandler::Get((key.into(), ack)))
                .expect("kv handler to always be running");
            let value = rx
                .await
                .expect("to always get an answer from the kv handler");
            Ok(value)
        } else {
            let res = self
                .cache_req_retry(
                    CacheRequest::Get {
                        cache_idx: cache.to_usize(),
                        key: key.into(),
                    },
                    true,
                )
                .await?;
            match res {
                CacheResponse::Value(opt) => Ok(opt),
                _ => unreachable!(),
            }
        }
    }

    /// `Put` a value into the cache.
    /// The optional `ttl` is the lifetime of the value in seconds from *now* on.
    ///
    /// ```rust, notest
    /// let key = "my key 1";
    /// let value = Value {
    ///     id: "some id".to_string(),
    ///     num: 1337,
    ///     description: Some("My Example Description".to_string()),
    /// };
    ///
    /// // Insert a value that will expire 1 second later. Each value has its own custom expiry.
    /// client.put(Cache::One, key, &value, Some(1)).await?;
    ///
    /// let v: Value = client.get(Cache::One, key).await?.unwrap();
    ///  assert_eq!(v, value);
    /// ```
    pub async fn put<C, K, V>(
        &self,
        cache: C,
        key: K,
        value: &V,
        ttl: Option<i64>,
    ) -> Result<(), Error>
    where
        C: CacheIndex,
        K: Into<Cow<'static, str>>,
        V: Serialize,
    {
        self.put_bytes(cache, key, serialize_network(value), ttl)
            .await?;
        Ok(())
    }

    /// GET a raw bytes value from the cache.
    ///
    /// Works in the same way as `.get()` without any value mapping.
    pub async fn put_bytes<C, K>(
        &self,
        cache: C,
        key: K,
        value: Vec<u8>,
        ttl: Option<i64>,
    ) -> Result<(), Error>
    where
        C: CacheIndex,
        K: Into<Cow<'static, str>>,
    {
        self.cache_req_retry(
            CacheRequest::Put {
                cache_idx: cache.to_usize(),
                key: key.into(),
                value,
                expires: ttl.map(|seconds| Utc::now().timestamp().saturating_add(seconds)),
            },
            false,
        )
        .await?;

        Ok(())
    }

    /// `Delete` a value from the cache.
    pub async fn delete<C, K>(&self, cache: C, key: K) -> Result<(), Error>
    where
        C: CacheIndex,
        K: Into<Cow<'static, str>>,
    {
        self.cache_req_retry(
            CacheRequest::Delete {
                cache_idx: cache.to_usize(),
                key: key.into(),
            },
            false,
        )
        .await?;

        Ok(())
    }

    pub(crate) async fn cache_req_retry(
        &self,
        cache_req: CacheRequest,
        is_remote_get: bool,
    ) -> Result<CacheResponse, Error> {
        match self.cache_req(cache_req.clone(), is_remote_get).await {
            Ok(resp) => Ok(resp),
            Err(err) => {
                if self
                    .was_leader_update_error(
                        &err,
                        &self.inner.leader_cache,
                        &self.inner.tx_client_cache,
                    )
                    .await
                {
                    self.cache_req(cache_req, is_remote_get).await
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn cache_req(
        &self,
        cache_req: CacheRequest,
        is_remote_get: bool,
    ) -> Result<CacheResponse, Error> {
        if let Some(state) = self.is_leader_cache_with_state().await {
            let res = state.raft_cache.raft.client_write(cache_req).await?;
            Ok(res.data)
        } else {
            let (ack, rx) = oneshot::channel();
            let payload = if is_remote_get {
                ClientStreamReq::KVGet(ClientKVPayload {
                    request_id: self.new_request_id(),
                    cache_req,
                    ack,
                })
            } else {
                ClientStreamReq::KV(ClientKVPayload {
                    request_id: self.new_request_id(),
                    cache_req,
                    ack,
                })
            };

            self.inner
                .tx_client_cache
                .send_async(payload)
                .await
                .expect("Client Stream Manager to always be running");
            let res = rx
                .await
                .expect("To always receive an answer from Client Stream Manager")?;
            match res {
                ApiStreamResponsePayload::KV(res) => res,
                #[cfg(any(feature = "sqlite", feature = "dlock", feature = "listen_notify_local"))]
                _ => unreachable!(),
            }
        }
    }
}
