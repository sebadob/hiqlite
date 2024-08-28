use crate::client::stream::{ClientKVPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::memory::dlock_handler::{
    LockAwaitPayload, LockRequest, LockState,
};
use crate::store::state_machine::memory::state_machine::{CacheRequest, CacheResponse};
use crate::{Client, Error};
use std::borrow::Cow;
use tokio::sync::oneshot;
use tokio::task;
use tracing::error;

/// A distributed lock with the feature `dlock`. Releases on drop automatically.
#[derive(Clone)]
pub struct Lock {
    id: u64,
    key: Cow<'static, str>,
    client: Client,
}

impl Drop for Lock {
    fn drop(&mut self) {
        let client = self.client.clone();
        let key = self.key.clone();
        let id = self.id;

        task::spawn(async move {
            if let Err(err) = client
                .lock_req_retry(CacheRequest::LockRelease((key.clone(), id)), false)
                .await
            {
                error!(
                    "Error releasing distributed lock for {} / {}: {}",
                    key, id, err
                );
            }
        });
    }
}

impl Client {
    // TODO
    // - lock_timeout

    /// Get a lock for the given key.
    ///
    /// ```rust, notest
    /// // In some cases, you need to make sure you get some lock for either longer running actions
    /// // or ones that need retrieving data, manipulating it and then sending it back to the DB.
    /// // In these cases you might not be able to do all at once in a SQL query.
    /// // Hiqlite has distributed locks (feature `dlock`) to achieve this.
    /// let lock = client.lock("my lock key").await?;
    ///
    /// // A lock key can be any String to provide the most flexibility.
    /// // It behaves the same as any other lock - it will be released on drop and as long as it
    /// // exists, other locks will have to wait.
    /// //
    /// // In the current implementation, distributed locks have an internal timeout of 10 seconds.
    /// // When this time expires, a lock will be considered "dead" because of network issues, just
    /// // in case it has not been possible to release the lock properly. This prevents deadlocks
    /// // just because some client or server crashed.
    /// drop(lock);
    /// ```
    pub async fn lock<K>(&self, key: K) -> Result<Lock, Error>
    where
        K: Into<Cow<'static, str>>,
    {
        let key = key.into();
        let state = self
            .lock_req_retry(CacheRequest::Lock((key.clone(), None)), false)
            .await?;
        match state {
            LockState::Locked(id) => Ok(Lock {
                id,
                key,
                client: self.clone(),
            }),
            LockState::Queued(id) => {
                let res = self.lock_await(key.clone(), id).await?;
                match res {
                    LockState::Released => {
                        let state = self
                            .lock_req_retry(CacheRequest::Lock((key.clone(), Some(id))), false)
                            .await?;
                        match state {
                            LockState::Locked(id) => Ok(Lock {
                                id,
                                key,
                                client: self.clone(),
                            }),
                            s => unreachable!("{:?}", s),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    pub(crate) async fn lock_await(
        &self,
        key: Cow<'static, str>,
        id: u64,
    ) -> Result<LockState, Error> {
        if let Some(state) = &self.inner.state {
            let (ack, rx) = oneshot::channel();
            state
                .raft_cache
                .tx_dlock
                .send(LockRequest::Await(LockAwaitPayload { key, id, ack }))
                .expect("kv handler to always be running");
            let state = rx
                .await
                .expect("to always get an answer from the kv handler");
            Ok(state)
        } else {
            self.lock_req_retry(CacheRequest::LockAwait((key.clone(), id)), true)
                .await
        }
    }

    pub(crate) async fn lock_req_retry(
        &self,
        cache_req: CacheRequest,
        is_remote_await: bool,
    ) -> Result<LockState, Error> {
        match self.lock_req(cache_req.clone(), is_remote_await).await {
            Ok(state) => Ok(state),
            Err(err) => {
                if self
                    .was_leader_update_error(
                        &err,
                        &self.inner.leader_cache,
                        &self.inner.tx_client_cache,
                    )
                    .await
                {
                    self.lock_req(cache_req, is_remote_await).await
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn lock_req(
        &self,
        cache_req: CacheRequest,
        is_remote_await: bool,
    ) -> Result<LockState, Error> {
        if let Some(state) = self.is_leader_cache().await {
            let res = state.raft_cache.raft.client_write(cache_req).await?;
            let data: CacheResponse = res.data;
            match data {
                CacheResponse::Lock(state) => Ok(state),
                _ => unreachable!(),
            }
        } else {
            let (ack, rx) = oneshot::channel();

            let payload = if is_remote_await {
                ClientStreamReq::LockAwait(ClientKVPayload {
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
                ApiStreamResponsePayload::KV(res) => match res? {
                    CacheResponse::Lock(state) => {
                        assert!(!is_remote_await);
                        Ok(state)
                    }
                    _ => unreachable!(),
                },
                ApiStreamResponsePayload::Lock(LockState::Released) => {
                    assert!(is_remote_await);
                    Ok(LockState::Released)
                }
                #[cfg(any(feature = "sqlite", feature = "dlock"))]
                _ => unreachable!(),
            }
        }
    }
}
