//! One connection of a browser: read the head and the body within the time limit, check in the fixed order, answer. One
//! request per connection (`Connection: close`, no keep-alive, no pipelining).

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};

use crate::sys::UnixStream;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::dienst;
use crate::fehler::{Abbruch, Abgelehnt, Antwort};
use crate::form::{Anfrage, MAX_KOPF};
use crate::pruefung::{self, Route};
use crate::seite;
use crate::startdatei::Config;
use crate::token;

/// The sink of the protocol lines: gets the reason word of a rejected request.
pub type Protokoll = Arc<dyn Fn(&str) + Send + Sync>;

/// What all connections share.
pub(crate) struct Gemeinsam {
    pub(crate) cfg: Config,
    pub(crate) token: Vec<u8>,
    pub(crate) port: u16,
    pub(crate) protokoll: Protokoll,
    /// The token file and the port file this bridge made, removed at the end through their folders (`rechte.rs`).
    pub(crate) dateien: Vec<crate::rechte::Angelegt>,
    /// The open streams: identifier to the writing end of the connection to the service.
    pub(crate) strome: Mutex<HashMap<String, Arc<Mutex<UnixStream>>>>,
    /// The open connections of browsers (a clone of each), counted against the limit and shut down at the end.
    pub(crate) offen: Mutex<HashMap<u64, TcpStream>>,
    pub(crate) naechste: AtomicU64,
}

/// A lock that a panicking thread did not make useless.
pub(crate) fn sperre<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

const ZEIT: Abgelehnt = Abgelehnt {
    status: 408,
    grund: "zeit",
};
const DIENST: Abgelehnt = Abgelehnt {
    status: 502,
    grund: "dienst",
};

/// Reads a chunk within `ende`. `Err(ZEIT)` when the time is over, `Stumm` when the peer left.
fn lesen_bis(s: &mut TcpStream, puffer: &mut [u8], ende: Instant) -> Result<usize, Abbruch> {
    let rest = ende.saturating_duration_since(Instant::now());
    if rest.is_zero() {
        return Err(ZEIT.into());
    }
    // see dienst.rs: a refused timeout is no reason to stop (the read gives the data or the end)
    let _ = s.set_read_timeout(Some(rest));
    match s.read(puffer) {
        Ok(0) => Err(Abbruch::Stumm),
        Ok(n) => Ok(n),
        Err(e) => match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => Err(ZEIT.into()),
            _ => Err(Abbruch::Stumm),
        },
    }
}

/// Reads until the end of the head. Returns the head (without the closing empty line) and the bytes already read behind it.
pub(crate) fn kopf_lesen(
    s: &mut TcpStream,
    frist: Duration,
) -> Result<(Vec<u8>, Vec<u8>), Abbruch> {
    let ende = Instant::now() + frist;
    let mut buf: Vec<u8> = Vec::new();
    let mut stueck = [0u8; 4096];
    let ende_kopf = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
        if buf.len() > MAX_KOPF {
            return Err(Abgelehnt {
                status: 431,
                grund: "kopf_zu_gross",
            }
            .into());
        }
        let n = lesen_bis(s, &mut stueck, ende)?;
        buf.extend_from_slice(&stueck[..n]);
    };
    if ende_kopf + 4 > MAX_KOPF {
        return Err(Abgelehnt {
            status: 431,
            grund: "kopf_zu_gross",
        }
        .into());
    }
    let rest = buf.split_off(ende_kopf + 4);
    buf.truncate(ende_kopf);
    Ok((buf, rest))
}

/// Reads `n` bytes of the body; `rest` holds what the head read already took. The time runs from the call.
pub(crate) fn koerper_lesen(
    s: &mut TcpStream,
    rest: &[u8],
    n: usize,
    frist: Duration,
) -> Result<Vec<u8>, Abbruch> {
    let ende = Instant::now() + frist;
    let mut buf: Vec<u8> = rest.iter().copied().take(n).collect();
    let mut stueck = vec![0u8; 65_536];
    while buf.len() < n {
        let moeglich = (n - buf.len()).min(stueck.len());
        let gelesen = lesen_bis(s, &mut stueck[..moeglich], ende)?;
        buf.extend_from_slice(&stueck[..gelesen]);
    }
    Ok(buf)
}

