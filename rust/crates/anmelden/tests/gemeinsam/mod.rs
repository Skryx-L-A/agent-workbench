//! Shared set-up of the tests: the stand-in of the Copilot CLI, the throw-away folders and the run of the program.
//! The stand-in prints the measured lines of the device flow and writes `config.json` into its `COPILOT_HOME`
//! (measurement point M1 of contract 9). No real CLI, no network, no real home.
#![allow(dead_code, missing_docs)]

use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

pub const BIN: &str = env!("CARGO_BIN_EXE_werkbank-anmelden");
pub const GERAETE_ZEILEN: &str = "To authenticate, visit https://github.com/login/device and enter code ABCD-1234\nWaiting for authorization...\n";

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);

pub fn fall_ordner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/anmelden")
}

pub fn json_lesen(pfad: &Path) -> Value {
    let text = fs::read_to_string(pfad).expect("file is readable");
    serde_json::from_str(&text).expect("file is JSON")
}

/// Token text of a case: the prefix and the filler, filled up to the length. Never a literal in a file.
pub fn token_text(t: &Value) -> String {
    let praefix = t["praefix"].as_str().expect("prefix");
    let fuell = t["fuell"].as_str().expect("filler");
    let laenge = usize::try_from(t["laenge"].as_u64().expect("length")).expect("fits");
    let rest = laenge - praefix.len();
    let mut text = String::from(praefix);
    text.extend(fuell.chars().cycle().take(rest));
    text
}

/// One run: all folders below one temp folder, removed (with permissions restored) at the end.
pub struct Aufbau {
    pub wurzel: PathBuf,
    /// The home that the test `passwd` names; the store of the holder lies below it.
    pub heim: PathBuf,
    /// The test `passwd` (seam `WB_ANMELDEN_PASSWD`): one entry of the own user id with `heim`.
    pub passwd: PathBuf,
    /// The store of the holder, `<heim>/.local/state/agent-workbench/halter/geheim`.
    pub ziel: PathBuf,
    /// A folder of the user outside the home (target of a link level).
    pub aussen: PathBuf,
    /// The key file of the former target `/var/lib/werkbank/<user>/copilot.schluessel`, below the test folder.
    pub alt: PathBuf,
    pub echt: PathBuf,
    pub bin: PathBuf,
    pub gh: PathBuf,
    pub wegwerf: PathBuf,
    pub log: PathBuf,
    pub pidfile: PathBuf,
    pub skript: PathBuf,
    /// A running helper process (pid of a living left-over); killed at the end.
    pub hilfskind: Option<Child>,
    /// The folder a link left-over points to, and the probe file in it.
    pub opfer: PathBuf,
}

impl Drop for Aufbau {
    fn drop(&mut self) {
        if let Some(k) = self.hilfskind.as_mut() {
            let _ = k.kill();
            let _ = k.wait();
        }
        for ebene in ablage_ebenen(&self.heim) {
            let _ = fs::set_permissions(ebene, fs::Permissions::from_mode(0o700));
        }
        if let Ok(liste) = fs::read_dir(&self.wegwerf) {
            for e in liste.flatten() {
                if let Ok(unter) = fs::read_dir(e.path()) {
                    for u in unter.flatten() {
                        let _ = fs::set_permissions(u.path(), fs::Permissions::from_mode(0o700));
                    }
                }
            }
        }
        let _ = fs::remove_dir_all(&self.wurzel);
    }
}

