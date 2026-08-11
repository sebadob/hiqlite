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
use std::collections::HashMap;
use std::env;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::debug;

const COOKIE_NAME: &str = "__Host-Hiqlite-Session";
const COOKIE_NAME_DEV: &str = "Hiqlite-Session";
const SESSION_LIFETIME: i64 = 3600;

pub static INSECURE_COOKIES: LazyLock<bool> = LazyLock::new(|| {
    env::var("HQL_INSECURE_COOKIE")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .expect("Cannot parse HQL_INSECURE_COOKIE as bool")
});

// Login brute-force protection. The frontend PoW is currently disabled (the embedded Svelte 5
// build cannot load the spow WASM), so login throttling is the active protection: after a few
// failed attempts per client, further attempts are rejected with 429 for a lockout window.
// Combined with the global single-flight lock in `password::verify_password`, this caps both
// the per-client rate and the overall parallel attempt throughput.
const MAX_LOGIN_FAILURES: u8 = 5;
const LOGIN_LOCKOUT: Duration = Duration::from_secs(60);
const MAX_TRACKED_CLIENTS: usize = 10_000;

#[derive(Debug, Default)]
struct LoginThrottle {
    failures: HashMap<String, (u8, Instant)>,
}

impl LoginThrottle {
    fn is_locked(&self, key: &str, now: Instant) -> bool {
        if let Some((count, since)) = self.failures.get(key) {
            *count >= MAX_LOGIN_FAILURES && now.duration_since(*since) < LOGIN_LOCKOUT
        } else {
            false
        }
    }

    fn record_failure(&mut self, key: &str, now: Instant) {
        if self.failures.len() >= MAX_TRACKED_CLIENTS {
            // avoid unbounded growth from many distinct spoofed client ids
            self.failures.clear();
        }
        // a lockout that already expired must not carry its count over, otherwise the very
        // next failure would re-lock the client forever
        let expired = self
            .failures
            .get(key)
            .map(|(_, since)| now.duration_since(*since) >= LOGIN_LOCKOUT)
            .unwrap_or(false);
        if expired {
            self.failures.remove(key);
        }
        let (count, since) = self.failures.entry(key.to_string()).or_insert((0, now));
        *count += 1;
        *since = now;
    }

    fn clear(&mut self, key: &str) {
        self.failures.remove(key);
    }
}

static LOGIN_THROTTLE: OnceLock<Mutex<LoginThrottle>> = OnceLock::new();

fn login_throttle() -> &'static Mutex<LoginThrottle> {
    LOGIN_THROTTLE.get_or_init(|| Mutex::new(LoginThrottle::default()))
}

/// Best-effort client key. Uses the rightmost `X-Forwarded-For` entry (the hop added by the
/// closest proxy). `None` when the header is absent: those clients cannot be told apart, so
/// they are not throttled per-client.
fn client_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(|addr| addr.trim().to_string())
        .filter(|addr| !addr.is_empty())
}

fn check_login_throttle(headers: &HeaderMap) -> Result<(), Error> {
    let Some(key) = client_key(headers) else {
        // no client id available: a shared bucket would let one attacker lock out everyone
        // without a proxy header. the global single-flight lock in password::verify_password
        // still caps the overall attempt throughput.
        return Ok(());
    };
    let throttle = login_throttle().lock().unwrap_or_else(|e| e.into_inner());
    if throttle.is_locked(&key, Instant::now()) {
        Err(Error::RateLimit(
            "too many failed login attempts, try again later".into(),
        ))
    } else {
        Ok(())
    }
}

fn record_login_failure(headers: &HeaderMap) {
    if let Some(key) = client_key(headers) {
        let mut throttle = login_throttle().lock().unwrap_or_else(|e| e.into_inner());
        throttle.record_failure(&key, Instant::now());
    }
}

fn clear_login_failures(headers: &HeaderMap) {
    if let Some(key) = client_key(headers) {
        let mut throttle = login_throttle().lock().unwrap_or_else(|e| e.into_inner());
        throttle.clear(&key);
    }
}

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
            format!(
                "{COOKIE_NAME}={b64}; Secure; HttpOnly; SameSite=Lax; Max-Age={max_age}; Path=/"
            )
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
    // reject locked-out clients before spending argon2 time on them
    check_login_throttle(headers)?;
    if let Some(pwd) = state.dashboard.password_dashboard.clone() {
        if let Err(err) = password::verify_password(password, pwd).await {
            record_login_failure(headers);
            return Err(err);
        }
        clear_login_failures(headers);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn key(i: u64) -> String {
        format!("10.0.0.{i}")
    }

    #[test]
    fn throttle_locks_after_max_failures_and_expires() {
        let mut throttle = LoginThrottle::default();
        let t0 = Instant::now();

        for i in 0..MAX_LOGIN_FAILURES - 1 {
            assert!(!throttle.is_locked(&key(1), t0));
            throttle.record_failure(&key(1), t0);
        }
        // one below the limit is still allowed
        assert!(!throttle.is_locked(&key(1), t0));

        throttle.record_failure(&key(1), t0);
        assert!(throttle.is_locked(&key(1), t0));
        // other clients are unaffected
        assert!(!throttle.is_locked(&key(2), t0));

        // lockout expires after the window
        let t1 = t0 + LOGIN_LOCKOUT + Duration::from_secs(1);
        assert!(!throttle.is_locked(&key(1), t1));
    }

    #[test]
    fn successful_login_clears_failures() {
        let mut throttle = LoginThrottle::default();
        let t0 = Instant::now();
        throttle.record_failure(&key(1), t0);
        throttle.record_failure(&key(1), t0);
        throttle.clear(&key(1));
        assert!(!throttle.is_locked(&key(1), t0));
    }

    #[test]
    fn throttle_map_is_bounded() {
        let mut throttle = LoginThrottle::default();
        let t0 = Instant::now();
        for i in 0..MAX_TRACKED_CLIENTS + 5 {
            throttle.record_failure(&key(i as u64), t0);
        }
        assert!(throttle.failures.len() <= MAX_TRACKED_CLIENTS);
    }

    #[test]
    fn client_key_uses_rightmost_xff_hop() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());
        assert_eq!(client_key(&headers), Some("5.6.7.8".to_string()));
        assert_eq!(client_key(&HeaderMap::new()), None);
    }

    #[test]
    fn lockout_expiry_resets_the_counter() {
        let mut throttle = LoginThrottle::default();
        let t0 = Instant::now();
        for _ in 0..MAX_LOGIN_FAILURES {
            throttle.record_failure(&key(1), t0);
        }
        assert!(throttle.is_locked(&key(1), t0));

        // after the lockout window the client is allowed again, and the very next failure
        // must not re-lock immediately (fresh counter)
        let t1 = t0 + LOGIN_LOCKOUT + Duration::from_secs(1);
        assert!(!throttle.is_locked(&key(1), t1));
        throttle.record_failure(&key(1), t1);
        assert!(!throttle.is_locked(&key(1), t1));
    }
}
