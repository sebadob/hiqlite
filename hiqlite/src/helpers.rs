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
#[cfg(any(feature = "sqlite", feature = "cache"))]
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::fs;
#[cfg(any(feature = "sqlite", feature = "cache"))]
use tokio::time::Instant;
use tracing::{error, info, warn};

pub(crate) async fn sync_file_data(path: impl AsRef<Path>) -> Result<(), Error> {
    // Windows requires a writable handle to flush a file.
    fs::OpenOptions::new()
        .read(true)
        .write(cfg!(windows))
        .open(path)
        .await?
        .sync_data()
        .await?;
    Ok(())
}

/// Flushes `src` before replacing `dest`. On Unix it also flushes `dest`'s parent directory.
pub async fn atomic_file_switch<S, D>(src: S, dest: D) -> Result<(), Error>
where
    S: Into<PathBuf>,
    D: Into<PathBuf>,
{
    let src = src.into();
    let dest = dest.into();

    sync_file_data(&src).await?;
    fs::rename(src, &dest).await?;
    #[cfg(unix)]
    if let Some(parent) = dest.parent() {
        fs::File::open(parent).await?.sync_data().await?;
    }

    Ok(())
}

#[inline]
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

#[inline]
pub fn deserialize<T: Decode<()>>(bytes: &[u8]) -> Result<T, DecodeError> {
    bincode_next::decode_from_slice::<T, _>(bytes, bincode_next::config::legacy())
        .map(|(res, _)| res)
}

/// Serde-adapter serialization for the few call sites whose types we do not own (openraft
/// payload types like `SnapshotMeta` or `StateMachineData` fields). Byte-identical to what the
/// native path above produces for the same values; kept until those boundaries move to native
/// derives with `#[bincode(with_serde)]` fields.
/// # Panics
/// If `value` cannot be serialized with bincode-next. Every type flowing through here is a
/// plain payload of our own protocol, so a failure is always a bug in the type, not a condition
/// a caller can recover from.
#[inline]
pub fn serialize_serde<T: Serialize>(value: &T) -> Vec<u8> {
    bincode_next::serde::encode_to_vec(value, bincode_next::config::legacy())
        .expect("Network payload serialization should always succeed")
}

/// Counterpart of [`serialize_serde`] for deserialization.
#[inline]
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

/// Whether this node may start an election on its own for the given group.
///
/// The value mirrors openraft's runtime `enable_elect`: a node with it disabled never nominates
/// itself, it still grants votes to others, and it still accepts a leadership transfer.
#[inline]
pub fn is_elect_enabled(state: &Arc<AppState>, raft_type: &RaftType) -> bool {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state.raft_db.enable_elect.load(Ordering::Relaxed),
        #[cfg(feature = "cache")]
        RaftType::Cache => state.raft_cache.enable_elect.load(Ordering::Relaxed),
        RaftType::Unknown => false,
    }
}

/// Enable or disable this node's own right to start elections for the given group.
///
/// openraft applies it to its in-memory runtime config, and we mirror it, because openraft has no
/// getter. The change is not persisted: a restart applies the `election_disable` marker of this
/// node's node line again.
pub fn set_enable_elect(state: &Arc<AppState>, raft_type: &RaftType, enable: bool) {
    let (previous, group) = match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => {
            let previous = state.raft_db.enable_elect.swap(enable, Ordering::Relaxed);
            state.raft_db.raft.runtime_config().elect(enable);
            (previous, "sqlite")
        }
        #[cfg(feature = "cache")]
        RaftType::Cache => {
            let previous = state
                .raft_cache
                .enable_elect
                .swap(enable, Ordering::Relaxed);
            state.raft_cache.raft.runtime_config().elect(enable);
            (previous, "cache")
        }
        RaftType::Unknown => (enable, "unknown"),
    };

    if previous != enable {
        warn!(
            "Node {} sets `enable_elect` of the {group} group from {previous} to {enable} - a \
             restart reverts this to the marker of its node line",
            state.id
        );
    }
}