pub fn aufbau() -> Aufbau {
    let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
    let wurzel = std::env::temp_dir().join(format!("anmelden-test-{}-{n}", std::process::id()));
    let heim = wurzel.join("heim");
    let a = Aufbau {
        ziel: ablage_ebenen(&heim).pop().expect("levels"),
        passwd: wurzel.join("passwd"),
        aussen: wurzel.join("aussen"),
        alt: wurzel.join("var/lib/werkbank/nutzer/copilot.schluessel"),
        heim,
        echt: wurzel.join("echt/.copilot"),
        bin: wurzel.join("bin"),
        gh: wurzel.join("ghbin"),
        wegwerf: wurzel.join("wegwerf"),
        log: wurzel.join("attrappe.log"),
        pidfile: wurzel.join("attrappe.pid"),
        skript: wurzel.join("bin/copilot"),
        hilfskind: None,
        opfer: wurzel.join("opfer"),
        wurzel,
    };
    for ordner in [&a.ziel, &a.echt, &a.bin, &a.gh, &a.wegwerf] {
        fs::create_dir_all(ordner).expect("folder");
    }
    for ebene in ablage_ebenen(&a.heim) {
        fs::set_permissions(ebene, fs::Permissions::from_mode(0o700)).expect("mode");
    }
    fs::set_permissions(&a.heim, fs::Permissions::from_mode(0o755)).expect("mode");
    passwd_schreiben(&a, &a.heim);
    skript_schreiben(&a.gh.join("gh"), "#!/bin/sh\nexit 1\n");
    a
}

/// The levels of the store below `heim`, from `.local` to `geheim`.
pub fn ablage_ebenen(heim: &Path) -> Vec<PathBuf> {
    let mut ort = heim.to_path_buf();
    [".local", "state", "agent-workbench", "halter", "geheim"]
        .iter()
        .map(|n| {
            ort = ort.join(n);
            ort.clone()
        })
        .collect()
}

/// The test `passwd`: an unrelated entry, then the own user id with `heim` as field 6.
pub fn passwd_schreiben(a: &Aufbau, heim: &Path) {
    let uid = eigene_uid(a);
    fs::write(
        &a.passwd,
        format!(
            "root:x:0:0:root:/root:/bin/sh
nutzer:x:{uid}:{uid}:Nutzer:{}:/bin/sh
",
            heim.display()
        ),
    )
    .expect("passwd");
}

/// Every entry below `ordner` with its mode and whether it is a link; a link is listed, never followed.
pub fn baum(ordner: &Path) -> Vec<(PathBuf, u32, bool)> {
    let mut aus = Vec::new();
    if let Ok(liste) = fs::read_dir(ordner) {
        for e in liste.flatten() {
            let p = e.path();
            let Ok(m) = fs::symlink_metadata(&p) else {
                continue;
            };
            aus.push((
                p.clone(),
                m.permissions().mode(),
                m.file_type().is_symlink(),
            ));
            if m.is_dir() {
                aus.extend(baum(&p));
            }
        }
    }
    aus.sort();
    aus
}

