use crate::app_state::RaftType;
use crate::network::handshake::HandshakeSecret;
use crate::network::{validate_secret, AppStateExt, Error};
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use fastwebsockets::{upgrade, FragmentCollectorRead, Frame, OpCode, Payload};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Debug;
use std::ops::Deref;
use std::time::Duration;
use tokio::{task, time};
use tracing::{error, info, warn};

#[cfg(feature = "cache")]
use crate::store::state_machine::memory::{
    kv_handler::CacheRequestHandler,
    state_machine::{CacheRequest, CacheResponse},
};

#[cfg(feature = "dlock")]
use crate::store::state_machine::memory::dlock_handler::{
    LockAwaitPayload, LockRequest, LockState,
};

#[cfg(feature = "sqlite")]
use crate::{
    migration::Migration,
    query::{query_consistent_local, query_owned_local, rows::RowOwned},
    store::state_machine::sqlite::state_machine::{Query, QueryWrite},
};

#[cfg(feature = "listen_notify")]
use crate::store::state_machine::memory::notify_handler::NotifyRequest;
#[cfg(feature = "listen_notify")]
use axum::response::sse;
#[cfg(feature = "listen_notify")]
use futures_util::stream::Stream;
// pub(crate) async fn write(
//     state: AppStateExt,
//     headers: HeaderMap,
//     body: body::Bytes,
// ) -> Result<Response, ApiError> {
//     let payload = get_payload::<Sql>(&headers, body)?;
//     fmt_ok(headers, state.raft_db.raft_db.raft.client_write(payload).await?)
// }

// pub(crate) async fn read(
//     state: AppStateExt,
//     headers: HeaderMap,
//     body: body::Bytes,
// ) -> Result<Response, ApiError> {
//     let key: String = bincode::deserialize(body.as_ref())?;
//     let value = read_local(&state.0, &key).await?;
//     // let kvs = app.kv_store.read().await;
//     // let value = kvs.get(&key).cloned();
//     fmt_ok(headers, value)
// }

// #[inline(always)]
// pub(crate) async fn read_local(
//     _state: &Arc<AppState>,
//     _key: &str,
// ) -> Result<Option<String>, ApiError> {
//     // TODO put behind feature flag?
//     Err(ApiError::Error("read not implemented for Sqlite".into()))
//     // let kvs = app.kv_store.read().await;
//     // let value = state.kv_store.read().await.get(key).cloned();
//     // Ok(value)
// }

// pub(crate) async fn consistent_read(
//     state: AppStateExt,
//     headers: HeaderMap,
//     body: body::Bytes,
// ) -> Result<Response, ApiError> {
//     validate_secret(&state, &headers)?;
//
//     let key: String = bincode::deserialize(body.as_ref())?;
//     let value = consistent_read_local(&state, &key).await?;
//     // let _ = app.raft.ensure_linearizable().await?;
//     //
//     // let kvs = app.kv_store.read().await;
//     // let value = kvs.get(&key);
//     fmt_ok(headers, value)
// }

// #[inline(always)]
// pub(crate) async fn consistent_read_local(
//     state: &Arc<AppState>,
//     _key: &str,
// ) -> Result<Option<String>, ApiError> {
//     let _ = state.raft_db.raft_db.raft.ensure_linearizable().await?;
//     // TODO put behind feature flag?
//     Err(ApiError::Error(
//         "read consistent not implemented for Sqlite".into(),
//     ))
//     // Ok(state.kv_store.read().await.get(key).cloned())
// }

pub async fn health(state: AppStateExt) -> Result<(), Error> {
    #[cfg(all(not(feature = "sqlite"), not(feature = "cache")))]
    panic!("neither `sqlite` nor `cache` feature enabled");

    #[cfg(any(feature = "sqlite", feature = "cache"))]
    {
        if check_health(&state).await.is_err() {
            // after at least 3 seconds, we should have a new leader
            time::sleep(Duration::from_secs(3)).await;
            check_health(&state).await?;
        }
    }

    Ok(())
}

