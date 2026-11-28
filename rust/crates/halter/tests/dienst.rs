//! The server under Linux against stand-ins of a model target and an MCP server on 127.0.0.1 (plain TCP, allowed only
//! for a loopback host): the security promises of section 11 that hold without root and without foreign processes
//! (W3, W4, W7, W8 and L6 to L9 in the form of version 1), the role from the program (L3 in part) and the search.
//!
//! The peer is this test process; its program is made the role program of the test (root of the role programs: the
//! folder of the test binary, owner: the own uid). Throw-away folders lie on tmpfs below `/dev/shm` ([`basis`]), else
//! below `CARGO_TARGET_TMPDIR`.
#![cfg(target_os = "linux")]
#![allow(
    clippy::many_single_char_names,
    reason = "a, c, d, z: stand-in, client, server, lines in every test"
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use halter::ablage::Ablage;
use halter::b64;
use halter::dienst::Dienst;
use halter::konfig;
use halter::pruefung::Rolle;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const MARKE: &str = "hgx1-geheimwert-0123";
const JETZT: i64 = 1_795_000_000_000;
const START: i64 = 1_794_996_400_000;
const PERSON: &str = "ps_a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);

fn uhr() -> i64 {
    JETZT
}

/// `TMPFS_MAGIC` of `statfs`.
const TMPFS: i128 = 0x0102_1994;

/// The least age of a folder of an ended test process before [`basis`] removes it.
const REST_ALTER: Duration = Duration::from_secs(3600);

/// The start of the name of a throw-away folder of this uid on tmpfs; the pid of the test process follows.
fn vorsilbe() -> String {
    format!("halter-test-{}-", uid())
}

/// Where the throw-away folders lie: `/dev/shm` if it is tmpfs, else `CARGO_TARGET_TMPDIR`. Every refused ticket
/// writes a chain line with `sync_data` under the lock of the chain; on a disk under load one such sync took longer
/// than [`HAENGEGRENZE`] (the holder in state D in `btrfs_btree_wait_writeback_range`, the other connections in
/// `flock`). On tmpfs the sync does not wait for a disk, so the bound measures the holder, not the disk. The first
/// call removes the folders of test processes of this uid that ended (nothing removes a folder on tmpfs otherwise).
/// A folder must also be older than [`REST_ALTER`]: a test in its own pid namespace that shares `/dev/shm` does not see
/// the pid of a test outside and would else take the folder of a running test.
fn basis() -> &'static Path {
    static BASIS: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BASIS.get_or_init(|| {
        let shm = Path::new("/dev/shm");
        if !rustix::fs::statfs(shm).is_ok_and(|s| i128::from(s.f_type) == TMPFS) {
            return PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        }
        let vorsilbe = vorsilbe();
        for e in std::fs::read_dir(shm).into_iter().flatten().flatten() {
            let name = e.file_name();
            let Some(pid) = name
                .to_str()
                .and_then(|n| n.strip_prefix(&vorsilbe))
                .and_then(|r| r.split('-').next())
            else {
                continue;
            };
            let eigen_und_alt = e.path().symlink_metadata().is_ok_and(|m| {
                m.is_dir()
                    && std::os::unix::fs::MetadataExt::uid(&m) == uid()
                    && m.modified()
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|a| a > REST_ALTER)
            });
            if eigen_und_alt && !Path::new("/proc").join(pid).exists() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
        shm.to_path_buf()
    })
}

fn ordner() -> PathBuf {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let p = basis().join(format!("{}{}-{n}", vorsilbe(), std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("folder");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("mode");
    p
}

fn heim_mit(geheimnisse: &[(&str, &str)]) -> PathBuf {
    let heim = ordner();
    let mut g = heim.clone();
    for e in halter::ablage::EBENEN {
        g = g.join(e);
        std::fs::create_dir(&g).expect("level");
        std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o700)).expect("mode");
    }
    for (n, w) in geheimnisse {
        let p = g.join(n);
        std::fs::write(&p, format!("{w}\n")).expect("secret");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("mode");
    }
    heim
}

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

fn egid() -> u32 {
    rustix::process::getegid().as_raw()
}

/// The role groups of a server in which the effective gid of this process (the peer of every test connection) is the
/// group of `rolle`.
fn eigene_gid_als(rolle: &str) -> BTreeMap<u32, Rolle> {
    BTreeMap::from([(egid(), Rolle::aus_text(rolle).expect("role"))])
}

/// A server with the role `rolle` for this process, the model target and the MCP server at `port`.
fn dienst(rolle: &str, port: u16, heim: &Path, start_ms: i64) -> Arc<Dienst> {
    let kid = "qvy5SgFhefK2a9hsceqk+MJ5evYpEJTTdGpqO3O85VY=";
    let k = json!({
        "fassung": 1,
        "konten": {uid().to_string(): {"person": PERSON}},
        "rollen": {rolle: "wb-test"},
        "ziele": {"copilot": {"host": "127.0.0.1", "port": port, "tls": false, "praefix": "", "kopf": "authorization",
            "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot",
            "pfade": ["GET /copilot_internal/*", "GET /responses", "POST /chat/completions"]}},
        "server": {"tickets": {"host": "127.0.0.1", "port": port, "tls": false, "pfad": "/mcp", "kopf": "authorization",
            "vorsatz": "Bearer ", "schluessel": "mcp.tickets", "freigabe": true,
            "werkzeuge": {"update": "schreibend", "get": "lesend"}, "lokal": false}},
        "kids": {"stelle-mcp-1": kid},
    });
    let konfig = konfig::lesen(k.to_string().as_bytes()).expect("konfig");
    let kette = heim.join("kette");
    std::fs::create_dir_all(&kette).expect("chain folder");
    let d = Arc::new(Dienst::neu(
        konfig,
        Ablage::neu(heim, uid()),
        uid(),
        start_ms,
        eigene_gid_als(rolle),
        Some(kette),
        uhr,
    ));
    if let Some(z) = ATTRAPPEN.lock().expect("lock").get(&port) {
        DIENSTE
            .lock()
            .expect("lock")
            .push((Arc::downgrade(&d), z.clone()));
    }
    d
}

/// How long a connection may go without a byte and without an end while the holder is busy (a chain line with
/// `sync_data`, a call to the stand-in): only a bound for a real hang, never the signal of a test. Under load a line
/// took longer than 5 s.
const HAENGEGRENZE: Duration = Duration::from_secs(60);
/// The bound of a whole connection (lines without end, a stream that never ends).
const GESAMTGRENZE: Duration = Duration::from_secs(120);
/// More lines than any test gets on one connection: a holder that sends lines without end fails at once.
const ZEILEN_MAX: usize = 10_000;
/// The step in which the reader looks whether the holder waits for this test.
const TAKT: Duration = Duration::from_millis(100);

/// The numbers of the system calls that read from a socket: `read`, `readv`, `recvfrom`, `recvmsg`.
#[cfg(target_arch = "x86_64")]
const LESEN: [u64; 4] = [0, 19, 45, 47];
#[cfg(target_arch = "aarch64")]
const LESEN: [u64; 4] = [63, 65, 207, 212];
/// `poll` and `ppoll`.
#[cfg(target_arch = "x86_64")]
const POLL: [u64; 2] = [7, 271];
#[cfg(target_arch = "aarch64")]
const POLL: [u64; 1] = [73];

/// What the reader sees of the stand-in of a server: its thread and, while it waits for the close of the holder
/// after its last answer, the inode of that connection.
#[derive(Clone)]
struct ZielSicht {
    faden: Arc<std::sync::OnceLock<PathBuf>>,
    danach: Arc<Mutex<Option<u64>>>,
}

/// The stand-ins by port, and the servers built by `dienst` with the stand-in of their port.
static ATTRAPPEN: Mutex<BTreeMap<u16, ZielSicht>> = Mutex::new(BTreeMap::new());
static DIENSTE: Mutex<Vec<(std::sync::Weak<Dienst>, ZielSicht)>> = Mutex::new(Vec::new());

fn ziel_von(d: &Arc<Dienst>) -> Option<ZielSicht> {
    let mut l = DIENSTE.lock().expect("lock");
    l.retain(|(w, _)| w.strong_count() > 0);
    l.iter()
        .find(|(w, _)| w.upgrade().is_some_and(|x| Arc::ptr_eq(&x, d)))
        .map(|(_, z)| z.clone())
}

/// The end of a connection on the side of the holder, in this process.
enum Gegenseite {
    /// The other end of a socket pair: its inode.
    Paar(u64),
    /// The connection accepted on the listener at `pfad` (`/proc/net/unix` names it with the path) that was not yet
    /// there before the connect (`vorher`); an accepted socket has an inode only after `accept`.
    Hoerer { pfad: PathBuf, vorher: Vec<u64> },
}

/// One connection to the server.
struct Client {
    strom: UnixStream,
    vorrat: Vec<u8>,
    roh: Vec<u8>,
    gegen: Gegenseite,
    /// The thread that serves the connection (`/proc/self/task/<tid>`), only for the diagnosis.
    faden: Arc<std::sync::OnceLock<PathBuf>>,
    /// The writers of the test on this connection that have not finished: while one writes, the holder may wait.
    schreiber: Arc<AtomicUsize>,
    /// The stand-in of the server, if the test built it with `attrappe` and `dienst`.
    ziel: Option<ZielSicht>,
    name: String,
    zeilen: usize,
    seit: std::time::Instant,
}

fn inode(s: &impl std::os::fd::AsFd) -> u64 {
    rustix::fs::fstat(s).expect("fstat").st_ino
}

fn neuer_client(strom: UnixStream, gegen: Gegenseite) -> Client {
    strom.set_read_timeout(Some(TAKT)).expect("timeout");
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    Client {
        strom,
        vorrat: Vec::new(),
        roh: Vec::new(),
        gegen,
        faden: Arc::new(std::sync::OnceLock::new()),
        schreiber: Arc::new(AtomicUsize::new(0)),
        ziel: None,
        name: format!(
            "{} connection {n}",
            std::thread::current()
                .name()
                .unwrap_or("a thread of the test")
        ),
        zeilen: 0,
        seit: std::time::Instant::now(),
    }
}

fn verbinden(d: &Arc<Dienst>) -> Client {
    let (a, b) = UnixStream::pair().expect("pair");
    let mut c = neuer_client(a, Gegenseite::Paar(inode(&b)));
    c.ziel = ziel_von(d);
    let d = Arc::clone(d);
    let faden = Arc::clone(&c.faden);
    std::thread::spawn(move || {
        if let Ok(p) = std::fs::read_link("/proc/thread-self") {
            let _ = faden.set(Path::new("/proc").join(p));
        }
        d.verbindung(b);
    });
    c
}

