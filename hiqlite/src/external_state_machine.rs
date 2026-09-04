//! SQLite state-machine machinery for applications that already own consensus.
//!
//! This module deliberately does **not** construct a Hiqlite Raft group, a
//! network service, membership metadata, or a raw-SQL replication protocol.
//! The caller submits an already-committed, typed operation in global commit
//! order. Hiqlite serializes it on one SQLite writer and persists the caller's
//! checkpoint and bounded response receipt in the same SQLite transaction.
//!
//! `ExternalSqlite` only makes the SQLite mutation, checkpoint, and receipt
//! atomic with each other. It cannot make another storage engine or the
//! caller's consensus log atomic with SQLite. Recovery must compare the last
//! persisted sequence with the caller's durable log and replay as needed.
//!
//! # Commit and retry contract
//!
//! [`CommitSequence`](crate::external_state_machine::CommitSequence) is the
//! only ordering key and must be dense. The native
//! coordinate `C` is opaque exact identity; it requires `Eq`, never `Ord`.
//! The caller supplies a SHA-256 digest of its versioned canonical command
//! bytes. For every global entry it must call either
//! [`ExternalSqlite::apply_committed`](crate::external_state_machine::ExternalSqlite::apply_committed)
//! or
//! [`ExternalSqlite::advance_committed`](crate::external_state_machine::ExternalSqlite::advance_committed).
//! Retained exact retries compare the sequence, coordinate, command digest,
//! schemas, entry kind, and receipt codec, then return the stored receipt
//! without running the operation again. Older retries return
//! [`ExternalError::ReceiptUnavailable`](crate::external_state_machine::ExternalError::ReceiptUnavailable).
//!
//! Coordinates use Hiqlite's bincode legacy encoding on disk. Operation
//! outputs use that encoding by default, while protocol owners may override
//! [`DeterministicSqliteOperation::encode_receipt`](crate::external_state_machine::DeterministicSqliteOperation::encode_receipt)
//! and its decoder. `C`, the selected output codec, and
//! [`DeterministicSqliteOperation::RECEIPT_CODEC`](crate::external_state_machine::DeterministicSqliteOperation::RECEIPT_CODEC)
//! must remain backward-decodable for every retained receipt and snapshot.
//! `receipt_schema` is identity evidence for the caller; this first version
//! does not dispatch decoders by schema.
//!
//! # Durability and ownership
//!
//! The default is WAL `synchronous=FULL`: a successful commit includes
//! SQLite's WAL sync. `NORMAL` can lose an acknowledged tail after power loss,
//! so the caller must replay from its durable log. `ReplayableOff` is explicit
//! and any unclean OFF marker requires
//! [`ExternalSqlite::rebuild_projection`](crate::external_state_machine::ExternalSqlite::rebuild_projection).
//! Before a clean shutdown removes the durable marker, every mode is upgraded
//! to FULL, checkpoints/truncates the WAL, closes SQLite, and syncs the database
//! file (plus its directory on Unix). An OS advisory lock prevents two engine
//! owners; restore-in-progress and malformed metadata are marked rebuild-only.
//! [`ExternalSqlite::open`](crate::external_state_machine::ExternalSqlite::open)
//! never deletes or auto-heals the database.
//!
//! # Snapshots
//!
//! Snapshot creation runs SQLite Online Backup behind the sole writer queue,
//! so it captures one exact external frontier and blocks later writes for the
//! copy duration while read-only pooled reads can continue. Online Backup is
//! used instead of `VACUUM INTO` because VACUUM may renumber implicit ROWIDs.
//! A temporary file is published without replacement only after a complete
//! copy, then the file and parent directory are synced on Unix before
//! acknowledgement. Evidence binds
//! format, page size, application/schema/frontier/receipts, byte length, and
//! SHA-256, and contains no membership. The caller owns retention of every
//! completed path returned by
//! [`ExternalSqlite::build_snapshot`](crate::external_state_machine::ExternalSqlite::build_snapshot);
//! artifacts are
//! not part of an outer manifest until the caller durably records them.
//!
//! Restore stages and validates the full artifact, then rechecks staleness
//! inside the writer queue. Any failure after live restore begins poisons the
//! engine and leaves a rebuild-required marker. Backup/S3 orchestration from
//! Hiqlite's Raft API is intentionally outside this feature.

use deadpool::unmanaged::PoolError;
use fs4::FileExt;
use rusqlite::backup::Progress;
use rusqlite::functions::FunctionFlags;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::{Debug, Display};
use std::fs::File;
use std::io::{BufReader, Read};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use thread_priority::ThreadPriority;
use tokio::sync::{RwLock, oneshot};
use tokio::{fs, task};
use tracing::warn;
use uuid::Uuid;

type SqlitePool = deadpool::unmanaged::Pool<Connection>;

fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, bincode::error::EncodeError> {
    bincode::serde::encode_to_vec(value, bincode::config::legacy())
}

async fn set_path_access(path: &str, mode: u32) -> Result<(), std::io::Error> {
    #[cfg(target_family = "unix")]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, Permissions::from_mode(mode)).await?;
    }
    #[cfg(not(target_family = "unix"))]
    let _ = (path, mode);
    Ok(())
}

async fn connect_external(
    path: String,
    filename: String,
    synchronous: &'static str,
    prepared_statement_cache_capacity: usize,
) -> Result<Connection, ExternalError> {
    task::spawn_blocking(move || {
        let conn = Connection::open(Path::new(&path).join(filename))?;
        apply_external_write_pragmas(&conn, synchronous, prepared_statement_cache_capacity)?;
        overwrite_non_deterministic_functions(&conn);
        Ok::<_, rusqlite::Error>(conn)
    })
    .await?
    .map_err(ExternalError::from)
}

async fn connect_read_pool_once(
    path: &str,
    filename: &str,
    prepared_statement_cache_capacity: usize,
    pool_size: usize,
) -> Result<SqlitePool, ExternalError> {
    let path = PathBuf::from(path).join(filename);
    let mut connections = Vec::with_capacity(pool_size);
    for _ in 0..pool_size {
        let path = path.clone();
        connections.push(
            task::spawn_blocking(move || {
                let conn = Connection::open_with_flags(
                    path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                conn.pragma_update(None, "query_only", true)?;
                conn.busy_timeout(Duration::from_secs(30))?;
                conn.set_prepared_statement_cache_capacity(prepared_statement_cache_capacity);
                Ok::<_, rusqlite::Error>(conn)
            })
            .await??,
        );
    }
    let pool = SqlitePool::from(connections);
    let conn = pool.get().await?;
    task::spawn_blocking(move || conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0)))
        .await??;
    Ok(pool)
}

fn apply_external_write_pragmas(
    conn: &Connection,
    synchronous: &str,
    prepared_statement_cache_capacity: usize,
) -> Result<(), rusqlite::Error> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", synchronous)?;
    conn.pragma_update(None, "page_size", 4096)?;
    conn.pragma_update(None, "journal_size_limit", 16384)?;
    conn.pragma_update(None, "wal_autocheckpoint", 4_000)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
    conn.pragma_update(None, "optimize", "0x10002")?;
    conn.busy_timeout(Duration::from_secs(30))?;
    conn.set_prepared_statement_cache_capacity(prepared_statement_cache_capacity);
    Ok(())
}

fn overwrite_non_deterministic_functions(conn: &Connection) {
    const FORBIDDEN: &[&str] = &[
        "date",
        "datetime",
        "julianday",
        "now",
        "random",
        "randomblob",
        "strftime",
        "time",
        "timediff",
        "unixepoch",
    ];
    for &name in FORBIDDEN {
        conn.create_scalar_function(
            name,
            -1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |_| -> rusqlite::Result<String> {
                panic!(
                    "forbidden usage of `{name}()` - non-deterministic functions must never be used for committed writes"
                )
            },
        )
        .expect("cannot register non-deterministic function guard");
    }
}

fn restore_snapshot(conn: &mut Connection, path: &Path) -> Result<(), rusqlite::Error> {
    conn.restore(
        "main",
        path,
        Some(|progress: Progress| {
            let _ = progress;
        }),
    )
}

/// On-disk format version of external state-machine metadata and receipts.
pub const EXTERNAL_FORMAT_VERSION: u16 = 1;

/// SQLite artifact format produced by [`ExternalSqlite::build_snapshot`].
///
/// Outer snapshot manifests may persist this value to bind their file
/// inventory without duplicating a private engine literal.
pub const EXTERNAL_SQLITE_SNAPSHOT_FORMAT: &str = "sqlite3-online-backup/page-image-v1";

const METADATA_TABLE: &str = "_hiqlite_external_state";
const RECEIPTS_TABLE: &str = "_hiqlite_external_receipts";
const ADVANCE_RECEIPT_CODEC: &str = "hiqlite/advance-v1";
const MAX_READ_POOL_SIZE: usize = 128;
const MAX_RECEIPT_RETENTION: usize = 1_000_000;
const MAX_RECEIPT_BYTES: usize = 16 * 1024 * 1024;
const MAX_PREPARED_STATEMENTS: usize = 1_000_000;
const DIRTY_FULL: &[u8] = b"active-full\n";
const DIRTY_NORMAL: &[u8] = b"active-normal\n";
const DIRTY_OFF: &[u8] = b"active-replayable-off\n";
const DIRTY_POISONED: &[u8] = b"restore-in-progress\n";

/// Dense ordering key for the external state machine.
///
/// The first accepted value is configured with
/// [`ExternalSqliteOptions::initial_sequence`]. Every later entry must be
/// exactly the previous value plus one. The caller's native Raft coordinate is
/// stored separately and compared for exact retry identity, never ordering.
#[derive(
    Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
pub struct CommitSequence(pub u64);

impl CommitSequence {
    pub const fn get(self) -> u64 {
        self.0
    }

    fn checked_next(self) -> Result<Self, ExternalError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ExternalError::SequenceOverflow)
    }
}

impl Display for CommitSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// A SHA-256 digest used to bind canonical commands, receipts, and snapshots.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Sha256Digest(pub [u8; 32]);

impl Sha256Digest {
    /// Hashes caller-owned canonical bytes.
    ///
    /// Hiqlite does not define a canonical command encoding. The caller must
    /// version that encoding and produce identical bytes for an exact retry.
    pub fn of(bytes: impl AsRef<[u8]>) -> Self {
        Self(Sha256::digest(bytes.as_ref()).into())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Identity and schema evidence for one caller-committed log entry.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExternalCommit<C> {
    /// Dense global state-machine order.
    pub sequence: CommitSequence,
    /// Exact caller-native coordinate, such as term/leader/index.
    pub coordinate: C,
    /// SHA-256 of the caller's stable canonical command bytes.
    pub command_digest: Sha256Digest,
    /// Caller-owned version of the SQLite application schema after this entry.
    pub state_schema: u64,
    /// Caller-owned version of the serialized response receipt.
    pub receipt_schema: u64,
}

impl<C> ExternalCommit<C> {
    pub fn new(
        sequence: CommitSequence,
        coordinate: C,
        command_digest: Sha256Digest,
        state_schema: u64,
        receipt_schema: u64,
    ) -> Self {
        Self {
            sequence,
            coordinate,
            command_digest,
            state_schema,
            receipt_schema,
        }
    }
}

/// Whether a checkpoint applied a typed operation or explicitly advanced over
/// a committed entry that did not mutate this SQLite lane.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ExternalEntryKind {
    Operation,
    Advance,
}

impl ExternalEntryKind {
    fn as_i64(self) -> i64 {
        match self {
            Self::Operation => 1,
            Self::Advance => 2,
        }
    }

    fn from_i64(value: i64) -> Result<Self, ExternalError> {
        match value {
            1 => Ok(Self::Operation),
            2 => Ok(Self::Advance),
            _ => Err(ExternalError::InvalidMetadata(format!(
                "unknown external entry kind {value}"
            ))),
        }
    }
}

/// Durable evidence for the latest globally applied entry.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExternalApplied<C> {
    pub commit: ExternalCommit<C>,
    pub kind: ExternalEntryKind,
    pub receipt_digest: Sha256Digest,
}

impl<C> ExternalApplied<C> {
    /// Creates applied-entry evidence for reconstruction from a caller-owned
    /// outer snapshot manifest.
    pub fn new(
        commit: ExternalCommit<C>,
        kind: ExternalEntryKind,
        receipt_digest: Sha256Digest,
    ) -> Self {
        Self {
            commit,
            kind,
            receipt_digest,
        }
    }
}

/// A deterministic, typed operation interpreted by the caller.
///
/// Infrastructure failures should be returned as `Err`. They roll back the
/// SQLite transaction and do not consume the sequence. Deterministic business
/// rejections must be represented inside `Ok(Output)` so the committed entry
/// advances and the same response can be recovered after a lost reply.
pub trait DeterministicSqliteOperation: Send + 'static {
    type Output: Serialize + DeserializeOwned + Send + 'static;
    type Error: Send + 'static;

    /// Stable caller-owned receipt codec name and version.
    const RECEIPT_CODEC: &'static str;

    fn apply(self, transaction: &Transaction<'_>) -> Result<Self::Output, Self::Error>;

    /// Encodes the durable lost-response receipt.
    ///
    /// The default retains Hiqlite's legacy bincode representation. Protocol
    /// owners may override both codec methods to own a stable canonical format.
    fn encode_receipt(output: &Self::Output) -> Result<Vec<u8>, String> {
        serialize(output).map_err(|err| err.to_string())
    }

    /// Decodes one durable lost-response receipt.
    fn decode_receipt(bytes: &[u8]) -> Result<Self::Output, String> {
        let (output, consumed) =
            bincode::serde::decode_from_slice::<Self::Output, _>(bytes, bincode::config::legacy())
                .map_err(|err| err.to_string())?;
        if consumed != bytes.len() {
            return Err("receipt contains trailing bytes".to_string());
        }
        Ok(output)
    }
}

