//! The program `werkbank-halter` under Linux in the test build (seams of `pruefbau`, `WB_TESTUMGEBUNG=1`): the order of
//! the start steps and their exit codes (section 2.1), the exclusive endpoint (L2), no local server with a token (L4),
//! the shielding against processes of the same uid (L5 without `ptrace`), the end by SIGTERM, the role groups of step 2
//! (section 2.3a, L10) and the role of a peer that set `PR_SET_DUMPABLE` 0 (addendum "Rolle aus der Gruppe": the role
//! comes from the effective gid, which survives the shielding).
#![cfg(all(target_os = "linux", feature = "pruefbau"))]

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
const MARKE: &str = "hgx5-l5-geheimwert";
/// The gid of the role group `wb-bruecke` in the file of the groups of a set-up: no gid of this test process.
const FREMDE_GID: u32 = 3_999_999_999;
/// The primary group of the person in the file of the accounts of a set-up (a seam file): never a role group.
const HAUPTGRUPPE: u32 = 3_999_999_998;

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

fn egid() -> u32 {
    rustix::process::getegid().as_raw()
}

/// root as this process sees it: 0, or the overflow uid inside a user namespace (an enclosure with bwrap).
fn root_uid() -> u32 {
    std::fs::metadata("/usr").expect("/usr").uid()
}

fn ordner() -> PathBuf {
    neuer_ordner(Path::new(env!("CARGO_TARGET_TMPDIR")))
}

/// A short folder for the socket (a Unix socket path has at most 107 bytes): below `TMPDIR` if that is short (the
/// enclosure sets it), else below `$HOME/.cache`, as the suites with a socket do.
fn kurzer_ordner() -> PathBuf {
    let basis = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.as_os_str().len() <= 48)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".cache"));
    neuer_ordner(&basis)
}

/// A new folder of its own below `basis` (mode 0700), like `mkdtemp`: the name carries pid, counter and 32 random bits,
/// and `mkdir` fails on an existing name, so a folder of another test or run is never taken over or removed; a taken
/// name draws again.
fn neuer_ordner(basis: &Path) -> PathBuf {
    for _ in 0..16 {
        let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let mut z = [0u8; 4];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut z))
            .expect("urandom");
        let p = basis.join(format!(
            "hs-{}-{n}-{:08x}",
            std::process::id(),
            u32::from_ne_bytes(z)
        ));
        match std::fs::DirBuilder::new().mode(0o700).create(&p) {
            Ok(()) => {
                modus(&p, 0o700);
                return p;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => panic!("folder {}: {e}", p.display()),
        }
    }
    panic!("no free folder name below {}", basis.display());
}

fn modus(p: &Path, m: u32) {
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).expect("mode");
}

struct Aufbau {
    wurzel: PathBuf,
    konfig: PathBuf,
    heim: PathBuf,
    laufzeit: PathBuf,
}

impl Drop for Aufbau {
    fn drop(&mut self) {
        // only the two folders this set-up made, each by its exact name
        let _ = std::fs::remove_dir_all(&self.laufzeit);
        let _ = std::fs::remove_dir_all(&self.wurzel);
    }
}

impl Aufbau {
    fn neu(konfig: &Value) -> Self {
        let wurzel = ordner();
        let etc = wurzel.join("etc");
        std::fs::create_dir(&etc).expect("etc");
        modus(&etc, 0o755);
        let datei = etc.join("halter.json");
        std::fs::write(&datei, konfig.to_string()).expect("konfig");
        modus(&datei, 0o644);
        let heim = wurzel.join("heim");
        std::fs::create_dir(&heim).expect("heim");
        modus(&heim, 0o700);
        std::fs::write(
            wurzel.join("passwd"),
            format!(
                "root:x:0:0:root:/root:/bin/sh\nperson:x:{}:{HAUPTGRUPPE}::{}:/bin/sh\n",
                uid(),
                heim.display()
            ),
        )
        .expect("passwd");
        std::fs::write(
            wurzel.join("group"),
            format!("root:x:0:\nwb-bruecke:x:{FREMDE_GID}:\n"),
        )
        .expect("group");
        let laufzeit = kurzer_ordner();
        Aufbau {
            konfig: datei,
            heim,
            laufzeit,
            wurzel,
        }
    }