/// The inodes of the connections accepted on the listener at `pfad`.
fn angenommen(pfad: &Path) -> Vec<u64> {
    let p = pfad.to_str().expect("path");
    std::fs::read_to_string("/proc/net/unix")
        .expect("/proc/net/unix")
        .lines()
        .filter_map(|z| {
            let f: Vec<&str> = z.split_whitespace().collect();
            // Num RefCount Protocol Flags Type St Inode Path; St 03: connected (the listener itself has 01)
            (f.len() == 8 && f[7] == p && f[5] == "03")
                .then(|| f[6].parse().ok())
                .flatten()
        })
        .collect()
}

/// The system call and its first argument in which the thread at `task` sleeps (state `S`); `None` while it runs, is
/// runnable (a woken thread may still show its system call) or is outside a system call.
fn schlaeft_in(task: &Path) -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string(task.join("stat")).ok()?;
    if !stat
        .rsplit_once(") ")
        .is_some_and(|(_, r)| r.starts_with('S'))
    {
        return None;
    }
    // "<nr> <arg0> ...", "running" or "-1 ..." outside a system call
    let s = std::fs::read_to_string(task.join("syscall")).ok()?;
    let mut t = s.split_whitespace();
    let nr = t.next()?.parse::<u64>().ok()?;
    let a0 = u64::from_str_radix(t.next()?.strip_prefix("0x")?, 16).ok()?;
    Some((nr, a0))
}

/// The socket inode behind the descriptor `fd` of this process.
fn socket_von(fd: u64) -> Option<u64> {
    let ziel = std::fs::read_link(format!("/proc/self/fd/{fd}")).ok()?;
    ziel.to_str()?
        .strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// `true` if a thread of this process sleeps in a read on one of `inodes`: the holder waits for a request.
fn liest_eines(inodes: &[u64]) -> bool {
    let Ok(faeden) = std::fs::read_dir("/proc/self/task") else {
        panic!("/proc/self/task cannot be read: the reader cannot see whether the holder waits")
    };
    faeden.flatten().any(|f| {
        schlaeft_in(&f.path()).is_some_and(|(nr, fd)| {
            LESEN.contains(&nr) && socket_von(fd).is_some_and(|i| inodes.contains(&i))
        })
    })
}

/// `true` if the stand-in of the server has sent all its answer and sleeps in the read of the same connection
/// (it waits for the close of the holder), while the serving thread sleeps in `poll` or in a read on a socket: the
/// holder waits for the target, which will not send again, and for the test.
fn ziel_wartet(faden: Option<&PathBuf>, ziel: Option<&ZielSicht>) -> bool {
    let (Some(f), Some(z)) = (faden, ziel) else {
        return false;
    };
    let Some(inode) = *z.danach.lock().expect("lock") else {
        return false;
    };
    let attrappe = z
        .faden
        .get()
        .and_then(|a| schlaeft_in(a))
        .is_some_and(|(nr, fd)| LESEN.contains(&nr) && socket_von(fd) == Some(inode));
    attrappe
        && schlaeft_in(f).is_some_and(|(nr, fd)| {
            POLL.contains(&nr) || (LESEN.contains(&nr) && socket_von(fd).is_some())
        })
}

/// What the serving thread does, for the diagnosis: state, wait channel and system call.
fn zustand(faden: Option<&PathBuf>) -> String {
    let Some(f) = faden else {
        return "thread unknown".to_owned();
    };
    let lies = |n: &str| {
        std::fs::read_to_string(f.join(n))
            .map_or_else(|e| format!("({e})"), |s| s.trim().to_owned())
    };
    let stat = lies("stat");
    let st = stat.rsplit_once(") ").map_or("?", |(_, r)| &r[..1]);
    format!(
        "state {st}, wchan {}, syscall {}",
        lies("wchan"),
        lies("syscall")
    )
}

fn zeile(v: &Value) -> Vec<u8> {
    let mut b = halter::json::kanonisch(v).expect("canonical").into_bytes();
    b.push(b'\n');
    b
}

impl Client {
    fn senden(&mut self, v: &Value) {
        self.strom.write_all(&zeile(v)).expect("send");
    }

    /// `true` if the holder waits in a read on this connection, or waits for its stand-in while that waits for the
    /// holder ([`ziel_wartet`]), while no writer of the test is active.
    fn wartet_auf_uns(&self) -> bool {
        if self.schreiber.load(Ordering::SeqCst) != 0 {
            return false;
        }
        if ziel_wartet(self.faden.get(), self.ziel.as_ref()) {
            return true;
        }
        match &self.gegen {
            Gegenseite::Paar(i) => liest_eines(&[*i]),
            Gegenseite::Hoerer { pfad, vorher } => {
                let neu: Vec<u64> = angenommen(pfad)
                    .into_iter()
                    .filter(|i| *i != 0 && !vorher.contains(i))
                    .collect();
                liest_eines(&neu)
            }
        }
    }

    /// Where the connection stands, for a failure.
    fn diagnose(&self, was: &str) -> String {
        let letzte = self
            .roh
            .rsplit(|b| *b == b'\n')
            .find(|z| !z.is_empty())
            .map(|z| String::from_utf8_lossy(&z[..z.len().min(200)]).into_owned())
            .unwrap_or_default();
        format!(
            "{was}: {}, after {:.1} s, {} lines and {} bytes read, {} bytes of a line pending, last line {letzte:?}; \
             holder: {}",
            self.name,
            self.seit.elapsed().as_secs_f64(),
            self.zeilen,
            self.roh.len(),
            self.vorrat.len(),
            zustand(self.faden.get())
        )
    }

    /// The next line, or `None` at the end of the connection. The reader waits for an event: a line, the end, or the
    /// holder waiting while nothing will come ([`Client::wartet_auf_uns`]: it holds the connection open while the test
    /// waits for it; this fails at once, independent of the load). Time only bounds a real hang ([`HAENGEGRENZE`],
    /// [`GESAMTGRENZE`]).
    fn eine(&mut self) -> Option<Value> {
        let mut buf = vec![0u8; 65_536];
        let mut zuletzt = std::time::Instant::now();
        let mut wartet = false;
        loop {
            assert!(
                self.zeilen < ZEILEN_MAX,
                "{}",
                self.diagnose("more lines than any test gets")
            );
            assert!(
                self.seit.elapsed() < GESAMTGRENZE,
                "{}",
                self.diagnose("the connection runs longer than the bound of a connection")
            );
            if let Some(p) = self.vorrat.iter().position(|b| *b == b'\n') {
                let rest = self.vorrat.split_off(p + 1);
                let z = std::mem::replace(&mut self.vorrat, rest);
                assert!(
                    halter::antwort::gueltig(&z[..z.len() - 1]),
                    "line outside the form"
                );
                self.zeilen += 1;
                return Some(serde_json::from_slice(&z).expect("json"));
            }
            match self.strom.read(&mut buf) {
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    // The holder was seen in a read on this connection with nothing for it, and the read after
                    // that found nothing either: it has sent all it will send and waits for the test.
                    let jetzt = self.wartet_auf_uns();
                    assert!(
                        !(wartet && jetzt),
                        "{}",
                        self.diagnose(
                            "the holder waits for a request, or for a stand-in that has sent all, while \
                             the test waits for a line or the end (it holds the connection open)"
                        )
                    );
                    wartet = jetzt;
                    assert!(
                        zuletzt.elapsed() < HAENGEGRENZE,
                        "{}",
                        self.diagnose(
                            "no line and no end of the connection within the bound of a hang"
                        )
                    );
                }
                Ok(0) | Err(_) => return None,
                Ok(n) => {
                    wartet = false;
                    zuletzt = std::time::Instant::now();
                    self.vorrat.extend_from_slice(&buf[..n]);
                    self.roh.extend_from_slice(&buf[..n]);
                }
            }
        }
    }

    /// Writes `bytes` from another thread (a line larger than the socket buffer); the reader does not take the
    /// holder waiting for a read as a failure while the write runs.
    fn nebenher_schreiben(&self, bytes: Vec<u8>) -> std::thread::JoinHandle<()> {
        let mut s = self.strom.try_clone().expect("clone");
        let aktiv = Arc::clone(&self.schreiber);
        aktiv.fetch_add(1, Ordering::SeqCst);
        std::thread::spawn(move || {
            let _ = s.write_all(&bytes);
            aktiv.fetch_sub(1, Ordering::SeqCst);
        })
    }

    /// The end of the connection must come and no line before it.
    fn ende_erwartet(&mut self, grund: &str) {
        if let Some(v) = self.eine() {
            panic!("{grund}: a line came instead of the end: {v}");
        }
    }

    /// Lines up to the first `antwort` or `ende`.
    fn bis_schluss(&mut self) -> Vec<Value> {
        let mut z = Vec::new();
        while let Some(v) = self.eine() {
            let art = v["art"].as_str().unwrap_or("").to_owned();
            z.push(v);
            if art == "antwort" || art == "ende" {
                break;
            }
        }
        z
    }

    fn marken(&self) -> usize {
        zaehlen(&self.roh, MARKE.as_bytes())
    }
}

fn zaehlen(heu: &[u8], nadel: &[u8]) -> usize {
    heu.windows(nadel.len()).filter(|f| *f == nadel).count()
}

fn teil_bytes(zeilen: &[Value]) -> Vec<u8> {
    zeilen
        .iter()
        .filter(|z| z["art"] == "teil")
        .flat_map(|z| b64::dekodieren(z["daten_b64"].as_str().expect("b64")).expect("b64"))
        .collect()
}

fn letzte_code(zeilen: &[Value]) -> Option<String> {
    zeilen
        .last()
        .and_then(|z| z["code"].as_str())
        .map(str::to_owned)
}

/// A stand-in target: every connection gets the next answer; it records each request (head and body) and, after the
/// answer, what else arrives until the close.
struct Attrappe {
    port: u16,
    anfragen: Arc<Mutex<Vec<Vec<u8>>>>,
    danach: Arc<Mutex<Vec<Vec<u8>>>>,
}

fn kopf_und_koerper(s: &mut TcpStream) -> Vec<u8> {
    let mut b = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(p) = b.windows(4).position(|f| f == b"\r\n\r\n") {
            let kopf = String::from_utf8_lossy(&b[..p]).to_ascii_lowercase();
            let laenge: usize = kopf
                .lines()
                .find_map(|z| z.strip_prefix("content-length: "))
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0);
            while b.len() < p + 4 + laenge {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => b.extend_from_slice(&buf[..n]),
                }
            }
            return b;
        }
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return b,
            Ok(n) => b.extend_from_slice(&buf[..n]),
        }
    }
}

