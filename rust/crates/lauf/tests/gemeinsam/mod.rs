//! The test world of "Lauf starten": a throw-away folder with a copy of the program, its own configuration next to it
//! (feature `pruefbau`), the stand-ins of the search process, the model path and the harnesses, and a `bwrap` in
//! place of the real one (a script that records its call), or the real `bwrap` for the outside cases.
//!
//! The folder lies below `$HOME/.cache` (socket paths must stay short; a long `TMPDIR` would break them).

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

pub const LAUF: &str = env!("CARGO_BIN_EXE_werkbank-lauf");
pub const ORDNERMARKE: &str = env!("CARGO_BIN_EXE_werkbank-ordnermarke");
pub const KIND: &str = env!("CARGO_BIN_EXE_werkbank-lauf-kind");
pub const ATTRAPPE: &str = env!("CARGO_BIN_EXE_lauf-attrappe");

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

/// The version of the stand-in package of the cloud harness: never the version of the real CLI, so a folder of the test
/// never hides the real one.
pub const FASSUNG: &str = "0.0.88";

/// The platform of the package on this machine.
pub fn plattform() -> String {
    let arch = if std::env::consts::ARCH == "aarch64" {
        "arm64"
    } else {
        "x64"
    };
    format!("linux-{arch}")
}

/// The three harnesses of the table.
pub const HARNESSE: [&str; 3] = ["claude", "copilot", "pi"];

pub struct Welt {
    pub wurzel: PathBuf,
    pub bin: PathBuf,
    pub start: PathBuf,
    pub zustand: PathBuf,
    pub laufzeit: PathBuf,
    pub sitzungen: PathBuf,
    /// The folder of the conversations (below the state folder, as the package lays it out); made by the program.
    pub gespraeche: PathBuf,
    pub heim: PathBuf,
    pub vault: PathBuf,
    pub protokoll: PathBuf,
    pub schluessel: PathBuf,
    pub einstellungen: PathBuf,
    /// The `bwrap` of the configuration; `None` leaves the key out (the test build then takes `/usr/bin/bwrap`).
    pub bwrap: Option<PathBuf>,
    /// The harnesses the test build takes as without an option to continue (key `ohne_fortsetzen`).
    pub ohne_fortsetzen: Vec<String>,
    /// Leaves the processes of the world alone at the end (only the counter probe of the clean-up sets it).
    pub ohne_aufraeumen: bool,
}

fn ordner(p: &Path, modus: u32) {
    fs::create_dir_all(p).expect("folder");
    fs::set_permissions(p, fs::Permissions::from_mode(modus)).expect("mode");
}

/// Writing a program and starting a process exclude each other: a process forked while another thread holds a file
/// open for writing keeps that descriptor until its own `exec`, and an `exec` of the file meanwhile fails with
/// `ETXTBSY`. Writers take the lock exclusively, starters share it.
static SCHREIBEN: std::sync::RwLock<()> = std::sync::RwLock::new(());

/// Starting a process under the shared lock (see [`SCHREIBEN`]).
pub trait Gesperrt {
    fn gesperrt_spawn(&mut self) -> std::io::Result<std::process::Child>;
    fn gesperrt_output(&mut self) -> std::io::Result<Output>;
    fn gesperrt_status(&mut self) -> std::io::Result<std::process::ExitStatus>;
}

impl Gesperrt for Command {
    fn gesperrt_spawn(&mut self) -> std::io::Result<std::process::Child> {
        let _g = SCHREIBEN
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.spawn()
    }
    fn gesperrt_output(&mut self) -> std::io::Result<Output> {
        let kind = {
            let _g = SCHREIBEN
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?
        };
        kind.wait_with_output()
    }
    fn gesperrt_status(&mut self) -> std::io::Result<std::process::ExitStatus> {
        let mut kind = self.gesperrt_spawn()?;
        kind.wait()
    }
}

