use crate::app_state::RaftType;
use crate::helpers::deserialize;
use crate::network::api::{ApiStreamResponse, ApiStreamResponsePayload};
use crate::network::{serialize_network, web_socket_connect};
use crate::{Client, Error, Node, NodeId};
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, WebSocket, WebSocketWrite};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::sync::oneshot::Sender;
use tokio::sync::{RwLock, oneshot};
use tokio::task::JoinHandle;
use tokio::{select, task, time};
use tracing::{debug, error, info};

#[cfg(any(feature = "sqlite", feature = "cache"))]
use crate::network::api::{ApiStreamRequest, ApiStreamRequestPayload};
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::state_machine::CacheRequest;
#[cfg(feature = "sqlite")]
use crate::{migration::Migration, store::state_machine::sqlite::state_machine::Query};

/// Interval between client-initiated keepalive pings on the API WebSocket. The server auto-pongs
/// every ping (fastwebsockets default), so a healthy idle connection always produces inbound
/// traffic within this interval, and a half-open connection eventually fails these writes.
const STREAM_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// Maximum time without any frame from the server before the client declares the connection dead
/// and reconnects. A healthy connection always receives the auto-pong of our keepalive pings, so
/// this only trips on genuinely stalled connections. Must be a multiple of
/// STREAM_KEEPALIVE_INTERVAL with margin.
const STREAM_READ_TIMEOUT: Duration = Duration::from_secs(35);

/// Number of consecutive connection failures that retry at the fast (1 s) interval before the
/// reconnect loop backs off to RECONNECT_SLOW_INTERVAL. A restarting node is only briefly
/// unreachable, so a handful of quick attempts is enough; anything more just delays reconnection.
const RECONNECT_FAST_RETRIES: u32 = 5;

/// Interval between connection attempts once RECONNECT_FAST_RETRIES consecutive failures have been
/// exhausted. Kept short on purpose so reconnection stays responsive; do not grow it further.
const RECONNECT_SLOW_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) enum ClientStreamReq {
    // coming from the `DbClient`
    #[cfg(feature = "sqlite")]
    Execute(ClientExecutePayload),
    #[cfg(feature = "sqlite")]
    ExecuteReturning(ClientExecutePayload),
    #[cfg(feature = "sqlite")]
    Transaction(ClientTransactionPayload),
    #[cfg(feature = "sqlite")]
    Query(ClientQueryPayload),
    #[cfg(feature = "sqlite")]
    QueryConsistent(ClientQueryPayload),
    #[cfg(feature = "sqlite")]
    Batch(ClientBatchPayload),
    #[cfg(feature = "sqlite")]
    Migrate(ClientMigratePayload),

    #[cfg(feature = "backup")]
    Backup(ClientBackupPayload),

    #[cfg(feature = "cache")]
    KV(ClientKVPayload),
    #[cfg(feature = "cache")]
    KVGet(ClientKVPayload),

    #[cfg(feature = "dlock")]
    LockAwait(ClientKVPayload),

    #[cfg(feature = "listen_notify_local")]
    Notify(ClientKVPayload),

    Shutdown,

    // coming from the WebSocket reader
    StreamResponse(ApiStreamResponse),
    CleanupBuffer,
    /// keepalive ping/pong traffic from the server; proves the connection is alive
    Liveness,

    // may come from `DbClient` or WebSocket reader
    LeaderChange((Option<u64>, Option<Node>)),
}

