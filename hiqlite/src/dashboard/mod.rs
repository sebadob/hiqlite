use crate::Error;
use cryptr::utils::b64_decode;
use cryptr::EncKeys;
use spow::pow::Pow;
use std::env;
use std::fmt::Debug;

pub mod handlers;
pub mod middleware;
mod password;
mod query;
pub mod session;
pub mod static_files;
mod table;

#[derive(Debug)]
pub struct DashboardState {
    pub password_dashboard: String,
}

impl DashboardState {
    pub fn from_env() -> Self {
        let b64 =
            env::var("HQL_PASSWORD_DASHBOARD").expect("HQL_PASSWORD_DASHBOARD does not exist");
        let password_dashboard = String::from_utf8(b64_decode(&b64).unwrap()).unwrap();

        Self { password_dashboard }
    }
}

pub fn init() -> Result<(), Error> {
    let enc_key_active = EncKeys::get_key_active()?;
    Pow::init_bytes(enc_key_active);

    Ok(())
}
