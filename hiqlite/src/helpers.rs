use crate::app_state::{AppState, RaftType};
use crate::{Error, Node};
use openraft::RaftMetrics;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::MutexGuard;

pub async fn is_raft_initialized(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> Result<bool, Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => Ok(state.raft_db.raft.is_initialized().await?),
        #[cfg(feature = "cache")]
        RaftType::Cache => Ok(state.raft_cache.raft.is_initialized().await?),
    }
}

pub async fn get_raft_leader(state: &Arc<AppState>, raft_type: &RaftType) -> Option<u64> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.raft_db.raft.current_leader().await,
        #[cfg(feature = "cache")]
        RaftType::Cache => state.raft_cache.raft.current_leader().await,
    }
}

pub async fn get_raft_metrics(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> RaftMetrics<u64, Node> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.raft_db.raft.metrics().borrow().clone(),
        #[cfg(feature = "cache")]
        RaftType::Cache => state.raft_cache.raft.metrics().borrow().clone(),
    }
}

pub async fn lock_raft<'a>(
    state: &'a Arc<AppState>,
    raft_type: &'a RaftType,
) -> MutexGuard<'a, ()> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.raft_db.lock.lock().await,
        #[cfg(feature = "cache")]
        RaftType::Cache => state.raft_cache.lock.lock().await,
    }
}

pub async fn add_new_learner(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    node: Node,
) -> Result<(), Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            state.raft_db.raft.add_learner(node.id, node, true).await?;
            Ok(())
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            state
                .raft_cache
                .raft
                .add_learner(node.id, node, true)
                .await?;
            Ok(())
        }
    }
}

pub async fn change_membership(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    members: BTreeSet<u64>,
    retain: bool,
) -> Result<(), Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            state
                .raft_db
                .raft
                .change_membership(members, retain)
                .await?;
            Ok(())
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            state
                .raft_cache
                .raft
                .change_membership(members, retain)
                .await?;
            Ok(())
        }
    }
}
