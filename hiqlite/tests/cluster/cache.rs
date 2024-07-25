use crate::log;
use hiqlite::{DbClient, Error};
use std::time::Duration;
use tokio::time;

pub async fn test_cache(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    let key = "my key 1";
    let value = b"test value 123";

    log("Put a value into the cache");
    client_1.put(key, value).await?;

    log("Make sure all clients can read the cache value");

    let v = client_1.get(key).await?;
    assert_eq!(&v, value);

    time::sleep(Duration::from_millis(10)).await;
    let v = client_2.get(key).await?;
    assert_eq!(&v, value);
    let v = client_3.get(key).await?;
    assert_eq!(&v, value);

    log("Delete the value and make sure it's gone");
    client_1.delete(key).await?;

    let res = client_1.get(key).await;
    assert!(res.is_err());

    time::sleep(Duration::from_millis(10)).await;
    let res = client_2.get(key).await;
    assert!(res.is_err());
    let res = client_3.get(key).await;
    assert!(res.is_err());

    assert_eq!(1, 2);
    Ok(())
}
