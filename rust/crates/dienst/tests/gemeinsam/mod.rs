//! Helpers of the integration tests: a throw-away folder, a start file, the program, and a small viewer that speaks the frames.
//! Nothing here depends on the time of day; every wait has a deadline.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rasterauszug::{Empfang, zeile_schreiben};
use serde_json::{Value, json};

/// The built program.
pub const DIENST: &str = env!("CARGO_BIN_EXE_wb-dienst");

const ART_EREIGNIS: u8 = 0x01;
const ART_UHR: u8 = 0x10;
const ART_STUMM: u8 = 0x11;
const ART_ZUSTAND: u8 = 0x12;
const ART_KERNFEHLER: u8 = 0x13;

/// A throw-away folder below `/tmp` (mode 0700), removed at the end. The socket folder `o` inside has mode 0700 too.
pub struct Wegwerf(pub PathBuf);

impl Wegwerf {
    pub fn neu(name: &str) -> Self {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let pfad = Path::new("/tmp").join(format!(
            "dt-{}-{}-{name}",
            std::process::id(),
            ZAEHLER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&pfad);
        std::fs::create_dir_all(&pfad).expect("throw-away folder");
        std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o700)).expect("mode");
        let o = pfad.join("o");
        std::fs::create_dir(&o).expect("socket folder");
        std::fs::set_permissions(&o, std::fs::Permissions::from_mode(0o700)).expect("mode");
        Self(pfad)
    }

    /// The socket folder.
    pub fn ordner(&self) -> PathBuf {
        self.0.join("o")
    }

    /// The socket path.
    pub fn socket(&self) -> PathBuf {
        self.ordner().join("dienst.sock")
    }
}

