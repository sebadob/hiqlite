use crate::migration::Migration;
use crate::network::api::{
    ApiStreamRequest, ApiStreamRequestPayload, ApiStreamResponse, ApiStreamResponsePayload,
};
use crate::network::handshake::HandshakeSecret;
use crate::store::state_machine::sqlite::state_machine::Query;
use crate::{tls, DbClient, Error, Node, NodeId};
use axum::http::header::{CONNECTION, UPGRADE};
use axum::http::Request;
use bytes::Bytes;
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, WebSocket, WebSocketWrite};
use http_body_util::Empty;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use std::borrow::Cow;
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

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::state_machine::CacheRequest;

#[derive(Debug)]
pub enum ClientStreamReq {
    // coming from the `DbClient`
    Execute(ClientExecutePayload),
    ExecuteReturning(ClientExecutePayload),
    Transaction(ClientTransactionPayload),
    QueryConsistent(ClientQueryConsistentPayload),
    Batch(ClientBatchPayload),
    Migrate(ClientMigratePayload),
    Backup(ClientBackupPayload),
    Shutdown,

    #[cfg(feature = "cache")]
    KV(ClientKVPayload),

    // coming from the WebSocket reader
    StreamResponse(ApiStreamResponse),
    CleanupBuffer,

    // may come from `DbClient` or WebSocket reader
    LeaderChange((Option<u64>, Option<Node>)),
}

#[derive(Debug)]
pub struct ClientExecutePayload {
    pub request_id: usize,
    pub sql: Query,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[derive(Debug)]
pub struct ClientTransactionPayload {
    pub request_id: usize,
    pub queries: Vec<Query>,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[derive(Debug)]
pub struct ClientQueryConsistentPayload {
    pub request_id: usize,
    pub query: Query,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[derive(Debug)]
pub struct ClientBatchPayload {
    pub request_id: usize,
    pub sql: Cow<'static, str>,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[derive(Debug)]
pub struct ClientMigratePayload {
    pub request_id: usize,
    pub migrations: Vec<Migration>,
    pub ack: oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
}

#[derive(Debug)]
pub struct ClientBackupPayload {
    pub request_id: usize,
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

impl DbClient {
    pub(crate) fn open_stream(
        node_id: NodeId,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        secret: Vec<u8>,
        leader: Arc<RwLock<(NodeId, String)>>,
    ) -> flume::Sender<ClientStreamReq> {
        // TODO option like "limit in-flight requests"
        let (tx, rx) = flume::unbounded();
        task::spawn(client_stream(node_id, tls_config, secret, leader, rx));
        tx
    }
}

/// Manager task which handles connection creation, split into sender / receiver, keeps the state,
/// handles reconnects and leader switches.
async fn client_stream(
    node_id: NodeId,
    tls_config: Option<Arc<rustls::ClientConfig>>,
    secret: Vec<u8>,
    leader: Arc<RwLock<(NodeId, String)>>,
    rx_req: flume::Receiver<ClientStreamReq>,
) {
    // HashMap vs Vec here: map in favor if we have typically more than ~20 in flight requests
    let mut in_flight: HashMap<usize, oneshot::Sender<Result<ApiStreamResponsePayload, Error>>> =
        HashMap::new();
    let mut in_flight_buf: HashMap<
        usize,
        oneshot::Sender<Result<ApiStreamResponsePayload, Error>>,
    > = HashMap::new();

    let mut shutdown = false;

    loop {
        let ws = match try_connect(
            node_id,
            &leader.read().await.1.clone(),
            tls_config.clone(),
            &secret,
        )
        .await
        {
            None => {
                time::sleep(Duration::from_millis(250)).await;
                warn!(
                    "Could not connect Client API WebSocket to {}",
                    leader.read().await.1
                );
                continue;
            }
            Some(ws) => {
                info!(
                    "Client API WebSocket to {} opened successfully",
                    leader.read().await.1
                );
                ws
            }
        };

        // TODO should we make this bounded to prevent stuff like overloading the leader?
        let (tx_write, rx_write) = flume::unbounded();
        let (tx_read, rx_read) = flume::unbounded();

        // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
        let (rx, write) = ws.split(tokio::io::split);
        // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
        let read = FragmentCollectorRead::new(rx);

        let handle_read = task::spawn(stream_reader(read, tx_read.clone()));
        let handle_write = task::spawn(stream_writer(write, rx_write));

        let handle_buf = cleanup_buffer_timeout(tx_read, 10);
        let mut awaiting_timeout = true;

        // default working loop
        loop {
            let res = select! {
                res = rx_read.recv_async() => res,
                res = rx_req.recv_async() => res,
            };
            let req = match res {
                Ok(req) => req,
                Err(err) => {
                    // We end up here if the WS Reader errored and closed the sender
                    error!("Client stream reader error: {}", err);
                    break;
                }
            };

            match req {
                ClientStreamReq::Execute(exec) => {
                    let req = ApiStreamRequest {
                        request_id: exec.request_id,
                        payload: ApiStreamRequestPayload::Execute(exec.sql),
                    };

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(exec.request_id, exec.ack);
                        }
                        Err(err) => {
                            error!("Error sending request to writer: {}", err);
                            let _ = exec
                                .ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
                }

                ClientStreamReq::ExecuteReturning(exec) => {
                    let req = ApiStreamRequest {
                        request_id: exec.request_id,
                        payload: ApiStreamRequestPayload::ExecuteReturning(exec.sql),
                    };

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(exec.request_id, exec.ack);
                        }
                        Err(err) => {
                            error!("Error sending request to writer: {}", err);
                            let _ = exec
                                .ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
                }

                ClientStreamReq::Transaction(txn) => {
                    let req = ApiStreamRequest {
                        request_id: txn.request_id,
                        payload: ApiStreamRequestPayload::Transaction(txn.queries),
                    };

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(txn.request_id, txn.ack);
                        }
                        Err(err) => {
                            error!("Error sending txn request to writer: {}", err);
                            let _ = txn
                                .ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
                }

                ClientStreamReq::QueryConsistent(query) => {
                    let req = ApiStreamRequest {
                        request_id: query.request_id,
                        payload: ApiStreamRequestPayload::QueryConsistent(query.query),
                    };

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(query.request_id, query.ack);
                        }
                        Err(err) => {
                            error!("Error sending txn request to writer: {}", err);
                            let _ = query
                                .ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
                }

                ClientStreamReq::Batch(batch) => {
                    let req = ApiStreamRequest {
                        request_id: batch.request_id,
                        payload: ApiStreamRequestPayload::Batch(batch.sql),
                    };

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(req.request_id, batch.ack);
                        }
                        Err(err) => {
                            error!("Error sending txn request to writer: {}", err);
                            let _ = batch
                                .ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
                }

                ClientStreamReq::Migrate(migrate) => {
                    let req = ApiStreamRequest {
                        request_id: migrate.request_id,
                        payload: ApiStreamRequestPayload::Migrate(migrate.migrations),
                    };

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(req.request_id, migrate.ack);
                        }
                        Err(err) => {
                            error!("Error sending txn request to writer: {}", err);
                            let _ = migrate
                                .ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
                }

                ClientStreamReq::Backup(ClientBackupPayload { request_id, ack }) => {
                    let req = ApiStreamRequest {
                        request_id,
                        payload: ApiStreamRequestPayload::Backup,
                    };

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(req.request_id, ack);
                        }
                        Err(err) => {
                            error!("Error sending txn request to writer: {}", err);
                            let _ = ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
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

                    match tx_write
                        .send_async(WritePayload::Payload(bincode::serialize(&req).unwrap()))
                        .await
                    {
                        Ok(_) => {
                            in_flight.insert(req.request_id, ack);
                        }
                        Err(err) => {
                            error!("Error sending txn request to writer: {}", err);
                            let _ = ack
                                .send(Err(Error::Connect("Connection to Raft leader lost".into())));
                            break;
                        }
                    }
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
                }

                ClientStreamReq::CleanupBuffer => {
                    in_flight_buf = HashMap::new();
                    awaiting_timeout = false;
                }

                ClientStreamReq::Shutdown => {
                    shutdown = true;
                    break;
                }
            }
        }

        handle_buf.abort();
        handle_write.abort();
        handle_read.abort();

        info!("make sure reader rx is empty and closed");
        while let Ok(req) = rx_read.recv_async().await {
            info!("Answer from reader into buffer: {:?}", req);
            match req {
                ClientStreamReq::Execute(_) => {
                    unreachable!("we should never receive ClientStreamReq::Execute from WS reader")
                }
                ClientStreamReq::ExecuteReturning(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::ExecuteReturning from WS reader"
                    )
                }
                ClientStreamReq::Transaction(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::Transaction from WS reader"
                    )
                }
                ClientStreamReq::QueryConsistent(_) => {
                    unreachable!(
                        "we should never receive ClientStreamReq::QueryConsistent from WS reader"
                    )
                }
                ClientStreamReq::Batch(_) => {
                    unreachable!("we should never receive ClientStreamReq::Batch from WS reader")
                }
                ClientStreamReq::Migrate(_) => {
                    unreachable!("we should never receive ClientStreamReq::Migrate from WS reader")
                }
                ClientStreamReq::Backup(_) => {
                    unreachable!("we should never receive ClientStreamReq::Backup from WS reader")
                }
                #[cfg(feature = "cache")]
                ClientStreamReq::KV(_) => {
                    unreachable!("we should never receive ClientStreamReq::KV from WS reader")
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
                    // ignore in this case
                }
            }
        }

