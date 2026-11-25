//! The way to a model target or an MCP server: TCP, TLS with the truststore of the system (plain TCP only for a loopback
//! host), one HTTP/1.1 request, the head of the answer and its body (length, chunked, or up to the close).
//!
//! Deadlines (section 3, addendum B05): connection and TLS within 10 s, the head of the answer within 600 s after the
//! request is sent, then no deadline while bytes flow. A broken deadline is `halter.ziel_fehler`.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::antwort::ANTWORTKOPF_VERBOTEN;
use crate::code::Code;
use crate::form::{KOPF_MAX, KOPFWERT_MAX};
use crate::konfig::Adresse;

/// Deadline for connection and TLS.
pub const FRIST_VERBINDEN: Duration = Duration::from_secs(10);
/// Deadline for the head of the answer.
pub const FRIST_KOPF: Duration = Duration::from_secs(600);
/// Largest head of an answer in bytes.
pub const KOPF_BYTES_MAX: usize = 1 << 20;

/// A connection to a target: the bytes and the TCP socket below them (for deadlines and polling).
pub trait Leitung: Read + Write + Send {
    /// The TCP socket.
    fn tcp(&self) -> &TcpStream;
}

impl Leitung for TcpStream {
    fn tcp(&self) -> &TcpStream {
        self
    }
}

impl Leitung for native_tls::TlsStream<TcpStream> {
    fn tcp(&self) -> &TcpStream {
        self.get_ref()
    }
}

/// Opens a connection to `a` within [`FRIST_VERBINDEN`].
///
/// # Errors
/// `halter.ziel_fehler` for every failure (name, connection, TLS, deadline).
pub fn verbinden(a: &Adresse) -> Result<Box<dyn Leitung>, Code> {
    let beginn = Instant::now();
    let adressen = (a.host.as_str(), a.port)
        .to_socket_addrs()
        .map_err(|_| Code::ZielFehler)?;
    let mut tcp = None;
    for adr in adressen {
        let rest = FRIST_VERBINDEN.saturating_sub(beginn.elapsed());
        if rest.is_zero() {
            break;
        }
        if let Ok(t) = TcpStream::connect_timeout(&adr, rest) {
            tcp = Some(t);
            break;
        }
    }
    let tcp = tcp.ok_or(Code::ZielFehler)?;
    let rest = FRIST_VERBINDEN
        .saturating_sub(beginn.elapsed())
        .max(Duration::from_millis(1));
    tcp.set_read_timeout(Some(rest))
        .map_err(|_| Code::ZielFehler)?;
    tcp.set_write_timeout(Some(rest))
        .map_err(|_| Code::ZielFehler)?;
    let _ = tcp.set_nodelay(true);
    if !a.tls {
        return Ok(Box::new(tcp));
    }
    let verbinder = native_tls::TlsConnector::builder()
        .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
        .build()
        .map_err(|_| Code::ZielFehler)?;
    let strom = verbinder
        .connect(&a.host, tcp)
        .map_err(|_| Code::ZielFehler)?;
    Ok(Box::new(strom))
}

/// The bytes of a request. `kopf` comes from the bridge (checked, no forbidden name); the holder adds `host`,
/// `accept-encoding: identity`, the key head and either `content-length` with `connection: close` or the upgrade.
#[must_use]
pub fn anfrage(
    methode: &str,
    ziel: &str,
    a: &Adresse,
    kopf: &Map<String, Value>,
    schluessel: &[u8],
    koerper: Option<&[u8]>,
    upgrade: bool,
) -> Vec<u8> {
    let mut b = Vec::with_capacity(512 + koerper.map_or(0, <[u8]>::len));
    b.extend_from_slice(format!("{methode} {ziel} HTTP/1.1\r\n").as_bytes());
    let host = if a.host.contains(':') {
        format!("[{}]", a.host)
    } else {
        a.host.clone()
    };
    let standard = if a.tls { 443 } else { 80 };
    if a.port == standard {
        b.extend_from_slice(format!("host: {host}\r\n").as_bytes());
    } else {
        b.extend_from_slice(format!("host: {host}:{}\r\n", a.port).as_bytes());
    }
    for (name, wert) in kopf {
        if let Some(w) = wert.as_str() {
            b.extend_from_slice(format!("{name}: {w}\r\n").as_bytes());
        }
    }
    b.extend_from_slice(b"accept-encoding: identity\r\n");
    b.extend_from_slice(a.kopf.as_bytes());
    b.extend_from_slice(b": ");
    b.extend_from_slice(a.vorsatz.as_bytes());
    b.extend_from_slice(schluessel);
    b.extend_from_slice(b"\r\n");
    if upgrade {
        b.extend_from_slice(b"upgrade: websocket\r\nconnection: upgrade\r\n");
    } else {
        let n = koerper.map_or(0, <[u8]>::len);
        if methode == "POST" || n > 0 {
            b.extend_from_slice(format!("content-length: {n}\r\n").as_bytes());
        }
        b.extend_from_slice(b"connection: close\r\n");
    }
    b.extend_from_slice(b"\r\n");
    if let Some(k) = koerper {
        b.extend_from_slice(k);
    }
    b
}

