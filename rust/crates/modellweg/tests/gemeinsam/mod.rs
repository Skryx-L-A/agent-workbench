//! Helpers of the integration tests: throw-away folders, the visible cases, a scripted target, a target on the loopback
//! (the stand-in of section 12), and the program as a process. No test talks to a real target or uses a real key.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use modellweg::betrieb::Server;
use modellweg::betriebsart::openai_lokal::Optionen;
use modellweg::dienst::{Dienst, Lage};
use modellweg::schluessel;
use modellweg::ziel::{Verbinder, Verbindung, Ziel};
use serde_json::Value;

/// The invented test key of the cases.
pub const SCHLUESSEL: &str = "mw-testschluessel-7f3a9c0e5b2d4618";
/// The model of the cases.
pub const MODELL: &str = "lokal-beispiel/modell-a";
/// The time of every line written through the library (2026-11-20T15:00:00.000Z).
pub const ZEIT: i64 = 1_795_186_800_000;
/// Deadline of every wait in a test, so that a wrong program fails the test instead of hanging it.
pub const FRIST: Duration = Duration::from_secs(10);

/// A folder below the resolved `/tmp` (short, so a socket path fits into 107 bytes), removed when dropped. The base is
/// resolved (`canonicalize`), because step 7 of the contract refuses a socket path with a link in any of its parts and
/// `/tmp` is a link on macOS.
pub struct Wegwerf(PathBuf);

impl Wegwerf {
    pub fn neu(name: &str) -> Self {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let nummer = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = fs::canonicalize("/tmp").expect("the base of the tests");
        let pfad = basis.join(format!("mw-{}-{nummer}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("create the test folder");
        fs::set_permissions(&pfad, fs::Permissions::from_mode(0o700)).expect("chmod");
        Self(pfad)
    }

    pub fn pfad(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn ordner(&self) -> &Path {
        &self.0
    }

    /// A key file with mode 0600.
    pub fn schluessel(&self, inhalt: &[u8]) -> PathBuf {
        let p = self.pfad("schluessel");
        fs::write(&p, inhalt).expect("write the key file");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).expect("chmod");
        p
    }

    /// The protocol folder `protokoll` (made on first use).
    pub fn protokoll(&self) -> PathBuf {
        let p = self.pfad("protokoll");
        fs::create_dir_all(&p).expect("protocol folder");
        // The umask is process-wide: a test that binds a socket sets it to 0177 for a moment.
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).expect("chmod");
        p
    }
}

impl Drop for Wegwerf {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A file of the visible cases.
pub fn fall_datei(name: &str) -> Value {
    let pfad = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/modellweg/faelle")
        .join(name);
    serde_json::from_slice(&fs::read(&pfad).expect("read the case file"))
        .expect("case file is JSON")
}

/// The bytes of a text of the cases (latin1: each character is one byte).
pub fn latin1(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| u8::try_from(u32::from(c)).expect("latin1"))
        .collect()
}

pub fn text(v: &Value) -> &str {
    v.as_str().expect("text")
}

/// The bytes one connection got.
pub type Puffer = Arc<Mutex<Vec<u8>>>;

/// A target that answers from a script: each connection records what it got and hands out the prepared parts, one per read.
#[derive(Clone, Default)]
pub struct Skript {
    pub antwort: Vec<Vec<u8>>,
    pub gesehen: Arc<Mutex<Vec<Puffer>>>,
    pub verbunden: Arc<AtomicUsize>,
    /// While closed, every connection waits (to hold requests in work).
    pub tor: Option<Arc<(Mutex<bool>, Condvar)>>,
    /// The first read fails with this kind.
    pub lesefehler: Option<io::ErrorKind>,
    /// No connection at all (`mw.ziel_nicht_erreichbar`).
    pub unerreichbar: bool,
    /// After the prepared parts the stream breaks instead of ending.
    pub abbruch_am_ende: bool,
    /// After the first part each connection waits until it is torn down (or `alle_freigeben`), then breaks.
    pub haengen: bool,
    pub haenger: Arc<Mutex<Vec<Tor>>>,
    /// After the prepared parts each read times out until this gate opens, then the stream ends (a switched
    /// connection that stays open; shared by every clone, so it can be set after the start).
    pub am_ende: Arc<Mutex<Option<Tor>>>,
}

impl Skript {
    pub fn antwortet(teile: Vec<Vec<u8>>) -> Self {
        Self {
            antwort: teile,
            ..Self::default()
        }
    }

