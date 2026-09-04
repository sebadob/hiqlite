//! Minimal walkthrough for the `external-state-machine` feature.
//!
//! This feature is for applications that **already own a consensus log**
//! (their own Raft group, an ordered event stream, ...) and only want
//! Hiqlite's SQLite state-machine machinery: one serialized writer, a
//! read-only connection pool, exact retry receipts, durable checkpoints and
//! validated page-image snapshots.
//!
//! No Hiqlite Raft node, network service, or membership store is started.
//!
//! The consensus log is mocked here with a plain in-memory `Vec`. In a real
//! application, replace `MockLog` with whatever already produces committed
//! entries in a total order.

use hiqlite::external_state_machine::{
    ApplyOutcome, CommitSequence, DeterministicSqliteOperation, ExternalApplyError, ExternalCommit,
    ExternalSnapshot, ExternalSqlite, ExternalSqliteOptions, Sha256Digest,
};
// Always use the re-exported `rusqlite` to avoid version conflicts with the
// one Hiqlite is compiled against.
use hiqlite::rusqlite::{self, params, Transaction};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Stable identity of *this* application state machine. It is embedded into
/// the database metadata and every snapshot, so a snapshot from a different
/// application can never be installed by accident.
const APPLICATION_ID: &str = "hiqlite-example-todos";

/// Version of the application schema after each entry has been applied.
/// Bump it when your migrations change the table layout.
const STATE_SCHEMA: u64 = 1;

/// Version of the `Receipt` type below. Bump it when `Receipt` changes.
const RECEIPT_SCHEMA: u64 = 1;

// ---------------------------------------------------------------------------
// 1. Your consensus log (mocked)
// ---------------------------------------------------------------------------

/// The coordinate your consensus system uses to identify an entry. For Raft
/// this is typically `(term, index)`. Hiqlite treats it as an opaque identity:
/// it must be `Eq` + serde, but is never used for ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Coordinate {
    term: u64,
    index: u64,
}

/// The commands your application replicates through its own consensus.
///
/// `MembershipChange` shows an entry that is committed in the global log but
/// does not touch SQLite at all. It still has to be accounted for with
/// `advance_committed`, so the persisted frontier always matches the log.
#[derive(Debug, Clone)]
enum Command {
    CreateSchema,
    AddTodo { id: i64, text: String },
    CompleteTodo { id: i64 },
    MembershipChange,
}

impl Command {
    /// A *canonical*, versioned byte encoding of the command.
    ///
    /// Hiqlite only stores the SHA-256 of these bytes and uses it to detect
    /// whether a retry for a given sequence is the exact same command or a
    /// conflicting one. Any deterministic encoding works, as long as it is
    /// stable across releases.
    fn canonical_bytes(&self) -> Vec<u8> {
        match self {
            Self::CreateSchema => b"v1|create-schema".to_vec(),
            Self::AddTodo { id, text } => format!("v1|add|{id}|{text}").into_bytes(),
            Self::CompleteTodo { id } => format!("v1|complete|{id}").into_bytes(),
            Self::MembershipChange => b"v1|membership".to_vec(),
        }
    }
}

/// One committed entry in the (mocked) consensus log.
#[derive(Debug, Clone)]
struct LogEntry {
    coordinate: Coordinate,
    command: Command,
}

/// Stand-in for a real consensus log. Entries are already committed and
/// totally ordered. Index `i` of the `Vec` is log index `i + 1`.
struct MockLog {
    entries: Vec<LogEntry>,
}

impl MockLog {
    fn new() -> Self {
        let commands = vec![
            Command::CreateSchema,
            Command::AddTodo {
                id: 1,
                text: "write the example".to_string(),
            },
            Command::AddTodo {
                id: 2,
                text: "review the PR".to_string(),
            },
            Command::MembershipChange,
            Command::CompleteTodo { id: 1 },
        ];

        let entries = commands
            .into_iter()
            .enumerate()
            .map(|(i, command)| LogEntry {
                coordinate: Coordinate {
                    term: 1,
                    index: i as u64 + 1,
                },
                command,
            })
            .collect();

        Self { entries }
    }

    /// Returns all committed entries with a log index `> after`.
    fn entries_after(&self, after: u64) -> impl Iterator<Item = (u64, &LogEntry)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i as u64 + 1, e))
            .filter(move |(idx, _)| *idx > after)
    }
}

// ---------------------------------------------------------------------------
// 2. The typed, deterministic SQLite operation
// ---------------------------------------------------------------------------

