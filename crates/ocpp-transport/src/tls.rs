use std::sync::Arc;

use base64::Engine;
use http::header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ClientConfig, RootCertStore};
use tokio_tungstenite::{tungstenite::handshake::client::generate_key, Connector};

use crate::error::TransportError;

/// Connection security mode.
#[derive(Debug, Clone)]
pub enum SecurityProfile {
    /// Plain `ws://` — dev only.
    Plain,
    /// `wss://` with optional HTTP-Basic auth (OCPP Security Profile 1).
    Tls {
        basic_auth: Option<(String, String)>,
    },
    /// `wss://` with mutual-TLS client cert (OCPP Security Profile 3).
    Mtls {
        ca_pem: Option<Vec<u8>>,
        client_cert_pem: Vec<u8>,
        client_key_pem: Vec<u8>,
    },
}

impl SecurityProfile {
    pub fn connector(&self) -> Result<Option<Connector>, TransportError> {
        match self {
            SecurityProfile::Plain => Ok(Some(Connector::Plain)),
            SecurityProfile::Tls { .. } => {
                let cfg = build_client_config(None, None)?;
                Ok(Some(Connector::Rustls(Arc::new(cfg))))
            }
            SecurityProfile::Mtls {
                ca_pem,
                client_cert_pem,
                client_key_pem,
            } => {
                let cfg = build_client_config(
                    ca_pem.as_deref(),
                    Some((client_cert_pem.as_slice(), client_key_pem.as_slice())),
                )?;
                Ok(Some(Connector::Rustls(Arc::new(cfg))))
            }
        }
    }

    /// Build a WS upgrade `Request` carrying the OCPP subprotocol header,
    /// optional HTTP-Basic auth, and standard handshake headers.
    pub fn build_request(
        &self,
        url: &url::Url,
        subprotocol: &str,
    ) -> Result<http::Request<()>, TransportError> {
        let host = url
            .host_str()
            .ok_or_else(|| TransportError::Tls("url has no host".into()))?;
        let mut builder = http::Request::builder()
            .method("GET")
            .uri(url.as_str())
            .header("Host", host)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", generate_key())
            .header(SEC_WEBSOCKET_PROTOCOL, subprotocol);

        if let SecurityProfile::Tls {
            basic_auth: Some((user, pass)),
        } = self
        {
            let creds = format!("{user}:{pass}");
            let encoded = base64::engine::general_purpose::STANDARD.encode(creds);
            builder = builder.header(AUTHORIZATION, format!("Basic {encoded}"));
        }

        builder
            .body(())
            .map_err(|e| TransportError::Tls(format!("bad ws request: {e}")))
    }
}

fn build_client_config(
    extra_ca_pem: Option<&[u8]>,
    client_auth: Option<(&[u8], &[u8])>,
) -> Result<ClientConfig, TransportError> {
    let mut roots = RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs()
        .map_err(|e| TransportError::Tls(format!("native certs: {e}")))?
    {
        let _ = roots.add(cert);
    }
    if let Some(pem) = extra_ca_pem {
        let mut rd = std::io::BufReader::new(pem);
        for cert in rustls_pemfile::certs(&mut rd) {
            let cert = cert.map_err(|e| TransportError::Tls(format!("ca pem: {e}")))?;
            roots
                .add(cert)
                .map_err(|e| TransportError::Tls(format!("ca add: {e}")))?;
        }
    }

    let builder = ClientConfig::builder().with_root_certificates(roots);

    let cfg = match client_auth {
        None => builder.with_no_client_auth(),
        Some((cert_pem, key_pem)) => {
            let certs: Vec<CertificateDer<'static>> =
                rustls_pemfile::certs(&mut std::io::BufReader::new(cert_pem))
                    .collect::<Result<_, _>>()
                    .map_err(|e| TransportError::Tls(format!("client cert: {e}")))?;
            let key: PrivateKeyDer<'static> =
                rustls_pemfile::private_key(&mut std::io::BufReader::new(key_pem))
                    .map_err(|e| TransportError::Tls(format!("client key: {e}")))?
                    .ok_or_else(|| TransportError::Tls("no private key in pem".into()))?;
            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| TransportError::Tls(format!("client auth: {e}")))?
        }
    };
    Ok(cfg)
}