    pub fn ok() -> Self {
        Self::antwortet(vec![
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}".to_vec(),
        ])
    }

    pub fn gesehen(&self) -> Vec<Vec<u8>> {
        self.gesehen
            .lock()
            .expect("lock")
            .iter()
            .map(|b| b.lock().expect("lock").clone())
            .collect()
    }

    /// Opens the gate of every hanging connection.
    pub fn alle_freigeben(&self) {
        for t in self.haenger.lock().expect("lock").iter() {
            oeffne(t);
        }
    }

    pub fn oeffnen(&self) {
        if let Some(tor) = &self.tor {
            *tor.0.lock().expect("lock") = true;
            tor.1.notify_all();
        }
    }
}

/// A gate one thread waits at.
pub type Tor = Arc<(Mutex<bool>, Condvar)>;

fn oeffne(tor: &Tor) {
    *tor.0.lock().expect("lock") = true;
    tor.1.notify_all();
}

struct MemStrom {
    lesen: VecDeque<io::Result<Vec<u8>>>,
    geschrieben: Arc<Mutex<Vec<u8>>>,
    haenger: Option<Tor>,
    gelesen: usize,
    am_ende: Option<Tor>,
}

impl Read for MemStrom {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.gelesen += 1;
        if let Some(tor) = &self.haenger
            && self.gelesen > 1
        {
            let mut offen = tor.0.lock().expect("lock");
            while !*offen {
                offen = tor.1.wait(offen).expect("wait");
            }
            return Err(io::ErrorKind::ConnectionReset.into());
        }
        match self.lesen.pop_front() {
            None => match &self.am_ende {
                Some(tor) if !*tor.0.lock().expect("lock") => {
                    std::thread::sleep(Duration::from_millis(2));
                    Err(io::ErrorKind::TimedOut.into())
                }
                _ => Ok(0),
            },
            Some(Err(e)) => Err(e),
            Some(Ok(mut t)) => {
                let n = t.len().min(buf.len());
                buf[..n].copy_from_slice(&t[..n]);
                if n < t.len() {
                    self.lesen.push_front(Ok(t.split_off(n)));
                }
                Ok(n)
            }
        }
    }
}