fn attrappe(antworten: Vec<Vec<Vec<u8>>>) -> Attrappe {
    attrappe_mit(antworten, true)
}

/// `schliessen`: after its answer the stand-in closes its sending side; else it waits for the close of the holder.
fn attrappe_mit(antworten: Vec<Vec<Vec<u8>>>, schliessen: bool) -> Attrappe {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let anfragen = Arc::new(Mutex::new(Vec::new()));
    let danach = Arc::new(Mutex::new(Vec::new()));
    let (a2, d2) = (Arc::clone(&anfragen), Arc::clone(&danach));
    let sicht = ZielSicht {
        faden: Arc::new(std::sync::OnceLock::new()),
        danach: Arc::new(Mutex::new(None)),
    };
    ATTRAPPEN.lock().expect("lock").insert(port, sicht.clone());
    std::thread::spawn(move || {
        if let Ok(p) = std::fs::read_link("/proc/thread-self") {
            let _ = sicht.faden.set(Path::new("/proc").join(p));
        }
        for teile in antworten {
            let Ok((mut s, _)) = l.accept() else { return };
            s.set_read_timeout(Some(Duration::from_secs(5)))
                .expect("timeout");
            let anfrage = kopf_und_koerper(&mut s);
            a2.lock().expect("lock").push(anfrage);
            for t in teile {
                if s.write_all(&t).is_err() {
                    break;
                }
                s.flush().expect("flush");
                std::thread::sleep(Duration::from_millis(30));
            }
            if schliessen {
                let _ = s.shutdown(std::net::Shutdown::Write);
            }
            s.set_read_timeout(Some(Duration::from_secs(20)))
                .expect("timeout");
            *sicht.danach.lock().expect("lock") = Some(inode(&s));
            let mut rest = Vec::new();
            let _ = s.read_to_end(&mut rest);
            *sicht.danach.lock().expect("lock") = None;
            d2.lock().expect("lock").push(rest);
        }
    });
    Attrappe {
        port,
        anfragen,
        danach,
    }
}

fn aufruf(methode: &str, pfad: &str, koerper: &[u8]) -> Value {
    json!({"abfrage": null, "fassung": 1, "id": "m-1", "koerper_b64": b64::kodieren(koerper),
        "kopf": {"content-type": "application/json"}, "methode": methode, "op": "modell.aufruf",
        "pfad": pfad, "ziel": "copilot"})
}

fn strom_anfrage() -> Value {
    json!({"abfrage": null, "fassung": 1, "id": "m-2", "kopf": {"sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
        "sec-websocket-version": "13"}, "op": "modell.strom", "pfad": "/responses", "ziel": "copilot"})
}

fn http(status: &str, kopf: &str, koerper: &[u8]) -> Vec<u8> {
    let mut b = format!(
        "HTTP/1.1 {status}\r\n{kopf}content-length: {}\r\n\r\n",
        koerper.len()
    )
    .into_bytes();
    b.extend_from_slice(koerper);
    b
}

fn rahmen(fin: bool, opcode: u8, nutz: &[u8]) -> Vec<u8> {
    let mut r = vec![if fin { 0x80 } else { 0 } | opcode];
    assert!(nutz.len() < 65_536);
    if nutz.len() < 126 {
        r.push(u8::try_from(nutz.len()).expect("len"));
    } else {
        r.push(126);
        r.extend_from_slice(&u16::try_from(nutz.len()).expect("len").to_be_bytes());
    }
    r.extend_from_slice(nutz);
    r
}

const ACHT_ZEHN_EINS: &[u8] = b"HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nsec-websocket-accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";

#[test]
fn vorhanden_nur_fuer_das_eigene_konto() {
    let heim = heim_mit(&[
        ("anmeldung.copilot", MARKE),
        (".anmeldung.copilot.tmp", "x"),
        ("modell.lokal", "hgx1-maschine"),
    ]);
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    for (name, soll) in [
        ("anmeldung.copilot", true),
        ("mcp.tickets", false),
        ("modell.copilot", false),
        ("modell.lokal", true),
        ("anmeldung.lokal", false),
    ] {
        c.senden(&json!({"fassung": 1, "id": "q-1", "name": name, "op": "schluessel.vorhanden"}));
        let z = c.bis_schluss();
        assert_eq!(z[0]["vorhanden"], json!(soll), "{name}");
    }
    assert_eq!(c.marken(), 0);
}

/// `schluessel.vorhanden` by `ziel` (section 4): the holder takes the name from `halter.json`; a known target with its
/// secret is `true`, without it `false`, an unknown target `halter.ziel_unbekannt`; another role than `bruecke` with
/// `ziel` gets `halter.rolle`, and `name` keeps working for it.
#[test]
fn vorhanden_nach_ziel() {
    let frage =
        |ziel: &str| json!({"fassung": 1, "id": "q-1", "op": "schluessel.vorhanden", "ziel": ziel});
    let mit = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", 9, &mit, START);
    let mut c = verbinden(&d);
    c.senden(&frage("copilot"));
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(true));
    c.senden(&frage("fremd"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.ziel_unbekannt")
    );
    c.senden(&json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden",
        "ziel": "copilot"}));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schluessel_unbekannt")
    );
    c.senden(&json!({"fassung": 1, "id": "q-1", "op": "schluessel.vorhanden"}));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schluessel_fehlt")
    );
    assert_eq!(c.marken(), 0);
    let ohne = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("bruecke", 9, &ohne, START);
    let mut c = verbinden(&d);
    c.senden(&frage("copilot"));
    let z = c.bis_schluss();
    assert_eq!(z[0]["ok"], json!(true));
    assert_eq!(
        z[0]["vorhanden"],
        json!(false),
        "a known target without its secret"
    );
    for rolle in ["hub", "oberflaeche"] {
        let d = dienst(rolle, 9, &mit, START);
        let mut c = verbinden(&d);
        c.senden(&frage("copilot"));
        assert_eq!(
            letzte_code(&c.bis_schluss()).as_deref(),
            Some("halter.rolle"),
            "{rolle}"
        );
        c.senden(&json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"}));
        assert_eq!(
            c.bis_schluss()[0]["vorhanden"],
            json!(true),
            "{rolle} by name"
        );
    }
}

/// N1: a path outside the rules of the target ends with `halter.pfad_verboten`, reaches no target and writes the chain
/// line `halter.pfad_verboten` with method and path, without the fixed journal line; a path inside the rules is the
/// counter-check and reaches the target.
#[test]
fn n1_pfad_ausserhalb_der_regeln_geht_in_die_kette() {
    let a = attrappe(vec![vec![http("200 OK", "", b"ok")]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    assert!(!d.kette_gemeldet());
    c.senden(&aufruf("GET", "/user/repos", b""));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.pfad_verboten")
    );
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        0,
        "nothing reached the target"
    );
    let k = kette_text(&heim);
    let zeile = k
        .lines()
        .map(|z| serde_json::from_str::<Value>(z).expect("line"))
        .find(|z| z["aktionsname"] == "halter.pfad_verboten");
    // the register of contract 5 holds the kind: the line is in the chain, and the fixed journal line "Kette nicht
    // fortgeschrieben" does not come
    let z = zeile.expect("the chain line halter.pfad_verboten");
    let mut felder: Vec<&str> = z
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    felder.sort_unstable();
    assert_eq!(
        felder,
        [
            "aktionsname",
            "ergebnis",
            "fassung",
            "grund",
            "methode",
            "op",
            "pfad",
            "vorher",
            "zeit",
            "ziel"
        ]
    );
    assert_eq!(z["methode"], "GET");
    assert_eq!(z["op"], "modell.aufruf");
    assert_eq!(z["pfad"], "/user/repos");
    assert_eq!(z["ziel"], "copilot");
    assert!(
        !d.kette_gemeldet(),
        "no journal line of a chain that cannot be continued"
    );
    let p = formate::kette::pruefe_quelle(
        &heim.join("kette"),
        "halter",
        formate::kette::NULL_HASH,
        None,
    )
    .expect("source");
    assert!(p.ok(), "{:?}", p.bruch);
    assert_eq!(zaehlen(k.as_bytes(), MARKE.as_bytes()), 0);
    c.senden(&aufruf("POST", "/chat/completions", b"{}"));
    let z = c.bis_schluss();
    assert_eq!(
        z.last().expect("end")["ok"],
        json!(true),
        "a path in the rules goes through"
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 1);
}

/// Counter-check to `n1_pfad_ausserhalb_der_regeln_geht_in_die_kette`: with a broken anchor the chain cannot take the
/// line of a refused path, and the server notes the fixed journal line (F16) once.
#[test]
fn n1_kaputte_kette_meldet_die_feste_journalzeile() {
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", 9, &heim, START);
    std::fs::write(heim.join("kette").join("halter.anker.jsonl"), b"kaputt\n").expect("anchor");
    let mut c = verbinden(&d);
    assert!(!d.kette_gemeldet());
    c.senden(&aufruf("GET", "/user/repos", b""));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.pfad_verboten")
    );
    assert!(
        d.kette_gemeldet(),
        "the fixed journal line of a broken chain"
    );
}

/// A server with the role `bruecke` and one target `copilot` at `port` with the rules `pfade` and, if given, the switch
/// of the first start `erststart_antwortform`.
fn dienst_erststart(
    port: u16,
    heim: &Path,
    pfade: &[&str],
    antwortform: Option<&[&str]>,
) -> Arc<Dienst> {
    let mut ziel = json!({"host": "127.0.0.1", "port": port, "tls": false, "praefix": "", "kopf": "authorization",
        "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot", "pfade": pfade});
    if let Some(a) = antwortform {
        ziel["erststart_antwortform"] = json!(a);
    }
    let k = json!({"fassung": 1, "konten": {uid().to_string(): {"person": PERSON}}, "rollen": {"bruecke": "wb-test"},
        "ziele": {"copilot": ziel}, "server": {}, "kids": {}});
    let konfig = konfig::lesen(k.to_string().as_bytes()).expect("konfig");
    let kette = heim.join("kette");
    std::fs::create_dir_all(&kette).expect("chain folder");
    Arc::new(Dienst::neu(
        konfig,
        Ablage::neu(heim, uid()),
        uid(),
        START,
        eigene_gid_als("bruecke"),
        Some(kette),
        uhr,
    ))
}

/// The rules of the profile of the first start at the model host (decision of the lead, 25.11.2026).
const ERSTSTART: [&str; 2] = ["GET /models", "POST /chat/completions"];

