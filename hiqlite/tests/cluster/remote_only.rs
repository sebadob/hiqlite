use crate::start::SECRET_API;
use crate::{check, log, start};
use hiqlite::{Client, Error, Node};

pub async fn test_remote_only_client() -> Result<(), Error> {
    log("Make sure remote clients work fine with any member node, even if none leader");

    let client_1 = build_client(1);
    let client_2 = build_client(2);
    let client_3 = build_client(3);

    check_client(&client_1, 1).await?;
    check_client(&client_2, 2).await?;
    check_client(&client_3, 3).await?;

    // TODO
    // assert_eq!(1, 2);

    Ok(())
}

async fn check_client(client: &Client, id: u64) -> Result<(), Error> {
    check::is_client_db_healthy(&client, Some(id)).await?;
    Ok(())
}

fn build_client(node_id: u64) -> Client {
    let addr = start::nodes()
        .into_iter()
        .filter(|n| n.id == node_id)
        .collect::<Vec<Node>>()
        .swap_remove(0)
        .addr_api;

    Client::remote(node_id, addr, false, false, SECRET_API.to_string())
}
