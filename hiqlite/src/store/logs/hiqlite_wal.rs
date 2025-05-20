use crate::helpers::{deserialize, serialize};
use crate::store::state_machine::sqlite::TypeConfigSqlite;
use crate::store::StorageResult;
use crate::NodeId;
use hiqlite_wal::{reader, writer, LogStore, LogStoreReader};
use openraft::storage::{LogFlushed, RaftLogStorage};
use openraft::{
    AnyError, Entry, ErrorSubject, ErrorVerb, LogId, OptionalSend, RaftLogReader, RaftTypeConfig,
    StorageError, StorageIOError, Vote,
};
use std::collections::Bound;
use std::fmt::Debug;
use std::future::Future;
use std::ops::RangeBounds;
use tokio::sync::oneshot;
use tracing::debug;

impl RaftLogReader<TypeConfigSqlite> for LogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> StorageResult<Vec<Entry<TypeConfigSqlite>>> {
        try_get_log_entries(&self.reader, range).await
    }
}

impl RaftLogReader<TypeConfigSqlite> for LogStoreReader {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> StorageResult<Vec<Entry<TypeConfigSqlite>>> {
        try_get_log_entries(&self.tx, range).await
    }
}

#[inline(always)]
async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
    tx: &flume::Sender<reader::Action>,
    range: RB,
) -> StorageResult<Vec<Entry<TypeConfigSqlite>>> {
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

    let mut res = Vec::with_capacity((until - from) as usize + 1);

    let (ack, rx) = flume::bounded(1);
    tx.send_async(reader::Action::Logs { from, until, ack })
        .await
        .expect("LogsReader to always be listening");

    while let Some(data_res) = rx.recv_async().await.unwrap() {
        let data = data_res.map_err(|err| StorageError::IO {
            source: StorageIOError::read_logs(&err),
        })?;
        let entry = deserialize::<Entry<_>>(&data).map_err(|err| StorageError::IO {
            source: StorageIOError::<NodeId>::read_logs(&err),
        })?;
        res.push(entry);
    }

    Ok(res)
}

impl RaftLogStorage<TypeConfigSqlite> for LogStore {
    type LogReader = LogStoreReader;

    async fn get_log_state(&mut self) -> StorageResult<openraft::LogState<TypeConfigSqlite>> {
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

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.spawn_reader()
            .expect("Error spawning additional LogStoreReader")
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
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

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
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

    #[tracing::instrument(level = "trace", skip_all)]
    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<TypeConfigSqlite>,
    ) -> StorageResult<()>
    where
        I: IntoIterator<Item = Entry<TypeConfigSqlite>> + Send,
        I::IntoIter: Send,
    {
        let (tx, rx) = flume::bounded(1);
        let (ack, ack_rx) = oneshot::channel();

        let callback = Box::new(move || callback.log_io_completed(Ok(())));
        self.writer
            .send_async(writer::Action::Append { rx, callback, ack })
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        for entry in entries {
            let data = serialize(&entry).unwrap();
            tx.send_async(Some((entry.log_id.index, data)))
                .await
                .map_err(|err| StorageIOError::write_logs(&err))?;
        }
        tx.send_async(None)
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        ack_rx
            .await
            .map_err(|err| StorageIOError::write_logs(&err))?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn truncate(&mut self, log_id: LogId<NodeId>) -> StorageResult<()> {
        debug!("delete_log: [{:?}, +oo)", log_id);

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

    #[tracing::instrument(level = "debug", skip(self))]
    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        debug!("delete_log: [0, {:?}]", log_id);

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
