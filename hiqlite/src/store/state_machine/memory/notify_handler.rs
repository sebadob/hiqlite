use tokio::task;
use tracing::{debug, error, info, warn};

pub enum NotifyRequest {
    Notify((i64, Vec<u8>)),
    Listen((flume::Sender<(i64, Vec<u8>)>)),
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
    let mut listeners: Vec<flume::Sender<(i64, Vec<u8>)>> = Vec::new();
    let mut remove_indexes = Vec::new();

    while let Ok(req) = rx_req.recv_async().await {
        match req {
            NotifyRequest::Notify(payload) => {
                debug!("new notification from {}", payload.0);

                for (idx, listener) in listeners.iter().enumerate() {
                    // unbounded channels can never block
                    if let Err(err) = listener.send(payload.clone()) {
                        error!("Error sending listener Notification: {}", err);
                        remove_indexes.push(idx);
                    }
                }

                // unbounded channels can never block
                if let Err(err) = tx_local.send(payload) {
                    error!("Error sending local Notification: {}", err);
                    break;
                }

                while let Some(idx) = remove_indexes.pop() {
                    info!("Removing Notification Listener at position {}", idx);
                    listeners.swap_remove(idx);
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