#[cfg(any(feature = "sqlite", feature = "cache"))]
async fn check_health(state: &AppStateExt) -> Result<(), Error> {
    #[cfg(feature = "sqlite")]
    {
        let metrics = state.raft_db.raft.metrics().borrow().clone();
        metrics.running_state?;
        if metrics.current_leader.is_none() {
            return Err(Error::LeaderChange(
                "The leader voting process has not finished yet for Raft DB".into(),
            ));
        }
    }
    #[cfg(feature = "cache")]
    {
        let metrics = state.raft_cache.raft.metrics().borrow().clone();
        metrics.running_state?;
        if metrics.current_leader.is_none() {
            return Err(Error::LeaderChange(
                "The leader voting process has not finished yet Raft Cache".into(),
            ));
        }
    }

    Ok(())
}

pub async fn ping() {}

#[cfg(feature = "listen_notify")]
pub async fn listen(
    state: AppStateExt,
    headers: HeaderMap,
) -> Result<sse::Sse<impl Stream<Item = Result<sse::Event, Error>>>, Error> {
    validate_secret(&state, &headers)?;

    let (tx, rx) = flume::bounded(2);
    state
        .raft_cache
        .tx_notify
        .send_async(NotifyRequest::Listen(tx))
        .await?;

    Ok(sse::Sse::new(rx.into_stream()).keep_alive(sse::KeepAlive::default()))
}

#[cfg(not(feature = "listen_notify"))]
pub async fn listen(state: AppStateExt, headers: HeaderMap) -> Result<(), Error> {
    validate_secret(&state, &headers)?;
    Err(Error::Config(
        "'listen_notify' feature is not active".into(),
    ))
}

// TODO maybe remove this endpoint in favor or a generic REST endpoint which chooses the
// the correct sub-method on its own? -> way better UX and response will be just `text` anyway?
// pub(crate) async fn execute(
//     state: AppStateExt,
//     headers: HeaderMap,
//     body: body::Bytes,
// ) -> Result<Response, Error> {
//     validate_secret(&state, &headers)?;
//
//     let payload = get_payload::<Query>(&headers, body)?;
//     match state
//         .raft_db
//         .raft
//         .client_write(QueryWrite::Execute(payload))
//         .await
//     {
//         Ok(resp) => {
//             let resp: crate::Response = resp.data;
//             let res = match resp {
//                 crate::Response::Execute(res) => res.result,
//                 _ => unreachable!(),
//             };
//             fmt_ok(headers, res)
//         }
//         Err(err) => {
//             eprintln!("\nError on leader: {:?}\n", err);
//             Err(Error::from(err))
//         }
//     }
// }

// pub(crate) async fn query(
//     state: AppStateExt,
//     headers: HeaderMap,
//     body: body::Bytes,
// ) -> Result<Response, Error> {
//     check_csrf(&Method::POST, &headers)?;
//     validate_secret(&state, &headers)?;
//
//     // TODO check accept header and allow JSON requests for ease of use as well
//     let _payload = get_payload::<Query>(&headers, body)?;
//
//     // match &payload {
//     //     Query::Execute(_) => {
//     //         return Err(ApiError::BadRequest(
//     //             "Query must be Query::Execute for this endpoint".into(),
//     //         ));
//     //     }
//     //     _ => {}
//     // };
//
//     // let conn = state.sql_reader.get().await?;
//     // let value = query_map(&state, payload).await?;
//     todo!()
//     // fmt_ok(headers, value)
// }

