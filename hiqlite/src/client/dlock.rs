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
                .lock_req_retry(CacheRequest::LockRelease((key.clone(), id)))
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

    pub async fn lock<K>(&self, key: K) -> Result<Lock, Error>
    where
        K: Into<Cow<'static, str>>,
    {
        let key = key.into();
        let state = self
            .lock_req_retry(CacheRequest::Lock((key.clone(), None)))
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
                            .lock_req_retry(CacheRequest::Lock((key.clone(), Some(id))))
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

    // pub async fn try_lock<K>(&self, key: K) -> Result<Lock, Error>
    // where
    //     K: Into<Cow<'static, str>>,
    // {
    //     let key = key.into();
    //     let state = self.lock_req_retry(CacheRequest::Lock(key.clone())).await?;
    //     match state {
    //         LockState::Locked(id) => Ok(Lock {
    //             id,
    //             key,
    //             client: self.clone(),
    //         }),
    //         LockState::Queued(id) => {
    //             let res = self.lock_await(key.clone(), id).await?;
    //             match res {
    //                 LockState::Released => {
    //                     let state = self.lock_req_retry(CacheRequest::Lock(key.clone())).await?;
    //                     match state {
    //                         LockState::Locked(id) => Ok(Lock {
    //                             id,
    //                             key,
    //                             client: self.clone(),
    //                         }),
    //                         _ => unreachable!(),
    //                     }
    //                 }
    //                 _ => unreachable!(),
    //             }
    //         }
    //         _ => unreachable!(),
    //     }
    // }

    async fn lock_await(&self, key: Cow<'static, str>, id: u64) -> Result<LockState, Error> {
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
            todo!("lock_await() for remote clients")
        }
    }

    async fn lock_req_retry(&self, cache_req: CacheRequest) -> Result<LockState, Error> {
        match self.lock_req(cache_req.clone()).await {
            Ok(state) => Ok(state),
            Err(err) => {
                if self.was_leader_update_error(&err).await {
                    self.lock_req(cache_req).await
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn lock_req(&self, cache_req: CacheRequest) -> Result<LockState, Error> {
        if let Some(state) = self.is_this_local_leader().await {
            let res = state.raft_cache.raft.client_write(cache_req).await?;
            let data: CacheResponse = res.data;
            match data {
                CacheResponse::Lock(state) => Ok(state),
                _ => unreachable!(),
            }
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
                ApiStreamResponsePayload::KV(res) => match res? {
                    CacheResponse::Lock(state) => Ok(state),
                    _ => unreachable!(),
                },
                #[cfg(feature = "sqlite")]
                _ => unreachable!(),
            }
        }
    }
}