/// Result of a new application or an exact retained retry.
#[derive(Debug, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum ApplyOutcome<T> {
    Applied(T),
    Recovered(T),
}

/// SQLite acknowledgement durability for external commits.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExternalDurability {
    /// SQLite WAL `synchronous=FULL`. A successful commit waits for the WAL to
    /// reach durable storage according to SQLite and the platform VFS.
    #[default]
    Full,
    /// SQLite WAL `synchronous=NORMAL`. The database remains consistent, but a
    /// power loss may roll back a recently acknowledged transaction. The
    /// caller must compare/replay from its durable consensus log.
    Normal,
    /// SQLite `synchronous=OFF`. This is only for a disposable projection that
    /// the caller will rebuild from a durable snapshot/log after an unclean
    /// shutdown. It is never selected implicitly.
    ReplayableOff,
}

impl ExternalDurability {
    fn pragma_value(self) -> &'static str {
        match self {
            Self::Full => "FULL",
            Self::Normal => "NORMAL",
            Self::ReplayableOff => "OFF",
        }
    }

    fn expected_pragma_value(self) -> i64 {
        match self {
            Self::Full => 2,
            Self::Normal => 1,
            Self::ReplayableOff => 0,
        }
    }

    fn dirty_marker(self) -> &'static [u8] {
        match self {
            Self::Full => DIRTY_FULL,
            Self::Normal => DIRTY_NORMAL,
            Self::ReplayableOff => DIRTY_OFF,
        }
    }
}

/// Configuration for an external SQLite state machine.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ExternalSqliteOptions {
    /// Parent directory. External files live below `external_state_machine/`.
    pub data_dir: PathBuf,
    /// SQLite file name inside the managed database directory.
    pub filename: String,
    /// Stable caller-owned identity used to reject snapshots from another app.
    pub application_id: String,
    /// First dense sequence accepted by an empty state machine.
    pub initial_sequence: CommitSequence,
    /// Application schema before the first entry is applied.
    pub initial_state_schema: u64,
    pub prepared_statement_cache_capacity: usize,
    pub read_pool_size: usize,
    /// Number of contiguous response receipts retained for exact retries.
    pub receipt_retention: usize,
    /// Maximum encoded size of any one response receipt.
    pub max_receipt_bytes: usize,
    /// Defaults to [`ExternalDurability::Full`].
    pub durability: ExternalDurability,
}

impl ExternalSqliteOptions {
    pub fn new(data_dir: impl Into<PathBuf>, application_id: impl Into<String>) -> Self {
        Self {
            data_dir: data_dir.into(),
            filename: "external.sqlite".to_string(),
            application_id: application_id.into(),
            initial_sequence: CommitSequence(1),
            initial_state_schema: 1,
            prepared_statement_cache_capacity: 128,
            read_pool_size: 4,
            receipt_retention: 1024,
            max_receipt_bytes: 64 * 1024,
            durability: ExternalDurability::Full,
        }
    }

    fn validate(&self) -> Result<(), ExternalError> {
        if self.data_dir.to_str().is_none() {
            return Err(ExternalError::InvalidOptions(
                "data_dir must be valid UTF-8 for hiqlite 0.14 path handling".to_string(),
            ));
        }
        if self.application_id.is_empty() || self.application_id.len() > 256 {
            return Err(ExternalError::InvalidOptions(
                "application_id must contain 1..=256 bytes".to_string(),
            ));
        }
        if !matches!(
            Path::new(&self.filename).components().next(),
            Some(std::path::Component::Normal(_))
        ) || Path::new(&self.filename).components().count() != 1
        {
            return Err(ExternalError::InvalidOptions(
                "filename must be one non-empty path component".to_string(),
            ));
        }
        if !(1..=MAX_READ_POOL_SIZE).contains(&self.read_pool_size) {
            return Err(ExternalError::InvalidOptions(format!(
                "read_pool_size must be in 1..={MAX_READ_POOL_SIZE}"
            )));
        }
        if !(1..=MAX_RECEIPT_RETENTION).contains(&self.receipt_retention) {
            return Err(ExternalError::InvalidOptions(format!(
                "receipt_retention must be in 1..={MAX_RECEIPT_RETENTION}"
            )));
        }
        if !(1..=MAX_RECEIPT_BYTES).contains(&self.max_receipt_bytes) {
            return Err(ExternalError::InvalidOptions(format!(
                "max_receipt_bytes must be in 1..={MAX_RECEIPT_BYTES}"
            )));
        }
        if self.prepared_statement_cache_capacity > MAX_PREPARED_STATEMENTS {
            return Err(ExternalError::InvalidOptions(format!(
                "prepared_statement_cache_capacity must be <= {MAX_PREPARED_STATEMENTS}"
            )));
        }
        Ok(())
    }
}

/// Portable evidence bound to a completed SQLite snapshot file.
///
/// It intentionally contains no consensus membership. A caller should embed
/// this value in its own outer snapshot manifest and verify it again on install.
/// Its Serde representation is not a stable wire-format promise; callers that
/// persist evidence should map it into a versioned format they own.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExternalSnapshotEvidence<C> {
    pub format_version: u16,
    pub snapshot_format: String,
    pub snapshot_id: String,
    pub application_id: String,
    pub initial_sequence: CommitSequence,
    pub initial_state_schema: u64,
    pub receipt_codec: String,
    pub checkpoint: Option<ExternalApplied<C>>,
    pub receipt_floor: Option<CommitSequence>,
    pub receipt_retention: u64,
    pub max_receipt_bytes: u64,
    pub sqlite_page_size: u32,
    pub sqlite_bytes: u64,
    pub sqlite_sha256: Sha256Digest,
}

impl<C> ExternalSnapshotEvidence<C> {
    /// Reconstructs engine evidence from a caller-owned, versioned outer
    /// snapshot manifest.
    ///
    /// The engine's `Serialize` representation is descriptive, not a promised
    /// stable wire format. Protocol owners should persist their own frozen
    /// representation and use this constructor when validating or installing
    /// an artifact. Snapshot validation rejects incompatible values.
    #[allow(clippy::too_many_arguments)]
    pub fn from_manifest(
        format_version: u16,
        snapshot_format: String,
        snapshot_id: String,
        application_id: String,
        initial_sequence: CommitSequence,
        initial_state_schema: u64,
        receipt_codec: String,
        checkpoint: Option<ExternalApplied<C>>,
        receipt_floor: Option<CommitSequence>,
        receipt_retention: u64,
        max_receipt_bytes: u64,
        sqlite_page_size: u32,
        sqlite_bytes: u64,
        sqlite_sha256: Sha256Digest,
    ) -> Self {
        Self {
            format_version,
            snapshot_format,
            snapshot_id,
            application_id,
            initial_sequence,
            initial_state_schema,
            receipt_codec,
            checkpoint,
            receipt_floor,
            receipt_retention,
            max_receipt_bytes,
            sqlite_page_size,
            sqlite_bytes,
            sqlite_sha256,
        }
    }
}

/// A completed, caller-staged snapshot and its exact evidence.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ExternalSnapshot<C> {
    pub evidence: ExternalSnapshotEvidence<C>,
    pub path: PathBuf,
}

impl<C> ExternalSnapshot<C> {
    pub fn new(evidence: ExternalSnapshotEvidence<C>, path: impl Into<PathBuf>) -> Self {
        Self {
            evidence,
            path: path.into(),
        }
    }
}

/// Engine and validation failures that never consume a commit sequence.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExternalError {
    #[error("external state machine is already open or was not shut down cleanly: {0}")]
    Locked(PathBuf),
    #[error("external projection must be rebuilt from a validated snapshot/log: {0}")]
    RebuildRequired(PathBuf),
    #[error("external projection does not require a rebuild: {0}")]
    RebuildNotRequired(PathBuf),
    #[error("existing projection cannot be adopted: {0}")]
    AdoptionNotAllowed(String),
    #[error("external state-machine options are invalid: {0}")]
    InvalidOptions(String),
    #[error("external state-machine metadata is missing")]
    MissingMetadata,
    #[error("external state-machine metadata is invalid: {0}")]
    InvalidMetadata(String),
    #[error("unsupported external state-machine format {0}")]
    UnsupportedFormat(u16),
    #[error("external state-machine configuration does not match persisted metadata: {0}")]
    ConfigurationMismatch(String),
    #[error("expected commit sequence {expected}, received {received}")]
    SequenceGap {
        expected: CommitSequence,
        received: CommitSequence,
    },
    #[error("commit sequence {received} is before configured initial sequence {initial}")]
    SequenceBeforeInitial {
        initial: CommitSequence,
        received: CommitSequence,
    },
    #[error("commit sequence overflow")]
    SequenceOverflow,
    #[error("commit identity conflicts with retained sequence {0}")]
    CommitConflict(CommitSequence),
    #[error("response receipt for sequence {0} is outside the retained window")]
    ReceiptUnavailable(CommitSequence),
    #[error("encoded response receipt has {actual} bytes, maximum is {maximum}")]
    ReceiptTooLarge { actual: usize, maximum: usize },
    #[error("serialization failed: {0}")]
    Serialization(String),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("external state-machine setup failed: {0}")]
    Setup(String),
    #[error("read pool error: {0}")]
    Pool(String),
    #[error("blocking task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("external writer is no longer available")]
    WriterClosed,
    #[error("external writer stopped before replying")]
    WriterStopped,
    #[error("external writer was poisoned and requires projection rebuild: {0}")]
    WriterPoisoned(String),
    #[error("snapshot evidence does not match its SQLite file: {0}")]
    SnapshotMismatch(String),
    #[error("snapshot destination already exists: {0}")]
    SnapshotDestinationExists(PathBuf),
    #[error("snapshot checkpoint is older than the live checkpoint")]
    StaleSnapshot,
}

impl From<PoolError> for ExternalError {
    fn from(value: PoolError) -> Self {
        Self::Pool(value.to_string())
    }
}

/// A typed operation failure or an engine failure.
#[derive(Debug)]
pub enum ExternalApplyError<E> {
    Operation(E),
    Engine(ExternalError),
}

impl<E: Display> Display for ExternalApplyError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Operation(err) => write!(f, "deterministic operation failed: {err}"),
            Self::Engine(err) => Display::fmt(err, f),
        }
    }
}

impl<E: Debug + Display> std::error::Error for ExternalApplyError<E> {}