impl Drop for Wegwerf {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A valid start file; `argv` is the child, `zusatz` adds or replaces keys on the top level.
pub fn startdatei(w: &Wegwerf, argv: &[&str], zusatz: &Value) -> Value {
    let mut start = json!({
        "socket": {"ordner": w.ordner().to_string_lossy()},
        "betrachter": {"person": "anna", "raum": "raum-nord", "ansicht": "fenster"},
        "kind": {"argv": argv, "env": {"TERM": "xterm-256color", "LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin"}},
        "groesse": [40, 10],
        "deflate": false,
        "pruefmodus": true,
        "anfang": {
            "personen": {"anna": {"art": "mensch", "rolle": "nutzer"}},
            "sitzungen": [{"id": "s-alpha", "kundenraum": "raum-nord", "person": "anna", "route": "r1", "titel": "Alpha"}]
        }
    });
    if let Some(o) = zusatz.as_object() {
        for (k, v) in o {
            start[k] = v.clone();
        }
    }
    start
}

/// Writes the start file into the folder and returns its path.
pub fn datei_schreiben(w: &Wegwerf, start: &Value) -> PathBuf {
    let pfad = w.0.join("start.json");
    std::fs::write(&pfad, start.to_string()).expect("write start file");
    pfad
}

/// The program, running, with its standard error in a file.
pub struct Prozess {
    pub kind: Child,
    pub stderr: PathBuf,
}

/// The command prefix under which a check of the sockets of the program starts it (Kernabbild, step 2a). A program that shields its memory from
/// core images (`PR_SET_DUMPABLE 0`) is invisible to `lsof` of the same user under Linux (measured: 0 lines, `/proc/<pid>/fd` denied). So it
/// runs in its own user namespace without capabilities (`unshare -Ur`, then `setpriv` drops them): the observer outside, the same user, owns
/// that namespace and sees it. `unshare` and `setpriv` exec, so the pid stays that of the program. Other systems have no `PR_SET_DUMPABLE`:
/// no prefix.
pub fn beobachtbar() -> Vec<String> {
    if cfg!(target_os = "linux") {
        [
            "unshare",
            "-Ur",
            "--",
            "setpriv",
            "--bounding-set=-all",
            "--inh-caps=-all",
            "--",
        ]
        .map(str::to_owned)
        .to_vec()
    } else {
        Vec::new()
    }
}

/// What `lsof` shows of `pid` with `art` (`-U` or `-i`), without the head line.
fn lsof(pid: u32, art: &str) -> Vec<String> {
    let aus = Command::new("lsof")
        .args(["-nP", "-a", "-p", &pid.to_string(), art])
        .output()
        .expect("lsof runs (it is needed for this test)");
    String::from_utf8_lossy(&aus.stdout)
        .lines()
        .skip(1)
        .filter(|z| !z.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// The check of the network sockets of the program `pid` whose socket is `socket`. First the positive control: `lsof` must show the Unix socket of
/// the program (folder name and file name in the same line); a program that `lsof` does not see shows 0 lines for everything, so a count of
/// 0 network sockets would mean nothing, and the check is red. Then no network socket (`-i`) may exist.
pub fn netz_sockets_pruefen(pid: u32, socket: &Path) -> Result<(), String> {
    let name = socket
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let ordner = socket
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let unix = lsof(pid, "-U");
    if !unix.iter().any(|z| z.contains(ordner) && z.contains(name)) {
        return Err(format!(
            "positive control: lsof does not show the Unix socket of the program ({} lines with -U); it would be invisible for the check of the network sockets",
            unix.len()
        ));
    }
    let netz = lsof(pid, "-i");
    if netz.is_empty() {
        Ok(())
    } else {
        Err(format!("network sockets: {}", netz.join(" | ")))
    }
}

impl Prozess {
    pub fn starten(w: &Wegwerf, start: &Value) -> Self {
        Self::starten_mit(w, start, &[DIENST.to_owned()])
    }

    /// Starts the program so that `lsof` of the same user sees it (`beobachtbar`).
    pub fn starten_beobachtbar(w: &Wegwerf, start: &Value) -> Self {
        let mut befehl = beobachtbar();
        befehl.push(DIENST.to_owned());
        Self::starten_mit(w, start, &befehl)
    }

    /// Starts `befehl` (the program with its prefix) with the start file as the last argument.
    pub fn starten_mit(w: &Wegwerf, start: &Value, befehl: &[String]) -> Self {
        let datei = datei_schreiben(w, start);
        let stderr = w.0.join("stderr.txt");
        let kind = Command::new(&befehl[0])
            .args(&befehl[1..])
            .arg(&datei)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&stderr).expect("stderr file"))
            .spawn()
            .expect("wb-dienst starts");
        let p = Self { kind, stderr };
        let frist = Instant::now() + Duration::from_secs(10);
        while !w.socket().exists() {
            assert!(Instant::now() < frist, "the socket did not appear");
            std::thread::sleep(Duration::from_millis(10));
        }
        p
    }

    /// The lines on standard error so far.
    pub fn stderr_zeilen(&self) -> Vec<String> {
        std::fs::read_to_string(&self.stderr)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Waits for the end of the program, at most `frist`; returns its exit code (`None`: signal or still running).
    pub fn warte_ende(&mut self, frist: Duration) -> Option<i32> {
        let ende = Instant::now() + frist;
        loop {
            if let Some(status) = self.kind.try_wait().expect("try_wait") {
                return status.code();
            }
            if Instant::now() >= ende {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Sends a signal by name (`TERM`, `INT`, ...) with the `kill` program.
    pub fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .arg(format!("-{name}"))
            .arg(self.kind.id().to_string())
            .status()
            .expect("kill runs");
        assert!(status.success());
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

/// `true` while a process with this id exists (`kill -0`).
pub fn lebt(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// A viewer: speaks and reads frames; what it saw is kept in the fields.
pub struct Probe {
    strom: UnixStream,
    puffer: Vec<u8>,
    empfang: Empfang,
    pub nachrichten: Vec<Value>,
    pub ablehnungen: Vec<String>,
    pub fehler: Vec<String>,
    pub zeilen: Vec<String>,
    pub zustaende: Vec<Value>,
    pub quittungen: usize,
    pub rahmen: usize,
    /// Raster frames the receiver refused (RA10), e.g. a change without a full picture before it.
    pub rahmenfehler: usize,
    /// The raster number of every raster frame the receiver took, in order.
    pub rahmennummern: Vec<u8>,
    pub ende: bool,
    /// Why the connection ended (`EOF` or the read error), empty while it is open.
    pub ende_grund: String,
}

impl Probe {
    /// Connects (tries for up to 5 s) and sets a short read timeout.
    pub fn neu(socket: &Path, deflate: bool) -> Self {
        let frist = Instant::now() + Duration::from_secs(5);
        let strom = loop {
            match UnixStream::connect(socket) {
                Ok(s) => break s,
                Err(e) => {
                    assert!(Instant::now() < frist, "connect failed: {e}");
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        };
        strom
            .set_read_timeout(Some(Duration::from_millis(20)))
            .expect("timeout");
        strom
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        Self {
            strom,
            puffer: Vec::new(),
            empfang: Empfang::neu(deflate),
            nachrichten: Vec::new(),
            ablehnungen: Vec::new(),
            fehler: Vec::new(),
            zeilen: Vec::new(),
            zustaende: Vec::new(),
            quittungen: 0,
            rahmen: 0,
            rahmenfehler: 0,
            rahmennummern: Vec::new(),
            ende: false,
            ende_grund: String::new(),
        }
    }

    pub fn roh_senden(&mut self, bytes: &[u8]) {
        self.strom.write_all(bytes).expect("write to the socket");
    }

    pub fn rahmen_senden(&mut self, art: u8, nutzlast: &[u8]) {
        let n = u32::try_from(nutzlast.len() + 1).expect("size");
        let mut r = n.to_be_bytes().to_vec();
        r.push(art);
        r.extend_from_slice(nutzlast);
        self.roh_senden(&r);
    }

    pub fn ereignis(&mut self, e: &Value) {
        self.rahmen_senden(ART_EREIGNIS, e.to_string().as_bytes());
    }

    pub fn abo(&mut self, spalten: u32, zeilen: u32) {
        self.ereignis(&json!({"art": "Abo", "sitzung": "s-alpha", "an": true, "spalten": spalten, "zeilen": zeilen}));
    }

    pub fn text(&mut self, text: &str) {
        self.ereignis(&json!({"art": "Text", "sitzung": "s-alpha", "text": text}));
    }

    pub fn uhr(&mut self, t: u64) {
        self.rahmen_senden(ART_UHR, json!({"t": t}).to_string().as_bytes());
    }

    pub fn stumm(&mut self, an: bool) {
        self.rahmen_senden(ART_STUMM, json!({"an": an}).to_string().as_bytes());
    }

    pub fn zustand_fragen(&mut self) {
        self.rahmen_senden(ART_ZUSTAND, b"");
    }

    pub fn kernfehler(&mut self) {
        self.rahmen_senden(ART_KERNFEHLER, b"");
    }

    /// Reads for up to `dauer` and handles every whole frame.
    pub fn lesen(&mut self, dauer: Duration) {
        let ende = Instant::now() + dauer;
        let mut stueck = vec![0_u8; 65536];
        while Instant::now() < ende && !self.ende {
            match self.strom.read(&mut stueck) {
                Ok(0) => {
                    self.ende = true;
                    "EOF".clone_into(&mut self.ende_grund);
                }
                Ok(n) => {
                    self.puffer.extend_from_slice(&stueck[..n]);
                    self.verarbeiten();
                }
                // Linux: a read with a timeout (`SO_RCVTIMEO`) ends with EINTR when the process was stopped and continued, also
                // without a signal handler (`man 7 signal`); the connection is still open
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => {
                    self.ende = true;
                    self.ende_grund = format!("{:?}: {e}", e.kind());
                }
            }
        }
    }

    fn verarbeiten(&mut self) {
        while self.puffer.len() >= 5 {
            let n = u32::from_be_bytes([
                self.puffer[0],
                self.puffer[1],
                self.puffer[2],
                self.puffer[3],
            ]) as usize;
            if self.puffer.len() < 4 + n {
                return;
            }
            let art = self.puffer[4];
            let nutzlast: Vec<u8> = self.puffer[5..4 + n].to_vec();
            self.puffer.drain(..4 + n);
            self.rahmen += 1;
            match art {
                0x05 => match self.empfang.nimm(&nutzlast) {
                    Ok((auszug, abbild)) => {
                        self.rahmennummern.push(auszug.nr);
                        self.zeilen = abbild.lines.iter().map(|z| zeile_schreiben(z)).collect();
                    }
                    Err(_) => self.rahmenfehler += 1,
                },
                0x11 => self.quittungen += 1,
                0x06 | 0x07 | 0x08 | 0x12 => {
                    let wert: Value = serde_json::from_slice(&nutzlast).unwrap_or(Value::Null);
                    match art {
                        0x06 => self.nachrichten.push(wert),
                        0x07 => self
                            .ablehnungen
                            .push(wert["grund"].as_str().unwrap_or("").to_owned()),
                        0x08 => self
                            .fehler
                            .push(wert["fehler"].as_str().unwrap_or("").to_owned()),
                        _ => self.zustaende.push(wert),
                    }
                }
                _ => {}
            }
        }
    }

    /// Reads until `bis` holds, at most `frist`.
    pub fn warte(&mut self, frist: Duration, bis: impl Fn(&Probe) -> bool) -> bool {
        let ende = Instant::now() + frist;
        loop {
            if bis(self) {
                return true;
            }
            if Instant::now() >= ende || self.ende {
                return bis(self);
            }
            self.lesen(Duration::from_millis(20));
        }
    }

    /// Waits until a line of the picture contains `text`.
    pub fn warte_text(&mut self, text: &str) -> bool {
        self.warte(Duration::from_secs(10), |p| {
            p.zeilen.iter().any(|z| z.contains(text))
        })
    }

    /// Like [`Probe::warte_text`] in test mode: the clock only moves by the frame `Uhr`, so one is sent in every turn (`t` rises by 100).
    pub fn warte_text_takt(&mut self, text: &str, t: &mut u64) -> bool {
        let ende = Instant::now() + Duration::from_secs(10);
        while Instant::now() < ende && !self.ende {
            if self.zeilen.iter().any(|z| z.contains(text)) {
                return true;
            }
            *t += 100;
            self.uhr(*t);
            self.lesen(Duration::from_millis(20));
        }
        self.zeilen.iter().any(|z| z.contains(text))
    }

    /// Asks for the state and waits for the answer (a barrier: everything sent to this viewer before is read).
    pub fn zustand(&mut self) -> Value {
        let vorher = self.zustaende.len();
        self.zustand_fragen();
        assert!(
            self.warte(Duration::from_secs(10), |p| p.zustaende.len() > vorher),
            "no answer to Zustand (end of the connection: {:?}, errors: {:?})",
            self.ende_grund,
            self.fehler
        );
        self.zustaende[vorher].clone()
    }

    /// The names (`art`) of the messages so far.
    pub fn arten(&self) -> Vec<String> {
        self.nachrichten
            .iter()
            .map(|m| m["art"].as_str().unwrap_or("").to_owned())
            .collect()
    }
}
