//! openraft 0.10's storage test suite, run over the cache group's real stores: the `hiqlite-wal`
//! log store and the in-memory one, each with the memory state machine, built on disk exactly the
//! way `start_raft_cache` builds them. It is ~50 storage tests, one fresh store pair per `build()`
//! call, and it includes the startup consistency checks that 0.10 added (a log store purged past
//! its last log id, a state machine applied past the log tail, a reader returning other indexes
//! during re-apply chunks) plus a snapshot transfer at the end.
//!
//! Only the cache group has the memory state machine, so the module is gated to that feature.
use crate::CacheVariants;
use crate::store::logs::logs_dir_cache;
use crate::store::logs::memory::LogStoreMemory;
use crate::store::state_machine::memory::TypeConfigCache;
use crate::store::state_machine::memory::state_machine::StateMachineMemory;
use hiqlite_wal::{LogStore, LogSync};
use openraft::StorageError;
use openraft::storage::{RaftLogStorage, RaftStateMachine};
use openraft::testing::log::{StoreBuilder, Suite};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 1 MB instead of the default 2 MB; the suite writes a few hundred entries at most.
const WAL_SIZE: u32 = 1024 * 1024;

#[derive(Debug)]
enum TestCache {
    One,
}

impl CacheVariants for TestCache {
    fn hiqlite_cache_index(&self) -> usize {
        0
    }

    fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
        &[(0, "One")]
    }
}

type Wal = LogStore<TypeConfigCache>;

type Mem = LogStoreMemory;

type Sm = Arc<StateMachineMemory>;

/// Builds one fresh (log store, memory state machine) pair per suite test, the state machine on
/// disk under `base`.
struct Builder {
    base: PathBuf,
    next: AtomicUsize,
}

impl Builder {
    /// One fresh data dir with a state machine in it, one per suite test.
    async fn sm(&self) -> (String, Sm) {
        let dir = self.base.join(format!(
            "test_{}",
            self.next.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let base = dir.to_str().unwrap().to_string();

        let sm = Arc::new(
            StateMachineMemory::new::<TestCache>(&base, false)
                .await
                .unwrap(),
        );

        (base, sm)
    }
}

impl StoreBuilder<TypeConfigCache, Wal, Sm, ()> for Builder {
    async fn build(&self) -> Result<((), Wal, Sm), StorageError<TypeConfigCache>> {
        let (base, sm) = self.sm().await;

        let wal =
            LogStore::<TypeConfigCache>::start(logs_dir_cache(&base), LogSync::Immediate, WAL_SIZE)
                .await
                .unwrap();

        Ok(((), wal, sm))
    }
}

impl StoreBuilder<TypeConfigCache, Mem, Sm, ()> for Builder {
    async fn build(&self) -> Result<((), Mem, Sm), StorageError<TypeConfigCache>> {
        let (_base, sm) = self.sm().await;

        Ok(((), Mem::new(), sm))
    }
}

/// Runs the whole suite against a builder for one log store impl, in a fresh temp dir so the
/// suites can run in parallel.
async fn run_suite<L>(dir_name: &str)
where
    L: RaftLogStorage<TypeConfigCache>,
    Sm: RaftStateMachine<TypeConfigCache>,
    Builder: StoreBuilder<TypeConfigCache, L, Sm, ()>,
{
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init()
        .ok();

    let base = TmpDir::new(dir_name);

    let builder = Builder {
        base: base.path().to_path_buf(),
        next: AtomicUsize::new(0),
    };

    Suite::<TypeConfigCache, L, Sm, Builder, ()>::test_all(builder)
        .await
        .unwrap();
}

/// A fresh temp dir for one suite run. It is emptied on creation and removed on drop, so a failing
/// test cleans up too. The name carries the process id, because a fixed name would make two runs of
/// the same binary - a CI matrix on one runner, or a `cargo test` next to a `cargo msrv test` -
/// delete each other's directories while they are still in use.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn openraft_log_storage_suite_wal() {
    run_suite::<Wal>("hiqlite_openraft_log_suite_wal").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn openraft_log_storage_suite_memory() {
    run_suite::<Mem>("hiqlite_openraft_log_suite_memory").await;
}
