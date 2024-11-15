use crate::Error;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordVerifier, Version};
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tokio::task;

// very simple way to rate-limit password hashing / login.
// only a single password hash at a time is allowed, the dashboard is just for debugging.
// prevents brute-fore effectively.
static IS_HASHING: LazyLock<RwLock<()>> = LazyLock::new(|| RwLock::new(()));

pub async fn verify_password(plain: String, hash: String) -> Result<(), Error> {
    let _ = IS_HASHING.write().await;

    task::spawn_blocking(move || {
        let parsed_hash = PasswordHash::new(&hash)?;
        build_hasher().verify_password(plain.as_bytes(), &parsed_hash)?;
        Ok::<(), Error>(())
    })
    .await??;

    Ok(())
}

pub fn build_hasher<'a>() -> Argon2<'a> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(32_768, 2, 2, Some(32)).unwrap(),
    )
}