/// The head of an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Antwortkopf {
    /// Status code.
    pub status: u16,
    /// Every head line, name in lower case, value without the space around it.
    pub zeilen: Vec<(String, String)>,
}

impl Antwortkopf {
    /// The first value of a head.
    #[must_use]
    pub fn wert(&self, name: &str) -> Option<&str> {
        self.zeilen
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, w)| w.as_str())
    }

    /// `true` if the head names this head at all.
    #[must_use]
    pub fn hat(&self, name: &str) -> bool {
        self.zeilen.iter().any(|(n, _)| n == name)
    }

    /// `halter.ziel_komprimiert` if `content-encoding` names anything but `identity` (section 4).
    #[must_use]
    pub fn komprimiert(&self) -> bool {
        self.zeilen
            .iter()
            .filter(|(n, _)| n == "content-encoding")
            .any(|(_, w)| {
                w.split(',')
                    .any(|t| !t.trim().eq_ignore_ascii_case("identity") && !t.trim().is_empty())
            })
    }

    /// The heads for `beginn`: all but the forbidden list, at most 64, each value at most 8192 printable characters;
    /// several lines of one name are joined with `, `.
    ///
    /// # Errors
    /// `halter.ziel_fehler` for too many or too long heads or a name or value outside the form.
    pub fn weiter(&self) -> Result<Map<String, Value>, Code> {
        let mut m: Map<String, Value> = Map::new();
        for (n, w) in &self.zeilen {
            if ANTWORTKOPF_VERBOTEN.contains(&n.as_str()) {
                continue;
            }
            let name_ok = (1..=64).contains(&n.len())
                && n.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
            if !name_ok || !w.bytes().all(|c| (0x20..=0x7e).contains(&c)) {
                return Err(Code::ZielFehler);
            }
            let neu = match m.get(n).and_then(Value::as_str) {
                Some(alt) => format!("{alt}, {w}"),
                None => w.clone(),
            };
            if neu.len() > KOPFWERT_MAX {
                return Err(Code::ZielFehler);
            }
            m.insert(n.clone(), Value::String(neu));
        }
        if m.len() > KOPF_MAX {
            return Err(Code::ZielFehler);
        }
        Ok(m)
    }
}

/// Reads the head of an answer within `frist`; returns the head and the bytes read after it.
///
/// # Errors
/// `halter.ziel_fehler` for a deadline, a close, a head over [`KOPF_BYTES_MAX`] or a head that is not HTTP/1.x.
pub fn kopf_lesen(l: &mut dyn Leitung, frist: Duration) -> Result<(Antwortkopf, Vec<u8>), Code> {
    let ende = Instant::now() + frist;
    let mut b = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if let Some(pos) = b.windows(4).position(|f| f == b"\r\n\r\n") {
            let rest = b.split_off(pos + 4);
            return Ok((kopf_zerlegen(&b[..pos])?, rest));
        }
        if b.len() > KOPF_BYTES_MAX {
            return Err(Code::ZielFehler);
        }
        let rest = ende.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            return Err(Code::ZielFehler);
        }
        l.tcp()
            .set_read_timeout(Some(rest))
            .map_err(|_| Code::ZielFehler)?;
        match l.read(&mut buf) {
            Ok(0) | Err(_) => return Err(Code::ZielFehler),
            Ok(n) => b.extend_from_slice(&buf[..n]),
        }
    }
}

