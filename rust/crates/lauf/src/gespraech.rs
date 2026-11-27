//! The conversation of a session (contract 1, "Fassung 2: `fortsetzung_von` und Fortsetzen"): the folder
//! `<gespraeche>/<sitzung>/` with `gespraech.json` and, from version 2 on, the state folder of the harness.
//!
//! Step 5d checks the conversation of the predecessor before the enclosure and before any child. Step 8 makes the
//! folder of a new start exclusively, or renames the folder of the predecessor to the new session without replacing
//! anything. This module never removes a conversation: clearing `gespraeche/` belongs to the service.

use std::ffi::CStr;
use std::io::Read;
use std::path::{Path, PathBuf};

use einstellungen::json::{Wert, lesen, text_schreiben};
use rustix::fd::OwnedFd;
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use start::{Startdatei, Startform};

use crate::dateien;
use crate::fehler::Fehler;

/// The file of the conversation inside its folder.
pub const DATEI: &str = "gespraech.json";

/// The state folder of the harness inside the folder of the conversation (bound at its place below the home).
pub const ZUSTAND: &str = "zustand";

/// The largest `gespraech.json` in bytes.
pub const DATEI_MAX: u64 = 4096;

/// The content of `gespraech.json`: harness, working folder, route and customer of the start file, the key of the harness
/// session (`None` for the conversation of a session of version 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gespraech {
    /// The harness of the start file.
    pub harness: String,
    /// The working folder of the start file, as written there.
    pub arbeitsordner: String,
    /// The route of the start file.
    pub route: String,
    /// The customer of the start file, `None` for `null`.
    pub kunde: Option<String>,
    /// The key of the harness session, a lower-case UUID v4, or `None` (version 1).
    pub kennung: Option<String>,
}

/// What step 5d found: the conversation of the predecessor that step 8 renames, and its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fortsetzung {
    /// The key of the predecessor session (`fortsetzung_von`).
    pub alt: String,
    /// The key of the harness session in its `gespraech.json`.
    pub kennung: String,
}

