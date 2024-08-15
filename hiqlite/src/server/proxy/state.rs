use crate::store::state_machine::memory::notify_handler::NotifyRequest;
use crate::Client;

pub struct AppStateProxy {
    pub client: Client,
    pub secret_api: String,
    pub tx_notify: flume::Sender<NotifyRequest>,
    // pub dashboard_password: String,
}
