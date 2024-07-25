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
    let value = "test value 123".to_string();

    log("Put a value into the cache");
    client_1.put(key, &value).await?;

    log("Make sure all clients can read the cache value");

    let v: String = client_1.get(key).await?;
    assert_eq!(&v, &value);

    time::sleep(Duration::from_millis(10)).await;
    let v: String = client_2.get(key).await?;
    assert_eq!(&v, &value);
    let v: String = client_3.get(key).await?;
    assert_eq!(&v, &value);

    log("Delete the value and make sure it's gone");
    client_1.delete(key).await?;

    let res: Result<String, Error> = client_1.get(key).await;
    assert!(res.is_err());

    time::sleep(Duration::from_millis(10)).await;
    let res: Result<String, Error> = client_2.get(key).await;
    assert!(res.is_err());
    let res: Result<String, Error> = client_3.get(key).await;
    assert!(res.is_err());

    assert_eq!(1, 2);
    Ok(())
}
