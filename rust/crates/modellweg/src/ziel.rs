//! The target (contract 8, section 6): the base address of a call, resolved once at the start, and the connection to it
//! (only host and port of the call, only the addresses of the start, deadline for connecting and TLS).

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::tls::Tls;

/// A stream to the target: TCP or TLS over TCP.
pub trait LeseSchreib: Read + Write + Send {}
impl<T: Read + Write + Send> LeseSchreib for T {}

/// The base address of a target, `https://<host>[:<port>][<praefix>]/v1` or `http://127.0.0.1…/v1` and `http://[::1]…/v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ziel {
    /// `https` (true) or `http` on the loopback (false).
    pub tls: bool,
    /// The host as written: a DNS name, `127.0.0.1` or `[::1]`.
    pub host: String,
    /// The port, if the address names one.
    pub port: Option<u16>,
    /// The path up to and including `/v1`.
    pub praefix: String,
}

impl Ziel {
    /// Reads a base address; `None` if it breaks the form of section 9.
    #[must_use]
    pub fn lies(s: &str) -> Option<Self> {
        let (tls, rest) = match s.strip_prefix("https://") {
            Some(rest) => (true, rest),
            None => (false, s.strip_prefix("http://")?),
        };
        let (host, rest) = if tls {
            let ende = rest.find([':', '/']).unwrap_or(rest.len());
            let host = &rest[..ende];
            if !hostname_ok(host) {
                return None;
            }
            (host, &rest[ende..])
        } else {
            ["127.0.0.1", "[::1]"]
                .into_iter()
                .find_map(|h| rest.strip_prefix(h).map(|r| (h, r)))?
        };
        let (port, pfad) = match rest.strip_prefix(':') {
            Some(r) => {
                let ende = r.find('/').unwrap_or(r.len());
                (Some(port_lies(&r[..ende])?), &r[ende..])
            }
            None => (None, rest),
        };
        praefix_ok(pfad).then(|| Self {
            tls,
            host: host.to_owned(),
            port,
            praefix: pfad.to_owned(),
        })
    }

    /// The port to connect to: the given one, else 443 for `https` and 80 for `http`.
    #[must_use]
    pub fn port_oder_vorgabe(&self) -> u16 {
        self.port.unwrap_or(if self.tls { 443 } else { 80 })
    }

    /// The value of the field `Host` towards the target: host, and the port only when the address names it.
    #[must_use]
    pub fn host_kopf(&self) -> String {
        match self.port {
            Some(port) => format!("{}:{port}", self.host),
            None => self.host.clone(),
        }
    }

    /// The loopback address of an `http` target (no resolution).
    fn loopback(&self) -> Option<IpAddr> {
        match self.host.as_str() {
            _ if self.tls => None,
            "[::1]" => Some(IpAddr::V6(Ipv6Addr::LOCALHOST)),
            _ => Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        }
    }
}

/// A DNS name of lower-case labels `[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?`, at most 253 bytes.
fn hostname_ok(host: &str) -> bool {
    host.len() <= 253
        && host.split('.').all(|label| {
            let b = label.as_bytes();
            let rand =
                |c: Option<&u8>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
            b.len() <= 63
                && rand(b.first())
                && rand(b.last())
                && b.iter()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        })
}

/// A port 1 to 65535 without a leading zero.
fn port_lies(text: &str) -> Option<u16> {
    let b = text.as_bytes();
    let ziffern = (1..=5).contains(&b.len()) && b[0] != b'0' && b.iter().all(u8::is_ascii_digit);
    if !ziffern {
        return None;
    }
    text.parse().ok()
}

/// A path of segments `/[A-Za-z0-9._~-]+` that ends with `/v1`.
fn praefix_ok(pfad: &str) -> bool {
    pfad.ends_with("/v1")
        && pfad.strip_prefix('/').is_some_and(|rest| {
            rest.split('/').all(|teil| {
                !teil.is_empty()
                    && teil
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"._~-".contains(&c))
            })
        })
}

/// A resolver: host and port to addresses (in production [`system_aufloeser`]). `Send` and `Sync`, because mode 2
/// resolves an unbound target only on its first CONNECT, in the thread of that connection ([`SpaeterVerbinder`]).
pub type Aufloeser = dyn Fn(&str, u16) -> io::Result<Vec<SocketAddr>> + Send + Sync;