/// The profile of the first start: each of the five paths not released at the first start ends with
/// `halter.pfad_verboten`, reaches no target and stands in the chain with method and path; the two released paths are
/// the counter-check and reach the target.
#[test]
fn erststart_sperrt_die_fuenf_pfade_mit_kettenzeile() {
    let a = attrappe(vec![
        vec![http("200 OK", "", b"{}")],
        vec![http("200 OK", "", b"{}")],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &ERSTSTART, None);
    let mut c = verbinden(&d);
    let gesperrt = [
        ("modell.strom", "GET", "/responses"),
        ("modell.aufruf", "POST", "/auto"),
        ("modell.aufruf", "POST", "/models/session"),
        ("modell.aufruf", "POST", "/models/session/intent"),
        ("modell.aufruf", "POST", "/agents/sessions"),
    ];
    for (op, methode, pfad) in gesperrt {
        if op == "modell.strom" {
            c.senden(&strom_anfrage());
        } else {
            c.senden(&aufruf(methode, pfad, b"{}"));
        }
        assert_eq!(
            letzte_code(&c.bis_schluss()).as_deref(),
            Some("halter.pfad_verboten"),
            "{methode} {pfad}"
        );
    }
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        0,
        "nothing reached the target"
    );
    let zeilen: Vec<Value> = kette_text(&heim)
        .lines()
        .map(|z| serde_json::from_str::<Value>(z).expect("line"))
        .filter(|z| z["aktionsname"] == "halter.pfad_verboten")
        .collect();
    assert_eq!(zeilen.len(), gesperrt.len());
    for (z, (op, methode, pfad)) in zeilen.iter().zip(gesperrt) {
        assert_eq!(z["op"], op);
        assert_eq!(z["methode"], methode);
        assert_eq!(z["pfad"], pfad);
        assert_eq!(z["ziel"], "copilot");
        assert_eq!(z["grund"], "pfad");
    }
    assert!(!d.kette_gemeldet());
    let p = formate::kette::pruefe_quelle(
        &heim.join("kette"),
        "halter",
        formate::kette::NULL_HASH,
        None,
    )
    .expect("source");
    assert!(p.ok(), "{:?}", p.bruch);
    // counter-check: the two released paths go through
    c.senden(&aufruf("GET", "/models", b""));
    assert_eq!(
        c.bis_schluss().last().expect("end")["ok"],
        json!(true),
        "GET /models"
    );
    c.senden(&aufruf("POST", "/chat/completions", b"{}"));
    assert_eq!(
        c.bis_schluss().last().expect("end")["ok"],
        json!(true),
        "POST /chat/completions"
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 2);
    assert_eq!(d.antwortform_zahl(), 0, "no diagnosis without the switch");
    assert_eq!(d.antwortform_letzte(), None);
}

/// The answer form of the first start (target 4 of the order): with the switch for `POST /auto`, the answer
/// `{"data":[{"id":..}],"access_token":..}` gives the names `data`, `data[].id` and `access_token` and the mark "stays
/// closed"; the mark value stands neither in the journal line nor in the chain. The answer itself reaches the bridge
/// unchanged (the value is no held secret, so the leak lock lets it pass as before). A path in the rules without the
/// switch gives no line.
#[test]
fn antwortform_nennt_die_namen_ohne_werte() {
    const WERT: &str = "hgx9-antwortwert-0815";
    let koerper = format!(r#"{{"access_token":"{WERT}","data":[{{"id":"{WERT}"}}]}}"#);
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            koerper.as_bytes(),
        )],
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            koerper.as_bytes(),
        )],
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            koerper.as_bytes(),
        )],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(
        a.port,
        &heim,
        &["GET /models", "POST /auto", "POST /chat/completions"],
        Some(&["POST /auto"]),
    );
    let mut c = verbinden(&d);
    c.senden(&aufruf("POST", "/auto", b"{}"));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(
        teil_bytes(&z),
        koerper.as_bytes(),
        "the answer goes on unchanged"
    );
    assert_eq!(d.antwortform_zahl(), 1);
    let zeile = d.antwortform_letzte().expect("the journal line");
    let text = zeile.strip_prefix("halter.antwortform: ").expect("prefix");
    let v: Value = serde_json::from_str(text.trim_end()).expect("line");
    assert_eq!(v["namen"], json!(["access_token", "data", "data[].id"]));
    assert_eq!(v["zu"], json!(true));
    assert_eq!(v["form"], "json");
    assert_eq!(v["methode"], "POST");
    assert_eq!(v["pfad"], "/auto");
    assert_eq!(v["ziel"], "copilot");
    assert_eq!(v["status"], 200);
    assert_eq!(zaehlen(zeile.as_bytes(), WERT.as_bytes()), 0);
    assert_eq!(zaehlen(zeile.as_bytes(), MARKE.as_bytes()), 0);
    let k = kette_text(&heim);
    assert_eq!(zaehlen(k.as_bytes(), WERT.as_bytes()), 0);
    assert_eq!(c.marken(), 0);
    // a path in the rules without the switch: no further line
    c.senden(&aufruf("POST", "/chat/completions", b"{}"));
    assert_eq!(c.bis_schluss().last().expect("end")["ok"], json!(true));
    assert_eq!(d.antwortform_zahl(), 1);
    // the same rule again: a second line, again without the value
    c.senden(&aufruf("POST", "/auto", b"{}"));
    assert_eq!(c.bis_schluss().last().expect("end")["ok"], json!(true));
    assert_eq!(d.antwortform_zahl(), 2);
    assert_eq!(d.antwortform_letzte().as_deref(), Some(zeile.as_str()));
}

/// Without the switch there is no diagnosis: the same answer on the same path gives no line, in every build.
#[test]
fn ohne_schalter_keine_antwortform() {
    let koerper = br#"{"access_token":"x","data":[{"id":"y"}]}"#;
    let a = attrappe(vec![vec![http("200 OK", "", koerper)]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &["POST /auto"], None);
    let mut c = verbinden(&d);
    c.senden(&aufruf("POST", "/auto", b"{}"));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(teil_bytes(&z), koerper);
    assert_eq!(d.antwortform_zahl(), 0);
    assert_eq!(d.antwortform_letzte(), None);
}

/// The values of the first start: with the switch for `GET /models` and `POST /chat/completions` the line of
/// `GET /models` names the model ids, not the value of another field; an error answer names its `error.code`. The
/// answers reach the bridge unchanged.
#[test]
fn antwortform_nennt_modellnamen_und_fehlercode() {
    let modelle = br#"{"data":[{"id":"gpt-6-luna","name":"name-wert"},{"id":"claude-sonnet-5.5"},{"id":"gpt 6"}],"access_token":"tok-wert"}"#;
    let fehler = br#"{"error":{"code":"model_not_supported","message":"nachricht-wert"}}"#;
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            modelle,
        )],
        vec![http(
            "400 Bad Request",
            "content-type: application/json\r\n",
            fehler,
        )],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &ERSTSTART, Some(&ERSTSTART));
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/models", b""));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(teil_bytes(&z), modelle);
    let zeile = d.antwortform_letzte().expect("the journal line");
    let v: Value = serde_json::from_str(
        zeile
            .strip_prefix("halter.antwortform: ")
            .expect("prefix")
            .trim_end(),
    )
    .expect("line");
    assert_eq!(
        v["werte"],
        json!({"anfrage.reasoning.effort": null, "anfrage.reasoning_effort": null,
            "data[].id": ["claude-sonnet-5.5", "gpt-6-luna"], "error.code": null,
            "modelle": {"claude-sonnet-5.5": {"capabilities.supports.reasoning_effort": null, "policy.state": null},
                "gpt-6-luna": {"capabilities.supports.reasoning_effort": null, "policy.state": null}}})
    );
    assert_eq!(v["wert_verworfen"], 1);
    for fremd in ["name-wert", "tok-wert", "gpt 6"] {
        assert_eq!(zaehlen(zeile.as_bytes(), fremd.as_bytes()), 0, "{fremd}");
    }
    c.senden(&aufruf("POST", "/chat/completions", b"{}"));
    let z = c.bis_schluss();
    assert_eq!(teil_bytes(&z), fehler);
    assert_eq!(d.antwortform_zahl(), 2);
    let zeile = d.antwortform_letzte().expect("the journal line");
    assert!(
        zeile.contains(r#""werte":{"anfrage.reasoning.effort":null,"anfrage.reasoning_effort":null,"data[].id":[],"error.code":"model_not_supported","modelle":{}}"#),
        "{zeile}"
    );
    assert!(zeile.contains(r#""status":400"#), "{zeile}");
    assert_eq!(zaehlen(zeile.as_bytes(), b"nachricht-wert"), 0);
    let k = kette_text(&heim);
    for wert in ["gpt-6-luna", "model_not_supported"] {
        assert_eq!(
            zaehlen(k.as_bytes(), wert.as_bytes()),
            0,
            "{wert} not in the chain"
        );
    }
}

/// The reasoning effort of a chat request and the levels of the model list (decision of the operator, 27.11.2026):
/// with the switch the line of `GET /models` names `policy.state` and the levels of each model, and the line of an
/// answer 400 to `POST /chat/completions` names the effort of the request, a known level as itself, any other as
/// `anderer`, never its text. The requests and answers pass unchanged; no value stands in the chain.
#[test]
fn antwortform_nennt_effort_und_stufen() {
    let modelle = br#"{"data":[{"id":"gpt-6-luna","policy":{"state":"enabled","terms":"t-wert"},"capabilities":{"supports":{"reasoning_effort":["none","low","medium","high","xhigh","max"]}}},{"id":"gpt-4.1","policy":{"state":"enabled"}}]}"#;
    let fehler = br#"{"error":{"code":"invalid_reasoning_effort","message":"nachricht-wert"}}"#;
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            modelle,
        )],
        vec![http(
            "400 Bad Request",
            "content-type: application/json\r\n",
            fehler,
        )],
        vec![http(
            "400 Bad Request",
            "content-type: application/json\r\n",
            fehler,
        )],
        vec![http("200 OK", "content-type: application/json\r\n", b"{}")],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &ERSTSTART, Some(&ERSTSTART));
    let mut c = verbinden(&d);
    let letzte = |d: &Dienst| -> Value {
        let z = d.antwortform_letzte().expect("the journal line");
        serde_json::from_str(
            z.strip_prefix("halter.antwortform: ")
                .expect("prefix")
                .trim_end(),
        )
        .expect("line")
    };
    c.senden(&aufruf("GET", "/models", b""));
    assert_eq!(teil_bytes(&c.bis_schluss()), modelle);
    let v = letzte(&d);
    assert_eq!(
        v["werte"]["modelle"],
        json!({"gpt-4.1": {"capabilities.supports.reasoning_effort": null, "policy.state": "enabled"},
            "gpt-6-luna": {"capabilities.supports.reasoning_effort": ["none","low","medium","high","xhigh","max"], "policy.state": "enabled"}})
    );
    assert_eq!(v["werte"]["anfrage.reasoning_effort"], Value::Null);
    // an unknown effort: `anderer`, the text of the request stands nowhere
    let unbekannt = br#"{"model":"gpt-6-luna","reasoning_effort":"stufe-wert-0815","messages":[]}"#;
    c.senden(&aufruf("POST", "/chat/completions", unbekannt));
    assert_eq!(teil_bytes(&c.bis_schluss()), fehler);
    let v = letzte(&d);
    assert_eq!(v["werte"]["anfrage.reasoning_effort"], "anderer");
    assert_eq!(v["werte"]["error.code"], "invalid_reasoning_effort");
    assert_eq!(v["status"], 400);
    // a known effort below `reasoning`
    c.senden(&aufruf(
        "POST",
        "/chat/completions",
        br#"{"reasoning":{"effort":"xhigh"}}"#,
    ));
    assert_eq!(teil_bytes(&c.bis_schluss()), fehler);
    let v = letzte(&d);
    assert_eq!(v["werte"]["anfrage.reasoning.effort"], "xhigh");
    assert_eq!(v["werte"]["anfrage.reasoning_effort"], Value::Null);
    // without an effort both stay `null`
    c.senden(&aufruf(
        "POST",
        "/chat/completions",
        br#"{"model":"gpt-6-luna"}"#,
    ));
    assert_eq!(c.bis_schluss().last().expect("end")["ok"], json!(true));
    let v = letzte(&d);
    assert_eq!(v["werte"]["anfrage.reasoning_effort"], Value::Null);
    assert_eq!(v["werte"]["anfrage.reasoning.effort"], Value::Null);
    assert_eq!(d.antwortform_zahl(), 4);
    // the requests reached the target unchanged
    let anfragen = a.anfragen.lock().expect("lock");
    assert_eq!(anfragen.len(), 4);
    assert_eq!(zaehlen(&anfragen[1], unbekannt), 1);
    drop(anfragen);
    let zeile = d.antwortform_letzte().expect("line");
    for fremd in ["stufe-wert-0815", "nachricht-wert", "t-wert"] {
        assert_eq!(zaehlen(zeile.as_bytes(), fremd.as_bytes()), 0, "{fremd}");
    }
    let k = kette_text(&heim);
    for wert in [
        "stufe-wert-0815",
        "anderer",
        "invalid_reasoning_effort",
        "xhigh",
        "enabled",
    ] {
        assert_eq!(
            zaehlen(k.as_bytes(), wert.as_bytes()),
            0,
            "{wert} not in the chain"
        );
    }
}