    fn befehl(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_werkbank-halter"));
        c.env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("WB_TESTUMGEBUNG", "1")
            .env("WB_HALTER_KONFIG", &self.konfig)
            .env("WB_HALTER_KONFIG_EIGNER", uid().to_string())
            .env("WB_HALTER_PASSWD", self.wurzel.join("passwd"))
            .env("WB_HALTER_GROUP", self.wurzel.join("group"))
            .env("WB_HALTER_LAUFZEIT", &self.laufzeit)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    }

    fn socket(&self) -> PathBuf {
        self.laufzeit.join("wb-halter/halter.sock")
    }

    fn geheim(&self) -> PathBuf {
        self.heim.join(".local/state/agent-workbench/halter/geheim")
    }

    /// Makes every level of the store with 0700 and puts the secret `anmeldung.copilot` (0600) in it.
    fn mit_anmeldung(&self) {
        let mut p = self.heim.clone();
        for e in halter::ablage::EBENEN {
            p = p.join(e);
            if !p.exists() {
                std::fs::create_dir(&p).expect("level");
            }
            modus(&p, 0o700);
        }
        let datei = p.join("anmeldung.copilot");
        std::fs::write(&datei, format!("{MARKE}\n")).expect("secret");
        modus(&datei, 0o600);
    }

