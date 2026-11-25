//! Step 6a (contract 1, "Paket der Copilot CLI"): the extracted package of the cloud harness, before the first child.
//!
//! The CLI extracts its bundled package at every start into `<home>/.cache/copilot/pkg/<platform>/<version>` unless the
//! folder is complete; inside the enclosure the home is a tmpfs of 16 MiB and the start fails with `ENOSPC` (measured
//! with 1.0.88). "Lauf starten" therefore
//!
//! 1. reads platform and version from the installed packages, without running a foreign program ([`form_lesen`]);
//! 2. keeps one cache folder per harness in the real home of the user, `($XDG_CACHE_HOME or <heim>/.cache)/werkbank/
//!    harness/copilot`, owned by the user, mode 0700, apart from every login store ([`cache_ordner`]);
//! 3. under an exclusive `flock` on [`SPERR_DATEI`] in that folder checks `<cache>/pkg/<platform>/<version>` on the host
//!    ([`einschluss::paket_pruefen`]); a missing folder is extracted, an incomplete one is first renamed aside through
//!    descriptors (never removed), then extracted; a link or a foreign owner is a fixed code and nothing is bound;
//! 4. extracts only in the small enclosure of [`einschluss::Vorentpacken`] (`<harness.copilot> --version`, no network,
//!    no token), and checks the folder again afterwards.
//!
//! The folder is then bound read-only into the enclosure of the session ([`einschluss::Paket`]).

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use einstellungen::json::{Wert, lesen};
use rustix::fs::{AtFlags, FileType, FlockOperation, Mode, OFlags};

use crate::fehler::Fehler;

/// The lock file in the cache folder: one extraction at a time per user and harness.
pub const SPERR_DATEI: &str = ".werkbank.lock";

/// The folder in the cache folder that takes incomplete package folders (renamed, never removed).
pub const BEISEITE: &str = ".werkbank-beiseite";

/// The longest wait for the lock of another start.
pub const SPERR_FRIST: Duration = Duration::from_secs(180);

/// The longest extraction (measured 3.93 s in the small enclosure; the limit is a guard against a hang).
pub const ENTPACK_FRIST: Duration = Duration::from_secs(120);

/// The largest `package.json` that is read.
const PAKET_JSON_MAX: u64 = 1 << 20;

/// How many bytes of the standard error of the extraction are looked at (the end is kept).
const MELDUNG_MAX: usize = 65_536;

/// Platform and version of the installed package of the cloud harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// `linux-x64` or `linux-arm64`.
    pub plattform: String,
    /// `<number>.<number>.<number>`, from the package, never fixed in the program.
    pub fassung: String,
}

fn feld<'a>(objekt: &'a [(String, Wert)], name: &str) -> Option<&'a Wert> {
    objekt.iter().find(|(n, _)| n == name).map(|(_, w)| w)
}

fn text_feld<'a>(objekt: &'a [(String, Wert)], name: &str) -> Option<&'a str> {
    match feld(objekt, name)? {
        Wert::Text(t) => Some(t),
        _ => None,
    }
}

fn liste_enthaelt(objekt: &[(String, Wert)], name: &str, wert: &str) -> bool {
    matches!(feld(objekt, name), Some(Wert::Liste(l)) if l.iter().any(|w| *w == Wert::Text(wert.to_owned())))
}

/// Reads `<ordner>/package.json` as a JSON object: a regular file (no link), at most 1 MiB.
fn paket_json(ordner: &Path) -> Option<Vec<(String, Wert)>> {
    let datei = ordner.join("package.json");
    let meta = std::fs::symlink_metadata(&datei).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut text = Vec::new();
    std::fs::File::open(&datei)
        .ok()?
        .take(PAKET_JSON_MAX + 1)
        .read_to_end(&mut text)
        .ok()?;
    if u64::try_from(text.len()).ok()? > PAKET_JSON_MAX {
        return None;
    }
    match lesen(&text).ok()? {
        Wert::Objekt(o) => Some(o),
        _ => None,
    }
}

