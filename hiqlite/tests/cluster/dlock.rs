use crate::log;
use hiqlite::{Client, Error, Lock};
use std::time::Duration;
use tokio::{task, time};

pub async fn test_dlock(
    client_1: &Client,
    client_2: &Client,
    client_3: &Client,
) -> Result<(), Error> {
    log("Acquire first 2 locks");
    let lock_1 = client_1.lock("1").await?;
    let lock_2 = client_2.lock("2").await?;

    log("Make sure the locks can't be obtained on any client while not dropped");
    let c3 = client_3.clone();
    let handle_1 = task::spawn(async move {
        let lock = c3.lock("1").await?;
        Ok::<Lock, Error>(lock)
    });

    let c2 = client_2.clone();
    // just make sure that this 2nd task is not faster by accident
    time::sleep(Duration::from_millis(50)).await;
    let handle_1_2 = task::spawn(async move {
        c2.lock("1").await?;
        Ok::<(), Error>(())
    });

    let c3 = client_3.clone();
    let handle_2 = task::spawn(async move {
        c3.lock("2").await?;
        Ok::<(), Error>(())
    });

    time::sleep(Duration::from_millis(100)).await;
    // should not finish until we release locks
    assert!(!handle_1.is_finished());
    assert!(!handle_1_2.is_finished());
    assert!(!handle_2.is_finished());

    log("Drop locks and make sure the queued await can acquire it");
    drop(lock_1);
    log("awaiting handle_1");
    let lock_1_awaited = handle_1.await??;
    assert!(!handle_1_2.is_finished());
    assert!(!handle_2.is_finished());

    drop(lock_2);
    log("awaiting handle_2");
    handle_2.await??;
    assert!(!handle_1_2.is_finished());

    drop(lock_1_awaited);
    log("awaiting handle_1_2");
    handle_1_2.await??;

    log("Locks tests finished");

    Ok(())
}
