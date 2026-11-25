//! The bridge as a process part: start (rights, bind, token, port file), accept loop, end.
//!
//! Order at the start (BR1): rights, bind and listen, token file, port file. The address is fixed in [`binden`]; no
//! parameter and no start key reaches it.

use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::fehler::{Abbruch, Abgelehnt, Antwort, StartFehler};
use crate::rechte::{self, Angelegt};
use crate::startdatei::Config;
use crate::token;
use crate::verbindung::{self, Gemeinsam, Protokoll, sperre};

/// The stack of a thread of a connection: small, there are at most 64 connections.
const STAPEL: usize = 262_144;

/// The protocol line for a rejected request: the word and nothing else.
pub(crate) fn protokollzeile(grund: &str) -> String {
    format!("wb-bruecke: abgelehnt {grund}\n")
}

/// The sink that writes the protocol to stderr, one line per call.
#[must_use]
pub fn protokoll_stderr() -> Protokoll {
    Arc::new(|grund: &str| {
        let _ = io::stderr().write_all(protokollzeile(grund).as_bytes());
    })
}

/// The stop of the bridge: a flag, and the port to knock on so that the blocking accept wakes up.
#[derive(Debug)]
pub struct Halt {
    gestoppt: AtomicBool,
    port: AtomicU16,
}

impl Halt {
    /// A new stop, not set.
    #[must_use]
    pub fn neu() -> Self {
        Self {
            gestoppt: AtomicBool::new(false),
            port: AtomicU16::new(0),
        }
    }

    /// Is the stop set?
    pub fn ist_gestoppt(&self) -> bool {
        self.gestoppt.load(Ordering::SeqCst)
    }

    /// Sets the stop and wakes the accept loop (if it already runs on a port).
    pub fn anhalten(&self) {
        self.gestoppt.store(true, Ordering::SeqCst);
        let port = self.port.load(Ordering::SeqCst);
        if port != 0 {
            let _ = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        }
    }
}

/// Binds the listener. The address is fixed: `127.0.0.1`. There is no parameter for another one and no second listener.
pub(crate) fn binden(port: u16) -> Result<TcpListener, StartFehler> {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).map_err(|_| StartFehler {
        code: 1,
        text: "Port laesst sich nicht binden",
    })
}

/// Writes the token file and then the port file. If the port file fails, the token file this call made is removed again.
pub(crate) fn dateien_anlegen(
    cfg: &Config,
    token: &[u8],
    port: u16,
) -> Result<Vec<Angelegt>, StartFehler> {
    let mut inhalt = token.to_vec();
    inhalt.push(b'\n');
    let tokendatei = rechte::anlegen(&cfg.token_datei, &inhalt)?;
    let mut dateien = Vec::new();
    if let Some(pd) = &cfg.port_datei {
        match rechte::anlegen(pd, format!("{port}\n").as_bytes()) {
            Ok(a) => dateien.push(a),
            Err(e) => {
                tokendatei.entfernen();
                return Err(e);
            }
        }
    }
    dateien.insert(0, tokendatei);
    Ok(dateien)
}

/// A started bridge: bound, files written, not yet accepting.
pub struct Bruecke {
    listener: TcpListener,
    g: Arc<Gemeinsam>,
}

/// Starts the bridge: checks the rights, binds, makes the token and writes the files.
///
/// # Errors
///
/// Exit code 3 for rights or files, 1 for the bind or the randomness.
pub fn starten(cfg: Config, protokoll: Protokoll) -> Result<Bruecke, StartFehler> {
    rechte::pruefen(&cfg.token_datei)?;
    if let Some(pd) = &cfg.port_datei {
        rechte::pruefen(pd)?;
    }
    let listener = binden(cfg.port)?;
    let port = listener
        .local_addr()
        .map_err(|_| StartFehler {
            code: 1,
            text: "Port unbekannt",
        })?
        .port();
    let token = token::token_erzeugen().map_err(|_| StartFehler {
        code: 1,
        text: "Zufall nicht lesbar",
    })?;
    let dateien = dateien_anlegen(&cfg, &token, port)?;
    Ok(Bruecke {
        listener,
        g: Arc::new(Gemeinsam {
            cfg,
            token,
            port,
            protokoll,
            dateien,
            strome: Mutex::new(HashMap::new()),
            offen: Mutex::new(HashMap::new()),
            naechste: AtomicU64::new(0),
        }),
    })
}

