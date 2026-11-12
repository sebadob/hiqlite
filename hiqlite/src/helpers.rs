use crate::app_state::{AppState, RaftType};

use crate::network::WireMembership;
use crate::{Error, Node};
use bincode_next::error::{DecodeError, EncodeError};
use bincode_next::{Decode, Encode};
use openraft::ChangeMembers;
use openraft::ServerState;
use openraft::type_config::async_runtime::watch::WatchReceiver;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::fs;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use tokio::time::Instant;
use tracing::{error, info, warn};

/// Will fsync the `src`, then `rename` to `dest`, and then flush the parent dir of `dest` to
/// guarantee the rename is persistent.
pub async fn atomic_file_switch<S, D>(src: S, dest: D) -> Result<(), Error>
where
    S: Into<PathBuf>,
    D: Into<PathBuf>,
{
    let src = src.into();
    let dest = dest.into();

    fs::File::open(&src).await?.sync_data().await?;
    fs::rename(src, &dest).await?;
    if let Some(parent) = dest.parent() {
        fs::File::open(parent).await?.sync_data().await?;
    }

    Ok(())
}

#[inline(always)]
pub fn serialize<T: Encode>(value: &T) -> Result<Vec<u8>, EncodeError> {
    // We are using the legacy config on purpose here. It uses fixed-width integer fields, which
    // uses a bit more space, but is faster.
    //
    // Native derived encoding (not the serde adapter): byte-identical to the old
    // `bincode_next::serde` path for every type that flows through this choke point (same u32
    // variant tags, same integer widths) - pinned by the bincode_compat_* / raft_wire_goldens
    // tests in state_machine.rs. Callers keep their serde derives for the JSON API path and
    // cross-decode tooling.
    bincode_next::encode_to_vec(value, bincode_next::config::legacy())
}

#[inline(always)]
pub fn deserialize<T: Decode<()>>(bytes: &[u8]) -> Result<T, DecodeError> {
    bincode_next::decode_from_slice::<T, _>(bytes, bincode_next::config::legacy())
        .map(|(res, _)| res)
}

/// Serde-adapter serialization for the few call sites whose types we do not own (openraft
/// payload types like `SnapshotMeta` or `StateMachineData` fields). Byte-identical to what the
/// native path above produces for the same values; kept until those boundaries move to native
/// derives with `#[bincode(with_serde)]` fields.
#[inline(always)]
pub fn serialize_serde<T: Serialize>(value: &T) -> Result<Vec<u8>, EncodeError> {
    bincode_next::serde::encode_to_vec(value, bincode_next::config::legacy())
}

/// Counterpart of [serialize_serde] for deserialization.
#[inline(always)]
pub fn deserialize_serde<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    bincode_next::serde::decode_from_slice::<T, _>(bytes, bincode_next::config::legacy())
        .map(|(res, _)| res)
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
        RaftType::Unknown => return Err(Error::Error("Unknown Raft type".into())),
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
pub async fn get_raft_leader(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> Result<Option<u64>, Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => Ok(state.raft_db.raft.current_leader().await),
        #[cfg(feature = "cache")]
        RaftType::Cache => Ok(state.raft_cache.raft.current_leader().await),
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
    }
}

/// The parts of a group's current metrics that the shared code paths need. openraft's
/// `RaftMetrics` is generic over the type config, so it cannot be shared between the two groups,
/// but these fields resolve to the same types for both - so a view is a copy plus one `Arc` clone
/// instead of a serialization round trip. The shared paths that do take a whole `RaftMetrics` read
/// a remote response, and the guards in `network` are what keep that safe.
#[derive(Debug)]
pub(crate) struct RaftView {
    pub state: ServerState,
    pub leader_id: Option<u64>,
    pub membership: Arc<WireMembership>,
}

pub(crate) fn get_raft_view(
    state: &Arc<AppState>,
    raft_type: &RaftType,
) -> Result<RaftView, Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            let rx = state.raft_db.raft.metrics();
            let metrics = rx.borrow_watched();
            Ok(RaftView {
                state: metrics.state,
                leader_id: metrics.current_leader,
                membership: metrics.membership_config.clone(),
            })
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            let rx = state.raft_cache.raft.metrics();
            let metrics = rx.borrow_watched();
            Ok(RaftView {
                state: metrics.state,
                leader_id: metrics.current_leader,
                membership: metrics.membership_config.clone(),
            })
        }
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
    }
}

