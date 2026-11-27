//! The TLS env config refuses values it cannot parse instead of falling back to plain text.
//!
//! This is its own test binary with a single test: it changes process-wide env vars, which is
//! only sound while no other thread reads or writes them.

#![cfg(any(feature = "sqlite", feature = "cache"))]

use hiqlite::tls::ServerTlsConfig;
use std::env;
use std::panic;

/// Run `ServerTlsConfig::from_env("RAFT")` with only the given TLS env vars set.
fn from_env_with(vars: &[(&str, &str)]) -> std::thread::Result<Option<ServerTlsConfig>> {
    for name in [
        "HQL_TLS_AUTO_CERTS",
        "HQL_TLS_RAFT_KEY",
        "HQL_TLS_RAFT_CERT",
        "HQL_TLS_RAFT_NO_VERIFY",
    ] {
        // SAFETY: this binary holds a single test and nothing else runs alongside it.
        unsafe { env::remove_var(name) };
    }
    for (name, value) in vars {
        // SAFETY: see above.
        unsafe { env::set_var(name, value) };
    }
    panic::catch_unwind(|| ServerTlsConfig::from_env("RAFT").unwrap())
}

#[test]
fn tls_env_values_must_parse() {
    assert!(from_env_with(&[]).unwrap().is_none());
    assert!(
        from_env_with(&[("HQL_TLS_AUTO_CERTS", "false")])
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        from_env_with(&[("HQL_TLS_AUTO_CERTS", "true")]).unwrap(),
        Some(ServerTlsConfig::TlsAutoCertificates)
    ));

    // Asking for TLS with a typo must not start the node in plain text.
    for typo in ["ture", "yes", "1", "TRUE", ""] {
        let res = from_env_with(&[("HQL_TLS_AUTO_CERTS", typo)]);
        assert!(
            res.is_err(),
            "HQL_TLS_AUTO_CERTS={typo:?} must be refused, got {:?}",
            res.unwrap()
        );
    }

    let res = from_env_with(&[
        ("HQL_TLS_RAFT_KEY", "tls/key.pem"),
        ("HQL_TLS_RAFT_CERT", "tls/cert-chain.pem"),
        ("HQL_TLS_RAFT_NO_VERIFY", "ture"),
    ]);
    assert!(res.is_err(), "a bad HQL_TLS_RAFT_NO_VERIFY must be refused");
}
