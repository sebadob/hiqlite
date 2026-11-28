use crate::error::Error;
use crate::log_store_impl::{WRITER_STOPPED, response_or_stopped};
use crate::{reader, writer};
use std::io;
use std::sync::Arc;
use tokio::sync::{OnceCell, oneshot, watch};
use tracing::{debug, info};

/// A `ShutdownHandle` making it possible to do a graceful shutdown of the WAL logs tasks.
/// You must call `.shutdown()` for a clean shutdown.
#[derive(Debug, Clone)]
pub struct ShutdownHandle {
    tx_write: flume::Sender<writer::Action>,
    writer_closed: flume::Receiver<()>,
    readers: Arc<reader::Readers>,
    finished: Arc<OnceCell<watch::Receiver<ShutdownResult>>>,
}

type ShutdownResult = Option<Result<(), Error>>;

impl ShutdownHandle {
    pub(crate) fn new(
        tx_write: flume::Sender<writer::Action>,
        writer_closed: flume::Receiver<()>,
        readers: Arc<reader::Readers>,
    ) -> Self {
        Self {
            tx_write,
            writer_closed,
            readers,
            finished: Arc::new(OnceCell::new()),
        }
    }

    /// Release the WAL mappings even if a caller stops waiting. Clones share one result.
    ///
    /// # Errors
    ///
    /// [`Error::Internal`] when the WAL writer task is no longer reachable, and
    /// [`Error::Generic`] when a shutdown acknowledgement is never sent.
    #[allow(unused)]
    pub async fn shutdown(&self) -> Result<(), Error> {
        let mut finished = self
            .finished
            .get_or_init(|| async {
                let (done, finished) = watch::channel(None);
                let readers = self.readers.clone();
                let writer = self.tx_write.clone();
                let writer_closed = self.writer_closed.clone();
                tokio::spawn(async move {
                    let _ = done.send_replace(Some(shutdown(readers, writer, writer_closed).await));
                });
                finished
            })
            .await
            .clone();

        let result = finished
            .wait_for(Option::is_some)
            .await
            .map_err(|_| Error::Generic("WAL shutdown task stopped".into()))?;
        match result.as_ref() {
            Some(Err(Error::Internal(message))) => Err(Error::Internal(message.clone())),
            Some(Err(Error::Generic(message))) => Err(Error::Generic(message.clone())),
            Some(Err(err)) => Err(Error::Generic(err.to_string().into())),
            _ => Ok(()),
        }
    }
}

async fn shutdown(
    readers: Arc<reader::Readers>,
    writer: flume::Sender<writer::Action>,
    writer_closed: flume::Receiver<()>,
) -> Result<(), Error> {
    debug!("Shutting down WAL readers");
    readers.shutdown().await;

    let (tx_ack, ack) = oneshot::channel();
    debug!("Sending Action::Shutdown to WAL writer");
    writer.send_async(writer::Action::Shutdown(tx_ack)).await?;
    // A shutdown queued behind a writer that already stopped is never answered.
    response_or_stopped(&writer_closed, WRITER_STOPPED, async {
        ack.await.map_err(io::Error::other)
    })
    .await?;
    info!("WAL writer Shutdown complete");

    Ok(())
}