/// A lower-case UUID v4: `^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`.
#[must_use]
pub fn ist_kennung(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            14 => *c == b'4',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

/// The UUID v4 of 16 random bytes: the version and variant bits set, lower-case hex in the five groups.
#[must_use]
pub fn kennung_aus(mut b: [u8; 16]) -> String {
    // the cleared bits are added, not or-ed: both are the same here, and a sum has no equivalent mutant
    b[6] = 0x40 + (b[6] & 0x0f);
    b[8] = 0x80 + (b[8] & 0x3f);
    let h = crate::marke::hex(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// A new key of a harness session from the randomness of the system.
#[must_use]
pub fn neue_kennung() -> Option<String> {
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .ok()?;
    Some(kennung_aus(b))
}

/// Reads `gespraech.json`: a closed object with exactly `fassung` (the integer 1), `harness`, `arbeitsordner` and
/// `route` (texts), `kunde` (a text or `null`) and `kennung` (a key of [`ist_kennung`] or `null`), after the JSON rules
/// of the contract. `None` for every other form.
#[must_use]
pub fn lesen_text(bytes: &[u8]) -> Option<Gespraech> {
    let Wert::Objekt(mut felder) = lesen(bytes).ok()? else {
        return None;
    };
    let mut nehmen = |name: &str| {
        let i = felder.iter().position(|(n, _)| n == name)?;
        Some(felder.remove(i).1)
    };
    if nehmen("fassung")? != Wert::Ganzzahl("1".to_owned()) {
        return None;
    }
    let (Wert::Text(harness), Wert::Text(arbeitsordner), Wert::Text(route)) = (
        nehmen("harness")?,
        nehmen("arbeitsordner")?,
        nehmen("route")?,
    ) else {
        return None;
    };
    let kunde = match nehmen("kunde")? {
        Wert::Null => None,
        Wert::Text(k) => Some(k),
        _ => return None,
    };
    let kennung = match nehmen("kennung")? {
        Wert::Null => None,
        Wert::Text(k) if ist_kennung(&k) => Some(k),
        _ => return None,
    };
    felder.is_empty().then_some(Gespraech {
        harness,
        arbeitsordner,
        route,
        kunde,
        kennung,
    })
}

/// The text of `gespraech.json`, in the order of the contract, with a line end.
#[must_use]
pub fn text(g: &Gespraech) -> String {
    format!(
        "{{\"fassung\":1,\"harness\":{},\"arbeitsordner\":{},\"route\":{},\"kunde\":{},\"kennung\":{}}}\n",
        text_schreiben(&g.harness),
        text_schreiben(&g.arbeitsordner),
        text_schreiben(&g.route),
        g.kunde
            .as_deref()
            .map_or_else(|| "null".to_owned(), text_schreiben),
        g.kennung
            .as_deref()
            .map_or_else(|| "null".to_owned(), text_schreiben)
    )
}

fn ordner_flags() -> OFlags {
    OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC)
}

/// The rule on `fstat` of a folder of the conversation: a folder of the own user with mode exactly 0700.
fn ordner_gut(fd: &OwnedFd) -> bool {
    rustix::fs::fstat(fd).is_ok_and(|st| {
        dateien::zaehlordner_gut(
            FileType::from_raw_mode(st.st_mode),
            st.st_uid,
            dateien::modus_von(&st),
            rustix::process::geteuid().as_raw(),
        )
    })
}

/// How the folder `gespraeche` could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Basis {
    /// It does not exist.
    Fehlt,
    /// It is a link, no folder, foreign or not 0700.
    Unbrauchbar,
}

/// Opens the folder `gespraeche` with `O_NOFOLLOW | O_DIRECTORY`: a real own folder with mode 0700.
fn basis_oeffnen(gespraeche: &Path) -> Result<OwnedFd, Basis> {
    let fd = rustix::fs::open(gespraeche, ordner_flags(), Mode::empty()).map_err(|e| match e {
        Errno::NOENT => Basis::Fehlt,
        _ => Basis::Unbrauchbar,
    })?;
    if ordner_gut(&fd) {
        Ok(fd)
    } else {
        Err(Basis::Unbrauchbar)
    }
}

fn name(text: &str) -> Result<std::ffi::CString, Fehler> {
    std::ffi::CString::new(text).map_err(|_| Fehler::Intern)
}

/// `gespraech.json` below the folder `ordner`: an own regular file with mode 0600, no link, at most [`DATEI_MAX`]
/// bytes, of the closed form. `None` for every fault.
fn datei_lesen(ordner: &OwnedFd) -> Option<Gespraech> {
    let datei: &CStr = c"gespraech.json";
    let fd = rustix::fs::openat(
        ordner,
        datei,
        OFlags::RDONLY
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC)
            .union(OFlags::NONBLOCK),
        Mode::empty(),
    )
    .ok()?;
    let st = rustix::fs::fstat(&fd).ok()?;
    if !dateien::start_modus_gut(
        FileType::from_raw_mode(st.st_mode),
        st.st_uid,
        dateien::modus_von(&st),
        rustix::process::geteuid().as_raw(),
    ) {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::from(fd)
        .take(DATEI_MAX + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if u64::try_from(bytes.len()).ok()? > DATEI_MAX {
        return None;
    }
    lesen_text(&bytes)
}

/// Step 5d, only for a start file with `fortsetzung_von`, in the order of the contract. `fortsetzbar` tells whether the
/// harness has an option to continue (harness table, column "Fortsetzen"). Returns `None` for a new start.
///
/// # Errors
/// `lauf.fortsetzung_unmoeglich` (5d.1, 5d.3a), `lauf.fortsetzung_fehlt` (5d.2, also without the folder
/// `gespraeche`), `lauf.fortsetzung_kaputt` (5d.3, also for a folder `gespraeche` that is a link, foreign or not
/// 0700), `lauf.fortsetzung_fremd` (5d.4: harness, working folder, route or customer not exactly equal), `lauf.sitzung_belegt` (5d.5).
pub fn pruefen(
    gespraeche: &Path,
    start: &Startdatei,
    fortsetzbar: bool,
) -> Result<Option<Fortsetzung>, Fehler> {
    let Some(alt) = start.fortsetzung_von() else {
        return Ok(None);
    };
    // 5d.1
    if !fortsetzbar {
        return Err(Fehler::FortsetzungUnmoeglich);
    }
    let basis = basis_oeffnen(gespraeche).map_err(|b| match b {
        Basis::Fehlt => Fehler::FortsetzungFehlt,
        Basis::Unbrauchbar => Fehler::FortsetzungKaputt,
    })?;
    // 5d.2 and 5d.3: the folder of the predecessor, then its file
    let ordner = rustix::fs::openat(&basis, name(alt)?.as_c_str(), ordner_flags(), Mode::empty())
        .map_err(|e| match e {
        Errno::NOENT => Fehler::FortsetzungFehlt,
        _ => Fehler::FortsetzungKaputt,
    })?;
    if !ordner_gut(&ordner) {
        return Err(Fehler::FortsetzungKaputt);
    }
    let g = datei_lesen(&ordner).ok_or(Fehler::FortsetzungKaputt)?;
    // 5d.3a
    let kennung = g.kennung.ok_or(Fehler::FortsetzungUnmoeglich)?;
    // 5d.4: equality only, no comparison of classes (a route of the same kind with another class is foreign too)
    if g.harness != start.harness()
        || g.arbeitsordner != start.arbeitsordner()
        || g.route != start.route()
        || g.kunde.as_deref() != start.kunde()
    {
        return Err(Fehler::FortsetzungFremd);
    }
    // 5d.5: any entry of the new name, a link too
    match rustix::fs::statat(
        &basis,
        name(start.sitzung())?.as_c_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(Errno::NOENT) => {}
        _ => return Err(Fehler::SitzungBelegt),
    }
    Ok(Some(Fortsetzung {
        alt: alt.to_owned(),
        kennung,
    }))
}

/// Step 8: opens the folder `gespraeche`, made with mode 0700 when it does not exist yet.
///
/// # Errors
/// `lauf.intern` when it cannot be made, or is a link, no folder, foreign or not 0700.
fn basis_fuer_start(gespraeche: &Path) -> Result<OwnedFd, Fehler> {
    match basis_oeffnen(gespraeche) {
        Err(Basis::Fehlt) => {
            // the result of making it does not count: another start may make it at the same time, and whatever is
            // there afterwards is checked like a folder that was found
            let _ = dateien::ordner_neu(gespraeche);
            basis_oeffnen(gespraeche).map_err(|_| Fehler::Intern)
        }
        andere => andere.map_err(|_| Fehler::Intern),
    }
}

/// Step 8, new start: makes `<gespraeche>/<sitzung>` exclusively (mode 0700), writes `gespraech.json` with `O_EXCL`
/// (mode 0600) and, with `mit_zustand`, the empty state folder of the harness (mode 0700). Returns the folder of the
/// conversation. Nothing made here is ever removed.
///
/// # Errors
/// `lauf.sitzung_belegt` when the folder exists already, `lauf.intern` for every other fault.
pub fn anlegen(
    gespraeche: &Path,
    sitzung: &str,
    g: &Gespraech,
    mit_zustand: bool,
) -> Result<PathBuf, Fehler> {
    basis_fuer_start(gespraeche)?;
    let ordner = gespraeche.join(sitzung);
    dateien::ordner_neu(&ordner).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => Fehler::SitzungBelegt,
        _ => Fehler::Intern,
    })?;
    dateien::datei_neu(&ordner.join(DATEI), text(g).as_bytes()).map_err(|_| Fehler::Intern)?;
    if mit_zustand {
        dateien::ordner_neu(&ordner.join(ZUSTAND)).map_err(|_| Fehler::Intern)?;
    }
    Ok(ordner)
}

