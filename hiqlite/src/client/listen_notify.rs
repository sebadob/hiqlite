use crate::client::helpers::await_channel_response;
use crate::client::stream::{ClientKVPayload, ClientStreamReq};
use crate::helpers::{deserialize_serde, serialize_serde};
use crate::network::api::ApiStreamResponsePayload;
use crate::store::state_machine::memory::state_machine::CacheRequest;
use crate::{Client, Error};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

/// The "listen_notify" feature currently enables _remote_.
#[cfg(feature = "listen_notify")]
pub(crate) mod remote {
    use crate::helpers::deserialize;
    use crate::network::web_socket_connect;
    use crate::{Error, NodeId};
    use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, WebSocket};
    use hyper::upgrade::Upgraded;
    use hyper_util::rt::TokioIo;
    use std::ops::Deref;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::ReadHalf;
    use tokio::sync::RwLock;
    use tokio::{select, task, time};
    use tracing::{debug, error, info};

    /// Interval between client-initiated keepalive pings on the /listen WebSocket. The server
    /// auto-pongs every ping (fastwebsockets default), so a healthy idle connection always produces
    /// inbound traffic within this interval even when no events are being pushed.
    const LISTEN_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

    /// Maximum time without any frame from the server before the client declares the /listen
    /// connection dead and reconnects. Must be a multiple of LISTEN_KEEPALIVE_INTERVAL with margin.
    const LISTEN_READ_TIMEOUT: Duration = Duration::from_secs(35);

    /// Number of consecutive connection failures that retry at the fast (1 s) interval before the
    /// reconnect loop backs off to RECONNECT_SLOW_INTERVAL.
    const RECONNECT_FAST_RETRIES: u32 = 5;

    const RECONNECT_SLOW_INTERVAL: Duration = Duration::from_secs(5);

    /// A frame the reader task reports back to the handler. `Event` carries a deserialized listen
    /// event `(ts, data)`; `Liveness` is a keepalive pong from the server proving the connection is
    /// alive (the server auto-pongs our pings).
    #[derive(Debug)]
    enum ListenRead {
        Event((i64, Vec<u8>)),
        Liveness,
    }

    pub(crate) struct RemoteListener;

    impl RemoteListener {
        pub(crate) fn spawn(
            leader_cache: Arc<RwLock<(NodeId, String)>>,
            tls_config: Option<Arc<rustls::ClientConfig>>,
            api_secret: String,
        ) -> flume::Receiver<(i64, Vec<u8>)> {
            let (tx, rx) = flume::unbounded();
            task::spawn(Self::handler(leader_cache, tls_config, api_secret, tx));
            rx
        }

        async fn handler(
            leader_cache: Arc<RwLock<(NodeId, String)>>,
            tls_config: Option<Arc<rustls::ClientConfig>>,
            api_secret: String,
            tx: flume::Sender<(i64, Vec<u8>)>,
        ) {
            let mut failed_retries: u32 = 0;
            loop {
                let ws = match Self::connect(&leader_cache, tls_config.clone(), &api_secret).await {
                    Ok(ws) => {
                        failed_retries = 0;
                        info!(
                            "Client /listen WebSocket to {} opened successfully",
                            leader_cache.read().await.1
                        );
                        ws
                    }
                    Err(err) => {
                        // Back off after RECONNECT_FAST_RETRIES consecutive fast (1 s) retries so a
                        // restarting node does not spam the log, but keep the window short so
                        // reconnection stays responsive.
                        let interval = if failed_retries < RECONNECT_FAST_RETRIES {
                            Duration::from_millis(1000)
                        } else {
                            RECONNECT_SLOW_INTERVAL
                        };
                        failed_retries += 1;
                        time::sleep(interval).await;
                        error!(
                            "Could not connect Client /listen WebSocket to {}: {}",
                            leader_cache.read().await.1,
                            err
                        );
                        continue;
                    }
                };

                let (read_half, mut write) = ws.split(tokio::io::split);
                // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8! So it runs in its own task and
                // the handler only selects on the channel it feeds (mirrors client/stream.rs).
                let read = FragmentCollectorRead::new(read_half);
                let (tx_read, rx_read) = flume::bounded(1);
                let handle_read = task::spawn(Self::listen_reader(read, tx_read));

                // Liveness tracking: any frame from the server resets `last_frame`. A healthy connection
                // always produces inbound traffic within LISTEN_READ_TIMEOUT because we ping every
                // LISTEN_KEEPALIVE_INTERVAL and the server auto-pongs.
                let mut last_frame = time::Instant::now();
                let mut keepalive = time::interval(LISTEN_KEEPALIVE_INTERVAL);

                loop {
                    let res: Option<Result<ListenRead, flume::RecvError>> = select! {
                        res = rx_read.recv_async() => Some(res),
                        _ = keepalive.tick() => {
                            // Client-initiated ping: makes a half-open connection fail its writes, and the
                            // server's auto-pong gives us inbound traffic to measure.
                            let frame = Frame::new(true, OpCode::Ping, None, Payload::from(vec![]));
                            if write.write_frame(frame).await.is_err() {
                                error!("Client /listen keepalive ping failed - connection lost");
                                break;
                            }
                            None
                        }
                        _ = time::sleep(LISTEN_READ_TIMEOUT.saturating_sub(last_frame.elapsed())) => {
                            error!(
                                "No frames from server for {:?} on /listen - assuming dead connection, reconnecting",
                                LISTEN_READ_TIMEOUT
                            );
                            break;
                        }
                    };

                    let Some(res) = res else {
                        continue;
                    };

                    match res {
                        Ok(ListenRead::Event((ts, data))) => {
                            last_frame = time::Instant::now();
                            if let Err(err) = tx.send((ts, data)) {
                                error!("Error sending /listen event to Client: {}", err);
                                break;
                            }
                        }
                        Ok(ListenRead::Liveness) => {
                            // keepalive pong from the server: connection is alive
                            last_frame = time::Instant::now();
                        }
                        Err(err) => {
                            // reader task exited (connection closed or errored): reconnect
                            debug!("Client /listen reader disconnected: {:?}", err);
                            break;
                        }
                    }
                }

                handle_read.abort();
                // drain any buffered event so it is not lost on reconnect
                while let Ok(item) = rx_read.recv_async().await {
                    if let ListenRead::Event((ts, data)) = item {
                        let _ = tx.send((ts, data));
                    }
                }

                debug!("Client /listen WebSocket closed - reconnecting");
            }
        }

        /// Reads frames off the /listen socket and reports deserialized events (or liveness pongs) to
        /// `tx`. Runs in its own task because the fastwebsockets reader is not cancel-safe.
        async fn listen_reader(
            mut read: FragmentCollectorRead<ReadHalf<TokioIo<Upgraded>>>,
            tx: flume::Sender<ListenRead>,
        ) {
            while let Ok(frame) = read
                .read_frame(&mut |frame| async move {
                    debug!(
                        "Received obligated send in /listen client: OpCode: {:?}: {:?}",
                        frame.opcode.clone(),
                        frame.payload
                    );
                    Ok::<(), Error>(())
                })
                .await
            {
                match frame.opcode {
                    OpCode::Binary => {
                        let bytes = frame.payload.deref();
                        let (ts, data) = match deserialize::<(i64, Vec<u8>)>(bytes) {
                            Ok(v) => v,
                            Err(err) => {
                                // corrupt frame from the server: do not panic the reader task, just
                                // drop the connection so the client can reconnect
                                error!("Error deserializing /listen event from server: {err:?}");
                                break;
                            }
                        };
                        if let Err(err) = tx.send_async(ListenRead::Event((ts, data))).await {
                            debug!("Client /listen reader: handler gone, dropping event: {:?}", err);
                        }
                    }
                    OpCode::Close => break,
                    // keepalive traffic from the server (auto-pong of our pings): a liveness marker so
                    // the handler can enforce the read deadline. Ping frames are normally consumed by
                    // auto_pong and never reach here.
                    OpCode::Ping | OpCode::Pong => {
                        let _ = tx.send_async(ListenRead::Liveness).await;
                    }
                    _ => {}
                }
            }

            debug!("Exiting Client /listen Reader");
        }

        async fn connect(
            leader_cache: &Arc<RwLock<(NodeId, String)>>,
            tls_config: Option<Arc<rustls::ClientConfig>>,
            api_secret: &str,
        ) -> Result<WebSocket<TokioIo<Upgraded>>, Error> {
            let (node_id, addr) = {
                let lock = leader_cache.read().await;
                (lock.0, lock.1.clone())
            };
            web_socket_connect::try_connect(node_id, &addr, "/listen", tls_config, api_secret.as_bytes())
                .await
        }
    }
}

impl Client {
    /// Listen to events on the distributed event bus
    pub async fn listen<T>(&self) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        let (_ts, bytes) = self.listen_rx().recv_async().await?;
        Ok(deserialize_serde(&bytes)?)
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
            Ok(Some(deserialize_serde(&bytes)?))
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
                return Ok(deserialize_serde(&bytes)?);
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
        self.rate_limit_cache().await?;
        let now = Utc::now().timestamp_micros();

        match self
            .notify_req(CacheRequest::Notify((now, serialize_serde(payload).expect("Network payload serialization should always succeed"))))
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
                    self.notify_req(CacheRequest::Notify((now, serialize_serde(payload).expect("Network payload serialization should always succeed"))))
                        .await
                } else {
                    Err(err)
                }
            }
        }
    }

    pub(crate) async fn notify_req(&self, cache_req: CacheRequest) -> Result<(), Error> {
        if let Some(state) = self.is_leader_cache_with_state().await {
            Self::client_write_local(&state.raft_cache.raft, cache_req).await?;
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
                .map_err(|err| Error::Error(err.to_string().into()))?;
            let res = await_channel_response(rx).await??;
            match res {
                ApiStreamResponsePayload::Notify(res) => res,
                _ => unreachable!(),
            }
        }
    }
}