/// Hand leadership over to the most up-to-date voter before we shut down, so the cluster doesn't
/// have to wait for an election. `deadline` is the shared handover budget of a shutdown
/// ([`shutdown_transfer_deadline`]), not this group's own timeout.
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub(crate) async fn transfer_leadership_before_shutdown(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    deadline: Instant,
) {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => transfer_leadership(state, &state.raft_db.raft, deadline).await,
        #[cfg(feature = "cache")]
        RaftType::Cache => transfer_leadership(state, &state.raft_cache.raft, deadline).await,
        RaftType::Unknown => {}
    }
}

/// The point the last leadership handover of a shutdown stops waiting at. One handover waits at most
/// an election timeout - openraft gives up on a transfer after one, and that is the election we'd be
/// facing without any of this - but a full build has two groups, so a per-group timeout would let a
/// graceful shutdown stall for two of them. All handovers of one shutdown share one budget instead:
/// the first group still gets its whole timeout, the groups after it get what's left of the budget.
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub(crate) fn shutdown_transfer_deadline(state: &Arc<AppState>) -> Instant {
    let mut timeout_ms = 0_u64;

    #[cfg(feature = "sqlite")]
    {
        timeout_ms = timeout_ms.max(state.raft_db.raft.config().election_timeout_max);
    }
    #[cfg(feature = "cache")]
    {
        timeout_ms = timeout_ms.max(state.raft_cache.raft.config().election_timeout_max);
    }

    Instant::now() + Duration::from_millis(timeout_ms)
}

#[cfg(any(feature = "sqlite", feature = "cache"))]
async fn transfer_leadership<C, SM>(
    state: &Arc<AppState>,
    raft: &openraft::Raft<C, SM>,
    deadline: Instant,
) where
    C: openraft::RaftTypeConfig<NodeId = u64>,
    SM: openraft::storage::RaftStateMachine<C>,
{
    let mut rx = raft.metrics();
    let Some(target) = transfer_target(&rx.borrow_watched(), state.id) else {
        return;
    };

    info!("Handing leadership over to node {target}");
    if let Err(err) = raft.trigger().transfer_leader(target).await {
        error!("Error handing leadership over to node {target}: {err:?}");
        return;
    }

    // the handover lands when the target takes over, and the shared budget runs to the point where
    // openraft gives up on a transfer - the election we'd be facing without any of this
    let moved = loop {
        if rx.borrow_watched().current_leader != Some(state.id) {
            break true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break false;
        }
        match tokio::time::timeout(remaining, rx.changed()).await {
            Ok(Ok(())) => {}
            // the deadline passed, or the watch is gone - nothing left to wait for
            Ok(Err(_)) | Err(_) => break false,
        }
    };

    if moved {
        info!("Leadership moved to node {target}");
    } else {
        warn!("Leadership did not move to node {target} - shutting down without the handover");
    }
}

/// The node leadership should go to: the other voter with the greatest matching log, which needs
/// the least catching up - the rule openraft follows when a leader that is removed from the
/// membership steps down. `None` when we aren't the leader - only a leader can hand over - or when
/// there is no other voter to hand it to.
#[cfg(any(feature = "sqlite", feature = "cache"))]
fn transfer_target<C>(metrics: &openraft::RaftMetrics<C>, our_id: u64) -> Option<u64>
where
    C: openraft::RaftTypeConfig<NodeId = u64>,
{
    if metrics.current_leader != Some(our_id) {
        return None;
    }

    let replication = metrics.replication.as_ref();
    let matching = |node_id: &C::NodeId| {
        replication
            .and_then(|matched| matched.get(node_id))
            .cloned()
            .flatten()
    };
    metrics
        .membership_config
        .voter_ids()
        .filter(|node_id| *node_id != our_id)
        .max_by_key(matching)
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
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
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
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
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
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
    }
}

pub async fn remove_voter(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    node_id: u64,
    retain: bool,
) -> Result<(), Error> {
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
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
    }
}

