use crate::execute_query::TestData;
use crate::{debug, log, params};
use hiqlite::{Client, Error};

use crate::cache::{KEY, KEY_2, VALUE, VALUE_2};
use crate::Cache;

pub async fn is_client_db_healthy(client: &Client, id: Option<u64>) -> Result<(), Error> {
    client.wait_until_healthy_db().await;
    client.wait_until_healthy_cache().await;

    log(format!("Checking DB health Node {:?}", id));
    let metrics = client.metrics_db().await?;
    let members = metrics.membership_config.nodes().count();
    assert_eq!(members, 3);

    log(format!("Checking Cache health {:?}", id));
    client.wait_until_healthy_cache().await;
    let metrics = client.metrics_cache().await?;
    let members = metrics.membership_config.nodes().count();
    assert_eq!(members, 3);

    // we will do the select 1 to catch leader switches that may have
    // happened in between and trigger a client stream switch that way
    client.batch("SELECT 1;").await?;

    // make sure our before inserted data exists
    let data: Result<Vec<TestData>, Error> = client
        .query_map("SELECT * FROM test WHERE id >= $1", params!(11))
        .await;
    debug(&data);
    let data = data?;

    assert_eq!(data.len(), 6);
    assert_eq!(data[0].id, 11);
    assert_eq!(data[1].id, 12);
    assert_eq!(data[2].id, 13);
    assert_eq!(data[3].id, 21);
    assert_eq!(data[4].id, 22);
    assert_eq!(data[5].id, 23);

    log(format!("Database healthy {:?}", id));

    let v: String = client.get(Cache::One, KEY).await?.unwrap();
    assert_eq!(&v, VALUE);
    let v: String = client.get(Cache::Two, KEY_2).await?.unwrap();
    assert_eq!(&v, VALUE_2);

    let v: Option<String> = client.get(Cache::One, KEY_2).await?;
    assert!(v.is_none());
    let v: Option<String> = client.get(Cache::Two, KEY).await?;
    assert!(v.is_none());

    log(format!("Cache healthy {:?}", id));

    // everything should still be healthy
    client
        .is_healthy_db()
        .await
        .expect("db should still be healthy");
    client
        .is_healthy_cache()
        .await
        .expect("cache should still be healthy");

    Ok(())
}