#[cfg(feature = "sqlite")]
#[derive(Debug)]
pub struct ClientExecutePayload {
    pub request_id: usize,
    pub sql: Query,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[cfg(feature = "sqlite")]
#[derive(Debug)]
pub struct ClientTransactionPayload {
    pub request_id: usize,
    pub queries: Vec<Query>,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[cfg(feature = "sqlite")]
#[derive(Debug)]
pub struct ClientQueryPayload {
    pub request_id: usize,
    pub query: Query,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[cfg(feature = "sqlite")]
#[derive(Debug)]
pub struct ClientBatchPayload {
    pub request_id: usize,
    pub sql: std::borrow::Cow<'static, str>,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[cfg(feature = "sqlite")]
#[derive(Debug)]
pub struct ClientMigratePayload {
    pub request_id: usize,
    pub migrations: Vec<Migration>,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[cfg(feature = "backup")]
#[derive(Debug)]
pub struct ClientBackupPayload {
    pub request_id: usize,
    pub node_id: NodeId,
    pub ts: i64,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[cfg(feature = "cache")]
#[derive(Debug)]
pub struct ClientKVPayload {
    pub request_id: usize,
    pub cache_req: CacheRequest,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[derive(Debug)]
enum WritePayload {
    Payload(Vec<u8>),
    Close,
    Ping,
}

impl Client {
    pub(crate) fn open_stream(
        &self,
        secret: Vec<u8>,
        leader: Arc<RwLock<(NodeId, String)>>,
        rx_client_stream: flume::Receiver<ClientStreamReq>,
        raft_type: RaftType,
    ) {
        task::spawn(Box::pin(client_stream(
            self.clone(),
            secret,
            leader,
            rx_client_stream,
            raft_type,
        )));
    }
}

/// Manager task which handles connection creation, split into sender / receiver, keeps the state,
/// handles reconnects and leader switches.
async fn client_stream(
    client: Client,
    secret: Vec<u8>,
    leader: Arc<RwLock<(NodeId, String)>>,
    rx_req: flume::Receiver<ClientStreamReq>,
    raft_type: RaftType,
) {
    let mut in_flight: HashMap<usize, oneshot::Sender<Result<ApiStreamResponsePayload, Error>>> =
        HashMap::with_capacity(8);
    let mut in_flight_buf: HashMap<
        usize,
        oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
    > = HashMap::new();

    let mut shutdown = false;
    let mut failed_retries: u32 = 0;

    loop {
        let ws = match try_connect(
            &leader,
            &raft_type,
            client.inner.tls_config.clone(),
            &secret,
        )
        .await
        {
            Ok(ws) => {
                failed_retries = 0;
                info!(
                    "Client API WebSocket to {} opened successfully",
                    leader.read().await.1
                );
                ws
            }
            Err(err) => {
                if let Error::Connect(_) = &err {
                    // TODO keep track if we are connected through a proxy and skip ?
                    client.find_set_active_leader().await;
                }

                // Back off after RECONNECT_FAST_RETRIES consecutive fast (1 s) retries so a
                // restarting node does not spam the log, but keep the window short so reconnection
                // stays responsive.
                let interval = if failed_retries < RECONNECT_FAST_RETRIES {
                    Duration::from_millis(1000)
                } else {
                    RECONNECT_SLOW_INTERVAL
                };
                failed_retries += 1;
                time::sleep(interval).await;
                error!(
                    "Could not connect Client API WebSocket to {}: {}",
                    leader.read().await.1,
                    err
                );
                continue;
            }
        };

        let (tx_write, rx_write) = flume::bounded(1);
        let (tx_read, rx_read) = flume::bounded(1);

        // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
        let (rx, write) = ws.split(tokio::io::split);
        // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
        let read = FragmentCollectorRead::new(rx);

        let handle_read = task::spawn(stream_reader(read, tx_read.clone()));
        let handle_write = task::spawn(stream_writer(write, rx_write));

        let handle_buf = cleanup_buffer_timeout(tx_read, 10);
        let mut awaiting_timeout = true;

        // Liveness tracking: any frame from the server resets `last_frame`. A healthy connection
        // always produces inbound traffic within STREAM_READ_TIMEOUT because we ping every
        // STREAM_KEEPALIVE_INTERVAL and the server auto-pongs, even while requests are in flight.
        let mut last_frame = time::Instant::now();
        let mut keepalive = time::interval(STREAM_KEEPALIVE_INTERVAL);

        loop {
            let res: Option<Result<ClientStreamReq, flume::RecvError>> = select! {
                res = rx_read.recv_async() => Some(res),
                res = rx_req.recv_async() => Some(res),
                _ = keepalive.tick() => {
                    // Client-initiated ping: makes a half-open connection fail its writes, and the
                    // server's auto-pong gives us inbound traffic to measure.
                    if let Err(err) = tx_write.send_async(WritePayload::Ping).await {
                        error!("Error sending keepalive ping to writer: {}", err);
                        for (_, ack) in in_flight.drain() {
                            let _ = ack.send(Err(Error::Connect(
                                "Connection to Raft leader lost".into(),
                            )));
                        }
                        break;
                    }
                    None
                }
                _ = time::sleep(STREAM_READ_TIMEOUT.saturating_sub(last_frame.elapsed())) => {
                    error!(
                        "No frames from server for {:?} - assuming dead connection, reconnecting",
                        STREAM_READ_TIMEOUT
                    );
                    for (_, ack) in in_flight.drain() {
                        let _ = ack.send(Err(Error::Connect(
                            "Connection to Raft leader lost".into(),
                        )));
                    }
                    break;
                }
            };

            let Some(res) = res else {
                continue;
            };

            let req = match res {
                Ok(req) => req,
                Err(err) => {
                    error!("Client stream reader error: {}", err,);
                    if rx_req.is_disconnected() {
                        let _ = tx_write.send_async(WritePayload::Close).await;
                        shutdown = true;
                    }
                    break;
                }
            };

            let payload = match req {
                #[cfg(feature = "sqlite")]
                ClientStreamReq::Execute(ClientExecutePayload {
                    request_id,
                    sql,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Execute(sql),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        req.request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "sqlite")]
                ClientStreamReq::ExecuteReturning(ClientExecutePayload {
                    request_id,
                    sql,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::ExecuteReturning(sql),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "sqlite")]
                ClientStreamReq::Transaction(ClientTransactionPayload {
                    request_id,
                    queries,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Transaction(queries),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "sqlite")]
                ClientStreamReq::Query(ClientQueryPayload {
                    request_id,
                    query,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Query(query),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "sqlite")]
                ClientStreamReq::QueryConsistent(ClientQueryPayload {
                    request_id,
                    query,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::QueryConsistent(query),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "sqlite")]
                ClientStreamReq::Batch(ClientBatchPayload {
                    request_id,
                    sql,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Batch(sql),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "sqlite")]
                ClientStreamReq::Migrate(ClientMigratePayload {
                    request_id,
                    migrations,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Migrate(migrations),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "backup")]
                ClientStreamReq::Backup(ClientBackupPayload {
                    request_id,
                    node_id,
                    ts,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Backup((node_id, ts)),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "cache")]
                ClientStreamReq::KV(ClientKVPayload {
                    request_id,
                    cache_req,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::KV(cache_req),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "cache")]
                ClientStreamReq::KVGet(ClientKVPayload {
                    request_id,
                    cache_req,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::KVGet(cache_req),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "dlock")]
                ClientStreamReq::LockAwait(ClientKVPayload {
                    request_id,
                    cache_req,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::LockAwait(cache_req),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                #[cfg(feature = "listen_notify_local")]
                ClientStreamReq::Notify(ClientKVPayload {
                    request_id,
                    cache_req,
                    ack,
                }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Notify(cache_req),
                    };
                    Some((
                        WritePayload::Payload(serialize_network(&req)),
                        request_id,
                        ack,
                    ))
                }

                ClientStreamReq::LeaderChange((node_id, node)) => {
                    // ignore result just in case the writer has already exited anyway
                    let _ = tx_write.send_async(WritePayload::Close).await;

                    // If we don't receive a value here, we expect the lock to
                    // have been updated already somewhere else
                    update_leader(&leader, node_id, node).await;

                    // in case of a leader change, we should not use the in flight buffer
                    // since no modifying write after this error will be Ok(_) anyway.
                    for (_, ack) in in_flight.drain() {
                        let _ = ack.send(Err(Error::LeaderChange(
                            "Action not allowed, Raft leader has changed".into(),
                        )));
                    }
                    break;
                }

                ClientStreamReq::StreamResponse(resp) => {
                    last_frame = time::Instant::now();
                    try_forward_response(
                        &mut in_flight,
                        &mut in_flight_buf,
                        awaiting_timeout,
                        resp,
                    )
                    .await;
                    None
                }

                ClientStreamReq::Liveness => {
                    // keepalive pong from the server: connection is alive
                    last_frame = time::Instant::now();
                    None
                }

                ClientStreamReq::CleanupBuffer => {
                    for (_, ack) in in_flight_buf {
                        let _ = ack.send(Err(Error::Connect("request timed out".to_string())));
                    }
                    in_flight_buf = HashMap::new();
                    awaiting_timeout = false;
                    None
                }

                ClientStreamReq::Shutdown => {
                    shutdown = true;
                    break;
                }
            };

            if let Some((payload, request_id, ack)) = payload {
                match tx_write.send_async(payload).await {
                    Ok(_) => {
                        in_flight.insert(request_id, ack);
                    }
                    Err(err) => {
                        error!("Error sending txn request to writer: {}", err);
                        let _ =
                            ack.send(Err(Error::Connect("Connection to Raft leader lost".into())));
                        break;
                    }
                }
            }
        }

        handle_buf.abort();
        handle_write.abort();
        handle_read.abort();

        debug!("make sure reader rx is empty and closed");
        while let Ok(req) = rx_read.recv_async().await {
            debug!("Answer from reader into buffer: {:?}", req);
            // we are very explicit here for better debugging
            match req {
                #[cfg(feature = "sqlite")]
                ClientStreamReq::Execute(_) => {
                    unreachable!("we should never receive ClientStreamReq::Execute from WS reader")
                }
                #[cfg(feature = "sqlite")]
                ClientStreamReq::ExecuteReturning(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::ExecuteReturning from WS reader"
                    )
                }
                #[cfg(feature = "sqlite")]
                ClientStreamReq::Transaction(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::Transaction from WS reader"
                    )
                }
                #[cfg(feature = "sqlite")]
                ClientStreamReq::Query(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::QueryConsistent from WS reader"
                    )
                }
                #[cfg(feature = "sqlite")]
                ClientStreamReq::QueryConsistent(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::QueryConsistent from WS reader"
                    )
                }
                #[cfg(feature = "sqlite")]
                ClientStreamReq::Batch(_) => {
                    unreachable!("we should never receive ClientStreamReq::Batch from WS reader")
                }
                #[cfg(feature = "sqlite")]
                ClientStreamReq::Migrate(_) => {
                    unreachable!("we should never receive ClientStreamReq::Migrate from WS reader")
                }
                #[cfg(feature = "backup")]
                ClientStreamReq::Backup(_) => {
                    unreachable!("we should never receive ClientStreamReq::Backup from WS reader")
                }
                #[cfg(feature = "cache")]
                ClientStreamReq::KV(_) => {
                    unreachable!("we should never receive ClientStreamReq::KV from WS reader")
                }
                #[cfg(feature = "cache")]
                ClientStreamReq::KVGet(_) => {
                    unreachable!("we should never receive ClientStreamReq::KVGet from WS reader")
                }
                #[cfg(feature = "dlock")]
                ClientStreamReq::LockAwait(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::LockAwait from WS reader"
                    )
                }
                #[cfg(feature = "listen_notify_local")]
                ClientStreamReq::Notify(_) => {
                    unreachable!("we should never receive ClientStreamReq::Notify from WS reader")
                }
                ClientStreamReq::Shutdown => {
                    unreachable!("we should never receive ClientStreamReq::Shutdown from WS reader")
                }
                ClientStreamReq::LeaderChange((node_id, node)) => {
                    update_leader(&leader, node_id, node).await;
                }
                ClientStreamReq::StreamResponse(resp) => {
                    try_forward_response(&mut in_flight, &mut in_flight_buf, false, resp).await;
                }
                ClientStreamReq::CleanupBuffer => {
                    // ignore - we are re-connecting anyway
                }
                ClientStreamReq::Liveness => {
                    // ignore - we are re-connecting anyway
                }
            }
        }

