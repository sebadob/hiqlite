use crate::Client;
use crate::store::state_machine::memory::notify_handler::NotifyRequest;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct AppStateProxy {
    pub client: Client,
    pub secret_api: String,
    pub tx_notify: flume::Sender<NotifyRequest>,
    pub active_streams_permits: Arc<Semaphore>,
}
