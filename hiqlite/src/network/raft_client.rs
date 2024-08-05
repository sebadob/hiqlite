use crate::network::handshake::HandshakeSecret;
use crate::network::raft_server::{RaftStreamRequest, RaftStreamResponse};
use crate::Node;
use crate::{tls, NodeId};
use bytes::Bytes;
use fastwebsockets::{Frame, OpCode, WebSocket};
use http_body_util::Empty;
use hyper::header::{CONNECTION, UPGRADE};
use hyper::upgrade::Upgraded;
use hyper::Request;
use hyper_util::rt::TokioIo;
use openraft::error::RPCError;
use openraft::error::{NetworkError, Unreachable};
use std::future::Future;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::TypeConfigKV;

#[cfg(feature = "sqlite")]
use crate::store::state_machine::sqlite::TypeConfigSqlite;

#[cfg(any(feature = "cache", feature = "sqlite"))]
use crate::Error;
#[cfg(any(feature = "cache", feature = "sqlite"))]
use openraft::{
    error::{InstallSnapshotError, RaftError, RemoteError},
    network::{RPCOption, RaftNetwork, RaftNetworkFactory},
    raft::{
        AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest,
        InstallSnapshotResponse, VoteRequest, VoteResponse,
    },
};

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
}

#[cfg(feature = "cache")]
impl RaftNetworkFactory<TypeConfigKV> for NetworkStreaming {
    type Network = NetworkConnectionStreaming;

    #[tracing::instrument(level = "debug", skip_all)]
    async fn new_client(&mut self, _target: NodeId, node: &Node) -> Self::Network {
        info!("Building new Raft Cache client with target {}", node);

        let (sender, rx) = flume::unbounded();

        let handle = tokio::task::spawn(Self::ws_handler(
            self.node_id,
            "/stream/cache",
            node.clone(),
            self.tls_config.clone(),
            self.secret_raft.clone(),
            rx,
        ));

        NetworkConnectionStreaming {
            node: node.clone(),
            sender,
            handle,
        }
    }
}

#[cfg(feature = "sqlite")]
impl RaftNetworkFactory<TypeConfigSqlite> for NetworkStreaming {
    type Network = NetworkConnectionStreaming;

    #[tracing::instrument(level = "debug", skip_all)]
    async fn new_client(&mut self, _target: NodeId, node: &Node) -> Self::Network {
        info!("Building new Raft DB client with target {}", node);

        let (sender, rx) = flume::unbounded();

        let handle = tokio::task::spawn(Self::ws_handler(
            self.node_id,
            "/stream/db",
            node.clone(),
            self.tls_config.clone(),
            self.secret_raft.clone(),
            rx,
        ));

        NetworkConnectionStreaming {
            node: node.clone(),
            sender,
            handle,
        }
    }
}

#[allow(clippy::type_complexity)]
impl NetworkStreaming {
    async fn ws_handler<Err>(
        this_node: NodeId,
        path: &'static str,
        node: Node,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        secret: Vec<u8>,
        rx: flume::Receiver<(
            RaftStreamRequest,
            oneshot::Sender<Result<RaftStreamResponse, RPCError<NodeId, Node, Err>>>,
        )>,
    ) where
        Err: std::error::Error + 'static + Clone,
    {
        let mut ws = None;
        // let mut req: Option<RaftStreamRequest> = None;
        // let mut ack: Option<
        //     oneshot::Sender<Result<RaftStreamResponse, RPCError<NodeId, Node, Err>>>,
        // > = None;

        'main: while let Ok((req, ack)) = rx.recv_async().await {
            if ack.is_closed() {
                continue;
            }

            while ws.is_none() {
                info!("WsHandler trying to connect to {}", node.addr_raft);
                // TODO we probably don't need the sleep since the Raft reqs will throttle passively
                // time::sleep(Duration::from_millis(100)).await;

                match Self::try_connect(
                    this_node,
                    &node.addr_raft,
                    path,
                    tls_config.clone(),
                    &secret,
                )
                .await
                {
                    Ok(socket) => {
                        ws = Some(socket);
                    }
                    Err(err) => {
                        error!("{:?}", err);

                        ack.send(Err(RPCError::Unreachable(Unreachable::new(&err))))
                            .unwrap();
                        continue 'main;
                    }
                }
            }

            let socket = ws.as_mut().unwrap();
            match socket.write_frame(req.as_payload()).await {
                Ok(_) => match socket.read_frame().await {
                    Ok(frame) => match frame.opcode {
                        OpCode::Binary => {
                            let bytes = frame.payload.to_vec();
                            let resp = RaftStreamResponse::from(bytes);
                            if let Err(err) = ack.send(Ok(resp)) {
                                error!(
                                    "Error forwarding response from Node {}: {:?}",
                                    node.id, err
                                );
                            }
                        }
                        _ => unreachable!(),
                    },
                    Err(err) => {
                        error!("Error receiving RPC response: {}", err);
                        ack.send(Err(RPCError::Network(NetworkError::new(&err))))
                            .unwrap();
                        ws = None;
                    }
                },
                Err(err) => {
                    error!("Error sending RPC request: {}", err);
                    ack.send(Err(RPCError::Network(NetworkError::new(&err))))
                        .unwrap();
                    ws = None;
                }
            }
        }

