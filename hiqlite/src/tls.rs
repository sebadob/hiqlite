use crate::Error;
use axum_server::tls_rustls::RustlsConfig;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

#[derive(Debug, Clone)]
pub struct ServerTlsConfig {
    pub key: &'static str,
    pub cert: &'static str,
    pub danger_tls_no_verify: bool,
}

impl ServerTlsConfig {
    pub fn new(key: &'static str, cert: &'static str) -> Self {
        Self {
            key,
            cert,
            danger_tls_no_verify: false,
        }
    }

    pub(crate) async fn server_config(&self) -> axum_server::tls_rustls::RustlsConfig {
        RustlsConfig::from_pem_file(PathBuf::from(self.cert), PathBuf::from(self.key))
            .await
            .expect("valid TLS certificate")
    }

    pub(crate) fn client_config(&self) -> Arc<ClientConfig> {
        build_tls_config(self.danger_tls_no_verify)
    }
}

pub fn build_tls_config(tls_no_verify: bool) -> Arc<ClientConfig> {
    let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
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
            SignatureScheme::ED25519,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
        ]
    }
}