/// A resolver as the start takes it, shared with the connectors of mode 2.
pub fn geteilt(
    f: impl Fn(&str, u16) -> io::Result<Vec<SocketAddr>> + Send + Sync + 'static,
) -> std::sync::Arc<Aufloeser> {
    std::sync::Arc::new(f)
}

/// Step 4 of the start: the addresses of the target, resolved once; `None` is `mw.ziel_unaufloesbar`.
/// `127.0.0.1` and `[::1]` are not resolved.
#[must_use]
pub fn aufloesen(ziel: &Ziel, aufloeser: &Aufloeser) -> Option<Vec<SocketAddr>> {
    let port = ziel.port_oder_vorgabe();
    if let Some(ip) = ziel.loopback() {
        return Some(vec![SocketAddr::new(ip, port)]);
    }
    aufloeser(&ziel.host, port)
        .ok()
        .filter(|adressen| !adressen.is_empty())
}

/// The resolver of the system (`getaddrinfo`).
///
/// # Errors
/// The error of the system.
pub fn system_aufloeser(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    Ok((host, port).to_socket_addrs()?.collect())
}

/// An open connection to the target and a way to tear it down from another thread (end of the process).
pub struct Verbindung {
    /// The stream (TCP or TLS).
    pub strom: Box<dyn LeseSchreib>,
    /// Shuts the connection down; reads and writes on `strom` then fail.
    pub abreissen: Box<dyn Fn() + Send + Sync>,
    /// Sets the deadline of each read (mode 2 polls both ways of a switched connection).
    pub lesefrist: Box<dyn Fn(Option<Duration>) + Send + Sync>,
}

/// Opens a connection to the target. `None` is `mw.ziel_nicht_erreichbar`.
pub trait Verbinder: Send + Sync {
    /// One new connection for one request.
    fn verbinde(&self) -> Option<Verbindung>;

    /// Whether the target has addresses (stage 6 of mode 2 at the CONNECT). A connector made from resolved addresses
    /// always has them.
    fn erreichbar(&self) -> bool {
        true
    }
}

/// Mode 2, step 6 (section 13.5): the connector of an unbound target that resolves its host once, at the first need
/// and not before readiness. A name that cannot be resolved, for example behind a blocked network, therefore never
/// holds up the start; the first CONNECT to it gets `mw.ziel_nicht_erreichbar`. The first result counts for the whole
/// session, a failure as well: one resolution per host, as before. The resolution waits at most the deadline for
/// connecting; a resolver still running after it ends on its own, and its late result is dropped.
pub struct SpaeterVerbinder {
    host: String,
    port: u16,
    aufloeser: std::sync::Arc<Aufloeser>,
    tls: std::sync::Mutex<Option<Box<dyn Tls>>>,
    fristen: Fristen,
    netz: std::sync::OnceLock<Option<NetzVerbinder>>,
}

impl SpaeterVerbinder {
    /// A connector for `host` and `port` that resolves with `aufloeser` at the first need; `tls` for an `https` target.
    #[must_use]
    pub fn neu(
        host: String,
        port: u16,
        aufloeser: std::sync::Arc<Aufloeser>,
        tls: Option<Box<dyn Tls>>,
        fristen: Fristen,
    ) -> Self {
        Self {
            host,
            port,
            aufloeser,
            tls: std::sync::Mutex::new(tls),
            fristen,
            netz: std::sync::OnceLock::new(),
        }
    }

    /// The connector of the resolved addresses; resolves on the first call only. `None` if the host has no address.
    fn netz(&self) -> Option<&NetzVerbinder> {
        self.netz
            .get_or_init(|| {
                let adressen = aufloesen_bis(
                    &self.aufloeser,
                    &self.host,
                    self.port,
                    self.fristen.verbinden,
                )?;
                let tls = self.tls.lock().ok()?.take();
                Some(NetzVerbinder::neu(
                    adressen,
                    self.host.clone(),
                    tls,
                    self.fristen,
                ))
            })
            .as_ref()
    }
}

