use crate::{LogStore, LogStoreReader, error, reader, writer};
use bincode_next::error::{DecodeError, EncodeError};
use openraft::entry::RaftEntry;
use openraft::storage::{IOFlushed, RaftLogReader, RaftLogStorage};
use openraft::type_config::alias::{EntryOf, LogIdOf, VoteOf};
use openraft::{LogState, OptionalSend, RaftTypeConfig};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::Bound;
use std::fmt::Debug;
use std::future::{Future, poll_fn};
use std::io;
use std::ops::RangeBounds;
use std::pin::pin;
use std::sync::PoisonError;
use std::task::Poll;
use tokio::sync::oneshot;
use tracing::{debug, warn};

#[inline]
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, EncodeError> {
    bincode_next::serde::encode_to_vec(value, bincode_next::config::legacy())
}

#[inline]
pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    bincode_next::serde::decode_from_slice::<T, _>(bytes, bincode_next::config::legacy())
        .map(|(res, _)| res)
}

/// A poisoned log store lock can only come from a panic inside the WAL writer or reader, so it is
/// reported as an I/O error and never swallowed.
#[inline]
fn lock_poisoned<T>(err: &PoisonError<T>) -> io::Error {
    io::Error::other(format!("log store lock poisoned: {err}"))
}

impl<T> RaftLogReader<T> for LogStore<T>
where
    T: RaftTypeConfig,
{
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<EntryOf<T>>, io::Error> {
        try_get_log_entries::<T, _>(&self.reader, &self.reader_closed, range).await
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn read_vote(&mut self) -> Result<Option<VoteOf<T>>, io::Error> {
        debug!("Entering read_vote()");

        read_vote(&self.reader, &self.reader_closed).await
    }
}

impl<T> RaftLogReader<T> for LogStoreReader<T>
where
    T: RaftTypeConfig,
{
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<EntryOf<T>>, io::Error> {
        try_get_log_entries::<T, _>(&self.tx, &self.closed, range).await
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn read_vote(&mut self) -> Result<Option<VoteOf<T>>, io::Error> {
        debug!("Entering read_vote()");

        read_vote(&self.tx, &self.closed).await
    }
}

async fn read_vote<V: DeserializeOwned>(
    tx: &flume::Sender<reader::Action>,
    closed: &flume::Receiver<()>,
) -> Result<Option<V>, io::Error> {
    let (ack, rx) = oneshot::channel();
    tx.send_async(reader::Action::Vote(ack))
        .await
        .map_err(|err| io::Error::from(error::Error::from(err)))?;

    let vote = response_or_stopped(closed, READER_STOPPED, async {
        rx.await.map_err(io::Error::other)?.map_err(io::Error::from)
    })
    .await?;
    vote.map(|bytes| deserialize(&bytes).map_err(io::Error::other))
        .transpose()
}

pub(crate) const READER_STOPPED: &str = "WAL reader stopped";
pub(crate) const WRITER_STOPPED: &str = "WAL writer stopped";

/// Awaits `response`, or fails with `stopped` once the thread behind `closed` has exited.
///
/// A request queued behind a thread that exits still owns its reply sender, and the queue keeps
/// it alive for as long as any sender of that channel exists, so the reply alone could be awaited
/// forever.
pub(crate) async fn response_or_stopped<T>(
    closed: &flume::Receiver<()>,
    stopped: &'static str,
    response: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    let mut response = pin!(response);
    let mut closed = pin!(closed.recv_async());
    poll_fn(|cx| match response.as_mut().poll(cx) {
        Poll::Pending if closed.as_mut().poll(cx).is_ready() => match response.as_mut().poll(cx) {
            Poll::Ready(result) => Poll::Ready(result),
            Poll::Pending => Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, stopped))),
        },
        result => result,
    })
    .await
}

#[tracing::instrument(skip_all)]
#[inline]
async fn try_get_log_entries<
    T: RaftTypeConfig,
    RB: RangeBounds<u64> + Clone + Debug + OptionalSend,
