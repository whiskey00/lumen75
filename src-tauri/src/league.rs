//! League of Legends: the live match data Riot's game serves on this PC
//! (Live Client Data API, `https://127.0.0.1:2999/liveclientdata/`, only during a match).

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, RootCertStore, SignatureScheme};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

// ------------------------------------------------------------ the local endpoint

const ALL_GAME_DATA: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";

/// Riot's root certificate for the game's local endpoints (from Riot's developer docs; the
/// copy in RiotGames/leaguedirector, identical to two independent copies).
fn riot_roots() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    for cert in rustls_pemfile::certs(&mut &include_bytes!("../certs/riotgames.pem")[..]).flatten() {
        let _ = roots.add(cert);
    }
    roots
}

/// The game's certificate chains to Riot's root but isn't issued for 127.0.0.1, so the name
/// is the one thing not checked. Only ever used for the fixed local address.
#[derive(Debug)]
struct RiotOnly(Arc<WebPkiServerVerifier>);

impl ServerCertVerifier for RiotOnly {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match self.0.verify_server_cert(end_entity, intermediates, server_name, ocsp, now) {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
            )) => Ok(ServerCertVerified::assertion()),
            other => other,
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(riot_roots()), provider.clone())
            .build()
            .expect("Riot's root certificate");
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("TLS versions")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(RiotOnly(verifier)))
            .with_no_client_auth();
        ureq::AgentBuilder::new().tls_config(Arc::new(tls)).timeout(Duration::from_millis(500)).build()
    })
}

/// What the endpoint answers when there's no match data yet (the loading screen).
fn is_error_reply(body: &str) -> bool {
    body.trim_start().starts_with('{') && body.contains("\"errorCode\"")
}

/// The live match data, or None when there's no match: nothing listening, a 404, or an
/// error reply. Err only for a bad certificate or another HTTP error.
pub fn fetch() -> Result<Option<String>, String> {
    match agent().get(ALL_GAME_DATA).call() {
        Ok(r) => {
            let body = r.into_string().map_err(|e| e.to_string())?;
            Ok((!is_error_reply(&body)).then_some(body))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(code, _)) => Err(format!("League's live data answered {code}.")),
        Err(ureq::Error::Transport(t)) => {
            // refused or timed out: the game isn't serving (no match); anything else is real
            let mut source = std::error::Error::source(&t);
            while let Some(e) = source {
                if let Some(io) = e.downcast_ref::<std::io::Error>() {
                    use std::io::ErrorKind::*;
                    if matches!(io.kind(), ConnectionRefused | TimedOut | WouldBlock | ConnectionReset) {
                        return Ok(None);
                    }
                }
                source = e.source();
            }
            Err(format!("Can't read League's live data: {t}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riot_root_certificate_loads() {
        assert_eq!(riot_roots().len(), 1);
    }

    // what the endpoint answers during the loading screen
    #[test]
    fn an_error_reply_is_no_match() {
        assert!(is_error_reply(r#"{"errorCode":"RESOURCE_NOT_FOUND","httpStatus":404,"message":"..."}"#));
        assert!(!is_error_reply(r#"{"activePlayer":{"riotId":"Kai#EUW"}}"#));
    }

}
