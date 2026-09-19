use crate::Client;
use crate::store::state_machine::memory::notify_handler::NotifyRequest;

pub struct AppStateProxy {
    pub client: Client,
    pub secret_api: String,
    pub tx_notify: flume::Sender<NotifyRequest>,
    // pub dashboard_password: String,
}
