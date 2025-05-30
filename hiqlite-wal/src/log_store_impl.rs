use crate::{reader, writer, LogStore, LogStoreReader};
use bincode::error::{DecodeError, EncodeError};
use openraft::storage::{LogFlushed, RaftLogStorage};
use openraft::{
    AnyError, ErrorSubject, ErrorVerb, LogId, OptionalSend, RaftLogId, RaftLogReader,
    RaftTypeConfig, StorageError, StorageIOError, Vote,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::Bound;
use std::fmt::Debug;
use std::ops::RangeBounds;
use tokio::sync::oneshot;
use tracing::debug;

#[inline(always)]
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, EncodeError> {
    // We are using the legacy config on purpose here. It uses fixed-width integer fields, which
    // uses a bit more space, but is faster.
    bincode::serde::encode_to_vec(value, bincode::config::legacy())
}

#[inline(always)]
pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    bincode::serde::decode_from_slice::<T, _>(bytes, bincode::config::legacy()).map(|(res, _)| res)
}

impl<T> RaftLogReader<T> for LogStore<T>
where
    T: RaftTypeConfig,
{
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<T::Entry>, StorageError<T::NodeId>> {
        try_get_log_entries::<T, _>(&self.reader, range).await
    }
}

impl<T> RaftLogReader<T> for LogStoreReader<T>
where
    T: RaftTypeConfig,
{
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<T::Entry>, StorageError<T::NodeId>> {
        try_get_log_entries::<T, _>(&self.tx, range).await
    }
}

#[tracing::instrument(skip_all)]
#[inline(always)]
async fn try_get_log_entries<
    T: RaftTypeConfig,
    RB: RangeBounds<u64> + Clone + Debug + OptionalSend,
>(
    tx: &flume::Sender<reader::Action>,
    range: RB,
) -> Result<Vec<T::Entry>, StorageError<T::NodeId>> {
    let from = match range.start_bound() {
        Bound::Included(i) => *i,
        Bound::Excluded(i) => *i + 1,
        Bound::Unbounded => 0,
    };
    let until = match range.end_bound() {
        Bound::Included(i) => *i,
        Bound::Excluded(i) => *i - 1,
        Bound::Unbounded => unreachable!(),
    };
    debug!("Entering try_get_log_entries() from {from} until {until}");

    let mut res: Vec<T::Entry> = Vec::with_capacity((until - from) as usize + 1);

    let (ack, rx) = flume::bounded(1);
    tx.send_async(reader::Action::Logs { from, until, ack })
        .await
        .expect("LogsReader to always be listening");

    while let Some(data_res) = rx.recv_async().await.unwrap() {
        let data = data_res.map_err(|err| StorageError::IO {
            source: StorageIOError::read_logs(&err),
        })?;
        let entry = deserialize::<T::Entry>(&data).map_err(|err| StorageError::IO {
            source: StorageIOError::<T::NodeId>::read_logs(&err),
        })?;
        res.push(entry);
    }

    Ok(res)
}

