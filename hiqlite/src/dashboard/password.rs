use crate::Error;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
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
        Argon2::default().verify_password(plain.as_bytes(), &parsed_hash)?;
        Ok::<(), Error>(())
    })
    .await??;

    Ok(())
}
