use crate::dashboard::handlers::LoginRequest;
use crate::dashboard::password;
use crate::network::AppStateExt;
use crate::Error;
use axum::body::Body;
use axum::extract::FromRequestParts;
use axum::http::header::SET_COOKIE;
use axum::http::{request, HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum_extra::extract::CookieJar;
use chrono::Utc;
use cryptr::utils::{b64_decode, b64_encode};
use cryptr::EncValue;
use serde::{Deserialize, Serialize};
use tracing::debug;

const COOKIE_NAME: &str = "__Host-Hiqlite-Session";
const COOKIE_NAME_DEV: &str = "Hiqlite-Session";
const SESSION_LIFETIME: i64 = 3600;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Session {
    created: i64,
    expires: i64,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let headers = &parts.headers;
        check_csrf(&parts.method, headers).await?;

        let jar = CookieJar::from_headers(headers);
        Ok(Session::try_from(&jar)?)
    }
}

impl TryFrom<&CookieJar> for Session {
    type Error = Error;

    fn try_from(jar: &CookieJar) -> Result<Self, Self::Error> {
        // TODO decide between dev and prod
        let c = jar
            .get(COOKIE_NAME_DEV)
            .ok_or(Error::Unauthorized("no session found".into()))?;
        let enc_bytes = b64_decode(c.value())?;
        let dec = EncValue::try_from_bytes(enc_bytes)?.decrypt()?;

        let slf: Self = bincode::deserialize(dec.as_ref()).unwrap();
        slf.is_valid()?;

        Ok(slf)
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
        let bytes = bincode::serialize(self).unwrap();
        let enc = EncValue::encrypt(&bytes)?;
        let enc_bytes = enc.into_bytes().to_vec();
        let b64 = b64_encode(&enc_bytes);

        let max_age = Utc::now().timestamp() - self.expires;
        Ok(format!(
            "{}={}; Secure; HttpOnly; SameSite=Lax; Max-Age={}",
            COOKIE_NAME_DEV, b64, max_age
        ))
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
    login: LoginRequest,
) -> Result<Response, Error> {
    check_csrf(&method, headers).await?;
    password::verify_password(login.password, state.dashboard.password_dashboard.clone()).await?;

    let cookie = Session::new().as_cookie()?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(SET_COOKIE, cookie)
        .body(Body::empty())
        .unwrap())
}

#[inline]
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

            debug!("sec-fetch-dest: {}, sec-fetch-mode: {}", dest, mode);

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
        // allow requests that do not contain the header
        Ok(())
    }
}
