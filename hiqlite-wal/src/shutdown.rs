use crate::error::Error;
use crate::{reader, writer};
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct ShutdownHandle {
    tx_write: flume::Sender<writer::Action>,
    tx_read: flume::Sender<reader::Action>,
}

impl ShutdownHandle {
    pub(crate) fn new(
        tx_write: flume::Sender<writer::Action>,
        tx_read: flume::Sender<reader::Action>,
    ) -> Self {
        Self { tx_write, tx_read }
    }

    #[allow(unused)]
    pub async fn shutdown(&self) -> Result<(), Error> {
        let (tx_ack, ack) = oneshot::channel();
        self.tx_write
            .send_async(writer::Action::Shutdown(tx_ack))
            .await?;
        ack.await?;

        self.tx_read.send_async(reader::Action::Shutdown);

        Ok(())
    }
}
