use crate::app_state::AppState;
use crate::migration::Migration;
use crate::network::handshake::HandshakeSecret;
use crate::network::{fmt_ok, get_payload, validate_secret, AppStateExt, Error};
use crate::store::state_machine::sqlite::state_machine::{Params, Query, QueryWrite};
use axum::body;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fastwebsockets::{upgrade, FragmentCollectorRead, Frame, OpCode, Payload};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;
use tokio::task;
use tracing::{error, info, warn};

// pub(crate) async fn write(
//     state: AppStateExt,
//     headers: HeaderMap,
//     body: body::Bytes,
// ) -> Result<Response, ApiError> {
//     let payload = get_payload::<Sql>(&headers, body)?;
//     fmt_ok(headers, state.raft.client_write(payload).await?)
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
//     let _ = state.raft.ensure_linearizable().await?;
//     // TODO put behind feature flag?
//     Err(ApiError::Error(
//         "read consistent not implemented for Sqlite".into(),
//     ))
//     // Ok(state.kv_store.read().await.get(key).cloned())
// }

pub async fn ping() {}

pub(crate) async fn execute(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    let payload = get_payload::<Query>(&headers, body)?;
    match state.raft.client_write(QueryWrite::Execute(payload)).await {
        Ok(resp) => {
            let resp: crate::Response = resp.data;
            let res = match resp {
                crate::Response::Execute(res) => res.result,
                _ => unreachable!(),
            };
            fmt_ok(headers, res)
        }
        Err(err) => {
            eprintln!("\nError on leader: {:?}\n", err);
            Err(Error::from(err))
        }
    }
}

