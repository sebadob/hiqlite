use crate::{Error, dashboard};
use argon2::PasswordHasher;
use cryptr::utils::b64_encode;
use tokio::task;

pub async fn hash_password_b64(password: String) -> Result<String, Error> {
    task::spawn_blocking(move || {
        let s = dashboard::password::build_hasher()
            .hash_password(password.as_bytes())?
            .to_string();

        Ok::<String, Error>(b64_encode(s.as_bytes()))
    })
    .await?
}