/// Without the switch a chat request with an effort gives no line.
#[test]
fn ohne_schalter_kein_effort() {
    let a = attrappe(vec![vec![http("200 OK", "", b"{}")]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &ERSTSTART, None);
    let mut c = verbinden(&d);
    c.senden(&aufruf(
        "POST",
        "/chat/completions",
        br#"{"reasoning_effort":"high"}"#,
    ));
    assert_eq!(c.bis_schluss().last().expect("end")["ok"], json!(true));
    assert_eq!(d.antwortform_zahl(), 0);
    assert_eq!(d.antwortform_letzte(), None);
}

/// Without the switch `GET /models` gives no line and no value.
#[test]
fn ohne_schalter_keine_modellnamen() {
    let koerper = br#"{"data":[{"id":"gpt-6-luna"}]}"#;
    let a = attrappe(vec![vec![http("200 OK", "", koerper)]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst_erststart(a.port, &heim, &ERSTSTART, None);
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/models", b""));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(teil_bytes(&z), koerper);
    assert_eq!(d.antwortform_zahl(), 0);
    assert_eq!(d.antwortform_letzte(), None);
}

#[test]
fn aufruf_traegt_schluessel_und_identity_und_gibt_alles_weiter() {
    // L9 (c): a body of 40 bytes arrives whole, then `ende` without a code
    let koerper = [b'k'; 40];
    let a = attrappe(vec![vec![http(
        "200 OK",
        "content-type: text/event-stream\r\nset-cookie: a=b\r\n",
        &koerper,
    )]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("POST", "/chat/completions", b"{\"x\":1}"));
    let z = c.bis_schluss();
    assert_eq!(z[0]["art"], "beginn");
    assert_eq!(z[0]["status"], 200);
    assert_eq!(z[0]["kopf"]["content-type"], "text/event-stream");
    assert!(z[0]["kopf"].get("set-cookie").is_none());
    assert_eq!(teil_bytes(&z), koerper);
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    let anfrage = String::from_utf8(a.anfragen.lock().expect("lock")[0].clone()).expect("utf-8");
    assert!(anfrage.starts_with("POST /chat/completions HTTP/1.1\r\n"));
    assert!(
        anfrage.contains(&format!("authorization: Bearer {MARKE}\r\n")),
        "the target got the key"
    );
    assert!(anfrage.contains("accept-encoding: identity\r\n"));
    assert!(
        !anfrage
            .to_ascii_lowercase()
            .contains("sec-websocket-extensions")
    );
    assert!(anfrage.ends_with("{\"x\":1}"));
    assert_eq!(c.marken(), 0);
}

#[test]
fn w3_leck_im_koerper_im_kopf_und_ueber_teilgrenzen() {
    let marke = MARKE.as_bytes();
    let (vorne, hinten) = marke.split_at(7);
    let mut geteilt1 = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n".to_vec();
    geteilt1.extend_from_slice(format!("{:x}\r\n", 9 + vorne.len()).as_bytes());
    geteilt1.extend_from_slice(b"daten vor");
    geteilt1.extend_from_slice(vorne);
    geteilt1.extend_from_slice(b"\r\n");
    let mut geteilt2 = format!("{:x}\r\n", hinten.len() + 4).into_bytes();
    geteilt2.extend_from_slice(hinten);
    geteilt2.extend_from_slice(b" end\r\n0\r\n\r\n");
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "",
            format!("fehler: token {MARKE} ungueltig").as_bytes(),
        )],
        vec![http(
            "401 Unauthorized",
            &format!("x-echo: Bearer {MARKE}\r\n"),
            b"",
        )],
        vec![geteilt1, geteilt2],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["art"], "ende");
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(z.len(), 1, "no beginn with the key in a head");
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    assert_eq!(c.marken(), 0, "no mark in any byte to the bridge");
    assert_eq!(zaehlen(&teil_bytes(&z), marke), 0);
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        3,
        "the target got every call"
    );
    let k = kette_text(&heim);
    assert_eq!(zaehlen(k.as_bytes(), marke), 0, "no value in the chain");
    assert_eq!(d.leck_treffer(), 3, "one journal line per hit");
    // F4: one chain line per hit, with the op and the target only
    leck_zeilen_pruefen(&heim, 3, "modell.aufruf", "copilot");
}

/// The chain of `heim` holds exactly `n` lines `halter.leck_gesperrt`, each with the fields of the contract (section 9),
/// `op` and `ziel` as given, no mark of the value, and the chain is whole for `formate` (the kind is in its register).
fn leck_zeilen_pruefen(heim: &Path, n: usize, op: &str, ziel: &str) {
    let k = kette_text(heim);
    assert_eq!(
        zaehlen(k.as_bytes(), MARKE.as_bytes()),
        0,
        "no value in the chain"
    );
    let zeilen: Vec<Value> = k
        .lines()
        .map(|z| serde_json::from_str::<Value>(z).expect("line"))
        .filter(|z| z["aktionsname"] == "halter.leck_gesperrt")
        .collect();
    assert_eq!(zeilen.len(), n, "one chain line per hit");
    for z in &zeilen {
        let mut felder: Vec<&str> = z
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        felder.sort_unstable();
        assert_eq!(
            felder,
            [
                "aktionsname",
                "ergebnis",
                "fassung",
                "grund",
                "op",
                "vorher",
                "zeit",
                "ziel"
            ]
        );
        assert_eq!(z["ergebnis"], "abgelehnt");
        assert_eq!(z["grund"], "leck");
        assert_eq!(z["op"], op);
        assert_eq!(z["ziel"], ziel);
    }
    let p = formate::kette::pruefe_quelle(
        &heim.join("kette"),
        "halter",
        formate::kette::NULL_HASH,
        None,
    )
    .expect("source");
    assert!(p.ok(), "{:?}", p.bruch);
}

fn prozent(bytes: &[u8], gross: bool) -> String {
    bytes
        .iter()
        .map(|b| {
            if gross {
                format!("%{b:02X}")
            } else {
                format!("%{b:02x}")
            }
        })
        .collect()
}

#[test]
fn w3_b_kodierte_rueckgabe_wird_gesperrt_harmloses_geht_durch() {
    let mit = b64::kodieren(format!("x{MARKE}").as_bytes());
    let ohne_url = mit
        .trim_end_matches('=')
        .replace('+', "-")
        .replace('/', "_");
    let mut geteilt1 = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n".to_vec();
    let p = prozent(MARKE.as_bytes(), false);
    let (vorne, hinten) = p.split_at(20);
    geteilt1.extend_from_slice(format!("{:x}\r\n{vorne}\r\n", vorne.len()).as_bytes());
    let geteilt2 = format!("{:x}\r\n{hinten}\r\n0\r\n\r\n", hinten.len()).into_bytes();
    let harmlos = b"kein Schluessel hier, 100% harmlos, aGd4MS1n".to_vec();
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "",
            format!("{{\"t\":\"{mit}\"}}").as_bytes(),
        )],
        vec![http("200 OK", &format!("x-echo: {ohne_url}\r\n"), b"")],
        vec![http(
            "200 OK",
            &format!("x-echo: {}\r\n", prozent(MARKE.as_bytes(), true)),
            b"",
        )],
        vec![geteilt1, geteilt2],
        vec![http("200 OK", "x-echo: harmlos\r\n", &harmlos)],
    ]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    for fall in [
        "base64 im koerper",
        "base64url im kopf",
        "prozent im kopf",
        "prozent ueber teile",
    ] {
        c.senden(&aufruf("GET", "/copilot_internal/user", b""));
        let z = c.bis_schluss();
        assert_eq!(
            letzte_code(&z).as_deref(),
            Some("halter.leck_gesperrt"),
            "{fall}"
        );
        let t = teil_bytes(&z);
        assert_eq!(zaehlen(&t, &mit.as_bytes()[..20]), 0, "{fall}");
        assert_eq!(
            zaehlen(&t, vorne.as_bytes()),
            0,
            "the first part stayed back: {fall}"
        );
    }
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(
        z.last().expect("end")["ok"],
        json!(true),
        "harmless text goes through"
    );
    assert_eq!(teil_bytes(&z), harmlos);
    assert_eq!(d.leck_treffer(), 4, "the harmless answer is no hit");
}

