//! Helpers of the unit tests: a folder of its own, a stand-in for the service and a small HTTP client on raw sockets.

use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::server::{self, Bruecke, Halt};
use crate::startdatei::Config;
use crate::verbindung::Protokoll;

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);

/// A new folder with the mode 0700 under the temp folder; removed when dropped. The name holds the process id.
pub(crate) struct Ordner(PathBuf);

impl Ordner {
    pub(crate) fn neu() -> Self {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        // short: the path of a Unix socket has at most 100 bytes here
        let p = PathBuf::from("/tmp").join(format!("wbbr-{}-{n}", std::process::id()));
        fs::create_dir(&p).expect("make the test folder");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).expect("chmod");
        Self(p)
    }

    pub(crate) fn wurzel(&self) -> &Path {
        &self.0
    }

    pub(crate) fn pfad(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub(crate) fn text(&self, name: &str) -> String {
        self.pfad(name).to_str().expect("utf8").to_string()
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A Unix socket server that hands every connection to `f` (with the number of the connection, from 0), each in a thread.
pub(crate) struct FakeDienst {
    pfad: String,
    verbunden: Arc<AtomicUsize>,
}

type Behandler = dyn Fn(usize, &mut UnixStream) + Send + Sync;

impl FakeDienst {
    /// Only the first connection is served, by `f`.
    pub(crate) fn neu(pfad: &Path, f: impl FnOnce(&mut UnixStream) + Send + 'static) -> Self {
        let f = Mutex::new(Some(f));
        Self::viele(pfad, move |_, s| {
            let g = f.lock().expect("lock").take();
            if let Some(g) = g {
                g(s);
            }
        })
    }

    pub(crate) fn viele(
        pfad: &Path,
        f: impl Fn(usize, &mut UnixStream) + Send + Sync + 'static,
    ) -> Self {
        let l = UnixListener::bind(pfad).expect("bind the stand-in");
        let verbunden = Arc::new(AtomicUsize::new(0));
        let zaehler = Arc::clone(&verbunden);
        let f: Arc<Behandler> = Arc::new(f);
        thread::spawn(move || {
            while let Ok((mut s, _)) = l.accept() {
                let n = zaehler.fetch_add(1, Ordering::SeqCst);
                let f = Arc::clone(&f);
                thread::spawn(move || f(n, &mut s));
            }
        });
        Self {
            pfad: pfad.to_str().expect("utf8").to_string(),
            verbunden,
        }
    }

    pub(crate) fn pfad(&self) -> String {
        self.pfad.clone()
    }

    /// How many connections arrived so far.
    pub(crate) fn verbindungen(&self) -> usize {
        self.verbunden.load(Ordering::SeqCst)
    }
}

/// The answer a test client got.
#[derive(Debug)]
pub(crate) struct Gelesen {
    pub(crate) status: u16,
    pub(crate) koepfe: Vec<(String, String)>,
    pub(crate) koerper: Vec<u8>,
}

impl Gelesen {
    pub(crate) fn kopf(&self, name: &str) -> Option<&str> {
        self.koepfe
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, w)| w.as_str())
    }
}

/// Parses the head of an answer. `None` if the head is not complete.
pub(crate) fn kopf_zerlegen(roh: &[u8]) -> Option<(Gelesen, usize)> {
    let p = roh.windows(4).position(|w| w == b"\r\n\r\n")?;
    let kopf = String::from_utf8_lossy(&roh[..p]).into_owned();
    let mut zeilen = kopf.split("\r\n");
    let status = zeilen.next()?.split(' ').nth(1)?.parse().ok()?;
    let koepfe = zeilen
        .filter_map(|z| z.split_once(": "))
        .map(|(n, w)| (n.to_string(), w.to_string()))
        .collect();
    Some((
        Gelesen {
            status,
            koepfe,
            koerper: Vec::new(),
        },
        p + 4,
    ))
}

/// Sends `roh`, then reads until the peer closes (or `frist` passes). `None` if the connection gave no answer.
pub(crate) fn anfrage(port: u16, roh: &[u8], frist: Duration) -> Option<Gelesen> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(frist)).ok()?;
    let _ = s.write_all(roh);
    let mut alles = Vec::new();
    let _ = s.read_to_end(&mut alles);
    let _ = s.shutdown(Shutdown::Both);
    let (mut g, n) = kopf_zerlegen(&alles)?;
    g.koerper = alles[n..].to_vec();
    Some(g)
}

