use crate::store::state_machine::memory::notify_handler::NotifyRequest;
use crate::{Client, Error};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct AppStateProxy {
    pub client: Client,
    pub secret_api: String,
    pub tx_notify: flume::Sender<NotifyRequest>,
    pub active_streams_permits: Arc<Semaphore>,
}

impl AppStateProxy {
    pub async fn get_stream_permit(&self) -> Result<OwnedSemaphorePermit, Error> {
        tokio::time::timeout(
            Duration::from_secs(10),
            self.active_streams_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| {
            Error::Timeout("Stream request timed out - max connections reached".to_string())
        })?
        .map_err(|_| Error::Request("Server is shutting down".to_string()))
    }
}
