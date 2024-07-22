use crate::self_heal::test_self_healing;
use hiqlite::Error;
use std::fmt::{Debug, Display};
use std::time::Duration;
use std::{env, process};
use tokio::{fs, time};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod backup;
mod backup_restore;
mod batch;
mod check;
mod execute_query;
mod self_heal;
mod start;
mod transaction;

pub const TEST_DATA_DIR: &str = "tests/data_test";

#[tokio::test(flavor = "multi_thread")]
async fn test_cluster() -> Result<(), Error> {
    set_panic_hook();

    // always start clean
    env::remove_var("HIQLITE_BACKUP_RESTORE");
    let _ = fs::remove_dir_all(TEST_DATA_DIR).await;

    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_env_filter(EnvFilter::new("info"))
        .init();

    log("Starting cluster");
    let (client_1, client_2, client_3) = start::start_test_cluster().await?;
    log("Cluster has been started");

    start::wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy");

    log("Starting data insertion and query tests");
    execute_query::test_execute_query(&client_1, &client_2, &client_3).await?;
    log("Basic query tests finished");

    log("Starting Transaction tests");
    transaction::test_transactions(&client_1, &client_2, &client_3).await?;
    log("Transaction tests finished");

    log("Starting batch tests");
    batch::test_batch(&client_1, &client_2, &client_3).await?;
    log("Batch tests finished");

    log("Test shutdown and restart");
    client_1.shutdown().await?;
    log("client_1 shutdown complete");
    client_2.shutdown().await?;
    log("client_2 shutdown complete");
    client_3.shutdown().await?;
    log("client_3 shutdown complete");

    time::sleep(Duration::from_millis(100)).await;

    let (client_1, client_2, client_3) = start::start_test_cluster().await?;
    log("Cluster has been restarted");

    start::wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy again");

    log("Make sure all data is ok");
    check::is_client_db_healthy(&client_1).await?;
    check::is_client_db_healthy(&client_2).await?;
    check::is_client_db_healthy(&client_3).await?;
    log("All client DB's are healthy after restart");

    log("Starting backup tests");
    backup::test_backup(&client_1).await?;
    log("Backup tests finished");

    log("Change current database as preparation for backup restore tests");
    backup::test_backup_restore_prerequisites(&client_1).await?;
    log("Current database has been changed");

    log("Shutting down nodes");
    client_1.shutdown().await?;
    client_2.shutdown().await?;
    client_3.shutdown().await?;

    // just give the s3 upload task in the background some time to finish
    time::sleep(Duration::from_millis(1000)).await;

    log("Trying to start the cluster again after shutdown with restore from backup");
    let (client_1, client_2, client_3) = backup_restore::start_test_cluster_with_backup().await?;
    log("Cluster has been started again");

    start::wait_for_healthy_cluster(&client_1, &client_2, &client_3).await?;
    log("Cluster is healthy again");

    log("Make sure databases are correctly restored");
    backup_restore::test_db_is_healthy_after_restore(&client_1).await?;
    backup_restore::test_db_is_healthy_after_restore(&client_2).await?;
    backup_restore::test_db_is_healthy_after_restore(&client_3).await?;

    log("Start self-healing capabilities tests");
    test_self_healing(client_1, client_2, client_3).await?;
    log("Self-healing capabilities tests finished");

    log("All tests successful");

    // TODO sometimes something makes the test get stuck at the very end

    // TODO impl + test
    // - migrations
    // - consistent queries on leader

    // TODO test
    // - self-heal capabilities after data loss

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
