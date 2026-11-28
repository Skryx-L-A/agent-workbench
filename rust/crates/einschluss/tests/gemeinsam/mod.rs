//! Shared set-up of the outside tests (Linux, feature `attrappe`): a throw-away world with a vault, a log folder, a start
//! file, two stand-in search processes, a stand-in harness, and the helpers that run a profile and judge its answer.
//! Everything is a stand-in: no real note, key or login data is read.
#![allow(dead_code, clippy::unused_self)]

use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use einschluss::{
    Beschreibung, COPILOT_PLATZHALTER, Cloud, GAST_WEITER, Profil, System, Umgebung, Verteilung,
    Zustand,
};

pub const ATTRAPPE: &str = env!("CARGO_BIN_EXE_einschluss-attrappe");
/// The forwarder program of the cloud exit (part of the delivery).
pub const WEITER: &str = env!("CARGO_BIN_EXE_einschluss-weiter");
/// The text that stands only in the stand-in of the real key (the relay swaps the placeholder for it) and in every
/// file or variable a mutant would leak.
pub const TOKEN_MARKE: &str = "gho_MARKE-5t9k2q7w1e8r3y6u4i0o";
/// The text in the stand-in key of the throw-away certificate authority.
pub const CA_MARKE: &str = "CA-KEY-MARKE-8d2";
/// The text in the session bundle (stands for the certificate that the harness may see).
pub const BUNDLE_MARKE: &str = "BUNDLE-MARKE-4c1";
/// The text that stands only in the stand-in note of the vault.
pub const NOTIZ_MARKE: &str = "MK-7Q2-qxz";
/// The route of the session under test and of the neighbour session.
pub const ROUTE: &str = "cloud-1";
pub const NACHBAR_ROUTE: &str = "lokal-1";

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

/// A stand-in search process; ends and is waited for when dropped.
pub struct Such {
    kind: Child,
    pub pid: u32,
    pub socket: PathBuf,
}

impl Such {
    pub fn starten(socket: &Path, route: &str) -> Such {
        Such::mit(&["such", &socket.to_string_lossy(), route], socket)
    }

    /// The stand-in relay from the Unix socket `socket` to the TCP address `ziel`.
    pub fn weiter(socket: &Path, ziel: &str) -> Such {
        Such::mit(&["weiter", &socket.to_string_lossy(), ziel], socket)
    }

    /// The stand-in relay of the cloud exit outside the enclosure.
    pub fn cloud(socket: &Path, platzhalter: &str, marke: &str) -> Such {
        Such::mit(
            &["cloud", &socket.to_string_lossy(), platzhalter, marke],
            socket,
        )
    }