        if shutdown {
            warn!("Shutting down Client stream receiver");
            break;
        }

        // copy all existing in-flight to in-flight buffer to make sure we use them first
        info!("copy all existing in-flight to in-flight buffer to make sure we use them first");
        for (req_id, ack) in in_flight.drain() {
            in_flight_buf.insert(req_id, ack);
        }

        info!("tasks killed - re-connect");
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
            warn!(
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
                let payload = bincode::deserialize::<ApiStreamResponse>(bytes).unwrap();
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
    node_id: NodeId,
    addr: &str,
    tls_config: Option<Arc<rustls::ClientConfig>>,
    secret: &[u8],
) -> Option<WebSocket<TokioIo<Upgraded>>> {
    let uri = if tls_config.is_some() {
        format!("https://{}/stream", addr)
    } else {
        format!("http://{}/stream", addr)
    };

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
        .ok()?;

    let stream = TcpStream::connect(addr).await.ok()?;
    let (mut ws, _) = if let Some(config) = tls_config {
        let (addr, _) = addr.split_once(':').unwrap_or((addr, ""));
        let tls_stream = match tls::into_tls_stream(addr, stream, config).await {
            Ok(s) => s,
            Err(err) => {
                error!("Error opening TLS stream to {}: {}", addr, err);
                return None;
            }
        };

        match fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await {
            Ok((ws, r)) => (ws, r),
            Err(err) => {
                error!("{}", err);
                return None;
            }
        }
    } else {
        fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
            .await
            .ok()?
    };

    if let Err(err) = HandshakeSecret::client(&mut ws, secret, node_id).await {
        let _ = ws
            .write_frame(Frame::close(1000, b"Invalid Handshake"))
            .await;
        // TODO what should be do in this case? This handler should never exit.
        // panic is the best option in case of misconfiguration?
        panic!("Error during API WebSocket handshake: {}", err);
    }

    Some(ws)
}