        if shutdown {
            debug!("Shutting down Client stream receiver");
            break;
        }

        for (req_id, ack) in in_flight.drain() {
            in_flight_buf.insert(req_id, ack);
        }
        // drain() already empties the map, so this is a debug-only sanity check.
        debug_assert!(in_flight.is_empty());

        debug!("client stream tasks killed - re-connecting now");
    }
}

#[inline(always)]
async fn try_forward_response(
    in_flight: &mut HashMap<usize, Sender<Result<ApiStreamResponsePayload, Error>>>,
    in_flight_buf: &mut HashMap<usize, Sender<Result<ApiStreamResponsePayload, Error>>>,
    awaiting_timeout: bool,
    response: ApiStreamResponse,
) {
    match in_flight.remove(&response.request_id) {
        None => {
            if awaiting_timeout {
                match in_flight_buf.remove(&response.request_id) {
                    None => {
                        error!("client ack for ApiStreamResponse missing");
                    }
                    Some(ack) => match ack.send(Ok(response.result)) {
                        Ok(_) => {
                            debug!("ApiStreamResponse sent to client from in_flight_buf");
                        }
                        Err(err) => {
                            error!("client ack could not be sent for {:?}", err);
                        }
                    },
                }
            } else {
                error!("client ack for ApiStreamResponse missing");
            }
        }

        Some(ack) => match ack.send(Ok(response.result)) {
            Ok(_) => {
                debug!("ApiStreamResponse sent to client");
            }
            Err(err) => {
                error!("client ack could not be sent for {:?}", err);
            }
        },
    }
}

async fn update_leader(
    leader: &Arc<RwLock<(NodeId, String)>>,
    node_id: Option<u64>,
    node: Option<Node>,
) {
    if let Some(leader_id) = node_id
        && let Some(node) = node
    {
        let api_addr = node.addr_api.clone();
        info!(
            "API Client received a Leader Change: {} / {}",
            leader_id, api_addr
        );
        {
            let mut lock = leader.write().await;
            *lock = (leader_id, api_addr);
        }
    }
}

fn cleanup_buffer_timeout(tx: flume::Sender<ClientStreamReq>, seconds: u64) -> JoinHandle<()> {
    task::spawn(async move {
        time::sleep(Duration::from_secs(seconds)).await;
        let _ = tx.send_async(ClientStreamReq::CleanupBuffer).await;
    })
}

async fn stream_reader(
    mut read: FragmentCollectorRead<ReadHalf<TokioIo<Upgraded>>>,
    tx: flume::Sender<ClientStreamReq>,
) {
    while let Ok(frame) = read
        .read_frame(&mut |frame| async move {
            // TODO obligated sends should be auto ping / pong / close ? -> verify!
            debug!(
                "Received obligated send in stream client: OpCode: {:?}: {:?}",
                frame.opcode.clone(),
                frame.payload
            );
            Ok::<(), Error>(())
        })
        .await
    {
        match frame.opcode {
            OpCode::Continuation => {}
            OpCode::Text => {}
            OpCode::Binary => {
                let bytes = frame.payload.deref();
                let payload = match deserialize::<ApiStreamResponse>(bytes) {
                    Ok(payload) => payload,
                    Err(err) => {
                        // corrupt frame from the server: do not panic the reader task, just
                        // drop the connection so the client can reconnect
                        error!("Error deserializing response from server: {err:?}");
                        break;
                    }
                };
                if let Err(err) = tx
                    .send_async(ClientStreamReq::StreamResponse(payload))
                    .await
                {
                    error!("Error sending Response to Client Stream Manager: {:?}", err);
                }
            }
            OpCode::Close => break,
            // keepalive traffic from the server (auto-pong of our pings): forward it as a liveness
            // marker so the manager can enforce the read deadline. Ping frames are normally
            // consumed by auto_pong and never reach here.
            OpCode::Ping | OpCode::Pong => {
                if let Err(err) = tx.send_async(ClientStreamReq::Liveness).await {
                    error!("Error sending Liveness to Client Stream Manager: {:?}", err);
                }
            }
        }
    }

    debug!("Exiting Client Stream Reader");
}

async fn stream_writer(
    mut write: WebSocketWrite<WriteHalf<TokioIo<Upgraded>>>,
    rx: flume::Receiver<WritePayload>,
) {
    while let Ok(payload) = rx.recv_async().await {
        match payload {
            WritePayload::Payload(bytes) => {
                let frame = Frame::binary(Payload::from(bytes));
                if let Err(err) = write.write_frame(frame).await {
                    error!("Client Stream error: {:?}", err);
                    break;
                }
            }
            WritePayload::Ping => {
                // keepalive ping; the server auto-pongs it (fastwebsockets default)
                let frame = Frame::new(true, OpCode::Ping, None, Payload::from(vec![]));
                if let Err(err) = write.write_frame(frame).await {
                    error!("Client Stream keepalive ping error: {:?}", err);
                    break;
                }
            }
            WritePayload::Close => {
                debug!("Received Close request in Client Stream Writer");
                let _ = write.write_frame(Frame::close(1000, b"go away")).await;
                break;
            }
        }
    }

    debug!("Exiting Client Stream Writer");
}

async fn try_connect(
    leader: &Arc<RwLock<(NodeId, String)>>,
    raft_type: &RaftType,
    tls_config: Option<Arc<rustls::ClientConfig>>,
    secret: &[u8],
) -> Result<WebSocket<TokioIo<Upgraded>>, Error> {
    let (node_id, addr) = {
        let lock = leader.read().await;
        (lock.0, lock.1.clone())
    };
    web_socket_connect::try_connect(node_id, &addr, raft_type, tls_config, secret).await
}
