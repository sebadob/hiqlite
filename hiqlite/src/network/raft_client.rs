use crate::network::handshake::HandshakeSecret;
use crate::network::raft_server::{
    RaftStreamRequest, RaftStreamResponse, RaftStreamResponsePayload,
};
use crate::Node;
use crate::{tls, NodeId};
use bytes::Bytes;
use fastwebsockets::{FragmentCollectorRead, Frame, OpCode, Payload, WebSocket, WebSocketWrite};
use http_body_util::Empty;
use hyper::header::{CONNECTION, UPGRADE};
use hyper::upgrade::Upgraded;
use hyper::Request;
use hyper_util::rt::TokioIo;
use openraft::error::RPCError;
use openraft::error::Unreachable;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::{select, task, time};
use tracing::{error, info, warn};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

use crate::app_state::RaftType;
use crate::helpers::{deserialize, serialize};
#[cfg(any(feature = "cache", feature = "sqlite"))]
use crate::Error;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::{
    error::{InstallSnapshotError, RaftError},
    network::{RPCOption, RaftNetwork, RaftNetworkFactory},
    raft::{
        AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
        InstallSnapshotResponse, VoteRequest, VoteResponse,
    },
};
use tokio::task::JoinHandle;

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

pub struct NetworkStreaming {
    pub node_id: NodeId,
    pub tls_config: Option<Arc<rustls::ClientConfig>>,
    pub secret_raft: Vec<u8>,
    pub raft_type: RaftType,
    pub heartbeat_interval: u64,
    // pub sender: flume::Sender<RaftRequest>,
}

#[cfg(feature = "cache")]
impl RaftNetworkFactory<TypeConfigKV> for NetworkStreaming {
    type Network = NetworkConnectionStreaming;

    #[tracing::instrument(level = "debug", skip_all)]
    async fn new_client(&mut self, _target: NodeId, node: &Node) -> Self::Network {
        info!("Building new Raft Cache client with target {}", node);

        let (sender, rx) = flume::bounded(1);

        let task = tokio::task::spawn(Self::ws_handler(
            self.node_id,
            self.raft_type.clone(),
            node.clone(),
            self.tls_config.clone(),
            self.secret_raft.clone(),
            rx,
            self.heartbeat_interval,
        ));

        NetworkConnectionStreaming {
            node: node.clone(),
            sender,
            task: Some(task),
        }
    }
}

#[cfg(feature = "sqlite")]
impl RaftNetworkFactory<TypeConfigSqlite> for NetworkStreaming {
    type Network = NetworkConnectionStreaming;

    #[tracing::instrument(level = "debug", skip_all)]
    async fn new_client(&mut self, _target: NodeId, node: &Node) -> Self::Network {
        info!("Building new Raft DB client with target {}", node);

        let (sender, rx) = flume::bounded(1);

        let task = tokio::task::spawn(Self::ws_handler(
            self.node_id,
            self.raft_type.clone(),
            node.clone(),
            self.tls_config.clone(),
            self.secret_raft.clone(),
            rx,
            self.heartbeat_interval,
        ));

        NetworkConnectionStreaming {
            node: node.clone(),
            sender,
            task: Some(task),
        }
    }
}

#[derive(Debug)]
enum RaftRequest {
    #[cfg(feature = "sqlite")]
    AppendDB(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            AppendEntriesRequest<TypeConfigSqlite>,
        ),
    ),
    #[cfg(feature = "sqlite")]
    VoteDB(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            VoteRequest<u64>,
        ),
    ),
    #[cfg(feature = "sqlite")]
    SnapshotDB(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            InstallSnapshotRequest<TypeConfigSqlite>,
        ),
    ),

    #[cfg(feature = "cache")]
    AppendCache(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            AppendEntriesRequest<TypeConfigKV>,
        ),
    ),
    #[cfg(feature = "cache")]
    VoteCache(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            VoteRequest<u64>,
        ),
    ),
    #[cfg(feature = "cache")]
    SnapshotCache(
        (
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
            InstallSnapshotRequest<TypeConfigKV>,
        ),
    ),

    StreamResponse(RaftStreamResponse),

    Shutdown,
}

