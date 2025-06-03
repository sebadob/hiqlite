use crate::app_state::{AppState, RaftType};
use crate::{Error, Node};
use bincode::error::{DecodeError, EncodeError};
use openraft::{ChangeMembers, RaftMetrics};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::info;

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
    let is_initialized = match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.raft_db.raft.is_initialized().await?,
        #[cfg(feature = "cache")]
        RaftType::Cache => state.raft_cache.raft.is_initialized().await?,
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
    };
    Ok(is_initialized)
}

#[inline]
pub fn is_raft_stopped(state: &Arc<AppState>, raft_type: &RaftType) -> bool {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.raft_db.is_raft_stopped.load(Ordering::Relaxed),
        #[cfg(feature = "cache")]
        RaftType::Cache => state.raft_cache.is_raft_stopped.load(Ordering::Relaxed),
        RaftType::Unknown => true,
    }
}

#[inline]
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

pub async fn remove_learner(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    node_id: u64,
) -> Result<(), Error> {
    info!("Removing Node {} from {:?} Learners", node_id, raft_type);
    let mut set = BTreeSet::new();
    set.insert(node_id);

    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            state
                .raft_db
                .raft
                .change_membership(ChangeMembers::RemoveNodes(set), false)
                .await?;
            Ok(())
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            state
                .raft_cache
                .raft
                .change_membership(ChangeMembers::RemoveNodes(set), false)
                .await?;
            Ok(())
        }
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
    }
}

// pub async fn remove_voter(
//     state: &Arc<AppState>,
//     raft_type: &RaftType,
//     new_members: BTreeMap<NodeId, Node>,
//     // node_id: u64,
//     retain: bool,
// ) -> Result<(), Error> {
//     info!(
//         "Removing Node from {:?} Voters, new members: {:?}",
//         raft_type, new_members
//     );
//     // info!("Removing Node {} from {:?} Voters", node_id, raft_type);
//     // let mut set = BTreeSet::new();
//     // set.insert(node_id);
//
//     match raft_type {
//         #[cfg(feature = "sqlite")]
//         RaftType::Sqlite => {
//             state
//                 .raft_db
//                 .raft
//                 .change_membership(ChangeMembers::SetNodes(new_members), retain)
//                 .await?;
//             Ok(())
//         }
//         #[cfg(feature = "cache")]
//         RaftType::Cache => {
//             state
//                 .raft_cache
//                 .raft
//                 // .change_membership(ChangeMembers::RemoveVoters(set), retain)
//                 .change_membership(ChangeMembers::SetNodes(new_members), retain)
//                 .await?;
//             Ok(())
//         }
//         RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
//     }
// }

pub async fn remove_voter(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    // new_members: BTreeMap<NodeId, Node>,
    node_id: u64,
    retain: bool,
) -> Result<(), Error> {
    // info!(
    //     "Removing Node from {:?} Voters, new members: {:?}",
    //     raft_type, new_members
    // );
    info!("Removing Node {} from {:?} Voters", node_id, raft_type);
    let mut set = BTreeSet::new();
    set.insert(node_id);

    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            state
                .raft_db
                .raft
                .change_membership(ChangeMembers::RemoveVoters(set), retain)
                // .change_membership(ChangeMembers::SetNodes(new_members), retain)
                .await?;
            Ok(())
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            state
                .raft_cache
                .raft
                .change_membership(ChangeMembers::RemoveVoters(set), retain)
                // .change_membership(ChangeMembers::SetNodes(new_members), retain)
                .await?;
            Ok(())
        }
        RaftType::Unknown => panic!("neither `sqlite` nor `cache` feature enabled"),
    }
}

/// Restricts the access for the given path.
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