impl Write for MemStrom {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.geschrieben
            .lock()
            .expect("lock")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Verbinder for Skript {
    fn verbinde(&self) -> Option<Verbindung> {
        self.verbunden.fetch_add(1, Ordering::SeqCst);
        if let Some(tor) = &self.tor {
            let mut offen = tor.0.lock().expect("lock");
            while !*offen {
                offen = tor.1.wait(offen).expect("wait");
            }
        }
        if self.unerreichbar {
            return None;
        }
        let geschrieben = Arc::new(Mutex::new(Vec::new()));
        self.gesehen
            .lock()
            .expect("lock")
            .push(Arc::clone(&geschrieben));
        let mut lesen: VecDeque<io::Result<Vec<u8>>> =
            self.antwort.iter().cloned().map(Ok).collect();
        if let Some(kind) = self.lesefehler {
            lesen.push_front(Err(kind.into()));
        }
        if self.abbruch_am_ende {
            lesen.push_back(Err(io::ErrorKind::ConnectionReset.into()));
        }
        let haenger: Option<Tor> = self
            .haengen
            .then(|| Arc::new((Mutex::new(false), Condvar::new())));
        if let Some(t) = &haenger {
            self.haenger.lock().expect("lock").push(Arc::clone(t));
        }
        let fuer_abriss = haenger.clone();
        Some(Verbindung {
            strom: Box::new(MemStrom {
                lesen,
                geschrieben,
                haenger,
                gelesen: 0,
                am_ende: self.am_ende.lock().expect("lock").clone(),
            }),
            abreissen: Box::new(move || {
                if let Some(t) = &fuer_abriss {
                    oeffne(t);
                }
            }),
            lesefrist: Box::new(|_| {}),
        })
    }
}

/// A service for the cases: the given target and model, the key file with `schluessel`, the protocol folder of `w`.
pub fn dienst(
    w: &Wegwerf,
    ziel: &str,
    modell: &str,
    schluessel: &str,
    verbinder: Box<dyn Verbinder>,
) -> Arc<Dienst> {
    dienst_mit(w, ziel, modell, schluessel, verbinder, None)
}

/// As [`dienst`], with the admin switch `--protokoll-pfad-voll` on (the whole path in the field `pfad`).
pub fn dienst_pfad_voll(
    w: &Wegwerf,
    ziel: &str,
    modell: &str,
    schluessel: &str,
    verbinder: Box<dyn Verbinder>,
) -> Arc<Dienst> {
    dienst_voll(w, ziel, modell, schluessel, verbinder, None, true)
}

/// As [`dienst`], with the marker count of a trial run.
pub fn dienst_mit(
    w: &Wegwerf,
    ziel: &str,
    modell: &str,
    schluessel: &str,
    verbinder: Box<dyn Verbinder>,
    probelauf: Option<modellweg::probelauf::Zaehlung>,
) -> Arc<Dienst> {
    dienst_voll(w, ziel, modell, schluessel, verbinder, probelauf, false)
}

/// As [`dienst_mit`], with the switch for the whole path.
fn dienst_voll(
    w: &Wegwerf,
    ziel: &str,
    modell: &str,
    schluessel: &str,
    verbinder: Box<dyn Verbinder>,
    probelauf: Option<modellweg::probelauf::Zaehlung>,
    pfad_voll: bool,
) -> Arc<Dienst> {
    let datei = w.schluessel(format!("{schluessel}\n").as_bytes());
    let schluessel =
        schluessel::lies(&datei, rustix::process::getuid().as_raw()).expect("key file");
    let lage = Lage {
        art: Optionen {
            ziel: Ziel::lies(ziel).expect("target"),
            modell: modell.to_owned(),
            schluessel_datei: datei,
        },
        sitzung: "s-2".to_owned(),
        route: "lokal-beispiel".to_owned(),
        protokoll: w.protokoll(),
        schluessel,
        probelauf,
        pfad_voll,
    };
    Arc::new(Dienst::neu(lage, verbinder, Box::new(|| ZEIT)))
}

/// Sends one request through `bediene` and returns all bytes of the answer up to the end of the connection.
pub fn anfrage<D, T>(d: &D, roh: &[u8]) -> Vec<u8>
where
    D: std::ops::Deref<Target = T>,
    T: Server + ?Sized,
{
    let (mut harness, modellweg) = UnixStream::pair().expect("pair");
    harness.set_read_timeout(Some(FRIST)).expect("timeout");
    let roh = roh.to_vec();
    let schreiber = std::thread::spawn(move || {
        let _ = harness.write_all(&roh);
        let _ = harness.shutdown(std::net::Shutdown::Write);
        let mut antwort = Vec::new();
        let _ = harness.read_to_end(&mut antwort);
        antwort
    });
    d.bediene(modellweg);
    schreiber.join().expect("harness thread")
}

/// An answer to the harness, taken apart: status, fields (lower-case names) and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Antwort {
    pub status: u16,
    pub kopf: Vec<(String, String)>,
    pub koerper: Vec<u8>,
}

impl Antwort {
    pub fn lies(bytes: &[u8]) -> Self {
        let ende = bytes
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or_else(|| panic!("no head in {:?}", String::from_utf8_lossy(bytes)));
        let kopf = String::from_utf8(bytes[..ende].to_vec()).expect("ascii head");
        let mut zeilen = kopf.split("\r\n");
        let start = zeilen.next().expect("status line");
        assert!(start.starts_with("HTTP/1.1 "), "{start}");
        let status = start[9..12].parse().expect("status");
        let kopf = zeilen
            .map(|z| {
                let (n, w) = z.split_once(':').expect("field");
                (n.to_ascii_lowercase(), w.trim().to_owned())
            })
            .collect();
        Self {
            status,
            kopf,
            koerper: bytes[ende + 4..].to_vec(),
        }
    }

    /// The fields without the own framing (`connection`, `content-length`).
    pub fn ohne_rahmung(&self) -> Vec<(String, String)> {
        self.kopf
            .iter()
            .filter(|(n, _)| n != "connection" && n != "content-length")
            .cloned()
            .collect()
    }
}

/// All lines of the chain `gateway` in a protocol folder, in file and line order.
pub fn zeilen(protokoll: &Path) -> Vec<Value> {
    let Ok(eintraege) = fs::read_dir(protokoll) else {
        return Vec::new();
    };
    let mut dateien: Vec<PathBuf> = eintraege
        .map(|e| e.expect("entry").path())
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            n.starts_with("gateway-") && p.extension().is_some_and(|e| e == "jsonl")
        })
        .collect();
    dateien.sort();
    dateien
        .iter()
        .flat_map(|d| {
            fs::read_to_string(d)
                .expect("read the chain")
                .lines()
                .map(|z| serde_json::from_str(z).expect("line is JSON"))
                .collect::<Vec<Value>>()
        })
        .collect()
}

