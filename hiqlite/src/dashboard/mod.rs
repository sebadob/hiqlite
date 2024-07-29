use cryptr::utils::b64_decode;
use std::env;
use std::fmt::Debug;

pub mod handlers;
pub mod middleware;
mod password;
pub mod session;
pub mod static_files;

#[derive(Debug)]
pub struct DashboardState {
    pub password_dashboard: String,
    // pub insecure_cookie: bool,
}

impl DashboardState {
    pub fn from_env() -> Self {
        let b64 =
            env::var("HQL_PASSWORD_DASHBOARD").expect("HQL_PASSWORD_DASHBOARD does not exist");
        let password_dashboard = String::from_utf8(b64_decode(&b64).unwrap()).unwrap();

        // let insecure_cookie = env::var("HQL_INSECURE_COOKIE")
        //     .unwrap_or_else(|_| "false".to_string())
        //     .parse::<bool>()
        //     .expect("Cannot parse HQL_INSECURE_COOKIE as bool");

        Self {
            password_dashboard,
            // insecure_cookie,
        }
    }
}

// #[async_trait]
// impl<S> FromRequestParts<S> for Arc<AppState>
// where
//     Self: FromRef<S>,
//     S: Send + Sync,
// {
//     type Rejection = Error;
//
//     async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//         Ok(Self::from_ref(state))
//     }
// }
