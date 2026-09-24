use crate::client::helpers::await_channel_response;
use crate::client::stream::{ClientKVPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::memory::dlock_handler::{
    LOCK_ALIVE_INTERVAL, LOCK_VALID_SECONDS, LockAwaitPayload, LockRequest, LockState,
};
use crate::store::state_machine::memory::state_machine::{CacheRequest, CacheResponse};
use crate::{Client, Error};
use chrono::Utc;
use std::borrow::Cow;
use std::ops::Sub;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task;
use tracing::{debug, error, warn};

/// A distributed lock with the feature `dlock`. Releases on drop automatically.
pub struct Lock {
    id: u64,
    key: Cow<'static, str>,
    client: Client,
    cancel: flume::Sender<()>,
    is_locked: Arc<AtomicBool>,
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Stop the heartbeat before releasing so no further lease extensions race the release.
        let _ = self.cancel.send(());

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

impl Lock {
    /// You can check if the lock is still locked. It may be unlocked if your network went down,
    /// and it was not possible to send out alive-heartbeats. The timeout is 10 seconds (2s in
    /// debug builds).
    pub fn is_locked(&self) -> bool {
        self.is_locked.load(Ordering::Relaxed)
    }
}

/// Spawn the heartbeat ticker that keeps a held lock's lease alive until it is cancelled.
///
/// The server caps any lock at one lease window (`LOCK_VALID_SECONDS`) so crashed clients can
/// never block a key forever; the ticker extends the lease every `LOCK_ALIVE_INTERVAL` while
/// the client is alive and connected. A `Released` answer means the lease was lost (expired or
/// re-granted), after which the lock is gone anyway, so the ticker stops with a warning.
fn spawn_alive_ticker(
    client: &Client,
    key: Cow<'static, str>,
    id: u64,
    is_locked: Arc<AtomicBool>,
) -> flume::Sender<()> {
    let client = client.clone();
    let (cancel_tx, cancel_rx) = flume::bounded(1);
    let mut last_renew = Utc::now();

    task::spawn(async move {
        let mut ticker = tokio::time::interval_at(
            tokio::time::Instant::now() + LOCK_ALIVE_INTERVAL,
            LOCK_ALIVE_INTERVAL,
        );
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = cancel_rx.recv_async() => break,
                _ = ticker.tick() => {}
            }

            match client
                .lock_req_retry(CacheRequest::LockAlive((key.clone(), id)), false)
                .await
            {
                Ok(LockState::Locked(_)) => {
                    is_locked.store(true, Ordering::Relaxed);
                    last_renew = Utc::now();
                }
                Ok(LockState::Released) => {
                    is_locked.store(false, Ordering::Relaxed);
                    warn!(
                        "Distributed lock for {} / {} is no longer held (lease expired or \
                        released); stopping heartbeat",
                        key, id
                    );
                    break;
                }
                Ok(s) => unreachable!("{s:?}"),
                Err(err) => {
                    debug!("Error extending distributed lock for {key} / {id}: {err}");

                    // As long as this lock is still within the timeout window before the next
                    // retry, we can still consider it as being locked.
                    if (Utc::now().sub(last_renew).as_seconds_f32() * 1000.0) as u64
                        > Duration::from_secs(LOCK_VALID_SECONDS as u64)
                            .sub(LOCK_ALIVE_INTERVAL)
                            .as_millis() as u64
                    {
                        is_locked.store(false, Ordering::Relaxed);
                        warn!(
                            "Distributed lock for {} / {} is no longer held (timed out); stopping \
                            heartbeat",
                            key, id
                        );
                        break;
                    }
                }
            }
        }
    });

    cancel_tx
}

impl Client {
    /// Get a lock for the given key.
    ///
    /// You can lock any key, then do whatever you need, and as soon as the Lock you will get is
    /// being dropped, it will be released automatically.
    ///
    /// **Important:**
    /// Distributed locks have a hard timeout of 10 seconds (2s in debug builds), and they send
    /// heartbeats every 3 seconds (500ms in debug builds). A heartbeat extends the expiry. The hard
    /// timeout makes sure that a stale or crashed client can never hold a lock forever.
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
    /// // In the current implementation, a held lock renews itself automatically (heartbeats),
    /// // so it can be held for as long as needed. If the client crashes without releasing, the
    /// // lock is considered "dead" once one lease window has passed without renewal. This
    /// // prevents deadlocks just because some client or server crashed.
    /// drop(lock);
    /// ```
    pub async fn lock<K>(&self, key: K) -> Result<Lock, Error>
    where
        K: Into<Cow<'static, str>>,
    {
        self.rate_limit_cache().await?;

        let key = key.into();
        // `ticket` keeps our queue ticket across re-claims. Reusing the same ticket is important:
        // a new first-try `Lock` would mint a fresh ticket and leave the old one orphaned in the
        // handler queue, where it could block the lock until its lease expires.
        let mut ticket: Option<u64> = None;
        loop {
            let state = self
                .lock_req_retry(CacheRequest::Lock((key.clone(), ticket)), false)
                .await?;
            match state {
                LockState::Locked(id) => {
                    return Ok(Self::acquired(self, key, id));
                }
                LockState::Queued(id) => {
                    // Wait for our position. The lock may be granted directly while waiting if
                    // the previous holder's lease expired.
                    match self.lock_await(key.clone(), id).await? {
                        LockState::Locked(id) => {
                            return Ok(Self::acquired(self, key, id));
                        }
                        // Released: the handler promoted our ticket. Re-request with the same
                        // ticket to claim it.
                        LockState::Released => ticket = Some(id),
                        s => unreachable!("{:?}", s),
                    }
                }
                s => unreachable!("{:?}", s),
            }
        }
    }

    /// Build a granted lock and start its heartbeat ticker.
    fn acquired(client: &Client, key: Cow<'static, str>, id: u64) -> Lock {
        let is_locked = Arc::new(AtomicBool::new(true));
        Lock {
            cancel: spawn_alive_ticker(client, key.clone(), id, is_locked.clone()),
            id,
            key,
            client: client.clone(),
            is_locked,
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
            let state = await_channel_response(rx).await?;
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
        if let Some(state) = self.is_leader_cache_with_state().await {
            let res = Self::client_write_local(&state.raft_cache.raft, cache_req).await?;
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
                .map_err(|err| Error::Error(err.to_string().into()))?;
            let res = await_channel_response(rx).await??;
            match res {
                ApiStreamResponsePayload::KV(res) => match res? {
                    CacheResponse::Lock(state) => {
                        assert!(!is_remote_await);
                        Ok(state)
                    }
                    _ => unreachable!(),
                },
                ApiStreamResponsePayload::Lock(Ok(LockState::Released)) => {
                    assert!(is_remote_await);
                    Ok(LockState::Released)
                }
                ApiStreamResponsePayload::Lock(Ok(LockState::Locked(id))) => {
                    assert!(is_remote_await);
                    Ok(LockState::Locked(id))
                }
                ApiStreamResponsePayload::Lock(Err(err)) => Err(err),
                #[cfg(any(feature = "sqlite", feature = "dlock"))]
                _ => unreachable!(),
            }
        }
    }
}