/// The response that is persisted alongside the mutation, so a lost reply can
/// be recovered without re-executing the operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Receipt {
    SchemaCreated,
    Added { id: i64 },
    Completed { id: i64, already_done: bool },
}

/// The operation Hiqlite executes inside a single SQLite transaction.
///
/// Rules:
/// - It must be deterministic: same input + same database state => same output.
///   Never use `random()`, `datetime('now')`, etc. Hiqlite guards against the
///   most common ones.
/// - Infrastructure failures are returned as `Err`. They roll back the
///   transaction and do *not* consume the sequence.
/// - Deterministic business outcomes (e.g. "already completed") are part of
///   `Ok(Receipt)` so the entry advances and the answer can be replayed.
struct TodoOperation(Command);

impl DeterministicSqliteOperation for TodoOperation {
    type Output = Receipt;
    type Error = rusqlite::Error;

    /// Name + version of the receipt encoding. The default codec is Hiqlite's
    /// bincode configuration; you can override `encode_receipt` /
    /// `decode_receipt` if you want to own the format.
    const RECEIPT_CODEC: &'static str = "hiqlite-example-todos/receipt-v1";

    fn apply(self, txn: &Transaction<'_>) -> Result<Self::Output, Self::Error> {
        match self.0 {
            Command::CreateSchema => {
                txn.execute(
                    "CREATE TABLE IF NOT EXISTS todos (
                        id   INTEGER PRIMARY KEY,
                        text TEXT NOT NULL,
                        done INTEGER NOT NULL DEFAULT 0
                    )",
                    [],
                )?;
                Ok(Receipt::SchemaCreated)
            }
            Command::AddTodo { id, text } => {
                txn.execute(
                    "INSERT INTO todos (id, text) VALUES (?1, ?2)",
                    params![id, text],
                )?;
                Ok(Receipt::Added { id })
            }
            Command::CompleteTodo { id } => {
                let changed = txn.execute(
                    "UPDATE todos SET done = 1 WHERE id = ?1 AND done = 0",
                    params![id],
                )?;
                Ok(Receipt::Completed {
                    id,
                    already_done: changed == 0,
                })
            }
            Command::MembershipChange => {
                unreachable!("membership entries are handled with advance_committed")
            }
        }
    }
}

type Engine = ExternalSqlite<Coordinate, TodoOperation>;

// ---------------------------------------------------------------------------
// 3. Wiring it together
// ---------------------------------------------------------------------------

/// Builds the commit identity Hiqlite needs for one log entry.
///
/// The `CommitSequence` must be dense: `initial_sequence`, then `+1` for
/// every following entry. Here the mocked log index doubles as the sequence.
fn commit_for(index: u64, entry: &LogEntry) -> ExternalCommit<Coordinate> {
    ExternalCommit::new(
        CommitSequence(index),
        entry.coordinate.clone(),
        Sha256Digest::of(entry.command.canonical_bytes()),
        STATE_SCHEMA,
        RECEIPT_SCHEMA,
    )
}

/// Applies every committed entry the engine has not seen yet.
///
/// This is the same code path for a fresh start, a normal run, and crash
/// recovery: compare the durable frontier with the log and replay the rest.
async fn replay(engine: &Engine, log: &MockLog) -> Result<(), Box<dyn std::error::Error>> {
    let frontier = engine
        .last_applied()
        .await?
        .map(|applied| applied.commit.sequence.get())
        .unwrap_or(0);
    info!("engine frontier is at sequence {frontier}, replaying entries after it");

    for (index, entry) in log.entries_after(frontier) {
        let commit = commit_for(index, entry);

        match &entry.command {
            // Entries that do not mutate SQLite still advance the frontier.
            Command::MembershipChange => {
                engine.advance_committed(commit).await?;
                info!("seq {index}: advanced over {:?}", entry.command);
            }
            command => {
                let outcome = engine
                    .apply_committed(commit, TodoOperation(command.clone()))
                    .await
                    .map_err(|err| match err {
                        ExternalApplyError::Operation(err) => {
                            format!("operation failed at seq {index}: {err}")
                        }
                        ExternalApplyError::Engine(err) => {
                            format!("engine failed at seq {index}: {err}")
                        }
                    })?;
                match outcome {
                    ApplyOutcome::Applied(receipt) => {
                        info!("seq {index}: applied {:?} -> {receipt:?}", entry.command)
                    }
                    ApplyOutcome::Recovered(receipt) => {
                        info!("seq {index}: recovered {receipt:?} without re-executing")
                    }
                    // `ApplyOutcome` is `#[non_exhaustive]`
                    other => info!("seq {index}: {other:?}"),
                }
            }
        }
    }

    Ok(())
}