/// Parses the given input into a type-safe `Duration`. The input can have the following suffixes:
/// - s -> seconds
/// - m -> minutes
/// - h -> hours
/// - d -> days
/// - w -> weeks
/// - y -> years
pub fn parse_duration<T: AsRef<str>>(input: T) -> Option<Duration> {
    let input = input.as_ref().trim();
    if input.is_empty() {
        return None;
    }
    let (value, unit) = input.split_at(input.len() - 1);

    let mul = match unit {
        "s" | "S" => 1,
        "m" | "M" => 60,
        "h" | "H" => 60 * 60,
        "d" | "D" => 60 * 60 * 24,
        "w" | "W" => 60 * 60 * 24 * 7,
        "y" | "Y" => 60 * 60 * 24 * 365,
        _ => {
            return if let Ok(i) = input.parse::<u64>() {
                Some(Duration::from_secs(i))
            } else {
                None
            };
        }
    };

    value
        .trim()
        // we only want positive values
        .parse::<u32>()
        .ok()
        // i64 casting here is necessary to guarantee safety when we downcast later on
        // for cache TTL and so on.
        .map(|v| Duration::from_secs((v as i64).saturating_mul(mul) as u64))
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

#[cfg(any(feature = "sqlite", feature = "backup"))]
pub async fn validate_db_backup_snapshot(path: String) -> Result<(), Error> {
    use crate::store::state_machine::sqlite::state_machine::StateMachineData;

    tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(path)?;

        // Metadata check: the backup must carry our state-machine metadata row.
        let mut stmt = conn.prepare_cached("SELECT data FROM _metadata WHERE key = 'meta'")?;
        let bytes = stmt.query_row((), |row| {
            let bytes: Vec<u8> = row.get(0)?;
            Ok(bytes)
        })?;
        deserialize_serde::<StateMachineData>(&bytes)?;

        // Full SQLite integrity check: a corrupt-but-openable DB must not pass
        // silently. `PRAGMA integrity_check` returns exactly one "ok" row when the
        // database is healthy, and one row per problem otherwise.
        let mut stmt = conn.prepare("PRAGMA integrity_check")?;
        let mut problems: Vec<String> = Vec::new();
        {
            let rows = stmt.query_map(rusqlite::params![], |row| row.get::<_, String>(0))?;
            for r in rows {
                problems.push(r?);
            }
        }
        if problems.len() != 1 || problems[0] != "ok" {
            return Err(Error::Sqlite(
                format!("Backup integrity check failed: {}", problems.join("; ")).into(),
            ));
        }

        Ok::<(), Error>(())
    })
    .await??;
    Ok(())
}

#[cfg(all(test, feature = "cache"))]
mod tests {
    use super::*;
    use crate::store::state_machine::memory::TypeConfigKV;
    use openraft::impls::leader_id_adv::LeaderId;
    use openraft::type_config::alias::{LogIdOf, MembershipOf, StoredMembershipOf};
    use std::collections::BTreeMap;

    fn metrics(
        our_id: u64,
        leader: Option<u64>,
        voters: &[u64],
        matched: &[(u64, u64)],
    ) -> openraft::RaftMetrics<TypeConfigKV> {
        let nodes = voters
            .iter()
            .map(|id| {
                (
                    *id,
                    Node {
                        id: *id,
                        addr_raft: format!("localhost:{}", 8100 + id),
                        addr_api: format!("localhost:{}", 8200 + id),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let membership = MembershipOf::<TypeConfigKV>::new(
            vec![BTreeSet::from_iter(voters.iter().copied())],
            nodes,
        )
        .unwrap();

        let mut metrics = openraft::RaftMetrics::new_initial(our_id);
        metrics.current_leader = leader;
        metrics.membership_config =
            Arc::new(StoredMembershipOf::<TypeConfigKV>::new(None, membership));
        metrics.replication = Some(
            matched
                .iter()
                .map(|(id, index)| (*id, Some(log_id(2, leader.unwrap_or(0), *index))))
                .collect::<BTreeMap<_, _>>(),
        );
        metrics
    }

    fn log_id(term: u64, leader: u64, index: u64) -> LogIdOf<TypeConfigKV> {
        LogIdOf::<TypeConfigKV> {
            leader_id: LeaderId {
                term,
                node_id: leader,
            },
            index,
        }
    }

    #[test]
    fn transfer_target_takes_the_most_up_to_date_voter() {
        let metrics = metrics(1, Some(1), &[1, 2, 3], &[(2, 4), (3, 9)]);
        assert_eq!(transfer_target(&metrics, 1), Some(3));
    }

    #[test]
    fn transfer_target_never_picks_ourself() {
        let metrics = metrics(1, Some(1), &[1, 2], &[(1, 9), (2, 1)]);
        assert_eq!(transfer_target(&metrics, 1), Some(2));
    }

    #[test]
    fn transfer_target_needs_us_to_be_the_leader() {
        let metrics = metrics(1, Some(2), &[1, 2], &[(2, 9)]);
        assert_eq!(transfer_target(&metrics, 1), None);
    }

    #[test]
    fn transfer_target_without_another_voter_is_none() {
        let metrics = metrics(1, Some(1), &[1], &[(1, 9)]);
        assert_eq!(transfer_target(&metrics, 1), None);
    }
}