    fn mit(args: &[&str], socket: &Path) -> Such {
        let mut kind = Command::new(ATTRAPPE)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .spawn()
            .expect("stand-in search starts");
        let frist = Instant::now() + Duration::from_secs(5);
        while !socket.exists() {
            assert!(Instant::now() < frist, "stand-in search made no socket");
            assert!(
                kind.try_wait().expect("wait").is_none(),
                "stand-in search ended early"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let pid = kind.id();
        Such {
            kind,
            pid,
            socket: socket.to_path_buf(),
        }
    }
}

impl Drop for Such {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

/// The throw-away world.
pub struct Welt {
    pub wurzel: PathBuf,
    pub programm: PathBuf,
    pub arbeit: PathBuf,
    pub zustand: PathBuf,
    pub vault: PathBuf,
    pub notiz: PathBuf,
    pub protokoll: PathBuf,
    pub start: PathBuf,
    pub startdatei: PathBuf,
    pub heim: PathBuf,
    pub such_ordner: PathBuf,
    pub nachbar_ordner: PathBuf,
    pub such: Such,
    pub nachbar: Such,
    pub cloud: Option<CloudWelt>,
}

/// What the cloud exit adds to the throw-away world: the stand-in relay outside, a bundle file, a stand-in key of the
/// certificate authority that is never mounted, and the things a mutant would mount (a config file with a token, a
/// login folder of `gh`, a bus socket).
pub struct CloudWelt {
    pub relay: Such,
    pub bus: Such,
    pub bundle: PathBuf,
    pub ca_schluessel: PathBuf,
    pub ca_ordner: PathBuf,
    pub konfig: PathBuf,
    pub gh: PathBuf,
}

pub fn testort() -> PathBuf {
    std::env::var_os("EINSCHLUSS_TESTORT").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from)
}

/// A named pipe at `pfad` (made once by `mkfifo`, mode 600).
pub fn fifo(pfad: &Path) -> PathBuf {
    if !pfad.exists() {
        let status = Command::new("/usr/bin/mkfifo")
            .arg("-m")
            .arg("600")
            .arg(pfad)
            .status()
            .expect("mkfifo");
        assert!(status.success(), "mkfifo");
    }
    pfad.to_path_buf()
}

/// The lifeline of a starter as a named pipe (`<wurzel>/leine`): a shell puts it on descriptor `LEINE_FD`
/// (`5<"$leine"`), and it reads end of file once no writer holds it any more ([`fifo_offen`]).
pub fn leine(wurzel: &Path) -> PathBuf {
    fifo(&wurzel.join("leine"))
}

/// Opens a named pipe for reading and writing (it does not block on Linux): while the handle is open this process is a
/// writer, so a lifeline lives and a reader never sees end of file.
pub fn fifo_offen(pfad: &Path) -> fs::File {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pfad)
        .expect("named pipe")
}

fn ordner(pfad: &Path, modus: u32) -> PathBuf {
    fs::create_dir_all(pfad).expect("folder");
    // The world of the outside tests exists on Linux only; elsewhere this helper is built but never called.
    #[cfg(unix)]
    fs::set_permissions(pfad, fs::Permissions::from_mode(modus)).expect("mode");
    #[cfg(not(unix))]
    let _ = modus;
    pfad.to_path_buf()
}

impl Welt {
    pub fn neu() -> Welt {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let wurzel = testort().join(format!("ei-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&wurzel);
        ordner(&wurzel, 0o700);
        let programm = ordner(&wurzel.join("programm"), 0o755);
        kopie_ausfuehrbar(Path::new(ATTRAPPE), &programm.join("einschluss-attrappe"));
        let arbeit = ordner(&wurzel.join("arbeit"), 0o755);
        fs::write(arbeit.join("arbeit.txt"), "arbeit").expect("file");
        let zustand = ordner(&wurzel.join("zustand"), 0o700);
        let vault = ordner(&wurzel.join("vault"), 0o700);
        let notiz = vault.join("notiz.md");
        fs::write(&notiz, format!("# Kundennotiz {NOTIZ_MARKE}\n")).expect("note");
        let protokoll = ordner(&wurzel.join("protokoll"), 0o700);
        let start = ordner(&wurzel.join("start"), 0o700);
        let startdatei = start.join("start.json");
        fs::write(&startdatei, "{\"route\":\"cloud-1\"}").expect("start file");
        let such_ordner = ordner(&wurzel.join("such"), 0o700);
        let nachbar_ordner = ordner(&wurzel.join("nachbar"), 0o700);
        let such = Such::starten(&such_ordner.join("such.sock"), ROUTE);
        let nachbar = Such::starten(&nachbar_ordner.join("such.sock"), NACHBAR_ROUTE);
        Welt {
            heim: wurzel.join("heim"),
            wurzel,
            programm,
            arbeit,
            zustand,
            vault,
            notiz,
            protokoll,
            start,
            startdatei,
            such_ordner,
            nachbar_ordner,
            such,
            nachbar,
            cloud: None,
        }
    }

    /// The world with the cloud exit: the stand-in relay that swaps the placeholder for [`TOKEN_MARKE`].
    pub fn neu_cloud() -> Welt {
        let mut welt = Welt::neu();
        let relay_ordner = ordner(&welt.wurzel.join("relay"), 0o700);
        let relay = Such::cloud(
            &relay_ordner.join("relay.sock"),
            COPILOT_PLATZHALTER,
            TOKEN_MARKE,
        );
        let bus_ordner = ordner(&welt.wurzel.join("bus"), 0o700);
        let bus = Such::starten(&bus_ordner.join("bus"), TOKEN_MARKE);
        let sitzung = ordner(&welt.wurzel.join("sitzung"), 0o700);
        let bundle = sitzung.join("bundle.pem");
        fs::write(
            &bundle,
            format!("-----BEGIN CERTIFICATE-----\n{BUNDLE_MARKE}\n-----END CERTIFICATE-----\n"),
        )
        .expect("bundle");
        let ca_ordner = ordner(&welt.wurzel.join("ca"), 0o700);
        let ca_schluessel = ca_ordner.join("ca.key");
        fs::write(&ca_schluessel, CA_MARKE).expect("ca key");
        let konfig = ordner(&welt.wurzel.join("copilot-konfig"), 0o700);
        fs::write(
            konfig.join("config.json"),
            format!("{{\"token\":\"{TOKEN_MARKE}\"}}"),
        )
        .expect("config");
        let gh = ordner(&welt.wurzel.join("gh"), 0o700);
        fs::write(
            gh.join("hosts.yml"),
            format!("github.com:\n  oauth_token: {TOKEN_MARKE}\n"),
        )
        .expect("gh");
        welt.cloud = Some(CloudWelt {
            relay,
            bus,
            bundle,
            ca_schluessel,
            ca_ordner,
            konfig,
            gh,
        });
        welt
    }

    pub fn cloud_teile(&self) -> &CloudWelt {
        self.cloud.as_ref().expect("a world with the cloud exit")
    }

    /// The description of the cloud harness. The distribution is the one of the host.
    pub fn beschreibung_cloud(&self) -> Beschreibung {
        let c = self.cloud_teile();
        let mut b = self.beschreibung();
        b.harness = String::from(einschluss::HARNESS_COPILOT);
        b.cloud = Some(Cloud {
            weiterreicher: c.relay.socket.clone(),
            weiter_programm: PathBuf::from(WEITER),
            bundle: c.bundle.clone(),
            verteilung: Verteilung::vom_host().expect("a known distribution"),
        });
        b
    }

    pub fn profil_cloud(&self) -> Profil {
        Profil::bauen(&self.beschreibung_cloud()).expect("cloud profile builds")
    }

    /// The home of the session inside the enclosure (the same path as on the host; there it is an empty tmpfs).
    pub fn heim_text(&self) -> String {
        self.heim.to_string_lossy().into_owned()
    }

    /// The path of the stand-in harness inside the enclosure (the program folder is mounted at the same path).
    pub fn attrappe_gast(&self) -> String {
        self.programm
            .join("einschluss-attrappe")
            .to_string_lossy()
            .into_owned()
    }

    pub fn beschreibung(&self) -> Beschreibung {
        Beschreibung {
            harness: "attrappe".to_owned(),
            arbeitsordner: self.arbeit.clone(),
            programmordner: vec![self.programm.clone()],
            zustand: Zustand {
                host: self.zustand.clone(),
                name: ".zustand".to_owned(),
                fortsetzen: false,
                gespraech: None,
            },
            heim: self.heim.clone(),
            such_socket: self.such.socket.clone(),
            modell_socket: None,
            anfordern_socket: None,
            lokal_weiter: None,
            dateien: Vec::new(),
            cloud: None,
            paket: None,
            umgebung: Umgebung {
                home: self.heim.to_string_lossy().into_owned(),
                path: "/usr/bin".to_owned(),
                term: "xterm-256color".to_owned(),
                lang: "C.UTF-8".to_owned(),
                tmpdir: "/tmp".to_owned(),
            },
            geschuetzt: vec![
                self.vault.clone(),
                self.protokoll.clone(),
                self.start.clone(),
            ],
            argv: vec![
                self.attrappe_gast(),
                "harness".to_owned(),
                "kennung".to_owned(),
            ],
            system: System::vom_host().expect("host system"),
        }
    }

    pub fn profil(&self) -> Profil {
        Profil::bauen(&self.beschreibung()).expect("profile builds")
    }

    /// The probe `probe` of the stand-in harness, as the tail of the argument list `args`.
    pub fn mit_probe(&self, args: &[String], probe: &[&str]) -> Vec<String> {
        let mut naht = args
            .iter()
            .position(|a| a == "--")
            .expect("the list has a --");
        // with a cloud exit the program runs below the forwarder: keep it, replace only the program behind it
        if args.get(naht + 1).map(String::as_str) == Some(GAST_WEITER) {
            naht += args[naht + 1..]
                .iter()
                .position(|a| a == "--")
                .expect("the forwarder call has a --")
                + 1;
        }
        let mut neu: Vec<String> = args[..=naht].to_vec();
        neu.push(self.attrappe_gast());
        neu.push("harness".to_owned());
        neu.extend(probe.iter().map(|p| (*p).to_owned()));
        neu
    }

    /// Runs `bwrap` with `args` and the probe and gives the one answer line of the stand-in (`name: result`).
    /// The command gets the extra variables `env` (in addition to nothing: the environment is cleared first).
    pub fn probe(&self, args: &[String], probe: &[&str]) -> String {
        self.probe_env(args, probe, &[])
    }

    pub fn probe_env(&self, args: &[String], probe: &[&str], env: &[(&str, &str)]) -> String {
        let mut c = Command::new("/usr/bin/bwrap");
        c.args(self.mit_probe(args, probe))
            .env_clear()
            .envs(env.iter().copied());
        zeile(&mut c, probe)
    }

    /// The same probe without any enclosure: the control that the thing probed exists.
    pub fn probe_aussen(&self, probe: &[&str]) -> String {
        let mut c = Command::new(ATTRAPPE);
        c.arg("harness").args(probe).env_clear();
        zeile(&mut c, probe)
    }
}

fn zeile(c: &mut Command, probe: &[&str]) -> String {
    let (status, aus, fehler) = lauf(c, Duration::from_secs(30));
    let text = aus.trim().to_owned();
    assert!(
        text.starts_with(&format!("{}: ", probe[0])),
        "no answer line for {probe:?}: status {status:?}, stdout {aus:?}, stderr {fehler:?}"
    );
    text
}

/// Runs a command with a time limit; returns the exit code, standard output and standard error.
pub fn lauf(c: &mut Command, frist: Duration) -> (Option<i32>, String, String) {
    let mut kind = c
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("command starts");
    let ende = Instant::now() + frist;
    let status = loop {
        if let Some(s) = kind.try_wait().expect("wait") {
            break s.code();
        }
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (mut aus, mut fehler) = (String::new(), String::new());
    if let Some(mut s) = kind.stdout.take() {
        let _ = s.read_to_string(&mut aus);
    }
    if let Some(mut s) = kind.stderr.take() {
        let _ = s.read_to_string(&mut fehler);
    }
    (status, aus, fehler)
}

impl Drop for Welt {
    fn drop(&mut self) {
        // the stand-in searches end first (their fields drop after this), then the throw-away folder goes
        let _ = self.such.kind_beenden();
        let _ = self.nachbar.kind_beenden();
        let _ = fs::remove_dir_all(&self.wurzel);
    }
}

impl Such {
    fn kind_beenden(&mut self) -> std::io::Result<()> {
        self.kind.kill()?;
        self.kind.wait().map(|_| ())
    }
}

/// Inserts `neu` in front of the first `--` of the list (a mutant of the profile).
pub fn davor(args: &[String], neu: &[&str]) -> Vec<String> {
    let naht = args.iter().position(|a| a == "--").expect("--");
    let mut r: Vec<String> = args[..naht].to_vec();
    r.extend(neu.iter().map(|n| (*n).to_owned()));
    r.extend_from_slice(&args[naht..]);
    r
}

/// Removes every occurrence of the switch `weg` (a mutant of the profile).
pub fn ohne(args: &[String], weg: &str) -> Vec<String> {
    args.iter().filter(|a| *a != weg).cloned().collect()
}

/// Replaces the first occurrence of `alt` (a run of arguments) by `neu`.
pub fn ersetzt(args: &[String], alt: &[&str], neu: &[&str]) -> Vec<String> {
    let i = args
        .windows(alt.len())
        .position(|w| w.iter().zip(alt).all(|(a, b)| a == b))
        .expect("the run to replace is in the list");
    let mut r: Vec<String> = args[..i].to_vec();
    r.extend(neu.iter().map(|n| (*n).to_owned()));
    r.extend_from_slice(&args[i + alt.len()..]);
    r
}

/// Copies a program that a test then runs, in a process of its own (`cp`).
///
/// Written by this test process, the descriptor of the new file would sit here open for writing, and a child that another
/// test thread forks at that moment holds a copy of it until its own `exec`. An `exec` of the new file in that time fails
/// with `ETXTBSY` (measured under load: "bwrap: execvp .../programm/einschluss-attrappe: Text file busy"). `cp` has ended
/// before this function returns, and this process never opens the file for writing.
pub fn kopie_ausfuehrbar(quelle: &Path, ziel: &Path) {
    let status = Command::new("/usr/bin/cp")
        .arg(quelle)
        .arg(ziel)
        .env_clear()
        .status()
        .expect("cp");
    assert!(status.success(), "copy of {}: {status:?}", quelle.display());
}

/// Writes a script that a test then runs, in a process of its own (`sh` reads the text from a pipe), and makes it
/// executable; for the reason see [`kopie_ausfuehrbar`].
pub fn skript_schreiben(ziel: &Path, text: &str) {
    let mut kind = Command::new("/bin/sh")
        .args(["-c", "cat > \"$0\"", &ziel.to_string_lossy()])
        .env_clear()
        .stdin(Stdio::piped())
        .spawn()
        .expect("sh");
    std::io::Write::write_all(&mut kind.stdin.take().expect("stdin"), text.as_bytes())
        .expect("text of the script");
    let status = kind.wait().expect("wait");
    assert!(status.success(), "script {}: {status:?}", ziel.display());
    // As in `ordner`: the scripts run on Linux only; elsewhere this helper is built but never called.
    #[cfg(unix)]
    fs::set_permissions(ziel, fs::Permissions::from_mode(0o755)).expect("mode");
}
