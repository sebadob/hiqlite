//! Waiting for Raft state machines to catch up with their WALs at startup.
use openraft::RaftTypeConfig;
use openraft::metrics::WaitError;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Wait until the state machine has applied every log entry that is currently in the WAL.
///
/// `Raft::new` returns as soon as the initial state (last applied from the DB, last log from the
/// WAL) is known; replaying the WAL into the state machine happens asynchronously, because openraft
/// only applies logs after they are re-committed through the cluster. On slow disks this can take
/// a while, so we must not start serving API requests or join the cluster before the node's data
/// is up to date again. If a replay has already finished by the time this runs, it returns without
/// waiting.
pub(crate) async fn wait_for_state_machine_catchup<TC>(raft: &openraft::Raft<TC>, name: &str)
where
    TC: RaftTypeConfig,
{
    let started = Instant::now();
    let timeout = Duration::from_secs(30);
    let deadline = started + timeout;

    // The metrics watch channel starts with an all-`None` initial value that is not a real state
    // report. A node that has logs in its WAL always reports `last_log_index` as `Some`, so any
    // other value on the channel is necessarily a real report and can be trusted.
    let caught_up = |m: &openraft::RaftMetrics<TC::NodeId, TC::Node>| {
        // No logs in the WAL → nothing to apply. Otherwise, the state machine must have applied at
        // least up to the last log.
        m.last_log_index.is_none_or(|last_log| {
            m.last_applied
                .as_ref()
                .is_some_and(|applied| applied.index >= last_log)
        })
    };
    let is_real_report = |m: &openraft::RaftMetrics<TC::NodeId, TC::Node>| {
        m.last_log_index.is_some() || m.last_applied.is_some()
    };

    let mut rx = raft.metrics();

    // Fast path: replay runs in parallel with the rest of startup so it may already be done. If a
    // real report shows the state machine caught up, we can return without waiting at all.
    {
        let current = rx.borrow();
        if is_real_report(&current) && caught_up(&current) {
            log_caught_up(name, started.elapsed(), &current);
            return;
        }
    }

    // If no real report exists yet, wait for the first one, so that `wait().metrics()` below does
    // not mistake the all-`None` initial value for "caught up".
    if !is_real_report(&rx.borrow()) {
        tokio::select! {
            () = tokio::time::sleep_until(deadline.into()) => {
                error!("{name}: no Raft metrics reported within {timeout:?}; proceeding with startup")
            }
            changed = rx.changed() => match changed {
                Ok(()) => {}
                Err(_) => error!(
                    "{name}: Raft metrics channel closed while waiting for the first report; \
                     proceeding with startup"
                ),
            },
        }
    }

    // Wait until the state machine has applied every log entry that is currently in its WAL.
    let remaining = deadline.saturating_duration_since(Instant::now());
    match raft
        .wait(Some(remaining))
        .metrics(
            caught_up,
            format!("startup: {name} state machine catches up with the WAL"),
        )
        .await
    {
        Ok(m) => log_caught_up(name, started.elapsed(), &m),
        Err(WaitError::Timeout(t, latest)) => warn!(
            "{name}: state machine still behind the WAL after {t:?} ({latest}); \
             proceeding with startup"
        ),
        Err(WaitError::ShuttingDown) => error!(
            "{name}: Raft shut down while waiting for the state machine to catch up; \
             proceeding with startup"
        ),
    }
}

fn log_caught_up<NID, N>(name: &str, elapsed: Duration, m: &openraft::RaftMetrics<NID, N>)
where
    NID: openraft::NodeId,
    N: openraft::Node,
{
    if elapsed >= Duration::from_secs(1) {
        info!(
            "{name}: state machine caught up with the WAL after {elapsed:?} \
             (last_log_index: {:?}, last_applied: {:?})",
            m.last_log_index, m.last_applied
        );
    } else {
        debug!(
            "{name}: state machine caught up with the WAL in {elapsed:?} \
             (last_log_index: {:?}, last_applied: {:?})",
            m.last_log_index, m.last_applied
        );
    }
}