impl Bruecke {
    /// The bound port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.g.port
    }

    /// Accepts until `halt` is set; then shuts every connection, removes the token file and the port file and returns.
    pub fn laufen(self, halt: &Halt) {
        halt.port.store(self.g.port, Ordering::SeqCst);
        while !halt.ist_gestoppt() {
            match self.listener.accept() {
                Ok((s, _)) => self.annehmen(s),
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        }
        self.beenden();
    }

    fn annehmen(&self, mut s: TcpStream) {
        let g = &self.g;
        let id = g.naechste.fetch_add(1, Ordering::SeqCst);
        {
            let mut offen = sperre(&g.offen);
            if offen.len() >= g.cfg.verbindungen_max {
                drop(offen);
                (g.protokoll)("verbindungen");
                antworten(
                    &mut s,
                    &Antwort::abgelehnt(Abgelehnt {
                        status: 503,
                        grund: "verbindungen",
                    }),
                );
                return;
            }
            let Ok(klon) = s.try_clone() else { return };
            offen.insert(id, klon);
        }
        let g2 = Arc::clone(g);
        let gestartet = thread::Builder::new().stack_size(STAPEL).spawn(move || {
            let eintrag = OffenEintrag { g: &g2, id };
            behandeln(&g2, s, eintrag);
        });
        if gestartet.is_err() {
            sperre(&g.offen).remove(&id);
        }
    }

    fn beenden(&self) {
        for s in sperre(&self.g.offen).values() {
            let _ = s.shutdown(Shutdown::Both);
        }
        for d in sperre(&self.g.strome).values() {
            let _ = sperre(d).shutdown(Shutdown::Both);
        }
        for d in &self.g.dateien {
            d.entfernen();
        }
    }
}

/// Takes a connection out of the table of open ones when its thread ends, also by a panic.
struct OffenEintrag<'a> {
    g: &'a Gemeinsam,
    id: u64,
}

impl Drop for OffenEintrag<'_> {
    fn drop(&mut self) {
        sperre(&self.g.offen).remove(&self.id);
    }
}

/// Handles the connection. The place in the table of open connections is given back before the answer is sent: a client that
/// has read the answer and connects again at once finds the place free.
fn behandeln(g: &Gemeinsam, mut s: TcpStream, eintrag: OffenEintrag<'_>) {
    let _ = s.set_nodelay(true);
    let ergebnis = verbindung::bedienen(g, &mut s);
    drop(eintrag);
    match ergebnis {
        Ok(Some(a)) => antworten(&mut s, &a),
        Err(Abbruch::Abgelehnt(a)) => {
            (g.protokoll)(a.grund);
            antworten(&mut s, &Antwort::abgelehnt(a));
        }
        Ok(None) | Err(Abbruch::Stumm) => {}
    }
    let _ = s.shutdown(Shutdown::Both);
}

fn antworten(s: &mut TcpStream, a: &Antwort) {
    let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
    let _ = s.write_all(&a.bytes());
    #[cfg(windows)]
    auslaufen(s);
    let _ = s.shutdown(Shutdown::Both);
}

/// Windows (Nachtrag 25.11.2026, contract section 17): a socket that is closed while the client still sends (the rest of a body
/// over the limit, the request of a connection over `verbindungen_max`) ends with a reset, and the client loses the answer that
/// was already sent (measured in the Windows VM: no status, only the end of the connection). So the bridge closes its writing
/// half first and reads what still comes, at most 64 KiB and 200 ms, and throws it away; only then the socket closes.
#[cfg(windows)]
fn auslaufen(s: &mut TcpStream) {
    use std::io::Read;
    let _ = s.shutdown(Shutdown::Write);
    let _ = s.set_read_timeout(Some(Duration::from_millis(20)));
    let ende = std::time::Instant::now() + Duration::from_millis(200);
    let mut puffer = [0_u8; 8192];
    let mut gelesen = 0;
    while std::time::Instant::now() < ende && gelesen < 65_536 {
        match s.read(&mut puffer) {
            Ok(0) => break,
            Ok(n) => gelesen += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
}