impl<E> From<ExternalError> for ExternalApplyError<E> {
    fn from(value: ExternalError) -> Self {
        Self::Engine(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct StoredState<C> {
    application_id: String,
    initial_sequence: CommitSequence,
    initial_state_schema: u64,
    receipt_codec: String,
    receipt_retention: usize,
    max_receipt_bytes: usize,
    last_applied: Option<ExternalApplied<C>>,
    receipt_floor: Option<CommitSequence>,
    snapshot_id: Option<String>,
}

#[derive(Debug)]
struct StoredReceipt<C> {
    commit: ExternalCommit<C>,
    kind: ExternalEntryKind,
    codec: String,
    receipt: Vec<u8>,
    receipt_digest: Sha256Digest,
}

type ApplyThunk<C> =
    Box<dyn FnOnce(&mut Connection, &mut StoredState<C>) -> Option<ExternalError> + Send>;

enum WriterRequest<C> {
    Apply(ApplyThunk<C>),
    State(oneshot::Sender<StoredState<C>>),
    Snapshot {
        snapshot_id: String,
        path: PathBuf,
        ack: oneshot::Sender<Result<StoredState<C>, ExternalError>>,
    },
    Restore {
        path: PathBuf,
        evidence: Box<ExternalSnapshotEvidence<C>>,
        ack: oneshot::Sender<Result<ExternalSnapshotEvidence<C>, ExternalError>>,
    },
    #[cfg(test)]
    Synchronous(oneshot::Sender<Result<i64, ExternalError>>),
    Shutdown(oneshot::Sender<Result<(), ExternalError>>),
}

struct InitLock {
    path: PathBuf,
    armed: bool,
}

impl InitLock {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for InitLock {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Single-writer SQLite state machine driven by an existing consensus layer.
pub struct ExternalSqlite<C, O> {
    read_pool: RwLock<Option<SqlitePool>>,
    read_pool_config: ExternalReadPoolConfig,
    write_tx: flume::Sender<WriterRequest<C>>,
    snapshots_dir: PathBuf,
    _operation: PhantomData<fn(O)>,
}

struct ExternalReadPoolConfig {
    db_dir: String,
    filename: String,
    prepared_statement_cache_capacity: usize,
    size: usize,
}

impl<C, O> ExternalSqlite<C, O>
where
    C: Clone + Debug + Eq + Serialize + DeserializeOwned + Send + 'static,
    O: DeterministicSqliteOperation,
{
    /// Opens the external engine without creating any Raft or network runtime.
    ///
    /// A held OS owner lock fails closed. A dirty OFF/restore marker requires
    /// explicit rebuild. This never invokes Hiqlite's Raft auto-heal behavior.
    pub async fn open(options: ExternalSqliteOptions) -> Result<Self, ExternalError> {
        Self::open_inner(options, false, false).await
    }

    /// Explicitly deletes a projection for which [`Self::open`] returned
    /// [`ExternalError::RebuildRequired`], then opens an empty engine so the
    /// caller can install a validated snapshot/replay. Completed snapshot
    /// files are preserved. This is never called by [`Self::open`].
    pub async fn rebuild_projection(options: ExternalSqliteOptions) -> Result<Self, ExternalError> {
        Self::open_inner(options, true, false).await
    }

    /// Explicitly adopts a quiescent existing SQLite projection.
    ///
    /// The database must already exist at the managed path, contain no
    /// external-state-machine metadata, pass `quick_check`, and have no dirty
    /// marker from an earlier external owner. This operation records an empty
    /// external frontier; the caller is responsible for proving that the
    /// adopted application state is the agreed initial state.
    pub async fn adopt_existing_projection(
        options: ExternalSqliteOptions,
    ) -> Result<Self, ExternalError> {
        Self::open_inner(options, false, true).await
    }

    async fn open_inner(
        options: ExternalSqliteOptions,
        rebuild_projection: bool,
        adopt_existing: bool,
    ) -> Result<Self, ExternalError> {
        options.validate()?;

        let base_dir = options.data_dir.join("external_state_machine");
        let db_dir = base_dir.join("db");
        let snapshots_dir = base_dir.join("snapshots");
        fs::create_dir_all(&db_dir).await?;
        fs::create_dir_all(&snapshots_dir).await?;
        set_path_access(base_dir.to_string_lossy().as_ref(), 0o700)
            .await
            .map_err(|err| ExternalError::Setup(err.to_string()))?;
        set_path_access(db_dir.to_string_lossy().as_ref(), 0o700)
            .await
            .map_err(|err| ExternalError::Setup(err.to_string()))?;
        set_path_access(snapshots_dir.to_string_lossy().as_ref(), 0o700)
            .await
            .map_err(|err| ExternalError::Setup(err.to_string()))?;

        let owner_path = base_dir.join("owner.lock");
        let owner_lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&owner_path)?;
        match FileExt::try_lock(&owner_lock) {
            Ok(()) => {}
            Err(fs4::TryLockError::WouldBlock) => {
                return Err(ExternalError::Locked(owner_path));
            }
            Err(fs4::TryLockError::Error(err)) => return Err(err.into()),
        }
        sync_file_and_parent_blocking(&owner_path)?;

        let dirty_path = base_dir.join("dirty");
        let dirty_existed = std::fs::exists(&dirty_path)?;
        if adopt_existing && dirty_existed {
            return Err(ExternalError::AdoptionNotAllowed(
                "an external dirty marker already exists".to_string(),
            ));
        }
        let rebuild_required = if dirty_existed {
            !matches!(
                std::fs::read(&dirty_path)?.as_slice(),
                DIRTY_FULL | DIRTY_NORMAL
            )
        } else {
            false
        };
        if rebuild_projection {
            if !rebuild_required {
                return Err(ExternalError::RebuildNotRequired(dirty_path));
            }
            remove_projection_files(&db_dir, &options.filename).await?;
        } else if rebuild_required {
            return Err(ExternalError::RebuildRequired(dirty_path));
        }

        let mut init_lock = InitLock {
            path: dirty_path.clone(),
            armed: !dirty_existed,
        };
        let active_marker = options.durability.dirty_marker();
        write_dirty_marker(&dirty_path, active_marker)?;

        let db_existed = fs::metadata(db_dir.join(&options.filename)).await.is_ok();
        if adopt_existing && !db_existed {
            return Err(ExternalError::AdoptionNotAllowed(
                "the managed SQLite database does not exist".to_string(),
            ));
        }
        if db_existed {
            let preflight_result = (|| {
                let preflight = Connection::open_with_flags(
                    db_dir.join(&options.filename),
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                if adopt_existing {
                    if metadata_table_exists(&preflight)? {
                        return Err(ExternalError::AdoptionNotAllowed(
                            "external metadata already exists".to_string(),
                        ));
                    }
                    let quick_check: String =
                        preflight.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
                    if quick_check != "ok" {
                        return Err(ExternalError::AdoptionNotAllowed(format!(
                            "SQLite quick_check returned {quick_check}"
                        )));
                    }
                } else {
                    require_metadata_table(&preflight)?;
                    let state = load_state::<C>(&preflight)?;
                    validate_configuration(&state, &options, O::RECEIPT_CODEC)?;
                    validate_receipts::<C, O>(&preflight, &state)?;
                }
                Ok::<(), ExternalError>(())
            })();
            if let Err(err) = preflight_result {
                if requires_projection_rebuild(&err) {
                    write_dirty_marker(&dirty_path, DIRTY_POISONED)?;
                    init_lock.disarm();
                }
                return Err(err);
            }
        }

        let db_dir_text = db_dir.to_string_lossy().into_owned();
        let conn = connect_external(
            db_dir_text.clone(),
            options.filename.clone(),
            options.durability.pragma_value(),
            options.prepared_statement_cache_capacity,
        )
        .await
        .map_err(|err| ExternalError::Setup(err.to_string()))?;
        let actual_sync: i64 = conn.pragma_query_value(None, "synchronous", |row| row.get(0))?;
        if actual_sync != options.durability.expected_pragma_value() {
            return Err(ExternalError::ConfigurationMismatch(format!(
                "requested synchronous={:?}, SQLite reported {actual_sync}",
                options.durability
            )));
        }

        if db_existed && !adopt_existing {
            require_metadata_table(&conn)?;
        } else {
            create_metadata_tables(&conn, &options, O::RECEIPT_CODEC)?;
        }
        let state = load_state::<C>(&conn)?;
        validate_configuration(&state, &options, O::RECEIPT_CODEC)?;
        validate_receipts::<C, O>(&conn, &state)?;

        let read_pool = connect_read_pool_once(
            &db_dir_text,
            &options.filename,
            options.prepared_statement_cache_capacity,
            options.read_pool_size,
        )
        .await
        .map_err(|err| ExternalError::Pool(err.to_string()))?;

        let db_path = db_dir.join(&options.filename);
        let write_tx = spawn_external_writer::<C, O>(
            conn,
            state,
            dirty_path,
            db_path,
            active_marker,
            owner_lock,
        );
        init_lock.disarm();

        Ok(Self {
            read_pool: RwLock::new(Some(read_pool)),
            read_pool_config: ExternalReadPoolConfig {
                db_dir: db_dir_text,
                filename: options.filename,
                prepared_statement_cache_capacity: options.prepared_statement_cache_capacity,
                size: options.read_pool_size,
            },
            write_tx,
            snapshots_dir,
            _operation: PhantomData,
        })
    }

    /// Applies a typed operation at the next global sequence.
    ///
    /// An exact retained retry returns [`ApplyOutcome::Recovered`] without
    /// executing the operation. Reusing a sequence with a different native
    /// coordinate, command digest, schema, entry kind, or receipt codec fails.
    pub async fn apply_committed(
        &self,
        commit: ExternalCommit<C>,
        operation: O,
    ) -> Result<ApplyOutcome<O::Output>, ExternalApplyError<O::Error>> {
        let (ack, rx) = oneshot::channel();
        let thunk = Box::new(move |conn: &mut Connection, state: &mut StoredState<C>| {
            let mut result = apply_operation::<C, O>(conn, state, commit, operation);
            let poison = reconcile_after_apply::<C, O>(conn, state, result.is_err())
                .err()
                .map(|err| err.to_string());
            if let Some(message) = poison.as_ref() {
                result = Err(ExternalError::WriterPoisoned(message.clone()).into());
            }
            let _ = ack.send(result);
            poison.map(ExternalError::WriterPoisoned)
        });
        self.write_tx
            .send_async(WriterRequest::Apply(thunk))
            .await
            .map_err(|_| ExternalError::WriterClosed)?;
        rx.await.map_err(|_| ExternalError::WriterStopped)?
    }

    /// Durably advances over a committed non-SQLite entry.
    ///
    /// Callers must invoke either this method or [`Self::apply_committed`] for
    /// every global state-machine entry. This keeps snapshots aligned to the
    /// common applied frontier and makes omitted entries fail as sequence gaps.
    pub async fn advance_committed(
        &self,
        commit: ExternalCommit<C>,
    ) -> Result<ApplyOutcome<()>, ExternalError> {
        let (ack, rx) = oneshot::channel();
        let thunk = Box::new(move |conn: &mut Connection, state: &mut StoredState<C>| {
            let mut result = apply_advance(conn, state, commit);
            let poison = reconcile_after_apply::<C, O>(conn, state, result.is_err())
                .err()
                .map(|err| err.to_string());
            if let Some(message) = poison.as_ref() {
                result = Err(ExternalError::WriterPoisoned(message.clone()));
            }
            let _ = ack.send(result);
            poison.map(ExternalError::WriterPoisoned)
        });
        self.write_tx
            .send_async(WriterRequest::Apply(thunk))
            .await
            .map_err(|_| ExternalError::WriterClosed)?;
        rx.await.map_err(|_| ExternalError::WriterStopped)?
    }

    pub async fn last_applied(&self) -> Result<Option<ExternalApplied<C>>, ExternalError> {
        Ok(self.state().await?.last_applied)
    }

    /// Runs a typed read on one pooled connection.
    ///
    /// Pool connections use `SQLITE_OPEN_READ_ONLY`; toggling `query_only`
    /// cannot turn them into a write bypass.
    pub async fn read<T, F>(&self, read: F) -> Result<T, ExternalError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let read_pool = self.read_pool.read().await;
        let pool = read_pool.as_ref().ok_or_else(|| {
            ExternalError::Pool("read pool is unavailable after snapshot restore".to_string())
        })?;
        let conn = pool.get().await?;
        task::spawn_blocking(move || read(&conn))
            .await?
            .map_err(ExternalError::from)
    }

    /// Builds a complete SQLite snapshot behind the writer queue.
    ///
    /// SQLite Online Backup writes to a temporary name and atomically renames
    /// the completed artifact. A final UUID file is the crash-completion marker;
    /// the returned format/page-size/length/SHA-256 bind it to an outer manifest.
    /// Completed snapshot-file retention belongs to the caller.
    pub async fn build_snapshot(&self) -> Result<ExternalSnapshot<C>, ExternalError> {
        let snapshot_id = Uuid::now_v7().to_string();
        let path = self.snapshots_dir.join(&snapshot_id);
        self.build_snapshot_at(snapshot_id, path).await
    }

    /// Builds a complete snapshot at a caller-owned final path.
    ///
    /// The destination must not exist and its parent must already exist. The
    /// final name is published without replacement only after the SQLite copy
    /// and staging file are durable. This lets an outer snapshot builder own
    /// staging and retention without granting it a raw database handle.
    pub async fn build_snapshot_into(
        &self,
        path: impl Into<PathBuf>,
    ) -> Result<ExternalSnapshot<C>, ExternalError> {
        self.build_snapshot_at(Uuid::now_v7().to_string(), path.into())
            .await
    }

    /// Validates a caller-staged snapshot without opening or mutating a live
    /// external state machine.
    ///
    /// This proves the regular file, digest, SQLite integrity, embedded
    /// metadata, checkpoint, and retained receipt window match the supplied
    /// evidence. It does not prove compatibility with a particular live
    /// engine, activate the image, or authorize consensus-log reclamation.
    pub async fn validate_snapshot(snapshot: &ExternalSnapshot<C>) -> Result<(), ExternalError> {
        let path = snapshot.path.clone();
        let evidence = snapshot.evidence.clone();
        task::spawn_blocking(move || validate_snapshot_file::<C, O>(&path, &evidence)).await??;
        Ok(())
    }

    async fn build_snapshot_at(
        &self,
        snapshot_id: String,
        path: PathBuf,
    ) -> Result<ExternalSnapshot<C>, ExternalError> {
        match fs::symlink_metadata(&path).await {
            Ok(_) => return Err(ExternalError::SnapshotDestinationExists(path)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        let (ack, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::Snapshot {
                snapshot_id: snapshot_id.clone(),
                path: path.clone(),
                ack,
            })
            .await
            .map_err(|_| ExternalError::WriterClosed)?;
        let state = rx.await.map_err(|_| ExternalError::WriterStopped)??;
        let (sqlite_page_size, sqlite_bytes, sqlite_sha256) =
            inspect_snapshot(path.clone()).await?;
        let evidence =
            state.snapshot_evidence(snapshot_id, sqlite_page_size, sqlite_bytes, sqlite_sha256);
        Ok(ExternalSnapshot { evidence, path })
    }

    /// Validates and restores a caller-staged snapshot.
    ///
    /// Format, application identity, checkpoint, receipt window, file size,
    /// SHA-256, and SQLite `quick_check` are verified before the live database
    /// is touched. Older snapshots are rejected.
    pub async fn install_snapshot(
        &self,
        snapshot: ExternalSnapshot<C>,
    ) -> Result<ExternalSnapshotEvidence<C>, ExternalError> {
        let source = snapshot.path.clone();
        let source_evidence = snapshot.evidence.clone();
        task::spawn_blocking(move || validate_snapshot_file::<C, O>(&source, &source_evidence))
            .await??;

        let parsed_id = Uuid::parse_str(&snapshot.evidence.snapshot_id)
            .map_err(|err| ExternalError::SnapshotMismatch(err.to_string()))?;
        let destination = self.snapshots_dir.join(parsed_id.to_string());
        if destination != snapshot.path {
            if fs::try_exists(&destination).await? {
                let path = destination.clone();
                let evidence = snapshot.evidence.clone();
                task::spawn_blocking(move || validate_snapshot_file::<C, O>(&path, &evidence))
                    .await??;
            } else {
                let staging = self.snapshots_dir.join(format!("{}.incoming", parsed_id));
                let _ = fs::remove_file(&staging).await;
                fs::copy(&snapshot.path, &staging).await?;
                sync_file(staging.clone()).await?;
                fs::rename(&staging, &destination).await?;
                sync_file_and_parent(destination.clone()).await?;
            }
        }

        // Prevent an old read transaction or prepared handle from spanning the
        // restore. Dropping the complete pool also makes every post-restore
        // read open against the restored SQLite generation.
        let mut read_pool = self.read_pool.write().await;
        let old_read_pool = read_pool.take();
        drop(old_read_pool);

        let (ack, rx) = oneshot::channel();
        let restore = match self
            .write_tx
            .send_async(WriterRequest::Restore {
                path: destination,
                evidence: Box::new(snapshot.evidence),
                ack,
            })
            .await
        {
            Ok(()) => rx.await.map_err(|_| ExternalError::WriterStopped)?,
            Err(_) => Err(ExternalError::WriterClosed),
        };

        let writer_can_serve_reads = !matches!(
            restore,
            Err(ExternalError::WriterClosed)
                | Err(ExternalError::WriterStopped)
                | Err(ExternalError::WriterPoisoned(_))
        );
        if writer_can_serve_reads {
            *read_pool = Some(
                connect_read_pool_once(
                    &self.read_pool_config.db_dir,
                    &self.read_pool_config.filename,
                    self.read_pool_config.prepared_statement_cache_capacity,
                    self.read_pool_config.size,
                )
                .await
                .map_err(|err| ExternalError::Pool(err.to_string()))?,
            );
        }
        restore
    }

    /// Gracefully checkpoints/closes the writer, removes the dirty marker, and
    /// releases the advisory owner lock. The `owner.lock` inode is persistent.
    pub async fn shutdown(self) -> Result<(), ExternalError> {
        let Self {
            write_tx,
            read_pool,
            read_pool_config: _,
            snapshots_dir: _,
            _operation: _,
        } = self;
        drop(read_pool.into_inner());
        let (ack, rx) = oneshot::channel();
        write_tx
            .send_async(WriterRequest::Shutdown(ack))
            .await
            .map_err(|_| ExternalError::WriterClosed)?;
        rx.await.map_err(|_| ExternalError::WriterStopped)?
    }

    async fn state(&self) -> Result<StoredState<C>, ExternalError> {
        let (ack, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::State(ack))
            .await
            .map_err(|_| ExternalError::WriterClosed)?;
        rx.await.map_err(|_| ExternalError::WriterStopped)
    }

    #[cfg(test)]
    async fn synchronous_for_test(&self) -> Result<i64, ExternalError> {
        let (ack, rx) = oneshot::channel();
        self.write_tx
            .send_async(WriterRequest::Synchronous(ack))
            .await
            .map_err(|_| ExternalError::WriterClosed)?;
        rx.await.map_err(|_| ExternalError::WriterStopped)?
    }
}

impl<C> StoredState<C>
where
    C: Clone,
{
    fn snapshot_evidence(
        &self,
        snapshot_id: String,
        sqlite_page_size: u32,
        sqlite_bytes: u64,
        sqlite_sha256: Sha256Digest,
    ) -> ExternalSnapshotEvidence<C> {
        ExternalSnapshotEvidence {
            format_version: EXTERNAL_FORMAT_VERSION,
            snapshot_format: EXTERNAL_SQLITE_SNAPSHOT_FORMAT.to_string(),
            snapshot_id,
            application_id: self.application_id.clone(),
            initial_sequence: self.initial_sequence,
            initial_state_schema: self.initial_state_schema,
            receipt_codec: self.receipt_codec.clone(),
            checkpoint: self.last_applied.clone(),
            receipt_floor: self.receipt_floor,
            receipt_retention: self.receipt_retention as u64,
            max_receipt_bytes: self.max_receipt_bytes as u64,
            sqlite_page_size,
            sqlite_bytes,
            sqlite_sha256,
        }
    }
}

fn spawn_external_writer<C, O>(
    mut conn: Connection,
    mut state: StoredState<C>,
    dirty_path: PathBuf,
    db_path: PathBuf,
    active_marker: &'static [u8],
    _owner_lock: File,
) -> flume::Sender<WriterRequest<C>>
where
    C: Clone + Debug + Eq + Serialize + DeserializeOwned + Send + 'static,
    O: DeterministicSqliteOperation,
{
    let (tx, rx) = flume::bounded(1);
    thread::spawn(move || {
        let _ = ThreadPriority::Max.set_for_current();
        let mut shutdown_ack = None;
        let mut poisoned = None;
        while let Ok(request) = rx.recv() {
            match request {
                WriterRequest::Apply(apply) => {
                    if let Some(err) = apply(&mut conn, &mut state) {
                        let _ = write_dirty_marker(&dirty_path, DIRTY_POISONED);
                        poisoned = Some(err);
                        break;
                    }
                }
                WriterRequest::State(ack) => {
                    let _ = ack.send(state.clone());
                }
                WriterRequest::Snapshot {
                    snapshot_id,
                    path,
                    ack,
                } => {
                    let result = (|| {
                        persist_snapshot_id(&conn, &snapshot_id)?;
                        state.snapshot_id = Some(snapshot_id);
                        create_external_snapshot(&conn, &path)?;
                        Ok(state.clone())
                    })();
                    let _ = ack.send(result);
                }
                WriterRequest::Restore {
                    path,
                    evidence,
                    ack,
                } => {
                    let preflight = (|| {
                        validate_snapshot_compatibility(&state, &evidence)?;
                        reject_stale_snapshot(
                            state.last_applied.as_ref(),
                            evidence.checkpoint.as_ref(),
                        )?;
                        validate_snapshot_file::<C, O>(&path, &evidence)?;
                        Ok::<(), ExternalError>(())
                    })();
                    if let Err(err) = preflight {
                        let _ = ack.send(Err(err));
                        continue;
                    }
                    if let Err(err) = write_dirty_marker(&dirty_path, DIRTY_POISONED) {
                        let _ = ack.send(Err(err));
                        continue;
                    }

                    let result = (|| {
                        restore_snapshot(&mut conn, &path)?;
                        let restored = load_state::<C>(&conn)?;
                        validate_receipts::<C, O>(&conn, &restored)?;
                        let restored_evidence = restored.snapshot_evidence(
                            evidence.snapshot_id.clone(),
                            evidence.sqlite_page_size,
                            evidence.sqlite_bytes,
                            evidence.sqlite_sha256,
                        );
                        if restored_evidence != *evidence {
                            return Err(ExternalError::SnapshotMismatch(
                                "restored metadata differs from supplied evidence".to_string(),
                            ));
                        }
                        write_dirty_marker(&dirty_path, active_marker)?;
                        state = restored.clone();
                        Ok(restored_evidence)
                    })();
                    match result {
                        Ok(evidence) => {
                            let _ = ack.send(Ok(evidence));
                        }
                        Err(err) => {
                            let message = err.to_string();
                            let err = ExternalError::WriterPoisoned(format!(
                                "live snapshot restore failed: {message}"
                            ));
                            let _ = ack.send(Err(err));
                            poisoned = Some(ExternalError::WriterPoisoned(format!(
                                "live snapshot restore failed: {message}"
                            )));
                            break;
                        }
                    }
                }
                #[cfg(test)]
                WriterRequest::Synchronous(ack) => {
                    let result = conn
                        .pragma_query_value(None, "synchronous", |row| row.get(0))
                        .map_err(ExternalError::from);
                    let _ = ack.send(result);
                }
                WriterRequest::Shutdown(ack) => {
                    shutdown_ack = Some(ack);
                    break;
                }
            }
        }
        if let Err(err) = conn.execute("PRAGMA optimize", []) {
            warn!("Error optimizing external SQLite state machine: {err}");
        }
        let durable_result = if poisoned.is_none() {
            finalize_projection(&conn)
        } else {
            Ok(())
        };
        let close_result = conn.close().map_err(|(_, err)| ExternalError::Sqlite(err));
        let exit_result = match (poisoned, durable_result, close_result) {
            (Some(err), _, _) => Err(err),
            (None, Err(err), _) => Err(err),
            (None, Ok(()), Err(err)) => Err(err),
            (None, Ok(()), Ok(())) => sync_file_and_parent_blocking(&db_path)
                .and_then(|()| remove_dirty_marker(&dirty_path)),
        };
        drop(_owner_lock);
        if let Some(ack) = shutdown_ack {
            let _ = ack.send(exit_result);
        }
    });
    tx
}

fn apply_operation<C, O>(
    conn: &mut Connection,
    state: &mut StoredState<C>,
    commit: ExternalCommit<C>,
    operation: O,
) -> Result<ApplyOutcome<O::Output>, ExternalApplyError<O::Error>>
where
    C: Clone + Debug + Eq + Serialize + DeserializeOwned,
    O: DeterministicSqliteOperation,
{
    match classify_commit(
        conn,
        state,
        &commit,
        ExternalEntryKind::Operation,
        O::RECEIPT_CODEC,
    )? {
        CommitClass::Retry(receipt) => {
            let output =
                O::decode_receipt(&receipt.receipt).map_err(ExternalError::Serialization)?;
            return Ok(ApplyOutcome::Recovered(output));
        }
        CommitClass::New => {}
    }

    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(ExternalError::from)?;
    transaction
        .authorizer(Some(external_operation_authorizer))
        .map_err(ExternalError::from)?;
    let operation_result = operation.apply(&transaction);
    transaction
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
        .map_err(ExternalError::from)?;
    let output = operation_result.map_err(ExternalApplyError::Operation)?;
    let receipt = O::encode_receipt(&output).map_err(ExternalError::Serialization)?;
    if receipt.len() > state.max_receipt_bytes {
        return Err(ExternalError::ReceiptTooLarge {
            actual: receipt.len(),
            maximum: state.max_receipt_bytes,
        }
        .into());
    }
    let receipt_digest = Sha256Digest::of(&receipt);
    persist_new_commit(
        &transaction,
        state,
        &commit,
        ExternalEntryKind::Operation,
        O::RECEIPT_CODEC,
        &receipt,
        receipt_digest,
    )?;
    transaction.commit().map_err(ExternalError::from)?;
    update_state_after_commit(state, commit, ExternalEntryKind::Operation, receipt_digest);
    Ok(ApplyOutcome::Applied(output))
}

fn reconcile_after_apply<C, O>(
    conn: &Connection,
    state: &mut StoredState<C>,
    apply_failed: bool,
) -> Result<(), ExternalError>
where
    C: Clone + Debug + Eq + Serialize + DeserializeOwned,
    O: DeterministicSqliteOperation,
{
    if !conn.is_autocommit() {
        conn.execute_batch("ROLLBACK")?;
    }
    if !conn.is_autocommit() {
        return Err(ExternalError::InvalidMetadata(
            "SQLite remained inside a transaction after apply".to_string(),
        ));
    }
    if apply_failed {
        let stored = load_state::<C>(conn)?;
        validate_receipts::<C, O>(conn, &stored)?;
        *state = stored;
    }
    Ok(())
}

fn external_operation_authorizer(context: AuthContext<'_>) -> Authorization {
    use Authorization::{Allow, Deny};

    let protected = |name: &str| name.starts_with("_hiqlite_external_");
    match context.action {
        AuthAction::CreateIndex {
            index_name,
            table_name,
        }
        | AuthAction::DropIndex {
            index_name,
            table_name,
        }
        | AuthAction::CreateTrigger {
            trigger_name: index_name,
            table_name,
        }
        | AuthAction::DropTrigger {
            trigger_name: index_name,
            table_name,
        } if protected(index_name) || protected(table_name) => Deny,
        AuthAction::CreateTable { table_name }
        | AuthAction::DropTable { table_name }
        | AuthAction::CreateVtable { table_name, .. }
        | AuthAction::DropVtable { table_name, .. }
        | AuthAction::AlterTable { table_name, .. }
        | AuthAction::Analyze { table_name }
            if protected(table_name) =>
        {
            Deny
        }
        AuthAction::Insert { table_name }
        | AuthAction::Delete { table_name }
        | AuthAction::Update { table_name, .. }
            if protected(table_name) =>
        {
            Deny
        }
        AuthAction::Read { table_name, .. } if protected(table_name) => Deny,
        AuthAction::Reindex { index_name } if protected(index_name) => Deny,
        AuthAction::CreateTempIndex { .. }
        | AuthAction::CreateTempTable { .. }
        | AuthAction::CreateTempTrigger { .. }
        | AuthAction::CreateTempView { .. }
        | AuthAction::DropTempIndex { .. }
        | AuthAction::DropTempTable { .. }
        | AuthAction::DropTempTrigger { .. }
        | AuthAction::DropTempView { .. }
        | AuthAction::Pragma { .. }
        | AuthAction::Attach { .. }
        | AuthAction::Detach { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. } => Deny,
        _ => Allow,
    }
}

fn apply_advance<C>(
    conn: &mut Connection,
    state: &mut StoredState<C>,
    commit: ExternalCommit<C>,
) -> Result<ApplyOutcome<()>, ExternalError>
where
    C: Clone + Debug + Eq + Serialize + DeserializeOwned,
{
    match classify_commit(
        conn,
        state,
        &commit,
        ExternalEntryKind::Advance,
        ADVANCE_RECEIPT_CODEC,
    )? {
        CommitClass::Retry(_) => return Ok(ApplyOutcome::Recovered(())),
        CommitClass::New => {}
    }

    let receipt = serialize(&()).map_err(|err| ExternalError::Serialization(err.to_string()))?;
    let receipt_digest = Sha256Digest::of(&receipt);
    let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    persist_new_commit(
        &transaction,
        state,
        &commit,
        ExternalEntryKind::Advance,
        ADVANCE_RECEIPT_CODEC,
        &receipt,
        receipt_digest,
    )?;
    transaction.commit()?;
    update_state_after_commit(state, commit, ExternalEntryKind::Advance, receipt_digest);
    Ok(ApplyOutcome::Applied(()))
}

enum CommitClass<C> {
    New,
    Retry(StoredReceipt<C>),
}

fn classify_commit<C>(
    conn: &Connection,
    state: &StoredState<C>,
    commit: &ExternalCommit<C>,
    kind: ExternalEntryKind,
    codec: &str,
) -> Result<CommitClass<C>, ExternalError>
where
    C: Clone + Debug + Eq + Serialize + DeserializeOwned,
{
    match &state.last_applied {
        None => match commit.sequence.cmp(&state.initial_sequence) {
            std::cmp::Ordering::Equal => Ok(CommitClass::New),
            std::cmp::Ordering::Greater => Err(ExternalError::SequenceGap {
                expected: state.initial_sequence,
                received: commit.sequence,
            }),
            std::cmp::Ordering::Less => Err(ExternalError::SequenceBeforeInitial {
                initial: state.initial_sequence,
                received: commit.sequence,
            }),
        },
        Some(current) if commit.sequence <= current.commit.sequence => {
            let receipt = load_receipt::<C>(conn, commit.sequence, state.max_receipt_bytes)?
                .ok_or(ExternalError::ReceiptUnavailable(commit.sequence))?;
            if receipt.commit != *commit || receipt.kind != kind || receipt.codec != codec {
                return Err(ExternalError::CommitConflict(commit.sequence));
            }
            Ok(CommitClass::Retry(receipt))
        }
        Some(current) => {
            let expected = current.commit.sequence.checked_next()?;
            if commit.sequence == expected {
                Ok(CommitClass::New)
            } else {
                Err(ExternalError::SequenceGap {
                    expected,
                    received: commit.sequence,
                })
            }
        }
    }
}

fn update_state_after_commit<C>(
    state: &mut StoredState<C>,
    commit: ExternalCommit<C>,
    kind: ExternalEntryKind,
    receipt_digest: Sha256Digest,
) {
    let floor_value = commit
        .sequence
        .0
        .saturating_sub(state.receipt_retention.saturating_sub(1) as u64)
        .max(state.initial_sequence.0);
    state.last_applied = Some(ExternalApplied {
        commit,
        kind,
        receipt_digest,
    });
    state.receipt_floor = Some(CommitSequence(floor_value));
}

fn persist_new_commit<C>(
    transaction: &Transaction<'_>,
    state: &StoredState<C>,
    commit: &ExternalCommit<C>,
    kind: ExternalEntryKind,
    codec: &str,
    receipt: &[u8],
    receipt_digest: Sha256Digest,
) -> Result<(), ExternalError>
where
    C: Serialize,
{
    let coordinate = serialize(&commit.coordinate)
        .map_err(|err| ExternalError::Serialization(err.to_string()))?;
    let floor = commit
        .sequence
        .0
        .saturating_sub(state.receipt_retention.saturating_sub(1) as u64)
        .max(state.initial_sequence.0);
    transaction.execute(
        &format!(
            "INSERT INTO {RECEIPTS_TABLE} \
             (sequence, coordinate, command_digest, kind, state_schema, receipt_schema, codec, receipt, receipt_digest) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        ),
        params![
            encode_u64(commit.sequence.0),
            coordinate,
            commit.command_digest.0.as_slice(),
            kind.as_i64(),
            encode_u64(commit.state_schema),
            encode_u64(commit.receipt_schema),
            codec,
            receipt,
            receipt_digest.0.as_slice(),
        ],
    )?;
    transaction.execute(
        &format!(
            "UPDATE {METADATA_TABLE} SET \
             last_sequence=?1, last_coordinate=?2, last_command_digest=?3, last_kind=?4, \
             last_state_schema=?5, last_receipt_schema=?6, last_receipt_digest=?7, \
             receipt_floor=?8 WHERE singleton=1"
        ),
        params![
            encode_u64(commit.sequence.0),
            serialize(&commit.coordinate)
                .map_err(|err| ExternalError::Serialization(err.to_string()))?,
            commit.command_digest.0.as_slice(),
            kind.as_i64(),
            encode_u64(commit.state_schema),
            encode_u64(commit.receipt_schema),
            receipt_digest.0.as_slice(),
            encode_u64(floor),
        ],
    )?;
    transaction.execute(
        &format!("DELETE FROM {RECEIPTS_TABLE} WHERE sequence < ?1"),
        params![encode_u64(floor)],
    )?;
    Ok(())
}

fn create_metadata_tables(
    conn: &Connection,
    options: &ExternalSqliteOptions,
    receipt_codec: &str,
) -> Result<(), ExternalError> {
    conn.execute_batch(&format!(
        "CREATE TABLE {METADATA_TABLE} (\
             singleton INTEGER PRIMARY KEY CHECK(singleton=1),\
             format_version INTEGER NOT NULL,\
             application_id TEXT NOT NULL,\
             initial_sequence BLOB NOT NULL,\
             initial_state_schema BLOB NOT NULL,\
             receipt_codec TEXT NOT NULL,\
             receipt_retention BLOB NOT NULL,\
             max_receipt_bytes BLOB NOT NULL,\
             last_sequence BLOB,\
             last_coordinate BLOB,\
             last_command_digest BLOB,\
             last_kind INTEGER,\
             last_state_schema BLOB,\
             last_receipt_schema BLOB,\
             last_receipt_digest BLOB,\
             receipt_floor BLOB,\
             snapshot_id TEXT\
         );\
         CREATE TABLE {RECEIPTS_TABLE} (\
             sequence BLOB PRIMARY KEY,\
             coordinate BLOB NOT NULL,\
             command_digest BLOB NOT NULL,\
             kind INTEGER NOT NULL,\
             state_schema BLOB NOT NULL,\
             receipt_schema BLOB NOT NULL,\
             codec TEXT NOT NULL,\
             receipt BLOB NOT NULL,\
             receipt_digest BLOB NOT NULL\
         );"
    ))?;
    conn.execute(
        &format!(
            "INSERT INTO {METADATA_TABLE} \
             (singleton, format_version, application_id, initial_sequence, initial_state_schema, receipt_codec, receipt_retention, max_receipt_bytes) \
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        ),
        params![
            i64::from(EXTERNAL_FORMAT_VERSION),
            options.application_id,
            encode_u64(options.initial_sequence.0),
            encode_u64(options.initial_state_schema),
            receipt_codec,
            encode_u64(options.receipt_retention as u64),
            encode_u64(options.max_receipt_bytes as u64),
        ],
    )?;
    Ok(())
}

fn require_metadata_table(conn: &Connection) -> Result<(), ExternalError> {
    if !metadata_table_exists(conn)? {
        return Err(ExternalError::MissingMetadata);
    }
    Ok(())
}

fn metadata_table_exists(conn: &Connection) -> Result<bool, ExternalError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
            [METADATA_TABLE],
            |row| row.get(0),
        )
        .optional()?;
    Ok(exists.is_some())
}

fn load_state<C>(conn: &Connection) -> Result<StoredState<C>, ExternalError>
where
    C: Clone + Eq + DeserializeOwned,
{
    require_metadata_table(conn)?;
    type StateRow = (
        i64,
        String,
        Vec<u8>,
        Vec<u8>,
        String,
        Vec<u8>,
        Vec<u8>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<i64>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        Option<String>,
    );
    let row: StateRow = conn.query_row(
        &format!(
            "SELECT format_version, application_id, initial_sequence, initial_state_schema, \
             receipt_codec, receipt_retention, max_receipt_bytes, last_sequence, last_coordinate, \
             last_command_digest, last_kind, last_state_schema, last_receipt_schema, \
             last_receipt_digest, receipt_floor, snapshot_id \
             FROM {METADATA_TABLE} WHERE singleton=1"
        ),
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
                row.get(15)?,
            ))
        },
    )?;
    let format_version = u16::try_from(row.0)
        .map_err(|_| ExternalError::InvalidMetadata(format!("invalid format version {}", row.0)))?;
    if format_version != EXTERNAL_FORMAT_VERSION {
        return Err(ExternalError::UnsupportedFormat(format_version));
    }
    let initial_sequence = CommitSequence(decode_u64(&row.2, "initial_sequence")?);
    let initial_state_schema = decode_u64(&row.3, "initial_state_schema")?;
    let receipt_retention = usize::try_from(decode_u64(&row.5, "receipt_retention")?)
        .map_err(|_| ExternalError::InvalidMetadata("receipt_retention overflow".to_string()))?;
    let max_receipt_bytes = usize::try_from(decode_u64(&row.6, "max_receipt_bytes")?)
        .map_err(|_| ExternalError::InvalidMetadata("max_receipt_bytes overflow".to_string()))?;
    if !(1..=MAX_RECEIPT_RETENTION).contains(&receipt_retention) {
        return Err(ExternalError::InvalidMetadata(
            "receipt_retention is outside the supported range".to_string(),
        ));
    }
    if !(1..=MAX_RECEIPT_BYTES).contains(&max_receipt_bytes) {
        return Err(ExternalError::InvalidMetadata(
            "max_receipt_bytes is outside the supported range".to_string(),
        ));
    }

    let last_values_present = [
        row.7.is_some(),
        row.8.is_some(),
        row.9.is_some(),
        row.10.is_some(),
        row.11.is_some(),
        row.12.is_some(),
        row.13.is_some(),
        row.14.is_some(),
    ];
    if last_values_present.iter().any(|present| *present)
        && last_values_present.iter().any(|present| !*present)
    {
        return Err(ExternalError::InvalidMetadata(
            "partial last-applied metadata".to_string(),
        ));
    }
    let (last_applied, receipt_floor) = if last_values_present[0] {
        let sequence = CommitSequence(decode_u64(
            row.7.as_deref().expect("checked"),
            "last_sequence",
        )?);
        let coordinate = deserialize_exact(
            row.8.as_deref().expect("checked"),
            "last-applied coordinate",
        )?;
        let command_digest = decode_digest(row.9.as_deref().expect("checked"), "command_digest")?;
        let kind = ExternalEntryKind::from_i64(row.10.expect("checked"))?;
        let state_schema = decode_u64(row.11.as_deref().expect("checked"), "state_schema")?;
        let receipt_schema = decode_u64(row.12.as_deref().expect("checked"), "receipt_schema")?;
        let receipt_digest = decode_digest(row.13.as_deref().expect("checked"), "receipt_digest")?;
        let floor = CommitSequence(decode_u64(
            row.14.as_deref().expect("checked"),
            "receipt_floor",
        )?);
        (
            Some(ExternalApplied {
                commit: ExternalCommit {
                    sequence,
                    coordinate,
                    command_digest,
                    state_schema,
                    receipt_schema,
                },
                kind,
                receipt_digest,
            }),
            Some(floor),
        )
    } else {
        (None, None)
    };
    Ok(StoredState {
        application_id: row.1,
        initial_sequence,
        initial_state_schema,
        receipt_codec: row.4,
        receipt_retention,
        max_receipt_bytes,
        last_applied,
        receipt_floor,
        snapshot_id: row.15,
    })
}

fn load_receipt<C>(
    conn: &Connection,
    sequence: CommitSequence,
    max_receipt_bytes: usize,
) -> Result<Option<StoredReceipt<C>>, ExternalError>
where
    C: DeserializeOwned,
{
    let encoded_size: Option<i64> = conn
        .query_row(
            &format!("SELECT length(receipt) FROM {RECEIPTS_TABLE} WHERE sequence=?1"),
            params![encode_u64(sequence.0)],
            |row| row.get(0),
        )
        .optional()?;
    let Some(encoded_size) = encoded_size else {
        return Ok(None);
    };
    let encoded_size = usize::try_from(encoded_size).map_err(|_| {
        ExternalError::InvalidMetadata(format!(
            "receipt at sequence {sequence} has an invalid size"
        ))
    })?;
    if encoded_size > max_receipt_bytes {
        return Err(ExternalError::InvalidMetadata(format!(
            "receipt at sequence {sequence} exceeds configured maximum"
        )));
    }

    type ReceiptRow = (
        Vec<u8>,
        Vec<u8>,
        i64,
        Vec<u8>,
        Vec<u8>,
        String,
        Vec<u8>,
        Vec<u8>,
    );
    let row: Option<ReceiptRow> = conn
        .query_row(
            &format!(
                "SELECT coordinate, command_digest, kind, state_schema, receipt_schema, codec, receipt, receipt_digest \
                 FROM {RECEIPTS_TABLE} WHERE sequence=?1"
            ),
            params![encode_u64(sequence.0)],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .optional()?;
    row.map(|row| {
        let coordinate = deserialize_exact(&row.0, "receipt coordinate")?;
        let receipt_digest = decode_digest(&row.7, "receipt_digest")?;
        if Sha256Digest::of(&row.6) != receipt_digest {
            return Err(ExternalError::InvalidMetadata(format!(
                "receipt digest mismatch at sequence {sequence}"
            )));
        }
        Ok(StoredReceipt {
            commit: ExternalCommit {
                sequence,
                coordinate,
                command_digest: decode_digest(&row.1, "command_digest")?,
                state_schema: decode_u64(&row.3, "state_schema")?,
                receipt_schema: decode_u64(&row.4, "receipt_schema")?,
            },
            kind: ExternalEntryKind::from_i64(row.2)?,
            codec: row.5,
            receipt: row.6,
            receipt_digest,
        })
    })
    .transpose()
}

fn validate_configuration<C>(
    state: &StoredState<C>,
    options: &ExternalSqliteOptions,
    receipt_codec: &str,
) -> Result<(), ExternalError> {
    let mut mismatches = Vec::new();
    if state.application_id != options.application_id {
        mismatches.push("application_id");
    }
    if state.initial_sequence != options.initial_sequence {
        mismatches.push("initial_sequence");
    }
    if state.initial_state_schema != options.initial_state_schema {
        mismatches.push("initial_state_schema");
    }
    if state.receipt_codec != receipt_codec {
        mismatches.push("receipt_codec");
    }
    if state.receipt_retention != options.receipt_retention {
        mismatches.push("receipt_retention");
    }
    if state.max_receipt_bytes != options.max_receipt_bytes {
        mismatches.push("max_receipt_bytes");
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(ExternalError::ConfigurationMismatch(mismatches.join(", ")))
    }
}

fn validate_receipts<C, O>(conn: &Connection, state: &StoredState<C>) -> Result<(), ExternalError>
where
    C: Clone + Debug + Eq + DeserializeOwned,
    O: DeterministicSqliteOperation,
{
    match (&state.last_applied, state.receipt_floor) {
        (None, None) => {
            let count: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM {RECEIPTS_TABLE}"),
                [],
                |row| row.get(0),
            )?;
            if count != 0 {
                return Err(ExternalError::InvalidMetadata(
                    "receipts exist without a checkpoint".to_string(),
                ));
            }
        }
        (Some(last), Some(floor)) => {
            if floor > last.commit.sequence {
                return Err(ExternalError::InvalidMetadata(
                    "receipt floor exceeds checkpoint".to_string(),
                ));
            }
            let expected_count = last.commit.sequence.0 - floor.0 + 1;
            if expected_count > state.receipt_retention as u64 {
                return Err(ExternalError::InvalidMetadata(
                    "receipt suffix exceeds configured retention".to_string(),
                ));
            }
            let expected_floor = last
                .commit
                .sequence
                .0
                .saturating_sub(state.receipt_retention.saturating_sub(1) as u64)
                .max(state.initial_sequence.0);
            if floor.0 != expected_floor {
                return Err(ExternalError::InvalidMetadata(
                    "receipt floor does not match configured retention".to_string(),
                ));
            }
            let mut statement = conn.prepare(&format!(
                "SELECT sequence FROM {RECEIPTS_TABLE} ORDER BY sequence"
            ))?;
            let sequences = statement
                .query_map([], |row| row.get::<_, Vec<u8>>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            if sequences.len() as u64 != expected_count {
                return Err(ExternalError::InvalidMetadata(
                    "receipt suffix count mismatch".to_string(),
                ));
            }
            for (offset, sequence) in sequences.iter().enumerate() {
                let actual = decode_u64(sequence, "receipt sequence")?;
                if actual != floor.0 + offset as u64 {
                    return Err(ExternalError::InvalidMetadata(
                        "receipt suffix contains a gap".to_string(),
                    ));
                }
                let receipt =
                    load_receipt::<C>(conn, CommitSequence(actual), state.max_receipt_bytes)?
                        .ok_or_else(|| {
                            ExternalError::InvalidMetadata(format!(
                                "receipt at sequence {actual} disappeared during validation"
                            ))
                        })?;
                let expected_codec = match receipt.kind {
                    ExternalEntryKind::Operation => state.receipt_codec.as_str(),
                    ExternalEntryKind::Advance => ADVANCE_RECEIPT_CODEC,
                };
                if receipt.codec != expected_codec {
                    return Err(ExternalError::InvalidMetadata(format!(
                        "receipt codec mismatch at sequence {actual}"
                    )));
                }
                if receipt.receipt.len() > state.max_receipt_bytes {
                    return Err(ExternalError::InvalidMetadata(format!(
                        "receipt at sequence {actual} exceeds configured maximum"
                    )));
                }
                match receipt.kind {
                    ExternalEntryKind::Operation => {
                        O::decode_receipt(&receipt.receipt).map_err(|err| {
                            ExternalError::InvalidMetadata(format!(
                                "operation receipt at sequence {actual} is not decodable: {err}"
                            ))
                        })?;
                    }
                    ExternalEntryKind::Advance => {
                        let canonical = serialize(&())
                            .map_err(|err| ExternalError::Serialization(err.to_string()))?;
                        if receipt.receipt != canonical {
                            return Err(ExternalError::InvalidMetadata(format!(
                                "advance receipt at sequence {actual} is not canonical"
                            )));
                        }
                    }
                }
            }
            let newest = load_receipt::<C>(conn, last.commit.sequence, state.max_receipt_bytes)?
                .ok_or_else(|| {
                    ExternalError::InvalidMetadata("newest receipt is missing".to_string())
                })?;
            if newest.commit != last.commit
                || newest.kind != last.kind
                || newest.receipt_digest != last.receipt_digest
            {
                return Err(ExternalError::InvalidMetadata(
                    "newest receipt does not match checkpoint".to_string(),
                ));
            }
        }
        _ => {
            return Err(ExternalError::InvalidMetadata(
                "checkpoint and receipt floor presence mismatch".to_string(),
            ));
        }
    }
    Ok(())
}

fn persist_snapshot_id(conn: &Connection, snapshot_id: &str) -> Result<(), ExternalError> {
    conn.execute(
        &format!("UPDATE {METADATA_TABLE} SET snapshot_id=?1 WHERE singleton=1"),
        [snapshot_id],
    )?;
    Ok(())
}

fn validate_snapshot_file<C, O>(
    path: &Path,
    evidence: &ExternalSnapshotEvidence<C>,
) -> Result<(), ExternalError>
where
    C: Clone + Debug + Eq + Serialize + DeserializeOwned,
    O: DeterministicSqliteOperation,
{
    if evidence.format_version != EXTERNAL_FORMAT_VERSION {
        return Err(ExternalError::UnsupportedFormat(evidence.format_version));
    }
    if evidence.snapshot_format != EXTERNAL_SQLITE_SNAPSHOT_FORMAT {
        return Err(ExternalError::SnapshotMismatch(
            "unsupported SQLite snapshot format".to_string(),
        ));
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ExternalError::SnapshotMismatch(
            "snapshot path is not a regular file".to_string(),
        ));
    }
    if metadata.len() != evidence.sqlite_bytes {
        return Err(ExternalError::SnapshotMismatch(
            "file size differs from evidence".to_string(),
        ));
    }
    let digest = hash_file_blocking(path)?;
    if digest != evidence.sqlite_sha256 {
        return Err(ExternalError::SnapshotMismatch(
            "file SHA-256 differs from evidence".to_string(),
        ));
    }
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(30))?;
    let quick_check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(ExternalError::SnapshotMismatch(format!(
            "SQLite quick_check returned {quick_check}"
        )));
    }
    let page_size: u32 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
    if page_size != evidence.sqlite_page_size {
        return Err(ExternalError::SnapshotMismatch(
            "SQLite page size differs from evidence".to_string(),
        ));
    }
    let state = load_state::<C>(&conn)?;
    validate_receipts::<C, O>(&conn, &state)?;
    if state.snapshot_id.as_deref() != Some(evidence.snapshot_id.as_str()) {
        return Err(ExternalError::SnapshotMismatch(
            "embedded snapshot ID differs from evidence".to_string(),
        ));
    }
    let actual = state.snapshot_evidence(
        evidence.snapshot_id.clone(),
        evidence.sqlite_page_size,
        evidence.sqlite_bytes,
        evidence.sqlite_sha256,
    );
    if actual != *evidence {
        return Err(ExternalError::SnapshotMismatch(
            "embedded metadata differs from evidence".to_string(),
        ));
    }
    Ok(())
}

fn validate_snapshot_compatibility<C>(
    live: &StoredState<C>,
    evidence: &ExternalSnapshotEvidence<C>,
) -> Result<(), ExternalError> {
    if evidence.format_version != EXTERNAL_FORMAT_VERSION {
        return Err(ExternalError::UnsupportedFormat(evidence.format_version));
    }
    if evidence.snapshot_format != EXTERNAL_SQLITE_SNAPSHOT_FORMAT {
        return Err(ExternalError::SnapshotMismatch(
            "unsupported SQLite snapshot format".to_string(),
        ));
    }
    let mut mismatches = Vec::new();
    if evidence.application_id != live.application_id {
        mismatches.push("application_id");
    }
    if evidence.initial_sequence != live.initial_sequence {
        mismatches.push("initial_sequence");
    }
    if evidence.initial_state_schema != live.initial_state_schema {
        mismatches.push("initial_state_schema");
    }
    if evidence.receipt_codec != live.receipt_codec {
        mismatches.push("receipt_codec");
    }
    if evidence.receipt_retention != live.receipt_retention as u64 {
        mismatches.push("receipt_retention");
    }
    if evidence.max_receipt_bytes != live.max_receipt_bytes as u64 {
        mismatches.push("max_receipt_bytes");
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(ExternalError::SnapshotMismatch(format!(
            "incompatible {}",
            mismatches.join(", ")
        )))
    }
}

fn reject_stale_snapshot<C: Eq>(
    live: Option<&ExternalApplied<C>>,
    incoming: Option<&ExternalApplied<C>>,
) -> Result<(), ExternalError> {
    match (live, incoming) {
        (Some(_), None) => Err(ExternalError::StaleSnapshot),
        (Some(live), Some(incoming)) if incoming.commit.sequence < live.commit.sequence => {
            Err(ExternalError::StaleSnapshot)
        }
        (Some(live), Some(incoming))
            if incoming.commit.sequence == live.commit.sequence && incoming != live =>
        {
            Err(ExternalError::CommitConflict(incoming.commit.sequence))
        }
        _ => Ok(()),
    }
}

fn requires_projection_rebuild(error: &ExternalError) -> bool {
    matches!(
        error,
        ExternalError::MissingMetadata
            | ExternalError::InvalidMetadata(_)
            | ExternalError::UnsupportedFormat(_)
            | ExternalError::Serialization(_)
            | ExternalError::Sqlite(_)
    )
}

fn create_external_snapshot(conn: &Connection, path: &Path) -> Result<(), ExternalError> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push("~");
    let temporary = PathBuf::from(temporary);
    let _ = std::fs::remove_file(&temporary);
    if let Err(err) = conn.backup("main", &temporary, None) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err.into());
    }
    if let Err(err) = sync_file_and_parent_blocking(&temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }
    if let Err(err) = std::fs::hard_link(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return if err.kind() == std::io::ErrorKind::AlreadyExists {
            Err(ExternalError::SnapshotDestinationExists(path.to_owned()))
        } else {
            Err(err.into())
        };
    }
    sync_file_and_parent_blocking(path)?;
    std::fs::remove_file(&temporary)?;
    if let Some(parent) = temporary.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

