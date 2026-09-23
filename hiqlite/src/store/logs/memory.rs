use crate::NodeId;
use crate::store::StorageResult;
use crate::store::state_machine::memory::TypeConfigKV;
use openraft::OptionalSend;
use openraft::RaftLogReader;
use openraft::StorageError;
use openraft::StorageIOError;
use openraft::Vote;
use openraft::storage::LogFlushed;
use openraft::storage::LogState;
use openraft::storage::RaftLogStorage;
use openraft::{CommittedLeaderId, Entry};
use openraft::{LeaderId, LogId};
use std::collections::{BTreeMap, Bound, VecDeque};
use std::fmt::Debug;
use std::ops::{Deref, RangeBounds};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, RwLock, oneshot};
use tokio::time::Instant;
use tokio::{fs, task};
use tracing::info;

type Logs = Arc<RwLock<VecDeque<Entry<TypeConfigKV>>>>;

#[derive(Debug, Clone)]
struct LogData {
    last_purged: Option<LogId<u64>>,
    vote: Option<Vote<NodeId>>,
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

impl RaftLogReader<TypeConfigKV> for LogStoreMemory {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> StorageResult<Vec<Entry<TypeConfigKV>>> {
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
            Bound::Unbounded => panic!("open end log entries get"),
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

        debug_assert!(end > 0);
        let first_log_id = logs
            .front()
            .expect("logs non-empty (checked above)")
            .log_id
            .index;
        debug_assert!(start >= first_log_id);

        let range_start = (start - first_log_id) as usize;
        let range_end = (end - first_log_id) as usize;
        debug_assert!(if !logs.is_empty() {
            logs.get(range_start).unwrap().log_id.index == start
        } else {
            range_start == 0
        });
        debug_assert!(if !logs.is_empty() {
            logs.get(range_end).unwrap().log_id.index == end
        } else {
            range_end == 0
        });

        let mut res = Vec::with_capacity((end - start) as usize);
        for entry in logs.range(range_start..=range_end) {
            res.push((*entry).clone());
        }

        debug_assert!(if !res.is_empty() {
            res.first().unwrap().log_id.index == start && res.last().unwrap().log_id.index == end
        } else {
            start == end
        });

        Ok(res)
    }
}

impl RaftLogStorage<TypeConfigKV> for LogStoreMemory {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> StorageResult<LogState<TypeConfigKV>> {
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

    // async fn save_committed(
    //     &mut self,
    //     committed: Option<LogId<NodeId>>,
    // ) -> Result<(), StorageError<NodeId>> {
    //     let mut lock = self.data.lock().await;
    //     lock.commited = committed;
    //     Ok(())
    // }
    //
    // async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
    //     Ok(self.data.lock().await.commited)
    // }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        let mut lock = self.data.lock().await;
        lock.vote = Some(*vote);
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        Ok(self.data.lock().await.vote)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<TypeConfigKV>,
    ) -> StorageResult<()>
    where
        I: IntoIterator<Item = Entry<TypeConfigKV>> + Send,
        I::IntoIter: Send,
    {
        {
            let mut logs = self.logs.write().await;
            for entry in entries {
                logs.push_back(entry);
            }
        }

        callback.log_io_completed(Ok(()));

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn truncate(&mut self, log_id: LogId<NodeId>) -> StorageResult<()> {
        let mut logs = self.logs.write().await;

        if logs.is_empty() {
            info!("Logs are empty - nothing to truncate");
            return Ok(());
        }

        let first_offset = logs.front().unwrap().log_id.index;
        debug_assert!(log_id.index >= first_offset);
        let truncate_from = (log_id.index - first_offset) as usize;
        // `truncate(log_id)` removes entries from `log_id` inclusive onward, so the cut point is
        // exactly the entry named by `log_id`. DeleteConflictLog always names a present entry.
        debug_assert_eq!(
            logs.get(truncate_from).map(|e| e.log_id.index),
            Some(log_id.index)
        );

        logs.truncate(truncate_from);

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let new_last_purged = {
            let mut logs = self.logs.write().await;

            if logs.is_empty() {
                info!("Logs are empty - nothing to purge");
                // The caller is telling us everything up to `log_id` (inclusive) is now
                // covered by a snapshot. Record it so get_log_state() can still report
                // the purged point even though no entries remain.
                Some(log_id)
            } else {
                let first_offset = logs.front().unwrap().log_id.index;
                debug_assert!(
                    first_offset <= log_id.index,
                    "first_offset <= log_id.index -> {} >= {}",
                    first_offset,
                    log_id.index
                );
                let purge_until = (log_id.index - first_offset) as usize;
                debug_assert!(
                    logs.len() >= purge_until,
                    "lock.len() >= purge_until -> {} >= {}",
                    logs.len(),
                    purge_until
                );

                logs.drain(..purge_until);
                // purge is inclusive
                logs.pop_front().map(|e| e.log_id)
            }
        };

        let mut data = self.data.lock().await;
        // Never regress the recorded purged point.
        data.last_purged = new_last_purged.or(data.last_purged);
        drop(data);

        Ok(())
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }
}