/// The architecture of the platform package for the machine (`x86_64` is `x64`, `aarch64` is `arm64`), or `None` for
/// any other machine and for a build without glibc (musl is not supported).
#[must_use]
pub fn architektur(maschine: &str, glibc: bool) -> Option<&'static str> {
    if !glibc {
        return None;
    }
    match maschine {
        "x86_64" => Some("x64"),
        "aarch64" => Some("arm64"),
        _ => None,
    }
}

/// The architecture of this build.
#[must_use]
pub fn eigene_architektur() -> Option<&'static str> {
    architektur(std::env::consts::ARCH, cfg!(target_env = "gnu"))
}

/// The two places of the platform package, in the order of the resolution of Node from the folder `a` of the main
/// package: (1) nested, `a/node_modules/@github/copilot-<platform>`; (2) beside, `<scope>/copilot-<platform>`, only
/// when `a` is `.../node_modules/@github/<name>`.
#[must_use]
pub fn plattform_orte(a: &Path, plattform: &str) -> Vec<PathBuf> {
    let name = format!("copilot-{plattform}");
    let mut orte = vec![a.join("node_modules").join("@github").join(&name)];
    if let Some(scope) = a.parent()
        && scope.file_name().is_some_and(|n| n == "@github")
        && scope
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "node_modules")
    {
        orte.push(scope.join(&name));
    }
    orte
}

/// Step 6a, first part: platform and version from the packages next to the program `programm` (`harness.copilot`),
/// read as files; no program is run. The main package `A/package.json` (`A` the folder of the real path of the program):
/// `name` is `@github/copilot`, `version` has the form `<n>.<n>.<n>`, `bin.copilot` is the file name of the program. The
/// platform package at the first existing place of [`plattform_orte`]: `name` is `@github/copilot-<platform>`, the same
/// `version`, `os` holds `linux`, `cpu` holds the architecture, `bin["copilot-<platform>"]` is `copilot`, and the file
/// `copilot` beside it is a regular file (no link).
///
/// # Errors
/// `lauf.paket_form` for every other form, a file that cannot be read and a machine without a known architecture.
pub fn form_lesen(programm: &Path, arch: Option<&str>) -> Result<Form, Fehler> {
    let falsch = || Fehler::PaketForm;
    let arch = arch.ok_or_else(falsch)?;
    let echt = std::fs::canonicalize(programm).map_err(|_| falsch())?;
    let (Some(a), Some(datei)) = (echt.parent(), echt.file_name().and_then(|n| n.to_str())) else {
        return Err(falsch());
    };
    let haupt = paket_json(a).ok_or_else(falsch)?;
    let fassung = text_feld(&haupt, "version").ok_or_else(falsch)?;
    let bin_gut = matches!(feld(&haupt, "bin"), Some(Wert::Objekt(b)) if text_feld(b, "copilot") == Some(datei));
    if text_feld(&haupt, "name") != Some("@github/copilot")
        || !einschluss::fassung_gut(fassung)
        || !bin_gut
    {
        return Err(falsch());
    }
    let plattform = format!("linux-{arch}");
    let ort = plattform_orte(a, &plattform)
        .into_iter()
        .find(|o| std::fs::symlink_metadata(o).is_ok_and(|m| m.is_dir()))
        .ok_or_else(falsch)?;
    let p = paket_json(&ort).ok_or_else(falsch)?;
    let schluessel = format!("copilot-{plattform}");
    let p_bin_gut = matches!(feld(&p, "bin"), Some(Wert::Objekt(b)) if text_feld(b, &schluessel) == Some("copilot"));
    let programm_gut =
        std::fs::symlink_metadata(ort.join("copilot")).is_ok_and(|m| m.file_type().is_file());
    if text_feld(&p, "name") != Some(format!("@github/{schluessel}").as_str())
        || text_feld(&p, "version") != Some(fassung)
        || !liste_enthaelt(&p, "os", "linux")
        || !liste_enthaelt(&p, "cpu", arch)
        || !p_bin_gut
        || !programm_gut
    {
        return Err(falsch());
    }
    Ok(Form {
        plattform,
        fassung: fassung.to_owned(),
    })
}

