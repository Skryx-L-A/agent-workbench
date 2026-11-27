//! The default of the firm for the route table (A45; contract 2, addendum "Vorgabe der Firma").
//!
//! The settings file of the user may only strike routes and targets, never add or change one. Every route of the user
//! must stand in the default under its key with the same kind, class (name and rank), target, switch
//! `eingeschraenktes_token` and, when the default fixes one, the same model. Every network target of it (host, port,
//! kind of access, key binding) must stand at that route of the default, and each of its method and path pairs must
//! stand there at the same target. The default is a settings file of contract 2 owned by root. A missing, foreign,
//! writable or linked default, or one in a folder that is not safe, ends every start before the target list (fail
//! closed, fixed reason).
//!
//! The check works on descriptors: from `/` down, each folder is opened with `O_NOFOLLOW` and `O_DIRECTORY` relative
//! to the one before and checked with `fstat`; the file itself is opened with `O_NOFOLLOW` and checked with `fstat`
//! before a byte is read. No step checks a path text and opens it afterwards.

use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};

use einstellungen::{Einstellungen, Netzziel, Route};
use rustix::fs::{FileType, Mode, OFlags};

use crate::dateien::modus_von;
use crate::fehler::Fehler;

/// The default of the delivered build: the one place of this path.
pub const VORGABE_PFAD: &str = "/etc/werkbank/vorgabe.json";

/// The largest default in bytes (the limit of contract 2).
const VORGABE_MAX: u64 = 65_536;

/// The path of the default in this build: [`VORGABE_PFAD`], in the test build (feature `pruefbau`) `vorgabe.json` next
/// to the program, like the own configuration.
#[must_use]
pub fn pfad() -> Option<PathBuf> {
    if cfg!(feature = "pruefbau") {
        let programm = std::env::current_exe().ok()?.canonicalize().ok()?;
        Some(programm.parent()?.join("vorgabe.json"))
    } else {
        Some(PathBuf::from(VORGABE_PFAD))
    }
}

/// The owner a default may have besides root: none in the delivered build (root only), the own user in the test build,
/// which cannot make a file of root.
#[must_use]
pub fn zweiter_eigner() -> Option<u32> {
    if cfg!(feature = "pruefbau") {
        Some(rustix::process::geteuid().as_raw())
    } else {
        None
    }
}

fn eigner_gut(eigner: u32, zweiter: Option<u32>) -> bool {
    eigner == 0 || zweiter == Some(eigner)
}

/// The rule for a folder above the default: a folder, of an allowed owner, which neither the group nor others may
/// write; a folder others may write is allowed only with the sticky bit (as `/tmp`), because then no one else can
/// rename or remove what lies in it, and only above the last folder.
#[must_use]
pub fn ordner_gut(
    art: FileType,
    eigner: u32,
    modus: u32,
    zweiter: Option<u32>,
    letzter: bool,
) -> bool {
    let schreibbar = modus & 0o022 != 0;
    let klebend = modus & 0o1000 != 0;
    art == FileType::Directory
        && eigner_gut(eigner, zweiter)
        && (!schreibbar || (klebend && !letzter))
}

/// The rule for the default itself: a regular file of an allowed owner, which neither the group nor others may write.
#[must_use]
pub fn datei_gut(art: FileType, eigner: u32, modus: u32, zweiter: Option<u32>) -> bool {
    art == FileType::RegularFile && eigner_gut(eigner, zweiter) && modus & 0o022 == 0
}

fn oeffnen(ordner: &OwnedFd, name: &Path, flags: OFlags) -> Result<OwnedFd, Fehler> {
    rustix::fs::openat(
        ordner,
        name,
        flags
            .union(OFlags::RDONLY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC)
            .union(OFlags::NONBLOCK),
        Mode::empty(),
    )
    .map_err(|e| match e {
        rustix::io::Errno::NOENT => Fehler::VorgabeFehlt,
        _ => Fehler::VorgabeUnsicher,
    })
}

