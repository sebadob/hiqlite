use crate::{Client, Error};
use tokio::sync::watch;

pub struct ShutdownHandle {
    client: Client,
    finished: watch::Receiver<Option<super::ShutdownResult>>,
    rx_shutdown: watch::Receiver<bool>,
}

impl ShutdownHandle {
    /// Wait for an OS signal or for another caller to complete shutdown.
    ///
    /// # Errors
    ///
    /// Returns the same shutdown errors as [`Client::shutdown`].
    pub async fn wait(&mut self) -> Result<(), Error> {
        tokio::select! {
            _ = self.rx_shutdown.wait_for(|signal| *signal) => {}
            _ = self.finished.wait_for(Option::is_some) => {}
        }
        self.client.shutdown().await
    }
}

impl Client {
    ///
    /// # Errors
    ///
    /// [`Error::Error`] for a remote client - only a node that runs its own Raft has something
    /// a handle can shut down.
    ///
    /// # Panics
    ///
    /// If a Ctrl-C handler is already registered and `ctrlc` therefore refuses to install its
    /// own.
    #[allow(unused)]
    pub fn shutdown_handle(&self) -> Result<ShutdownHandle, Error> {
        if let Some(finished) = self.inner.tx_shutdown.as_ref() {
            let (tx, rx_shutdown) = watch::channel(false);

            ctrlc::set_handler(move || {
                let _ = tx.send(true);
            })
            .expect("Error setting shutdown handle");

            Ok(ShutdownHandle {
                client: self.clone(),
                finished: finished.subscribe(),
                rx_shutdown,
            })
        } else {
            Err(Error::Error(
                "A shutdown handle can only be registered for local clients".into(),
            ))
        }
    }
}
