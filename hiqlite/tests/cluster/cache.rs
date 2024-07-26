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
    client_1.put(KEY, &VALUE.to_string()).await?;

    log("Make sure all clients can read the cache value");

    let v: String = client_1.get(KEY).await?;
    assert_eq!(&v, VALUE);

    time::sleep(Duration::from_millis(10)).await;
    let v: String = client_2.get(KEY).await?;
    assert_eq!(&v, VALUE);
    let v: String = client_3.get(KEY).await?;
    assert_eq!(&v, VALUE);

    log("Delete the value and make sure it's gone");
    client_1.delete(KEY).await?;

    let res: Result<String, Error> = client_1.get(KEY).await;
    assert!(res.is_err());

    time::sleep(Duration::from_millis(10)).await;
    let res: Result<String, Error> = client_2.get(KEY).await;
    assert!(res.is_err());
    let res: Result<String, Error> = client_3.get(KEY).await;
    assert!(res.is_err());

    insert_test_value_cache(client_1).await?;

    Ok(())
}

pub async fn insert_test_value_cache(client: &DbClient) -> Result<(), Error> {
    log("Insert a test value again to be able to test replication after self-healing");
    client.put(KEY, &VALUE.to_string()).await?;

    Ok(())
}
