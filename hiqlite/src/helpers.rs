use crate::app_state::{AppState, RaftType};
use crate::{Error, Node};
use openraft::RaftMetrics;
use std::collections::BTreeSet;
use std::sync::Arc;

pub async fn is_raft_initialized(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> Result<bool, Error> {
    if raft_type == &RaftType::Sqlite {
        #[cfg(feature = "sqlite")]
        return Ok(state.raft_db.raft.is_initialized().await?);
    } else if raft_type == &RaftType::Cache {
        #[cfg(feature = "cache")]
        return Ok(state.raft_cache.raft.is_initialized().await?);
    }
    Err(Error::Error("invalid raft type".into()))
}

pub async fn get_raft_metrics(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> Result<RaftMetrics<u64, Node>, Error> {
    if raft_type == &RaftType::Sqlite {
        #[cfg(feature = "sqlite")]
        return Ok(state.raft_db.raft.metrics().borrow().clone());
    } else if raft_type == &RaftType::Cache {
        #[cfg(feature = "cache")]
        return Ok(state.raft_cache.raft.metrics().borrow().clone());
    }
    Err(Error::Error("invalid raft type".into()))
}

pub async fn add_new_learner(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    node: Node,
) -> Result<(), Error> {
    if raft_type == &RaftType::Sqlite {
        #[cfg(feature = "sqlite")]
        state.raft_db.raft.add_learner(node.id, node, true).await?;
        return Ok(());
    } else if raft_type == &RaftType::Cache {
        #[cfg(feature = "cache")]
        state
            .raft_cache
            .raft
            .add_learner(node.id, node, true)
            .await?;
        return Ok(());
    }
    Err(Error::Error("invalid raft type".into()))
}

pub async fn change_membership(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    members: BTreeSet<u64>,
    retain: bool,
) -> Result<(), Error> {
    if raft_type == &RaftType::Sqlite {
        #[cfg(feature = "sqlite")]
        state
            .raft_db
            .raft
            .change_membership(members, retain)
            .await?;
        return Ok(());
    } else if raft_type == &RaftType::Cache {
        #[cfg(feature = "cache")]
        state
            .raft_cache
            .raft
            .change_membership(members, retain)
            .await?;
        return Ok(());
    }
    Err(Error::Error("invalid raft type".into()))
}
