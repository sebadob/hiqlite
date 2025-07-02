use crate::app_state::RaftType;
use crate::helpers::deserialize;
use crate::network::api::{ApiStreamResponse, ApiStreamResponsePayload};
use crate::network::handshake::HandshakeSecret;
use crate::network::serialize_network;
use crate::{tls, Client, Error, Node, NodeId};
use axum::http::header::{CONNECTION, UPGRADE};
use axum::http::Request;
use bytes::Bytes;
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, WebSocket, WebSocketWrite};
use http_body_util::Empty;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;
use tokio::{select, task, time};
use tracing::{debug, error, info, warn};

#[cfg(any(feature = "sqlite", feature = "cache"))]
use crate::network::api::{ApiStreamRequest, ApiStreamRequestPayload};
#[cfg(feature = "cache")]
use crate::store::state_machine::memory::state_machine::CacheRequest;
#[cfg(feature = "sqlite")]
use crate::{migration::Migration, store::state_machine::sqlite::state_machine::Query};

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
}

impl Client {
    pub(crate) fn open_stream(
        &self,
        // client: Client,
        // tls_config: Option<Arc<rustls::ClientConfig>>,
        secret: Vec<u8>,
        leader: Arc<RwLock<(NodeId, String)>>,
        rx_client_stream: flume::Receiver<ClientStreamReq>,
        raft_type: RaftType,
    ) {
        task::spawn(client_stream(
            self.clone(),
            secret,
            leader,
            rx_client_stream,
            raft_type,
        ));
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

                time::sleep(Duration::from_millis(1000)).await;
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

        loop {
            let res = select! {
                res = rx_read.recv_async() => res,
                res = rx_req.recv_async() => res,
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
                    try_forward_response(
                        &mut in_flight,
                        &mut in_flight_buf,
                        awaiting_timeout,
                        resp,
                    )
                    .await;
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
            }
        }

        if shutdown {
            warn!("Shutting down Client stream receiver");
            break;
        }

        for (req_id, ack) in in_flight.drain() {
            in_flight_buf.insert(req_id, ack);
        }
        assert!(in_flight.is_empty());

        info!("client stream tasks killed - re-connecting now");
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
    if node_id.is_some() && node.is_some() {
        let api_addr = node.as_ref().unwrap().addr_api.clone();
        let leader_id = node_id.unwrap();
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
                let payload = deserialize::<ApiStreamResponse>(bytes).unwrap();
                if let Err(err) = tx
                    .send_async(ClientStreamReq::StreamResponse(payload))
                    .await
                {
                    error!("Error sending Response to Client Stream Manager: {:?}", err);
                }
            }
            OpCode::Close => break,
            OpCode::Ping => {}
            OpCode::Pong => {}
        }
    }

    warn!("Exiting Client Stream Reader");
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
            WritePayload::Close => {
                warn!("Received Close request in Client Stream Writer");
                let _ = write.write_frame(Frame::close(1000, b"go away")).await;
                break;
            }
        }
    }

    warn!("Exiting Client Stream Writer");
}

struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
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

    let scheme = if tls_config.is_some() {
        "https"
    } else {
        "http"
    };
    let uri = format!("{}://{}/stream/{}", scheme, addr, raft_type.as_str());
    info!("Client API WebSocket trying to connect to: {}", uri);

    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
        )
        .header("Sec-WebSocket-Version", "13")
        .body(Empty::<Bytes>::new())
        .map_err(|err| Error::Error(err.to_string().into()))?;

    let stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(err) => {
            return Err(Error::Connect(err.to_string()));
        }
    };

    let (mut ws, _) = match if let Some(config) = tls_config {
        let (addr, _) = addr.split_once(':').unwrap_or((&addr, ""));
        let tls_stream = tls::into_tls_stream(addr, stream, config).await?;
        fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await
    } else {
        fastwebsockets::handshake::client(&SpawnExecutor, req, stream).await
    } {
        Ok(conn) => conn,
        Err(err) => {
            return Err(Error::Connect(err.to_string()));
        }
    };

    if let Err(err) = HandshakeSecret::client(&mut ws, secret, node_id).await {
        let _ = ws
            .write_frame(Frame::close(1000, b"Invalid Handshake"))
            .await;
        // panic is the best option in case of a misconfiguration
        panic!("Error during API WebSocket handshake: {err}");
    }

    Ok(ws)
}
