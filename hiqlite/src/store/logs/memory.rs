use crate::store::state_machine::memory::TypeConfigKV;
use crate::store::StorageResult;
use crate::NodeId;
use openraft::storage::LogFlushed;
use openraft::storage::LogState;
use openraft::storage::RaftLogStorage;
use openraft::OptionalSend;
use openraft::RaftLogReader;
use openraft::StorageError;
use openraft::StorageIOError;
use openraft::Vote;
use openraft::{CommittedLeaderId, Entry};
use openraft::{LeaderId, LogId};
use std::collections::{BTreeMap, Bound, VecDeque};
use std::fmt::Debug;
use std::ops::{Deref, RangeBounds};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex, RwLock};
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
            Bound::Excluded(i) => *i - 1,
            Bound::Unbounded => panic!("open end log entries get"),
        };
        if end < start {
            return Ok(Vec::default());
        }

        let logs = self.logs.read().await;

        debug_assert!(end > 0);
        let first_log_id = logs
            .front()
            .expect("to have at least 1 entry in logs as long as end > 0")
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
        let lock = self.data.lock().await;
        let last_purged_log_id = lock.last_purged;

        let last_log_id = {
            let logs = self.logs.read().await;
            logs.get(logs.len()).map(|entry| entry.log_id)
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
        debug_assert!(truncate_from == logs.get(truncate_from).unwrap().log_id.index as usize);

        logs.truncate(truncate_from);

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let mut logs = self.logs.write().await;

        if logs.is_empty() {
            info!("Logs are empty - nothing to purge");
            return Ok(());
        }

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

        Ok(())
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }
}
