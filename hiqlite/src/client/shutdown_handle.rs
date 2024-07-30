use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::{Client, Error};
use std::sync::Arc;
use tokio::sync::watch;
use tracing::info;

pub struct ShutdownHandle {
    state: Arc<AppState>,
    tx_client: flume::Sender<ClientStreamReq>,
    tx_shutdown: Option<watch::Sender<bool>>,
    rx_shutdown: watch::Receiver<bool>,
}

impl ShutdownHandle {
    pub async fn wait(&mut self) -> Result<(), Error> {
        let _ = self.rx_shutdown.changed().await;
        info!("ShutdownHandle received shutdown signal - shutting down the Raft node now");
        Client::shutdown_execute(&self.state, &self.tx_client, &self.tx_shutdown).await
    }
}

impl Client {
    #[allow(unused)]
    pub fn shutdown_handle(&self) -> Result<ShutdownHandle, Error> {
        if let Some(state) = self.inner.state.clone() {
            let (tx, rx_shutdown) = watch::channel(false);

            ctrlc::set_handler(move || {
                let _ = tx.send(true);
            })
            .expect("Error setting shutdown handle");

            Ok(ShutdownHandle {
                state,
                tx_client: self.inner.tx_client.clone(),
                tx_shutdown: self.inner.tx_shutdown.clone(),
                rx_shutdown,
            })
        } else {
            Err(Error::Error(
                "A shutdown handle can only be registered for local clients".into(),
            ))
        }
    }
}
