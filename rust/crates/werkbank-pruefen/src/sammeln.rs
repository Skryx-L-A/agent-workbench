//! Reads the real machine into a [`Lage`]. Nothing here changes the machine.
//!
//! Every check has a time limit of [`FRIST`]; a check that could not be done reports `nicht_pruefbar` (fail closed, never
//! a quiet green). The only commands that start are `systemctl` (read), `dpkg-query` (read), `id` (read) and `setpriv`
//! with `bwrap` as an unprivileged user (the two probes of the profile; they change nothing). The key file is never read:
//! only its kind, owner, mode and size.
//!
//! Who the checks are for: with `sudo` (real user id 0) the user that called it (`SUDO_USER`); without root the user of the real
//! user id, named by `/etc/passwd` (then `SUDO_USER` is ignored: an environment variable of the caller sets no other user).
//! Without root every probe runs directly as that user and a check that the user cannot read stays `nicht_pruefbar`.

use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use formate::kette::sha256_hex;

use crate::lage::{Abweichung, DIENSTE, Einstellungen, HARNESS, Lage, OK, Paketstand, ordner_form};
use crate::manifest::{self, Manifest, PROFIL_A, PROFIL_B, STARTER};
use crate::ohne_doppel;
use crate::programmordner;

/// The time limit of one check.
pub const FRIST: Duration = Duration::from_secs(10);

/// The largest settings file in bytes (contract 2).
pub const EINSTELLUNGEN_GRENZE: usize = 65_536;

/// The largest `lauf.json` in bytes that is read for the approval of the CA (the file is some hundred bytes).
pub const LAUF_GRENZE: usize = 65_536;
/// The largest state file `programmordner-abgelehnt` that is read (two short lines are the rule).
pub const ABGELEHNT_GRENZE: usize = 4096;

/// The user the probes run as when no `sudo` user is known: `nobody`.
const NIEMAND: (&str, &str) = ("65534", "65534");

/// The output of a command that ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ausgabe {
    /// The exit code; `None` when a signal ended the program.
    pub rc: Option<i32>,
    /// What it wrote to standard output.
    pub stdout: String,
    /// What it wrote to standard error (at most 64 KiB; read only after it ended).
    pub stderr: String,
}

/// Starts programs. A test replaces it.
pub trait System {
    /// Runs `programm` with `args` and waits at most `frist`. `None` when it did not start or did not end in time.
    fn befehl(&self, programm: &str, args: &[&str], frist: Duration) -> Option<Ausgabe>;
}

/// The real system: a child process with a time limit, no shell, no standard input.
pub struct Echt;

impl System for Echt {
    fn befehl(&self, programm: &str, args: &[&str], frist: Duration) -> Option<Ausgabe> {
        let mut kind = Command::new(programm)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;
        let start = Instant::now();
        let status = loop {
            if let Some(s) = kind.try_wait().ok()? {
                break s;
            }
            if start.elapsed() > frist {
                kind.kill().ok();
                kind.wait().ok();
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let mut stdout = String::new();
        kind.stdout
            .take()?
            .take(1 << 20)
            .read_to_string(&mut stdout)
            .ok()?;
        let mut stderr = String::new();
        kind.stderr
            .take()?
            .take(1 << 16)
            .read_to_string(&mut stderr)
            .ok()?;
        Some(Ausgabe {
            rc: status.code(),
            stdout,
            stderr,
        })
    }
}

/// Where and as whom the checks run.
pub struct Umgebung<'a> {
    /// The root of the file tree (`/`; a test uses a folder).
    pub wurzel: PathBuf,
    /// The user the checks are for (see [`aufrufer`]), if any.
    pub nutzer: Option<String>,
    /// `true` when the program runs as root (real user id 0); `false` when a user without root called it.
    pub root: bool,
    /// How programs start.
    pub system: &'a dyn System,
}

/// The real user id of this process (`getuid`), read from `/proc/self/status`; `None` when it cannot be read.
///
/// Linux only: macOS has no `/proc`, so there the id is always `None` and every check stays `nicht_pruefbar` (fail
/// closed, see [`aufrufer`]). The program checks a Linux installation (`dpkg`, `systemctl`, `bwrap`).
#[must_use]
pub fn reale_uid() -> Option<u32> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|z| z.strip_prefix("Uid:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// The user the checks are for and whether the program runs as root: `(user, root)`.
///
/// Real user id 0: the user that called `sudo` (`sudo_nutzer`), or none. Any other id: the user of that id from `etc/passwd`
/// below `wurzel`, never `sudo_nutzer`. An unknown id (`None`) counts as no root and no user, so every check stays
/// `nicht_pruefbar` (fail closed).
#[must_use]
pub fn aufrufer(
    sudo_nutzer: Option<&str>,
    uid: Option<u32>,
    wurzel: &Path,
) -> (Option<String>, bool) {
    match uid {
        Some(0) => (
            sudo_nutzer.filter(|n| nutzer_gut(n)).map(str::to_owned),
            true,
        ),
        Some(uid) => {
            let name = fs::read_to_string(wurzel.join("etc/passwd"))
                .ok()
                .and_then(|t| {
                    t.lines().find_map(|z| {
                        let f: Vec<&str> = z.split(':').collect();
                        (f.len() >= 7 && f[2].parse::<u32>().ok() == Some(uid))
                            .then(|| f[0].to_owned())
                    })
                });
            (name.filter(|n| nutzer_gut(n)), false)
        }
        None => (None, false),
    }
}

/// A user name as `adduser` allows it; anything else is not a user for this program.
#[must_use]
pub fn nutzer_gut(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0] == b'_')
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_-".contains(c))
}

