use crate::{dashboard, Error};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::PasswordHasher;
use cryptr::utils::b64_encode;
use tokio::task;

pub async fn hash_password_b64(password: String) -> Result<String, Error> {
    task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        let s = dashboard::password::build_hasher()
            .hash_password(password.as_bytes(), &salt)?
            .to_string();

        Ok::<String, Error>(b64_encode(s.as_bytes()))
    })
    .await?
}