    /// Runs the program to its end (at most 20 s) and returns exit code and standard error.
    #[allow(
        clippy::unused_self,
        reason = "every call reads as a step of the set-up"
    )]
    fn ende(&self, c: &mut Command) -> (i32, String) {
        let mut kind = c.spawn().expect("spawn");
        let ende = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(st) = kind.try_wait().expect("wait") {
                let mut e = String::new();
                kind.stderr
                    .take()
                    .expect("stderr")
                    .read_to_string(&mut e)
                    .expect("read");
                return (st.code().unwrap_or(-1), e);
            }
            if Instant::now() > ende {
                let _ = kind.kill();
                let _ = kind.wait();
                panic!("the holder did not end within 20 s");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Starts the holder and waits until a socket other than the one with inode `alt` lies at the path.
    fn laufend_nach_neuem_socket(&self, alt: u64) -> Child {
        let mut kind = self.befehl().spawn().expect("spawn");
        let ende = Instant::now() + Duration::from_secs(20);
        while std::fs::symlink_metadata(self.socket()).map_or(true, |m| m.ino() == alt) {
            if let Some(st) = kind.try_wait().expect("wait") {
                let mut e = String::new();
                let _ = kind.stderr.take().expect("stderr").read_to_string(&mut e);
                panic!("the holder ended with {st} before its socket: {e}");
            }
            assert!(Instant::now() < ende, "no new socket within 20 s");
            std::thread::sleep(Duration::from_millis(20));
        }
        kind
    }

    fn laufend(&self) -> Child {
        let mut kind = self.befehl().spawn().expect("spawn");
        let ende = Instant::now() + Duration::from_secs(20);
        while !self.socket().exists() {
            if let Some(st) = kind.try_wait().expect("wait") {
                let mut e = String::new();
                let _ = kind.stderr.take().expect("stderr").read_to_string(&mut e);
                panic!("the holder ended with {st} before its socket: {e}");
            }
            assert!(Instant::now() < ende, "no socket within 20 s");
            std::thread::sleep(Duration::from_millis(20));
        }
        kind
    }
}

fn konfig_mit(rollen: &Value) -> Value {
    json!({"fassung": 1, "konten": {uid().to_string(): {"person": null}}, "rollen": rollen,
        "ziele": {"copilot": {"host": "127.0.0.1", "port": 9, "tls": false, "praefix": "", "kopf": "authorization",
            "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot", "pfade": ["GET /*"]}},
        "server": {}, "kids": {}})
}

fn standard() -> Value {
    konfig_mit(&json!({"bruecke": "wb-bruecke"}))
}

fn beenden(mut kind: Child) -> i32 {
    let pid = rustix::process::Pid::from_child(&kind);
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).expect("TERM");
    let ende = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(st) = kind.try_wait().expect("wait") {
            return st.code().unwrap_or(-1);
        }
        assert!(Instant::now() < ende, "no end within 20 s after SIGTERM");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn schritt_1_argument_ist_64() {
    let a = Aufbau::neu(&standard());
    let (code, e) = a.ende(a.befehl().arg("x"));
    assert_eq!(code, 64);
    assert_eq!(e, "halter.aufruf: falscher Aufruf\n");
}

#[test]
fn schritt_2_konfig_ist_78() {
    // L4: a local server with a token
    let mut k = standard();
    k["server"] = json!({"x": {"host": "127.0.0.1", "port": 9, "tls": false, "pfad": "/mcp", "kopf": "authorization",
        "vorsatz": "Bearer ", "schluessel": "mcp.x", "freigabe": true, "werkzeuge": {}, "lokal": true}});
    let a = Aufbau::neu(&k);
    let (code, e) = a.ende(&mut a.befehl());
    assert_eq!(
        (code, e.as_str()),
        (78, "halter.konfig: Konfiguration ungueltig\n")
    );
    assert!(!a.socket().exists(), "no child, no socket");
    // writable for the group, owned by someone else, not closed, folder writable for others
    let a = Aufbau::neu(&standard());
    modus(&a.konfig, 0o664);
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
    let a = Aufbau::neu(&standard());
    let (code, _) = a.ende(a.befehl().env("WB_HALTER_KONFIG_EIGNER", "0"));
    assert_eq!(code, 78);
    let mut k = standard();
    k["zusatz"] = json!(1);
    let a = Aufbau::neu(&k);
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
    let a = Aufbau::neu(&standard());
    modus(a.konfig.parent().expect("etc"), 0o777);
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
    // L10: the role group with a member, with a password, as the primary group of the person, missing, or no file of
    // the groups at all: each 78 with the fixed line, no socket
    for (group, passwd) in [
        (format!("wb-bruecke:x:{FREMDE_GID}:person\n"), None),
        (format!("wb-bruecke:$6$a$b:{FREMDE_GID}:\n"), None),
        (format!("wb-bruecke:x:{FREMDE_GID}:\n"), Some(FREMDE_GID)),
        ("root:x:0:\n".to_owned(), None),
    ] {
        let a = Aufbau::neu(&standard());
        std::fs::write(a.wurzel.join("group"), &group).expect("group");
        if let Some(g) = passwd {
            let p = std::fs::read_to_string(a.wurzel.join("passwd")).expect("passwd");
            std::fs::write(
                a.wurzel.join("passwd"),
                p.replace(&format!(":{HAUPTGRUPPE}:"), &format!(":{g}:")),
            )
            .expect("passwd");
        }
        let (code, e) = a.ende(&mut a.befehl());
        assert_eq!(
            (code, e.as_str()),
            (78, "halter.konfig: Konfiguration ungueltig\n"),
            "{group}"
        );
        assert!(!a.socket().exists());
    }
    let a = Aufbau::neu(&standard());
    std::fs::remove_file(a.wurzel.join("group")).expect("rm group");
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
    // the old form: role programs as a list of paths
    let a = Aufbau::neu(&konfig_mit(
        &json!({"bruecke": ["/usr/lib/werkbank/werkbank-modellweg"]}),
    ));
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
    // a link instead of the file
    let a = Aufbau::neu(&standard());
    let echt = a.wurzel.join("echt.json");
    std::fs::rename(&a.konfig, &echt).expect("rename");
    std::os::unix::fs::symlink(&echt, &a.konfig).expect("link");
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
}

#[test]
fn schritt_3_ablage_ist_78_und_legt_fehlendes_an() {
    let a = Aufbau::neu(&standard());
    // a link as level `state`
    std::fs::create_dir(a.heim.join(".local")).expect("local");
    let anders = a.wurzel.join("anders");
    std::fs::create_dir(&anders).expect("dir");
    std::os::unix::fs::symlink(&anders, a.heim.join(".local/state")).expect("link");
    let (code, e) = a.ende(&mut a.befehl());
    assert_eq!(
        (code, e.as_str()),
        (78, "halter.ablage_rechte: Ablage hat falsche Rechte\n")
    );
    assert_eq!(
        std::fs::read_dir(&anders).expect("dir").count(),
        0,
        "nothing made behind the link"
    );
    // geheim with 0750
    let a = Aufbau::neu(&standard());
    a.mit_anmeldung();
    modus(&a.geheim(), 0o750);
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
    // a level writable for the group
    let a = Aufbau::neu(&standard());
    a.mit_anmeldung();
    modus(&a.heim.join(".local/state/agent-workbench"), 0o770);
    assert_eq!(a.ende(&mut a.befehl()).0, 78);
    // missing levels are made with 0700
    let a = Aufbau::neu(&standard());
    let kind = a.laufend();
    let mut p = a.heim.clone();
    for e in halter::ablage::EBENEN {
        p = p.join(e);
        let m = std::fs::symlink_metadata(&p).expect("level");
        assert!(m.is_dir() && m.mode() & 0o7777 == 0o700, "{}", p.display());
    }
    assert_eq!(beenden(kind), 0);
}

#[test]
fn l2_belegter_endpunkt_ist_74_und_bleibt_gleich() {
    let a = Aufbau::neu(&standard());
    std::fs::create_dir(a.laufzeit.join("wb-halter")).expect("dir");
    modus(&a.laufzeit.join("wb-halter"), 0o700);
    std::fs::write(a.socket(), b"vorher\x00\x01").expect("file");
    let (code, e) = a.ende(&mut a.befehl());
    assert_eq!(
        (code, e.as_str()),
        (74, "halter.pipe_belegt: Endpunkt schon belegt\n")
    );
    assert_eq!(std::fs::read(a.socket()).expect("read"), b"vorher\x00\x01");
    // a foreign socket at the path
    let a = Aufbau::neu(&standard());
    std::fs::create_dir(a.laufzeit.join("wb-halter")).expect("dir");
    modus(&a.laufzeit.join("wb-halter"), 0o700);
    let fremd = std::os::unix::net::UnixListener::bind(a.socket()).expect("bind");
    let ino = std::fs::symlink_metadata(a.socket()).expect("stat").ino();
    assert_eq!(a.ende(&mut a.befehl()).0, 74);
    assert_eq!(
        std::fs::symlink_metadata(a.socket()).expect("stat").ino(),
        ino
    );
    drop(fremd);
    // a second holder while the first runs
    let a = Aufbau::neu(&standard());
    let kind = a.laufend();
    assert_eq!(a.ende(&mut a.befehl()).0, 74);
    assert!(a.socket().exists(), "the second holder changed nothing");
    assert_eq!(beenden(kind), 0);
}

/// F2: the rest of a holder ended hard (a socket of the own uid without a listener) does not keep the holder from
/// starting: the start removes it and serves. Counter-check: a living holder at the path stays, and the second start
/// ends with 74 (also in `l2_belegter_endpunkt_ist_74_und_bleibt_gleich`).
///
/// The dead socket is the rest of a real holder killed with SIGKILL. A listener bound and dropped in this process is
/// not dead for sure: a child that another test thread forks in that moment holds a copy of the descriptor until its
/// `exec`, and a connect in that window succeeds (measured under load: 3 of 200 runs of this binary).
#[test]
fn f2_toter_socket_wird_geraeumt_lebender_halter_bleibt() {
    let a = Aufbau::neu(&standard());
    // a holder that ends without removing its socket: SIGKILL
    let mut tot = a.laufend();
    rustix::process::kill_process(
        rustix::process::Pid::from_child(&tot),
        rustix::process::Signal::KILL,
    )
    .expect("KILL");
    let _ = tot.wait().expect("wait");
    let alt = std::fs::symlink_metadata(a.socket())
        .expect("dead socket")
        .ino();
    // Another test of this binary may fork for a holder just between `bind` and `drop`: until its `exec` the child
    // holds a copy of the listening descriptor, and a connection succeeds. The socket is dead once the refusal comes.
    let ende = Instant::now() + Duration::from_secs(5);
    while UnixStream::connect(a.socket()).is_ok() {
        assert!(Instant::now() < ende, "nobody listens at the dead socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    let kind = a.laufend_nach_neuem_socket(alt);
    let mut s = UnixStream::connect(a.socket()).expect("the new holder listens");
    s.write_all(b"{\"fassung\":1,\"id\":\"q-1\",\"name\":\"anmeldung.copilot\",\"op\":\"schluessel.vorhanden\"}\n")
        .expect("send");
    let mut b = String::new();
    s.read_to_string(&mut b).expect("read");
    assert!(b.contains("halter.peer"), "{b}");
    // counter-check: while this holder lives, a second one ends with 74 and leaves the socket
    let ino = std::fs::symlink_metadata(a.socket()).expect("stat").ino();
    assert_eq!(a.ende(&mut a.befehl()).0, 74);
    assert_eq!(
        std::fs::symlink_metadata(a.socket()).expect("stat").ino(),
        ino
    );
    assert_eq!(beenden(kind), 0);
}

#[test]
fn socket_0600_und_sigterm_raeumt_nur_den_eigenen() {
    let a = Aufbau::neu(&standard());
    let kind = a.laufend();
    let m = std::fs::symlink_metadata(a.socket()).expect("stat");
    assert_eq!(m.mode() & 0o7777, 0o600);
    assert_eq!(
        std::fs::symlink_metadata(a.laufzeit.join("wb-halter"))
            .expect("stat")
            .mode()
            & 0o7777,
        0o700
    );
    assert_eq!(beenden(kind), 0);
    assert!(!a.socket().exists(), "the own socket is removed at the end");
}

/// SIGTERM the moment the socket is there ends the holder with 0 and removes the socket: the holder takes over
/// SIGTERM before it makes the socket. Before, the socket came first, and a SIGTERM in between ended the holder by the
/// signal with the socket left (measured under load: 2 of 400 runs of `socket_0600_und_sigterm_raeumt_nur_den_eigenen`).
/// Twenty rounds, each with its own set-up, widen the window of the race; none is tried again.
#[test]
fn sigterm_gleich_nach_dem_socket_endet_mit_0_und_raeumt() {
    for runde in 0..20 {
        let a = Aufbau::neu(&standard());
        let kind = a.laufend();
        assert_eq!(beenden(kind), 0, "round {runde}");
        assert!(!a.socket().exists(), "round {runde}: the socket is removed");
    }
}

#[test]
fn l5_speicher_umgebung_und_kernabbild_gegen_dieselbe_uid() {
    let a = Aufbau::neu(&standard());
    a.mit_anmeldung();
    let kind = a.laufend();
    let pid = kind.id();
    let proc = PathBuf::from(format!("/proc/{pid}"));
    assert!(proc.exists(), "/proc/<pid> is visible");
    // with PR_SET_DUMPABLE 0 the entries belong to root
    for e in ["mem", "environ"] {
        let m = std::fs::metadata(proc.join(e)).expect("stat");
        assert_eq!(m.uid(), root_uid(), "{e}");
        assert_ne!(m.uid(), uid(), "{e}");
    }
    assert!(
        std::fs::read(proc.join("environ")).is_err(),
        "environ not readable"
    );
    let mut mem = std::fs::File::open(proc.join("mem"));
    if let Ok(f) = &mut mem {
        let mut b = [0u8; 16];
        assert!(f.read(&mut b).is_err(), "mem not readable");
    }
    assert!(
        std::fs::read_link(proc.join("exe")).is_err(),
        "exe not readable"
    );
    let status = std::fs::read_to_string(proc.join("limits")).unwrap_or_default();
    let kern = status
        .lines()
        .find(|z| z.starts_with("Max core file size"))
        .unwrap_or("");
    assert!(
        kern.split_whitespace().filter(|w| *w == "1").count() >= 2,
        "RLIMIT_CORE 1/1: {kern}"
    );
    assert_eq!(beenden(kind), 0);
}

/// A peer program (the interpreter of the system) asks `schluessel.vorhanden`, once as it is and once after
/// `prctl(PR_SET_DUMPABLE, 0)`, against a holder whose role group `wb-bruecke` has the effective gid of the peer. Measures
/// the fix of the finding of the build: a peer that shields itself keeps its role, because the role comes from the gid.
/// Counter-check: the same two peers against a holder whose role group has another gid get `halter.peer`.
#[test]
fn peer_mit_dumpable_null_bekommt_die_rolle_aus_der_gid() {
    let Ok(py) = std::fs::canonicalize("/usr/bin/python3") else {
        return; // no interpreter on this machine: nothing to measure here
    };
    let lauf = |gid: u32| {
        let a = Aufbau::neu(&standard());
        std::fs::write(
            a.wurzel.join("group"),
            format!("root:x:0:\nwb-bruecke:x:{gid}:\n"),
        )
        .expect("group");
        a.mit_anmeldung();
        let skript = a.wurzel.join("client.py");
        std::fs::write(
            &skript,
            "import ctypes, socket, sys\n\
             if sys.argv[2] == '0':\n    ctypes.CDLL(None).prctl(4, 0, 0, 0, 0)\n\
             s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)\ns.connect(sys.argv[1])\n\
             s.sendall(b'{\"fassung\":1,\"id\":\"q-1\",\"name\":\"anmeldung.copilot\",\"op\":\"schluessel.vorhanden\"}\\n')\n\
             b = b''\nwhile not b.endswith(b'\\n'):\n    t = s.recv(4096)\n    if not t: break\n    b += t\n\
             sys.stdout.write(b.decode())\n",
        )
        .expect("script");
        let kind = a.laufend();
        let frage = |dumpable: &str| {
            let aus = Command::new(&py)
                .arg("-I")
                .arg(&skript)
                .arg(a.socket())
                .arg(dumpable)
                .env_clear()
                .output()
                .expect("python");
            String::from_utf8(aus.stdout).expect("utf-8")
        };
        let normal = frage("1");
        let geschirmt = frage("0");
        assert_eq!(beenden(kind), 0);
        (normal, geschirmt)
    };
    let ja = "{\"art\":\"antwort\",\"fassung\":1,\"id\":\"q-1\",\"ok\":true,\"vorhanden\":true}\n";
    let nein = "{\"art\":\"antwort\",\"code\":\"halter.peer\",\"fassung\":1,\"grund\":\"Aufrufer nicht zugelassen\",\"id\":null,\"ok\":false}\n";
    let (normal, geschirmt) = lauf(egid());
    assert_eq!((normal.as_str(), geschirmt.as_str()), (ja, ja));
    let (normal, geschirmt) = lauf(FREMDE_GID);
    assert_eq!((normal.as_str(), geschirmt.as_str()), (nein, nein));
}

#[test]
fn ein_lauf_ohne_geheimnis_in_ausgabe_und_protokoll() {
    let a = Aufbau::neu(&standard());
    a.mit_anmeldung();
    let mut kind = a.laufend();
    // a peer without a role (the gid of this test binary is not the gid of the role group): halter.peer, then closed
    let mut s = UnixStream::connect(a.socket()).expect("connect");
    s.write_all(b"{\"fassung\":1,\"id\":\"q-1\",\"name\":\"anmeldung.copilot\",\"op\":\"schluessel.vorhanden\"}\n")
        .expect("send");
    let mut b = String::new();
    s.read_to_string(&mut b).expect("read");
    assert!(b.contains("halter.peer"));
    let pid = rustix::process::Pid::from_child(&kind);
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).expect("TERM");
    let st = kind.wait().expect("wait");
    let mut aus = String::new();
    let mut err = String::new();
    kind.stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut aus)
        .expect("read");
    kind.stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut err)
        .expect("read");
    assert_eq!(st.code(), Some(0));
    assert!(!aus.contains(MARKE) && !err.contains(MARKE) && !b.contains(MARKE));
}