#[test]
fn l7_a_komprimiert_geht_nicht_durch() {
    let a = attrappe(vec![vec![http(
        "200 OK",
        "content-encoding: gzip\r\n",
        b"\x1f\x8b\x08\x00abcdef",
    )]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    let z = c.bis_schluss();
    assert_eq!(z.len(), 1);
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.ziel_komprimiert"));
    assert_eq!(teil_bytes(&z), Vec::<u8>::new());
}

#[test]
fn w7_rolle_trennt_die_ops() {
    let a = attrappe(vec![]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE), ("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&aufruf("GET", "/copilot_internal/user", b""));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.rolle")
    );
    c.senden(&json!({"fassung": 1, "id": "s-1", "nachweis": "nw", "name": "mcp.tickets", "op": "schluessel.setzen",
        "wert_b64": "YmVpc3BpZWx3ZXJ0"}));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.rolle")
    );
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(
        &json!({"argumente": {}, "fassung": 1, "id": 7, "lauf": "l-beispiel", "op": "mcp.aufruf",
        "person": PERSON, "server": "tickets", "vorschau_sha256": null, "werkzeug": "get"}),
    );
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.rolle")
    );
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        0,
        "the stand-in counts 0"
    );
}

/// L3 (addendum "Rolle aus der Gruppe"): the role comes from the effective gid of the peer only. The same peer (this
/// process) gets `halter.peer` when its gid is not the gid of a role group, and the role when it is (counter-check);
/// a peer of another uid gets no role with the right gid.
#[test]
fn l3_rolle_nur_mit_der_gid_der_rollengruppe() {
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let k = json!({"fassung": 1, "konten": {uid().to_string(): {"person": null}},
        "rollen": {"bruecke": "wb-bruecke"}, "ziele": {}, "server": {}, "kids": {}});
    let mit = |gruppen: BTreeMap<u32, Rolle>, konto: u32| {
        Arc::new(Dienst::neu(
            konfig::lesen(k.to_string().as_bytes()).expect("konfig"),
            Ablage::neu(&heim, uid()),
            konto,
            START,
            gruppen,
            None,
            uhr,
        ))
    };
    let frage = json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"});
    // another gid: no role, the connection is closed
    let mut c = verbinden(&mit(
        BTreeMap::from([(egid().wrapping_add(1), Rolle::Bruecke)]),
        uid(),
    ));
    c.senden(&frage);
    let z = c.bis_schluss();
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.peer"));
    assert_eq!(z[0]["id"], Value::Null);
    c.ende_erwartet("the holder closes after halter.peer");
    // no role group at all
    let mut c = verbinden(&mit(BTreeMap::new(), uid()));
    c.senden(&frage);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.peer")
    );
    // counter-check: the same peer with the gid of the role group gets the role
    let mut c = verbinden(&mit(eigene_gid_als("bruecke"), uid()));
    c.senden(&frage);
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(true));
    // the right gid, but the holder runs under another uid: the peer is no account of the holder
    let mut c = verbinden(&mit(eigene_gid_als("bruecke"), uid().wrapping_add(1)));
    c.senden(&frage);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.peer")
    );
}

/// Case and counter-check of L3 across processes, without root: a client program (the interpreter of the system) asks
/// `schluessel.vorhanden` once as it is and once started through `newgrp` on a supplementary group of the test user, so
/// that its effective gid is that group. Only the second gets the role. Skipped (with a line on stderr) when the
/// machine has no interpreter, no `newgrp`, the user no supplementary group, or `newgrp` does not change the gid (a
/// process under `no_new_privs`, as inside an enclosure).
#[test]
#[allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    reason = "a skip and the measured gids are named on stderr; one test holds case and counter-check"
)]
fn l3_gleicher_peer_mit_und_ohne_rollengruppe() {
    let Ok(py) = std::fs::canonicalize("/usr/bin/python3") else {
        eprintln!("skipped: no /usr/bin/python3");
        return;
    };
    let Some(newgrp) = ["/usr/bin/newgrp", "/bin/newgrp"]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
    else {
        eprintln!("skipped: no newgrp");
        return;
    };
    let eigene = egid();
    let Some(gruppe) = rustix::process::getgroups()
        .unwrap_or_default()
        .into_iter()
        .map(rustix::process::Gid::as_raw)
        .find(|g| *g != eigene && *g != 0)
    else {
        eprintln!("skipped: the test user has no supplementary group");
        return;
    };
    let name = std::fs::read_to_string("/etc/group")
        .unwrap_or_default()
        .lines()
        .find_map(|z| {
            let f: Vec<&str> = z.split(':').collect();
            (f.len() == 4 && f[2].parse::<u32>().ok() == Some(gruppe)).then(|| f[0].to_owned())
        });
    let Some(name) = name else {
        eprintln!("skipped: the supplementary group {gruppe} has no name in /etc/group");
        return;
    };
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let k = json!({"fassung": 1, "konten": {uid().to_string(): {"person": null}},
        "rollen": {"bruecke": "wb-bruecke"}, "ziele": {}, "server": {}, "kids": {}});
    let d = Arc::new(Dienst::neu(
        konfig::lesen(k.to_string().as_bytes()).expect("konfig"),
        Ablage::neu(&heim, uid()),
        uid(),
        START,
        BTreeMap::from([(gruppe, Rolle::Bruecke)]),
        None,
        uhr,
    ));
    let o = ordner();
    let sock = o.join("l3.sock");
    let hoerer = std::os::unix::net::UnixListener::bind(&sock).expect("bind");
    let skript = o.join("client.py");
    std::fs::write(
        &skript,
        "import os, select, socket, sys\n\
         s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)\ns.connect(sys.argv[1])\n\
         s.sendall(b'{\"fassung\":1,\"id\":\"q-1\",\"name\":\"anmeldung.copilot\",\"op\":\"schluessel.vorhanden\"}\\n')\n\
         b = b''\nn = 0\nwhile not b.endswith(b'\\n'):\n\
         \x20   if select.select([s], [], [], 0.1)[0]:\n\
         \x20       t = s.recv(4096)\n        if not t: break\n        b += t\n\
         \x20   else:\n        n += 1\n\
         \x20       open(sys.argv[2] + '.neu', 'w').write('%d %d' % (n, len(b)))\n\
         \x20       os.replace(sys.argv[2] + '.neu', sys.argv[2] + '.stand')\n\
         open(sys.argv[2], 'w').write(str(os.getegid()) + ' ' + b.decode())\n",
    )
    .expect("script");
    let lauf = |mit_gruppe: bool, aus: &Path| {
        let d = Arc::clone(&d);
        let h = hoerer.try_clone().expect("listener");
        let faden = Arc::new(std::sync::OnceLock::new());
        let f2 = Arc::clone(&faden);
        let server = std::thread::spawn(move || {
            let (s, _) = h.accept().expect("accept");
            if let Ok(p) = std::fs::read_link("/proc/thread-self") {
                let _ = f2.set((Path::new("/proc").join(p), inode(&s)));
            }
            d.verbindung(s);
        });
        let mut kind = if mit_gruppe {
            let mut k = std::process::Command::new(&newgrp)
                .arg(&name)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("newgrp");
            let zeile = format!(
                "exec {} -I {} {} {}\n",
                py.display(),
                skript.display(),
                sock.display(),
                aus.display()
            );
            k.stdin
                .take()
                .expect("stdin")
                .write_all(zeile.as_bytes())
                .expect("write");
            k
        } else {
            std::process::Command::new(&py)
                .arg("-I")
                .arg(&skript)
                .arg(&sock)
                .arg(aus)
                .env_clear()
                .spawn()
                .expect("python")
        };
        let stand = PathBuf::from(format!("{}.stand", aus.display()));
        let st = warten_auf_den_client(&mut kind, &faden, &stand);
        if !st.success() {
            // the client never connected; let the server thread go
            let _ = UnixStream::connect(&sock);
        }
        server.join().expect("server");
        std::fs::read_to_string(aus).unwrap_or_default()
    };
    let ohne = lauf(false, &o.join("ohne.txt"));
    let mit = lauf(true, &o.join("mit.txt"));
    let gid_von = |t: &str| t.split(' ').next().and_then(|g| g.parse::<u32>().ok());
    if gid_von(&mit) != Some(gruppe) {
        eprintln!("skipped: newgrp did not change the effective gid ({mit:?})");
        return;
    }
    assert_eq!(gid_von(&ohne), Some(eigene));
    assert!(
        ohne.contains("\"code\":\"halter.peer\""),
        "without the role group: {ohne}"
    );
    assert!(
        mit.contains("\"ok\":true,\"vorhanden\":true"),
        "with the role group as effective gid: {mit}"
    );
    eprintln!("measured: gid {eigene} -> halter.peer, gid {gruppe} ({name}) -> role bruecke");
}

/// Waits for the end of the client program of L3 across processes. After its one line the client waits for the
/// answer in steps of 100 ms and writes `<step> <bytes received>` to `stand` after every empty step. It fails at once
/// when the holder waits in a read on the connection and two steps of the client, both begun after that, found
/// nothing: the holder has sent nothing and waits for a request, neither side will move. Time only bounds a real hang.
fn warten_auf_den_client(
    kind: &mut std::process::Child,
    faden: &std::sync::OnceLock<(PathBuf, u64)>,
    stand: &Path,
) -> std::process::ExitStatus {
    let lies = || -> Option<(u64, u64)> {
        let t = std::fs::read_to_string(stand).ok()?;
        let (n, b) = t.split_once(' ')?;
        Some((n.parse().ok()?, b.parse().ok()?))
    };
    let beginn = std::time::Instant::now();
    // the step of the client when the holder was first seen waiting
    let mut seit: Option<u64> = None;
    loop {
        if let Some(st) = kind.try_wait().expect("wait") {
            return st;
        }
        let wartet = faden.get().is_some_and(|(_, i)| liest_eines(&[*i]));
        let schritt = lies();
        let mut grund = "";
        if wartet {
            let jetzt = schritt.map_or(0, |(n, _)| n);
            let erster = *seit.get_or_insert(jetzt);
            if let Some((n, 0)) = schritt
                && n >= erster + 2
            {
                grund = "the holder waits for a request after the client sent its line, without an answer";
            }
        } else {
            seit = None;
        }
        if grund.is_empty() && beginn.elapsed() > HAENGEGRENZE {
            grund = "the client did not end within the bound of a hang";
        }
        if !grund.is_empty() {
            let z = zustand(faden.get().map(|(f, _)| f));
            let _ = kind.kill();
            let _ = kind.wait();
            panic!("{grund}; client step {schritt:?}; holder: {z}");
        }
        std::thread::sleep(TAKT);
    }
}

