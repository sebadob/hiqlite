use crate::helpers::{deserialize, serialize};
use crate::network::api::{
    ApiStreamRequest, ApiStreamRequestPayload, ApiStreamResponse, ApiStreamResponsePayload,
    WsWriteMsg,
};
use crate::network::handshake::HandshakeSecret;
use crate::server::proxy::handlers::AppStateExt;
use crate::store::state_machine::sqlite::state_machine::Query;
use crate::{Client, Error};
use fastwebsockets::{upgrade, FragmentCollectorRead, Frame, OpCode, Payload};
use std::ops::Deref;
use tokio::task;
use tracing::{debug, error, warn};

pub async fn handle_socket(
    state: AppStateExt,
    socket: upgrade::UpgradeFut,
) -> Result<(), fastwebsockets::WebSocketError> {
    let mut ws = socket.await?;
    ws.set_auto_close(true);

    if let Err(err) = HandshakeSecret::server(&mut ws, state.secret_api.as_bytes()).await {
        error!("Error during WebSocket handshake: {}", err);
        ws.write_frame(Frame::close(1000, b"Invalid Handshake"))
            .await?;
        return Ok(());
    };

    let (tx_write, rx_write) = flume::bounded::<WsWriteMsg>(1);
    // let (tx_write, rx_write) = flume::unbounded::<WsWriteMsg>();
    // let (tx_read, rx_read) = flume::unbounded();

    // TODO splitting needs `unstable-split` feature right now but is about to be stabilized soon
    let (rx, mut write) = ws.split(tokio::io::split);
    // IMPORTANT: the reader is NOT CANCEL SAFE in v0.8!
    let mut read = FragmentCollectorRead::new(rx);

    let handle_write = task::spawn(async move {
        while let Ok(req) = rx_write.recv_async().await {
            match req {
                WsWriteMsg::Payload(resp) => {
                    let bytes = serialize(&resp).unwrap();
                    let frame = Frame::binary(Payload::Borrowed(&bytes));
                    if let Err(err) = write.write_frame(frame).await {
                        error!("Error during WebSocket write: {}", err);
                        // // if we have a WebSocket error, save all open requests into the client_buffer
                        // let payload = bincode::serialize(&resp).unwrap();
                        // buf_tx
                        //     .send_async(payload)
                        //     .await
                        //     .expect("client_buffer to always be working");

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

        // warn!("emptying server stream writer channel into buffer");
        // while let Ok(req) = rx_write.recv_async().await {
        //     if let WsWriteMsg::Payload(resp) = req {
        //         let payload = bincode::serialize(&resp).unwrap();
        //         buf_tx
        //             .send_async(payload)
        //             .await
        //             .expect("client_buffer to always be working");
        //     }
        // }

        warn!("server stream exiting");
    });

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
        let req = match frame.opcode {
            OpCode::Close => {
                warn!("received Close frame in server stream");
                break;
            }
            OpCode::Binary => {
                let bytes = frame.payload.deref();
                match deserialize::<ApiStreamRequest>(bytes) {
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
            let client = &state.client;
            // exchange orig req id for our own to avoid conflicts
            let request_id = req.request_id;

            let res = match req.payload {
                ApiStreamRequestPayload::Execute(sql) => {
                    let res = client.execute(sql.sql, sql.params).await;
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Execute(res),
                    }
                }

                ApiStreamRequestPayload::ExecuteReturning(query) => {
                    match client.execute_returning_req(query.clone()).await {
                        Ok(res) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::ExecuteReturning(Ok(res)),
                        },
                        Err(err) => {
                            if client
                                .was_leader_update_error(
                                    &err,
                                    &client.inner.leader_db,
                                    &client.inner.tx_client_db,
                                )
                                .await
                            {
                                let res = client.execute_returning_req(query).await;
                                ApiStreamResponse {
                                    request_id,
                                    result: ApiStreamResponsePayload::ExecuteReturning(res),
                                }
                            } else {
                                ApiStreamResponse {
                                    request_id,
                                    result: ApiStreamResponsePayload::ExecuteReturning(Err(err)),
                                }
                            }
                        }
                    }
                }

                ApiStreamRequestPayload::Transaction(queries) => {
                    let res = match client.txn_execute(queries.clone()).await {
                        Ok(res) => Ok(res),
                        Err(err) => {
                            if client
                                .was_leader_update_error(
                                    &err,
                                    &client.inner.leader_db,
                                    &client.inner.tx_client_db,
                                )
                                .await
                            {
                                client.txn_execute(queries).await
                            } else {
                                Err(err)
                            }
                        }
                    };
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Transaction(res),
                    }
                }

                ApiStreamRequestPayload::QueryConsistent(q) => {
                    query(client, request_id, q, true).await
                }

                ApiStreamRequestPayload::Batch(sql) => {
                    let res = client.batch(sql).await;
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Batch(res),
                    }
                }

                ApiStreamRequestPayload::Migrate(migrations) => {
                    let res = match client.migrate_execute(migrations.clone()).await {
                        Ok(res) => Ok(res),
                        Err(err) => {
                            if client
                                .was_leader_update_error(
                                    &err,
                                    &client.inner.leader_db,
                                    &client.inner.tx_client_db,
                                )
                                .await
                            {
                                client.migrate_execute(migrations).await
                            } else {
                                Err(err)
                            }
                        }
                    };
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Migrate(res),
                    }
                }

                ApiStreamRequestPayload::Backup(_node_id) => {
                    let res = client.backup().await;
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Backup(res),
                    }
                }

                ApiStreamRequestPayload::Query(q) => query(client, request_id, q, false).await,

                ApiStreamRequestPayload::KV(cache_req) => {
                    let res = client.cache_req_retry(cache_req, false).await;
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::KV(res),
                    }
                }

                ApiStreamRequestPayload::KVGet(cache_req) => {
                    let res = client.cache_req_retry(cache_req, true).await;
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::KV(res),
                    }
                }

                ApiStreamRequestPayload::MembershipRemove(_) => {
                    error!("Received ApiStreamRequestPayload::MembershipRemove which should never happen for the proxy");
                    return;
                }

                ApiStreamRequestPayload::LockAwait(cache_req) => {
                    match client.lock_req_retry(cache_req, true).await {
                        Ok(res) => ApiStreamResponse {
                            request_id,
                            result: ApiStreamResponsePayload::Lock(res),
                        },
                        Err(_) => {
                            todo!("how should be handle await errors? wrap state inside inner result just for the proxy or retry endlessly?")
                        } // Err(err) => ApiStreamResponse {
                          //     request_id,
                          //     result: ApiStreamResponsePayload::Lock(Err(err)),
                          // },
                    }
                }

                ApiStreamRequestPayload::Notify(cache_req) => {
                    let res = client.notify_req(cache_req).await;
                    ApiStreamResponse {
                        request_id,
                        result: ApiStreamResponsePayload::Notify(res),
                    }
                }
            };

            if let Err(err) = tx_write.send_async(WsWriteMsg::Payload(res)).await {
                error!("Error sending payload to tx_write: {}", err);
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

#[inline]
async fn query(
    client: &Client,
    request_id: usize,
    query: Query,
    consistent: bool,
) -> ApiStreamResponse {
    let res = match client.query_remote_req(query.clone(), consistent).await {
        Ok(res) => Ok(res),
        Err(err) => {
            if client
                .was_leader_update_error(&err, &client.inner.leader_db, &client.inner.tx_client_db)
                .await
            {
                client.query_remote_req(query, consistent).await
            } else {
                Err(err)
            }
        }
    };

    let result = if consistent {
        ApiStreamResponsePayload::QueryConsistent(res)
    } else {
        ApiStreamResponsePayload::Query(res)
    };
    ApiStreamResponse { request_id, result }
}