/// The cache folder of the harness: `<xdg>/werkbank/harness/copilot` when `$XDG_CACHE_HOME` of the program is an
/// absolute, normal path strictly inside the home, else `<heim>/.cache/werkbank/harness/copilot`.
#[must_use]
pub fn cache_ordner(heim: &Path, xdg: Option<&OsString>) -> PathBuf {
    let basis = xdg
        .and_then(|x| x.to_str())
        .filter(|x| crate::ordner_form(x))
        .map(PathBuf::from)
        .filter(|x| x.starts_with(heim) && x != heim)
        .unwrap_or_else(|| heim.join(".cache"));
    basis.join("werkbank").join("harness").join("copilot")
}

/// Why the extraction did not give a complete folder: the exit code of the extraction (`None` for a signal, a fault of
/// the start or the time limit) and a fixed word.
fn entpack_fehler(rc: Option<i32>, wort: &'static str) -> Fehler {
    Fehler::PaketEntpacken { rc, wort }
}

/// How the extraction ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ende {
    /// The exit code, `None` for a signal.
    pub rc: Option<i32>,
    /// Standard error named `ENOSPC` (the place was full).
    pub kein_platz: bool,
    /// The starter refused with its own line (`lauf.einschluss_fehlt`).
    pub starter: bool,
}

/// Runs the extraction `befehl` with an empty environment, closed standard input and output, waits at most `frist`
/// and keeps the end of standard error only to name the cause with a fixed word; no byte of it is written anywhere.
///
/// # Errors
/// `lauf.paket_entpacken` when the program cannot be started or the time is up (the child is ended by its PID).
pub fn lauf_messen(mut befehl: std::process::Command, frist: Duration) -> Result<Ende, Fehler> {
    use std::process::Stdio;
    let mut kind = befehl
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| entpack_fehler(None, "start"))?;
    let strom = kind
        .stderr
        .take()
        .ok_or_else(|| entpack_fehler(None, "start"))?;
    let fd = std::os::fd::OwnedFd::from(strom);
    let flags = rustix::fs::fcntl_getfl(&fd).map_err(|_| entpack_fehler(None, "start"))?;
    rustix::fs::fcntl_setfl(&fd, flags.union(OFlags::NONBLOCK))
        .map_err(|_| entpack_fehler(None, "start"))?;
    let mut meldung: Vec<u8> = Vec::new();
    let mut puffer = [0_u8; 4096];
    let ende = Instant::now() + frist;
    let mut lesen_bis_leer = |meldung: &mut Vec<u8>| {
        while let Ok(n) = rustix::io::read(&fd, &mut puffer) {
            if n == 0 {
                break;
            }
            meldung.extend_from_slice(&puffer[..n]);
            // keep the end only: what lies before the last `MELDUNG_MAX` bytes goes
            if let Some(weg) = meldung.len().checked_sub(MELDUNG_MAX) {
                meldung.drain(..weg);
            }
        }
    };
    let status = loop {
        lesen_bis_leer(&mut meldung);
        match kind.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if !ende.saturating_duration_since(Instant::now()).is_zero() => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => {
                // the own child, by its PID
                let _ = kind.kill();
                let _ = kind.wait();
                return Err(entpack_fehler(None, "frist"));
            }
        }
    };
    lesen_bis_leer(&mut meldung);
    let text = String::from_utf8_lossy(&meldung);
    Ok(Ende {
        rc: status.code(),
        kein_platz: text.contains("ENOSPC"),
        starter: text.starts_with("lauf.einschluss_fehlt"),
    })
}