fn kopf_zerlegen(b: &[u8]) -> Result<Antwortkopf, Code> {
    let text = std::str::from_utf8(b).map_err(|_| Code::ZielFehler)?;
    let mut zeilen = text.split("\r\n");
    let status_zeile = zeilen.next().ok_or(Code::ZielFehler)?;
    let mut teile = status_zeile.splitn(3, ' ');
    let fassung = teile.next().unwrap_or("");
    if fassung != "HTTP/1.1" && fassung != "HTTP/1.0" {
        return Err(Code::ZielFehler);
    }
    let status: u16 = teile
        .next()
        .filter(|s| s.len() == 3 && s.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|s| s.parse().ok())
        .filter(|s| (100..=599).contains(s))
        .ok_or(Code::ZielFehler)?;
    let mut aus = Vec::new();
    for z in zeilen {
        let (n, w) = z.split_once(':').ok_or(Code::ZielFehler)?;
        if n.is_empty() || n.bytes().any(|c| c <= b' ' || c >= 0x7f) {
            return Err(Code::ZielFehler);
        }
        aus.push((
            n.to_ascii_lowercase(),
            w.trim_matches([' ', '\t']).to_owned(),
        ));
    }
    Ok(Antwortkopf {
        status,
        zeilen: aus,
    })
}

enum Art {
    Laenge(u64),
    Stueckweise(Stueck),
    BisEnde,
}

enum Stueck {
    Groesse(Vec<u8>),
    Daten(u64),
    DatenEnde(usize),
    Anhang(Vec<u8>),
    Fertig,
}

/// The body of an answer, decoded (length, chunked or up to the close).
pub struct Koerper<'a> {
    leitung: &'a mut dyn Leitung,
    vorrat: Vec<u8>,
    art: Art,
    fertig: bool,
}

impl<'a> Koerper<'a> {
    /// The body after `kopf`; `vorrat` are the bytes read after the head.
    ///
    /// # Errors
    /// `halter.ziel_fehler` for a `content-length` that is not a number or a transfer coding other than `chunked`.
    pub fn neu(
        leitung: &'a mut dyn Leitung,
        kopf: &Antwortkopf,
        vorrat: Vec<u8>,
        methode: &str,
    ) -> Result<Self, Code> {
        let ohne =
            methode == "HEAD" || kopf.status == 204 || kopf.status == 304 || kopf.status < 200;
        let art = if ohne {
            Art::Laenge(0)
        } else if let Some(te) = kopf.wert("transfer-encoding") {
            if !te.eq_ignore_ascii_case("chunked") {
                return Err(Code::ZielFehler);
            }
            Art::Stueckweise(Stueck::Groesse(Vec::new()))
        } else if let Some(cl) = kopf.wert("content-length") {
            Art::Laenge(cl.trim().parse().map_err(|_| Code::ZielFehler)?)
        } else {
            Art::BisEnde
        };
        // no deadline while bytes flow
        leitung
            .tcp()
            .set_read_timeout(None)
            .map_err(|_| Code::ZielFehler)?;
        Ok(Koerper {
            leitung,
            vorrat,
            art,
            fertig: false,
        })
    }