/// Writes an executable script from a child process. A write descriptor opened in this process can leak into the fork
/// of another test thread; `exec` of the script then fails with ETXTBSY ("text file busy") until that fork has
/// executed. In the child process the only write descriptor lives and ends there, before this function returns.
pub fn skript_schreiben(pfad: &Path, text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut kind = Command::new("/bin/sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
        .arg(pfad)
        .stdin(Stdio::piped())
        .spawn()
        .expect("writer");
    kind.stdin
        .take()
        .expect("stdin")
        .write_all(text.as_bytes())
        .expect("script text");
    assert!(kind.wait().expect("writer end").success(), "writer failed");
}

/// The text of the stand-in. `modus` is `attrappe` of the case; `token` goes only into this script.
pub fn attrappe_text(a: &Aufbau, modus: &str, token: &str) -> String {
    let kopf = format!(
        "#!/bin/sh\numask 077\nLOG='{log}'\n\
         {{\n echo \"argv:$*\"\n echo \"home:$HOME\"\n echo \"chome:$COPILOT_HOME\"\n echo \"path:$PATH\"\n \
         echo \"gh:$(command -v gh)\"\n echo \"modus:$(/bin/ls -ld \"$HOME\" \"$COPILOT_HOME\")\"\n \
         echo \"settingsmodus:$(/bin/ls -l \"$COPILOT_HOME/settings.json\")\"\n \
         echo \"settings:$(/bin/cat \"$COPILOT_HOME/settings.json\")\"\n \
         while read -r z; do case \"$z\" in 'Max core file size'*) echo \"kernlimit:$z\";; esac; done 2>/dev/null < /proc/self/limits\n \
         echo \"umgebung:\"\n /usr/bin/env\n\
         }} > \"$LOG\"\necho $$ > '{pid}'\n\
         printf 'To authenticate, visit https://github.com/login/device and enter code ABCD-1234\\n'\n\
         printf 'Waiting for authorization...\\n'\n",
        log = a.log.display(),
        pid = a.pidfile.display(),
    );
    let konfig =
        |inhalt: &str| format!("printf '%s' '{inhalt}' > \"$COPILOT_HOME/config.json\"\nexit 0\n");
    let rumpf = match modus {
        "erfolg" => konfig(&format!(
            "// User settings belong in settings.json.\n// This file is managed automatically.\n{{\n  \"appTipShown\": true,\n  \
             \"authTokens\": {{\n    \"https://github.com\": {{\n      \"attrappe\": \"{token}\"\n    }}\n  }},\n  \
             \"firstLaunchAt\": \"2026-11-21T09:00:00.000Z\",\n  \"lastLoggedInUser\": {{\"host\": \"https://github.com\", \
             \"login\": \"attrappe\"}},\n  \"loggedInUsers\": [{{\"host\": \"https://github.com\", \"login\": \"attrappe\"}}],\n  \
             \"reasoningSummariesCleanupDone\": true\n}}\n"
        )),
        "wettlauf" => format!(
            "echo fremd > '{}/anmeldung.copilot'\n{}",
            a.ziel.display(),
            konfig(&format!(
                "{{\"authTokens\":{{\"h\":{{\"a\":\"{token}\"}}}}}}"
            ))
        ),
        "abbruch" => "exit 1\n".to_owned(),
        "ohne-config" => "exit 0\n".to_owned(),
        "config-ohne-feld" => konfig("{\"banner\":\"never\"}"),
        "zwei-tokens" => konfig(&format!(
            "{{\"authTokens\":{{\"h\":{{\"a\":\"{token}\",\"b\":\"{token}x\"}}}}}}"
        )),
        "config-defekt" => konfig("{nicht json"),
        "alte-form" => konfig(&format!(
            "{{\"copilot_tokens\":{{\"https://github.com:attrappe\":\"{token}\"}}}}"
        )),
        "haengt" => "exec /bin/sleep 60\n".to_owned(),
        andere => panic!("unknown stand-in {andere}"),
    };
    format!("{kopf}{rumpf}")
}

/// Sets up a case. Returns the set-up and the token text.
pub fn fall_aufbauen(fall: &Value) -> (Aufbau, String) {
    let a = aufbau();
    let token = token_text(&fall["token"]);
    let modus = fall["attrappe"].as_str().expect("stand-in");
    if modus != "fehlt" {
        skript_schreiben(&a.skript, &attrappe_text(&a, modus, &token));
    }
    match fall["bundtool"].as_str().expect("tool") {
        "keins" => {}
        "leer" => skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 1\n"),
        "gefunden" => skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 0\n"),
        andere => panic!("unknown tool {andere}"),
    }
    match fall["ziel"].as_str().expect("target") {
        "frei" => {}
        "datei-da" => {
            let datei = a.ziel.join("anmeldung.copilot");
            fs::write(&datei, "alt-inhalt\n").expect("old file");
            fs::set_permissions(&datei, fs::Permissions::from_mode(0o600)).expect("mode");
        }
        andere => ziel_bauen(&a, andere),
    }
    match fall["echtes_heim"].as_str().expect("real home") {
        "leer" => {}
        "config-mit-token" => fs::write(
            a.echt.join("config.json"),
            format!(
                "{{\"authTokens\":{{\"h\":{{\"u\":\"gho_{}\"}}}}}}",
                "Z".repeat(36)
            ),
        )
        .expect("config"),
        "config-defekt" => fs::write(a.echt.join("config.json"), "{nicht json").expect("config"),
        andere => panic!("unknown real home {andere}"),
    }
    let mut a = a;
    match fall["reste"].as_str().expect("left-overs") {
        "keine" => {}
        "tot" => {
            rest_anlegen(&a.wegwerf, tote_pid());
        }
        "lebend" => {
            let k = Command::new("sleep").arg("60").spawn().expect("sleep");
            rest_anlegen(&a.wegwerf, k.id());
            a.hilfskind = Some(k);
        }
        "verweis" => {
            fs::create_dir_all(&a.opfer).expect("victim");
            fs::write(a.opfer.join("probe.txt"), "bleibt\n").expect("probe");
            std::os::unix::fs::symlink(&a.opfer, a.wegwerf.join(rest_name(tote_pid())))
                .expect("link");
        }
        "nicht-loeschbar" => {
            let ordner = rest_anlegen(&a.wegwerf, tote_pid());
            fs::set_permissions(ordner.join("heim"), fs::Permissions::from_mode(0o500))
                .expect("mode");
        }
        andere => panic!("unknown left-over {andere}"),
    }
    if fall["wegwerf_wurzel"] == "fehlt" {
        fs::remove_dir(&a.wegwerf).expect("remove");
    }
    (a, token)
}

/// The values of `ziel` that both ways share (contract 9, section 7, "Ziel der Fälle"); `frei` and the key file
/// cases are built by the caller.
pub fn ziel_bauen(a: &Aufbau, ziel: &str) {
    match ziel {
        "frei" => {}
        "ordner-fehlt" => fs::remove_dir_all(a.heim.join(".local")).expect("remove"),
        "schreibgeschuetzt" => {
            fs::set_permissions(&a.ziel, fs::Permissions::from_mode(0o500)).expect("mode");
        }
        "heim-fehlt" => passwd_schreiben(a, &a.wurzel.join("kein-heim")),
        "ebene-verweis" => {
            let state = a.heim.join(".local/state");
            fs::remove_dir_all(&state).expect("remove");
            fs::create_dir_all(a.aussen.join("agent-workbench/halter/geheim")).expect("outside");
            for ebene in [
                a.aussen.clone(),
                a.aussen.join("agent-workbench"),
                a.aussen.join("agent-workbench/halter"),
                a.aussen.join("agent-workbench/halter/geheim"),
            ] {
                fs::set_permissions(ebene, fs::Permissions::from_mode(0o700)).expect("mode");
            }
            std::os::unix::fs::symlink(&a.aussen, &state).expect("link");
        }
        "ebene-geteilt" => {
            fs::set_permissions(
                a.heim.join(".local/state/agent-workbench"),
                fs::Permissions::from_mode(0o770),
            )
            .expect("mode");
        }
        "alte-datei" => {
            fs::create_dir_all(a.alt.parent().expect("folder")).expect("old folder");
            fs::write(&a.alt, "alter-schluessel\n").expect("old key");
            fs::set_permissions(&a.alt, fs::Permissions::from_mode(0o600)).expect("mode");
        }
        andere => panic!("unknown target {andere}"),
    }
}

/// The pid of a process that has ended and is reaped.
pub fn tote_pid() -> u32 {
    let mut k = Command::new("true").spawn().expect("true");
    let pid = k.id();
    k.wait().expect("wait");
    pid
}

/// Name of a left-over folder for `pid`.
pub fn rest_name(pid: u32) -> String {
    format!("wb-anmelden-{pid}-1700000000000000000")
}

/// A left-over folder with a file in it (it may hold a token) below `wurzel`.
pub fn rest_anlegen(wurzel: &Path, pid: u32) -> PathBuf {
    let ordner = wurzel.join(rest_name(pid));
    fs::create_dir_all(ordner.join("heim/.copilot")).expect("left-over");
    fs::write(ordner.join("heim/.copilot/config.json"), "{}").expect("file");
    fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("mode");
    ordner
}

pub const VERBOTEN: [&str; 6] = [
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_RUNTIME_DIR",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "COPILOT_GITHUB_TOKEN",
    "SSH_AUTH_SOCK",
];

pub const PROXY: &str = "http://127.0.0.1:9";

/// The command under test with a clean environment, the seams and the forbidden variables set on purpose.
pub fn befehl(a: &Aufbau, args: &[String]) -> Command {
    let mut c = Command::new(BIN);
    c.args(args)
        .env_clear()
        .env("PATH", format!("{}:{}", a.bin.display(), a.gh.display()))
        .env("HOME", a.wurzel.join("echt"))
        .env("WB_TESTUMGEBUNG", "1")
        .env("WB_ANMELDEN_PASSWD", &a.passwd)
        .env("WB_ANMELDEN_COPILOT_HEIM", &a.echt)
        .env("WB_ANMELDEN_WURZEL", &a.wegwerf)
        .env("HTTPS_PROXY", PROXY)
        .env("UNRELATED_VARIABLE", "bleibt-draussen")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in VERBOTEN {
        c.env(name, "dummy-umgebungswert");
    }
    c
}

pub struct Ausgang {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn ausgang(kind: Child) -> Ausgang {
    let aus = kind.wait_with_output().expect("output");
    Ausgang {
        exit: aus.status.code().expect("normal exit"),
        stdout: String::from_utf8(aus.stdout).expect("utf-8"),
        stderr: String::from_utf8(aus.stderr).expect("utf-8"),
    }
}

/// The output of a program that has already ended (after [`warte_auf_ende`]). Its pipes must close at once; a child
/// that outlives it (the CLI after a signal) would hold them open. Fails after 5 s instead of hanging.
pub fn ausgabe_beendet(kind: Child) -> std::process::Output {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(kind.wait_with_output());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("the output of the ended program stays open: a child of it lives on")
        .expect("output")
}

/// [`ausgabe_beendet`] as an [`Ausgang`].
pub fn ausgang_beendet(kind: Child) -> Ausgang {
    let aus = ausgabe_beendet(kind);
    Ausgang {
        exit: aus.status.code().expect("normal exit"),
        stdout: String::from_utf8(aus.stdout).expect("utf-8"),
        stderr: String::from_utf8(aus.stderr).expect("utf-8"),
    }
}

pub fn laufen(a: &Aufbau, args: &[String]) -> Ausgang {
    laufen_benannt(a, args, "")
}

/// [`laufen`] with a deadline of 20 s: a program that does not end is killed by its pid and the test fails with
/// `name` instead of hanging.
pub fn laufen_benannt(a: &Aufbau, args: &[String], name: &str) -> Ausgang {
    let kind = befehl(a, args).spawn().expect("spawn");
    let pid = kind.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(ausgang(kind));
    });
    let Ok(aus) = rx.recv_timeout(Duration::from_secs(20)) else {
        signal_senden("KILL", pid);
        panic!("{name}: the program did not end within 20 s");
    };
    aus
}

