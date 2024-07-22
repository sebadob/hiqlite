use crate::log;
use hiqlite::{DbClient, Error};

pub async fn test_self_healing(
    client_1: &DbClient,
    client_2: &DbClient,
    client_3: &DbClient,
) -> Result<(), Error> {
    is_healthy_after_self_heal(client_1).await?;
    is_healthy_after_self_heal(client_2).await?;
    is_healthy_after_self_heal(client_3).await?;

    log("Test recovery from state machine data loss");
    let client = choose_non_leader_client(client_1, client_2).await?;
    is_healthy_after_self_heal(client).await?;

    Ok(())
}

async fn is_healthy_after_self_heal(client: &DbClient) -> Result<(), Error> {
    assert!(client.is_healthy().await.is_ok());
    client.batch("SELECT 1;").await?;
    Ok(())
}

async fn choose_leader_client<'a>(
    client_1: &'a DbClient,
    client_2: &'a DbClient,
    client_3: &'a DbClient,
) -> Result<&'a DbClient, Error> {
    if is_leader(client_1, 1).await? {
        Ok(client_1)
    } else if is_leader(client_2, 2).await? {
        Ok(client_2)
    } else {
        Ok(client_3)
    }
}

async fn choose_non_leader_client<'a>(
    client_1: &'a DbClient,
    client_2: &'a DbClient,
) -> Result<&'a DbClient, Error> {
    if !is_leader(client_1, 1).await? {
        Ok(client_1)
    } else {
        Ok(client_2)
    }
}

async fn is_leader(client: &DbClient, node_id: u64) -> Result<bool, Error> {
    if let Some(leader) = client.metrics().await?.current_leader {
        Ok(leader == node_id)
    } else {
        Err(Error::LeaderChange("No leader exists right now".into()))
    }
}