#[inline(always)]
pub(crate) async fn query(
    state: AppStateExt,
    headers: HeaderMap,
    body: body::Bytes,
) -> Result<Response, Error> {
    validate_secret(&state, &headers)?;

    // TODO check accept header and allow JSON requests for ease of use as well
    let _payload = get_payload::<Query>(&headers, body)?;

    // match &payload {
    //     Query::Execute(_) => {
    //         return Err(ApiError::BadRequest(
    //             "Query must be Query::Execute for this endpoint".into(),
    //         ));
    //     }
    //     _ => {}
    // };

    // let conn = state.sql_reader.get().await?;
    // let value = query_map(&state, payload).await?;
    todo!()
    // fmt_ok(headers, value)
}

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
    ws: upgrade::IncomingUpgrade,
) -> Result<impl IntoResponse, Error> {
    let (response, socket) = ws.upgrade()?;

    tokio::task::spawn(async move {
        if let Err(err) = handle_socket_concurrent(state, socket).await {
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
    Execute(Query),
    Transaction(Vec<Query>),
    Batch(Cow<'static, str>),
    Migrate(Vec<Migration>),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiStreamResponse {
    pub(crate) request_id: usize,
    pub(crate) result: Result<ApiStreamResponsePayload, Error>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum ApiStreamResponsePayload {
    Execute(Result<usize, Error>),
    Transaction(Result<Vec<Result<usize, Error>>, Error>),
    Batch(Vec<Result<usize, Error>>),
    Migrate(Result<(), Error>),
}

#[derive(Debug)]
enum WsWriteMsg {
    Payload(ApiStreamResponse),
    Break,
}

async fn handle_socket_concurrent(
    state: AppStateExt,
    socket: upgrade::UpgradeFut,
) -> Result<(), fastwebsockets::WebSocketError> {
    let mut ws = socket.await?;
    ws.set_auto_close(true);

    // let mut ws = fastwebsockets::FragmentCollector::new(socket.await?);

    let client_id = match HandshakeSecret::server(&mut ws, state.secret_api.as_bytes()).await {
        Ok(id) => id,
        Err(err) => {
            error!("Error during WebSocket handshake: {}", err);
            ws.write_frame(Frame::close(1000, b"Invalid Handshake"))
                .await?;
            return Ok(());
        }
    };

    // make sure to NEVER loose the result of an execute from remote!
    // if we received one which is being executed and the TCP stream dies in between, we MUST ENSURE
    // that in case it was an Ok(_), the result gets to the client! Otherwise with retry logic we might
    // end up modifying something twice!
    let (buf_tx, buf_rx) = state
        .client_buffers
        .get(&client_id)
        .expect("Client ID to always be in client_buffers");

    let (tx_write, rx_write) = flume::unbounded::<WsWriteMsg>();
    // let (tx_read, rx_read) = flume::unbounded();

    // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
    let (rx, mut write) = ws.split(tokio::io::split);
    // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
    let mut read = FragmentCollectorRead::new(rx);

    info!("Emptying buffered Client Stream responses");
    while let Ok(payload) = buf_rx.try_recv() {
        let frame = Frame::binary(Payload::Borrowed(&payload));
        if let Err(err) = write.write_frame(frame).await {
            // if we error again, put the payload back into the buffer and exit
            let _ = buf_tx.send_async(payload).await;
            error!("Error during WebSocket handshake: {}", err);
            return Ok(());
        }
    }

    let buf_tx = buf_tx.clone();
    let handle_write = task::spawn(async move {
        while let Ok(req) = rx_write.recv_async().await {
            match req {
                WsWriteMsg::Payload(resp) => {
                    let bytes = bincode::serialize(&resp).unwrap();
                    let frame = Frame::binary(Payload::Borrowed(&bytes));
                    if let Err(err) = write.write_frame(frame).await {
                        error!("Error during WebSocket handshake: {}", err);
                        // if we have a WebSocket error, save all open requests into the client_buffer
                        let payload = bincode::serialize(&resp).unwrap();
                        buf_tx
                            .send_async(payload)
                            .await
                            .expect("client_buffer to always be working");

                        break;
                    }
                }
                WsWriteMsg::Break => {
                    // we ignore any errors here since it may be possible that the reader
                    // has closed already - we just try a graceful connection close
                    let _ = write
                        .write_frame(Frame::close(1000, b"Invalid Request"))
                        .await;
                    warn!("server stream break message");
                    break;
                }
            }
        }

        warn!("emptying server stream writer channel into buffer");
        while let Ok(req) = rx_write.recv_async().await {
            if let WsWriteMsg::Payload(resp) = req {
                let payload = bincode::serialize(&resp).unwrap();
                buf_tx
                    .send_async(payload)
                    .await
                    .expect("client_buffer to always be working");
            }
        }

        warn!("server stream exiting");
    });

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
                        // ws.write_frame(Frame::close(1000, b"Error deserializing ApiStreamRequest"))
                        //     .await?;
                        let _ = tx_write.send_async(WsWriteMsg::Break).await;
                        break;
                    }
                }
            }
            _ => {
                let _ = tx_write.send_async(WsWriteMsg::Break).await;
                // ws.write_frame(Frame::close(1000, b"Invalid Request"))
                //     .await?;
                break;
            }
        };

        let state = state.clone();
        let tx_write = tx_write.clone();
        task::spawn(async move {
            let res = match req.payload {
                ApiStreamRequestPayload::Execute(sql) => {
                    match state.raft.client_write(QueryWrite::Execute(sql)).await {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Execute(res) => res.result,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id: req.request_id,
                                result: Ok(ApiStreamResponsePayload::Execute(res)),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id: req.request_id,
                            result: Ok(ApiStreamResponsePayload::Execute(Err(Error::from(err)))),
                        },
                    }
                }

                ApiStreamRequestPayload::Transaction(queries) => {
                    match state
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
                                request_id: req.request_id,
                                result: Ok(ApiStreamResponsePayload::Transaction(res)),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id: req.request_id,
                            result: Ok(ApiStreamResponsePayload::Execute(Err(Error::from(err)))),
                        },
                    }
                }

                ApiStreamRequestPayload::Batch(sql) => {
                    match state.raft.client_write(QueryWrite::Batch(sql)).await {
                        Ok(resp) => {
                            let resp: crate::Response = resp.data;
                            let res = match resp {
                                crate::Response::Batch(res) => res,
                                _ => unreachable!(),
                            };
                            ApiStreamResponse {
                                request_id: req.request_id,
                                result: Ok(ApiStreamResponsePayload::Batch(res.result)),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id: req.request_id,
                            result: Ok(ApiStreamResponsePayload::Execute(Err(Error::from(err)))),
                        },
                    }
                }

                ApiStreamRequestPayload::Migrate(migrations) => {
                    match state
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
                                request_id: req.request_id,
                                result: Ok(ApiStreamResponsePayload::Migrate(res)),
                            }
                        }
                        Err(err) => ApiStreamResponse {
                            request_id: req.request_id,
                            result: Ok(ApiStreamResponsePayload::Execute(Err(Error::from(err)))),
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

// TODO
// - query_one
// - query_optional
// - Transaction read + write
// - query_simple
// - Batch (same as simple?)

pub(crate) async fn query_map<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<Vec<T>, Error>
where
    T: for<'r> From<&'r rusqlite::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let stmt: Cow<'static, str> = stmt.into();
    let conn = state.read_pool.get().await?;

    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(stmt.as_ref())?;

        let mut idx = 1;
        for param in params {
            stmt.raw_bind_parameter(idx, param.into_sql())?;
            idx += 1;
        }

        let mut rows = stmt.raw_query();
        let mut res = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            res.push(T::from(row));
        }
        Ok::<Vec<T>, Error>(res)
    })
    .await?
}

pub(crate) async fn query_map_one<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<T, Error>
where
    T: for<'r> From<&'r rusqlite::Row<'r>> + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_map(state, stmt, params).await?;
    if rows.is_empty() {
        Err(Error::Sqlite("no rows returned".into()))
    } else {
        Ok(rows.swap_remove(0))
    }
}

pub(crate) async fn query_as<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<Vec<T>, Error>
where
    T: DeserializeOwned + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let stmt: Cow<'static, str> = stmt.into();
    let conn = state.read_pool.get().await?;

    task::spawn_blocking(move || {
        let mut stmt = conn.prepare_cached(stmt.as_ref())?;

        let mut idx = 1;
        for param in params {
            stmt.raw_bind_parameter(idx, param.into_sql())?;
            idx += 1;
        }

        let mut rows = serde_rusqlite::from_rows::<T>(stmt.raw_query());
        let mut res = Vec::new();
        while let Some(Ok(ty)) = rows.next() {
            res.push(ty);
        }
        Ok::<Vec<T>, Error>(res)
    })
    .await?
}

pub(crate) async fn query_as_one<T, S>(
    state: &Arc<AppState>,
    stmt: S,
    params: Params,
) -> Result<T, Error>
where
    T: DeserializeOwned + Send + 'static,
    S: Into<Cow<'static, str>>,
{
    let mut rows: Vec<T> = query_as(state, stmt, params).await?;
    if rows.is_empty() {
        Err(Error::Sqlite("no rows returned".into()))
    } else {
        Ok(rows.swap_remove(0))
    }
}