/// Opens the default at the absolute path `pfad` through descriptors and reads it.
///
/// # Errors
/// `lauf.vorgabe_fehlt` when the file or a folder above it does not exist; `lauf.vorgabe_unsicher` when the path is not
/// absolute and plain, a part is a link, a folder or the file breaks [`ordner_gut`] or [`datei_gut`], or the file is
/// larger than the limit or cannot be read.
pub fn lesen_bytes(pfad: &Path, zweiter: Option<u32>) -> Result<Vec<u8>, Fehler> {
    let mut teile = Vec::new();
    for (i, teil) in pfad.components().enumerate() {
        match teil {
            Component::RootDir if i == 0 => {}
            Component::Normal(t) => teile.push(t),
            _ => return Err(Fehler::VorgabeUnsicher),
        }
    }
    let Some((name, ordner_teile)) = teile.split_last() else {
        return Err(Fehler::VorgabeUnsicher);
    };
    if !pfad.has_root() {
        return Err(Fehler::VorgabeUnsicher);
    }
    let mut ordner = rustix::fs::open(
        "/",
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )
    .map_err(|_| Fehler::VorgabeUnsicher)?;
    let pruefen_ordner = |fd: &OwnedFd, letzter: bool| -> Result<(), Fehler> {
        let st = rustix::fs::fstat(fd).map_err(|_| Fehler::VorgabeUnsicher)?;
        if ordner_gut(
            FileType::from_raw_mode(st.st_mode),
            st.st_uid,
            modus_von(&st),
            zweiter,
            letzter,
        ) {
            Ok(())
        } else {
            Err(Fehler::VorgabeUnsicher)
        }
    };
    pruefen_ordner(&ordner, ordner_teile.is_empty())?;
    for (i, teil) in ordner_teile.iter().enumerate() {
        ordner = oeffnen(&ordner, Path::new(teil), OFlags::DIRECTORY)?;
        pruefen_ordner(&ordner, i + 1 == ordner_teile.len())?;
    }
    let fd = oeffnen(&ordner, Path::new(name), OFlags::empty())?;
    let st = rustix::fs::fstat(&fd).map_err(|_| Fehler::VorgabeUnsicher)?;
    if !datei_gut(
        FileType::from_raw_mode(st.st_mode),
        st.st_uid,
        modus_von(&st),
        zweiter,
    ) {
        return Err(Fehler::VorgabeUnsicher);
    }
    let mut text = Vec::new();
    File::from(fd)
        .take(VORGABE_MAX + 1)
        .read_to_end(&mut text)
        .map_err(|_| Fehler::VorgabeUnsicher)?;
    if u64::try_from(text.len()).map_err(|_| Fehler::VorgabeUnsicher)? > VORGABE_MAX {
        return Err(Fehler::VorgabeUnsicher);
    }
    Ok(text)
}

/// Reads the default of this build ([`pfad`], [`zweiter_eigner`]) as settings of contract 2.
///
/// # Errors
/// As [`lesen_bytes`]; `lauf.vorgabe_kaputt` when the file breaks contract 2.
pub fn lesen() -> Result<Einstellungen, Fehler> {
    let pfad = pfad().ok_or(Fehler::VorgabeFehlt)?;
    let bytes = lesen_bytes(&pfad, zweiter_eigner())?;
    einstellungen::lesen(&bytes).map_err(|_| Fehler::VorgabeKaputt)
}

fn gleiches_ziel(nutzer: &Netzziel, vorgabe: &Netzziel) -> bool {
    nutzer.host() == vorgabe.host()
        && nutzer.ist_unterdomaene() == vorgabe.ist_unterdomaene()
        && nutzer.port() == vorgabe.port()
}

/// Whether one target of the user stays inside its counterpart of the default: the same kind of access, a key only
/// where the default binds one (striking the binding narrows the target), and every method and path pair of the user
/// stands at the default target.
fn innerhalb(nutzer: &Netzziel, vorgabe: &Netzziel) -> bool {
    nutzer.zugriff() == vorgabe.zugriff()
        && (!nutzer.schluessel() || vorgabe.schluessel())
        && nutzer.pfade().iter().all(|p| {
            vorgabe
                .pfade()
                .iter()
                .any(|v| v.methode() == p.methode() && v.pfad() == p.pfad())
        })
}

/// The intersection rule: every route of the settings of the user `nutzer` against the route with the same key in the
/// default. Striking a route, a target, a path or a key binding holds.
///
/// # Errors
/// `lauf.vorgabe_route_fremd` when the default has no route of a key of the user; `lauf.vorgabe_route_abweichend` when
/// kind, class (its name or its rank in `klassen`) or target differ, or the default fixes a model the user does not
/// keep; `lauf.vorgabe_schalter` when the switch `eingeschraenktes_token` differs; `lauf.vorgabe_erweitert` when a
/// target of the user is missing at that route of the default, has another kind of access, binds the key where the
/// default does not, or has a method and path pair the default target lacks.
pub fn pruefen(vorgabe: &Einstellungen, nutzer: &Einstellungen) -> Result<(), Fehler> {
    for route in nutzer.routen() {
        let v = vorgabe
            .route(route.schluessel())
            .ok_or(Fehler::VorgabeRouteFremd)?;
        route_pruefen(v, route)?;
    }
    Ok(())
}

/// One route of the user `n` against its route `v` of the default.
fn route_pruefen(v: &Route, n: &Route) -> Result<(), Fehler> {
    let modell_gut = v.modell().is_none_or(|m| n.modell() == Some(m));
    if n.art() != v.art()
        || n.klasse() != v.klasse()
        || n.rang() != v.rang()
        || n.ziel() != v.ziel()
        || !modell_gut
    {
        return Err(Fehler::VorgabeRouteAbweichend);
    }
    if n.eingeschraenktes_token() != v.eingeschraenktes_token() {
        return Err(Fehler::VorgabeSchalter);
    }
    for z in n.netzziele() {
        let gut = v
            .netzziele()
            .iter()
            .find(|vz| gleiches_ziel(z, vz))
            .is_some_and(|vz| innerhalb(z, vz));
        if !gut {
            return Err(Fehler::VorgabeErweitert);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
