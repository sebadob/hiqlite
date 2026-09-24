use crate::http_client::build_http_client;
use crate::{Error, NodeConfig};
use std::env;
use std::fmt::Write;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::fs;
use tracing::{debug, error, info, warn};

/// This function will provide the possibility to help with the migration to v0.15 from an earlier
/// version. When conditions for this version are incorrect coming from older data, the cache layer
/// will most probably be inconsistent or crash directly. This must not happen. The cached data must
/// be cleaned up before starting this new version because of a fundamental change in the WAL log
/// format.
///
/// This function checks if the migration is necessary, and it tries to protect the user from
/// screwing up by checking the status of remote nodes as well. It will throw an error when it finds
/// any issues. This is not a guarantee that a user cannot screw up, but it's a help. This function
/// will do nothing if we have data that was running on the new version already, so it's very cheap
/// to consistently call it on startup for the whole `v0.15` release.
#[tracing::instrument(skip_all)]
pub async fn check_migrate(config: &NodeConfig) -> Result<(), Error> {
    #[cfg(debug_assertions)]
    if !crate::APP_VERSION.starts_with("0.15.") {
        todo!("Clean up the whole check_migrate() function after v0.15 - it's not needed anymore");
    }

    #[cfg(feature = "in-memory-snapshots")]
    let is_pure_in_mem = true;
    #[cfg(not(feature = "in-memory-snapshots"))]
    let is_pure_in_mem = false;

    let data_dir = config.data_dir.as_ref();
    let path_logs = format!("{data_dir}/logs_cache");
    let path_sm = format!("{data_dir}/state_machine_cache");
    let force_migration = env::var("HQL_CACHE_WAL_FORCE_MIGRATE").as_deref() == Ok("true");
    if force_migration {
        warn!(
            "\n\nHQL_CACHE_WAL_FORCE_MIGRATE is set - enforcing cache data dir cleanup in 10 seconds!\n"
        );
        tokio::time::sleep(Duration::from_secs(10)).await;
    } else if !is_pure_in_mem {
        // There can only be exiting cache metadata after this version was started at least once.
        let path_sm_metadata = format!("{data_dir}/state_machine_cache/cache_index.meta");
        if fs::try_exists(&path_sm_metadata).await? {
            debug!("Found existing cache index metadata - all good");
            return Ok(());
        }

        // If the cache dirs are empty, this is a fresh startup.
        let mut exit_early = true;
        if let Ok(mut dir) = fs::read_dir(&path_logs).await
            && let Ok(next) = dir.next_entry().await
            && next.is_some()
        {
            exit_early = false;
        }
        if exit_early
            && let Ok(mut dir) = fs::read_dir(&path_sm).await
            && let Ok(next) = dir.next_entry().await
            && next.is_some()
        {
            exit_early = false;
        }
        if exit_early {
            debug!("No existing cache data found - all good");
            return Ok(());
        }

        if env::var("HQL_CACHE_WAL_AUTO_MIGRATE").as_deref() != Ok("true") {
            return Err(Error::Error(
            r#"
    The Cache cleanup must be done as mentioned in the changelog.
    Either you do it manually, or set the env var `HQL_CACHE_WAL_AUTO_MIGRATE=true` before the startup.
    Read the changelog!
"#.into(),
        ));
        }
    }

    // If this is not a single instance, we must check possibly running members on old versions.
    if config.nodes.len() > 1 {
        // We don't care about TLS validation at this point, and if we ignore it directly, we
        // exclude a possible error source during our checks.
        let client = build_http_client(true);

        const ERR: &str = r#"

    You are trying to do an invalid version upgrade!
    Read the changelog! Any existing cluster must be fully shut down before
    trying to upgrade to this version. You MUST NOT do a rolling release!

            "#;

        if let Err(err) = ensure_no_old_members_running(config, &client).await {
            error!("{err:}");
            error!("{ERR}");
            return Err(err);
        }
        info!(
            "Found no old and still running cluster member. Waiting 5 seconds before testing a 2nd time to really be sure."
        );

        tokio::time::sleep(Duration::from_secs(5)).await;
        if let Err(err) = ensure_no_old_members_running(config, &client).await {
            error!("{err:}");
            error!("{ERR}");
            return Err(err);
        }
        info!("Still no old, running cluster member found - proceeding with the auto-cleanup.");
    }

    if !is_pure_in_mem {
        // all good - cleanup data
        warn!("Cleaning up {path_logs}");
        if let Err(err) = fs::remove_dir_all(&path_logs).await
            && !matches!(err.kind(), ErrorKind::NotFound)
        {
            return Err(Error::Error(
                format!("Error during auto-cleaning {path_logs}: {err:?}").into(),
            ));
        }

        warn!("Cleaning up {path_sm}");
        if let Err(err) = fs::remove_dir_all(&path_sm).await
            && !matches!(err.kind(), ErrorKind::NotFound)
        {
            return Err(Error::Error(
                format!("Error during auto-cleaning {path_sm}: {err:?}").into(),
            ));
        }

        info!("Auto-Cleanup of cache data successful - proceeding with normal startup now.");
    } else {
        info!("No auto-cleanup needed for pure in memory caches.")
    }

    Ok(())
}

