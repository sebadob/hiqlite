use crate::Error;
use axum::response::sse;
use cryptr::utils::b64_encode;
use tokio::task;
use tracing::{debug, error, info, warn};

pub enum NotifyRequest {
    Notify((i64, Vec<u8>)),
    Listen((flume::Sender<Result<sse::Event, Error>>)),
}

pub fn spawn() -> (
    flume::Sender<NotifyRequest>,
    flume::Receiver<(i64, Vec<u8>)>,
) {
    let (tx_req, rx_req) = flume::unbounded();
    let (tx_local, rx_local) = flume::unbounded();
    task::spawn(handler(rx_req, tx_local));
    (tx_req, rx_local)
}

async fn handler(rx_req: flume::Receiver<NotifyRequest>, tx_local: flume::Sender<(i64, Vec<u8>)>) {
    let mut listeners: Vec<flume::Sender<Result<sse::Event, Error>>> = Vec::new();
    let mut remove_indexes = Vec::new();

    while let Ok(req) = rx_req.recv_async().await {
        match req {
            NotifyRequest::Notify((ts, data)) => {
                debug!("new notification from {}", ts);

                if !listeners.is_empty() {
                    let event = sse::Event::default().data(format!("{} {}", ts, b64_encode(&data)));

                    for (idx, listener) in listeners.iter().enumerate() {
                        // unbounded channels can never block
                        if let Err(err) = listener.send(Ok(event.clone())) {
                            error!("Error sending listener Notification: {}", err);
                            remove_indexes.push(idx);
                        }
                    }

                    while let Some(idx) = remove_indexes.pop() {
                        info!("Removing Notification Listener at position {}", idx);
                        listeners.swap_remove(idx);
                    }
                }

                // unbounded channels can never block
                if let Err(err) = tx_local.send((ts, data)) {
                    error!("Error sending local Notification: {}", err);
                    break;
                }
            }
            NotifyRequest::Listen(tx) => {
                info!("New notification listener subscribed");
                listeners.push(tx);
            }
        }
    }

    warn!("Listen / Notify handler exiting");
}
