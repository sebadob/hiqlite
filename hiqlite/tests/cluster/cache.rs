use crate::log;
use hiqlite::{DbClient, Error};
use std::string::ToString;
use std::time::Duration;
use tokio::time;

pub const KEY: &str = "my key 1";
pub const VALUE: &str = "test value 123";

pub async fn test_cache(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    log("Put a value into the cache");
    client_1.put(KEY, &VALUE.to_string(), None).await?;

    log("Make sure all clients can read the cache value");

    let v: String = client_1.get(KEY).await?.unwrap();
    assert_eq!(&v, VALUE);

    time::sleep(Duration::from_millis(10)).await;
    let v: String = client_2.get(KEY).await?.unwrap();
    assert_eq!(&v, VALUE);
    let v: String = client_3.get(KEY).await?.unwrap();
    assert_eq!(&v, VALUE);

    log("Delete the value and make sure it's gone");
    client_1.delete(KEY).await?;

    let res: Option<String> = client_1.get(KEY).await?;
    assert!(res.is_none());

    time::sleep(Duration::from_millis(10)).await;
    let res: Option<String> = client_2.get(KEY).await?;
    assert!(res.is_none());
    let res: Option<String> = client_3.get(KEY).await?;
    assert!(res.is_none());

    log("Test cache TTL");
    let key = "key exp";
    let value = "some expiring value".to_string();
    client_1.put(key, &value, Some(1)).await?;

    // make sure it is still there for the next second
    time::sleep(Duration::from_millis(500)).await;
    let v: String = client_1.get(key).await?.unwrap();
    assert_eq!(v, value);
    let v: String = client_2.get(key).await?.unwrap();
    assert_eq!(v, value);
    let v: String = client_3.get(key).await?.unwrap();
    assert_eq!(v, value);

    // it should be gone shortly after the expiry
    time::sleep(Duration::from_millis(600)).await;
    let v: Option<String> = client_1.get(key).await?;
    assert!(v.is_none());
    let v: Option<String> = client_2.get(key).await?;
    assert!(v.is_none());
    let v: Option<String> = client_3.get(key).await?;
    assert!(v.is_none());

    insert_test_value_cache(client_1).await?;

    Ok(())
}

pub async fn insert_test_value_cache(client: &DbClient) -> Result<(), Error> {
    log("Insert a test value again to be able to test replication after self-healing");
    client.put(KEY, &VALUE.to_string(), None).await?;

    Ok(())
}
