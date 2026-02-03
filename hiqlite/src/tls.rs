use crate::Error;
use axum_server::tls_rustls::RustlsConfig;
use rcgen::{CertificateParams, DnType, ExtendedKeyUsagePurpose, Issuer};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use std::borrow::Cow;
use std::env;
use std::ops::{Add, Sub};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use time::OffsetDateTime;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tracing::info;

static KEY_PAIR: OnceLock<rcgen::KeyPair> = OnceLock::new();

/// `TlsAutoCertificates` will generate self-signed TLS certificates. Clients will not validate
/// the certificates for ease of use because they don't have to. They do a 3-way handshake
/// anyway, which validates both client and server without the secret ever being sent over the
/// network.
///
/// If you want to handle certificates yourself, your can use the `Specific` variant.
#[derive(Debug, Clone)]
pub enum ServerTlsConfig {
    TlsAutoCertificates,
    Specific(ServerTlsConfigCerts),
}

#[derive(Debug, Clone)]
pub struct ServerTlsConfigCerts {
    pub key: Cow<'static, str>,
    pub cert: Cow<'static, str>,
    pub danger_tls_no_verify: bool,
}

impl ServerTlsConfigCerts {
    pub fn new<S: Into<Cow<'static, str>>>(key: S, cert: S) -> Self {
        Self {
            key: key.into(),
            cert: cert.into(),
            danger_tls_no_verify: false,
        }
    }
}

impl ServerTlsConfig {
    pub fn danger_tls_no_verify(&self) -> bool {
        match self {
            ServerTlsConfig::TlsAutoCertificates => true,
            ServerTlsConfig::Specific(s) => s.danger_tls_no_verify,
        }
    }

    pub fn from_env(variant: &str) -> Option<Self> {
        let tls_auto_certificates = env::var("HQL_TLS_AUTO_CERTS")
            .map(|v| v.parse::<bool>().unwrap_or(false))
            .unwrap_or(false);

        let key = env::var(format!("HQL_TLS_{variant}_KEY")).ok();
        let cert = env::var(format!("HQL_TLS_{variant}_CERT")).ok();
        let no_verify = env::var(format!("HQL_TLS_{variant}_DANGER_TLS_NO_VERIFY"))
            .ok()
            .map(|v| {
                v.parse::<bool>()
                    .expect("Cannot parse HQL_TLS_*_DANGER_TLS_NO_VERIFY to bool")
            });

        #[allow(clippy::unnecessary_unwrap)]
        if key.is_some() && cert.is_some() {
            Some(Self::Specific(ServerTlsConfigCerts {
                key: key.unwrap().into(),
                cert: cert.unwrap().into(),
                danger_tls_no_verify: no_verify.unwrap_or(false),
            }))
        } else if tls_auto_certificates {
            Some(Self::TlsAutoCertificates)
        } else {
            None
        }
    }

    pub async fn server_config(&self, url: &str) -> axum_server::tls_rustls::RustlsConfig {
        match self {
            ServerTlsConfig::TlsAutoCertificates => Self::server_config_self_signed(url).await,
            ServerTlsConfig::Specific(s) => RustlsConfig::from_pem_file(
                PathBuf::from(s.cert.as_ref()),
                PathBuf::from(s.key.as_ref()),
            )
            .await
            .expect("valid TLS certificate"),
        }
    }

    pub async fn server_config_self_signed(url: &str) -> axum_server::tls_rustls::RustlsConfig {
        let key_pair = if let Some(kp) = KEY_PAIR.get() {
            kp
        } else {
            info!("Generating new self-signed TLS certificates");
            let key_pair = tokio::task::spawn_blocking(|| rcgen::KeyPair::generate().unwrap())
                .await
                .unwrap();
            KEY_PAIR.set(key_pair).unwrap();
            KEY_PAIR.get().unwrap()
        };

        let name = if let Some((name, _)) = url.rsplit_once(":") {
            name
        } else {
            url
        };

        let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
        params.distinguished_name.push(DnType::CommonName, name);
        // params.use_authority_key_identifier_extension = true;
        params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::ServerAuth);
        params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::ClientAuth);
        let now = OffsetDateTime::now_utc();
        params.not_before = now.sub(Duration::from_secs(60));
        // The certificate will be valid for 3 years. We don't really need to care here. It will
        // not be verified anyway. The 3-way handshake with the secrets will validate client and
        // server once the connection is established. We only want to take advantage of the
        // encryption at this point.
        let exp = now.add(Duration::from_secs(3600 * 365 * 3));
        params.not_after = exp;

        let iss = Issuer::from_params(&params, &key_pair);
        let cert = params.signed_by(&key_pair, &iss).unwrap();

        let pem_key = key_pair.serialize_pem();
        let pem_cert = cert.pem();

        RustlsConfig::from_pem(pem_cert.as_bytes().to_vec(), pem_key.as_bytes().to_vec())
            .await
            .expect("Cannot build self-signed TLS certificates")
    }

    pub fn client_config(&self) -> Arc<ClientConfig> {
        match self {
            ServerTlsConfig::TlsAutoCertificates => build_tls_config(true),
            ServerTlsConfig::Specific(s) => build_tls_config(s.danger_tls_no_verify),
        }
    }
}

pub fn build_tls_config(tls_no_verify: bool) -> Arc<ClientConfig> {
    #[allow(unused_mut)]
    let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
    #[cfg(feature = "webpki-roots")]
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = if tls_no_verify {
        tokio_rustls::rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoTlsVerifier {}))
            .with_no_client_auth()
    } else {
        tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    };

    Arc::new(config)
}

pub async fn into_tls_stream(
    host: &str,
    stream: TcpStream,
    config: Arc<ClientConfig>,
) -> Result<TlsStream<TcpStream>, Error> {
    let dnsname = ServerName::try_from(host.to_string()).expect("invalid host address");
    let connector = tokio_rustls::TlsConnector::from(config);
    let tls_stream = connector.connect(dnsname, stream).await?;
    Ok(tls_stream)
}

#[derive(Debug)]
struct NoTlsVerifier {}

impl ServerCertVerifier for NoTlsVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}
