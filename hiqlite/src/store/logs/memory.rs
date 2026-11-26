use crate::store::StorageResult;
use crate::store::state_machine::memory::TypeConfigCache;
use openraft::OptionalSend;
use openraft::storage::{IOFlushed, LogState, RaftLogReader, RaftLogStorage};
use openraft::type_config::alias::{EntryOf, LogIdOf, VoteOf};
use std::collections::{Bound, VecDeque};
use std::fmt::Debug;
use std::io;
use std::ops::RangeBounds;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;

type Logs = Arc<RwLock<VecDeque<EntryOf<TypeConfigCache>>>>;

#[derive(Debug, Clone)]
struct LogData {
    last_purged: Option<LogIdOf<TypeConfigCache>>,
    vote: Option<VoteOf<TypeConfigCache>>,
}

#[derive(Debug, Clone)]
pub struct LogStoreMemory {
    logs: Logs,
    data: Arc<Mutex<LogData>>,
}

impl LogStoreMemory {
    pub fn new() -> Self {
        // TODO we could initialize with the correct amount of when to take snapshots and purge logs
        let logs = Arc::new(RwLock::new(VecDeque::with_capacity(1000)));
        let data = LogData {
            last_purged: None,
            vote: None,
        };

        Self {
            logs,
            data: Arc::new(Mutex::new(data)),
        }
    }
}

impl RaftLogReader<TypeConfigCache> for LogStoreMemory {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> StorageResult<Vec<EntryOf<TypeConfigCache>>> {
        let start = match range.start_bound() {
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(i) => *i,
            Bound::Excluded(i) => {
                if *i == 0 {
                    return Ok(Vec::default());
                }
                *i - 1
            }
            Bound::Unbounded => u64::MAX,
        };
        if end < start {
            return Ok(Vec::default());
        }

        let logs = self.logs.read().await;

        // An empty store has no present entries, so every requested range yields nothing. This is
        // reachable after a snapshot install + purge (all entries are covered by the snapshot);
        // only new appends repopulate the deque. Returning early also keeps `front()` below safe.
        if logs.is_empty() {
            return Ok(Vec::default());
        }

        // openraft tolerates a range whose beginning or end the log does not contain - indexes
        // already covered by a purge, the slot before the first entry, entries not appended yet -
        // so the requested bounds get clamped to the entries that are actually present.
        let first_log_id = logs
            .front()
            .expect("logs non-empty (checked above)")
            .log_id
            .index;
        let last_log_id = logs
            .back()
            .expect("logs non-empty (checked above)")
            .log_id
            .index;
        let start = start.max(first_log_id);
        let end = end.min(last_log_id);
        if end < start {
            return Ok(Vec::default());
        }

        let range_start = (start - first_log_id) as usize;
        let range_end = (end - first_log_id) as usize;

        let mut res = Vec::with_capacity(range_end - range_start + 1);
        for entry in logs.range(range_start..=range_end) {
            res.push((*entry).clone());
        }

        Ok(res)
    }

    async fn read_vote(&mut self) -> StorageResult<Option<VoteOf<TypeConfigCache>>> {
        Ok(self.data.lock().await.vote)
    }
}

impl RaftLogStorage<TypeConfigCache> for LogStoreMemory {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> StorageResult<LogState<TypeConfigCache>> {
        let last_purged_log_id = self.data.lock().await.last_purged;

        // Per the RaftLogStorage contract, `last_log_id` is the last present entry, or
        // `last_purged_log_id` when there is no entry at all.
        let last_log_id = match self
            .logs
            .read()
            .await
            .iter()
            .last()
            .map(|entry| entry.log_id)
        {
            Some(id) => Some(id),
            None => last_purged_log_id,
        };

        Ok(LogState {
            last_purged_log_id,
            last_log_id,
        })
    }

    // `save_committed` / `read_committed` stay on openraft's no-op defaults on purpose. This store
    // is not durable: at startup it is empty, so there is nothing to replay and `None` is the
    // correct answer. The disk-backed cache uses the WAL log store, which persists the pointer and
    // gets the startup replay there.

    #[tracing::instrument(level = "trace", skip(self))]
    async fn save_vote(&mut self, vote: &VoteOf<TypeConfigCache>) -> Result<(), io::Error> {
        let mut lock = self.data.lock().await;
        lock.vote = Some(*vote);
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn append<I>(
        &mut self,
        entries: I,
        callback: IOFlushed<TypeConfigCache>,
    ) -> StorageResult<()>
    where
        I: IntoIterator<Item = EntryOf<TypeConfigCache>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        {
            let mut logs = self.logs.write().await;
            for entry in entries {
                logs.push_back(entry);
            }
        }

        callback.io_completed(Ok(()));

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn truncate_after(
        &mut self,
        last_log_id: Option<LogIdOf<TypeConfigCache>>,
    ) -> StorageResult<()> {
        // keep everything up to and including `last_log_id`, drop the rest; `None` means all
        let from = match last_log_id {
            Some(id) => id.index.saturating_add(1),
            None => 0,
        };

        let mut logs = self.logs.write().await;

        if logs.is_empty() {
            info!("Logs are empty - nothing to truncate");
            return Ok(());
        }

        debug_assert!(
            from == 0 || logs.front().unwrap().log_id.index <= from,
            "truncate_after must never cut below the first present entry"
        );

        // the deque is ordered by log index, so a plain retain keeps the wanted prefix
        logs.retain(|entry| entry.log_id.index < from);

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn purge(&mut self, log_id: LogIdOf<TypeConfigCache>) -> Result<(), io::Error> {
        {
            let mut logs = self.logs.write().await;

            // Purge is inclusive, and the point can sit outside what is still here: below the
            // first entry when a snapshot already covers it, or past the tail when everything
            // gets dropped. Only the entries that are actually present can be removed.
            let purge_count = logs
                .iter()
                .take_while(|entry| entry.log_id.index <= log_id.index)
                .count();
            logs.drain(..purge_count);
        }

        let mut data = self.data.lock().await;
        // Never regress the recorded purged point.
        data.last_purged = data.last_purged.max(Some(log_id));

        Ok(())
    }

    // openraft declares this trait method `async`, so the impl has to keep `async`; our reader
    // is a plain `Clone` of the log and has nothing to await.
    #[allow(clippy::unused_async_trait_impl)]
    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }
}
