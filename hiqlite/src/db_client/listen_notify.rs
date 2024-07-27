use crate::store::state_machine::memory::state_machine::CacheRequest;
use crate::{DbClient, Error};
use chrono::Utc;
use serde::{Deserialize, Serialize};

impl DbClient {
    /// Listen to events on the distributed event bus
    pub async fn listen<T>(&self) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(state) = &self.state {
            let (_ts, bytes) = state.raft_cache.rx_notify.recv_async().await?;
            let res = bincode::deserialize(&bytes).unwrap();
            Ok(res)
        } else {
            Err(Error::Cache(
                "Listen / Notify does not work for remote clients yet".into(),
            ))
        }
    }

    /// Tries to receive an event and returns immediately, is non is currently waiting.
    pub fn try_listen<T>(&self) -> Result<Option<T>, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(state) = &self.state {
            if let Ok((_, bytes)) = state.raft_cache.rx_notify.try_recv() {
                let res = bincode::deserialize(&bytes).unwrap();
                Ok(Some(res))
            } else {
                Ok(None)
            }
        } else {
            Err(Error::Cache(
                "Listen / Notify does not work for remote clients yet".into(),
            ))
        }
    }

    /// Listen to events on the distributed event bus when their unix timestamp in microseconds
    /// is > `after_ts_micros`. This is helpful in case of applications restarts when cache
    /// events may be replayed to avoid duplicate event handling, if this applies to your case.
    pub async fn listen_after<T>(&self, after_ts_micros: i64) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(state) = &self.state {
            loop {
                let (ts, bytes) = state.raft_cache.rx_notify.recv_async().await?;
                if ts > after_ts_micros {
                    let res = bincode::deserialize(&bytes).unwrap();
                    return Ok(res);
                }
            }
        } else {
            Err(Error::Cache(
                "Listen / Notify does not work for remote clients yet".into(),
            ))
        }
    }

    /// Listen to events on the distributed event bus that are "new" in the sense that they must
    /// have been created after this application has been started.
    pub async fn listen_after_start<T>(&self) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.listen_after(self.app_start).await
    }

    /// Notify all other Raft members with this new event data.
    pub async fn notify<P>(&self, payload: &P) -> Result<(), Error>
    where
        P: Serialize,
    {
        if let Some(state) = &self.state {
            let payload = bincode::serialize(payload).unwrap();
            state
                .raft_cache
                .raft
                .client_write(CacheRequest::Notify((
                    Utc::now().timestamp_micros(),
                    payload,
                )))
                .await?;
            Ok(())
        } else {
            Err(Error::Cache(
                "Listen / Notify does not work for remote clients yet".into(),
            ))
        }
    }
}