/// Handles one request. `Ok(Some(answer))` is sent by the caller; `Ok(None)` means the answer was a stream that is over.
pub(crate) fn bedienen(g: &Gemeinsam, s: &mut TcpStream) -> Result<Option<Antwort>, Abbruch> {
    let (kopf, rest) = kopf_lesen(s, g.cfg.lese_frist)?;
    let (anfrage, route) = pruefung::pruefen(&kopf, g.port, &g.token)?;
    match route {
        Route::Seite => Ok(Some(seiten_antwort(
            seite::HTML,
            "text/html; charset=utf-8",
        ))),
        Route::Skript => Ok(Some(seiten_antwort(
            seite::SKRIPT,
            "text/javascript; charset=utf-8",
        ))),
        Route::Wasm => Ok(Some(seiten_antwort(seite::WASM, "application/wasm"))),
        Route::Strom => {
            strom(g, s)?;
            Ok(None)
        }
        Route::Starten | Route::Eingabe => {
            let n = pruefung::post_pruefen(&anfrage, route)?;
            let koerper = koerper_lesen(s, &rest, n, g.cfg.lese_frist)?;
            if route == Route::Starten {
                starten(g, &koerper).map(Some)
            } else {
                eingabe(g, &anfrage, &koerper).map(Some)
            }
        }
    }
}

fn seiten_antwort(inhalt: &[u8], typ: &'static str) -> Antwort {
    let mut a = Antwort::mit(200, typ, inhalt.to_vec());
    a.extra
        .push(("Content-Security-Policy", seite::CSP.to_string()));
    a
}

fn json_fehler() -> Abgelehnt {
    Abgelehnt {
        status: 400,
        grund: "json",
    }
}

/// `POST /starten`: the body must be JSON; the structure is written anew into the event, never as text.
fn starten(g: &Gemeinsam, koerper: &[u8]) -> Result<Antwort, Abbruch> {
    let start: Value = serde_json::from_slice(koerper).map_err(|_| json_fehler())?;
    Ok(dienst::starten(&g.cfg.dienst, &start)?)
}

/// `POST /eingabe`: the body goes unchanged as an event frame to the connection of the identifier.
fn eingabe(g: &Gemeinsam, a: &Anfrage, koerper: &[u8]) -> Result<Antwort, Abbruch> {
    let kennung = a
        .kopf("x-verbindung")
        .filter(|k| k.len() == 32 && k.iter().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .and_then(|k| std::str::from_utf8(k).ok())
        .ok_or(Abgelehnt {
            status: 400,
            grund: "kennung",
        })?;
    if !serde_json::from_slice::<Value>(koerper).is_ok_and(|v| v.is_object()) {
        return Err(json_fehler().into());
    }
    let ziel = sperre(&g.strome).get(kennung).cloned().ok_or(Abgelehnt {
        status: 404,
        grund: "kennung",
    })?;
    let mut dienst = sperre(&ziel);
    dienst
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| DIENST)?;
    dienst
        .write_all(&dienst::rahmen(dienst::ART_EREIGNIS, koerper))
        .map_err(|_| DIENST)?;
    Ok(Antwort::leer(204))
}

/// Takes the stream out of the table when it ends.
struct StromEintrag<'a> {
    g: &'a Gemeinsam,
    kennung: String,
}

impl Drop for StromEintrag<'_> {
    fn drop(&mut self) {
        sperre(&self.g.strome).remove(&self.kennung);
    }
}

/// `GET /strom`: opens a connection to the service, answers 200 with the identifier and then hands every byte of the
/// service to the browser unchanged. Ends when either side ends; then both are closed.
fn strom(g: &Gemeinsam, s: &mut TcpStream) -> Result<(), Abbruch> {
    let dienst = dienst::verbinden(&g.cfg.dienst)?;
    let kennung = token::kennung_erzeugen().map_err(|_| DIENST)?;
    let schreiber = dienst.try_clone()?;
    sperre(&g.strome).insert(kennung.clone(), Arc::new(Mutex::new(schreiber)));
    let _eintrag = StromEintrag {
        g,
        kennung: kennung.clone(),
    };
    let kopf = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nX-Verbindung: {kennung}\r\nX-Deflate: {}\r\nConnection: close\r\n\r\n",
        u8::from(g.cfg.deflate)
    );
    s.set_write_timeout(Some(Duration::from_secs(10)))?;
    s.write_all(kopf.as_bytes())?;
    s.set_write_timeout(None)?;
    // the stream has no read limit after the head (the clone of the watcher shares this socket)
    s.set_read_timeout(None)?;
    pumpen(s, dienst)
}

fn pumpen(s: &mut TcpStream, mut dienst: UnixStream) -> Result<(), Abbruch> {
    let mut beobachter = s.try_clone()?;
    let dienst_zu = dienst.try_clone()?;
    // the browser side: any byte is ignored; its end or error ends the stream
    thread::Builder::new().stack_size(262_144).spawn(move || {
        let mut sink = [0u8; 256];
        while matches!(beobachter.read(&mut sink), Ok(n) if n > 0) {}
        let _ = dienst_zu.shutdown(Shutdown::Both);
    })?;
    let mut puffer = vec![0u8; 65_536];
    while let Ok(n) = dienst.read(&mut puffer) {
        if n == 0 || s.write_all(&puffer[..n]).is_err() {
            break;
        }
    }
    let _ = s.shutdown(Shutdown::Both);
    let _ = dienst.shutdown(Shutdown::Both);
    Ok(())
}