        warn!("Raft Client shut down, tx closed, exiting WsHandler");

        // 'main: loop {
        //     while ws.is_none() {
        //         info!("WsHandler trying to connect to {}", node.addr_raft);
        //         // TODO we probably don't need the sleep since the Raft reqs will throttle passively
        //         // time::sleep(Duration::from_millis(100)).await;
        //
        //         match Self::try_connect(
        //             this_node,
        //             &node.addr_raft,
        //             path,
        //             tls_config.clone(),
        //             &secret,
        //         )
        //         .await
        //         {
        //             Ok(socket) => {
        //                 ws = Some(socket);
        //             }
        //             Err(err) => {
        //                 error!("{:?}", err);
        //                 if let Some(tx) = ack {
        //                     tx.send(Err(RPCError::Unreachable(Unreachable::new(&err))))
        //                         .unwrap()
        //                 }
        //                 ack = None;
        //                 req = None;
        //                 // we want to break here to get the next raft request,
        //                 // so the channel never fills up
        //                 break;
        //             }
        //         }
        //     }
        //
        //     if let Some(r) = &req {
        //         let socket = ws.as_mut().unwrap();
        //         let ack_tx = ack.unwrap();
        //
        //         match socket.write_frame(r.as_payload()).await {
        //             Ok(_) => match socket.read_frame().await {
        //                 Ok(frame) => match frame.opcode {
        //                     OpCode::Binary => {
        //                         let bytes = frame.payload.to_vec();
        //                         let resp = RaftStreamResponse::from(bytes);
        //                         if let Err(err) = ack_tx.send(Ok(resp)) {
        //                             error!(
        //                                 "Error forwarding response from Node {}: {:?}",
        //                                 node.id, err
        //                             );
        //                         }
        //                     }
        //                     _ => unreachable!(),
        //                 },
        //                 Err(err) => {
        //                     error!("Error receiving RPC response: {}", err);
        //                     ack_tx
        //                         .send(Err(RPCError::Network(NetworkError::new(&err))))
        //                         .unwrap();
        //                     ws = None;
        //                 }
        //             },
        //             Err(err) => {
        //                 error!("Error sending RPC request: {}", err);
        //                 ack_tx
        //                     .send(Err(RPCError::Network(NetworkError::new(&err))))
        //                     .unwrap();
        //                 ws = None;
        //             }
        //         }
        //
        //         ack = None;
        //     }
        //
        //     loop {
        //         match rx.recv_async().await.ok() {
        //             None => {
        //                 warn!("Raft Client shut down, tx closed, exiting WsHandler");
        //                 break 'main;
        //             }
        //             Some((r, a)) => {
        //                 // we need the loop and closed check in case of long-running
        //                 // re-connect loops to other nodes
        //                 if !a.is_closed() {
        //                     req = Some(r);
        //                     ack = Some(a);
        //                     break;
        //                 }
        //             }
        //         }
        //     }
        // }
    }

    async fn try_connect(
        node_id: NodeId,
        addr: &str,
        path: &str,
        tls_config: Option<Arc<rustls::ClientConfig>>,
        secret: &[u8],
    ) -> Result<WebSocket<TokioIo<Upgraded>>, Error> {
        let uri = if tls_config.is_some() {
            format!("https://{}{}", addr, path)
        } else {
            format!("http://{}{}", addr, path)
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
            .map_err(|err| Error::Connect(err.to_string()))?;

        let stream = TcpStream::connect(addr).await?;
        let (mut ws, _) = if let Some(config) = tls_config {
            let (addr, _) = addr.split_once(':').unwrap_or((addr, ""));
            let tls_stream = tls::into_tls_stream(addr, stream, config).await?;
            fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await
        } else {
            fastwebsockets::handshake::client(&SpawnExecutor, req, stream).await
        }?;

        if let Err(err) = HandshakeSecret::client(&mut ws, secret, node_id).await {
            let _ = ws
                .write_frame(Frame::close(1000, b"Invalid Handshake"))
                .await;
            // TODO what should be do in this case? This handler should never exit.
            // panic is the best option in case of misconfiguration?
            panic!("Error during API WebSocket handshake: {}", err);
        }

        Ok(ws)
        //
        // let (mut ws, _) = if let Some(config) = tls_config {
        //     let (addr, _) = addr.split_once(':').unwrap_or((addr, ""));
        //     let tls_stream = match tls::into_tls_stream(addr, stream, config).await {
        //         Ok(s) => s,
        //         Err(err) => {
        //             error!("Error opening TLS stream to {}: {}", addr, err);
        //             return None;
        //         }
        //     };
        //
        //     match fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await {
        //         Ok((ws, r)) => (ws, r),
        //         Err(err) => {
        //             error!("{}", err);
        //             return None;
        //         }
        //     }
        // } else {
        //     fastwebsockets::handshake::client(&SpawnExecutor, req, stream)
        //         .await
        //         .ok()?
        // };
        //
        // if let Err(err) = HandshakeSecret::client(&mut ws, secret, node_id).await {
        //     let _ = ws
        //         .write_frame(Frame::close(1000, b"Invalid Handshake"))
        //         .await;
        //     // TODO what should be do in this case? This handler should never exit.
        //     // panic is the best option in case of misconfiguration?
        //     panic!("Error during WebSocket handshake: {}", err);
        // }
        //
        // Some(ws)
    }
}

