//! `werkbank-anmelden copilot --uebernehmen [--neu]` (contract 9, section 3b): takes over the login that the CLI
//! already holds in `config.json` of the caller. The token moves into the key file and leaves `config.json`; no
//! login, no browser, no stdin. Every file is opened, made, linked, renamed and deleted relative to an open folder
//! (`openat`, `linkat`, `renameat`, `unlinkat`, never following a link). No output of its own: the caller prints.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags, Stat};
use rustix::io::Errno;

use crate::fehler::Fehler;
use crate::form::{CONFIG_GRENZE, Fund, Token, ohne_token, token_aus_config, token_form_ok};
/// The store of the holder (step 6), opened and checked level by level; it lives in `lauf`, both ways use it.
pub use crate::lauf::ziel_oeffnen;
use crate::lauf::{
    Erfolg, SCHLUESSELDATEI, TempArt, Umgebung, echtes_heim_pruefen, einfacher_name, ordner_oeffnen,
};

/// Name of the file of the CLI that holds the token.
pub const CONFIG: &str = "config.json";

/// Largest key file that is compared with the token (a token has at most 255 characters).
const SCHLUESSEL_GRENZE: u64 = 4096;

/// The points between the steps of section 3b, for the probes of the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Punkt {
    /// Every check is done, nothing is written yet (the last point where a signal ends the run).
    VorSchreiben,
    /// The temp file of the new `config.json` is written and synced.
    ConfigTemp,
    /// The temp file of the key is written and synced.
    SchluesselTemp,
    /// The key file holds the token (link or exchange done).
    Schluessel,
    /// The temp name of the key is deleted (not after an exchange: it holds the old key until the end).
    TempGeloescht,
    /// `config.json` is replaced.
    Config,
}

impl Punkt {
    /// The point named `name` (`vor-schreiben`, `config-temp`, `schluessel-temp`, `schluessel`, `temp-geloescht`,
    /// `config`).
    #[must_use]
    pub fn aus_name(name: &str) -> Option<Punkt> {
        Some(match name {
            "vor-schreiben" => Punkt::VorSchreiben,
            "config-temp" => Punkt::ConfigTemp,
            "schluessel-temp" => Punkt::SchluesselTemp,
            "schluessel" => Punkt::Schluessel,
            "temp-geloescht" => Punkt::TempGeloescht,
            "config" => Punkt::Config,
            _ => return None,
        })
    }
}

/// A probe of the tests at one point (seam of `main`, only in the test build with `WB_TESTUMGEBUNG=1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// The process kills itself with SIGKILL at the point: a hard end between two steps.
    Toeten(Punkt),
    /// The process writes the mark file and waits (at most 10 seconds) for a signal at the point.
    Warten(Punkt, PathBuf),
    /// The replacing of `config.json` fails as if the system refused it.
    ConfigStoeren,
}

/// What a successful takeover reports.
#[derive(Debug, PartialEq, Eq)]
pub struct Uebernahme {
    /// What the check of the real home reports.
    pub erfolg: Erfolg,
    /// The key file already held the token: an earlier run ended after the key and before `config.json`; this run
    /// only finished `config.json`.
    pub wiederanlauf: bool,
}

/// How the key file got the token, so that a later error can undo it.
enum Zusage {
    /// The key file was already the token (restart): nothing to undo.
    Vorher,
    /// A new name by `linkat`: undo deletes the key file if it is still this inode (stat of the temp file).
    Neu(Stat),
    /// `--neu` over an old key by exchange: the temp name holds the old key; undo exchanges back.
    Getauscht,
}

/// A temp name that is deleted (relative to its folder) when the value goes away. After a rename or an unlink the
/// name is already gone and the delete does nothing; after an exchange it removes the old key.
pub(crate) struct Temp<'a> {
    ordner: &'a OwnedFd,
    pub(crate) name: String,
}

impl Drop for Temp<'_> {
    fn drop(&mut self) {
        let _ = rustix::fs::unlinkat(self.ordner, self.name.as_str(), AtFlags::empty());
    }
}