async fn inspect_snapshot(path: PathBuf) -> Result<(u32, u64, Sha256Digest), ExternalError> {
    task::spawn_blocking(move || {
        let bytes = std::fs::metadata(&path)?.len();
        let digest = hash_file_blocking(&path)?;
        let conn = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let page_size: u32 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
        Ok((page_size, bytes, digest))
    })
    .await?
}

fn hash_file_blocking(path: &Path) -> Result<Sha256Digest, ExternalError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Sha256Digest(hasher.finalize().into()))
}

async fn sync_file(path: PathBuf) -> Result<(), ExternalError> {
    task::spawn_blocking(move || File::open(path)?.sync_all()).await??;
    Ok(())
}

async fn sync_file_and_parent(path: PathBuf) -> Result<(), ExternalError> {
    task::spawn_blocking(move || sync_file_and_parent_blocking(&path)).await??;
    Ok(())
}

async fn remove_projection_files(db_dir: &Path, filename: &str) -> Result<(), ExternalError> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let path = db_dir.join(format!("{filename}{suffix}"));
        match fs::remove_file(path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    sync_parent_directory(db_dir)?;
    Ok(())
}

fn remove_dirty_marker(path: &Path) -> Result<(), ExternalError> {
    std::fs::remove_file(path)?;
    if let Some(parent) = path.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

fn write_dirty_marker(path: &Path, state: &[u8]) -> Result<(), ExternalError> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(state)?;
    file.sync_all()?;
    if let Some(parent) = path.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

fn finalize_projection(conn: &Connection) -> Result<(), ExternalError> {
    conn.pragma_update(None, "synchronous", "FULL")?;
    let (busy, _, _): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    if busy != 0 {
        return Err(ExternalError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("final WAL checkpoint remained busy".to_string()),
        )));
    }
    Ok(())
}