    fn roh(&mut self) -> Result<Option<Vec<u8>>, Code> {
        if !self.vorrat.is_empty() {
            return Ok(Some(std::mem::take(&mut self.vorrat)));
        }
        let mut buf = vec![0u8; 65_536];
        loop {
            match self.leitung.read(&mut buf) {
                Ok(0) => return Ok(None),
                Ok(n) => {
                    buf.truncate(n);
                    return Ok(Some(buf));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return Err(Code::ZielFehler),
            }
        }
    }

    /// The next decoded bytes, `None` at the end of the body.
    ///
    /// # Errors
    /// `halter.ziel_fehler` for a read error, a body cut before its length or a broken chunk.
    pub fn naechstes(&mut self) -> Result<Option<Vec<u8>>, Code> {
        loop {
            if self.fertig {
                return Ok(None);
            }
            match &mut self.art {
                Art::Laenge(0) => {
                    self.fertig = true;
                }
                Art::Laenge(rest) => {
                    let rest_n = *rest;
                    let Some(mut b) = self.roh()? else {
                        return Err(Code::ZielFehler);
                    };
                    if b.len() as u64 > rest_n {
                        b.truncate(usize::try_from(rest_n).unwrap_or(usize::MAX));
                    }
                    if let Art::Laenge(r) = &mut self.art {
                        *r -= b.len() as u64;
                    }
                    return Ok(Some(b));
                }
                Art::BisEnde => match self.roh()? {
                    Some(b) => return Ok(Some(b)),
                    None => self.fertig = true,
                },
                Art::Stueckweise(_) => {
                    let Some(b) = self.roh()? else {
                        return Err(Code::ZielFehler);
                    };
                    let (aus, rest) = self.stuecke(&b)?;
                    self.vorrat = rest;
                    if !aus.is_empty() {
                        return Ok(Some(aus));
                    }
                }
            }
        }
    }

    /// Decodes chunked bytes; returns the data and what is left after the end.
    fn stuecke(&mut self, mut b: &[u8]) -> Result<(Vec<u8>, Vec<u8>), Code> {
        let mut aus = Vec::new();
        let Art::Stueckweise(st) = &mut self.art else {
            return Err(Code::Intern);
        };
        while !b.is_empty() {
            match st {
                Stueck::Groesse(z) => {
                    let c = b[0];
                    b = &b[1..];
                    if c == b'\n' {
                        let zeile = std::str::from_utf8(z).map_err(|_| Code::ZielFehler)?;
                        let zeile = zeile.strip_suffix('\r').ok_or(Code::ZielFehler)?;
                        let hex = zeile.split(';').next().unwrap_or("").trim();
                        if hex.is_empty() || hex.len() > 15 {
                            return Err(Code::ZielFehler);
                        }
                        let n = u64::from_str_radix(hex, 16).map_err(|_| Code::ZielFehler)?;
                        *st = if n == 0 {
                            Stueck::Anhang(Vec::new())
                        } else {
                            Stueck::Daten(n)
                        };
                    } else {
                        if z.len() > 1024 {
                            return Err(Code::ZielFehler);
                        }
                        z.push(c);
                    }
                }
                Stueck::Daten(rest) => {
                    let n = usize::try_from(*rest).unwrap_or(usize::MAX).min(b.len());
                    aus.extend_from_slice(&b[..n]);
                    b = &b[n..];
                    *rest -= n as u64;
                    if *rest == 0 {
                        *st = Stueck::DatenEnde(0);
                    }
                }
                Stueck::DatenEnde(i) => {
                    let soll = if *i == 0 { b'\r' } else { b'\n' };
                    if b[0] != soll {
                        return Err(Code::ZielFehler);
                    }
                    b = &b[1..];
                    *st = if *i == 0 {
                        Stueck::DatenEnde(1)
                    } else {
                        Stueck::Groesse(Vec::new())
                    };
                }
                Stueck::Anhang(z) => {
                    z.push(b[0]);
                    b = &b[1..];
                    if z.len() > 65_536 {
                        return Err(Code::ZielFehler);
                    }
                    if z.ends_with(b"\r\n\r\n") || z.as_slice() == b"\r\n" {
                        *st = Stueck::Fertig;
                    }
                }
                Stueck::Fertig => break,
            }
        }
        if matches!(st, Stueck::Fertig) {
            self.fertig = true;
        }
        Ok((aus, b.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kopf_und_weitergabe() {
        let k = kopf_zerlegen(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nSet-Cookie: a=b\r\nX-A: 1\r\nx-a: 2\r\nConnection: close",
        )
        .expect("head");
        assert_eq!(k.status, 200);
        let w = k.weiter().expect("heads");
        assert_eq!(
            w.get("content-type").and_then(Value::as_str),
            Some("text/event-stream")
        );
        assert_eq!(w.get("x-a").and_then(Value::as_str), Some("1, 2"));
        assert!(!w.contains_key("set-cookie") && !w.contains_key("connection"));
        assert!(!k.komprimiert());
        let k = kopf_zerlegen(b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip").expect("head");
        assert!(k.komprimiert());
        let k = kopf_zerlegen(b"HTTP/1.1 200 OK\r\nContent-Encoding: identity").expect("head");
        assert!(!k.komprimiert());
        assert!(kopf_zerlegen(b"HTTP/2 200").is_err());
        assert!(kopf_zerlegen(b"HTTP/1.1 999 x").is_err());
        let lang = format!("HTTP/1.1 200 OK\r\nx: {}", "a".repeat(8193));
        assert_eq!(
            kopf_zerlegen(lang.as_bytes()).expect("head").weiter(),
            Err(Code::ZielFehler)
        );
        let viele: String = (0..65).fold(String::new(), |mut s, i| {
            use std::fmt::Write;
            let _ = write!(s, "\r\nk{i}: v");
            s
        });
        let k = kopf_zerlegen(format!("HTTP/1.1 200 OK{viele}").as_bytes()).expect("head");
        assert_eq!(k.weiter(), Err(Code::ZielFehler));
    }
}