/// Make this node campaign for the leadership of the given group.
///
/// openraft always runs its Pre-Vote round first, so the term is untouched when this node cannot
/// win, and a node with `enable_elect` off is refused by its own gate. There is no way to bypass
/// either: a leader change that must happen is a leadership transfer, asked for by the node that
/// holds the seat, and a campaign is only ever a request a healthy cluster can refuse.
pub async fn trigger_elect(state: &Arc<AppState>, raft_type: &RaftType) -> Result<(), Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => state
            .raft_db
            .raft
            .trigger()
            // `true` is openraft's Pre-Vote round: probe before the term moves.
            .elect(true)
            .await
            .map_err(|err| Error::RaftErrorFatalDb(Box::new(err)))?,
        #[cfg(feature = "cache")]
        RaftType::Cache => state
            .raft_cache
            .raft
            .trigger()
            .elect(true)
            .await
            .map_err(|err| Error::RaftErrorFatalCache(Box::new(err)))?,
        RaftType::Unknown => {}
    }

    Ok(())
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
    let candidates = transfer_targets(&rx.borrow_watched(), state.id, &state.election_disable);
    let count = candidates.len();

    // openraft gives no answer about whether a transfer lands: a target that is unreachable, or
    // whose log is behind, simply ignores it. Every candidate therefore gets an equal slice of the
    // remaining budget, and the next one is tried when the current one did not take over.
    for (idx, target) in candidates.into_iter().enumerate() {
        let Some(remaining) = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
        else {
            warn!("No handover budget left - shutting down without the handover");
            return;
        };
        let slice_deadline = Instant::now() + remaining / (count - idx) as u32;

        info!("Handing leadership over to node {target}");
        if let Err(err) = raft.trigger().transfer_leader(target).await {
            error!("Error handing leadership over to node {target}: {err:?}");
            continue;
        }

        // the handover lands when the target takes over; the slice ends where openraft gives up on
        // a transfer - the election we'd be facing without any of this
        let moved = loop {
            if rx.borrow_watched().current_leader != Some(state.id) {
                break true;
            }
            let remaining = slice_deadline.saturating_duration_since(Instant::now());
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
            return;
        }
        warn!("Leadership did not move to node {target}");
    }

    if count > 0 {
        warn!("Leadership was not handed over - shutting down without the handover");
    }
}

/// The nodes leadership should go to, best candidate first: the voters with the greatest matching
/// log, which need the least catching up - the rule openraft follows when a leader that is removed
/// from the membership steps down - and the nodes that are configured with `election_disable` at
/// the very end. Those never start an election on their own, so they are only a fallback: a
/// leadership transfer does reach them, while a self-elected takeover never will. Empty when we
/// aren't the leader - only a leader can hand over - and never containing ourselves.
#[cfg(any(feature = "sqlite", feature = "cache"))]
fn transfer_targets<C>(
    metrics: &openraft::RaftMetrics<C>,
    our_id: u64,
    election_disable: &[u64],
) -> Vec<u64>
where
    C: openraft::RaftTypeConfig<NodeId = u64>,
{
    if metrics.current_leader != Some(our_id) {
        return Vec::new();
    }

    let replication = metrics.replication.as_ref();
    let matching = |node_id: &C::NodeId| {
        replication
            .and_then(|matched| matched.get(node_id))
            .cloned()
            .flatten()
    };

    let mut candidates = metrics
        .membership_config
        .voter_ids()
        .filter(|node_id| *node_id != our_id)
        .collect::<Vec<_>>();

    candidates.sort_by_key(|node_id| {
        (
            election_disable.contains(node_id),
            Reverse(matching(node_id)),
            Reverse(*node_id),
        )
    });
    candidates
}

/// Hand leadership over to one specific node, and answer whether it landed.
///
/// The runtime counterpart of the shutdown handover, with a single target: openraft treats a
/// transfer as a request, so a target that is unreachable, or whose log is behind, simply ignores
/// it and this returns `Ok(false)` when `deadline` passes. A learner never takes over, so callers
/// must refuse it as a target.
#[cfg(any(feature = "sqlite", feature = "cache"))]
pub(crate) async fn transfer_leadership_to(
    state: &Arc<AppState>,
    raft_type: &RaftType,
    target: u64,
    deadline: Instant,
) -> Result<bool, Error> {
    match raft_type {
        #[cfg(feature = "sqlite")]
        RaftType::Sqlite => transfer_to(state, &state.raft_db.raft, target, deadline)
            .await
            .map_err(|err| Error::RaftErrorFatalDb(Box::new(err))),
        #[cfg(feature = "cache")]
        RaftType::Cache => transfer_to(state, &state.raft_cache.raft, target, deadline)
            .await
            .map_err(|err| Error::RaftErrorFatalCache(Box::new(err))),
        RaftType::Unknown => Err(Error::Error("Unknown Raft type".into())),
    }
}

#[cfg(any(feature = "sqlite", feature = "cache"))]
async fn transfer_to<C, SM>(
    state: &Arc<AppState>,
    raft: &openraft::Raft<C, SM>,
    target: u64,
    deadline: Instant,
) -> Result<bool, openraft::errors::Fatal<C>>
where
    C: openraft::RaftTypeConfig<NodeId = u64>,
    SM: openraft::storage::RaftStateMachine<C>,
{
    let mut rx = raft.metrics();

    info!(
        "Node {} asks node {target} to take the leadership",
        state.id
    );
    raft.trigger().transfer_leader(target).await?;

    loop {
        if rx.borrow_watched().current_leader == Some(target) {
            info!("Leadership moved to node {target}");
            return Ok(true);
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }

        match tokio::time::timeout(remaining, rx.changed()).await {
            Ok(Ok(())) => {}
            // the deadline passed, or the watch is gone - nothing left to wait for
            Ok(Err(_)) | Err(_) => return Ok(false),
        }
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
/// - ms -> milliseconds
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
    // `ms` is the only two-letter unit, so it is read before the single-letter split: `100ms`
    // would otherwise read as the value `100m` on the unit `s`.
    let bytes = input.as_bytes();
    if bytes.len() > 2 && bytes[bytes.len() - 2..].eq_ignore_ascii_case(b"ms") {
        return input[..bytes.len() - 2]
            .trim()
            .parse::<u64>()
            .ok()
            .map(Duration::from_millis);
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
        // for cache TTL and so on: the saturating product can never exceed an `i64`.
        .map(|v| Duration::from_secs((v as i64).saturating_mul(mul).cast_unsigned()))
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
    tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(path)?;
        validate_db_conn(&conn).map(|_| ())
    })
    .await??;
    Ok(())
}