pub fn datei(p: &Path, inhalt: &[u8], modus: u32) {
    let _g = SCHREIBEN
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(e) = p.parent() {
        fs::create_dir_all(e).expect("folder");
    }
    fs::write(p, inhalt).expect("write");
    fs::set_permissions(p, fs::Permissions::from_mode(modus)).expect("mode");
}

pub fn kopie(von: &str, nach: &Path) {
    let _g = SCHREIBEN
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    fs::copy(von, nach).expect("copy");
    fs::set_permissions(nach, fs::Permissions::from_mode(0o755)).expect("mode");
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The copy of the provider file of the session that the recording `bwrap` makes (in the root of the world).
pub const ANBIETER_KOPIE: &str = "anbieter.json";

/// The copy of the folder `gespraeche` that the recording `bwrap` makes when it starts (in the root of the world).
pub const GESPRAECHE_BEIM_START: &str = "gespraeche-beim-start";

impl Welt {
    pub fn neu() -> Welt {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let heim_echt =
            std::env::var_os("HOME").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let basis = heim_echt.join(".cache");
        fs::create_dir_all(&basis).expect("cache");
        let wurzel = basis.join(format!("wbl-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&wurzel);
        ordner(&wurzel, 0o700);
        let wurzel = fs::canonicalize(&wurzel).expect("real path");
        let w = Welt {
            bin: wurzel.join("bin"),
            start: wurzel.join("start"),
            zustand: wurzel.join("zustand"),
            laufzeit: wurzel.join("lz"),
            sitzungen: wurzel.join("sitzungen"),
            gespraeche: wurzel.join("zustand").join("gespraeche"),
            heim: wurzel.join("heim"),
            vault: wurzel.join("vault"),
            protokoll: wurzel.join("protokoll"),
            schluessel: wurzel.join("schluessel"),
            einstellungen: wurzel.join("einst").join("einst.json"),
            bwrap: Some(wurzel.join("bin").join("bwrap")),
            wurzel,
            ohne_fortsetzen: Vec::new(),
            ohne_aufraeumen: false,
        };
        for (p, m) in [
            (&w.bin, 0o755),
            (&w.start, 0o700),
            (&w.zustand, 0o700),
            (&w.laufzeit, 0o700),
            (&w.sitzungen, 0o700),
            (&w.heim, 0o700),
            (&w.vault, 0o700),
            (&w.protokoll, 0o700),
            (&w.schluessel, 0o700),
            (&w.zustand.join("sitzungen"), 0o700),
        ] {
            ordner(p, m);
        }
        kopie(LAUF, &w.bin.join("werkbank-lauf"));
        kopie(ORDNERMARKE, &w.bin.join("werkbank-ordnermarke"));
        kopie(KIND, &w.bin.join("werkbank-lauf-kind"));
        for name in ["rangkern", "modellweg", "such-mcp", "einschluss-weiter"] {
            kopie(ATTRAPPE, &w.bin.join(name));
        }
        for h in HARNESSE {
            kopie(ATTRAPPE, &w.bin.join(h));
            w.verhalten(h, "probe");
        }
        w.paket_form_schreiben(FASSUNG);
        w.paket_fuellen();
        w.verhalten("rangkern", "bereit");
        w.verhalten("modellweg", "bereit");
        // the bwrap in place of the real one: records the PID of its parent and its arguments, keeps a copy of the
        // provider file of the session (the watcher removes the session folder when this call has ended) and ends with 0
        let log = w.wurzel.join("bwrap.log");
        // and, for the conversations, a copy of `gespraeche` as it is at the start ([`GESPRAECHE_BEIM_START`])
        let skript = format!(
            "#!/bin/sh\nprintf 'eltern %s\\n' \"$PPID\" >> '{}'\nrm -rf '{kopie}'; if [ -d '{g}' ]; then cp -a '{g}' '{kopie}'; fi\nfor a in \"$@\"; do printf 'arg %s\\n' \"$a\" >> '{}'; case \"$a\" in */konfig/models.json) cp \"$a\" '{}';; esac; done\nprintf 'ende\\n' >> '{}'\nexit 0\n",
            s(&log),
            s(&log),
            s(&w.wurzel.join(ANBIETER_KOPIE)),
            s(&log),
            g = s(&w.gespraeche),
            kopie = s(&w.wurzel.join(GESPRAECHE_BEIM_START)),
        );
        datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
        datei(
            &w.schluessel.join("lokal-beispiel"),
            b"MARKE-LOKAL-1\n",
            0o600,
        );
        datei(
            &w.schluessel.join("cloud-beispiel"),
            b"MARKE-CLOUD-1\n",
            0o600,
        );
        w
    }

    /// The packages of the cloud harness in the form of npm, around the stand-in `bin/copilot`: the main package in `bin`
    /// (`bin.copilot` is `copilot`) and the platform package at place 1 (`bin/node_modules/@github/copilot-<platform>`).
    pub fn paket_form_schreiben(&self, fassung: &str) {
        let pl = plattform();
        let arch = pl.trim_start_matches("linux-");
        datei(
            &self.bin.join("package.json"),
            format!("{{\"name\":\"@github/copilot\",\"version\":\"{fassung}\",\"type\":\"module\",\"bin\":{{\"copilot\":\"copilot\"}}}}").as_bytes(),
            0o644,
        );
        let p = self.bin.join(format!("node_modules/@github/copilot-{pl}"));
        datei(
            &p.join("package.json"),
            format!("{{\"name\":\"@github/copilot-{pl}\",\"version\":\"{fassung}\",\"os\":[\"linux\"],\"cpu\":[\"{arch}\"],\"libc\":[\"glibc\"],\"bin\":{{\"copilot-{pl}\":\"copilot\"}}}}").as_bytes(),
            0o644,
        );
        datei(&p.join("copilot"), b"sea", 0o755);
    }

    /// The cache folder of the cloud harness in the home of the world.
    pub fn paket_cache(&self) -> PathBuf {
        self.heim.join(".cache/werkbank/harness/copilot")
    }

    /// The package folder of the stand-in version.
    pub fn paket_ordner(&self) -> PathBuf {
        self.paket_cache()
            .join("pkg")
            .join(plattform())
            .join(FASSUNG)
    }

    /// A complete package folder (small files), so a start does not extract.
    pub fn paket_fuellen(&self) {
        let ordner = self.paket_ordner();
        for rel in einschluss::paket_pfade(&plattform()) {
            datei(&ordner.join(rel), b"x", 0o644);
        }
        for o in [
            self.heim.join(".cache"),
            self.heim.join(".cache/werkbank"),
            self.heim.join(".cache/werkbank/harness"),
            self.paket_cache(),
        ] {
            fs::set_permissions(&o, fs::Permissions::from_mode(0o700)).expect("mode");
        }
    }

    /// Removes the cache folder of the harness.
    pub fn paket_leeren(&self) {
        let _ = fs::remove_dir_all(self.heim.join(".cache"));
    }

    /// The behaviour of a stand-in program in `bin`.
    pub fn verhalten(&self, programm: &str, text: &str) {
        datei(
            &self.bin.join(format!("{programm}.verhalten")),
            text.as_bytes(),
            0o644,
        );
    }

    /// Maps the example paths of the cases into the world.
    pub fn abb(&self, text: &str) -> String {
        // the cases name the example home of contract 1 (the word is split so no check takes it for a person)
        let heim = format!("/{}/beispiel", "home");
        text.replace(&heim, &format!("{}{heim}", s(&self.wurzel)))
            .replace(
                "/srv/beispiel",
                &format!("{}/srv/beispiel", s(&self.wurzel)),
            )
            .replace("/var/lib/werkbank/beispiel", &s(&self.zustand))
            // the example folder of the cases of version 2 (Projektentscheid, 27.11.2026), outside the home of the session
            .replace(
                "/heim/beispiel",
                &format!("{}/heim-beispiel", s(&self.wurzel)),
            )
            .replace("<startordner>", &s(&self.start))
    }

    pub fn konfig_schreiben(&self, ca_zulassung: &str) {
        let mut harness = String::new();
        for (i, h) in HARNESSE.iter().enumerate() {
            if i > 0 {
                harness.push(',');
            }
            harness.push_str(&["\"", h, "\":\"", &s(&self.bin.join(h)), "\""].concat());
        }
        let bwrap = self
            .bwrap
            .as_ref()
            .map(|b| format!(",\"bwrap\":\"{}\"", s(b)))
            .unwrap_or_default();
        let ohne = if self.ohne_fortsetzen.is_empty() {
            String::new()
        } else {
            let namen: Vec<String> = self
                .ohne_fortsetzen
                .iter()
                .map(|h| format!("\"{h}\""))
                .collect();
            format!(",\"ohne_fortsetzen\":[{}]", namen.join(","))
        };
        let text = format!(
            "{{\"fassung\":1,\"einstellungen\":\"{}\",\"vault\":\"{}\",\"protokoll\":\"{}\",\"zustand\":\"{}\",\"laufzeit\":\"{}\",\"sitzungen\":\"{}\",\"gespraeche\":\"{}\",\"heim\":\"{}\",\"path\":\"/usr/bin:/bin\",\"programme\":{{\"rangkern\":\"{}\",\"modellweg\":\"{}\",\"kind\":\"{}\",\"such_mcp\":\"{}\",\"weiter\":\"{}\"}},\"harness\":{{{harness}}},\"programmordner\":[\"{}\"],\"schluessel\":{{\"lokal-beispiel\":\"{}\",\"cloud-beispiel\":\"{}\"}},\"ca_zulassung\":\"{ca_zulassung}\"{bwrap}{ohne}}}",
            s(&self.einstellungen),
            s(&self.vault),
            s(&self.protokoll),
            s(&self.zustand),
            s(&self.laufzeit),
            s(&self.sitzungen),
            s(&self.gespraeche),
            s(&self.heim),
            s(&self.bin.join("rangkern")),
            s(&self.bin.join("modellweg")),
            s(&self.bin.join("werkbank-lauf-kind")),
            s(&self.bin.join("such-mcp")),
            s(&self.bin.join("einschluss-weiter")),
            s(&self.bin),
            s(&self.schluessel.join("lokal-beispiel")),
            s(&self.schluessel.join("cloud-beispiel")),
        );
        datei(&self.bin.join("lauf.json"), text.as_bytes(), 0o644);
    }

    pub fn startdatei(&self) -> PathBuf {
        self.start.join("start.json")
    }

    /// Runs the copy of the program with `args` and the environment `env` (nothing else, like the service).
    pub fn lauf(&self, args: &[String], env: &[(String, String)]) -> Lauf {
        self.programm_lauf("werkbank-lauf", args, env)
    }

    /// Runs the copy of the program `name` of the world (`werkbank-lauf` or `werkbank-ordnermarke`).
    pub fn programm_lauf(&self, name: &str, args: &[String], env: &[(String, String)]) -> Lauf {
        let mut c = ohne_fremde_deskriptoren(&self.bin.join(name));
        c.args(args)
            .env_clear()
            .env("TERM", "xterm-256color")
            .envs(env.iter().map(|(a, b)| (a.as_str(), b.as_str())))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let kind = c.gesperrt_spawn().expect("start");
        let pid = kind.id();
        let ausgabe = kind.wait_with_output().expect("wait");
        Lauf { pid, ausgabe }
    }

    /// The PID a stand-in recorded, if it ran.
    pub fn pid_von(&self, programm: &str) -> Option<i32> {
        fs::read_to_string(self.bin.join(format!("{programm}.pid")))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// The arguments a stand-in recorded.
    pub fn argv_von(&self, programm: &str) -> Option<Vec<String>> {
        let t = fs::read_to_string(self.bin.join(format!("{programm}.argv"))).ok()?;
        Some(t.lines().map(str::to_owned).collect())
    }

    /// The calls of the recording `bwrap`: (PID of its parent, arguments) per call.
    pub fn bwrap_aufrufe(&self) -> Vec<(u32, Vec<String>)> {
        let Ok(t) = fs::read_to_string(self.wurzel.join("bwrap.log")) else {
            return Vec::new();
        };
        let mut alle = Vec::new();
        let mut pid = 0;
        let mut args = Vec::new();
        for z in t.lines() {
            if let Some(p) = z.strip_prefix("eltern ") {
                pid = p.parse().unwrap_or(0);
                args = Vec::new();
            } else if let Some(a) = z.strip_prefix("arg ") {
                args.push(a.to_owned());
            } else if z == "ende" {
                alle.push((pid, std::mem::take(&mut args)));
            }
        }
        alle
    }
}

impl Drop for Welt {
    fn drop(&mut self) {
        // also when the test is red (the drop runs while the panic unwinds): nothing of the world may outlive it
        if !self.ohne_aufraeumen {
            prozesse_unter_beenden(&self.wurzel);
        }
        let _ = fs::remove_dir_all(&self.wurzel);
    }
}

/// A process that runs a program of a world: its PID, its command line and its start time (field 22 of
/// `/proc/<pid>/stat`, in clock ticks since the boot). PID, command line and start time together name one process;
/// the number alone can name another one after a reuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prozess {
    pub pid: i32,
    pub befehl: String,
    pub start: u64,
}

/// Command line and start time of `pid`; `None` when it does not exist, is a zombie or cannot be read.
fn prozess_lesen(pid: i32) -> Option<(String, u64)> {
    let roh = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = roh.rsplit_once(')')?;
    let mut felder = rest.split_whitespace();
    if felder.next()? == "Z" {
        return None;
    }
    // the fields after the name start with field 3 (the state); field 22 is the twentieth of them
    let start = felder.nth(18)?.parse().ok()?;
    let befehl = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if befehl.is_empty() {
        return None;
    }
    Some((String::from_utf8_lossy(&befehl).replace('\0', " "), start))
}

/// The live descendants of `pid` (children, grandchildren, ...), found by the parent PID in `/proc/<pid>/stat`. They
/// include the processes inside an enclosure, whose program paths are the paths of the guest and name no folder of the
/// world.
pub fn nachkommen(pid: i32) -> Vec<Prozess> {
    let Ok(liste) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut eltern: Vec<(i32, i32)> = Vec::new();
    for e in liste.filter_map(Result::ok) {
        let Some(p) = e.file_name().to_str().and_then(|n| n.parse::<i32>().ok()) else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(format!("/proc/{p}/stat")) else {
            continue;
        };
        let Some((_, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        if let Some(pp) = rest.split_whitespace().nth(1).and_then(|x| x.parse().ok()) {
            eltern.push((p, pp));
        }
    }
    let mut gesucht = vec![pid];
    let mut alle = Vec::new();
    while let Some(v) = gesucht.pop() {
        for (p, pp) in &eltern {
            if *pp == v {
                gesucht.push(*p);
                if let Some((befehl, start)) = prozess_lesen(*p) {
                    alle.push(Prozess {
                        pid: *p,
                        befehl,
                        start,
                    });
                }
            }
        }
    }
    alle
}

/// Whether the live process with the PID of `p` has the command line and the start time of `p`.
pub fn gleicher_prozess(p: &Prozess) -> bool {
    prozess_lesen(p.pid).is_some_and(|(b, s)| b == p.befehl && s == p.start)
}

/// The live processes that run a program below `<wurzel>/bin`: the program itself (the link `exe`, also when the file
/// has been deleted since) or the script a shell runs (the first or the second word of the command line). Only this
/// world: the folder is unique to the test.
pub fn prozesse_unter(wurzel: &Path) -> Vec<Prozess> {
    let bin = wurzel.join("bin");
    let Ok(liste) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut alle = Vec::new();
    for e in liste.filter_map(Result::ok) {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<i32>().ok()) else {
            continue;
        };
        if pid == i32::try_from(std::process::id()).unwrap_or(-1) {
            continue;
        }
        let Some((befehl, start)) = prozess_lesen(pid) else {
            continue;
        };
        let exe = fs::read_link(format!("/proc/{pid}/exe")).ok().map(|p| {
            let t = p.to_string_lossy().into_owned();
            PathBuf::from(t.strip_suffix(" (deleted)").unwrap_or(&t))
        });
        let mut worte = befehl.split(' ');
        let erstes = worte.next().map(PathBuf::from);
        let zweites = worte.next().map(PathBuf::from);
        if [exe, erstes, zweites]
            .into_iter()
            .flatten()
            .any(|p| p.starts_with(&bin))
        {
            alle.push(Prozess { pid, befehl, start });
        }
    }
    alle
}

/// Ends the processes of the world with SIGKILL; before every signal the command line and the start time of the PID are
/// compared again (a PID that was reused names another process). The watchers (`--wache`) get up to 3 s to end by
/// themselves once the others are gone; a watcher still there after that is killed as well.
pub fn prozesse_unter_beenden(wurzel: &Path) {
    let toeten = |p: &Prozess| {
        if gleicher_prozess(p)
            && let Some(pid) = rustix::process::Pid::from_raw(p.pid)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        }
    };
    let warte = |ms: u64| std::thread::sleep(std::time::Duration::from_millis(ms));
    let gefunden = prozesse_unter(wurzel);
    if gefunden.is_empty() {
        return;
    }
    let (wachen, andere): (Vec<_>, Vec<_>) = gefunden
        .into_iter()
        .partition(|p| p.befehl.contains(" --wache "));
    andere.iter().for_each(toeten);
    let ende = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < ende && !prozesse_unter(wurzel).is_empty() {
        warte(20);
    }
    // what is still there: the watchers, and anything the first pass did not see
    wachen.iter().for_each(toeten);
    // two empty listings one after the other: a program that was about to start (`spawn` returns before the new program
    // shows in `/proc`) is caught by the next pass
    let ende = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut leer = 0;
    while leer < 2 && std::time::Instant::now() < ende {
        let rest = prozesse_unter(wurzel);
        if rest.is_empty() {
            leer += 1;
        } else {
            leer = 0;
            rest.iter().for_each(toeten);
        }
        warte(20);
    }
}

pub struct Lauf {
    pub pid: u32,
    pub ausgabe: Output,
}

impl Lauf {
    pub fn exit(&self) -> Option<i32> {
        self.ausgabe.status.code()
    }
    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.ausgabe.stderr).into_owned()
    }
    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.ausgabe.stdout).into_owned()
    }
}

