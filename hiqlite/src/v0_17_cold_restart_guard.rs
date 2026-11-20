use crate::http_client::build_http_client;
use crate::{Error, NodeConfig};
use std::env;
use std::fmt::Write;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// The lowest `MAJOR.MINOR.PATCH` a running cluster member must report on its `/version` endpoint.
const MIN_MEMBER_VERSION: (u64, u64, u64) = (0, 17, 0);

/// Set this env var to `true` and the whole guard is skipped. The integration tests use it because
/// they start nodes all the time, and the re-check delay is not what they are testing.
const ENV_DISABLE: &str = "HQL_DISABLE_COLD_RESTART_GUARD";

/// Time between the first and the second full check of all other cluster members.
const RECHECK_DELAY: Duration = Duration::from_secs(5);

/// The block that is logged after the concrete error of a found old cluster member.
const ERR: &str = r#"

    You are trying to do an invalid version upgrade!
    Read the changelog! v0.17 needs a full cold restart of the cluster. Shut
    down every node before starting this version. You MUST NOT do a rolling release!

            "#;

/// This function enforces the cold restart that the upgrade to v0.17 requires.
///
/// v0.17 replaces openraft 0.9 with 0.10, which breaks the Raft network layer. A node of this
/// version can only talk to members that run 0.17.0 or newer, so the whole cluster must be shut
/// down for the upgrade and all nodes must be updated at once. A rolling release would connect
/// nodes that cannot exchange Raft messages at all, which ends in a broken and inconsistent
/// cluster.
///
/// A cluster member that we cannot reach is doing exactly what we expect: it is shut down. A
/// member that does answer must run the new version already. We do the whole check twice with a
/// short delay in between, to catch a member that comes up right after our first pass.
///
/// It throws an error when it finds any issue. This is not a guarantee that a user cannot screw
/// up, but it's a help for anyone not reading the changelog properly. A single instance has no
/// peers to check at all, so it's very cheap to call it on each startup for the whole `v0.17`
/// release.
///
/// Setting `HQL_DISABLE_COLD_RESTART_GUARD` to `true` skips the whole guard. The integration
/// tests do that on every node they spin up, because the re-check delay is not what they test.
#[tracing::instrument(skip_all)]
pub async fn check_cold_restart(config: &NodeConfig) -> Result<(), Error> {
    #[cfg(debug_assertions)]
    if !(crate::APP_VERSION.starts_with("0.17.")
        || crate::APP_VERSION.starts_with("0.18.")
        || crate::APP_VERSION.starts_with("1.0."))
    {
        todo!("Clean up the v0_17_cold_restart_guard module after v0.18 - it's not needed anymore");
    }

    if env::var(ENV_DISABLE).is_ok_and(|v| v == "true") {
        warn!("The v0.17 cold restart guard is disabled via {ENV_DISABLE}");
        return Ok(());
    }

    if config.nodes.len() < 2 {
        debug!("Single instance - no cluster members to check");
        return Ok(());
    }

    // We don't care about TLS validation at this point, and if we ignore it directly, we
    // exclude a possible error source during our checks.
    let client = build_http_client(true);

    for check in 0..2 {
        if let Err(err) = ensure_no_old_members_running(config, &client).await {
            error!("{err}");
            error!("{ERR}");
            return Err(err);
        }

        if check == 0 {
            info!(
                "Found no old and still running cluster member. Checking a 2nd time in {} seconds.",
                RECHECK_DELAY.as_secs()
            );
            tokio::time::sleep(RECHECK_DELAY).await;
        }
    }

    info!("No old, running cluster member found - the cold restart of the full cluster was done.");
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
        write!(url, "{scheme}://{}/ping", node.addr_api)?;
        debug!("Sending request to {url}");

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
        }

        // The node is alive. Let's check which version it runs.
        url.clear();
        write!(url, "{scheme}://{}/version", node.addr_api)?;
        debug!("Sending request to {url}");

        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();

                if status != 200 {
                    // If there is no /version endpoint, but /ping was a success, it can only be a
                    // node running a version from way before this endpoint existed -> MUST NOT
                    // happen for a valid upgrade.
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

                let body = resp.text().await.unwrap_or_default();
                let body = body.trim();
                // A version body is `MAJOR.MINOR.PATCH` plus an optional pre-release suffix.
                let mut numbers = body
                    .split('-')
                    .next()
                    .unwrap_or_default()
                    .split('.')
                    .map(str::parse::<u64>);
                let version = match (numbers.next(), numbers.next(), numbers.next()) {
                    (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch))) => (major, minor, patch),
                    _ => {
                        // This should really never happen. If we get a success from that endpoint,
                        // it can only be a proper version string, unless it's a completely
                        // different application.
                        let msg =
                            format!("Invalid remote node version from {url} with body: {body}");
                        error!("{msg}");
                        return Err(Error::Request(msg));
                    }
                };

                if version < MIN_MEMBER_VERSION {
                    let msg = format!(
                        r#"
    Cluster member {url} is running the old version {body}.
    This version cannot sync with any node below 0.17.0 because the Raft network
    layer is incompatible. Shut down the full cluster, update every node, and
    start them all again. Check the changelog!
"#
                    );
                    error!("{msg}");
                    return Err(Error::Request(msg));
                }

                info!("Found running cluster member that is already on the new version: {body}");
            }
            Err(err) => {
                // Realistically, after being able to /ping the node, we should never have a
                // connection error at this point, unless the node actually went down between the
                // ping and this request. A pure network hiccup is caught by our 2nd full check,
                // and a node that really just shut down is fine for us as well.
                error!("Network error during /version check: {err:?}");
            }
        }
    }

    info!("All cluster nodes checked for not running at all or running a new version already.");
    Ok(())
}
