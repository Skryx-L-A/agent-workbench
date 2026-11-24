//! TLS to the target (contract 8, section 6): the truststore of the system, never a fixed CA bundle; host name and chain
//! are checked against the host of the target. The rest of the crate sees only the trait [`Tls`], so the library below can
//! change without touching the way to the target.

use std::io;
use std::net::TcpStream;

use crate::ziel::LeseSchreib;

/// Wraps an open TCP connection into TLS for a host.
pub trait Tls: Send + Sync {
    /// The handshake on `tcp` with the name `host` (SNI, check of name and chain).
    ///
    /// # Errors
    /// Any failure of the handshake or of the check.
    fn umhuellen(&self, tcp: TcpStream, host: &str) -> io::Result<Box<dyn LeseSchreib>>;
}

/// TLS of the system through `native-tls`: OpenSSL with its default trust paths on Linux, Security.framework on macOS.
pub struct SystemTls {
    verbinder: native_tls::TlsConnector,
}

impl SystemTls {
    /// A connector with the trust of the system and at least TLS 1.2.
    ///
    /// # Errors
    /// The library cannot build a connector.
    pub fn neu() -> io::Result<Self> {
        native_tls::TlsConnector::builder()
            .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
            .build()
            .map(|verbinder| Self { verbinder })
            .map_err(io::Error::other)
    }
}

impl Tls for SystemTls {
    fn umhuellen(&self, tcp: TcpStream, host: &str) -> io::Result<Box<dyn LeseSchreib>> {
        match self.verbinder.connect(host, tcp) {
            Ok(strom) => Ok(Box::new(strom)),
            Err(_) => Err(io::Error::other("tls")),
        }
    }
}