async fn ensure_no_old_members_running(
    config: &NodeConfig,
    client: &reqwest::Client,
) -> Result<(), Error> {
    let scheme = if config.tls_api.is_some() {
        "https"
    } else {
        "http"
    };
    let mut url = String::with_capacity(48);

    for node in &config.nodes {
        if node.id == config.node_id {
            continue;
        }

        // If we can't reach the node via /ping, we don't need any further tests.
        url.clear();
        write!(url, "{}://{}/ping", scheme, node.addr_api,)?;
        debug!("Sending request to {}", url);

        match client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    // This should really never happen. The ping endpoint is open and
                    // unauthenticated.
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    let msg = format!(
                        "Reached remote node via {url} with unexpected response: {status} - {body}"
                    );
                    error!("{msg}");
                    return Err(Error::Request(msg));
                }
            }
            Err(_) => {
                // In case of any connection error, this node is simply not reachable.
                continue;
            }
        };

        // The node is alive. Let's check if it's an old or new version.
        url.clear();
        write!(url, "{}://{}/version", scheme, node.addr_api,)?;
        debug!("Sending request to {}", url);

        // We are looping twice to catch any network hiccups in case we have a connection error at
        // this point.
        for _ in 0..2 {
            match client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();

                    if status == 200 {
                        // The version endpoint is reachable and returns a (proper) version. This must
                        // be a node that already did the cache data clear successfully. We can proceed
                        // in this case, as there is no risk of syncing invalid cache data from that
                        // node once we started the Raft.
                        //
                        // Just double-check that the version is actually as expected.
                        let version = resp.text().await.unwrap_or_default();
                        // Make sure to also see the next upcoming version as valid to not have any
                        // issue with a rolling release with the next cycle.
                        if version.starts_with("0.15.")
                            || version.starts_with("0.16.")
                            || version.starts_with("1.0.")
                        {
                            info!(
                                "Found running cluster member that is already on the new version."
                            );
                            break;
                        } else {
                            // This should really never happen. If we get a success from that endpoint,
                            // it can only be a proper version string, unless it's a completely
                            // different application.
                            let msg = format!(
                                "Invalid remote node version from {url} with unexpected body: {version}"
                            );
                            error!("{msg}");
                            return Err(Error::Request(msg));
                        }
                    } else {
                        // If there is no /version endpoint, but /ping was a success, it can only be a
                        // node running an older version -> MUST NOT happen for this migration to be
                        // safe.
                        let msg = format!(
                            r#"
    Remote node {url} does not have a /version endpoint: HTTP {status}.
    This means it can only be a running node with an old version. You MUST shut
    down the full cluster before attempting an upgrade to this version.
    Check the changelog!
"#
                        );
                        error!("{msg}");
                        return Err(Error::Request(msg));
                    }
                }
                Err(err) => {
                    // Realistically, after being able to /ping the node, we should never have a
                    // connection error at this point, unless the node actually went down between the
                    // ping and this request. That's exactly why we have a loop for this 2nd check.
                    // If it happens both times, the node probably just shut down, which is fine
                    // for us as well.
                    error!("Network error during /version check: {err:?}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    info!(
        "All cluster nodes checked for either not being running or running on the new version already."
    );
    Ok(())
}