impl<T> RaftLogStorage<T> for LogStore<T>
where
    T: RaftTypeConfig,
{
    type LogReader = LogStoreReader<T>;

    #[tracing::instrument(skip_all)]
    async fn get_log_state(&mut self) -> Result<openraft::LogState<T>, StorageError<T::NodeId>> {
        debug!("Entering get_log_state()");

        let (ack, rx) = oneshot::channel();
        self.reader
            .send_async(reader::Action::LogState(ack))
            .await
            .map_err(|err| {
                StorageIOError::new(ErrorSubject::Logs, ErrorVerb::Read, AnyError::new(&err))
            })?;

        let log_state = rx.await.unwrap().map_err(|err| {
            StorageIOError::new(ErrorSubject::Logs, ErrorVerb::Read, AnyError::new(&err))
        })?;

        let last_purged_log_id = if let Some(bytes) = log_state.last_purged_log_id {
            Some(deserialize(&bytes).map_err(|err| {
                StorageIOError::new(ErrorSubject::Logs, ErrorVerb::Read, AnyError::new(&err))
            })?)
        } else {
            None
        };
        let last_log_id = if let Some(bytes) = log_state.last_log {
            Some(deserialize(&bytes).map_err(|err| {
                StorageIOError::new(ErrorSubject::Logs, ErrorVerb::Read, AnyError::new(&err))
            })?)
        } else {
            None
        };

        Ok(openraft::LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn get_log_reader(&mut self) -> Self::LogReader {
        debug!("Entering get_log_reader()");

        self.spawn_reader()
            .expect("Error spawning additional LogStoreReader")
    }

    #[tracing::instrument(skip_all)]
    #[tracing::instrument(level = "trace", skip(self))]
    async fn save_vote(&mut self, vote: &Vote<T::NodeId>) -> Result<(), StorageError<T::NodeId>> {
        debug!("Entering save_vote(): {:?}", vote);

        let (ack, rx) = oneshot::channel();
        self.writer
            .send_async(writer::Action::Vote {
                value: serialize(vote).unwrap(),
                ack,
            })
            .await
            .expect("Writer to always be running");

        rx.await.unwrap().map_err(|err| StorageError::IO {
            source: StorageIOError::write_vote(&err),
        })?;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn read_vote(&mut self) -> Result<Option<Vote<T::NodeId>>, StorageError<T::NodeId>> {
        debug!("Entering read_vote()");

        let (ack, rx) = oneshot::channel();

        self.reader
            .send_async(reader::Action::Vote(ack))
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?;

        let vote = rx
            .await
            .unwrap()
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?
            .map(|b| deserialize(&b).unwrap());

        Ok(vote)
    }

    #[tracing::instrument(skip_all)]
    #[tracing::instrument(level = "trace", skip_all)]
    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<T>,
    ) -> Result<(), StorageError<T::NodeId>>
    where
        I: IntoIterator<Item = T::Entry> + Send,
        I::IntoIter: Send,
    {
        debug!("Entering append()");

        let (tx, rx) = flume::bounded(1);
        let (ack, ack_rx) = oneshot::channel();

        let callback = Box::new(move || callback.log_io_completed(Ok(())));
        self.writer
            .send_async(writer::Action::Append { rx, callback, ack })
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        for entry in entries {
            let data = serialize(&entry).unwrap();
            tx.send_async(Some((entry.get_log_id().index, data)))
                .await
                .map_err(|err| StorageIOError::write_logs(&err))?;
        }
        tx.send_async(None)
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        ack_rx
            .await
            .unwrap()
            .map_err(|err| StorageIOError::write_logs(&err))?;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn truncate(&mut self, log_id: LogId<T::NodeId>) -> Result<(), StorageError<T::NodeId>> {
        debug!("truncate(): [{:?}, +oo)", log_id);

        let (ack, rx) = oneshot::channel();
        self.writer
            .send_async(writer::Action::Remove {
                from: log_id.index,
                until: u64::MAX,
                last_log: None,
                ack,
            })
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?;

        rx.await.unwrap().map_err(|err| StorageError::IO {
            source: StorageIOError::read_vote(&err),
        })
    }

    #[tracing::instrument(skip_all)]
    async fn purge(&mut self, log_id: LogId<T::NodeId>) -> Result<(), StorageError<T::NodeId>> {
        debug!("purge(): [0, {:?}]", log_id);

        let last_log = Some(serialize(&log_id).unwrap());
        let (ack, rx) = oneshot::channel();
        self.writer
            .send_async(writer::Action::Remove {
                from: 0,
                until: log_id.index + 1,
                last_log,
                ack,
            })
            .await
            .map_err(|err| StorageError::IO {
                source: StorageIOError::read_vote(&err),
            })?;

        rx.await.unwrap().map_err(|err| StorageError::IO {
            source: StorageIOError::read_vote(&err),
        })
    }
}