/// One stream: the lines to the bridge, the marks in their bytes, the stand-in and the hits of the leak lock.
fn strom_lauf(nach_101: Vec<Vec<u8>>) -> (Vec<Value>, usize, Attrappe, u64) {
    let mut teile = vec![ACHT_ZEHN_EINS.to_vec()];
    teile.extend(nach_101);
    let a = attrappe(vec![teile]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&strom_anfrage());
    let z = c.bis_schluss();
    let m = c.marken();
    let treffer = d.leck_treffer();
    leck_zeilen_pruefen(
        &heim,
        usize::try_from(treffer).expect("count"),
        "modell.strom",
        "copilot",
    );
    (z, m, a, treffer)
}

#[test]
fn l6_leck_im_strom_ueber_zwei_rahmen_mit_ping() {
    let (vorne, hinten) = MARKE.as_bytes().split_at(9);
    let (z, m, _a, t) = strom_lauf(vec![
        rahmen(false, 0x1, vorne),
        rahmen(true, 0x9, b"p"),
        rahmen(true, 0x0, hinten),
    ]);
    assert_eq!(z[0]["status"], 101);
    assert!(z[0]["kopf"].get("upgrade").is_none() && z[0]["kopf"].get("connection").is_none());
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    assert_eq!(m, 0);
    assert_eq!(t, 1, "one journal line per hit");
    let (z, m, _a, t) = strom_lauf(vec![rahmen(true, 0x1, format!("x {MARKE} y").as_bytes())]);
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.leck_gesperrt"));
    assert_eq!(m, 0);
    assert_eq!(t, 1);
    let kodiert = b64::kodieren(MARKE.as_bytes());
    let (z, _m, _a, t) = strom_lauf(vec![rahmen(true, 0x1, kodiert.as_bytes())]);
    assert_eq!(
        letzte_code(&z).as_deref(),
        Some("halter.leck_gesperrt"),
        "Base64 in a frame"
    );
    assert_eq!(t, 1);
}

#[test]
fn l7_b_und_c_strom_komprimiert() {
    let a = attrappe(vec![vec![
        b"HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nSec-WebSocket-Extensions: permessage-deflate\r\n\r\n".to_vec(),
        rahmen(true, 0x1, b"abc"),
    ]]);
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&strom_anfrage());
    let z = c.bis_schluss();
    assert_eq!(z.len(), 1, "no 101 goes out");
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.ziel_komprimiert"));
    let anfrage = String::from_utf8(a.anfragen.lock().expect("lock")[0].clone()).expect("utf-8");
    assert!(
        anfrage.contains("accept-encoding: identity\r\n")
            && anfrage.contains("upgrade: websocket\r\n")
    );
    assert!(!anfrage.to_ascii_lowercase().contains("extensions"));
    let mut rsv = rahmen(true, 0x1, b"abcdef");
    rsv[0] |= 0x40;
    let (z, _, _a, _) = strom_lauf(vec![rsv]);
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.ziel_komprimiert"));
    assert!(teil_bytes(&z).is_empty(), "0 bytes of the frame");
}

#[test]
fn l8_rahmenregeln() {
    let verstoesse = [
        rahmen(false, 0x9, b"x"),
        rahmen(true, 0xA, &[0u8; 126]),
        rahmen(true, 0x8, &[0u8; 126]),
        rahmen(true, 0x0, b"x"),
        [rahmen(false, 0x1, b"a"), rahmen(true, 0x1, b"b")].concat(),
    ];
    for (i, v) in verstoesse.into_iter().enumerate() {
        let vorher = rahmen(true, 0x2, b"gut");
        let gesendet = [vorher.clone(), v.clone()].concat();
        let (z, _, a, t) = strom_lauf(vec![vorher.clone(), v]);
        assert_eq!(
            letzte_code(&z).as_deref(),
            Some("halter.ziel_fehler"),
            "case {i}"
        );
        assert_eq!(t, 0, "case {i}: a break is no hit of the leak lock");
        let bytes = teil_bytes(&z);
        // the payload "gut" is still kept back by the leak lock and is dropped with the break; at most the head of
        // the frame before the violation went out
        assert!(
            gesendet.starts_with(&bytes) && bytes.len() <= vorher.len() + 2,
            "case {i}: nothing after the break"
        );
        // the holder closed towards the stand-in: the stand-in saw the end
        for _ in 0..100 {
            if !a.danach.lock().expect("lock").is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            a.danach.lock().expect("lock").len(),
            1,
            "case {i}: connection to the stand-in closed"
        );
    }
    let f = [
        rahmen(false, 0x1, b"abc"),
        rahmen(true, 0x9, b"12345"),
        rahmen(true, 0x0, b"def"),
    ]
    .concat();
    let (z, _, _a, _) = strom_lauf(vec![f.clone()]);
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    assert_eq!(teil_bytes(&z), f);
    let g = rahmen(true, 0x9, &[7u8; 125]);
    let (z, _, _a, _) = strom_lauf(vec![g.clone()]);
    assert_eq!(teil_bytes(&z), g);
}

#[test]
fn l9_zurueckgehaltenes_geht_am_ende_hinaus() {
    let r = rahmen(true, 0x1, &[b'n'; 40]);
    let (z, _, _a, _) = strom_lauf(vec![r.clone()]);
    assert_eq!(teil_bytes(&z), r);
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    let mut b = r.clone();
    b.extend_from_slice(&rahmen(true, 0x1, &[b'o'; 30])[..12]);
    let (z, _, _a, _) = strom_lauf(vec![b.clone()]);
    assert_eq!(teil_bytes(&z), b, "everything the stand-in sent");
    assert!(z.last().expect("end").get("code").is_none());
}

