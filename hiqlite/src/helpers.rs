use crate::app_state::{AppState, RaftType};
use crate::{helpers, Error, Node};
use bincode::error::{DecodeError, EncodeError};
use openraft::RaftMetrics;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::Arc;
use tracing::{info, warn};

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

pub async fn is_raft_initialized(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> Result<bool, Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            if !state.raft_db.raft.is_initialized().await? {
                // We might have an initialized Raft from old ticks, but still be in an
                // un-initialized state with no members after a volume loss
                let members = get_raft_metrics(state, raft_type).await.membership_config;
                let count = members.membership().nodes().count();
                if count > 0 {
                    warn!(
                        "Raft is already initialized. Node Count: {}\n{:?}",
                        count,
                        members.membership()
                    );
                    Ok(true)
                } else {
                    warn!("Raft is initialized but the membership config is empty\n{:?}\nnodes count {count}", members.membership());
                    Ok(false)
                }
                // Ok(false)
            } else {
                /*
                We can get in a tricky situation here.
                In most cases, the `.is_initialized()` gives just the information we want.
                But, if *this* node lost its volume and therefore membership state, and another
                leader is still running and trying to reach *this* node before it can fully start
                up (race condition), the raft will report being initialized via this check, while
                it actually is not, because it lost all its state.
                If we get into this situation, we will have a committed leader vote, but no other
                data like logs and membership config.
                 */

                let metrics = state.raft_db.raft.server_metrics().borrow().clone();

                #[cfg(debug_assertions)]
                if metrics.current_leader.is_none()
                    && metrics.vote.leader_id().node_id == state.id
                    && metrics.vote.committed
                {
                    panic!(
                        "current_leader.is_none() && metrics.vote.leader_id().node_id == \
                        state.id && metrics.vote.committed:\n{:?}",
                        metrics
                    )
                }

                if metrics.vote.committed
                    && metrics.vote.leader_id.node_id != state.id
                    && metrics.current_leader.is_none()
                {
                    // If we get here, we have a race condition and a remote leader initialized this
                    // node after a data volume loss before it had a change to re-join and sync data.
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            if !state.raft_cache.raft.is_initialized().await? {
                // We might have an initialized Raft from old ticks, but still be in an
                // un-initialized state with no members after a volume loss
                let members = get_raft_metrics(state, raft_type).await.membership_config;
                let count = members.membership().nodes().count();
                if count > 0 {
                    warn!(
                        "Raft is already initialized. Node Count: {}\n{:?}",
                        count,
                        members.membership()
                    );
                    Ok(true)
                } else {
                    warn!("Raft is initialized but the membership config is empty\n{:?}\nnodes count {count}", members.membership());
                    Ok(false)
                }
                // Ok(false)
            } else {
                /*
                We can get in a tricky situation here.
                In most cases, the `.is_initialized()` gives just the information we want.
                But, if *this* node lost its volume and therefore membership state, and another
                leader is still running and trying to reach *this* node before it can fully start
                up (race condition), the raft will report being initialized via this check, while
                it actually is not, because it lost all its state.
                If we get into this situation, we will have a committed leader vote, but no other
                data like logs and membership config.
                 */

                let metrics = state.raft_cache.raft.server_metrics().borrow().clone();

                #[cfg(debug_assertions)]
                if metrics.current_leader.is_none()
                    && metrics.vote.leader_id().node_id == state.id
                    && metrics.vote.committed
                {
                    panic!(
                        "current_leader.is_none() && metrics.vote.leader_id().node_id == \
                        state.id && metrics.vote.committed:\n{:?}",
                        metrics
                    )
                }

                if metrics.vote.committed
                    && metrics.vote.leader_id.node_id != state.id
                    && metrics.current_leader.is_none()
                {
                    // If we get here, we have a race condition and a remote leader initialized this
                    // node after a data volume loss before it had a change to re-join and sync data.
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
        }
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

pub async fn add_new_learner(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    node: Node,
) -> Result<(), Error> {
    info!("Adding Node as new {:?} Learner: {:?}", raft_type, node);
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
    info!("Changing {:?} Raft membership to: {:?}", raft_type, members);
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
pub async fn set_path_access(path: &str, mode: u32) -> Result<(), Error> {
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