/// A valid request with this body.
pub fn post(koerper: &str, extra: &str) -> Vec<u8> {
    format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n{koerper}",
        koerper.len()
    )
    .into_bytes()
}

pub fn koerper(inhalt: &str) -> String {
    format!(r#"{{"model":"{MODELL}","messages":[{{"role":"user","content":"{inhalt}"}}]}}"#)
}

/// A request the stand-in got: head and body.
#[derive(Debug, Clone)]
pub struct Erhalten {
    pub kopf: String,
    pub koerper: Vec<u8>,
}

impl Erhalten {
    pub fn alles(&self) -> String {
        format!(
            "{}\r\n\r\n{}",
            self.kopf,
            String::from_utf8_lossy(&self.koerper)
        )
    }
}

/// What the stand-in sends back: parts written one after another with a short pause, so they arrive apart.
/// An empty part stands for a long silence (30 s) of the target.
pub type Antworter = dyn Fn(&Erhalten) -> Vec<Vec<u8>> + Send + Sync;

/// The stand-in of the target on the loopback (port 0): it counts connections, records requests and answers.
pub struct Attrappe {
    pub adresse: SocketAddr,
    pub verbunden: Arc<AtomicUsize>,
    pub erhalten: Arc<Mutex<Vec<Erhalten>>>,
}

impl Attrappe {
    pub fn neu(antworter: Box<Antworter>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the stand-in");
        let adresse = listener.local_addr().expect("address");
        let verbunden = Arc::new(AtomicUsize::new(0));
        let erhalten = Arc::new(Mutex::new(Vec::new()));
        let (v, e) = (Arc::clone(&verbunden), Arc::clone(&erhalten));
        let antworter: Arc<Antworter> = Arc::from(antworter);
        std::thread::spawn(move || {
            for strom in listener.incoming() {
                let Ok(strom) = strom else { continue };
                v.fetch_add(1, Ordering::SeqCst);
                let (e, a) = (Arc::clone(&e), Arc::clone(&antworter));
                std::thread::spawn(move || attrappe_bedienen(strom, &e, &*a));
            }
        });
        Self {
            adresse,
            verbunden,
            erhalten,
        }
    }

    /// The base address `http://127.0.0.1:<port>/v1`.
    pub fn ziel(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.adresse.port())
    }

    pub fn erhalten(&self) -> Vec<Erhalten> {
        self.erhalten.lock().expect("lock").clone()
    }
}

fn attrappe_bedienen(mut strom: TcpStream, erhalten: &Mutex<Vec<Erhalten>>, antworter: &Antworter) {
    let _ = strom.set_read_timeout(Some(FRIST));
    let Some(anfrage) = lies_anfrage(&mut strom) else {
        return;
    };
    erhalten.lock().expect("lock").push(anfrage.clone());
    for (i, teil) in antworter(&anfrage).iter().enumerate() {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(30));
        }
        if teil.is_empty() {
            std::thread::sleep(Duration::from_secs(30));
            continue;
        }
        if strom.write_all(teil).and_then(|()| strom.flush()).is_err() {
            return;
        }
    }
}

/// Reads one request with `Content-Length` from a stream.
pub fn lies_anfrage<R: Read>(r: &mut R) -> Option<Erhalten> {
    let mut puf = Vec::new();
    let mut block = [0u8; 4096];
    let ende = loop {
        if let Some(e) = puf.windows(4).position(|w| w == b"\r\n\r\n") {
            break e;
        }
        let n = r.read(&mut block).ok()?;
        if n == 0 {
            return None;
        }
        puf.extend_from_slice(&block[..n]);
    };
    let kopf = String::from_utf8_lossy(&puf[..ende]).into_owned();
    let laenge: usize = kopf
        .lines()
        .find_map(|z| z.strip_prefix("Content-Length: "))
        .and_then(|w| w.trim().parse().ok())
        .unwrap_or(0);
    let mut koerper = puf[ende + 4..].to_vec();
    while koerper.len() < laenge {
        let n = r.read(&mut block).ok()?;
        if n == 0 {
            return None;
        }
        koerper.extend_from_slice(&block[..n]);
    }
    Some(Erhalten { kopf, koerper })
}

