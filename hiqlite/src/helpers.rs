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
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
    }
}

pub async fn get_raft_leader(state: &Arc<AppState>, raft_type: &RaftType) -> Option<u64> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.raft_db.raft.current_leader().await,
        #[cfg(feature = "cache")]
        RaftType::Cache => state.raft_cache.raft.current_leader().await,
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
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
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
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
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
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
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
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
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
    }
}

/// Restricts the access for the given path.
#[cfg(feature = "sqlite")]
#[inline]
pub async fn fn_access(path: &str, mode: u32) -> Result<(), Error> {
    #[cfg(target_family = "unix")]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&path, Permissions::from_mode(mode)).await?;
    }
    Ok(())
}

/// Reads a single line from stdin and returns it `trim`ed.
#[cfg(feature = "server")]
pub async fn read_line_stdin() -> Result<String, Error> {
    let line = tokio::task::spawn_blocking(|| {
        let mut buf = String::with_capacity(4);
        std::io::stdin().read_line(&mut buf)?;
        Ok::<String, Error>(buf.trim().to_string())
    })
    .await??;
    Ok(line)
}