// #[inline(always)]
// pub(crate) async fn query_consistent(
//     state: AppStateExt,
//     headers: HeaderMap,
//     body: body::Bytes,
// ) -> Result<Response, ApiError> {
//     validate_secret(&state, &headers)?;
//
//     // TODO check accept header and allow JSON requests for ease of use as well
//     let _payload = get_payload::<Sql>(&headers, body)?;
//
//     // match &payload {
//     //     Query::Execute(_) => {
//     //         return Err(ApiError::BadRequest(
//     //             "Query must be Query::Execute for this endpoint".into(),
//     //         ));
//     //     }
//     //     _ => {}
//     // };
//
//     // let conn = state.sql_reader.get().await?;
//     // let value = query_map(&state, payload).await?;
//     todo!()
//     // fmt_ok(headers, value)
// }

pub async fn stream(
    state: AppStateExt,
    Path(raft_type): Path<RaftType>,
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    let (response, socket) = ws.upgrade()?;

    tokio::task::spawn(async move {
        if let Err(err) = handle_socket_concurrent(state, raft_type, socket).await {
            // if let Err(err) = handle_socket_sequential(state, socket).await {
            error!("Error in websocket connection: {}", err);
        }
    });

    Ok(response)
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiStreamRequest {
    pub(crate) request_id: usize,
    pub(crate) payload: ApiStreamRequestPayload,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum ApiStreamRequestPayload {
    #[cfg(feature = "sqlite")]
    Execute(Query),
    #[cfg(feature = "sqlite")]
    ExecuteReturning(Query),
    #[cfg(feature = "sqlite")]
    Transaction(Vec<Query>),
    #[cfg(feature = "sqlite")]
    QueryConsistent(Query),
    #[cfg(feature = "sqlite")]
    Batch(std::borrow::Cow<'static, str>),
    #[cfg(feature = "sqlite")]
    Migrate(Vec<Migration>),

    #[cfg(feature = "backup")]
    Backup(crate::NodeId),

    #[cfg(feature = "cache")]
    KV(CacheRequest),

    // remote-only clients
    #[cfg(feature = "sqlite")]
    Query(Query),
    #[cfg(feature = "cache")]
    KVGet(CacheRequest),
    #[cfg(feature = "dlock")]
    LockAwait(CacheRequest),
    #[cfg(feature = "listen_notify")]
    Notify(CacheRequest),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiStreamResponse {
    pub(crate) request_id: usize,
    pub(crate) result: ApiStreamResponsePayload,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum ApiStreamResponsePayload {
    #[cfg(feature = "sqlite")]
    Execute(Result<usize, Error>),
    #[cfg(feature = "sqlite")]
    ExecuteReturning(Result<Vec<Result<RowOwned, Error>>, Error>),
    #[cfg(feature = "sqlite")]
    Transaction(Result<Vec<Result<usize, Error>>, Error>),
    #[cfg(feature = "sqlite")]
    Query(Result<Vec<RowOwned>, Error>),
    #[cfg(feature = "sqlite")]
    QueryConsistent(Result<Vec<RowOwned>, Error>),
    #[cfg(feature = "sqlite")]
    Batch(Result<Vec<Result<usize, Error>>, Error>),
    #[cfg(feature = "sqlite")]
    Migrate(Result<(), Error>),

    #[cfg(feature = "backup")]
    Backup(Result<(), Error>),

    #[cfg(feature = "cache")]
    KV(Result<CacheResponse, Error>),

    #[cfg(feature = "dlock")]
    Lock(LockState),

    #[cfg(feature = "listen_notify")]
    Notify(Result<(), Error>),
}

#[derive(Debug)]
pub(crate) enum WsWriteMsg {
    Payload(ApiStreamResponse),
    Break,
}

async fn handle_socket_concurrent(
    state: AppStateExt,
    raft_type: RaftType,
    socket: upgrade::UpgradeFut,
) -> Result<(), fastwebsockets::WebSocketError> {
    let mut ws = socket.await?;
    ws.set_auto_close(true);

    let client_id = match HandshakeSecret::server(&mut ws, state.secret_api.as_bytes()).await {
        Ok(id) => id,
        Err(err) => {
            error!("Error during WebSocket handshake: {}", err);
            ws.write_frame(Frame::close(1000, b"Invalid Handshake"))
                .await?;
            return Ok(());
        }
    };

    let (tx_write, rx_write) = flume::bounded::<WsWriteMsg>(2);
    // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
    let (rx, mut write) = ws.split(tokio::io::split);
    // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
    let mut read = FragmentCollectorRead::new(rx);

    {
        // make sure to NEVER lose the result of an execute from remote!
        // if we received one which is being executed and the TCP stream dies in between, we MUST
        // ENSURE that in case it was an Ok(_), the result gets to the client! Otherwise, with retry
        // logic we might end up modifying something twice!
        let mut buf = {
            let mut map = state.get_buf_lock(&raft_type).await;
            map.remove(&client_id).unwrap_or_default()
        };

        info!("Emptying buffered Client Stream responses");
        while let Some(payload) = buf.pop_front() {
            let frame = Frame::binary(Payload::Owned(payload));
            if let Err(err) = write.write_frame(frame).await {
                // if we error again, we will throw the buffer away, since the problem is bigger
                // for sure and messages will most probably not be relevant anymore when we are
                // back online
                error!("Error during WebSocket handshake: {}", err);
                return Ok(());
            }
        }
    }

    let st = state.clone();
    let handle_write = task::spawn(async move {
        let mut buf = VecDeque::default();

        while let Ok(req) = rx_write.recv_async().await {
            match req {
                WsWriteMsg::Payload(resp) => {
                    let bytes = bincode::serialize(&resp).unwrap();
                    let frame = Frame::binary(Payload::Borrowed(&bytes));
                    if let Err(err) = write.write_frame(frame).await {
                        error!("Error during WebSocket write: {}", err);
                        // if we have a WebSocket error, save all open requests into the client_buffer
                        let payload = bincode::serialize(&resp).unwrap();
                        buf.push_back(payload);
                        break;
                    }
                }
                WsWriteMsg::Break => {
                    // we ignore any errors here since it may be possible that the reader
                    // has closed already - we just try a graceful connection close
                    warn!("server stream break message");
                    break;
                }
            }
        }

        warn!("emptying server stream writer channel into buffer");
        while let Ok(req) = rx_write.recv_async().await {
            if let WsWriteMsg::Payload(resp) = req {
                let payload = bincode::serialize(&resp).unwrap();
                buf.push_back(payload);
            }
        }

        {
            let mut lock = st.get_buf_lock(&raft_type).await;
            let old = lock.insert(client_id, buf);
            assert!(
                old.is_none() || old == Some(VecDeque::default()),
                "client buffer for {} should never exist already when we insert a new one:\n{:?}",
                raft_type.as_str(),
                old
            );
        }

        let _ = write
            .write_frame(Frame::close(1000, b"Invalid Request"))
            .await;

        warn!("server stream exiting");
    });

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
        let req = match frame.opcode {
            OpCode::Close => {
                warn!("received Close frame in server stream");
                break;
            }
            OpCode::Binary => {
                let bytes = frame.payload.deref();
                match bincode::deserialize::<ApiStreamRequest>(bytes) {
                    Ok(req) => req,
                    Err(err) => {
                        error!("Error deserializing ApiStreamRequest: {:?}", err);
                        let _ = tx_write.send_async(WsWriteMsg::Break).await;
                        break;
                    }
                }
            }
            _ => {
                let _ = tx_write.send_async(WsWriteMsg::Break).await;
                break;
            }
        };

        let state = state.clone();
        let tx_write = tx_write.clone();
        task::spawn(async move {
            let request_id = req.request_id;

            let res = match req.payload {
                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Execute(sql) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Execute(sql))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Execute(res) => res.result,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Execute(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Execute(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::ExecuteReturning(sql) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::ExecuteReturning(sql))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::ExecuteReturning(res) => res.result,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::ExecuteReturning(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::ExecuteReturning(Err(Error::from(
                                err,
                            ))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Transaction(queries) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Transaction(queries))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Transaction(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Transaction(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Transaction(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::QueryConsistent(Query { sql, params }) => {
                    let res = query_consistent_local(
                        &state.raft_db.raft,
                        state.raft_db.log_statements,
                        state.raft_db.read_pool.clone(),
                        sql,
                        params,
                    )
                    .await;

                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::QueryConsistent(res),
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Batch(sql) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Batch(sql))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Batch(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Batch(Ok(res.result)),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Batch(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Migrate(migrations) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Migration(migrations))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Migrate(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Migrate(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Migrate(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "backup")]
                ApiStreamRequestPayload::Backup(node_id) => {
                    match state
                        .raft_db
                        .raft
                        .client_write(QueryWrite::Backup(node_id))
                        .await
                    {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Backup(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::Backup(res),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Backup(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "sqlite")]
                ApiStreamRequestPayload::Query(Query { sql, params }) => {
                    let res = query_owned_local(
                        state.raft_db.log_statements,
                        state.raft_db.read_pool.clone(),
                        sql,
                        params,
                    )
                    .await;

                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Query(res),
                    }
                }

                #[cfg(feature = "cache")]
                ApiStreamRequestPayload::KV(cache_req) => {
                    match state.raft_cache.raft.client_write(cache_req).await {
                        Ok(resp) => {
                            let resp: CacheResponse = resp.data;
                            ApiStreamResponse {
                                request_id,
                                result: ApiStreamResponsePayload::KV(Ok(resp)),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::KV(Err(Error::from(err))),
                        },
                    }
                }

                #[cfg(feature = "cache")]
                ApiStreamRequestPayload::KVGet(cache_req) => {
                    let (cache_idx, key) = match cache_req {
                        CacheRequest::Get { cache_idx, key } => (cache_idx, key),
                        _ => unreachable!(),
                    };

                    let (ack, rx) = tokio::sync::oneshot::channel();
                    state
                        .raft_cache
                        .tx_caches
                        .get(cache_idx)
                        .unwrap()
                        .send(CacheRequestHandler::Get((key, ack)))
                        .expect("kv handler to always be running");
                    let value = rx.await.expect("to always get an answer from kv handler");
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::KV(Ok(CacheResponse::Value(value))),
                    }
                }

                #[cfg(feature = "dlock")]
                ApiStreamRequestPayload::LockAwait(cache_req) => {
                    let (key, id) = match cache_req {
                        CacheRequest::LockAwait((key, id)) => (key, id),
                        _ => unreachable!(),
                    };

                    let (ack, rx) = tokio::sync::oneshot::channel();
                    state
                        .raft_cache
                        .tx_dlock
                        .send(LockRequest::Await(LockAwaitPayload { key, id, ack }))
                        .expect("kv handler to always be running");
                    let lock_state = rx
                        .await
                        .expect("to always get an answer from the kv handler");

                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Lock(lock_state),
                    }
                }

                #[cfg(feature = "listen_notify")]
                ApiStreamRequestPayload::Notify(cache_req) => {
                    let (ts, data) = match cache_req {
                        CacheRequest::Notify((ts, data)) => (ts, data),
                        _ => unreachable!(),
                    };

                    match state
                        .raft_cache
                        .raft
                        .client_write(CacheRequest::Notify((ts, data)))
                        .await
                    {
                        Ok(_) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Notify(Ok(())),
                        },
                        Err(err) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Notify(Err(Error::from(err))),
                        },
                    }
                }
            };

            if let Err(err) = tx_write.send_async(WsWriteMsg::Payload(res)).await {
                panic!(
                    "Error sending payload to tx_write - this should never happen: {}",
                    err
                );
            }
        });
    }

    // ignore the result in case the writer has already exited and drop the channel
    // on purpose to make sure a maybe still running writer catches it
    let _ = tx_write.send_async(WsWriteMsg::Break).await;
    drop(tx_write);

    handle_write.await.unwrap();

    Ok(())
}
