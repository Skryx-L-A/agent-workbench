//! Reads the real machine into a [`Lage`]. Nothing here changes the machine.
//!
//! Every check has a time limit of [`FRIST`]; a check that could not be done reports `nicht_pruefbar` (fail closed, never
//! a quiet green). The only commands that start are `systemctl` (read), `journalctl` (read, only the lines of
//! `wb-halter.service` of the user in its latest run), `dpkg-query` (read), `id` (read) and `setpriv`
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

use crate::gruppe::{nss_nur_dateien, private_gruppe};
use crate::lage::{Abweichung, DIENSTE, Einstellungen, HARNESS, Lage, OK, Paketstand, ordner_form};

/// Most journal lines of the holder read for the check of its chain.
pub const JOURNAL_ZEILEN: &str = "20000";

/// The state of the chain of the holder after the text of its journal (one message per line): `rechte` if a line starts
/// with `halter.kette_rechte:`, else `fehler` if a line starts with `halter.kette_fehler:`, else `ok`.
#[must_use]
pub fn halter_kette_aus(journal: &str) -> &'static str {
    let hat = |p: &str| journal.lines().any(|z| z.starts_with(p));
    if hat("halter.kette_rechte:") {
        "rechte"
    } else if hat("halter.kette_fehler:") {
        "fehler"
    } else {
        OK
    }
}
/// The value of `schluessel` in the answer of `systemctl show` (lines `key=value`); `None` when the line is missing.
#[must_use]
pub fn wert<'a>(zeigen: &'a str, schluessel: &str) -> Option<&'a str> {
    zeigen
        .lines()
        .find_map(|z| z.strip_prefix(schluessel).and_then(|r| r.strip_prefix('=')))
}

/// The state of a unit after the answer of `systemctl show` (`None`: the command failed).
#[must_use]
pub fn dienst_aus(zeigen: Option<&str>) -> &'static str {
    let Some(z) = zeigen else {
        return "nicht_pruefbar";
    };
    match (
        wert(z, "LoadState").unwrap_or(""),
        wert(z, "ActiveState").unwrap_or(""),
    ) {
        ("not-found", _) => "fehlt",
        (_, "active") => OK,
        (_, "failed") => "fehlgeschlagen",
        ("loaded" | "masked" | "error", _) => "inaktiv",
        _ => "nicht_pruefbar",
    }
}

/// `true` for the id of a run of a unit as systemd writes it: 32 lowercase hex digits.
#[must_use]
pub fn aufruf_kennung(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// The state "restart needed" of all units from the state of each (see `Umgebung::veraltet`): `noetig` when one runs an older
/// program, else `nicht_pruefbar` when one cannot be told, else `ok`.
#[must_use]
pub fn neustart_aus(einheiten: &[Result<Option<bool>, ()>]) -> &'static str {
    if einheiten.contains(&Ok(Some(true))) {
        "noetig"
    } else if einheiten.iter().any(Result::is_err) {
        "nicht_pruefbar"
    } else {
        OK
    }
}

/// The program of each unit of [`DIENSTE`], relative to the root (the `ExecStart` of the unit files of the package).
pub const PROGRAMME: [&str; 3] = [
    "usr/lib/werkbank/werkbank-kern",
    "usr/lib/werkbank/werkbank-bruecke",
    "usr/lib/werkbank/werkbank-halter",
];
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

/// Owner, group, mode and kind of a file, looked at without following a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kennwerte {
    /// Owner (uid).
    pub uid: u32,
    /// Group (gid).
    pub gid: u32,
    /// The mode bits `0o7777` (with setuid, setgid and sticky).
    pub modus: u32,
    /// `true` for an ordinary file.
    pub datei: bool,
}

/// Starts programs and looks at files whose owner a test cannot make. A test replaces it.
pub trait System {
    /// Runs `programm` with `args` and waits at most `frist`. `None` when it did not start or did not end in time.
    fn befehl(&self, programm: &str, args: &[&str], frist: Duration) -> Option<Ausgabe>;