#[derive(Debug)]
enum WritePayload {
    Payload(Vec<u8>),
    Close,
}

#[allow(clippy::type_complexity)]
impl NetworkStreaming {
    // pub async fn spawn_ws_handler(
    //     this_node: NodeId,
    //     node: Node,
    //     tls_config: Option<Arc<rustls::ClientConfig>>,
    //     secret: Vec<u8>,
    // ) -> flume::Sender<RaftRequest> {
    //     let (tx, rx) = flume::unbounded();
    //     task::spawn(Self::ws_handler(this_node, node, tls_config, secret, rx));
    //     tx
    // }

    async fn ws_handler(
        this_node: NodeId,
        raft_type: RaftType,
        node: Node,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        secret: Vec<u8>,
        rx: flume::Receiver<RaftRequest>,
        heartbeat_interval: u64,
    ) {
        let mut request_id = 0usize;
        // TODO probably, a Vec<_> is faster here since we would never have too many in flight reqs
        // for raft internal replication and voting? -> check
        // maybe feature-gate an alternative impl, even though it might not make the biggest difference
        let mut in_flight: HashMap<
            usize,
            oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        > = HashMap::with_capacity(8);
        // let mut in_flight: Vec<(
        //     usize,
        //     oneshot::Sender<Result<RaftStreamResponsePayload, Error>>,
        // )> = Vec::with_capacity(8);
        let mut shutdown = false;

        loop {
            let socket = {
                match Self::try_connect(
                    this_node,
                    &node.addr_raft,
                    &raft_type,
                    tls_config.clone(),
                    &secret,
                )
                .await
                {
                    Ok(socket) => socket,
                    Err(err) => {
                        error!("Socket connect error: {:?}", err);

                        // TODO not sure which is better, sleep before drain or after -> more testing
                        time::sleep(Duration::from_millis(heartbeat_interval * 3)).await;

                        // make sure messages don't pile up
                        rx.drain().for_each(|req| {
                            let ack = match req {
                                #[cfg(feature = "sqlite")]
                                RaftRequest::AppendDB((ack, _)) => Some(ack),
                                #[cfg(feature = "sqlite")]
                                RaftRequest::VoteDB((ack, _)) => Some(ack),
                                #[cfg(feature = "sqlite")]
                                RaftRequest::SnapshotDB((ack, _)) => Some(ack),
                                #[cfg(feature = "cache")]
                                RaftRequest::AppendCache((ack, _)) => Some(ack),
                                #[cfg(feature = "cache")]
                                RaftRequest::VoteCache((ack, _)) => Some(ack),
                                #[cfg(feature = "cache")]
                                RaftRequest::SnapshotCache((ack, _)) => Some(ack),
                                RaftRequest::StreamResponse(_) => None,
                                RaftRequest::Shutdown => None,
                            };

                            if let Some(ack) = ack {
                                let _ = ack.send(Err(Error::Connect(format!(
                                    "Cannot connect to {}",
                                    node.addr_raft
                                ))));
                            }
                        });

                        // if there is a network error, don't try too hard to connect
                        // time::sleep(Duration::from_millis(heartbeat_interval * 3)).await;
                        continue;
                    }
                }
            };
            assert!(
                in_flight.is_empty(),
                "raft in flight buffer should always be empty when restoring a connection"
            );

            let (tx_write, rx_write) = flume::bounded(1);
            let (tx_read, rx_read) = flume::bounded(1);

            // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
            let (read, write) = socket.split(tokio::io::split);
            // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
            let read = FragmentCollectorRead::new(read);

            let handle_read = task::spawn(Self::stream_reader(read, tx_read.clone()));
            let handle_write = task::spawn(Self::stream_writer(write, rx_write));

            loop {
                let res = select! {
                    res = rx.recv_async() => res,
                    res = rx_read.recv_async() => res,
                };

                let req = match res {
                    Ok(r) => r,
                    Err(err) => {
                        error!("Client stream reader error: {}", err,);

                        if rx.is_disconnected() {
                            let _ = tx_write.send_async(WritePayload::Close).await;
                            shutdown = true;
                            break;
                        }

                        break;
                    }
                };

                let stream_req = match req {
                    #[cfg(feature = "sqlite")]
                    RaftRequest::AppendDB((ack, req)) => {
                        Some((ack, RaftStreamRequest::AppendDB((request_id, req))))
                    }
                    #[cfg(feature = "sqlite")]
                    RaftRequest::VoteDB((ack, req)) => {
                        Some((ack, RaftStreamRequest::VoteDB((request_id, req))))
                    }
                    #[cfg(feature = "sqlite")]
                    RaftRequest::SnapshotDB((ack, req)) => {
                        Some((ack, RaftStreamRequest::SnapshotDB((request_id, req))))
                    }

                    #[cfg(feature = "cache")]
                    RaftRequest::AppendCache((ack, req)) => {
                        Some((ack, RaftStreamRequest::AppendCache((request_id, req))))
                    }
                    #[cfg(feature = "cache")]
                    RaftRequest::VoteCache((ack, req)) => {
                        Some((ack, RaftStreamRequest::VoteCache((request_id, req))))
                    }
                    #[cfg(feature = "cache")]
                    RaftRequest::SnapshotCache((ack, req)) => {
                        Some((ack, RaftStreamRequest::SnapshotCache((request_id, req))))
                    }

                    RaftRequest::StreamResponse(resp) => {
                        match in_flight.remove(&resp.request_id) {
                            None => {
                                error!("client ack for RaftStreamResponse missing");
                            }
                            Some(ack) => {
                                if ack.send(Ok(resp.payload)).is_err() {
                                    error!("sending back stream response from raft server");
                                }
                            }
                        }
                        None
                    }

                    RaftRequest::Shutdown => {
                        shutdown = true;
                        break;
                    }
                };

                if let Some((ack, payload)) = stream_req {
                    let bytes = serialize(&payload).unwrap();

                    if let Err(err) = tx_write.send_async(WritePayload::Payload(bytes)).await {
                        let _ = ack.send(Err(Error::Connect(format!(
                            "Error sending Write Request to WebSocket writer: {}",
                            err
                        ))));
                        break;
                    }

                    // in_flight.push((request_id, ack));
                    in_flight.insert(request_id, ack);
                    request_id += 1;
                }
            }

            handle_write.abort();
            handle_read.abort();

            // for (_, ack) in in_flight.drain(..) {
            for (_, ack) in in_flight.drain() {
                let _ = ack.send(Err(Error::Connect("Raft WebSocket stream ended".into())));
            }
            // reset to a reasonable size for the next start to keep memory usage under control
            in_flight = HashMap::with_capacity(8);

            if shutdown {
                break;
            }
        }

        warn!("Raft Client shut down, tx closed, exiting WsHandler");
    }