fn eigen() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// Opens the folder `name` below `eltern`: never through a link, only a folder of the own user.
fn ordner_at(eltern: &rustix::fd::OwnedFd, name: &str) -> Result<rustix::fd::OwnedFd, Fehler> {
    ordner_at_mit(eltern, name, || {})
}

/// [`ordner_at`] with a seam: `zwischen` runs right before the `openat`, the place where a swap of the entry must not
/// change the result. The product passes an empty seam.
fn ordner_at_mit(
    eltern: &rustix::fd::OwnedFd,
    name: &str,
    zwischen: impl FnOnce(),
) -> Result<rustix::fd::OwnedFd, Fehler> {
    zwischen();
    let fd = rustix::fs::openat(
        eltern,
        name,
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )
    .map_err(|_| Fehler::PaketCache("beiseite"))?;
    let st = rustix::fs::fstat(&fd).map_err(|_| Fehler::PaketCache("beiseite"))?;
    if st.st_uid != eigen() {
        return Err(Fehler::PaketCache("fremd"));
    }
    Ok(fd)
}

/// Renames the incomplete folder `<cache>/pkg/<platform>/<version>` aside to `<cache>/.werkbank-beiseite/<platform>-
/// <version>-<nanoseconds>`, through descriptors only: every folder on the way is opened with `O_NOFOLLOW` and must
/// belong to the own user, and the entry itself is looked at without following a link. Nothing is removed.
///
/// # Errors
/// `lauf.paket_cache` with the word `beiseite` or `fremd`.
pub fn beiseite(cache: &rustix::fd::OwnedFd, form: &Form) -> Result<String, Fehler> {
    let pkg = ordner_at(cache, "pkg")?;
    let platz = ordner_at(&pkg, &form.plattform)?;
    let st = rustix::fs::statat(&platz, form.fassung.as_str(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| Fehler::PaketCache("beiseite"))?;
    if st.st_uid != eigen() {
        return Err(Fehler::PaketCache("fremd"));
    }
    match rustix::fs::mkdirat(cache, BEISEITE, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(_) => return Err(Fehler::PaketCache("beiseite")),
    }
    let ziel = ordner_at(cache, BEISEITE)?;
    let zeit = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Fehler::PaketCache("beiseite"))?
        .as_nanos();
    let name = format!("{}-{}-{zeit}", form.plattform, form.fassung);
    rustix::fs::renameat(&platz, form.fassung.as_str(), &ziel, name.as_str())
        .map_err(|_| Fehler::PaketCache("beiseite"))?;
    Ok(name)
}

/// Makes the cache folder when it is missing (each missing folder with mode 0700) and checks it: a real path without
/// links, a folder of the own user with mode exactly 0700, no secret segment and no login store of the account.
///
/// # Errors
/// `lauf.paket_cache` with the word `anmeldespeicher`, `anlegen`, `verweis` or `fremd`.
pub fn cache_bereit(cache: &Path, heim: &Path) -> Result<rustix::fd::OwnedFd, Fehler> {
    cache_bereit_mit(cache, heim, || {})
}

