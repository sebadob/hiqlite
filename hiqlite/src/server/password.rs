use crate::Error;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use cryptr::utils::b64_encode;
use tokio::task;

// TODO move into single thread handler to prevent DoS possibilities
pub async fn hash_password_b64(password: String) -> Result<String, Error> {
    task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        let s = Argon2::default()
            .hash_password(password.as_bytes(), &salt)?
            .to_string();

        Ok::<String, Error>(b64_encode(s.as_bytes()))
    })
    .await?
}
