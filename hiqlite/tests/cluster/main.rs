use crate::self_heal::test_self_healing;
use futures_util::future::join_all;
use hiqlite::cache_idx::CacheIndex;
use hiqlite::Error;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::time::Duration;
use std::{env, fs, process};
use tokio::time;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

mod backup;
mod backup_restore;
mod batch;
mod check;
mod execute_query;
mod migration;
mod self_heal;
mod start;
mod transaction;

mod cache;
mod dlock;
mod listen_notify;
mod remote_only;
mod type_conversions;

pub const TEST_DATA_DIR: &str = "tests/data_test";

#[macro_export]
macro_rules! params {
    ( $( $param:expr ),* ) => {
        {
            #[allow(unused_mut)]
            let mut params = Vec::with_capacity(2);
            $(
                params.push(hiqlite::Param::from($param));
            )*
            params
        }
    };
}

#[derive(Debug, Serialize, Deserialize, strum::EnumIter)]
enum Cache {
    One,
    Two,
    Three,
}

impl CacheIndex for Cache {
    fn to_usize(self) -> usize {
        self as usize
    }
}

#[tokio::test(flavor = "multi_thread")]
// #[tokio::test(start_paused = true)]
async fn test_cluster() {
    let console_layer = console_subscriber::spawn();

    set_panic_hook();

    // always start clean
    unsafe { env::remove_var("HQL_BACKUP_RESTORE") };
    let _ = fs::remove_dir_all(TEST_DATA_DIR);

    dotenvy::from_filename("../config").ok();

    let tracing_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_filter(EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(tracing_layer)
        .init();

    match exec_tests().await {
        Ok(_) => {
            log("All tests successful");
            // TODO sometimes the test gets stuck here
            process::exit(0);
        }
        Err(err) => {
            panic!("\n!!!\n{}\n!!!\n", err);
        }
    }
}

async fn exec_tests() -> Result<(), Error> {
    log("Starting cluster");
    let (client_1, client_2, client_3) = start::start_test_cluster().await?;
    log("Cluster has been started");

    start::wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy");

    debug(&client_1.metrics_db().await?);
    debug(&client_1.metrics_cache().await?);

    log("Starting migration tests");
    migration::test_migrations(&client_1, &client_2, &client_3).await?;
    log("Migration tests finished");

    log("Starting data insertion and query tests");
    execute_query::test_execute_query(&client_1, &client_2, &client_3).await?;
    log("Basic query tests finished");

    log("Starting Transaction tests");
    transaction::test_transactions(&client_1, &client_2, &client_3).await?;
    log("Transaction tests finished");

    log("Starting batch tests");
    batch::test_batch(&client_1, &client_2, &client_3).await?;
    log("Batch tests finished");

    log("Starting SQL type conversion tests");
    type_conversions::test_type_conversions(&client_1).await?;
    log("SQL type conversion tests finished");

    log("Test cache operations");
    cache::test_cache(&client_1, &client_2, &client_3).await?;
    log("Cache operations finished");

    log("Test listen / notify");
    listen_notify::test_listen_notify(&client_1, &client_2, &client_3).await?;
    log("listen / notify finished");

    log("Test distributed locks");
    dlock::test_dlock(&client_1, &client_2, &client_3).await?;
    log("Distributed locks tests finished");

    log("Test remote-only client");
    remote_only::test_remote_only_client().await?;
    log("Remote-only client tests finished");

    log("Test shutdown and restart");
    join_all([
        client_1.shutdown(),
        client_2.shutdown(),
        client_3.shutdown(),
    ])
    .await;
    log("Clients shutdown complete");

    // logs sync task runs every 200ms -> needs to catch the closed channel
    time::sleep(Duration::from_millis(250)).await;

    let (client_1, client_2, client_3) = start::start_test_cluster().await?;
    log("Cluster has been restarted");

    start::wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy again");

    // give caches some additional time to re-sync
    time::sleep(Duration::from_millis(250)).await;
    cache::insert_test_value_cache(&client_1).await?;

    log("Make sure all data is ok");
    check::is_client_db_healthy(&client_1, Some(1)).await?;
    check::is_client_db_healthy(&client_2, Some(1)).await?;
    check::is_client_db_healthy(&client_3, Some(1)).await?;
    log("All client DB's are healthy after restart");

    log("Starting backup tests");
    backup::test_backup(&client_1).await?;
    log("Backup tests finished");

    // BEGIN: test S3 backup restore
    log("Change current database as preparation for backup restore tests");
    backup::test_backup_restore_prerequisites(&client_1).await?;
    log("Current database has been changed");

    log("Shutting down nodes");
    join_all([
        client_1.shutdown(),
        client_2.shutdown(),
        client_3.shutdown(),
    ])
    .await;

    // just give the s3 upload task in the background some time to finish
    time::sleep(Duration::from_millis(1000)).await;

    log("Trying to start the cluster again after shutdown with restore from S3 backup");
    let (client_1, client_2, client_3) =
        backup_restore::start_test_cluster_with_backup(false).await?;
    log("Cluster has been started again");

    start::wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy again");
    // END: test S3 backup restore

    // BEGIN: test local file backup restore
    log("Change current database as preparation for backup restore tests");
    backup::test_backup_restore_prerequisites(&client_1).await?;
    log("Current database has been changed");

    log("Shutting down nodes");
    join_all([
        client_1.shutdown(),
        client_2.shutdown(),
        client_3.shutdown(),
    ])
    .await;

    time::sleep(Duration::from_millis(1000)).await;

    log("Trying to start the cluster again after shutdown with restore from local file backup");
    let (client_1, client_2, client_3) =
        backup_restore::start_test_cluster_with_backup(true).await?;
    log("Cluster has been started again");

    start::wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy again");
    // ENV: test local file backup restore

    cache::insert_test_value_cache(&client_1).await?;

    time::sleep(Duration::from_millis(1000)).await;

    log("Make sure databases are correctly restored");
    backup_restore::test_db_is_healthy_after_restore(&client_1).await?;
    backup_restore::test_db_is_healthy_after_restore(&client_2).await?;
    backup_restore::test_db_is_healthy_after_restore(&client_3).await?;

    // we need to wait a bit until all backup nodes have created a new snapshot
    time::sleep(Duration::from_millis(1000)).await;

    log("Start self-healing capabilities tests");
    test_self_healing(client_1, client_2, client_3).await?;
    log("Self-healing capabilities tests finished");

    Ok(())
}

fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic| {
        eprintln!("{}\n", panic);

        if let Some(location) = panic.location() {
            tracing::error!(
                message = %panic,
                panic.file = location.file(),
                panic.line = location.line(),
                panic.column = location.column(),
            );
            eprintln!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        } else {
            tracing::error!(message = %panic);
        }

        process::exit(1);
    }));
}

pub fn log<S: Display>(s: S) {
    info!("\n\n>>> {}\n", s);
}

#[allow(unused)]
pub fn debug<S: Debug>(s: &S) {
    info!("\n\n>>> {:?}\n", s);
}
