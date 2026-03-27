use std::time::Duration;

// not really dead code
// It will be used in any (real) scenario. This is only to get rid of a warning during some
// `clippy` checks.
#[allow(dead_code)]
pub fn build_http_client(tls_no_verify: bool) -> reqwest::Client {
    #[allow(unused_mut)]
    let mut builder = reqwest::Client::builder()
        .http2_prior_knowledge()
        .tls_danger_accept_invalid_certs(tls_no_verify)
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(30));

    #[cfg(feature = "webpki-roots")]
    {
        builder = builder.tls_certs_merge(
            webpki_root_certs::TLS_SERVER_ROOT_CERTS
                .iter()
                .map(|c| reqwest::Certificate::from_der(c).unwrap()),
        );
    }

    builder.build().unwrap()
}