impl Verbinder for SpaeterVerbinder {
    fn verbinde(&self) -> Option<Verbindung> {
        self.netz()?.verbinde()
    }

    fn erreichbar(&self) -> bool {
        self.netz().is_some()
    }
}

/// One resolution of `host` and `port` in a thread of its own, waited for at most `frist`. `None` without an address,
/// with an error of the resolver or after the deadline.
fn aufloesen_bis(
    aufloeser: &std::sync::Arc<Aufloeser>,
    host: &str,
    port: u16,
    frist: Duration,
) -> Option<Vec<SocketAddr>> {
    let (senden, empfangen) = std::sync::mpsc::channel();
    let aufloeser = std::sync::Arc::clone(aufloeser);
    let host = host.to_owned();
    std::thread::Builder::new()
        .spawn(move || {
            let _ = senden.send(aufloeser(&host, port));
        })
        .ok()?;
    empfangen
        .recv_timeout(frist)
        .ok()?
        .ok()
        .filter(|adressen| !adressen.is_empty())
}

/// The deadlines of section 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fristen {
    /// Connecting and TLS together.
    pub verbinden: Duration,
    /// Each read and write after the connection, so the head of the answer after sending.
    pub kopf: Duration,
}

/// The deadlines of the contract: 10 s for connecting and TLS, 600 s for the head.
pub const FRISTEN: Fristen = Fristen {
    verbinden: Duration::from_secs(10),
    kopf: Duration::from_secs(600),
};

/// The connector of the program: only the addresses of the start, TLS with the truststore of the system for `https`.
pub struct NetzVerbinder {
    adressen: Vec<SocketAddr>,
    host: String,
    tls: Option<Box<dyn Tls>>,
    fristen: Fristen,
}

impl NetzVerbinder {
    /// A connector for the resolved addresses of `host`; `tls` for an `https` target.
    #[must_use]
    pub fn neu(
        adressen: Vec<SocketAddr>,
        host: String,
        tls: Option<Box<dyn Tls>>,
        fristen: Fristen,
    ) -> Self {
        Self {
            adressen,
            host,
            tls,
            fristen,
        }
    }
}

