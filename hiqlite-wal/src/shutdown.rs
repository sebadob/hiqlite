use crate::error::Error;
use crate::Action;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct ShutdownSender {
    tx: flume::Sender<Action>,
}

impl ShutdownSender {
    pub(crate) fn new(tx: flume::Sender<Action>) -> Self {
        Self { tx }
    }

    #[allow(unused)]
    pub async fn shutdown(self) -> Result<(), Error> {
        let (tx_ack, ack) = oneshot::channel();
        self.tx.send_async(Action::Shutdown(tx_ack)).await?;
        ack.await?;
        Ok(())
    }
}