async fn print_todos(engine: &Engine) -> Result<(), Box<dyn std::error::Error>> {
    // Reads run on pooled connections opened with `SQLITE_OPEN_READ_ONLY`.
    let rows = engine
        .read(|conn| {
            let mut stmt = conn.prepare("SELECT id, text, done FROM todos ORDER BY id")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?;

    for (id, text, done) in rows {
        info!("  [{}] {id}: {text}", if done { "x" } else { " " });
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(EnvFilter::from("info"))
        .init();

    // Always start clean for this example.
    let _ = fs::remove_dir_all("./data").await;
    fs::create_dir_all("./data").await?;

    let log = MockLog::new();

    // --- First run: open an empty engine and replay the whole log -----------
    let mut options = ExternalSqliteOptions::new("./data/node", APPLICATION_ID);
    // The first sequence an empty engine accepts. Our mocked log starts at 1.
    options.initial_sequence = CommitSequence(1);
    options.initial_state_schema = STATE_SCHEMA;
    // `ExternalDurability::Full` (fsync on every commit) is the default.
    // `Normal` / `ReplayableOff` are explicit opt-ins that require replay.

    info!("opening the external state machine");
    let engine = Engine::open(options.clone()).await?;
    replay(&engine, &log).await?;
    print_todos(&engine).await?;

    // --- Exact retry: same sequence, same command -> stored receipt --------
    // Imagine the reply to entry 3 was lost and the caller resubmits it.
    let (index, entry) = log.entries_after(2).next().unwrap();
    let outcome = engine
        .apply_committed(
            commit_for(index, entry),
            TodoOperation(entry.command.clone()),
        )
        .await
        .map_err(|err| err.to_string())?;
    assert!(matches!(
        outcome,
        ApplyOutcome::Recovered(Receipt::Added { id: 2 })
    ));
    info!("retry of seq {index} was answered from the retained receipt");

    // Reusing a sequence with a *different* command is a conflict.
    let mut conflicting = commit_for(index, entry);
    conflicting.command_digest = Sha256Digest::of(b"v1|something-else");
    let err = engine
        .apply_committed(conflicting, TodoOperation(entry.command.clone()))
        .await
        .unwrap_err();
    info!("conflicting retry was rejected: {err}");

    // --- Snapshot: one exact frontier, validated page image -----------------
    // The snapshot is taken behind the single writer, so it captures exactly
    // one applied sequence. Persist `snapshot.evidence` in *your* outer
    // snapshot manifest and hand it back on install.
    let snapshot = engine.build_snapshot().await?;
    info!(
        "snapshot {} at sequence {} ({} bytes, sha256 {:02x?}...)",
        snapshot.evidence.snapshot_id,
        snapshot
            .evidence
            .checkpoint
            .as_ref()
            .map(|c| c.commit.sequence.get())
            .unwrap_or(0),
        snapshot.evidence.sqlite_bytes,
        &snapshot.evidence.sqlite_sha256.as_bytes()[..4],
    );
    // Validation is possible without a live engine, e.g. before activation.
    Engine::validate_snapshot(&snapshot).await?;

    // Clean shutdown checkpoints the WAL and removes the dirty marker.
    engine.shutdown().await?;

    // --- Second run: reopen and resume from the durable frontier -----------
    info!("reopening after clean shutdown");
    let engine = Engine::open(options).await?;
    // Nothing new in the log, so nothing is replayed.
    replay(&engine, &log).await?;
    engine.shutdown().await?;

    // --- New node: bootstrap from the snapshot, then replay the tail -------
    // In a real deployment the snapshot file plus its evidence would travel
    // through your own snapshot transfer.
    info!("bootstrapping a second node from the snapshot");
    let mut options = ExternalSqliteOptions::new("./data/node-2", APPLICATION_ID);
    options.initial_sequence = CommitSequence(1);
    options.initial_state_schema = STATE_SCHEMA;

    let engine = Engine::open(options).await?;
    engine
        .install_snapshot(ExternalSnapshot::new(
            snapshot.evidence.clone(),
            &snapshot.path,
        ))
        .await?;
    // If the log had grown past the snapshot, `replay` would apply the rest.
    replay(&engine, &log).await?;
    print_todos(&engine).await?;
    engine.shutdown().await?;

    info!("done");
    Ok(())
}