fn sync_parent_directory(path: &Path) -> Result<(), ExternalError> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    Ok(())
}

fn sync_file_and_parent_blocking(path: &Path) -> Result<(), ExternalError> {
    File::open(path)?.sync_all()?;
    if let Some(parent) = path.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

fn encode_u64(value: u64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

fn decode_u64(bytes: &[u8], field: &str) -> Result<u64, ExternalError> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| {
        ExternalError::InvalidMetadata(format!("{field} must contain exactly 8 bytes"))
    })?;
    Ok(u64::from_be_bytes(bytes))
}

fn decode_digest(bytes: &[u8], field: &str) -> Result<Sha256Digest, ExternalError> {
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
        ExternalError::InvalidMetadata(format!("{field} must contain exactly 32 bytes"))
    })?;
    Ok(Sha256Digest(bytes))
}

fn deserialize_exact<T: DeserializeOwned>(bytes: &[u8], field: &str) -> Result<T, ExternalError> {
    let (value, consumed) =
        bincode::serde::decode_from_slice::<T, _>(bytes, bincode::config::legacy())
            .map_err(|err| ExternalError::Serialization(err.to_string()))?;
    if consumed != bytes.len() {
        return Err(ExternalError::InvalidMetadata(format!(
            "{field} contains trailing bytes"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

    #[derive(Debug)]
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let id = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "hiqlite-external-{name}-{}-{id}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
    struct Coordinate {
        term: u64,
        node: u64,
        index: u64,
    }

    #[derive(Debug)]
    enum TestOperation {
        Create,
        CreateImplicit,
        Insert { id: i64, value: &'static str },
        InsertImplicit { value: &'static str },
        DeleteImplicit { rowid: i64 },
        InsertThenFail { id: i64 },
        Reject(&'static str),
        TamperMetadata,
    }

    #[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
    enum TestReceipt {
        Created,
        Inserted(i64),
        Deleted(i64),
        Rejected(String),
    }

    #[derive(Debug, Clone, Eq, PartialEq)]
    enum TestOperationError {
        Forced,
        Sqlite(String),
    }

    impl Display for TestOperationError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Forced => write!(f, "forced operation failure"),
                Self::Sqlite(err) => write!(f, "SQLite operation failure: {err}"),
            }
        }
    }

    impl DeterministicSqliteOperation for TestOperation {
        type Output = TestReceipt;
        type Error = TestOperationError;

        const RECEIPT_CODEC: &'static str = "hiqlite-test-receipt-v1";

        fn apply(self, transaction: &Transaction<'_>) -> Result<Self::Output, Self::Error> {
            match self {
                Self::Create => {
                    transaction
                        .execute(
                            "CREATE TABLE items (id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
                            [],
                        )
                        .unwrap();
                    Ok(TestReceipt::Created)
                }
                Self::CreateImplicit => {
                    transaction
                        .execute("CREATE TABLE implicit_ids (value TEXT NOT NULL)", [])
                        .map_err(|err| TestOperationError::Sqlite(err.to_string()))?;
                    Ok(TestReceipt::Created)
                }
                Self::Insert { id, value } => {
                    transaction
                        .execute(
                            "INSERT INTO items (id, value) VALUES (?1, ?2)",
                            params![id, value],
                        )
                        .unwrap();
                    Ok(TestReceipt::Inserted(id))
                }
                Self::InsertImplicit { value } => {
                    transaction
                        .execute("INSERT INTO implicit_ids (value) VALUES (?1)", [value])
                        .map_err(|err| TestOperationError::Sqlite(err.to_string()))?;
                    Ok(TestReceipt::Inserted(transaction.last_insert_rowid()))
                }
                Self::DeleteImplicit { rowid } => {
                    transaction
                        .execute("DELETE FROM implicit_ids WHERE rowid=?1", [rowid])
                        .map_err(|err| TestOperationError::Sqlite(err.to_string()))?;
                    Ok(TestReceipt::Deleted(rowid))
                }
                Self::InsertThenFail { id } => {
                    transaction
                        .execute(
                            "INSERT INTO items (id, value) VALUES (?1, 'rolled-back')",
                            [id],
                        )
                        .unwrap();
                    Err(TestOperationError::Forced)
                }
                Self::Reject(reason) => Ok(TestReceipt::Rejected(reason.to_string())),
                Self::TamperMetadata => transaction
                    .execute("DELETE FROM _hiqlite_external_state", [])
                    .map(|_| TestReceipt::Created)
                    .map_err(|err| TestOperationError::Sqlite(err.to_string())),
            }
        }

        fn encode_receipt(output: &Self::Output) -> Result<Vec<u8>, String> {
            let mut bytes = b"test-receipt-v1\0".to_vec();
            bytes.extend(serialize(output).map_err(|err| err.to_string())?);
            Ok(bytes)
        }

        fn decode_receipt(bytes: &[u8]) -> Result<Self::Output, String> {
            let bytes = bytes
                .strip_prefix(b"test-receipt-v1\0")
                .ok_or_else(|| "invalid test receipt prefix".to_string())?;
            deserialize_exact(bytes, "test receipt").map_err(|err| err.to_string())
        }
    }

    type TestEngine = ExternalSqlite<Coordinate, TestOperation>;

    fn options(dir: &TestDir) -> ExternalSqliteOptions {
        let mut options = ExternalSqliteOptions::new(&dir.0, "workspace-test-v1");
        options.read_pool_size = 2;
        options.receipt_retention = 2;
        options
    }

    fn commit(sequence: u64, command: &str) -> ExternalCommit<Coordinate> {
        ExternalCommit {
            sequence: CommitSequence(sequence),
            coordinate: Coordinate {
                term: 7,
                node: 3,
                index: sequence + 100,
            },
            command_digest: Sha256Digest::of(command),
            state_schema: 1,
            receipt_schema: 1,
        }
    }

    async fn count_items(engine: &TestEngine) -> i64 {
        engine
            .read(|conn| conn.query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0)))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn dense_frontier_exact_retries_conflicts_and_rollback() {
        let dir = TestDir::new("frontier");
        let options = options(&dir);
        let engine = TestEngine::open(options.clone()).await.unwrap();

        let err = engine
            .apply_committed(commit(0, "before-initial"), TestOperation::Create)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ExternalApplyError::Engine(ExternalError::SequenceBeforeInitial { .. })
        ));

        assert_eq!(
            engine
                .apply_committed(commit(1, "create-v1"), TestOperation::Create)
                .await
                .unwrap(),
            ApplyOutcome::Applied(TestReceipt::Created)
        );
        assert_eq!(
            engine
                .advance_committed(commit(2, "global-membership-entry"))
                .await
                .unwrap(),
            ApplyOutcome::Applied(())
        );
        let insert = commit(3, "insert-1-v1");
        assert_eq!(
            engine
                .apply_committed(
                    insert.clone(),
                    TestOperation::Insert {
                        id: 1,
                        value: "one",
                    },
                )
                .await
                .unwrap(),
            ApplyOutcome::Applied(TestReceipt::Inserted(1))
        );

        // The exact retry returns the stored receipt and never executes the supplied value.
        assert_eq!(
            engine
                .apply_committed(insert.clone(), TestOperation::InsertThenFail { id: 99 })
                .await
                .unwrap(),
            ApplyOutcome::Recovered(TestReceipt::Inserted(1))
        );
        assert_eq!(count_items(&engine).await, 1);

        let mut conflicting = insert.clone();
        conflicting.command_digest = Sha256Digest::of("different-canonical-command");
        let err = engine
            .apply_committed(conflicting, TestOperation::Reject("unused"))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ExternalApplyError::Engine(ExternalError::CommitConflict(CommitSequence(3)))
        ));
        assert!(matches!(
            engine.advance_committed(insert.clone()).await.unwrap_err(),
            ExternalError::CommitConflict(CommitSequence(3))
        ));

        let err = engine
            .apply_committed(
                commit(5, "gap"),
                TestOperation::Insert {
                    id: 5,
                    value: "gap",
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ExternalApplyError::Engine(ExternalError::SequenceGap {
                expected: CommitSequence(4),
                received: CommitSequence(5)
            })
        ));

        let rejected = commit(4, "deterministic-rejection-v1");
        let err = engine
            .apply_committed(rejected.clone(), TestOperation::InsertThenFail { id: 4 })
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ExternalApplyError::Operation(TestOperationError::Forced)
        ));
        assert_eq!(
            engine
                .last_applied()
                .await
                .unwrap()
                .unwrap()
                .commit
                .sequence,
            CommitSequence(3)
        );
        assert_eq!(count_items(&engine).await, 1);

        assert_eq!(
            engine
                .apply_committed(rejected.clone(), TestOperation::Reject("denied"))
                .await
                .unwrap(),
            ApplyOutcome::Applied(TestReceipt::Rejected("denied".to_string()))
        );
        assert_eq!(
            engine
                .apply_committed(rejected.clone(), TestOperation::InsertThenFail { id: 44 })
                .await
                .unwrap(),
            ApplyOutcome::Recovered(TestReceipt::Rejected("denied".to_string()))
        );

        // Retention=2 now covers [3, 4], so the explicit advance at 2 is unavailable.
        assert!(matches!(
            engine
                .advance_committed(commit(2, "global-membership-entry"))
                .await
                .unwrap_err(),
            ExternalError::ReceiptUnavailable(CommitSequence(2))
        ));

        engine.shutdown().await.unwrap();
        let reopened = TestEngine::open(options).await.unwrap();
        assert_eq!(
            reopened
                .apply_committed(rejected, TestOperation::InsertThenFail { id: 45 })
                .await
                .unwrap(),
            ApplyOutcome::Recovered(TestReceipt::Rejected("denied".to_string()))
        );
        assert_eq!(count_items(&reopened).await, 1);
        reopened.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn snapshot_evidence_restore_receipts_and_staleness() {
        let source_dir = TestDir::new("snapshot-source");
        let source_options = options(&source_dir);
        let source = TestEngine::open(source_options).await.unwrap();
        source
            .apply_committed(commit(1, "create-v1"), TestOperation::Create)
            .await
            .unwrap();
        let insert = commit(2, "insert-1-v1");
        source
            .apply_committed(
                insert.clone(),
                TestOperation::Insert {
                    id: 1,
                    value: "one",
                },
            )
            .await
            .unwrap();
        let snapshot = source.build_snapshot().await.unwrap();
        assert_eq!(snapshot.evidence.format_version, EXTERNAL_FORMAT_VERSION);
        assert_eq!(snapshot.evidence.application_id, "workspace-test-v1");
        assert_eq!(
            snapshot.evidence.receipt_codec,
            TestOperation::RECEIPT_CODEC
        );
        assert_eq!(
            snapshot
                .evidence
                .checkpoint
                .as_ref()
                .unwrap()
                .commit
                .sequence,
            CommitSequence(2)
        );
        assert!(snapshot.evidence.sqlite_bytes > 0);
        TestEngine::validate_snapshot(&snapshot).await.unwrap();
        let caller_path = source_dir.0.join("caller-owned.snapshot");
        let caller_snapshot = source.build_snapshot_into(&caller_path).await.unwrap();
        assert_eq!(caller_snapshot.path, caller_path);
        assert!(matches!(
            source.build_snapshot_into(&caller_path).await.unwrap_err(),
            ExternalError::SnapshotDestinationExists(path) if path == caller_path
        ));
        source.shutdown().await.unwrap();

        let corrupt_path = source_dir.0.join("corrupt-receipt-window.sqlite");
        std::fs::copy(&snapshot.path, &corrupt_path).unwrap();
        let corrupt_conn = Connection::open(&corrupt_path).unwrap();
        corrupt_conn
            .execute(
                &format!(
                    "DELETE FROM {RECEIPTS_TABLE} WHERE sequence=(SELECT MIN(sequence) FROM {RECEIPTS_TABLE})"
                ),
                [],
            )
            .unwrap();
        drop(corrupt_conn);
        let (page_size, bytes, digest) = inspect_snapshot(corrupt_path.clone()).await.unwrap();
        let mut corrupt_receipts = snapshot.clone();
        corrupt_receipts.path = corrupt_path;
        corrupt_receipts.evidence.sqlite_page_size = page_size;
        corrupt_receipts.evidence.sqlite_bytes = bytes;
        corrupt_receipts.evidence.sqlite_sha256 = digest;

        let target_dir = TestDir::new("snapshot-target");
        let target_options = options(&target_dir);
        let target = TestEngine::open(target_options).await.unwrap();

        assert!(matches!(
            target.install_snapshot(corrupt_receipts).await.unwrap_err(),
            ExternalError::InvalidMetadata(_)
        ));

        let mut wrong_format = snapshot.clone();
        wrong_format.evidence.format_version += 1;
        assert!(matches!(
            target.install_snapshot(wrong_format).await.unwrap_err(),
            ExternalError::UnsupportedFormat(_)
        ));
        let mut wrong_application = snapshot.clone();
        wrong_application.evidence.application_id = "another-app".to_string();
        assert!(matches!(
            target
                .install_snapshot(wrong_application)
                .await
                .unwrap_err(),
            ExternalError::SnapshotMismatch(_)
        ));
        let mut wrong_hash = snapshot.clone();
        wrong_hash.evidence.sqlite_sha256 = Sha256Digest::of("wrong");
        assert!(matches!(
            target.install_snapshot(wrong_hash).await.unwrap_err(),
            ExternalError::SnapshotMismatch(_)
        ));

        assert_eq!(
            target.install_snapshot(snapshot.clone()).await.unwrap(),
            snapshot.evidence.clone()
        );
        assert_eq!(count_items(&target).await, 1);
        assert_eq!(
            target
                .apply_committed(insert, TestOperation::InsertThenFail { id: 99 })
                .await
                .unwrap(),
            ApplyOutcome::Recovered(TestReceipt::Inserted(1))
        );
        target
            .apply_committed(
                commit(3, "insert-2-v1"),
                TestOperation::Insert {
                    id: 2,
                    value: "two",
                },
            )
            .await
            .unwrap();
        assert_eq!(count_items(&target).await, 2);
        assert!(matches!(
            target.install_snapshot(snapshot).await.unwrap_err(),
            ExternalError::StaleSnapshot
        ));
        target.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn adoption_is_explicit_and_preserves_existing_application_state() {
        let dir = TestDir::new("adopt-existing");
        let options = options(&dir);
        let db_dir = dir.0.join("external_state_machine/db");
        std::fs::create_dir_all(&db_dir).unwrap();
        let db_path = db_dir.join("external.sqlite");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE adopted (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO adopted(id, value) VALUES (1, 'kept');",
        )
        .unwrap();
        drop(conn);

        let engine = TestEngine::adopt_existing_projection(options.clone())
            .await
            .unwrap();
        assert_eq!(
            engine
                .read(|conn| {
                    conn.query_row("SELECT value FROM adopted WHERE id=1", [], |row| {
                        row.get::<_, String>(0)
                    })
                })
                .await
                .unwrap(),
            "kept"
        );
        assert!(engine.last_applied().await.unwrap().is_none());
        engine.shutdown().await.unwrap();

        assert!(matches!(
            TestEngine::adopt_existing_projection(options.clone())
                .await
                .err()
                .unwrap(),
            ExternalError::AdoptionNotAllowed(_)
        ));
        TestEngine::open(options)
            .await
            .unwrap()
            .shutdown()
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn snapshot_restore_waits_for_reads_and_replaces_the_pool() {
        let source_dir = TestDir::new("restore-read-source");
        let source = TestEngine::open(options(&source_dir)).await.unwrap();
        source
            .apply_committed(commit(1, "create-v1"), TestOperation::Create)
            .await
            .unwrap();
        source
            .apply_committed(
                commit(2, "insert-v1"),
                TestOperation::Insert {
                    id: 1,
                    value: "restored",
                },
            )
            .await
            .unwrap();
        let snapshot = source.build_snapshot().await.unwrap();
        source.shutdown().await.unwrap();

        let target_dir = TestDir::new("restore-read-target");
        let target = std::sync::Arc::new(TestEngine::open(options(&target_dir)).await.unwrap());
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let reader = std::sync::Arc::clone(&target);
        let read = tokio::spawn(async move {
            reader
                .read(move |conn| {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                })
                .await
        });
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("read closure should start");

        let installer = std::sync::Arc::clone(&target);
        let mut install = tokio::spawn(async move { installer.install_snapshot(snapshot).await });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut install)
                .await
                .is_err()
        );
        release_tx.send(()).unwrap();
        assert_eq!(read.await.unwrap().unwrap(), 1);
        install.await.unwrap().unwrap();
        assert_eq!(count_items(&target).await, 1);

        std::sync::Arc::try_unwrap(target)
            .ok()
            .expect("all test handles should be dropped")
            .shutdown()
            .await
            .unwrap();
    }

    #[test]
    fn external_codec_rejects_trailing_bytes() {
        let mut bytes = TestOperation::encode_receipt(&TestReceipt::Created).unwrap();
        assert_eq!(
            TestOperation::decode_receipt(&bytes).unwrap(),
            TestReceipt::Created
        );
        bytes.push(0);
        assert!(
            TestOperation::decode_receipt(&bytes)
                .unwrap_err()
                .contains("trailing bytes")
        );
        assert!(TestOperation::decode_receipt(b"wrong-prefix").is_err());
    }

    #[tokio::test]
    async fn durability_is_explicit_and_unclean_replayable_off_fails_closed() {
        let dir = TestDir::new("durability");
        let full_options = options(&dir);
        assert_eq!(full_options.durability, ExternalDurability::Full);
        let full = TestEngine::open(full_options.clone()).await.unwrap();
        assert_eq!(full.synchronous_for_test().await.unwrap(), 2);
        assert!(matches!(
            TestEngine::open(full_options.clone()).await.err().unwrap(),
            ExternalError::Locked(_)
        ));

        let mut off_options = full_options.clone();
        off_options.durability = ExternalDurability::ReplayableOff;
        assert!(matches!(
            TestEngine::open(off_options.clone()).await.err().unwrap(),
            ExternalError::Locked(_)
        ));
        assert!(
            dir.0
                .join("external_state_machine/db/external.sqlite")
                .exists()
        );
        full.shutdown().await.unwrap();

        let mut normal_options = full_options.clone();
        normal_options.durability = ExternalDurability::Normal;
        let normal = TestEngine::open(normal_options.clone()).await.unwrap();
        assert_eq!(normal.synchronous_for_test().await.unwrap(), 1);
        normal.shutdown().await.unwrap();

        let off = TestEngine::open(off_options.clone()).await.unwrap();
        assert_eq!(off.synchronous_for_test().await.unwrap(), 0);
        off.shutdown().await.unwrap();

        let dirty = dir.0.join("external_state_machine/dirty");
        write_dirty_marker(&dirty, DIRTY_OFF).unwrap();
        assert!(matches!(
            TestEngine::open(off_options.clone()).await.err().unwrap(),
            ExternalError::RebuildRequired(_)
        ));
        let rebuilt = TestEngine::rebuild_projection(off_options.clone())
            .await
            .unwrap();
        assert!(rebuilt.last_applied().await.unwrap().is_none());
        rebuilt.shutdown().await.unwrap();
        assert!(matches!(
            TestEngine::rebuild_projection(off_options.clone())
                .await
                .err()
                .unwrap(),
            ExternalError::RebuildNotRequired(_)
        ));

        write_dirty_marker(&dirty, DIRTY_OFF).unwrap();
        assert!(matches!(
            TestEngine::open(full_options.clone()).await.err().unwrap(),
            ExternalError::RebuildRequired(_)
        ));
        TestEngine::rebuild_projection(full_options.clone())
            .await
            .unwrap()
            .shutdown()
            .await
            .unwrap();

        write_dirty_marker(&dirty, DIRTY_POISONED).unwrap();
        assert!(matches!(
            TestEngine::open(normal_options.clone())
                .await
                .err()
                .unwrap(),
            ExternalError::RebuildRequired(_)
        ));
        TestEngine::rebuild_projection(normal_options)
            .await
            .unwrap()
            .shutdown()
            .await
            .unwrap();

        write_dirty_marker(&dirty, DIRTY_FULL).unwrap();
        TestEngine::open(full_options)
            .await
            .unwrap()
            .shutdown()
            .await
            .unwrap();

        assert_eq!(
            Sha256Digest::of("abc").0,
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    #[tokio::test]
    async fn typed_write_boundary_cannot_be_bypassed() {
        let dir = TestDir::new("write-boundary");
        let engine = TestEngine::open(options(&dir)).await.unwrap();
        engine
            .apply_committed(commit(1, "create-v1"), TestOperation::Create)
            .await
            .unwrap();

        let tamper = engine
            .apply_committed(commit(2, "tamper-v1"), TestOperation::TamperMetadata)
            .await
            .unwrap_err();
        assert!(matches!(
            tamper,
            ExternalApplyError::Operation(TestOperationError::Sqlite(_))
        ));
        assert_eq!(
            engine
                .last_applied()
                .await
                .unwrap()
                .unwrap()
                .commit
                .sequence,
            CommitSequence(1)
        );

        let bypass = engine
            .read(|conn| {
                conn.pragma_update(None, "query_only", false)?;
                conn.execute("INSERT INTO items (id, value) VALUES (99, 'bypass')", [])?;
                Ok(())
            })
            .await
            .unwrap_err();
        assert!(matches!(bypass, ExternalError::Sqlite(_)));
        assert_eq!(count_items(&engine).await, 0);

        engine
            .apply_committed(
                commit(2, "insert-v1"),
                TestOperation::Insert {
                    id: 1,
                    value: "one",
                },
            )
            .await
            .unwrap();
        assert_eq!(count_items(&engine).await, 1);
        engine.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn online_backup_snapshot_preserves_implicit_rowids() {
        let source_dir = TestDir::new("rowid-source");
        let source = TestEngine::open(options(&source_dir)).await.unwrap();
        source
            .apply_committed(
                commit(1, "create-implicit-v1"),
                TestOperation::CreateImplicit,
            )
            .await
            .unwrap();
        source
            .apply_committed(
                commit(2, "insert-implicit-first-v1"),
                TestOperation::InsertImplicit { value: "first" },
            )
            .await
            .unwrap();
        source
            .apply_committed(
                commit(3, "insert-implicit-second-v1"),
                TestOperation::InsertImplicit { value: "second" },
            )
            .await
            .unwrap();
        source
            .apply_committed(
                commit(4, "delete-implicit-first-v1"),
                TestOperation::DeleteImplicit { rowid: 1 },
            )
            .await
            .unwrap();
        let snapshot = source.build_snapshot().await.unwrap();
        assert_eq!(
            snapshot.evidence.snapshot_format,
            EXTERNAL_SQLITE_SNAPSHOT_FORMAT
        );
        assert!(snapshot.evidence.sqlite_page_size > 0);
        let evidence = &snapshot.evidence;
        assert_eq!(
            ExternalSnapshotEvidence::from_manifest(
                evidence.format_version,
                evidence.snapshot_format.clone(),
                evidence.snapshot_id.clone(),
                evidence.application_id.clone(),
                evidence.initial_sequence,
                evidence.initial_state_schema,
                evidence.receipt_codec.clone(),
                evidence.checkpoint.clone(),
                evidence.receipt_floor,
                evidence.receipt_retention,
                evidence.max_receipt_bytes,
                evidence.sqlite_page_size,
                evidence.sqlite_bytes,
                evidence.sqlite_sha256,
            ),
            evidence.clone()
        );
        source.shutdown().await.unwrap();

        let target_dir = TestDir::new("rowid-target");
        let target = TestEngine::open(options(&target_dir)).await.unwrap();
        target.install_snapshot(snapshot).await.unwrap();
        let rowid = target
            .read(|conn| {
                conn.query_row("SELECT rowid FROM implicit_ids", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .await
            .unwrap();
        assert_eq!(rowid, 2);
        target.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn oversized_receipt_rolls_back_operation_and_frontier() {
        let dir = TestDir::new("receipt-limit");
        let mut small = options(&dir);
        small.max_receipt_bytes = 32;
        let engine = TestEngine::open(small.clone()).await.unwrap();
        engine
            .apply_committed(commit(1, "create-v1"), TestOperation::Create)
            .await
            .unwrap();
        let err = engine
            .apply_committed(
                commit(2, "large-v1"),
                TestOperation::Reject(
                    "this receipt is deliberately much larger than thirty-two bytes",
                ),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ExternalApplyError::Engine(ExternalError::ReceiptTooLarge { .. })
        ));
        assert_eq!(
            engine
                .last_applied()
                .await
                .unwrap()
                .unwrap()
                .commit
                .sequence,
            CommitSequence(1)
        );
        engine.shutdown().await.unwrap();

        let engine = TestEngine::open(small).await.unwrap();
        assert_eq!(
            engine
                .apply_committed(
                    commit(2, "insert-v1"),
                    TestOperation::Insert {
                        id: 1,
                        value: "one",
                    },
                )
                .await
                .unwrap(),
            ApplyOutcome::Applied(TestReceipt::Inserted(1))
        );
        engine.shutdown().await.unwrap();
    }
}