    /// Owner, group, mode and kind of `pfad` without following a link: `Ok(None)` when nothing is there, `Err` when the
    /// look fails. A test stands in for a file of root with setgid, which it cannot make itself.
    ///
    /// # Errors
    /// The error of `lstat` other than "not found".
    fn kennwerte(&self, pfad: &Path) -> std::io::Result<Option<Kennwerte>> {
        match fs::symlink_metadata(pfad) {
            Ok(m) => Ok(Some(Kennwerte {
                uid: m.uid(),
                gid: m.gid(),
                modus: m.mode() & 0o7777,
                datei: m.file_type().is_file(),
            })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
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
            // Past the limit (no comparison operator: the moment of equality is not observable).
            if frist.checked_sub(start.elapsed()).is_none() {
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

/// The role group of the bridge (addendum "Rolle aus der Gruppe").
pub const ROLLENGRUPPE: &str = "wb-bruecke";
/// The role program of the bridge, relative to the root.
pub const ROLLENPROGRAMM: &str = "usr/lib/werkbank/werkbank-modellweg";
/// The largest `/etc/group`, `/etc/passwd` or `/etc/gshadow` that is read for the role group, in bytes.
pub const KONTEN_GRENZE: usize = 1 << 24;

/// The fields of the lines of a file in the form of `/etc/group` with `n` fields; other lines do not count.
fn zeilen(text: &str, n: usize) -> impl Iterator<Item = Vec<&str>> {
    text.split('\n')
        .filter(|z| !z.is_empty())
        .map(|z| z.split(':').collect::<Vec<_>>())
        .filter(move |f| f.len() == n)
}

/// The state of the role group from the texts of `/etc/group`, `/etc/passwd` and `/etc/gshadow` (`None`: not there):
/// `fehlt` without an entry; `mitglied` for a second entry, a gid that is no number or 0, a member, a password in
/// `/etc/group` (not `x`, `!`, `*` or empty), an account of `/etc/passwd` with it as primary group, or a password, an
/// administrator or a member in `/etc/gshadow`; else `ok` with the gid.
#[must_use]
pub fn rollengruppe_zustand(
    group: &str,
    passwd: &str,
    gshadow: Option<&str>,
) -> (&'static str, Option<u32>) {
    let mut treffer = zeilen(group, 4).filter(|f| f[0] == ROLLENGRUPPE);
    let Some(f) = treffer.next() else {
        return ("fehlt", None);
    };
    if treffer.next().is_some() {
        return ("mitglied", None);
    }
    let Some(gid) = Some(f[2])
        .filter(|g| !g.is_empty() && g.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|g| g.parse::<u32>().ok())
        .filter(|g| *g != 0)
    else {
        return ("mitglied", None);
    };
    let hauptgruppe = zeilen(passwd, 7).any(|p| p[3].parse::<u32>().ok() == Some(gid));
    let schatten = gshadow.is_some_and(|s| {
        zeilen(s, 4).filter(|g| g[0] == ROLLENGRUPPE).any(|g| {
            !(g[1].is_empty() || g[1].starts_with('!') || g[1].starts_with('*'))
                || !g[2].is_empty()
                || !g[3].is_empty()
        })
    });
    if !f[3].is_empty() || !["x", "!", "*", ""].contains(&f[1]) || hauptgruppe || schatten {
        return ("mitglied", Some(gid));
    }
    (OK, Some(gid))
}

/// The state of the role program from its look (`Ok(None)`: nothing there) and the gid of the role group (`None`: the
/// group is missing or broken): `fehlt`; `modus` unless an ordinary file of uid 0 with the group of the role group and
/// exactly the mode 2755; `nicht_pruefbar` when the look failed.
#[must_use]
pub fn rollenprogramm_zustand(
    blick: &std::io::Result<Option<Kennwerte>>,
    gid: Option<u32>,
) -> &'static str {
    match blick {
        Err(_) => "nicht_pruefbar",
        Ok(None) => "fehlt",
        Ok(Some(k)) if k.datei && k.uid == 0 && Some(k.gid) == gid && k.modus == 0o2755 => OK,
        Ok(Some(_)) => "modus",
    }
}

/// The levels of the store of the holder below the home (contract rust/spec/halter, section 2.3).
pub const ABLAGE: [&str; 5] = [".local", "state", "agent-workbench", "halter", "geheim"];

/// Reads at most `grenze` + 1 bytes of `pfad`, but only if the open file is still the one `vorher` describes (same device
/// and inode): a link or another file put in its place between the look and the read is refused.
fn datei_ohne_tausch(
    pfad: &Path,
    vorher: &fs::Metadata,
    grenze: usize,
) -> std::io::Result<Vec<u8>> {
    let datei = fs::File::open(pfad)?;
    let jetzt = datei.metadata()?;
    if jetzt.dev() != vorher.dev() || jetzt.ino() != vorher.ino() {
        return Err(std::io::Error::other("replaced"));
    }
    let mut puffer = Vec::new();
    datei
        .take(u64::try_from(grenze).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut puffer)?;
    Ok(puffer)
}

/// The finding on the sign-in file from its look (kind, owner, mode with the type bits, size) for the user `uid`; `None`
/// when it may be read. Separate from the file system, so a test can give it another owner: without root no test can
/// put a file of another owner into folders of the user.
fn anmeldedatei_befund(
    ist_datei: bool,
    eigentuemer: u32,
    modus: u32,
    laenge: u64,
    uid: u32,
) -> Option<&'static str> {
    if !ist_datei {
        Some("art")
    } else if eigentuemer != uid {
        Some("eigentuemer")
    } else if modus & 0o7777 != 0o600 {
        Some("modus")
    } else if laenge == 0 {
        Some("leer")
    } else {
        None
    }
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

    /// The answer of `systemctl show` for `einheit`: load and active state, main process, start of the main process (seconds since
    /// the epoch, `--timestamp=unix`) and the id of the latest run. As root the user manager of the user is asked by name; a user without
    /// root asks its own manager. `None` without a user or when the command fails.
    fn zeigen(&self, einheit: &str) -> Option<String> {
        let nutzer = self.nutzer.as_deref()?;
        let ziel = format!("--machine={nutzer}@");
        let mut args = vec!["--user"];
        if self.root {
            args.push(&ziel);
        }
        args.extend([
            "show",
            "--no-pager",
            "--timestamp=unix",
            "-p",
            "LoadState",
            "-p",
            "ActiveState",
            "-p",
            "MainPID",
            "-p",
            "ExecMainStartTimestamp",
            "-p",
            "InvocationID",
            "--",
            einheit,
        ]);
        let a = self.system.befehl("systemctl", &args, FRIST)?;
        (a.rc == Some(0)).then_some(a.stdout)
    }

    /// State "restart needed" of one unit (decision of the operator, 28.11.2026): `None` when no main process runs, `Some(true)` for a
    /// process of an older program, `Some(false)` for one of the program file, `Err(())` when it cannot be told. As root the link
    /// `/proc/<MainPID>/exe` decides (it ends with ` (deleted)`, or device and inode differ from the file); without root the start
    /// of the main process against the ctime of the file (a start in an earlier second is an older program).
    fn veraltet(&self, zeigen: &str, programm: &str) -> Result<Option<bool>, ()> {
        if wert(zeigen, "ActiveState") != Some("active") {
            return Ok(None);
        }
        let pid = wert(zeigen, "MainPID")
            .and_then(|p| p.parse::<u32>().ok())
            .ok_or(())?;
        if pid == 0 {
            return Ok(None);
        }
        let datei = fs::metadata(self.pfad(programm)).map_err(|_| ())?;
        if self.root {
            let exe = self.pfad(&format!("proc/{pid}/exe"));
            let ziel = fs::read_link(&exe).map_err(|_| ())?;
            if ziel.to_string_lossy().ends_with(" (deleted)") {
                return Ok(Some(true));
            }
            let laeuft = fs::metadata(&exe).map_err(|_| ())?;
            Ok(Some(
                (laeuft.dev(), laeuft.ino()) != (datei.dev(), datei.ino()),
            ))
        } else {
            let start = wert(zeigen, "ExecMainStartTimestamp")
                .and_then(|s| s.strip_prefix('@'))
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or(())?;
            Ok(Some(start < datei.ctime()))
        }
    }

    /// State of the chain of the holder after the journal of its latest run (decision of the operator, 28.11.2026): the user unit
    /// `wb-halter.service` of the uid and the `InvocationID` of `zeigen` only, the message text only (`-o cat`), read with
    /// `journalctl`. No uid, no id of 32 hex digits or a failed read gives `nicht_pruefbar`; there is no read of the whole boot.
    fn halter_kette(&self, uid: Option<u32>, zeigen: Option<&str>) -> &'static str {
        let (Some(uid), Some(id)) = (uid, zeigen.and_then(|z| wert(z, "InvocationID"))) else {
            return "nicht_pruefbar";
        };
        if !aufruf_kennung(id) {
            return "nicht_pruefbar";
        }
        let nutzer = format!("_UID={uid}");
        let lauf = format!("_SYSTEMD_INVOCATION_ID={id}");
        let args = [
            "--no-pager",
            "-q",
            "-o",
            "cat",
            "-n",
            JOURNAL_ZEILEN,
            "_SYSTEMD_USER_UNIT=wb-halter.service",
            nutzer.as_str(),
            lauf.as_str(),
        ];
        match self.system.befehl("journalctl", &args, FRIST) {
            Some(a) if a.rc == Some(0) => halter_kette_aus(&a.stdout),
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

    /// The home of the user: field 6 of its entry in `etc/passwd` below the root, an absolute path (never `HOME`; contract
    /// rust/spec/halter, section 2.3). `None` without a user or without such an entry.
    fn heim(&self) -> Option<String> {
        let nutzer = self.nutzer.as_deref()?;
        let text = fs::read_to_string(self.pfad("etc/passwd")).ok()?;
        text.lines().find_map(|z| {
            let f: Vec<&str> = z.split(':').collect();
            (f.len() >= 7 && f[0] == nutzer && f[5].starts_with('/')).then(|| f[5].to_owned())
        })
    }

    /// The sign-in file of the route `copilot`, `anmeldung.copilot` in the store of the holder below the home (addendum
    /// holder): first the levels `.local`, `.local/state`, `agent-workbench`, `halter` and `geheim`, each looked at without
    /// following a link (`ablage`: a link, no folder, not the user's, writable by others, writable by a group that is not the private group of
    /// the user after [`Umgebung::private_gruppe_von`] (`etc/nsswitch.conf`, `etc/passwd` and `etc/group` below the root, decision of the operator, 28.11.2026), or
    /// `geheim` not exactly 0700; a missing level is `fehlt`), then kind, owner, mode, emptiness and form of the file (the form of the holder,
    /// finding F11: a value of 1 to 12288 printable ASCII characters without a space and exactly one final line feed). Nothing is made. The content never
    /// leaves this function.
    fn copilot(&self, uid: Option<u32>) -> &'static str {
        let (Some(heim), Some(uid)) = (self.heim(), uid) else {
            return "nicht_pruefbar";
        };
        let gruppe = self.private_gruppe_von(uid);
        let mut ort = self.pfad(heim.trim_start_matches('/'));
        for (i, ebene) in ABLAGE.iter().enumerate() {
            ort.push(ebene);
            match mode_von(&ort) {
                None => return "fehlt",
                Some(m)
                    if !m.file_type().is_dir()
                        || m.uid() != uid
                        || m.mode() & 0o002 != 0
                        || (m.mode() & 0o020 != 0 && gruppe != Some(m.gid()))
                        || (i + 1 == ABLAGE.len() && m.mode() & 0o7777 != 0o700) =>
                {
                    return "ablage";
                }
                Some(_) => {}
            }
        }
        let pfad = ort.join("anmeldung.copilot");
        let Some(m) = mode_von(&pfad) else {
            return "fehlt";
        };
        if let Some(befund) =
            anmeldedatei_befund(m.file_type().is_file(), m.uid(), m.mode(), m.len(), uid)
        {
            return befund;
        }
        match datei_ohne_tausch(&pfad, &m, COPILOT_MAX + 1) {
            Ok(b) if copilot_form(&b) => OK,
            Ok(_) => "form",
            Err(_) => "nicht_pruefbar",
        }
    }

    /// The private group of `uid` (decision of the operator, 28.11.2026, and its addendum 1): only when
    /// `etc/nsswitch.conf` below the root takes accounts and groups from the files and systemd alone
    /// ([`nss_nur_dateien`]), then after [`private_gruppe`] from `etc/passwd` and `etc/group`; a file that cannot be read
    /// gives `None`, so no group write.
    fn private_gruppe_von(&self, uid: u32) -> Option<u32> {
        self.konten_text("etc/nsswitch.conf")
            .filter(|n| nss_nur_dateien(n))
            .and_then(|_| self.konten_text("etc/passwd"))
            .zip(self.konten_text("etc/group"))
            .and_then(|(p, g)| private_gruppe(&p, &g, uid))
    }

    /// The levels above `geheim` that are writable for a group that is not the private group of the user (addendum 1 to
    /// the decision of the operator, 28.11.2026), as absolute paths of the machine in the order of the chain, for the
    /// notes with the remedy `chmod g-w`: folders of the user, not writable by others, with g+w and another gid. Only when
    /// the check `copilot` says `ablage`; the walk never follows a link and stops at a level that is no folder.
    fn copilot_gruppe(&self, uid: Option<u32>, zustand: &str) -> Vec<String> {
        let (Some(heim), Some(uid), "ablage") = (self.heim(), uid, zustand) else {
            return Vec::new();
        };
        let gruppe = self.private_gruppe_von(uid);
        let mut ort = self.pfad(heim.trim_start_matches('/'));
        let mut echt = PathBuf::from(&heim);
        let mut treffer = Vec::new();
        for ebene in &ABLAGE[..4] {
            ort.push(ebene);
            echt.push(ebene);
            match mode_von(&ort) {
                Some(m) if m.file_type().is_dir() => {
                    if m.uid() == uid
                        && m.mode() & 0o002 == 0
                        && m.mode() & 0o020 != 0
                        && gruppe != Some(m.gid())
                    {
                        treffer.push(echt.to_string_lossy().into_owned());
                    }
                }
                _ => break,
            }
        }
        treffer
    }

    /// A file in the form of `/etc/passwd` or `/etc/group` below the root as text: `None` if it cannot be read, is
    /// larger than [`KONTEN_GRENZE`] or is no UTF-8.
    fn konten_text(&self, rel: &str) -> Option<String> {
        lesen_bis(&self.pfad(rel), KONTEN_GRENZE)
            .ok()
            .filter(|b| b.len() <= KONTEN_GRENZE)
            .and_then(|b| String::from_utf8(b).ok())
    }

    /// The role group and the role program (addendum "Rolle aus der Gruppe"): `(rollengruppe, rollenprogramm)`.
    /// `/etc/gshadow` counts only when it is there; there but not readable (without root) is `nicht_pruefbar`.
    fn rolle(&self) -> (&'static str, &'static str) {
        let lesen = |rel: &str| self.konten_text(rel);
        let gshadow = match mode_von(&self.pfad("etc/gshadow")) {
            None => Ok(None),
            Some(_) => lesen("etc/gshadow").map(Some).ok_or(()),
        };
        let (gruppe, gid) = match (lesen("etc/group"), lesen("etc/passwd"), &gshadow) {
            (Some(g), Some(p), Ok(s)) => rollengruppe_zustand(&g, &p, s.as_deref()),
            _ => ("nicht_pruefbar", None),
        };
        // the gid of the program is compared even when the group has a member: only its form decides the program
        let gid = gid.or_else(|| {
            lesen("etc/group").and_then(|g| {
                let mut t = zeilen(&g, 4).filter(|f| f[0] == ROLLENGRUPPE);
                match (t.next(), t.next()) {
                    (Some(f), None) => f[2].parse::<u32>().ok().filter(|g| *g != 0),
                    _ => None,
                }
            })
        });
        let blick = self.system.kennwerte(&self.pfad(ROLLENPROGRAMM));
        (gruppe, rollenprogramm_zustand(&blick, gid))
    }

    /// `true` when the file of the former target `/var/lib/werkbank/<user>/copilot.schluessel` is still there, in any state
    /// (it is never read).
    fn copilot_alt(&self) -> bool {
        self.nutzer_ordner()
            .is_some_and(|o| mode_von(&self.pfad(&format!("{o}/copilot.schluessel"))).is_some())
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
        let (rollengruppe, rollenprogramm) = self.rolle();
        let copilot = self.copilot(uid);
        let zeigen: Vec<Option<String>> = DIENSTE.iter().map(|n| self.zeigen(n)).collect();
        let neustart: Vec<Result<Option<bool>, ()>> = zeigen
            .iter()
            .zip(PROGRAMME)
            .map(|(z, p)| z.as_deref().map_or(Err(()), |z| self.veraltet(z, p)))
            .collect();
        Lage {
            konten: self.konten(),
            dienste: DIENSTE
                .iter()
                .zip(&zeigen)
                .map(|(n, z)| ((*n).to_owned(), dienst_aus(z.as_deref()).to_owned()))
                .collect(),
            rollengruppe: rollengruppe.to_owned(),
            rollenprogramm: rollenprogramm.to_owned(),
            halter_kette: self.halter_kette(uid, zeigen[2].as_deref()).to_owned(),
            neustart: neustart_aus(&neustart).to_owned(),
            apparmor: apparmor.0.to_owned(),
            system_bwrap: if apparmor.1 { "ubuntu" } else { "gesperrt" }.to_owned(),
            einstellungen: self.einstellungen(),
            nutzerordner: self.nutzerordner(uid).to_owned(),
            schluessel: schluessel.to_owned(),
            copilot: copilot.to_owned(),
            copilot_gruppe: self.copilot_gruppe(uid, copilot),
            copilot_alt: self.copilot_alt(),
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

/// The longest value of the sign-in file in bytes (contract rust/spec/halter, section 2.3; the file holds one LF more).
pub const COPILOT_MAX: usize = 12_288;

/// The form of the sign-in file, the same as the holder reads (finding F11): a value of 1 to [`COPILOT_MAX`] characters
/// from `!` to `~` and exactly one final LF.
#[must_use]
pub fn copilot_form(bytes: &[u8]) -> bool {
    let Some(zeile) = bytes.strip_suffix(b"\n") else {
        return false;
    };
    (1..=COPILOT_MAX).contains(&zeile.len()) && zeile.iter().all(|b| (0x21..=0x7e).contains(b))
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

#[cfg(test)]
mod tests {
    use super::{anmeldedatei_befund, datei_ohne_tausch};
    use std::fs;

    #[test]
    fn der_befund_der_anmeldedatei_folgt_art_eigentuemer_modus_und_groesse() {
        let datei = 0o100_600;
        assert_eq!(
            anmeldedatei_befund(false, 1000, datei, 5, 1000),
            Some("art")
        );
        assert_eq!(
            anmeldedatei_befund(true, 1001, datei, 5, 1000),
            Some("eigentuemer")
        );
        assert_eq!(
            anmeldedatei_befund(true, 1000, 0o100_644, 5, 1000),
            Some("modus")
        );
        assert_eq!(
            anmeldedatei_befund(true, 1000, datei, 0, 1000),
            Some("leer")
        );
        assert_eq!(anmeldedatei_befund(true, 1000, datei, 5, 1000), None);
    }

    /// The read refuses a file that is not the one looked at, even on the same device.
    #[test]
    fn eine_andere_datei_als_die_angesehene_wird_nicht_gelesen() {
        let o = std::env::temp_dir().join(format!("wb-pruefen-tausch-{}", std::process::id()));
        fs::create_dir_all(&o).expect("folder");
        let (a, b) = (o.join("a"), o.join("b"));
        fs::write(&a, b"eins").expect("a");
        fs::write(&b, b"zwei").expect("b");
        let vorher = fs::symlink_metadata(&a).expect("look at a");
        assert_eq!(datei_ohne_tausch(&a, &vorher, 100).expect("a"), b"eins");
        assert!(datei_ohne_tausch(&b, &vorher, 100).is_err());
        fs::remove_dir_all(&o).ok();
    }
}