/// Step 8, continuing: renames `<gespraeche>/<alt>` to `<gespraeche>/<neu>` without replacing anything
/// (`renameat2` with `RENAME_NOREPLACE`). Returns the folder of the conversation under the new name.
///
/// # Errors
/// `lauf.sitzung_belegt` when the new name exists, `lauf.fortsetzung_fehlt` when the old one is gone, `lauf.intern`
/// for every other fault.
pub fn uebernehmen(gespraeche: &Path, alt: &str, neu: &str) -> Result<PathBuf, Fehler> {
    let basis = basis_oeffnen(gespraeche).map_err(|_| Fehler::Intern)?;
    rustix::fs::renameat_with(
        &basis,
        name(alt)?.as_c_str(),
        &basis,
        name(neu)?.as_c_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|e| match e {
        Errno::EXIST => Fehler::SitzungBelegt,
        Errno::NOENT => Fehler::FortsetzungFehlt,
        _ => Fehler::Intern,
    })?;
    Ok(gespraeche.join(neu))
}

/// After a fault before the exec: renames the conversation back from `neu` to `alt`, best effort and without replacing
/// anything.
pub fn zurueck(gespraeche: &Path, neu: &str, alt: &str) {
    let _ = uebernehmen(gespraeche, neu, alt);
}

#[cfg(test)]
mod tests;