/// The checks of `validate_db_backup_snapshot` on a database that is already open: it must carry
/// our state-machine metadata row, and SQLite's `integrity_check` must find nothing wrong with it.
/// Returns the decoded metadata.
#[cfg(any(feature = "sqlite", feature = "backup"))]
pub fn validate_db_conn(
    conn: &rusqlite::Connection,
) -> Result<crate::store::state_machine::sqlite::state_machine::StateMachineData, Error> {
    // Metadata check: the file must carry our state-machine metadata row.
    let mut stmt = conn.prepare_cached("SELECT data FROM _metadata WHERE key = 'meta'")?;
    let bytes = stmt.query_row((), |row| {
        let bytes: Vec<u8> = row.get(0)?;
        Ok(bytes)
    })?;
    let metadata = deserialize_serde(&bytes)?;

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
            format!("SQLite integrity check failed: {}", problems.join("; ")).into(),
        ));
    }

    Ok(metadata)
}

#[cfg(all(test, feature = "cache"))]
mod tests {
    use super::*;
    use crate::store::state_machine::memory::TypeConfigCache;
    use openraft::impls::leader_id_adv::LeaderId;
    use openraft::type_config::alias::{LogIdOf, MembershipOf, StoredMembershipOf};
    use std::collections::BTreeMap;

    fn metrics(
        our_id: u64,
        leader: Option<u64>,
        voters: &[u64],
        matched: &[(u64, u64)],
    ) -> openraft::RaftMetrics<TypeConfigCache> {
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

        let membership = MembershipOf::<TypeConfigCache>::new(
            vec![BTreeSet::from_iter(voters.iter().copied())],
            nodes,
        )
        .unwrap();

        let mut metrics = openraft::RaftMetrics::new_initial(our_id);
        metrics.current_leader = leader;
        metrics.membership_config =
            Arc::new(StoredMembershipOf::<TypeConfigCache>::new(None, membership));
        metrics.replication = Some(
            matched
                .iter()
                .map(|(id, index)| (*id, Some(log_id(2, leader.unwrap_or(0), *index))))
                .collect::<BTreeMap<_, _>>(),
        );
        metrics
    }

    fn log_id(term: u64, leader: u64, index: u64) -> LogIdOf<TypeConfigCache> {
        LogIdOf::<TypeConfigCache> {
            leader_id: LeaderId {
                term,
                node_id: leader,
            },
            index,
        }
    }

    #[test]
    fn transfer_targets_prefer_the_most_up_to_date_voter() {
        let metrics = metrics(1, Some(1), &[1, 2, 3], &[(2, 4), (3, 9)]);
        assert_eq!(transfer_targets(&metrics, 1, &[]), vec![3, 2]);
    }

    #[test]
    fn transfer_targets_never_pick_ourself() {
        let metrics = metrics(1, Some(1), &[1, 2], &[(1, 9), (2, 1)]);
        assert_eq!(transfer_targets(&metrics, 1, &[]), vec![2]);
    }

    #[test]
    fn transfer_targets_need_us_to_be_the_leader() {
        let metrics = metrics(1, Some(2), &[1, 2], &[(2, 9)]);
        assert!(transfer_targets(&metrics, 1, &[]).is_empty());
    }

    #[test]
    fn transfer_targets_without_another_voter_are_empty() {
        let metrics = metrics(1, Some(1), &[1], &[(1, 9)]);
        assert!(transfer_targets(&metrics, 1, &[]).is_empty());
    }

    /// A node with `election_disable` never starts an election on its own, so it is only ever a
    /// fallback: a leadership transfer reaches it, a self-elected takeover never will.
    #[test]
    fn transfer_targets_put_election_disabled_nodes_last() {
        let metrics = metrics(1, Some(1), &[1, 2, 3], &[(2, 4), (3, 9)]);
        assert_eq!(transfer_targets(&metrics, 1, &[3]), vec![2, 3]);
    }

    #[test]
    fn transfer_targets_tie_break_to_the_higher_id() {
        let metrics = metrics(1, Some(1), &[1, 2, 3], &[(2, 7), (3, 7)]);
        assert_eq!(transfer_targets(&metrics, 1, &[]), vec![3, 2]);
    }

    #[test]
    fn transfer_targets_put_unmatched_voters_last() {
        let metrics = metrics(1, Some(1), &[1, 2, 3], &[(2, 4)]);
        assert_eq!(transfer_targets(&metrics, 1, &[]), vec![2, 3]);
    }
}