/// A bridge in a thread of the test, with a folder, a stand-in for the service and a protocol collected in a list.
pub(crate) struct Stand {
    _ordner: Ordner,
    pub(crate) port: u16,
    pub(crate) token: String,
    pub(crate) protokoll: Arc<Mutex<Vec<String>>>,
    halt: Arc<Halt>,
    faden: Option<JoinHandle<()>>,
}

impl Stand {
    /// Starts with the socket of the service at `ordner/d.sock`. `aendern` adjusts the configuration.
    pub(crate) fn neu(ordner: Ordner, aendern: impl FnOnce(&mut Config)) -> Self {
        let mut cfg = Config {
            dienst: ordner.text("d.sock"),
            port: 0,
            token_datei: ordner.text("t"),
            port_datei: None,
            deflate: false,
            lese_frist: Duration::from_millis(10_000),
            verbindungen_max: 64,
        };
        aendern(&mut cfg);
        let protokoll = Arc::new(Mutex::new(Vec::new()));
        let sammler = Arc::clone(&protokoll);
        let sink: Protokoll =
            Arc::new(move |g: &str| sammler.lock().expect("lock").push(g.to_string()));
        let token_datei = cfg.token_datei.clone();
        let b: Bruecke = server::starten(cfg, sink).expect("the bridge starts");
        let port = b.port();
        let token = fs::read_to_string(&token_datei)
            .expect("token file")
            .trim_end()
            .to_string();
        let halt = Arc::new(Halt::neu());
        let h = Arc::clone(&halt);
        let faden = Some(thread::spawn(move || b.laufen(&h)));
        Self {
            _ordner: ordner,
            port,
            token,
            protokoll,
            halt,
            faden,
        }
    }

    pub(crate) fn mit_dienst(
        dienst: impl FnOnce(&Path) -> FakeDienst,
        aendern: impl FnOnce(&mut Config),
    ) -> (Self, FakeDienst) {
        let o = Ordner::neu();
        let d = dienst(&o.pfad("d.sock"));
        (Self::neu(o, aendern), d)
    }

    pub(crate) fn ohne_dienst(aendern: impl FnOnce(&mut Config)) -> Self {
        Self::neu(Ordner::neu(), aendern)
    }

    pub(crate) fn host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    pub(crate) fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// A good request text: method, target, Host, Origin, the token and `extra` heads, and the body with its length.
    pub(crate) fn text(
        &self,
        methode: &str,
        ziel: &str,
        extra: &[(&str, &str)],
        koerper: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut t = format!(
            "{methode} {ziel} HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nAuthorization: Bearer {}\r\n",
            self.host(),
            self.origin(),
            self.token
        );
        for (n, w) in extra {
            t.push_str(&format!("{n}: {w}\r\n"));
        }
        let mut aus = Vec::new();
        if let Some(k) = koerper {
            t.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                k.len()
            ));
            aus.extend_from_slice(t.as_bytes());
            aus.extend_from_slice(b"\r\n");
            aus.extend_from_slice(k);
        } else {
            t.push_str("\r\n");
            aus.extend_from_slice(t.as_bytes());
        }
        aus
    }

    pub(crate) fn senden(&self, roh: &[u8]) -> Gelesen {
        anfrage(self.port, roh, Duration::from_secs(10)).expect("an answer")
    }

    pub(crate) fn zeilen(&self) -> Vec<String> {
        self.protokoll.lock().expect("lock").clone()
    }
}

impl Drop for Stand {
    fn drop(&mut self) {
        self.halt.anhalten();
        if let Some(f) = self.faden.take() {
            let ergebnis = abwarten(f);
            assert!(
                ergebnis.is_some() || thread::panicking(),
                "the bridge did not stop within 5 s"
            );
        }
    }
}

/// Waits at most 5 s for a thread. `None` if it does not end: a test that waits without a limit would hang for ever when
/// the code under test is broken, and the mutation run would count a time out instead of a caught mutant.
pub(crate) fn abwarten<T>(faden: JoinHandle<T>) -> Option<T> {
    let ende = std::time::Instant::now() + Duration::from_secs(5);
    while !faden.is_finished() && std::time::Instant::now() < ende {
        thread::sleep(Duration::from_millis(10));
    }
    if faden.is_finished() {
        faden.join().ok()
    } else {
        None
    }
}
