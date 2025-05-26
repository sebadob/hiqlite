use crate::app_state::AppState;
use crate::client::stream::ClientStreamReq;
use crate::{Client, Error};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;
use tracing::{info, warn};

pub struct ShutdownHandle {
    state: Arc<AppState>,
    #[cfg(feature = "cache")]
    tx_client_cache: flume::Sender<ClientStreamReq>,
    #[cfg(feature = "sqlite")]
    tx_client_db: flume::Sender<ClientStreamReq>,
    tx_shutdown: Option<watch::Sender<bool>>,
    rx_shutdown: watch::Receiver<bool>,
}

impl ShutdownHandle {
    pub async fn wait(&mut self) -> Result<(), Error> {
        let _ = self.rx_shutdown.changed().await;
        info!("ShutdownHandle received shutdown signal - shutting down the Raft node now");

        if time::timeout(
            Duration::from_secs(15),
            Client::shutdown_execute(
                &self.state,
                #[cfg(feature = "cache")]
                &self.tx_client_cache,
                #[cfg(feature = "sqlite")]
                &self.tx_client_db,
                &self.tx_shutdown,
            ),
        )
        .await
        .is_err()
        {
            warn!("Timeout reached while waiting for Client::shutdown_execute");
        }
        Ok(())
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
                #[cfg(feature = "cache")]
                tx_client_cache: self.inner.tx_client_cache.clone(),
                #[cfg(feature = "sqlite")]
                tx_client_db: self.inner.tx_client_db.clone(),
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