#[test]
fn strom_teil_und_ende_dann_wieder_anfragen() {
    let a = attrappe_mit(
        vec![vec![ACHT_ZEHN_EINS.to_vec(), rahmen(true, 0x1, b"hallo")]],
        false,
    );
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&strom_anfrage());
    // beginn and the first part
    let b = c.eine().expect("beginn");
    assert_eq!(b["status"], 101);
    let t = c.eine().expect("teil");
    assert_eq!(t["art"], "teil");
    // another op while the stream is open: halter.strom
    c.senden(&json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"}));
    assert_eq!(c.eine().expect("refusal")["code"], "halter.strom");
    let harness = [
        0x81u8,
        0x85,
        1,
        2,
        3,
        4,
        b'a' ^ 1,
        b'b' ^ 2,
        b'c' ^ 3,
        b'd' ^ 4,
        b'e' ^ 1,
    ];
    c.senden(&json!({"daten_b64": b64::kodieren(&harness), "fassung": 1, "id": "m-2", "op": "strom.teil"}));
    c.senden(&json!({"fassung": 1, "id": "m-2", "op": "strom.ende"}));
    let z = c.bis_schluss();
    assert_eq!(z.last().expect("end")["ok"], json!(true));
    c.senden(&json!({"fassung": 1, "id": "q-2", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"}));
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(true));
    drop(c);
    for _ in 0..100 {
        if !a.danach.lock().expect("lock").is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        a.danach.lock().expect("lock")[0],
        harness,
        "the bytes of the harness arrive unchanged"
    );
}

const SCHEIN: &str = r#"{"daten":{"ablauf":1795000030000,"anfrage":"mf_0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d","art":"mcp-schein","ausgestellt":1794999990000,"digest":"ffc2016a50770971ab9be5aac167e9165ac7a0a189d6e5f7c9fac57af4acb258","einmal":"5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e","fassung":1,"kid":"stelle-mcp-1","lauf":"l-beispiel"},"signatur":"G7BKnSp/7wiP0ydW6cycjMf96T3EtW05zH1Rs8wexOQiD5QO1LdIuk31jKXY/pWMRyj4bat0H+907SiaxUx9Cg=="}"#;

fn schreibaufruf(status: &str) -> Value {
    json!({"argumente": {"id": "t-1", "status": status}, "fassung": 1, "id": 7, "lauf": "l-beispiel",
        "op": "mcp.aufruf", "person": PERSON, "schein": serde_json::from_str::<Value>(SCHEIN).expect("ticket"),
        "server": "tickets", "vorschau_sha256": "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3",
        "werkzeug": "update"})
}

fn mcp_antwort() -> Vec<Vec<u8>> {
    vec![http(
        "200 OK",
        "content-type: application/json\r\n",
        br#"{"id":1,"jsonrpc":"2.0","result":{"content":[{"text":"ok","type":"text"}]}}"#,
    )]
}

fn kette_text(heim: &Path) -> String {
    let mut t = String::new();
    for e in std::fs::read_dir(heim.join("kette")).expect("chain") {
        let p = e.expect("entry").path();
        if p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("halter-"))
        {
            t.push_str(&std::fs::read_to_string(p).expect("read"));
        }
    }
    t
}

#[test]
fn w4_einmal_einloesung() {
    // (a) the same ticket twice: ok, then benutzt; (d) a changed argument: digest
    let a = attrappe(vec![mcp_antwort(), mcp_antwort()]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&schreibaufruf("erledigt"));
    let z = c.bis_schluss();
    assert_eq!(z[0]["ok"], json!(true), "{z:?}");
    assert_eq!(z[0]["ergebnis"]["content"][0]["text"], "ok");
    c.senden(&schreibaufruf("erledigt"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schein_benutzt")
    );
    c.senden(&schreibaufruf("offen"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schein_digest")
    );
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        1,
        "the stand-in counts 1"
    );
    let anfrage = String::from_utf8(a.anfragen.lock().expect("lock")[0].clone()).expect("utf-8");
    assert!(anfrage.contains(&format!("authorization: Bearer {MARKE}\r\n")));
    assert!(anfrage.contains(r#""method":"tools/call""#));
    let k = kette_text(&heim);
    assert!(
        k.contains(r#""grund":"benutzt""#) && k.contains(r#""grund":"digest""#),
        "{k}"
    );
    assert!(!k.contains(MARKE) && !k.contains(PERSON) && !k.contains("t-1"));
    assert_eq!(c.marken(), 0);
}

#[test]
fn w4_b_acht_verbindungen_gleichzeitig() {
    let a = attrappe((0..8).map(|_| mcp_antwort()).collect());
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let faeden: Vec<_> = (0..8)
        .map(|_| {
            let d = Arc::clone(&d);
            std::thread::spawn(move || {
                let mut c = verbinden(&d);
                c.senden(&schreibaufruf("erledigt"));
                c.bis_schluss()
            })
        })
        .collect();
    let ergebnisse: Vec<Vec<Value>> = faeden
        .into_iter()
        .map(|f| f.join().expect("thread"))
        .collect();
    let ok = ergebnisse
        .iter()
        .filter(|z| z[0]["ok"] == json!(true))
        .count();
    let benutzt = ergebnisse
        .iter()
        .filter(|z| letzte_code(z).as_deref() == Some("halter.schein_benutzt"))
        .count();
    assert_eq!((ok, benutzt), (1, 7));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        a.anfragen.lock().expect("lock").len(),
        1,
        "exactly one call"
    );
}

#[test]
fn w4_c_nach_neustart_abgelaufen() {
    let a = attrappe(vec![mcp_antwort()]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    // a holder started after the ticket was issued
    let d = dienst("hub", a.port, &heim, 1_794_999_995_000);
    let mut c = verbinden(&d);
    c.senden(&schreibaufruf("erledigt"));
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.schein_abgelaufen")
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 0);
    assert!(kette_text(&heim).contains(r#""grund":"abgelaufen""#));
}

#[test]
fn w8_fremde_person_bekommt_keinen_token() {
    let a = attrappe(vec![]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    let mut v = schreibaufruf("erledigt");
    v["person"] = json!("ps_b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2");
    c.senden(&v);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.kein_schluessel")
    );
    assert_eq!(a.anfragen.lock().expect("lock").len(), 0);
}

#[test]
fn mcp_leck_im_ergebnis_wird_gesperrt_und_gezaehlt() {
    let a = attrappe(vec![
        vec![http(
            "200 OK",
            "content-type: application/json\r\n",
            format!(r#"{{"id":1,"jsonrpc":"2.0","result":{{"content":[{{"text":"{}","type":"text"}}]}}}}"#,
                b64::kodieren(MARKE.as_bytes())).as_bytes(),
        )],
        vec![http("500 Internal Server Error", "", b"")],
    ]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    let lesend = json!({"argumente": {"id": "t-1"}, "fassung": 1, "id": 8, "lauf": "l-beispiel", "op": "mcp.aufruf",
        "person": PERSON, "server": "tickets", "vorschau_sha256": null, "werkzeug": "get"});
    c.senden(&lesend);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.leck_gesperrt")
    );
    assert_eq!(d.leck_treffer(), 1);
    c.senden(&lesend);
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.ziel_fehler")
    );
    assert_eq!(d.leck_treffer(), 1, "another error is no hit");
    leck_zeilen_pruefen(&heim, 1, "mcp.aufruf", "tickets");
}

#[test]
fn mcp_fehler_nur_als_zahl() {
    let a = attrappe(vec![vec![http(
        "200 OK",
        "content-type: application/json\r\n",
        format!(r#"{{"error":{{"code":-32603,"message":"token {MARKE} ungueltig"}},"id":1,"jsonrpc":"2.0"}}"#).as_bytes(),
    )]]);
    let heim = heim_mit(&[("mcp.tickets", MARKE)]);
    let d = dienst("hub", a.port, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&json!({"argumente": {"id": "t-1"}, "fassung": 1, "id": 8, "lauf": "l-beispiel", "op": "mcp.aufruf",
        "person": PERSON, "server": "tickets", "vorschau_sha256": null, "werkzeug": "get"}));
    let z = c.bis_schluss();
    assert_eq!(z[0]["code"], "halter.mcp_fehler");
    assert_eq!(z[0]["mcp_code"], -32603);
    assert_eq!(c.marken(), 0);
}

#[test]
fn suche_zaehlt_nur_zahlen_und_folgt_keinem_verweis() {
    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let arbeit = ordner();
    std::fs::write(arbeit.join("a.txt"), format!("x {MARKE} y {MARKE}")).expect("write");
    std::fs::create_dir(arbeit.join("unter")).expect("dir");
    std::fs::write(arbeit.join("unter/b.txt"), "nichts").expect("write");
    let datei = heim.join(".local/state/agent-workbench/halter/geheim/anmeldung.copilot");
    std::os::unix::fs::symlink(&datei, arbeit.join("verweis")).expect("link");
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    c.senden(&json!({"fassung": 1, "id": "u-1", "op": "modell.suche", "pfade": [arbeit.to_str().expect("utf-8")],
        "ziel": "copilot"}));
    let z = c.bis_schluss();
    assert_eq!(
        z[0]["suche"],
        json!({"dateien": 2, "nicht_lesbar": 0, "treffer": 2, "uebersprungen": 1})
    );
    assert_eq!(c.marken(), 0);
}

/// `Peer::lebt` through the pidfd: a peer program (the interpreter of the system, if there is one) connects, is
/// measured, then ends; the bound process is gone and no new one takes its place.
#[test]
fn peer_lebt_nur_solange_der_prozess_laeuft() {
    let Ok(py) = std::fs::canonicalize("/usr/bin/python3") else {
        return;
    };
    let o = ordner();
    let pfad = o.join("p.sock");
    let hoerer = std::os::unix::net::UnixListener::bind(&pfad).expect("bind");
    let mut kind = std::process::Command::new(&py)
        .arg("-I")
        .arg("-c")
        .arg("import socket,sys\ns=socket.socket(socket.AF_UNIX)\ns.connect(sys.argv[1])\nsys.stdin.read()")
        .arg(&pfad)
        .stdin(std::process::Stdio::piped())
        .env_clear()
        .spawn()
        .expect("python");
    let (strom, _) = hoerer.accept().expect("accept");
    let p = halter::peer::messen(&strom).expect("peer");
    assert_eq!(p.pid, kind.id());
    assert_eq!(p.programm.as_deref(), Some(py.as_path()));
    assert!(p.lebt());
    let gruppen = BTreeMap::from([(p.gid, Rolle::Bruecke)]);
    assert_eq!(
        halter::peer::rolle(&p, uid(), &gruppen),
        Some(Rolle::Bruecke)
    );
    drop(kind.stdin.take());
    kind.wait().expect("wait");
    assert!(!p.lebt(), "the pidfd shows the end of the process");
    assert_eq!(
        halter::peer::rolle(&p, uid(), &gruppen),
        None,
        "an ended peer has no role"
    );
}

#[test]
fn zeile_genau_an_der_grenze_wird_gelesen() {
    let heim = heim_mit(&[]);
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    let mut z = vec![b'x'; halter::pruefung::ZEILE_MAX];
    z.push(b'\n');
    let schreiber = c.nebenher_schreiben(z);
    // exactly 32 MiB is no break of stage 1: the line is read and refused as JSON, and the connection stays
    assert_eq!(
        letzte_code(&c.bis_schluss()).as_deref(),
        Some("halter.json_ungueltig")
    );
    let _ = schreiber.join();
    c.senden(&json!({"fassung": 1, "id": "q-1", "name": "mcp.x", "op": "schluessel.vorhanden"}));
    assert_eq!(c.bis_schluss()[0]["vorhanden"], json!(false));
}

#[test]
fn zu_grosse_zeile_schliesst_ohne_rest() {
    let heim = heim_mit(&[]);
    let d = dienst("bruecke", 9, &heim, START);
    let mut c = verbinden(&d);
    let gross = vec![b'x'; halter::pruefung::ZEILE_MAX + 10];
    let schreiber = c.nebenher_schreiben(gross);
    let z = c.bis_schluss();
    assert_eq!(letzte_code(&z).as_deref(), Some("halter.zu_gross"));
    c.ende_erwartet("the holder closes after halter.zu_gross");
    let _ = schreiber.join();
}

/// A short folder for a socket (a Unix socket path has at most 107 bytes): below `TMPDIR` if that is short (the
/// enclosure sets it), else below `$HOME/.cache`.
fn kurzer_ordner() -> PathBuf {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let basis = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.as_os_str().len() <= 48)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".cache"));
    let p = basis.join(format!("hd-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("short folder");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("mode");
    p
}

/// A client on the socket at `p`; its end on the side of the holder is the connection accepted after the connect.
fn verbinden_mit(p: &Path) -> Client {
    let vorher = angenommen(p);
    let a = UnixStream::connect(p).expect("connect");
    neuer_client(
        a,
        Gegenseite::Hoerer {
            pfad: p.to_owned(),
            vorher,
        },
    )
}

/// `true` if the connection was served: the answer to `schluessel.vorhanden` comes.
fn bedient(c: &mut Client) -> bool {
    // a connection closed unanswered may refuse the request already
    let frage = json!({"fassung": 1, "id": "q-1", "name": "anmeldung.copilot", "op": "schluessel.vorhanden"});
    if c.strom.write_all(&zeile(&frage)).is_err() {
        return false;
    }
    c.bis_schluss()
        .first()
        .is_some_and(|z| z["vorhanden"] == json!(true))
}

/// The accept loop of the program (section 3): it serves a connection, closes one above the limit unanswered, serves
/// again once a place is free and ends after the stop flag (SIGTERM and SIGINT set it in the program).
#[test]
fn annahme_bedient_haelt_die_grenze_und_endet_mit_halt() {
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::AtomicBool;

    let heim = heim_mit(&[("anmeldung.copilot", MARKE)]);
    let d = dienst("bruecke", 9, &heim, START);
    let o = kurzer_ordner();
    let pfad = o.join("h.sock");
    let hoerer = UnixListener::bind(&pfad).expect("bind");
    hoerer.set_nonblocking(true).expect("non-blocking");
    let halt = Arc::new(AtomicBool::new(false));
    let (fertig_tx, fertig_rx) = std::sync::mpsc::channel();
    let halt2 = Arc::clone(&halt);
    let schleife = std::thread::spawn(move || {
        halter::dienst::annehmen(&d, &hoerer, &halt2, 1);
        let _ = fertig_tx.send(());
    });
    // the first connection is served and stays open
    let mut a = verbinden_mit(&pfad);
    assert!(bedient(&mut a), "the first connection was not served");
    // a second one above the limit of 1 is closed without an answer
    let mut b = verbinden_mit(&pfad);
    b.ende_erwartet("the connection above the limit was not closed");
    // after the end of the first one there is a place again; the test waits for that event, the bound is only against
    // a real hang
    drop(a);
    let ende = std::time::Instant::now() + HAENGEGRENZE;
    loop {
        let mut c = verbinden_mit(&pfad);
        if bedient(&mut c) {
            break;
        }
        assert!(
            std::time::Instant::now() < ende,
            "no place within {HAENGEGRENZE:?} after the end of the first connection (bound against a hang)"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    halt.store(true, Ordering::Relaxed);
    assert!(
        fertig_rx.recv_timeout(HAENGEGRENZE).is_ok(),
        "the accept loop did not end within {HAENGEGRENZE:?} after the stop flag (bound against a hang)"
    );
    schleife.join().expect("loop");
    let _ = std::fs::remove_dir_all(&o);
}