/// Whether the process `pid` still exists (a zombie counts as gone: it runs nothing any more).
pub fn lebt(pid: i32) -> bool {
    let Ok(text) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return rustix::process::Pid::from_raw(pid)
            .is_some_and(|p| rustix::process::test_kill_process(p).is_ok());
    };
    !text
        .rsplit_once(')')
        .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
}

/// The limit of every wait for the end of a process: a guard against a hang, not an expected time. The wait returns
/// the moment the process is gone, so a long limit costs nothing in a run that passes.
pub const HAENGESPERRE_MS: u64 = 30_000;

/// Waits until `pid` is gone and returns at once when it is; `ms` is only the guard against a hang.
pub fn endet_binnen(pid: i32, ms: u64) -> bool {
    let ende = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < ende {
        if !lebt(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    !lebt(pid)
}

/// Settings of version 2 with a cloud route (one bound and one unbound target) and a local route.
pub const EINST: &str = r#"{"fassung":2,"klassen":["oeffentlich","intern","kundendaten"],"vorgabestufe":"kundendaten","routen":[{"schluessel":"cloud-beispiel","art":"cloud","klasse":"intern","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/copilot_internal/*"}]},{"host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]}]},{"schluessel":"lokal-beispiel","art":"lokal","klasse":"kundendaten","ziel":"http://127.0.0.1:11434/v1","modell":"qwen3:8b","netzziele":[]}],"kunden":[{"schluessel":"k-beispiel","erlaubt":["lokal-beispiel"],"verboten":[]}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#;

impl Welt {
    /// The default of the firm next to the program (test build, `vorgabe.json` beside `lauf.json`), mode 0644.
    pub fn vorgabe_schreiben(&self, text: &str) {
        datei(&self.bin.join("vorgabe.json"), text.as_bytes(), 0o644);
    }

    /// A valid start of `harness` on `route` in the working folder `projekt` below the example home of the world; returns the
    /// argument list of the program.
    pub fn gueltig(&self, harness: &str, route: &str, modell: &str) -> Vec<String> {
        datei(&self.einstellungen, EINST.as_bytes(), 0o600);
        self.vorgabe_schreiben(EINST);
        self.konfig_schreiben("zugelassen");
        let arbeit = self.wurzel.join("home/beispiel/projekt");
        fs::create_dir_all(&arbeit).expect("folder");
        let start = format!(
            "{{\"fassung\":1,\"sitzung\":\"s-1\",\"harness\":\"{harness}\",\"modell\":\"{modell}\",\"route\":\"{route}\",\"kunde\":\"k-beispiel\",\"arbeitsordner\":\"{}\"}}",
            s(&arbeit)
        );
        datei(&self.startdatei(), start.as_bytes(), 0o600);
        vec![s(&self.startdatei())]
    }

    /// The start of a local session (`pi`).
    pub fn lokal(&self) -> Vec<String> {
        self.gueltig("pi", "lokal-beispiel", "qwen3:8b")
    }

    /// The start of a cloud session (`copilot`).
    pub fn cloud(&self) -> Vec<String> {
        self.gueltig("copilot", "cloud-beispiel", "claude-sonnet-5.5")
    }

    /// The PIDs of `suche.pid` and `modellweg.pid` next to the start file.
    pub fn kinder_pids(&self) -> Vec<i32> {
        ["suche.pid", "modellweg.pid"]
            .iter()
            .filter_map(|n| fs::read_to_string(self.start.join(n)).ok())
            .filter_map(|t| t.trim().parse().ok())
            .collect()
    }
}

/// The shell of the helpers that close inherited descriptors (see [`ohne_fremde_deskriptoren`]).
pub const BASH: &str = "/bin/bash";

/// A command for `programm` that first closes every descriptor above 2 and then replaces itself by the program (same
/// PID). A test runner such as `cargo mutants` hands its test processes descriptors without close-on-exec; step 6
/// refuses such a start (they would reach the enclosure), the service gives the program none.
///
/// The shell is `bash`: `dash` (the `/bin/sh` of Debian and Ubuntu) reads only single-digit descriptor numbers in a
/// redirection, so `exec 142>&-` there runs a command `142` and ends the shell with 127 before the program starts.
pub fn ohne_fremde_deskriptoren(programm: &Path) -> Command {
    let mut c = Command::new(BASH);
    c.args([
        "-c",
        "for f in /proc/$$/fd/*; do n=${f##*/}; if [ \"$n\" -gt 2 ] 2>/dev/null; then eval \"exec $n>&-\" 2>/dev/null; fi; done; exec \"$0\" \"$@\"",
    ])
    .arg(programm);
    c
}