/// Waits for a file, at most `frist`.
pub fn warte_auf(pfad: &Path, frist: Duration) -> bool {
    let ende = Instant::now() + frist;
    while Instant::now() < ende {
        if pfad.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    pfad.exists()
}

/// Waits for the end of the child, at most `frist`; kills it and fails after that.
pub fn warte_auf_ende(kind: &mut Child, frist: Duration) {
    let ende = Instant::now() + frist;
    while Instant::now() < ende {
        if kind.try_wait().expect("wait").is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = kind.kill();
    panic!("the program did not end in time");
}

pub fn signal_senden(name: &str, pid: u32) {
    let st = Command::new("kill")
        .args(["-s", name, &pid.to_string()])
        .status()
        .expect("kill");
    assert!(st.success(), "kill -s {name} {pid}");
}

pub fn prozess_lebt(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .expect("kill")
        .success()
}

/// Every file under `ordner`, recursive.
pub fn dateien(ordner: &Path) -> Vec<PathBuf> {
    let mut aus = Vec::new();
    if let Ok(liste) = fs::read_dir(ordner) {
        for e in liste.flatten() {
            let p = e.path();
            if p.is_dir() {
                aus.extend(dateien(&p));
            } else {
                aus.push(p);
            }
        }
    }
    aus
}

pub fn log_lesen(a: &Aufbau) -> Option<String> {
    let mut text = String::new();
    fs::File::open(&a.log)
        .ok()?
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

pub fn eigene_uid(a: &Aufbau) -> u32 {
    fs::metadata(&a.wurzel).expect("meta").uid()
}
