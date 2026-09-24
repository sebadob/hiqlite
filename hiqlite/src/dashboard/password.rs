use crate::Error;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordVerifier, Version};
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task;

// Very simple rate-limiting. The dashboard is meant for debugging purposes only. A single
// login / password hash at a time should be absolutely enough. This is not only additional
// brute-force protection (in addition to the PoW), but also resource exhaustion proctection.
static HASH_PERMITS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(1));

pub async fn verify_password(plain: String, hash: String) -> Result<(), Error> {
    let permit = tokio::time::timeout(Duration::from_secs(5), HASH_PERMITS.acquire())
        .await
        .map_err(|_| {
            Error::Request("Timeout while waiting for a password hashing permit".to_string())
        })?
        .map_err(|err| {
            Error::Request(format!(
                "Error while waiting for a password hashing permit: {err:?}"
            ))
        })?;

    let res = task::spawn_blocking(move || {
        let parsed_hash = PasswordHash::new(&hash)?;
        build_hasher().verify_password(plain.as_bytes(), &parsed_hash)?;
        Ok::<(), Error>(())
    })
    .await?;

    match res {
        Ok(_) => Ok(()),
        Err(_) => {
            // In case of an error, to somewhat prevent a permanent DoS because of the single
            // hasher allowed, we drop the permit early but delay this requests response. This is
            // not a complete protection, of course, because the client could brute-force
            // concurrently in a loop, but it gets us pretty far with simple mechanics. We don't
            // want to overcomplicate the dashboard, as it's meant for internal debugging oinly
            // and should never be exposed publicly anyway.
            drop(permit);

            tokio::time::sleep(Duration::from_secs(3)).await;
            Err(Error::Unauthorized("unauthorized".into()))
        }
    }
}

pub fn build_hasher<'a>() -> Argon2<'a> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(32_768, 2, 2, Some(32)).unwrap(),
    )
}