/// [`cache_bereit`] with a seam: `zwischen` runs between the check with `canonicalize` and the `open`, the place where
/// a swap of the folder must not change the result. The product passes an empty seam.
fn cache_bereit_mit(
    cache: &Path,
    heim: &Path,
    zwischen: impl FnOnce(),
) -> Result<rustix::fd::OwnedFd, Fehler> {
    use std::os::unix::fs::DirBuilderExt;
    if einschluss::verbote::hat_geheimnisteil(cache)
        || einschluss::verbote::beruehrt_anmeldespeicher(cache, heim, einschluss::HARNESS_COPILOT)
    {
        return Err(Fehler::PaketCache("anmeldespeicher"));
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(cache)
        .map_err(|_| Fehler::PaketCache("anlegen"))?;
    if std::fs::canonicalize(cache).map_or(true, |e| e != cache) {
        return Err(Fehler::PaketCache("verweis"));
    }
    zwischen();
    let fd = rustix::fs::open(
        cache,
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )
    .map_err(|_| Fehler::PaketCache("verweis"))?;
    let st = rustix::fs::fstat(&fd).map_err(|_| Fehler::PaketCache("anlegen"))?;
    if st.st_uid != eigen() {
        return Err(Fehler::PaketCache("fremd"));
    }
    if FileType::from_raw_mode(st.st_mode) != FileType::Directory
        || crate::dateien::modus_von(&st) & 0o7777 != 0o700
    {
        return Err(Fehler::PaketCache("modus"));
    }
    Ok(fd)
}

/// Takes the exclusive lock on [`SPERR_DATEI`] in the cache folder (made with mode 0600, never through a link), waiting
/// at most `frist`. The lock ends with the descriptor.
///
/// # Errors
/// `lauf.paket_cache` with the word `sperre`; `lauf.paket_entpacken` with the word `sperre-frist` when the time is up.
pub fn sperren(
    cache: &rustix::fd::OwnedFd,
    frist: Duration,
) -> Result<rustix::fd::OwnedFd, Fehler> {
    let sperre = rustix::fs::openat(
        cache,
        SPERR_DATEI,
        OFlags::RDWR
            .union(OFlags::CREATE)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| Fehler::PaketCache("sperre"))?;
    let ende = Instant::now() + frist;
    loop {
        match rustix::fs::flock(&sperre, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(sperre),
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR)
                if !ende.saturating_duration_since(Instant::now()).is_zero() =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => {
                return Err(entpack_fehler(None, "sperre-frist"));
            }
            Err(_) => return Err(Fehler::PaketCache("sperre")),
        }
    }
}

/// What step 6a did (for the tests and the log of a person; the program writes none of it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tat {
    /// The folder was complete: nothing was extracted.
    Vorhanden,
    /// The folder was missing and is extracted now.
    Entpackt,
    /// The folder was incomplete: renamed aside (the new name), then extracted.
    Erneuert(String),
}

/// Step 6a, second part: the complete package folder `<cache>/pkg/<platform>/<version>` under the lock; `entpacken`
/// runs the small enclosure. Returns the folder to bind and what was done.
///
/// # Errors
/// `lauf.paket_cache` (folder unusable, link, foreign owner) or `lauf.paket_entpacken` (the extraction failed, or the
/// folder is not complete afterwards).
pub fn bereitstellen(
    cache: &Path,
    heim: &Path,
    form: &Form,
    entpacken: impl FnOnce() -> Result<Ende, Fehler>,
) -> Result<(PathBuf, Tat), Fehler> {
    let cache_fd = cache_bereit(cache, heim)?;
    let _sperre = sperren(&cache_fd, SPERR_FRIST)?;
    let ordner = cache.join("pkg").join(&form.plattform).join(&form.fassung);
    let uid = eigen();
    let tat = match einschluss::paket_pruefen(&ordner, &form.plattform, uid) {
        Ok(()) => return Ok((ordner, Tat::Vorhanden)),
        Err(einschluss::PaketMangel::Verweis) => return Err(Fehler::PaketCache("verweis")),
        Err(einschluss::PaketMangel::Fremd) => return Err(Fehler::PaketCache("fremd")),
        Err(einschluss::PaketMangel::Fehlt) => Tat::Entpackt,
        Err(einschluss::PaketMangel::Unvollstaendig) => Tat::Erneuert(beiseite(&cache_fd, form)?),
    };
    let ende = entpacken()?;
    let wort = if ende.starter {
        "starter"
    } else if ende.kein_platz {
        "kein-platz"
    } else {
        "rc"
    };
    if ende.rc != Some(0) {
        return Err(entpack_fehler(ende.rc, wort));
    }
    match einschluss::paket_pruefen(&ordner, &form.plattform, uid) {
        Ok(()) => Ok((ordner, tat)),
        Err(_) => Err(entpack_fehler(ende.rc, "unvollstaendig")),
    }
}

#[cfg(test)]
mod tests;