fn lesen_bis(pfad: &Path, grenze: usize) -> std::io::Result<Vec<u8>> {
    let mut puffer = Vec::new();
    fs::File::open(pfad)?
        .take(u64::try_from(grenze).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut puffer)?;
    Ok(puffer)
}

fn mode_von(pfad: &Path) -> Option<fs::Metadata> {
    fs::symlink_metadata(pfad).ok()
}

impl Umgebung<'_> {
    fn pfad(&self, relativ: &str) -> PathBuf {
        self.wurzel.join(relativ)
    }

    fn nutzer_ordner(&self) -> Option<String> {
        self.nutzer
            .as_ref()
            .map(|n| format!("var/lib/werkbank/{n}"))
    }

    /// The numeric ids (`uid`, `gid`) of the user, from `id`.
    fn ids(&self) -> Option<(String, String)> {
        let nutzer = self.nutzer.as_deref()?;
        let nummer = |opt: &str| {
            let a = self.system.befehl("id", &[opt, "--", nutzer], FRIST)?;
            let t = a.stdout.trim().to_owned();
            (a.rc == Some(0) && !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit())).then_some(t)
        };
        Some((nummer("-u")?, nummer("-g")?))
    }

    fn dienst(&self, einheit: &str) -> &'static str {
        let Some(nutzer) = self.nutzer.as_deref() else {
            return "nicht_pruefbar";
        };
        // As root the user manager of the user is asked by name; a user without root asks its own manager.
        let ziel = format!("--machine={nutzer}@");
        let mut args = vec!["--user"];
        if self.root {
            args.push(&ziel);
        }
        args.extend([
            "show",
            "--no-pager",
            "-p",
            "LoadState",
            "-p",
            "ActiveState",
            "--",
            einheit,
        ]);
        let a = self.system.befehl("systemctl", &args, FRIST);
        let Some(a) = a.filter(|a| a.rc == Some(0)) else {
            return "nicht_pruefbar";
        };
        let wert = |k: &str| {
            a.stdout
                .lines()
                .find_map(|z| z.strip_prefix(k).and_then(|r| r.strip_prefix('=')))
                .unwrap_or("")
        };
        match (wert("LoadState"), wert("ActiveState")) {
            ("not-found", _) => "fehlt",
            (_, "active") => OK,
            (_, "failed") => "fehlgeschlagen",
            ("loaded" | "masked" | "error", _) => "inaktiv",
            _ => "nicht_pruefbar",
        }
    }

    fn einstellungen(&self) -> Einstellungen {
        let eins = |z: &str, c: Option<&str>| Einstellungen {
            zustand: z.to_owned(),
            code: c.map(str::to_owned),
        };
        let Some(ordner) = self.nutzer_ordner() else {
            return eins("nicht_pruefbar", None);
        };
        let pfad = self.pfad(&format!("{ordner}/einstellungen.json"));
        match mode_von(&pfad) {
            None => return eins("fehlt", None),
            Some(m) if !m.is_file() => return eins("fehlt", None),
            Some(_) => {}
        }
        let Ok(roh) = lesen_bis(&pfad, EINSTELLUNGEN_GRENZE) else {
            return eins("fehlt", None);
        };
        // Stages 1 and 2 of contract 2 only: the library `einstellungen::lesen` is not in the tree yet.
        if roh.len() > EINSTELLUNGEN_GRENZE {
            return eins("ungueltig", Some("einst.zu_gross"));
        }
        if roh.starts_with(&[0xef, 0xbb, 0xbf])
            || !ohne_doppel::lesen(&roh).is_ok_and(|v| v.is_object())
        {
            return eins("ungueltig", Some("einst.json_ungueltig"));
        }
        eins("ok", None)
    }

    /// The approval of the local CA from `/etc/werkbank/lauf.json`: `""` when the file is not there (the package wrote none, so there is
    /// nothing to report), `zugelassen`, `offen` or `abgelehnt`, else `unbekannt` (a link, not a file, unreadable, not JSON, a repeated
    /// key, or none of the three values: the cloud route counts as closed). Nothing else of the file is read or shown.
    fn ca_zulassung(&self) -> &'static str {
        let pfad = self.pfad("etc/werkbank/lauf.json");
        match mode_von(&pfad) {
            None => return "",
            Some(m) if !m.is_file() => return "unbekannt",
            Some(_) => {}
        }
        let Ok(roh) = lesen_bis(&pfad, LAUF_GRENZE) else {
            return "unbekannt";
        };
        if roh.len() > LAUF_GRENZE {
            return "unbekannt";
        }
        // The package counts the key as text (`"ca_zulassung"` more than once is `offen`); so does this check, before it reads JSON.
        if roh
            .windows(14)
            .filter(|f| *f == b"\"ca_zulassung\"")
            .count()
            > 1
        {
            return "doppelt";
        }
        match ohne_doppel::lesen(&roh)
            .ok()
            .as_ref()
            .and_then(|w| w.get("ca_zulassung"))
            .and_then(|z| z.as_str())
        {
            Some("zugelassen") => "zugelassen",
            Some("offen") => "offen",
            Some("abgelehnt") => "abgelehnt",
            _ => "unbekannt",
        }
    }

    /// `lauf.json` as JSON (a repeated key or a large file gives `None`: then there is nothing to check here; `ca_zulassung` reports it).
    fn lauf_wert(&self) -> Option<serde_json::Value> {
        let pfad = self.pfad("etc/werkbank/lauf.json");
        if !mode_von(&pfad)?.is_file() {
            return None;
        }
        let roh = lesen_bis(&pfad, LAUF_GRENZE).ok()?;
        if roh.len() > LAUF_GRENZE {
            return None;
        }
        ohne_doppel::lesen(&roh).ok()
    }

    /// The program folders of `lauf.json` against the rule of the enclosure (`programmordner::verboten`), and the harnesses that the file
    /// does not list. `("ok", [])` when there is no file or it has no usable `heim`.
    fn programmordner_und_harness(&self) -> (&'static str, Vec<(String, String)>) {
        let Some(w) = self.lauf_wert() else {
            return (OK, Vec::new());
        };
        let Some(heim) = w
            .get("heim")
            .and_then(serde_json::Value::as_str)
            .filter(|h| h.starts_with('/') && !h.chars().any(char::is_control))
        else {
            return (OK, Vec::new());
        };
        let verboten = w
            .get("programmordner")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|l| {
                l.iter()
                    .filter_map(serde_json::Value::as_str)
                    .any(|o| programmordner::verboten(Path::new(o), Path::new(heim)))
            });
        let eingetragen = w.get("harness").and_then(serde_json::Value::as_object);
        let mut harness = Vec::new();
        for name in HARNESS {
            if eingetragen.is_some_and(|h| h.contains_key(name)) {
                continue;
            }
            harness.push((name.to_owned(), self.harness_zustand(name, heim).to_owned()));
        }
        (if verboten { "verboten" } else { OK }, harness)
    }

    /// The state file of the package `var/lib/werkbank/programmordner-abgelehnt` (written by `postinst`, one line `<harness> <folder>` for each harness
    /// it did not enter because of a program folder): the harnesses of [`HARNESS`] in their order, each once. A file that is no plain file, is larger
    /// than 4096 bytes or holds a line of another form gives nothing for that line; no program runs and no second search is made here.
    fn abgelehnt(&self) -> Vec<(String, String)> {
        let pfad = self.pfad("var/lib/werkbank/programmordner-abgelehnt");
        if !mode_von(&pfad).is_some_and(|m| m.is_file()) {
            return Vec::new();
        }
        let Ok(roh) = lesen_bis(&pfad, ABGELEHNT_GRENZE) else {
            return Vec::new();
        };
        if roh.len() > ABGELEHNT_GRENZE {
            return Vec::new();
        }
        let text = String::from_utf8_lossy(&roh);
        let mut aus: Vec<(String, String)> = Vec::new();
        for zeile in text.lines() {
            let Some((h, o)) = zeile.split_once(' ') else {
                continue;
            };
            if HARNESS.contains(&h) && ordner_form(o) && !aus.iter().any(|(n, _)| n == h) {
                aus.push((h.to_owned(), o.to_owned()));
            }
        }
        aus.sort_by_key(|(n, _)| HARNESS.iter().position(|k| k == n));
        aus
    }

    /// Why a harness is not in `lauf.json`: a read-only look into the fixed places of the search (no program runs; only `stat` and the real path).
    fn harness_zustand(&self, name: &str, heim: &str) -> &'static str {
        let mut orte: Vec<String> = Vec::new();
        if let Ok(liste) = fs::read_dir(self.pfad(&format!(
            "{}/.nvm/versions/node",
            heim.trim_start_matches('/')
        ))) {
            for e in liste.flatten() {
                orte.push(format!(
                    "{heim}/.nvm/versions/node/{}/bin",
                    e.file_name().to_string_lossy()
                ));
            }
        }
        for o in [".local/bin", ".npm-global/bin", ".volta/bin"] {
            orte.push(format!("{heim}/{o}"));
        }
        orte.extend(["/usr/local/bin".to_owned(), "/usr/bin".to_owned()]);
        let mut gefunden = false;
        let wurzel = fs::canonicalize(&self.wurzel).unwrap_or_else(|_| self.wurzel.clone());
        for o in orte {
            let datei = self.pfad(&format!("{}/{name}", o.trim_start_matches('/')));
            let Ok(m) = fs::metadata(&datei) else {
                continue;
            };
            if !m.is_file() || m.mode() & 0o111 == 0 {
                continue;
            }
            gefunden = true;
            let Ok(echt) = fs::canonicalize(&datei) else {
                continue;
            };
            let echt = echt
                .strip_prefix(&wurzel)
                .map_or_else(|_| echt.clone(), |r| Path::new("/").join(r));
            let ok = ["/usr/", "/opt/"].iter().any(|p| echt.starts_with(p))
                || [".nvm", ".local", ".npm-global", ".volta"]
                    .iter()
                    .any(|d| echt.starts_with(Path::new(heim).join(d)));
            if ok {
                return "nicht_eingetragen";
            }
        }
        if gefunden {
            "ausserhalb"
        } else {
            "nicht_gefunden"
        }
    }

    fn ordner(&self, relativ: &str, uid: Option<u32>) -> &'static str {
        let Some(uid) = uid else {
            return "nicht_pruefbar";
        };
        match mode_von(&self.pfad(relativ)) {
            None => "fehlt",
            Some(m) if !m.is_dir() => "kein_ordner",
            Some(m) if m.uid() != uid || m.mode() & 0o7777 != 0o700 => "rechte",
            Some(_) => OK,
        }
    }

    fn nutzerordner(&self, uid: Option<u32>) -> &'static str {
        let Some(o) = self.nutzer_ordner() else {
            return "nicht_pruefbar";
        };
        match self.ordner(&o, uid) {
            "kein_ordner" => "rechte",
            z => z,
        }
    }

    fn vault(&self, uid: Option<u32>) -> &'static str {
        let Some(o) = self.nutzer_ordner() else {
            return "nicht_pruefbar";
        };
        self.ordner(&format!("{o}/vorfuehr-vault"), uid)
    }

    fn schluessel(&self, uid: Option<u32>) -> &'static str {
        let (Some(o), Some(uid)) = (self.nutzer_ordner(), uid) else {
            return "nicht_pruefbar";
        };
        match mode_von(&self.pfad(&format!("{o}/modellweg.schluessel"))) {
            None => "fehlt",
            Some(m) if !m.file_type().is_file() => "art",
            Some(m) if m.uid() != uid => "eigentuemer",
            Some(m) if m.mode() & 0o7777 != 0o600 => "modus",
            Some(m) if m.len() == 0 => "leer",
            Some(_) => OK,
        }
    }

    /// The sign-in file of the route `copilot`: kind, owner, mode, emptiness and form (one line of 1 to 4096 printable ASCII
    /// characters without a space, with or without the final line feed). The content never leaves this function.
    fn copilot(&self, uid: Option<u32>) -> &'static str {
        let (Some(o), Some(uid)) = (self.nutzer_ordner(), uid) else {
            return "nicht_pruefbar";
        };
        let pfad = self.pfad(&format!("{o}/copilot.schluessel"));
        match mode_von(&pfad) {
            None => "fehlt",
            Some(m) if !m.file_type().is_file() => "art",
            Some(m) if m.uid() != uid => "eigentuemer",
            Some(m) if m.mode() & 0o7777 != 0o600 => "modus",
            Some(m) if m.len() == 0 => "leer",
            Some(_) => match lesen_bis(&pfad, 4098) {
                Ok(b) if copilot_form(&b) => OK,
                Ok(_) => "form",
                Err(_) => "nicht_pruefbar",
            },
        }
    }

    /// The manifest and what it says about the files. `Err` holds the state of the manifest.
    fn manifest(&self) -> Result<Manifest, &'static str> {
        let pfad = self.pfad(manifest::ORT);
        match mode_von(&pfad) {
            None => return Err("fehlt"),
            Some(m) if !m.is_file() => return Err("ungueltig"),
            Some(_) => {}
        }
        let roh = lesen_bis(&pfad, manifest::GRENZE).map_err(|_| "nicht_pruefbar")?;
        manifest::pruefen(&roh).map_err(|_| "ungueltig")
    }

    fn paketstand(&self, m: &Result<Manifest, &'static str>) -> Paketstand {
        let leer = |z: &str| Paketstand {
            manifest: z.to_owned(),
            testbau: false,
            version_abweichend: false,
            dateien: Vec::new(),
        };
        let m = match m {
            Ok(m) => m,
            Err(z) => return leer(z),
        };
        let installiert = self
            .system
            .befehl("dpkg-query", &["-W", "-f=${Version}", "werkbank"], FRIST)
            .filter(|a| a.rc == Some(0));
        let Some(installiert) = installiert else {
            return leer("nicht_pruefbar");
        };
        let mut dateien = Vec::new();
        for d in &m.dateien {
            match mode_von(&self.pfad(&d.pfad)) {
                None => dateien.push((d.pfad.clone(), Abweichung::Fehlt)),
                Some(md) => {
                    let gleich = md.is_file()
                        && md.len() == d.groesse
                        && fs::read(self.pfad(&d.pfad)).is_ok_and(|b| sha256_hex(&b) == d.sha256);
                    if !gleich {
                        dateien.push((d.pfad.clone(), Abweichung::Abweichend));
                    }
                }
            }
        }
        let gelistet: std::collections::BTreeSet<&str> =
            m.dateien.iter().map(|d| d.pfad.as_str()).collect();
        for wurzel in ["usr/lib/werkbank", "usr/share/werkbank"] {
            let mut stapel = vec![wurzel.to_owned()];
            while let Some(rel) = stapel.pop() {
                let Ok(eintraege) = fs::read_dir(self.pfad(&rel)) else {
                    continue;
                };
                for e in eintraege.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let kind = format!("{rel}/{name}");
                    match e.file_type() {
                        Ok(t) if t.is_dir() => stapel.push(kind),
                        Ok(_) => {
                            // The copy of `bwrap` (form B of the profile) is made by `postinst` and is not in the package.
                            if !gelistet.contains(kind.as_str())
                                && kind != manifest::ORT
                                && kind != "usr/lib/werkbank/bwrap"
                            {
                                dateien.push((kind, Abweichung::Fremd));
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        for (pfad, _) in &mut dateien {
            *pfad = crate::bewerte::kuerzen(pfad);
        }
        Paketstand {
            manifest: OK.to_owned(),
            testbau: m.testbau,
            version_abweichend: installiert.stdout.trim() != m.version,
            dateien,
        }
    }

    /// The names of the profiles in a profile file (`profile <name> ...` or `/path ... {`).
    fn profilnamen(text: &str) -> Vec<String> {
        text.lines()
            .filter_map(|z| {
                let z = z.split('#').next().unwrap_or("").trim();
                if !z.ends_with('{') {
                    return None;
                }
                let mut w = z.split_whitespace();
                match w.next()? {
                    "profile" => w.next().map(str::to_owned),
                    erstes if erstes.starts_with('/') => Some(erstes.to_owned()),
                    _ => None,
                }
            })
            .collect()
    }

    /// Runs `programm` as the unprivileged user with the ids `ids` (no groups). Without root the program already runs as
    /// that user and `setpriv` could not change the ids, so the program starts directly.
    fn als_nutzer(&self, ids: &(String, String), programm: &str, args: &[&str]) -> Option<Ausgabe> {
        if !self.root {
            return self.system.befehl(programm, args, FRIST);
        }
        let mut a = vec![
            "--reuid",
            ids.0.as_str(),
            "--regid",
            ids.1.as_str(),
            "--clear-groups",
            programm,
        ];
        a.extend_from_slice(args);
        self.system.befehl("setpriv", &a, FRIST)
    }

    /// `true` while a profile of the product (label `werkbank-*` or a path below `/usr/lib/werkbank/`) is loaded in the
    /// mode `unconfined`. No profile of the package may be `unconfined`; an unreadable list says nothing here.
    fn unconfined_geladen(&self) -> bool {
        let Ok(geladen) = fs::read_to_string(self.pfad("sys/kernel/security/apparmor/profiles"))
        else {
            return false;
        };
        geladen.lines().any(|z| {
            z.rsplit_once(" (").is_some_and(|(name, modus)| {
                (name.starts_with("werkbank-") || name.starts_with("/usr/lib/werkbank/"))
                    && modus.trim_end().trim_end_matches(')') == "unconfined"
            })
        })
    }

    /// The state of Ubuntu's own profile `bwrap-userns-restrict` (labels `bwrap` and `unpriv_bwrap`): `geladen`, `entladen`,
    /// or `""` when the lock is off or nothing can be said. As root the list of profiles is the source. Without root the list is
    /// not readable, so the state comes from the probe of the system `bwrap` that the user starts: a child whose label holds
    /// `bwrap` or `unpriv_bwrap` means `geladen`; a probe that fails with `uid map: Permission denied` means `entladen`;
    /// anything else says nothing.
    fn ubuntu_profil(&self) -> &'static str {
        let sperre =
            fs::read_to_string(self.pfad("proc/sys/kernel/apparmor_restrict_unprivileged_userns"))
                .is_ok_and(|s| s.trim() == "1");
        if !sperre {
            return "";
        }
        let Ok(geladen) = fs::read_to_string(self.pfad("sys/kernel/security/apparmor/profiles"))
        else {
            return if self.root {
                ""
            } else {
                self.ubuntu_profil_probe()
            };
        };
        let da = geladen.lines().any(|z| {
            z.rsplit_once(" (")
                .is_some_and(|(n, _)| n == "bwrap" || n == "unpriv_bwrap")
        });
        if da { "geladen" } else { "entladen" }
    }

    /// The state of Ubuntu's profile from the probe of the system `bwrap` as the user (see [`Self::ubuntu_profil`]).
    fn ubuntu_profil_probe(&self) -> &'static str {
        let ids = self
            .ids()
            .unwrap_or_else(|| (NIEMAND.0.to_owned(), NIEMAND.1.to_owned()));
        let Some(a) = self.als_nutzer(
            &ids,
            "/usr/bin/bwrap",
            &[
                "--unshare-user",
                "--ro-bind",
                "/",
                "/",
                "cat",
                "/proc/self/attr/current",
            ],
        ) else {
            return "";
        };
        if a.rc == Some(0) {
            let label = a
                .stdout
                .trim()
                .rsplit_once(" (")
                .map_or(a.stdout.trim(), |(n, _)| n);
            return if label
                .split("//")
                .any(|t| matches!(t.trim_start_matches('&'), "bwrap" | "unpriv_bwrap"))
            {
                "geladen"
            } else {
                ""
            };
        }
        if a.stderr.contains("uid map: Permission denied") {
            "entladen"
        } else {
            ""
        }
    }

    /// The checks that apply only while the lock on user namespaces is on: profile loaded, no foreign profile on
    /// `bwrap` or on the starter, and the system `bwrap` stays closed to an unprivileged user. The second value is `true`
    /// when the system `bwrap` is open only through the profile that Ubuntu ships with `bubblewrap` (a note, not a finding).
    fn sperre_befund(&self, text: &[u8], ids: &(String, String)) -> (Option<&'static str>, bool) {
        let Ok(geladen) = fs::read_to_string(self.pfad("sys/kernel/security/apparmor/profiles"))
        else {
            return (Some("nicht_pruefbar"), false);
        };
        let geladen: Vec<&str> = geladen
            .lines()
            .filter_map(|z| z.rsplit_once(" (").map(|(n, _)| n))
            .collect();
        let eigene = Self::profilnamen(&String::from_utf8_lossy(text));
        if eigene.is_empty() || eigene.iter().any(|n| !geladen.contains(&n.as_str())) {
            return (Some("nicht_geladen"), false);
        }
        if geladen.iter().any(|n| {
            n.starts_with('/')
                && !eigene.iter().any(|e| e == n)
                && (anhang_trifft(n, "/usr/bin/bwrap") || anhang_trifft(n, &format!("/{STARTER}")))
        }) {
            return (Some("fremd"), false);
        }
        // The child of the probe prints its own label, so the finding depends on who opened the system `bwrap`.
        match self.als_nutzer(
            ids,
            "/usr/bin/bwrap",
            &[
                "--unshare-user",
                "--ro-bind",
                "/",
                "/",
                "cat",
                "/proc/self/attr/current",
            ],
        ) {
            None => (Some("nicht_pruefbar"), false),
            Some(a) if a.rc == Some(0) => match bwrap_label(&a.stdout) {
                Label::Ubuntu => (None, true),
                Label::Frei => (Some("system_bwrap_frei"), false),
                Label::Fremd => (Some("nicht_pruefbar"), false),
            },
            Some(_) => (None, false),
        }
    }

    /// The state of the profile and the note on the system `bwrap` (see `sperre_befund`).
    fn apparmor(&self, m: &Result<Manifest, &'static str>) -> (&'static str, bool) {
        let mut hinweis = false;
        let zustand = self.apparmor_zustand(m, &mut hinweis);
        (zustand, hinweis)
    }

    fn apparmor_zustand(
        &self,
        m: &Result<Manifest, &'static str>,
        hinweis: &mut bool,
    ) -> &'static str {
        if self.unconfined_geladen() {
            return "unconfined";
        }
        if mode_von(&self.pfad("usr/bin/bwrap")).is_none() {
            return "bwrap_fehlt";
        }
        let (form_b, datei) = if mode_von(&self.pfad(PROFIL_A)).is_some() {
            (false, PROFIL_A)
        } else if mode_von(&self.pfad(PROFIL_B)).is_some() {
            (true, PROFIL_B)
        } else {
            return "datei_fehlt";
        };
        let Ok(text) = fs::read(self.pfad(datei)) else {
            return "nicht_pruefbar";
        };
        if let Ok(m) = m
            && m.dateien
                .iter()
                .any(|d| d.pfad == datei && d.sha256 != sha256_hex(&text))
        {
            return "abweichend";
        }
        let sperre =
            fs::read_to_string(self.pfad("proc/sys/kernel/apparmor_restrict_unprivileged_userns"))
                .is_ok_and(|s| s.trim() == "1");
        let ids = self
            .ids()
            .unwrap_or_else(|| (NIEMAND.0.to_owned(), NIEMAND.1.to_owned()));
        let probe_args = [
            "--unshare-user",
            "--unshare-pid",
            "--unshare-net",
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "true",
        ];
        if sperre {
            let (befund, ubuntu) = self.sperre_befund(&text, &ids);
            *hinweis = ubuntu;
            if let Some(befund) = befund {
                return befund;
            }
        }
        if form_b {
            let gleich = match (
                fs::read(self.pfad("usr/lib/werkbank/bwrap")),
                fs::read(self.pfad("usr/bin/bwrap")),
            ) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            };
            if !gleich {
                return "kopie_abweichend";
            }
        }
        // The probe of the enclosure. With the lock on, form A passes the profile only through the fixed starter, so the probe is
        // `werkbank-einschluss selbsttest`; without the starter in the tree it cannot run honestly. Without the lock, `bwrap`
        // is tried directly.
        let starter = format!("/{STARTER}");
        let (programm, args): (&str, &[&str]) = if form_b {
            ("/usr/lib/werkbank/bwrap", &probe_args)
        } else if sperre {
            if mode_von(&self.pfad(STARTER)).is_none() {
                return "nicht_pruefbar";
            }
            (starter.as_str(), &["selbsttest"])
        } else {
            ("/usr/bin/bwrap", &probe_args)
        };
        match self.als_nutzer(&ids, programm, args) {
            None => "nicht_pruefbar",
            Some(a) if a.rc == Some(0) => OK,
            Some(_) => "probelauf",
        }
    }

    fn konten(&self) -> u32 {
        let n = fs::read_to_string(self.pfad("etc/passwd")).map_or(1, |t| {
            t.lines()
                .filter(|z| {
                    let f: Vec<&str> = z.split(':').collect();
                    f.len() >= 7
                        && f[2]
                            .parse::<u32>()
                            .is_ok_and(|u| (1000..60000).contains(&u))
                        && !f[6].ends_with("nologin")
                        && !f[6].ends_with("/false")
                })
                .count()
        });
        u32::try_from(n).unwrap_or(u32::MAX).max(1)
    }

    /// Runs the seven checks.
    #[must_use]
    pub fn lage(&self) -> Lage {
        let m = self.manifest();
        let uid = self.ids().and_then(|(u, _)| u.parse::<u32>().ok());
        let schluessel = self.schluessel(uid);
        let apparmor = self.apparmor(&m);
        let (programmordner, harness) = self.programmordner_und_harness();
        Lage {
            konten: self.konten(),
            dienste: DIENSTE
                .iter()
                .map(|n| ((*n).to_owned(), self.dienst(n).to_owned()))
                .collect(),
            apparmor: apparmor.0.to_owned(),
            system_bwrap: if apparmor.1 { "ubuntu" } else { "gesperrt" }.to_owned(),
            einstellungen: self.einstellungen(),
            nutzerordner: self.nutzerordner(uid).to_owned(),
            schluessel: schluessel.to_owned(),
            copilot: self.copilot(uid).to_owned(),
            ubuntu_profil: self.ubuntu_profil().to_owned(),
            vault: self.vault(uid).to_owned(),
            ca_zulassung: self.ca_zulassung().to_owned(),
            programmordner: programmordner.to_owned(),
            abgelehnt: self.abgelehnt(),
            harness,
            paketstand: self.paketstand(&m),
            // The package has no decision point (the Probelauf has no site); A108 does not apply.
            stelle_fehlt: false,
            ohne_root: !self.root,
        }
    }
}

