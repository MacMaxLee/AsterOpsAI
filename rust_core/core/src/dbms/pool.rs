//! Connection pool construction (requirement 2): every connection this
//! tool opens sets `application_name='ai-ops-coordinator'`,
//! `statement_timeout=5s`, `idle_in_transaction_session_timeout=10s`, and
//! an explicit `sslmode` — never a bare, unconfigured connection.

use std::sync::Arc;

use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::connection_metadata::{ConnectionMetadata, TlsMode};
use super::DbmsError;

const APPLICATION_NAME: &str = "ai-ops-coordinator";
const STATEMENT_TIMEOUT_MS: u32 = 5_000;
const IDLE_IN_TRANSACTION_TIMEOUT_MS: u32 = 10_000;

/// `sslmode=require`'s real, documented PostgreSQL semantics are "encrypt,
/// but don't verify the server's identity" — a weaker guarantee than
/// `verify-ca`/`verify-full` by design (see the `libpq` docs on `sslmode`).
/// This verifier implements exactly that: real TLS 1.2/1.3 handshake
/// signature verification (via rustls's own `verify_tls12_signature`/
/// `verify_tls13_signature` helpers, so the cryptography itself is not
/// weakened) while skipping certificate-chain/trust-anchor validation —
/// not a shortcut, the documented behavior of the mode this connects with.
/// `verify-ca`/`verify-full` are out of scope for this unit (see
/// `TlsMode`'s doc comment).
#[derive(Debug)]
struct EncryptOnlyVerifier {
    supported_algs: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for EncryptOnlyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.supported_algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.supported_algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported_algs.supported_schemes()
    }
}

fn build_deadpool_config(
    metadata: &ConnectionMetadata,
    password: &str,
) -> deadpool_postgres::Config {
    let mut cfg = deadpool_postgres::Config::new();
    cfg.host = Some(metadata.host.clone());
    cfg.port = Some(metadata.port);
    cfg.dbname = Some(metadata.database.clone());
    cfg.user = Some(metadata.username.clone());
    cfg.password = Some(password.to_string());
    cfg.application_name = Some(APPLICATION_NAME.to_string());
    // Startup-packet GUC options — the standard way to set session-level
    // config for every connection the pool opens, applied before this
    // tool's first query runs on that connection.
    cfg.options = Some(format!(
        "-c statement_timeout={STATEMENT_TIMEOUT_MS} \
         -c idle_in_transaction_session_timeout={IDLE_IN_TRANSACTION_TIMEOUT_MS}"
    ));
    cfg.ssl_mode = Some(metadata.tls_mode.into());
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    cfg
}

/// Builds the pool, branching on `TlsMode` for which TLS connector (if any)
/// backs it — `deadpool_postgres::Config::create_pool` is generic over the
/// connector type but always returns the same (non-generic) `Pool`, so both
/// branches produce an identical type here.
pub fn build_pool(metadata: &ConnectionMetadata, password: &str) -> Result<Pool, DbmsError> {
    let cfg = build_deadpool_config(metadata, password);

    match metadata.tls_mode {
        TlsMode::Disable => cfg
            .create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
            .map_err(|e| DbmsError::Other(e.to_string())),
        TlsMode::Prefer | TlsMode::Require => {
            let supported_algs =
                rustls::crypto::ring::default_provider().signature_verification_algorithms;
            let tls_config = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(EncryptOnlyVerifier { supported_algs }))
                .with_no_client_auth();
            let connector = MakeRustlsConnect::new(tls_config);
            cfg.create_pool(Some(Runtime::Tokio1), connector)
                .map_err(|e| DbmsError::Other(e.to_string()))
        }
    }
}

impl From<TlsMode> for deadpool_postgres::SslMode {
    fn from(mode: TlsMode) -> Self {
        match mode {
            TlsMode::Disable => Self::Disable,
            TlsMode::Prefer => Self::Prefer,
            TlsMode::Require => Self::Require,
        }
    }
}