/// Runs the takeover.
///
/// # Errors
/// Every `Fehler` of section 3b. On `EchteAnmeldung` the key file is stored and `config.json` is without the token.
pub fn uebernehmen(
    umg: &Umgebung,
    neu: bool,
    signal: &AtomicUsize,
    probe: Option<&Probe>,
) -> Result<Uebernahme, Fehler> {
    // Steps 2 to 4: the source, read once through a descriptor.
    let quelle = quelle_oeffnen(&umg.echtes_copilot_heim, umg.uid)?;
    let (config_stat, text) = config_lesen(&quelle, umg.uid)?;
    let Fund::Eins(token) = token_aus_config(&text) else {
        return Err(Fehler::QuelleOhneToken);
    };
    if !token_form_ok(token.as_str()) {
        return Err(Fehler::FormFalsch);
    }
    let neu_text = ohne_token(&text, &token).ok_or(Fehler::QuelleOhneToken)?;
    // Steps 5 and 6: the store of the holder (missing levels are made) and the key file that is there.
    let ziel = ziel_oeffnen(&umg.heim, umg.uid, umg.gruppe)?;
    let vorher = match rustix::fs::statat(&ziel, SCHLUESSELDATEI, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => Vorhanden::Nein,
        Err(_) => return Err(Fehler::ZielNichtSchreibbar),
        Ok(st) if schluessel_ist_token(&ziel, &st, &token, umg.uid) => Vorhanden::Gleich,
        Ok(_) => Vorhanden::Anders,
    };
    if vorher == Vorhanden::Anders && !neu {
        return Err(Fehler::DateiDa);
    }
    stelle(probe, Punkt::VorSchreiben, signal);
    signal_pruefen(signal)?;
    // Step 7: the new config.json as a temp file next to it, synced, not yet in place.
    let config_temp = config_temp_schreiben(&quelle, &config_stat, &neu_text)?;
    stelle(probe, Punkt::ConfigTemp, signal);
    // Step 8: the key file.
    let mut schluessel_temp = None;
    let zusage = if vorher == Vorhanden::Gleich {
        Zusage::Vorher
    } else {
        let (temp, st) = schluessel_temp_schreiben(&ziel, &token)?;
        stelle(probe, Punkt::SchluesselTemp, signal);
        let zusage = schluessel_setzen(&ziel, &temp.name, &st, neu)?;
        stelle(probe, Punkt::Schluessel, signal);
        schluessel_temp = Some(temp);
        zusage
    };
    if let (Zusage::Neu(_), Some(temp)) = (&zusage, schluessel_temp.as_ref()) {
        if rustix::fs::unlinkat(&ziel, temp.name.as_str(), AtFlags::empty()).is_err() {
            zuruecknehmen(&ziel, &zusage, schluessel_temp.as_ref());
            return Err(Fehler::ZielNichtSchreibbar);
        }
        stelle(probe, Punkt::TempGeloescht, signal);
    }
    let _ = rustix::fs::fsync(&ziel);
    // Step 9: config.json in place, only if it is still the file that was read.
    let noch_gleich = rustix::fs::statat(&quelle, CONFIG, AtFlags::SYMLINK_NOFOLLOW)
        .is_ok_and(|st| gleiche_datei(&st, &config_stat));
    let ersetzt = noch_gleich
        && probe != Some(&Probe::ConfigStoeren)
        && rustix::fs::renameat(&quelle, config_temp.name.as_str(), &quelle, CONFIG).is_ok();
    if !ersetzt {
        zuruecknehmen(&ziel, &zusage, schluessel_temp.as_ref());
        return Err(Fehler::QuelleNichtSchreibbar);
    }
    let _ = rustix::fs::fsync(&quelle);
    stelle(probe, Punkt::Config, signal);
    // Step 10: after an exchange the temp name holds the old key; it goes now (the drop deletes it).
    drop(schluessel_temp);
    let _ = rustix::fs::fsync(&ziel);
    // Step 11: the check of the real home (section 4).
    Ok(Uebernahme {
        erfolg: echtes_heim_pruefen(umg)?,
        wiederanlauf: matches!(zusage, Zusage::Vorher),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vorhanden {
    Nein,
    Gleich,
    Anders,
}

fn signal_pruefen(signal: &AtomicUsize) -> Result<(), Fehler> {
    match signal.load(Ordering::SeqCst) {
        0 => Ok(()),
        n => Err(Fehler::Signal(i32::try_from(n).unwrap_or(0))),
    }
}

/// The probe of the tests at `punkt`, if it is this one.
fn stelle(probe: Option<&Probe>, punkt: Punkt, signal: &AtomicUsize) {
    match probe {
        Some(Probe::Toeten(p)) if *p == punkt => {
            let _ = rustix::process::kill_process(
                rustix::process::getpid(),
                rustix::process::Signal::KILL,
            );
            std::process::abort();
        }
        Some(Probe::Warten(p, marke)) if *p == punkt => {
            let _ = std::fs::write(marke, b"da\n");
            let ende = Instant::now() + Duration::from_secs(10);
            while signal.load(Ordering::SeqCst) == 0
                && ende.checked_duration_since(Instant::now()).is_some()
            {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        _ => {}
    }
}

/// The folder of the CLI (`COPILOT_HOME`, else `~/.copilot`): no link at its last part, a folder of `uid`, not
/// writable by group or others. Missing: there is no login to take over.
///
/// # Errors
/// `QuelleOhneToken` when it is missing, `QuelleUnsicher` otherwise.
pub fn quelle_oeffnen(pfad: &std::path::Path, uid: u32) -> Result<OwnedFd, Fehler> {
    let fd = match ordner_oeffnen(pfad) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Err(Fehler::QuelleOhneToken),
        Err(_) => return Err(Fehler::QuelleUnsicher),
    };
    let st = rustix::fs::fstat(&fd).map_err(|_| Fehler::QuelleUnsicher)?;
    if st.st_uid != uid || st.st_mode & 0o022 != 0 {
        return Err(Fehler::QuelleUnsicher);
    }
    Ok(fd)
}

/// `config.json` through a descriptor relative to the open folder: no link, a regular file of `uid` with one name, at
/// most 1 MiB. Returns its stat and its bytes.
///
/// # Errors
/// `QuelleOhneToken` when it is missing, `QuelleUnsicher` otherwise.
pub fn config_lesen(quelle: &OwnedFd, uid: u32) -> Result<(Stat, Vec<u8>), Fehler> {
    let fd = match rustix::fs::openat(
        quelle,
        CONFIG,
        OFlags::RDONLY
            .union(OFlags::NOFOLLOW)
            .union(OFlags::NONBLOCK)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Err(Fehler::QuelleOhneToken),
        Err(_) => return Err(Fehler::QuelleUnsicher),
    };
    let st = rustix::fs::fstat(&fd).map_err(|_| Fehler::QuelleUnsicher)?;
    if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile
        || st.st_uid != uid
        || st.st_nlink != 1
    {
        return Err(Fehler::QuelleUnsicher);
    }
    let mut text = Vec::new();
    File::from(fd)
        .take(CONFIG_GRENZE + 1)
        .read_to_end(&mut text)
        .map_err(|_| Fehler::QuelleUnsicher)?;
    if u64::try_from(text.len()).map_or(true, |n| n > CONFIG_GRENZE) {
        return Err(Fehler::QuelleUnsicher);
    }
    Ok((st, text))
}

/// Is the key file at `st` (the stat of `anmeldung.copilot` in `ziel`, without following a link) exactly the token
/// and LF, a regular file of `uid` with one name and mode 0600?
#[must_use]
pub fn schluessel_ist_token(ziel: &OwnedFd, st: &Stat, token: &Token, uid: u32) -> bool {
    if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile
        || st.st_uid != uid
        || st.st_nlink != 1
        || st.st_mode & 0o7777 != 0o600
    {
        return false;
    }
    let Ok(fd) = rustix::fs::openat(
        ziel,
        SCHLUESSELDATEI,
        OFlags::RDONLY
            .union(OFlags::NOFOLLOW)
            .union(OFlags::NONBLOCK)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    ) else {
        return false;
    };
    if !rustix::fs::fstat(&fd).is_ok_and(|f| gleiche_datei(&f, st)) {
        return false;
    }
    let mut inhalt = Vec::new();
    File::from(fd)
        .take(SCHLUESSEL_GRENZE)
        .read_to_end(&mut inhalt)
        .is_ok_and(|_| token.ist_schluessel(&inhalt))
}

/// Same inode, same size, same time of the last change: the file was not replaced or written meanwhile.
fn gleiche_datei(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
}

/// Makes a new file `name` in `ordner` with `O_EXCL`, never following a link.
fn neu_anlegen(ordner: &OwnedFd, name: &str) -> rustix::io::Result<OwnedFd> {
    if !einfacher_name(name) {
        return Err(Errno::INVAL);
    }
    rustix::fs::openat(
        ordner,
        name,
        OFlags::WRONLY
            .union(OFlags::CREATE)
            .union(OFlags::EXCL)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::from_raw_mode(0o600),
    )
}

/// Writes the new `config.json` into a temp file next to it: same mode and group as the old one, synced.
fn config_temp_schreiben<'a>(
    quelle: &'a OwnedFd,
    alt: &Stat,
    inhalt: &[u8],
) -> Result<Temp<'a>, Fehler> {
    let name = TempArt::Config.name(std::process::id());
    let fd = neu_anlegen(quelle, &name).map_err(|_| Fehler::QuelleNichtSchreibbar)?;
    let temp = Temp {
        ordner: quelle,
        name,
    };
    rustix::fs::fchown(&fd, None, Some(rustix::fs::Gid::from_raw(alt.st_gid)))
        .map_err(|_| Fehler::QuelleNichtSchreibbar)?;
    rustix::fs::fchmod(&fd, Mode::from_raw_mode(alt.st_mode & 0o7777))
        .map_err(|_| Fehler::QuelleNichtSchreibbar)?;
    let mut datei = File::from(fd);
    datei
        .write_all(inhalt)
        .and_then(|()| datei.sync_all())
        .map_err(|_| Fehler::QuelleNichtSchreibbar)?;
    Ok(temp)
}

/// Writes the token and LF into the temp file of the key (0600, `O_EXCL`), synced. Returns it and its stat.
pub(crate) fn schluessel_temp_schreiben<'a>(
    ziel: &'a OwnedFd,
    token: &Token,
) -> Result<(Temp<'a>, Stat), Fehler> {
    let name = TempArt::Schluessel.name(std::process::id());
    let fd = neu_anlegen(ziel, &name).map_err(|_| Fehler::ZielNichtSchreibbar)?;
    let temp = Temp { ordner: ziel, name };
    rustix::fs::fchmod(&fd, Mode::from_raw_mode(0o600)).map_err(|_| Fehler::ZielNichtSchreibbar)?;
    let st = rustix::fs::fstat(&fd).map_err(|_| Fehler::ZielNichtSchreibbar)?;
    let mut datei = File::from(fd);
    datei
        .write_all(token.as_str().as_bytes())
        .and_then(|()| datei.write_all(b"\n"))
        .and_then(|()| datei.sync_all())
        .map_err(|_| Fehler::ZielNichtSchreibbar)?;
    Ok((temp, st))
}

/// Puts the temp file at the name of the key: a hard link that never replaces; with `--neu` over an existing name an
/// exchange (`RENAME_EXCHANGE` on Linux, `RENAME_SWAP` on macOS), so that the old key can come back on a later error.
fn schluessel_setzen(ziel: &OwnedFd, temp: &str, st: &Stat, neu: bool) -> Result<Zusage, Fehler> {
    match rustix::fs::linkat(ziel, temp, ziel, SCHLUESSELDATEI, AtFlags::empty()) {
        Ok(()) => Ok(Zusage::Neu(*st)),
        Err(Errno::EXIST) if !neu => Err(Fehler::DateiDa),
        Err(Errno::EXIST) => {
            rustix::fs::renameat_with(ziel, temp, ziel, SCHLUESSELDATEI, RenameFlags::EXCHANGE)
                .map(|()| Zusage::Getauscht)
                .map_err(|_| Fehler::ZielNichtSchreibbar)
        }
        Err(_) => Err(Fehler::ZielNichtSchreibbar),
    }
}

/// Undoes the key after a later error: the new name goes if it is still our inode; an exchange is exchanged back
/// (the temp name, with the new token again, goes with its drop). Best effort: a failure here leaves the state of a
/// hard end after step 8, which the next call finishes (section 3b, restart).
fn zuruecknehmen(ziel: &OwnedFd, zusage: &Zusage, temp: Option<&Temp<'_>>) {
    match zusage {
        Zusage::Vorher => {}
        Zusage::Neu(eigen) => {
            if rustix::fs::statat(ziel, SCHLUESSELDATEI, AtFlags::SYMLINK_NOFOLLOW)
                .is_ok_and(|st| st.st_dev == eigen.st_dev && st.st_ino == eigen.st_ino)
            {
                let _ = rustix::fs::unlinkat(ziel, SCHLUESSELDATEI, AtFlags::empty());
            }
        }
        Zusage::Getauscht => {
            if let Some(t) = temp {
                let _ = rustix::fs::renameat_with(
                    ziel,
                    t.name.as_str(),
                    ziel,
                    SCHLUESSELDATEI,
                    RenameFlags::EXCHANGE,
                );
            }
        }
    }
    let _ = rustix::fs::fsync(ziel);
}
