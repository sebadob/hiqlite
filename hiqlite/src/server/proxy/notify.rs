use crate::store::state_machine::memory::notify_handler;
use crate::store::state_machine::memory::notify_handler::NotifyRequest;
use crate::Client;
use tokio::task;
use tracing::error;
use tracing::log::debug;

pub fn spawn_listener(client: Client) -> flume::Sender<NotifyRequest> {
    let (tx_notify, rx_notify) = notify_handler::spawn();
    task::spawn(router(client, tx_notify.clone()));
    task::spawn(listener(rx_notify));
    tx_notify
}

async fn router(client: Client, tx: flume::Sender<NotifyRequest>) {
    loop {
        if let Ok(msg) = client.listen_bytes().await {
            if tx.send_async(NotifyRequest::Notify(msg)).await.is_err() {
                error!("Error sending notification - exiting router");
                break;
            }
        }
    }
}

// we just need to make sure that the channel does not fill up
async fn listener(rx: flume::Receiver<(i64, Vec<u8>)>) {
    while let Ok((ts, _)) = rx.recv_async().await {
        debug!("Event from {}", ts);
    }
}
