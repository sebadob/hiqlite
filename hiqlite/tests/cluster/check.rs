use crate::debug;
use crate::execute_query::TestData;
use hiqlite::{params, DbClient, Error, Param};

pub async fn is_client_db_healthy(client: &DbClient) -> Result<(), Error> {
    let is_healthy = client.is_healthy().await;
    debug(&is_healthy);
    assert!(is_healthy.is_ok());

    #[cfg(feature = "sqlite")]
    {
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
    }

    #[cfg(feature = "cache")]
    {
        let v: String = client.get(crate::cache::KEY).await?.unwrap();
        assert_eq!(&v, crate::cache::VALUE);
    }

    Ok(())
}