/// Who opened the system `bwrap` for the probe of the unprivileged user.
#[derive(Debug, PartialEq, Eq)]
pub enum Label {
    /// Only the profile that Ubuntu ships with `bubblewrap` (`bwrap`, `unpriv_bwrap`).
    Ubuntu,
    /// A profile of the product, no profile at all (`unconfined`) or an empty answer.
    Frei,
    /// Any other profile: not judged here.
    Fremd,
}

/// Reads the label that the child of the probe printed (`/proc/self/attr/current`, for example `unpriv_bwrap (enforce)`).
#[must_use]
pub fn bwrap_label(ausgabe: &str) -> Label {
    let label = ausgabe
        .trim()
        .rsplit_once(" (")
        .map_or(ausgabe.trim(), |(n, _)| n);
    if label.is_empty() || label == "unconfined" || label.contains("werkbank-") {
        return Label::Frei;
    }
    if label
        .split("//")
        .map(|t| t.trim_start_matches('&'))
        .all(|t| t == "bwrap" || t == "unpriv_bwrap")
    {
        return Label::Ubuntu;
    }
    Label::Fremd
}

/// The form of the sign-in file: exactly one line of 1 to 4096 characters from `!` to `~`, with or without a final LF.
#[must_use]
pub fn copilot_form(bytes: &[u8]) -> bool {
    let zeile = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    (1..=4096).contains(&zeile.len()) && zeile.iter().all(|b| (0x21..=0x7e).contains(b))
}

/// A coarse counterpart of an `AppArmor` attachment: `*` and `**` stand for any text. Enough to see a foreign profile on `bwrap`.
#[must_use]
pub fn anhang_trifft(muster: &str, pfad: &str) -> bool {
    let teile: Vec<&str> = muster.split('*').filter(|t| !t.is_empty()).collect();
    if !muster.contains('*') {
        return muster == pfad;
    }
    let mut rest = pfad;
    for (i, t) in teile.iter().enumerate() {
        match rest.find(t) {
            Some(p) if i > 0 || muster.starts_with('*') || p == 0 => rest = &rest[p + t.len()..],
            _ => return false,
        }
    }
    muster.ends_with('*') || rest.is_empty()
}