#[allow(clippy::type_complexity)]
pub struct NetworkConnectionStreaming {
    node: Node,
    sender: flume::Sender<(
        RaftStreamRequest,
        oneshot::Sender<Result<RaftStreamResponse, RPCError<NodeId, Node>>>,
    )>,
    handle: JoinHandle<()>,
}

impl Drop for NetworkConnectionStreaming {
    fn drop(&mut self) {
        eprintln!("Connection to {} has been dropped", self.node);
        self.handle.abort();
    }
}

impl NetworkConnectionStreaming {
    #[inline(always)]
    async fn send<Err>(
        &mut self,
        req: RaftStreamRequest,
    ) -> Result<RaftStreamResponse, RPCError<NodeId, Node, Err>>
    where
        Err: std::error::Error + 'static + Clone,
    {
        tracing::debug!(
            req = debug(&req),
            "sending rpc request to {}",
            self.node.addr_raft
        );

        let (tx, rx) = oneshot::channel();
        self.sender
            .send_async((req, tx))
            .await
            .map_err(|err| RPCError::Network(NetworkError::new(&err)))?;

        rx.await
            .unwrap()
            .map_err(|err| RPCError::Network(NetworkError::new(&err)))

        // let res = rx
        //     .await
        //     .unwrap()
        //     .map_err(|err| RPCError::Network(NetworkError::new(&err)));
        //
        // if let Err(err) = &res {
        //     error!("\n\nerr in raft client send:\n{:?}\n", err);
        // }
        //
        // res
    }
}

#[cfg(feature = "sqlite")]
#[allow(clippy::blocks_in_conditions)]
impl RaftNetwork<TypeConfigSqlite> for NetworkConnectionStreaming {
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfigSqlite>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        let resp = self.send(RaftStreamRequest::AppendDB(req)).await?;
        match resp {
            RaftStreamResponse::AppendDB(res) => Ok(res),
            RaftStreamResponse::Error(Error::RaftError(err)) => {
                Err(RPCError::RemoteError(RemoteError::new(self.node.id, err)))
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
        let resp = self.send(RaftStreamRequest::SnapshotDB(req)).await?;
        match resp {
            RaftStreamResponse::SnapshotDB(res) => Ok(res),
            RaftStreamResponse::Error(Error::SnapshotError(err)) => {
                Err(RPCError::RemoteError(RemoteError::new(self.node.id, err)))
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
        let resp = self.send(RaftStreamRequest::VoteDB(req)).await?;
        match resp {
            RaftStreamResponse::VoteDB(res) => Ok(res),
            RaftStreamResponse::Error(Error::RaftError(err)) => {
                Err(RPCError::RemoteError(RemoteError::new(self.node.id, err)))
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "cache")]
#[allow(clippy::blocks_in_conditions)]
impl RaftNetwork<TypeConfigKV> for NetworkConnectionStreaming {
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfigKV>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        let resp = self.send(RaftStreamRequest::AppendCache(req)).await?;
        match resp {
            RaftStreamResponse::AppendCache(res) => Ok(res),
            RaftStreamResponse::Error(Error::RaftError(err)) => {
                Err(RPCError::RemoteError(RemoteError::new(self.node.id, err)))
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
        let resp = self.send(RaftStreamRequest::SnapshotCache(req)).await?;
        match resp {
            RaftStreamResponse::SnapshotCache(res) => Ok(res),
            RaftStreamResponse::Error(Error::SnapshotError(err)) => {
                Err(RPCError::RemoteError(RemoteError::new(self.node.id, err)))
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
        let resp = self.send(RaftStreamRequest::VoteCache(req)).await?;
        match resp {
            RaftStreamResponse::VoteCache(res) => Ok(res),
            RaftStreamResponse::Error(Error::RaftError(err)) => {
                Err(RPCError::RemoteError(RemoteError::new(self.node.id, err)))
            }
            _ => unreachable!(),
        }
    }
}
