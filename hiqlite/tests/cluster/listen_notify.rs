use crate::log;
use hiqlite::{Client, Error};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::time::Duration;
use tokio::time;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Event {
    id: i64,
    text: Cow<'static, str>,
}

pub async fn test_listen_notify(
    client_1: &Client,
    client_2: &Client,
    client_3: &Client,
) -> Result<(), Error> {
    log("Publish an event and make sure all members can see it");

    let event = Event {
        id: 133,
        text: "my event text".into(),
    };

    client_1.notify(&event).await?;

    let evt = client_1.listen::<Event>().await?;
    assert_eq!(evt, event);
    let evt = client_2.listen::<Event>().await?;
    assert_eq!(evt, event);
    let evt = client_3.listen::<Event>().await?;
    assert_eq!(evt, event);

    // make sure there a no other messages in the pipeline
    time::sleep(Duration::from_millis(20)).await;
    let res = client_1.try_listen::<Option<Event>>()?;
    assert!(res.is_none());
    let res = client_2.try_listen::<Option<Event>>()?;
    assert!(res.is_none());
    let res = client_3.try_listen::<Option<Event>>()?;
    assert!(res.is_none());

    Ok(())
}
