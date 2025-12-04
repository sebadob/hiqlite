use crate::Error;
use crate::dashboard::password;
use crate::helpers::deserialize;
use crate::network::{AppStateExt, serialize_network};
use axum::Json;
use axum::extract::FromRequestParts;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, Method, request};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use cryptr::EncValue;
use cryptr::utils::{b64_decode, b64_encode};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::LazyLock;
use tracing::{debug, warn};

const COOKIE_NAME: &str = "__Host-Hiqlite-Session";
const COOKIE_NAME_DEV: &str = "Hiqlite-Session";
const SESSION_LIFETIME: i64 = 3600;

pub static INSECURE_COOKIES: LazyLock<bool> = LazyLock::new(|| {
    env::var("HQL_INSECURE_COOKIE")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .expect("Cannot parse HQL_INSECURE_COOKIE as bool")
});

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Session {
    created: i64,
    expires: i64,
}

impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // TODO we can't impl the from req for Arc<AppState>
        // let st = parts
        //     .extract_with_state::<Arc<AppState>, _>(state)
        //     .await
        //     .expect("AppState to be available");

        let headers = &parts.headers;
        check_csrf(&parts.method, headers).await?;

        let jar = CookieJar::from_headers(headers);
        Session::try_from_jar(&jar)
    }
}

impl Session {
    fn new() -> Self {
        let created = Utc::now().timestamp();
        let expires = created + SESSION_LIFETIME;
        Self { created, expires }
    }

    fn as_cookie(&self) -> Result<String, Error> {
        // TODO decide between dev and prod
        let bytes = serialize_network(self);
        let enc = EncValue::encrypt(&bytes)?;
        let enc_bytes = enc.into_bytes().to_vec();
        let b64 = b64_encode(&enc_bytes);

        let max_age = self.expires - Utc::now().timestamp();

        let cookie_header = if *INSECURE_COOKIES {
            format!("{COOKIE_NAME_DEV}={b64}; HttpOnly; SameSite=Lax; Max-Age={max_age}")
        } else {
            format!("{COOKIE_NAME}={b64}; Secure; HttpOnly; SameSite=Lax; Max-Age={max_age} Path=/")
        };

        Ok(cookie_header)
    }

    // async fn try_from_headers(headers: &HeaderMap, method: &Method) -> Result<Self, Error> {
    //     check_csrf(&method, headers).await?;
    //     let jar = CookieJar::from_headers(headers);
    //     Ok(Session::try_from_jar(&jar)?)
    // }

    fn try_from_jar(jar: &CookieJar) -> Result<Self, Error> {
        // TODO decide between dev and prod
        let name = if *INSECURE_COOKIES {
            COOKIE_NAME_DEV
        } else {
            COOKIE_NAME
        };
        let cookie = jar
            .get(name)
            .ok_or(Error::Unauthorized("no session found".into()))?;

        let enc_bytes = b64_decode(cookie.value())?;
        let dec = EncValue::try_from_bytes(enc_bytes)?.decrypt()?;

        let slf: Self = deserialize(dec.as_ref())?;
        slf.is_valid()?;

        Ok(slf)
    }

    #[inline]
    fn is_valid(&self) -> Result<(), Error> {
        if self.expires < Utc::now().timestamp() {
            Err(Error::Unauthorized("session has expired".into()))
        } else {
            Ok(())
        }
    }
}

pub async fn set_session_verify(
    state: &AppStateExt,
    method: Method,
    headers: &HeaderMap,
    password: String,
) -> Result<Response, Error> {
    check_csrf(&method, headers).await?;
    if let Some(pwd) = state.dashboard.password_dashboard.clone() {
        password::verify_password(password, pwd).await?;

        let session = Session::new();
        let cookie = session.as_cookie()?;
        Ok(([(SET_COOKIE, cookie)], Json(session)).into_response())
    } else {
        Err(Error::Unauthorized("unauthorized".into()))
    }
}

async fn check_csrf(method: &Method, headers: &HeaderMap) -> Result<(), Error> {
    if let Some(site) = headers.get("sec-fetch-site") {
        let site = site.to_str().unwrap_or_default();

        // same origin is always allowed
        if site == "same-origin" {
            return Ok(());
        }

        if method == Method::GET {
            // user interactions will be 'none'
            if site == "none" {
                return Ok(());
            }

            // allow links and redirects from external sites
            let dest = headers
                .get("sec-fetch-dest")
                .map(|h| h.to_str().unwrap_or_default())
                .unwrap_or_default();
            let mode = headers
                .get("sec-fetch-mode")
                .map(|h| h.to_str().unwrap_or_default())
                .unwrap_or_default();

            debug!("sec-fetch-dest: {dest}, sec-fetch-mode: {mode}");

            // allow images fetches like favicon
            if dest == "image" && mode == "no-cors" {
                return Ok(());
            }

            // allow navigation to this site but no embedding
            if mode == "navigate" && !["embed", "iframe", "object"].contains(&dest) {
                return Ok(());
            }
        }

        Err(Error::Unauthorized(
            "cross-origin request forbidden for this resource".into(),
        ))
    } else {
        debug!("sec-fetch-site is missing");
        if *INSECURE_COOKIES {
            // Sec-* headers will not be added in an insecure context
            Ok(())
        } else {
            Err(Error::Unauthorized("CSRF violation".into()))
        }
    }
}
