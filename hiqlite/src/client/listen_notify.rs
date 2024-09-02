use crate::client::stream::{ClientKVPayload, ClientStreamReq};
use crate::network::api::ApiStreamResponsePayload;
use crate::network::HEADER_NAME_SECRET;
use crate::store::state_machine::memory::state_machine::CacheRequest;
use crate::{Client, Error, NodeId};
use chrono::Utc;
use cryptr::utils::b64_decode;
use eventsource_client::{Client as ClientES, SSE};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};
use tokio::{task, time};
use tracing::{error, info, warn};

pub(crate) struct RemoteListener;

impl RemoteListener {
    pub(crate) fn spawn(
        leader_cache: Arc<RwLock<(NodeId, String)>>,
        tls: bool,
        api_secret: String,
    ) -> flume::Receiver<(i64, Vec<u8>)> {
        let (tx, rx) = flume::unbounded();
        task::spawn(Self::handler(leader_cache, api_secret, tls, tx));
        rx
    }

    async fn handler(
        leader_cache: Arc<RwLock<(NodeId, String)>>,
        api_secret: String,
        tls: bool,
        tx: flume::Sender<(i64, Vec<u8>)>,
    ) {
        'main: loop {
            let client = {
                let url = {
                    let scheme = if tls { "https" } else { "http" };
                    let lock = leader_cache.read().await;
                    format!("{}://{}/listen", scheme, lock.1)
                };
                info!("Connecting to listen SSE stream: {}", url);

                // TODO what about tls_no_verify in this case?
                eventsource_client::ClientBuilder::for_url(&url)
                    .expect("invalid listen SSE URL")
                    .header(HEADER_NAME_SECRET, &api_secret)
                    .unwrap()
                    .build()
            };

            let mut stream = client.stream();
            while let Some(res) = stream.next().await {
                match res {
                    Ok(sse) => match sse {
                        SSE::Connected(c) => {
                            info!("Opened /listen events stream: {:?}", c);
                        }
                        SSE::Event(event) => {
                            let (ts, data) = event
                                .data
                                .split_once(' ')
                                .expect("Invalid listen event from server");
                            let ts = ts
                                .parse::<i64>()
                                .expect("Cannot parse ts to i64 from listen event");
                            let bytes =
                                b64_decode(data).expect("Cannot decode data from listen event");

                            if let Err(err) = tx.send((ts, bytes)) {
                                error!("Error sending listen event to Client: {}", err);
                                break 'main;
                            }
                        }
                        SSE::Comment(_) => {}
                    },
                    Err(err) => {
                        error!("{:?}", err);
                        break;
                    }
                }
            }

            time::sleep(Duration::from_secs(1)).await;
        }

        warn!("RemoteListener exiting");
    }
}

impl Client {
    /// Listen to events on the distributed event bus
    pub async fn listen<T>(&self) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        let (_ts, bytes) = self.listen_rx().recv_async().await?;
        let res = bincode::deserialize(&bytes).unwrap();
        Ok(res)
    }

    /// Listen to events on the distributed event bus and get the raw bytes response
    pub async fn listen_bytes(&self) -> Result<(i64, Vec<u8>), Error> {
        Ok(self.listen_rx().recv_async().await?)
    }

    /// Tries to receive an event and returns immediately, if none is currently waiting.
    pub fn try_listen<T>(&self) -> Result<Option<T>, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Ok((_, bytes)) = self.listen_rx().try_recv() {
            let res = bincode::deserialize(&bytes).unwrap();
            Ok(Some(res))
        } else {
            Ok(None)
        }
    }

    /// Listen to events on the distributed event bus when their unix timestamp in microseconds
    /// is > `after_ts_micros`. This is helpful in case of applications restarts when cache
    /// events may be replayed to avoid duplicate event handling, if this applies to your case.
    pub async fn listen_after<T>(&self, after_ts_micros: i64) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        let rx = self.listen_rx();
        loop {
            let (ts, bytes) = rx.recv_async().await?;
            if ts > after_ts_micros {
                let res = bincode::deserialize(&bytes).unwrap();
                return Ok(res);
            }
        }
    }

    /// Listen to events on the distributed event bus that are "new" in the sense that they must
    /// have been created after this application has been started.
    pub async fn listen_after_start<T>(&self) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.listen_after(self.inner.app_start).await
    }

    #[inline]
    fn listen_rx(&self) -> &flume::Receiver<(i64, Vec<u8>)> {
        if let Some(state) = &self.inner.state {
            &state.raft_cache.rx_notify
        } else {
            self.inner
                .rx_notify
                .as_ref()
                .expect("a remote client must always have Some(_) inner.rx_notify")
        }
    }

    /// Notify all other Raft members with this new event data.
    pub async fn notify<P>(&self, payload: &P) -> Result<(), Error>
    where
        P: Serialize,
    {
        let now = Utc::now().timestamp_micros();

        match self
            .notify_req(CacheRequest::Notify((
                now,
                bincode::serialize(payload).unwrap(),
            )))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                if self
                    .was_leader_update_error(
                        &err,
                        &self.inner.leader_cache,
                        &self.inner.tx_client_cache,
                    )
                    .await
                {
                    self.notify_req(CacheRequest::Notify((
                        now,
                        bincode::serialize(payload).unwrap(),
                    )))
                    .await
                } else {
                    Err(err)
                }
            }
        }
    }

    pub(crate) async fn notify_req(&self, cache_req: CacheRequest) -> Result<(), Error> {
        if let Some(state) = self.is_leader_cache_with_state().await {
            state.raft_cache.raft.client_write(cache_req).await?;
            Ok(())
        } else {
            let (ack, rx) = oneshot::channel();
            self.inner
                .tx_client_cache
                .send_async(ClientStreamReq::Notify(ClientKVPayload {
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
                ApiStreamResponsePayload::Notify(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
