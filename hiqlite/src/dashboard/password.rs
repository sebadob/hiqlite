use crate::Error;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use tokio::task;
use tracing::info;

pub async fn verify_password(plain: String, hash: String) -> Result<(), Error> {
    info!("hash: {}", hash);
    task::spawn_blocking(move || {
        let parsed_hash = PasswordHash::new(&hash)?;
        Argon2::default().verify_password(plain.as_bytes(), &parsed_hash)?;
        Ok::<(), Error>(())
    })
    .await??;
    Ok(())
}
