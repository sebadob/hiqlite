use crate::config::RateLimitConfig;
use crate::{Client, Error};
use std::cmp::max;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task;

impl Client {
    pub(crate) fn spawn_rate_limit_ticker(
        &self,
        config_cache: Option<RateLimitConfig>,
        config_db: Option<RateLimitConfig>,
        cache_await: crossbeam::channel::Receiver<oneshot::Sender<Result<(), Error>>>,
        db_await: crossbeam::channel::Receiver<oneshot::Sender<Result<(), Error>>>,
    ) {
        task::spawn(Self::run_ticker(
            self.clone(),
            config_cache,
            config_db,
            cache_await,
            db_await,
        ));
    }

    #[allow(unused_variables)]
    async fn run_ticker(
        slf: Client,
        config_cache: Option<RateLimitConfig>,
        config_db: Option<RateLimitConfig>,
        cache_await: crossbeam::channel::Receiver<oneshot::Sender<Result<(), Error>>>,
        db_await: crossbeam::channel::Receiver<oneshot::Sender<Result<(), Error>>>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            #[cfg(feature = "cache")]
            if let Some(config) = &config_cache {
                let lim = slf.inner.rate_limit_cache.as_ref().unwrap();

                lim.try_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    Some(max(current + config.rps, config.burst))
                })
                .ok();

                while let Ok(tx) = cache_await.try_recv() {
                    if tx.send(slf.try_rate_limit_cache()).is_err() {
                        lim.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }

            #[cfg(feature = "sqlite")]
            if let Some(config) = &config_db {
                let lim = slf.inner.rate_limit_db.as_ref().unwrap();

                lim.try_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    Some(max(current + config.rps, config.burst))
                })
                .ok();

                while let Ok(tx) = db_await.try_recv() {
                    if tx.send(slf.try_rate_limit_db()).is_err() {
                        lim.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    #[cfg(feature = "cache")]
    pub(crate) fn try_rate_limit_cache(&self) -> Result<(), Error> {
        if let Some(v) = &self.inner.rate_limit_cache {
            v.try_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                if current > 0 { Some(current - 1) } else { None }
            })
            .map_err(|_| Error::RateLimit("Raft rate-limit hit for the Cache layer".into()))?;
        }

        Ok(())
    }

    #[cfg(feature = "cache")]
    pub(crate) async fn rate_limit_cache(&self) -> Result<(), Error> {
        match self.try_rate_limit_cache() {
            Ok(_) => Ok(()),
            Err(err) => {
                let (tx, rx) = oneshot::channel();
                if self.inner.rate_limit_cache_await.try_send(tx).is_ok() {
                    rx.await.unwrap()
                } else {
                    Err(err)
                }
            }
        }
    }

    #[cfg(feature = "sqlite")]
    pub(crate) fn try_rate_limit_db(&self) -> Result<(), Error> {
        if let Some(v) = &self.inner.rate_limit_db {
            v.try_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                if current > 0 { Some(current - 1) } else { None }
            })
            .map_err(|_| Error::RateLimit("Raft rate-limit hit for the DB layer".into()))?;
        }

        Ok(())
    }

    #[cfg(feature = "sqlite")]
    pub(crate) async fn rate_limit_db(&self) -> Result<(), Error> {
        match self.try_rate_limit_db() {
            Ok(_) => Ok(()),
            Err(err) => {
                let (tx, rx) = oneshot::channel();
                if self.inner.rate_limit_db_await.try_send(tx).is_ok() {
                    rx.await.unwrap()
                } else {
                    Err(err)
                }
            }
        }
    }
}