>(
    tx: &flume::Sender<reader::Action>,
    closed: &flume::Receiver<()>,
    range: RB,
) -> Result<Vec<EntryOf<T>>, io::Error> {
    let from = match range.start_bound() {
        Bound::Included(i) => *i,
        // `Excluded(u64::MAX)` means "nothing after this" - treat it as an empty range below
        Bound::Excluded(i) => i.checked_add(1).unwrap_or(u64::MAX),
        Bound::Unbounded => 0,
    };
    let until = match range.end_bound() {
        Bound::Included(i) => *i,
        // `Excluded(0)` means "up to but not including log 0" - an empty range
        Bound::Excluded(i) => i.saturating_sub(1),
        // an open-ended range; the reader answers it with everything it holds
        Bound::Unbounded => reader::NO_END,
    };
    debug!("Entering try_get_log_entries() from {from} until {until}");

    if from > until {
        // empty range - nothing to read (also covers `Excluded(u64::MAX)` / `Excluded(0)`)
        return Ok(Vec::new());
    }
    // an open-ended range (`NO_END`) has no size we could reserve up-front
    let cap = if until == reader::NO_END {
        0
    } else {
        (until - from + 1) as usize
    };
    let mut res: Vec<EntryOf<T>> = Vec::with_capacity(cap);

    let (ack, rx) = flume::bounded(1);
    tx.send_async(reader::Action::Logs { from, until, ack })
        .await
        .map_err(|err| io::Error::from(error::Error::from(err)))?;

    response_or_stopped(closed, READER_STOPPED, async {
        while let Some(data_res) = rx.recv_async().await.map_err(io::Error::other)? {
            let data = data_res.map_err(io::Error::from)?;
            let entry = deserialize::<EntryOf<T>>(&data)
                .map_err(|err| io::Error::from(error::Error::from(err)))?;
            res.push(entry);
        }
        Ok(res)
    })
    .await
}

