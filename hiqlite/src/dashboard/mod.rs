use crate::Error;
use cryptr::utils::b64_decode;
use cryptr::EncKeys;
use spow::pow::Pow;
use std::env;
use std::fmt::Debug;
use tracing::warn;

pub mod handlers;
pub mod middleware;
pub mod password;
mod query;
pub mod session;
pub mod static_files;
mod table;

#[derive(Debug)]
pub struct DashboardState {
    pub password_dashboard: Option<String>,
}

impl DashboardState {
    pub fn from_env() -> Self {
        match env::var("HQL_PASSWORD_DASHBOARD") {
            Ok(b64) => {
                let hash = String::from_utf8(b64_decode(&b64).unwrap()).unwrap();
                Self {
                    password_dashboard: Some(hash),
                }
            }
            Err(_) => {
                warn!("HQL_PASSWORD_DASHBOARD has not been set and the dashboard will be disabled");
                Self {
                    password_dashboard: None,
                }
            }
        }
    }
}

pub fn init() -> Result<(), Error> {
    let enc_key_active = EncKeys::get_key_active()?;
    Pow::init_bytes(enc_key_active);

    Ok(())
}