    async fn stream_reader(
        mut read: FragmentCollectorRead<ReadHalf<TokioIo<Upgraded>>>,
        tx: flume::Sender<RaftRequest>,
    ) {
        while let Ok(frame) = read
            .read_frame(&mut |frame| async move {
                // TODO obligated sends should be auto ping / pong / close ? -> verify!
                warn!(
                    "\n\nReceived obligated send in stream client: OpCode: {:?}: {:?}\n\n",
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
                    let payload = deserialize::<RaftStreamResponse>(bytes).unwrap();
                    if let Err(err) = tx.send_async(RaftRequest::StreamResponse(payload)).await {
                        error!(
                            "Error sending Response to Raft client stream manager: {:?}",
                            err
                        );
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

    async fn try_connect(
        node_id: NodeId,
        addr: &str,
        raft_type: &RaftType,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        secret: &[u8],
    ) -> Result<WebSocket<TokioIo<Upgraded>>, Error> {
        let scheme = if tls_config.is_some() {
            "https"
        } else {
            "http"
        };
        let uri = format!("{}://{}/stream/{}", scheme, addr, raft_type.as_str());
        info!("Trying to connect to: {}", uri);

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
            .map_err(|err| Error::Connect(err.to_string()))?;

        let stream = TcpStream::connect(addr).await?;
        let (mut ws, _) = if let Some(config) = tls_config {
            let (addr, _) = addr.split_once(':').unwrap_or((addr, ""));
            let tls_stream = tls::into_tls_stream(addr, stream, config).await?;
            fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await
        } else {
            fastwebsockets::handshake::client(&SpawnExecutor, req, stream).await
        }?;
        ws.set_auto_close(true);

        if let Err(err) = HandshakeSecret::client(&mut ws, secret, node_id).await {
            let _ = ws
                .write_frame(Frame::close(1000, b"Invalid Handshake"))
                .await;
            Err(Error::Connect(format!(
                "Error during API WebSocket handshake: {}",
                err
            )))
        } else {
            Ok(ws)
        }
    }
}

#[allow(clippy::type_complexity)]
pub struct NetworkConnectionStreaming {
    node: Node,
    sender: flume::Sender<RaftRequest>,
    task: Option<JoinHandle<()>>,
}

impl Drop for NetworkConnectionStreaming {
    fn drop(&mut self) {
        let _ = self.sender.send(RaftRequest::Shutdown);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl NetworkConnectionStreaming {
    #[inline(always)]
    async fn send<Err>(
        &mut self,
        req: RaftRequest,
        rx: oneshot::Receiver<Result<RaftStreamResponsePayload, Error>>,
    ) -> Result<RaftStreamResponsePayload, RPCError<NodeId, Node, Err>>
    where
        Err: std::error::Error + 'static + Clone,
    {
        tracing::debug!(
            req = debug(&req),
            "sending rpc request to {}",
            self.node.addr_raft
        );

        self.sender
            .send_async(req)
            .await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?;

        rx.await
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))?
            .map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
    }
}

#[cfg(feature = "sqlite")]
impl RaftNetwork<TypeConfigSqlite> for NetworkConnectionStreaming {
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfigSqlite>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        let (ack, rx) = oneshot::channel();
        match self.send(RaftRequest::AppendDB((ack, req)), rx).await? {
            RaftStreamResponsePayload::AppendDB(resp) => {
                resp.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn install_snapshot(
        &mut self,
        req: InstallSnapshotRequest<TypeConfigSqlite>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, Node, RaftError<NodeId, InstallSnapshotError>>,
    > {
        let (ack, rx) = oneshot::channel();
        match self.send(RaftRequest::SnapshotDB((ack, req)), rx).await? {
            RaftStreamResponsePayload::SnapshotDB(resp) => {
                resp.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn vote(
        &mut self,
        req: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        let (ack, rx) = oneshot::channel();
        match self.send(RaftRequest::VoteDB((ack, req)), rx).await? {
            RaftStreamResponsePayload::VoteDB(resp) => {
                resp.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "cache")]
impl RaftNetwork<TypeConfigKV> for NetworkConnectionStreaming {
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfigKV>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        let (ack, rx) = oneshot::channel();
        match self.send(RaftRequest::AppendCache((ack, req)), rx).await? {
            RaftStreamResponsePayload::AppendCache(resp) => {
                resp.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn install_snapshot(
        &mut self,
        req: InstallSnapshotRequest<TypeConfigKV>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, Node, RaftError<NodeId, InstallSnapshotError>>,
    > {
        let (ack, rx) = oneshot::channel();
        match self
            .send(RaftRequest::SnapshotCache((ack, req)), rx)
            .await?
        {
            RaftStreamResponsePayload::SnapshotCache(resp) => {
                resp.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn vote(
        &mut self,
        req: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        let (ack, rx) = oneshot::channel();
        match self.send(RaftRequest::VoteCache((ack, req)), rx).await? {
            RaftStreamResponsePayload::VoteCache(resp) => {
                resp.map_err(|err| RPCError::Unreachable(Unreachable::new(&err)))
            }
            _ => unreachable!(),
        }
    }
}