impl Verbinder for NetzVerbinder {
    fn verbinde(&self) -> Option<Verbindung> {
        let ende = Instant::now() + self.fristen.verbinden;
        let rest = || {
            ende.checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
        };
        let tcp = self
            .adressen
            .iter()
            .find_map(|adresse| TcpStream::connect_timeout(adresse, rest()?).ok())?;
        let frist = rest()?;
        tcp.set_read_timeout(Some(frist)).ok()?;
        tcp.set_write_timeout(Some(frist)).ok()?;
        let griff = tcp.try_clone().ok()?;
        let fristgriff = tcp.try_clone().ok()?;
        let strom: Box<dyn LeseSchreib> = match &self.tls {
            Some(tls) => tls.umhuellen(tcp, &self.host).ok()?,
            None => Box::new(tcp),
        };
        griff.set_read_timeout(Some(self.fristen.kopf)).ok()?;
        griff.set_write_timeout(Some(self.fristen.kopf)).ok()?;
        Some(Verbindung {
            strom,
            abreissen: Box::new(move || {
                let _ = griff.shutdown(Shutdown::Both);
            }),
            lesefrist: Box::new(move |frist| {
                let _ = fristgriff.set_read_timeout(frist);
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Ziel, aufloesen, hostname_ok, port_lies, praefix_ok};
    use std::io;
    use std::net::SocketAddr;

    #[test]
    fn a_target_is_read_into_its_parts() {
        let z = Ziel::lies("https://a.b-c.d:8443/x/y/v1").expect("valid");
        assert!(z.tls);
        assert_eq!(z.host, "a.b-c.d");
        assert_eq!(z.port, Some(8443));
        assert_eq!(z.praefix, "/x/y/v1");
        assert_eq!(z.host_kopf(), "a.b-c.d:8443");
        assert_eq!(z.port_oder_vorgabe(), 8443);
        let z = Ziel::lies("https://a/v1").expect("valid");
        assert_eq!((z.host_kopf().as_str(), z.port_oder_vorgabe()), ("a", 443));
        let z = Ziel::lies("http://127.0.0.1/v1").expect("valid");
        assert!(!z.tls);
        assert_eq!(
            (z.host_kopf().as_str(), z.port_oder_vorgabe()),
            ("127.0.0.1", 80)
        );
        let z = Ziel::lies("http://[::1]:1/v1").expect("valid");
        assert_eq!(z.host_kopf(), "[::1]:1");
    }

    #[test]
    fn wrong_targets_are_refused() {
        for s in [
            "ftp://a/v1",
            "https://A/v1",
            "https:/a/v1",
            "http://localhost/v1",
            "http://127.0.0.10/v1",
            "http://[::2]/v1",
            "https://u@a/v1",
            "https://a/v1?x",
            "https://a/v1/",
            "https://a/v2",
            "https://a:0/v1",
            "https://a:080/v1",
            "https://a:65536/v1",
            "https://a:/v1",
            "https://a//v1",
            "https:///v1",
            "https://a/b%20/v1",
        ] {
            assert_eq!(Ziel::lies(s), None, "{s}");
        }
        assert!(Ziel::lies("https://a:65535/v1").is_some());
        assert!(Ziel::lies("https://a:1/~._-A9/v1").is_some());
    }

    #[test]
    fn a_host_name_has_limits() {
        let label = "a".repeat(63);
        assert!(hostname_ok(&label));
        assert!(!hostname_ok(&format!("{label}a")));
        let lang = [label.as_str(); 4].join(".");
        assert_eq!(lang.len(), 255);
        assert!(!hostname_ok(&lang));
        assert!(hostname_ok(&lang[2..]));
        assert!(!hostname_ok(&lang[1..]));
        assert!(hostname_ok("a-b"));
        assert!(!hostname_ok("-a"));
        assert!(!hostname_ok("a-"));
        assert!(!hostname_ok("a..b"));
        assert!(!hostname_ok("a_b"));
        assert!(hostname_ok("0.9"));
    }

    #[test]
    fn a_port_has_limits() {
        assert_eq!(port_lies("1"), Some(1));
        assert_eq!(port_lies("65535"), Some(65535));
        assert_eq!(port_lies("65536"), None);
        assert_eq!(port_lies("099"), None);
        assert_eq!(port_lies("123456"), None);
        assert_eq!(port_lies(""), None);
        assert_eq!(port_lies("1a"), None);
    }

    #[test]
    fn a_prefix_ends_with_v1() {
        assert!(praefix_ok("/v1"));
        assert!(praefix_ok("/a/v1"));
        assert!(!praefix_ok("v1"));
        assert!(!praefix_ok("/a/v1/"));
        assert!(!praefix_ok("/a b/v1"));
        assert!(!praefix_ok("/xv1"));
    }

    #[test]
    fn loopback_targets_are_not_resolved() {
        let nie = |_: &str, _: u16| -> io::Result<Vec<SocketAddr>> { panic!("resolved") };
        let z = Ziel::lies("http://[::1]:5/v1").expect("valid");
        assert_eq!(
            aufloesen(&z, &nie),
            Some(vec!["[::1]:5".parse().expect("addr")])
        );
        let z = Ziel::lies("http://127.0.0.1/v1").expect("valid");
        assert_eq!(
            aufloesen(&z, &nie),
            Some(vec!["127.0.0.1:80".parse().expect("addr")])
        );
        let z = Ziel::lies("https://a/v1").expect("valid");
        let leer = |_: &str, _: u16| -> io::Result<Vec<SocketAddr>> { Ok(Vec::new()) };
        assert_eq!(aufloesen(&z, &leer), None);
        let fehler =
            |_: &str, _: u16| -> io::Result<Vec<SocketAddr>> { Err(io::Error::other("x")) };
        assert_eq!(aufloesen(&z, &fehler), None);
        let gut = |h: &str, p: u16| -> io::Result<Vec<SocketAddr>> {
            assert_eq!((h, p), ("a", 443));
            Ok(vec!["10.0.0.1:443".parse().expect("addr")])
        };
        assert_eq!(
            aufloesen(&z, &gut),
            Some(vec!["10.0.0.1:443".parse().expect("addr")])
        );
    }
}