/// A chunked body of these parts.
pub fn stueckweise(teile: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for t in teile {
        v.extend_from_slice(format!("{:x}\r\n", t.len()).as_bytes());
        v.extend_from_slice(t);
        v.extend_from_slice(b"\r\n");
    }
    v.extend_from_slice(b"0\r\n\r\n");
    v
}

/// The program as a process with an empty environment (plus `env`), stdin `/dev/null`.
pub struct Prozess {
    pub kind: Child,
    pub stdout: mpsc::Receiver<String>,
}

pub fn programm() -> &'static str {
    env!("CARGO_BIN_EXE_modellweg")
}

/// Runs the program to its end and returns exit code, stdout and stderr (for failures before readiness).
pub fn lauf_bis_ende(args: &[OsString]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut kind = Command::new(programm())
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the program");
    let ende = Instant::now() + FRIST;
    loop {
        if let Some(status) = kind.try_wait().expect("wait") {
            let mut out = Vec::new();
            let mut err = Vec::new();
            kind.stdout
                .take()
                .expect("stdout")
                .read_to_end(&mut out)
                .expect("stdout");
            kind.stderr
                .take()
                .expect("stderr")
                .read_to_end(&mut err)
                .expect("stderr");
            return (status.code().expect("exit code"), out, err);
        }
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            panic!("the program did not end");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl Prozess {
    /// Starts the program and waits for its first line on stdout.
    pub fn start(args: &[OsString], env: &[(&str, &OsString)]) -> (Self, String) {
        let mut cmd = Command::new(programm());
        cmd.args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut kind = cmd.spawn().expect("start the program");
        let stdout = kind.stdout.take().expect("stdout");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for zeile in BufReader::new(stdout).lines() {
                let Ok(zeile) = zeile else { break };
                if tx.send(zeile).is_err() {
                    break;
                }
            }
        });
        let erste = rx.recv_timeout(FRIST).unwrap_or_default();
        (Self { kind, stdout: rx }, erste)
    }

    pub fn pid(&self) -> i32 {
        i32::try_from(self.kind.id()).expect("pid")
    }

    pub fn signal(&self, signal: rustix::process::Signal) {
        let pid = rustix::process::Pid::from_raw(self.pid()).expect("pid");
        rustix::process::kill_process(pid, signal).expect("signal");
    }

    /// Waits at most `frist` for the end; the exit code, or `None` if it still runs.
    pub fn warte(&mut self, frist: Duration) -> Option<i32> {
        let ende = Instant::now() + frist;
        loop {
            if let Some(status) = self.kind.try_wait().expect("wait") {
                return status.code();
            }
            if Instant::now() > ende {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// SIGTERM and the exit code.
    pub fn beende(&mut self) -> Option<i32> {
        if self.kind.try_wait().expect("wait").is_none() {
            self.signal(rustix::process::Signal::TERM);
        }
        self.warte(FRIST)
    }
}

impl Drop for Prozess {
    fn drop(&mut self) {
        if self.kind.try_wait().ok().flatten().is_none() {
            let _ = self.kind.kill();
            let _ = self.kind.wait();
        }
    }
}

/// The arguments of a valid call; `socket`, `ziel`, `schluessel` and `protokoll` from the caller.
pub fn argv(socket: &Path, ziel: &str, schluessel: &Path, protokoll: &Path) -> Vec<OsString> {
    [
        "--socket",
        socket.to_str().expect("utf8"),
        "--socket-rechte",
        "0600",
        "--ziel",
        ziel,
        "--modell",
        MODELL,
        "--schluessel-datei",
        schluessel.to_str().expect("utf8"),
        "--sitzung",
        "s-2",
        "--route",
        "lokal-beispiel",
        "--protokoll",
        protokoll.to_str().expect("utf8"),
    ]
    .iter()
    .map(OsString::from)
    .collect()
}

/// One request over the socket of a running program; all bytes of the answer.
pub fn ueber_socket(socket: &Path, roh: &[u8]) -> Vec<u8> {
    let mut s = UnixStream::connect(socket).expect("connect to the socket");
    s.set_read_timeout(Some(FRIST)).expect("timeout");
    s.write_all(roh).expect("send");
    let mut antwort = Vec::new();
    let _ = s.read_to_end(&mut antwort);
    antwort
}

/// Whether `heu` holds `nadel`.
pub fn enthaelt(heu: &[u8], nadel: &[u8]) -> bool {
    heu.windows(nadel.len()).any(|w| w == nadel)
}