impl<T> RaftLogStorage<T> for LogStore<T>
where
    T: RaftTypeConfig,
    LogIdOf<T>: Serialize + DeserializeOwned,
{
    type LogReader = LogStoreReader<T>;

    #[tracing::instrument(skip_all)]
    async fn get_log_state(&mut self) -> Result<LogState<T>, io::Error> {
        debug!("Entering get_log_state()");

        let (ack, rx) = oneshot::channel();
        self.reader
            .send_async(reader::Action::LogState(ack))
            .await
            .map_err(|err| io::Error::from(error::Error::from(err)))?;

        let log_state = response_or_stopped(&self.reader_closed, READER_STOPPED, async {
            rx.await.map_err(io::Error::other)?.map_err(io::Error::from)
        })
        .await?;

        let last_purged_log_id = if let Some(bytes) = log_state.last_purged_log_id {
            Some(deserialize(&bytes).map_err(|err| io::Error::from(error::Error::from(err)))?)
        } else {
            None
        };
        let last_log_id = if let Some(bytes) = log_state.last_log {
            Some(deserialize(&bytes).map_err(|err| io::Error::from(error::Error::from(err)))?)
        } else {
            None
        };

        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn get_log_reader(&mut self) -> Self::LogReader {
        debug!("Entering get_log_reader()");

        self.spawn_reader()
    }

    /// Keeps the pointer in memory and returns without touching the disk.
    ///
    /// openraft awaits this once per apply batch on its state machine command path, and the value
    /// is only ever read back at startup - it is a startup-only pointer, so a fsync here would be
    /// paid on every batch for no steady-state gain. The WAL writer publishes it into the active
    /// file's header on the flush that is already running, which is the only place it becomes
    /// durable. Losing the newest values is safe: `read_committed` then reports less, and the
    /// missing tail is re-synced from the cluster.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn save_committed(&mut self, committed: Option<LogIdOf<T>>) -> Result<(), io::Error> {
        let bytes = committed
            .map(|id| serialize(&id))
            .transpose()
            .map_err(|err| io::Error::from(error::Error::from(err)))?;
        *self.committed.write().map_err(|err| lock_poisoned(&err))? = bytes;
        Ok(())
    }

    /// Returns the persisted `committed` pointer, clamped to the durable log tail.
    ///
    /// The clamp is what makes the pointer safe. openraft re-applies `(last_applied, committed]`
    /// from the log store at startup and fails hard when the range is not exactly there, so a
    /// pointer above `last_log_id` turns a restart into a startup failure. That is reachable
    /// here, not theoretical: the WAL acknowledges appends before flushing them, and openraft
    /// commits on a quorum of those acks, so after a `kill -9` the log tail can be behind the
    /// committed position. Clamping to the tail replays exactly what survived; the rest comes
    /// from the leader.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn read_committed(&mut self) -> Result<Option<LogIdOf<T>>, io::Error> {
        let saved = self
            .committed
            .read()
            .map_err(|err| lock_poisoned(&err))?
            .clone();
        let Some(bytes) = saved else {
            return Ok(None);
        };
        let committed: LogIdOf<T> =
            deserialize(&bytes).map_err(|err| io::Error::from(error::Error::from(err)))?;

        let last_log_id = self.get_log_state().await?.last_log_id;
        if let Some(last) = last_log_id
            && committed.index > last.index
        {
            warn!(
                "Persisted committed {} is ahead of the durable log tail {} - clamping to the \
                 log tail; the entries in between are re-synced from the cluster",
                committed.index, last.index,
            );
            return Ok(Some(last));
        }

        Ok(Some(committed))
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn save_vote(&mut self, vote: &VoteOf<T>) -> Result<(), io::Error> {
        debug!("Entering save_vote(): {:?}", vote);

        let value = serialize(vote).map_err(|err| io::Error::from(error::Error::from(err)))?;
        let (ack, rx) = oneshot::channel();
        self.send_to_writer(writer::Action::Vote { value, ack })
            .await?;
        self.writer_response(rx).await
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn append<I>(&mut self, entries: I, callback: IOFlushed<T>) -> Result<(), io::Error>
    where
        I: IntoIterator<Item = EntryOf<T>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        debug!("Entering append()");

        let (tx, rx) = flume::bounded(1);
        let (ack, ack_rx) = oneshot::channel();

        let callback = Box::new(move |res| callback.io_completed(res));
        self.send_to_writer(writer::Action::Append { rx, callback, ack })
            .await?;

        // The writer stops reading a batch as soon as it rejects it, which closes the entry
        // channel. The reason for that is in the ack, so a failed send must not hide it.
        let sent = response_or_stopped(&self.writer_closed, WRITER_STOPPED, async {
            for entry in entries {
                let data =
                    serialize(&entry).map_err(|err| io::Error::from(error::Error::from(err)))?;
                if tx
                    .send_async(Some((entry.log_id().index, data)))
                    .await
                    .is_err()
                {
                    return Ok(false);
                }
            }
            Ok(tx.send_async(None).await.is_ok())
        })
        .await?;

        self.writer_response(ack_rx).await?;
        if sent {
            Ok(())
        } else {
            Err(io::Error::other(
                "WAL writer acknowledged an append it did not fully receive",
            ))
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn truncate_after(&mut self, last_log_id: Option<LogIdOf<T>>) -> Result<(), io::Error> {
        debug!("truncate_after(): [{:?}, +oo)", last_log_id);

        // keep everything up to and including `last_log_id`, drop the rest; `None` means all
        let from = match last_log_id {
            Some(id) => id.index.saturating_add(1),
            None => 0,
        };

        let (ack, rx) = oneshot::channel();
        self.send_to_writer(writer::Action::Remove {
            from,
            until: u64::MAX,
            last_log: None,
            ack,
        })
        .await?;
        self.writer_response(rx).await
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn purge(&mut self, log_id: LogIdOf<T>) -> Result<(), io::Error> {
        debug!("purge(): [0, {:?}]", log_id);

        let last_log =
            Some(serialize(&log_id).map_err(|err| io::Error::from(error::Error::from(err)))?);
        let (ack, rx) = oneshot::channel();
        self.send_to_writer(writer::Action::Remove {
            from: 0,
            // openraft's `purge` is inclusive, while `shift_delete_logs` keeps the id it is
            // given as the new front - so the new front is the entry after the purge point.
            until: log_id.index + 1,
            last_log,
            ack,
        })
        .await?;
        self.writer_response(rx).await
    }
}

impl<T> LogStore<T>
where
    T: RaftTypeConfig,
{
    async fn send_to_writer(&self, action: writer::Action) -> io::Result<()> {
        self.writer
            .send_async(action)
            .await
            .map_err(|err| io::Error::from(error::Error::from(err)))
    }

    /// Awaits the writer's answer to an action, failing once the writer thread is gone.
    async fn writer_response(
        &self,
        ack: oneshot::Receiver<Result<(), error::Error>>,
    ) -> io::Result<()> {
        response_or_stopped(&self.writer_closed, WRITER_STOPPED, async {
            // the writer drops an ack unanswered only when it exits
            ack.await
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, WRITER_STOPPED))?
                .map_err(io::Error::from)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogStore;
    use crate::LogSync;
    use crate::error::Error;
    use openraft::Entry;
    use openraft::LogId;
    use openraft::entry::RaftEntry;
    use openraft::impls::leader_id_adv::LeaderId;
    use openraft::storage::IOFlushed;
    use openraft::storage::RaftLogStorage;
    use openraft::type_config::TypeConfigExt;
    use openraft::vote::RaftLeaderId;
    use std::fs;
    use std::future::Future;
    use std::time::Duration;
    use tokio::time::timeout;

    openraft::declare_raft_types!(
        pub TestConfig:
            D = String,
            R = u64,
    );

    fn log_id(index: u64) -> LogIdOf<TestConfig> {
        LogId::new(LeaderId::new(1, 0), index)
    }

    /// `tokio` has no `macros` feature in this workspace, so a test runtime is built by hand.
    fn block_on<F: Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }

    #[test]
    fn reader_shutdown_unblocks_a_queued_response() {
        block_on(async {
            let (done, closed) = flume::bounded::<()>(0);
            let (_queued_reply, reply) = oneshot::channel::<()>();
            drop(done);
            let err = response_or_stopped(&closed, READER_STOPPED, async {
                reply.await.map_err(io::Error::other)
            })
            .await
            .unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
        });
    }

    /// `read_committed` must never report a position past the durable log tail.
    ///
    /// openraft re-applies `(last_applied, committed]` from the log store at startup and fails
    /// hard when the range is not exactly there, so an unclamped pointer turns a restart into a
    /// startup failure. The WAL acknowledges appends before flushing them, which is how a crash
    /// leaves the tail behind the pointer in the first place.
    #[test]
    fn read_committed_is_clamped_to_the_log_tail() -> Result<(), Error> {
        block_on(async {
            let base = "test_data/read_committed_clamp".to_string();
            let _ = fs::remove_dir_all(&base);
            fs::create_dir_all(&base)?;

            let mut store =
                LogStore::<TestConfig>::start(base, LogSync::Immediate, 64 * 1024).await?;

            let entries = (1..=5).map(|i| Entry::new_normal(log_id(i), format!("payload-{i}")));
            store.append(entries, IOFlushed::noop()).await.unwrap();

            // a pointer above the tail, as a crash can leave it
            store.save_committed(Some(log_id(10))).await.unwrap();
            let committed = store.read_committed().await.unwrap();
            assert_eq!(committed.map(|id| id.index), Some(5));

            // a pointer inside the log is reported unchanged
            store.save_committed(Some(log_id(3))).await.unwrap();
            let committed = store.read_committed().await.unwrap();
            assert_eq!(committed.map(|id| id.index), Some(3));

            // a node that never committed anything stays `None`, so openraft re-applies nothing
            store.save_committed(None).await.unwrap();
            assert_eq!(store.read_committed().await.unwrap(), None);

            Ok(())
        })
    }

    /// The pointer is durable only as a passenger of the WAL flush, so a restarted store must
    /// come back with the value it published last - and `save_committed` must not fsync.
    #[test]
    fn committed_survives_a_store_restart() -> Result<(), Error> {
        block_on(async {
            let base = "test_data/read_committed_restart".to_string();
            let _ = fs::remove_dir_all(&base);
            fs::create_dir_all(&base)?;

            let mut store =
                LogStore::<TestConfig>::start(base.clone(), LogSync::Immediate, 64 * 1024).await?;

            let entries = (1..=5).map(|i| Entry::new_normal(log_id(i), format!("payload-{i}")));
            store.append(entries, IOFlushed::noop()).await.unwrap();
            store.save_committed(Some(log_id(4))).await.unwrap();
            assert!(store.get_log_state().await.unwrap().last_log_id.is_some());
            let mut reader = store.get_log_reader().await;
            assert_eq!(reader.try_get_log_entries(1..=5).await.unwrap().len(), 5);

            let shutdown = store.shutdown_handle();
            {
                let mut waiting = pin!(shutdown.shutdown());
                poll_fn(|cx| {
                    let _ = waiting.as_mut().poll(cx);
                    Poll::Ready(())
                })
                .await;
            }
            let concurrent = tokio::spawn(async move { shutdown.shutdown().await });
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                store.shutdown_handle().shutdown().await.unwrap();
                concurrent.await.unwrap().unwrap();
            })
            .await
            .unwrap();
            assert!(store.reader.is_disconnected());
            assert!(reader.tx.is_disconnected());

            let mut store =
                LogStore::<TestConfig>::start(base, LogSync::Immediate, 64 * 1024).await?;
            let committed = store.read_committed().await.unwrap();
            assert_eq!(committed.map(|id| id.index), Some(4));

            Ok(())
        })
    }

    const PROMPTLY: Duration = Duration::from_secs(10);

    async fn start_store(name: &str, wal_size: u32) -> Result<LogStore<TestConfig>, Error> {
        let base = format!("test_data/{name}");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base)?;
        LogStore::<TestConfig>::start(base, LogSync::Immediate, wal_size).await
    }

    fn entry_bytes(index: u64, payload: String) -> (u64, Vec<u8>) {
        let entry = EntryOf::<TestConfig>::new_normal(log_id(index), payload);
        (index, serialize(&entry).unwrap())
    }

    /// Two of these never fit into one 64 kB WAL file.
    fn big_payload() -> String {
        "x".repeat(40 * 1024)
    }

    /// Hands the writer an append and one of its entries, but not the end marker. The entry
    /// channel is a rendezvous, so once this returns the writer is busy with that batch.
    async fn open_batch(
        store: &LogStore<TestConfig>,
        first: (u64, Vec<u8>),
    ) -> (
        flume::Sender<Option<(u64, Vec<u8>)>>,
        oneshot::Receiver<Result<(), Error>>,
        std::sync::mpsc::Receiver<Result<(), io::Error>>,
    ) {
        let (entries, rx) = flume::bounded(0);
        let (ack, ack_rx) = oneshot::channel();
        let (cb_tx, cb_rx) = std::sync::mpsc::channel();
        let callback = Box::new(move |res| {
            let _ = cb_tx.send(res);
        });
        store
            .writer
            .send_async(writer::Action::Append { rx, callback, ack })
            .await
            .unwrap();
        entries.send_async(Some(first)).await.unwrap();
        (entries, ack_rx, cb_rx)
    }

    async fn wait_until_queued(writer: &flume::Sender<writer::Action>) {
        timeout(PROMPTLY, async {
            while writer.is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the action never reached the writer queue");
    }

    /// An append whose entry sender is dropped before the end marker is incomplete, and must
    /// never be reported as a success on the ack or the openraft callback.
    #[test]
    fn a_truncated_append_is_an_error_and_stops_the_writer() -> Result<(), Error> {
        block_on(async {
            let mut store = start_store("truncated_append", 64 * 1024).await?;
            let (entries, ack, callback) =
                open_batch(&store, entry_bytes(1, "payload-1".into())).await;
            drop(entries);

            let res = timeout(PROMPTLY, ack).await.unwrap().unwrap();
            assert!(
                res.is_err(),
                "a truncated append must not be acked: {res:?}"
            );
            let res = callback.recv_timeout(PROMPTLY).unwrap();
            assert!(
                res.is_err(),
                "a truncated append must not complete: {res:?}"
            );

            // the writer has stopped, and says so instead of panicking or hanging
            let entries = (2..=3).map(|i| Entry::new_normal(log_id(i), format!("payload-{i}")));
            let res = timeout(PROMPTLY, store.append(entries, IOFlushed::noop()))
                .await
                .unwrap();
            assert!(res.is_err(), "the writer must have stopped: {res:?}");
            Ok(())
        })
    }

    /// Work that is already queued when the writer stops is never read and never dropped, so it
    /// must fail on the writer having stopped instead of waiting forever.
    #[test]
    fn work_queued_behind_a_stopped_writer_fails_instead_of_hanging() -> Result<(), Error> {
        block_on(async {
            let mut store = start_store("queued_behind_stop_append", 64 * 1024).await?;
            let writer = store.writer.clone();
            let (entries, _ack, _callback) =
                open_batch(&store, entry_bytes(1, big_payload())).await;
            let queued = tokio::spawn(async move {
                let entries = (2..=4).map(|i| Entry::new_normal(log_id(i), format!("p-{i}")));
                let res = store.append(entries, IOFlushed::noop()).await;
                let vote = store.save_vote(&openraft::Vote::new(1, 0)).await;
                (res, vote)
            });
            wait_until_queued(&writer).await;
            // the next entry needs a new WAL file, which cannot be created
            fs::remove_dir_all("test_data/queued_behind_stop_append")?;
            let _ = entries
                .send_async(Some(entry_bytes(2, big_payload())))
                .await;

            let (res, vote) = timeout(PROMPTLY, queued)
                .await
                .expect("an append queued behind a stopped writer hangs")
                .unwrap();
            let err = res.unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::BrokenPipe, "{err}");
            assert!(vote.is_err(), "the writer must have stopped: {vote:?}");

            let store = start_store("queued_behind_stop_shutdown", 64 * 1024).await?;
            let writer = store.writer.clone();
            let (entries, _ack, _callback) =
                open_batch(&store, entry_bytes(1, big_payload())).await;
            let shutdown = store.shutdown_handle();
            let queued = tokio::spawn(async move { shutdown.shutdown().await });
            wait_until_queued(&writer).await;
            fs::remove_dir_all("test_data/queued_behind_stop_shutdown")?;
            let _ = entries
                .send_async(Some(entry_bytes(2, big_payload())))
                .await;

            let res = timeout(PROMPTLY, queued)
                .await
                .expect("a shutdown queued behind a stopped writer hangs")
                .unwrap();
            assert!(
                res.is_err(),
                "the writer stopped before the shutdown: {res:?}"
            );
            Ok(())
        })
    }

    /// A WAL file that cannot be created ends the writer, and the append that needed it is
    /// answered with the error on both channels instead of leaving its caller to panic.
    #[test]
    fn a_failed_roll_over_fails_the_append() -> Result<(), Error> {
        block_on(async {
            let mut store = start_store("failed_roll_over", 64 * 1024).await?;
            let big = |i: u64| Entry::new_normal(log_id(i), big_payload());
            store
                .append([big(1)], IOFlushed::noop())
                .await
                .expect("the first entry fits into the active WAL file");

            // the next entry needs a new WAL file, and its directory is gone
            fs::remove_dir_all("test_data/failed_roll_over")?;
            let (cb_tx, cb_rx) = TestConfig::oneshot();
            let res = timeout(PROMPTLY, store.append([big(2)], IOFlushed::signal(cb_tx)))
                .await
                .unwrap();
            assert!(
                res.is_err(),
                "a failed roll over must fail the append: {res:?}"
            );
            let res = timeout(PROMPTLY, cb_rx).await.unwrap().unwrap();
            assert!(
                res.is_err(),
                "a failed roll over must not complete: {res:?}"
            );

            let res = timeout(PROMPTLY, store.append([big(3)], IOFlushed::noop()))
                .await
                .unwrap();
            assert!(res.is_err(), "the writer must have stopped: {res:?}");
            Ok(())
        })
    }

    /// The writer closes the entry channel when it rejects a batch, which fails the next send.
    /// The caller must still see the reason from the ack, not the closed channel.
    #[cfg(feature = "oversized-entry-error")]
    #[test]
    fn a_rejected_append_reports_the_writer_error() -> Result<(), Error> {
        block_on(async {
            let mut store = start_store("rejected_append", 64 * 1024).await?;
            let entries = (1..=3).map(|i| Entry::new_normal(log_id(i), "x".repeat(70 * 1024)));
            let err = timeout(PROMPTLY, store.append(entries, IOFlushed::noop()))
                .await
                .unwrap()
                .unwrap_err();
            assert!(err.to_string().contains("WalSizeExceeded"), "{err}");
            Ok(())
        })
    }
}
