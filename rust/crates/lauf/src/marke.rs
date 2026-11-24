//! The folder mark (contract 1, E6): `ordnermarken.json` in the state folder of the installation, read, checked and
//! written under an exclusive `flock` on `ordnermarken.lock`, written atomically (new file, `fsync`, `rename`). The
//! mark only rises. Every fault of the file or the folder is `lauf.ordner_marke_kaputt` (fail closed).

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;
use std::time::{Duration, Instant};

use einstellungen::json::{Wert, lesen, text_schreiben};
use rustix::fs::{FlockOperation, Mode, OFlags};

use crate::fehler::Fehler;

/// The file of the marks.
pub const MARKEN_DATEI: &str = "ordnermarken.json";
/// The file of the lock.
pub const SPERR_DATEI: &str = "ordnermarken.lock";
/// The largest file of marks.
const MARKEN_MAX: u64 = 1024 * 1024;
/// The most marks.
const ANZAHL_MAX: usize = 4096;
/// How long a start waits for the lock before it gives up (fail closed).
pub const SPERR_FRIST: Duration = Duration::from_secs(5);

/// One mark: the resolved folder, its key, the customer (`None` is the value "no customer") and the highest class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marke {
    /// The folder, already resolved (`realpath`).
    pub ordner: String,
    /// The key of the mark: 32 lower-case hex characters, drawn once, kept when the class rises.
    pub kennung: String,
    /// The customer, or `None` for "no customer".
    pub kunde: Option<String>,
    /// The highest class that worked in the folder, a name of `klassen`.
    pub klasse_hoch: String,
}

/// What the start asks for: the resolved working folder, the customer of the start and the class of the route.
#[derive(Debug, Clone, Copy)]
pub struct Anfrage<'a> {
    /// `realpath(arbeitsordner)`.
    pub ordner: &'a str,
    /// The customer of the start.
    pub kunde: Option<&'a str>,
    /// The class of the route.
    pub klasse: &'a str,
}

/// `a` is `b` or lies below it, compared whole segments at `/`.
fn innerhalb(a: &str, b: &str) -> bool {
    Path::new(a).starts_with(b)
}

/// The mark is related to the folder: the same folder, an ancestor or a descendant.
#[must_use]
pub fn verwandt(marke: &str, ordner: &str) -> bool {
    innerhalb(ordner, marke) || innerhalb(marke, ordner)
}

fn rang(klassen: &[String], klasse: &str) -> Option<usize> {
    klassen.iter().position(|k| k == klasse)
}

/// Reads the text of the file.
///
/// # Errors
/// `lauf.ordner_marke_kaputt` for a broken file: not the closed form, a class the settings do not know, a folder twice,
/// more than 4096 marks. Then, for a file of the right form, `lauf.ordner_marke_kennung` when any mark (related or not)
/// has no key, a key of the wrong form, or the key of another entry.
pub fn lesen_marken(bytes: &[u8], klassen: &[String]) -> Result<Vec<Marke>, Fehler> {
    let kaputt = Fehler::OrdnerMarkeKaputt;
    if u64::try_from(bytes.len()).map_err(|_| Fehler::OrdnerMarkeKaputt)? > MARKEN_MAX {
        return Err(kaputt);
    }
    let Ok(Wert::Objekt(felder)) = lesen(bytes) else {
        return Err(kaputt);
    };
    if felder.len() != 2
        || einstellungen::json::feld(&felder, "fassung") != Some(&Wert::Ganzzahl("1".to_owned()))
    {
        return Err(kaputt);
    }
    let Some(Wert::Liste(liste)) = einstellungen::json::feld(&felder, "marken") else {
        return Err(kaputt);
    };
    if liste.len() > ANZAHL_MAX {
        return Err(kaputt);
    }
    let mut marken: Vec<Marke> = Vec::with_capacity(liste.len());
    let mut kennung_falsch = false;
    for eintrag in liste {
        let Wert::Objekt(f) = eintrag else {
            return Err(kaputt);
        };
        if f.iter()
            .any(|(n, _)| !matches!(n.as_str(), "ordner" | "kennung" | "kunde" | "klasse_hoch"))
        {
            return Err(kaputt);
        }
        let (Some(Wert::Text(ordner)), Some(kunde_wert), Some(Wert::Text(klasse_hoch))) = (
            einstellungen::json::feld(f, "ordner"),
            einstellungen::json::feld(f, "kunde"),
            einstellungen::json::feld(f, "klasse_hoch"),
        ) else {
            return Err(kaputt);
        };
        let kunde = match kunde_wert {
            Wert::Null => None,
            Wert::Text(k) if einstellungen::ist_kennung(k) => Some(k.clone()),
            _ => return Err(kaputt),
        };
        if !crate::ordner_form(ordner)
            || rang(klassen, klasse_hoch).is_none()
            || marken.iter().any(|m| &m.ordner == ordner)
        {
            return Err(kaputt);
        }
        let kennung = match einstellungen::json::feld(f, "kennung") {
            Some(Wert::Text(k)) if start::ist_marken_kennung(k) => k.clone(),
            _ => {
                kennung_falsch = true;
                String::new()
            }
        };
        marken.push(Marke {
            ordner: ordner.clone(),
            kennung,
            kunde,
            klasse_hoch: klasse_hoch.clone(),
        });
    }
    let doppelt = marken
        .iter()
        .enumerate()
        .any(|(i, m)| marken[i + 1..].iter().any(|n| n.kennung == m.kennung));
    if kennung_falsch || doppelt {
        return Err(Fehler::OrdnerMarkeKennung);
    }
    Ok(marken)
}

/// A new key from the randomness of the system: 16 bytes of `/dev/urandom` as lower-case hex.
#[must_use]
pub fn zufall_kennung() -> Option<String> {
    let mut b = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .ok()?;
    Some(hex(&b))
}

/// Bytes as lower-case hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    const ZIFFERN: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from(ZIFFERN[usize::from(b >> 4)]));
        s.push(char::from(ZIFFERN[usize::from(b & 0x0f)]));
    }
    s
}

/// How often a key is drawn before the start gives up (a clash of 128 random bits does not happen; a broken source of
/// randomness does).
const ZUEGE_MAX: usize = 16;

/// The text of the file.
#[must_use]
pub fn schreiben_marken(marken: &[Marke]) -> String {
    let mut s = String::from("{\"fassung\":1,\"marken\":[");
    for (i, m) in marken.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"ordner\":");
        s.push_str(&text_schreiben(&m.ordner));
        s.push_str(",\"kennung\":");
        s.push_str(&text_schreiben(&m.kennung));
        s.push_str(",\"kunde\":");
        match &m.kunde {
            Some(k) => s.push_str(&text_schreiben(k)),
            None => s.push_str("null"),
        }
        s.push_str(",\"klasse_hoch\":");
        s.push_str(&text_schreiben(&m.klasse_hoch));
        s.push('}');
    }
    s.push_str("]}\n");
    s
}

/// The decision on the marks, without the file: the codes of steps 4 and 5, and the new list when it changed.
///
/// # Errors
/// `lauf.ordner_anderer_kunde`, `lauf.ordner_hoeher_markiert`, or `lauf.ordner_marke_kaputt` when the class of the
/// route is unknown or the list is full; `lauf.intern` when `zufall` gives no new key. A new mark gets a key from
/// `zufall`, drawn again while another mark of the file carries it.
pub fn entscheiden(
    mut marken: Vec<Marke>,
    anfrage: Anfrage<'_>,
    klassen: &[String],
    zufall: &mut dyn FnMut() -> Option<String>,
) -> Result<Option<Vec<Marke>>, Fehler> {
    let route = rang(klassen, anfrage.klasse).ok_or(Fehler::OrdnerMarkeKaputt)?;
    let verwandte: Vec<&Marke> = marken
        .iter()
        .filter(|m| verwandt(&m.ordner, anfrage.ordner))
        .collect();
    if verwandte
        .iter()
        .any(|m| m.kunde.as_deref() != anfrage.kunde)
    {
        return Err(Fehler::OrdnerAndererKunde);
    }
    let hoechste = verwandte
        .iter()
        .filter_map(|m| rang(klassen, &m.klasse_hoch))
        .max();
    if hoechste.is_some_and(|h| route < h) {
        return Err(Fehler::OrdnerHoeherMarkiert);
    }
    if let Some(m) = marken.iter_mut().find(|m| m.ordner == anfrage.ordner) {
        let alt = rang(klassen, &m.klasse_hoch).ok_or(Fehler::OrdnerMarkeKaputt)?;
        if route <= alt {
            return Ok(None);
        }
        anfrage.klasse.clone_into(&mut m.klasse_hoch);
        return Ok(Some(marken));
    }
    if marken.len() >= ANZAHL_MAX {
        return Err(Fehler::OrdnerMarkeKaputt);
    }
    let kennung = (0..ZUEGE_MAX)
        .filter_map(|_| zufall())
        .find(|k| start::ist_marken_kennung(k) && marken.iter().all(|m| &m.kennung != k))
        .ok_or(Fehler::Intern)?;
    marken.push(Marke {
        ordner: anfrage.ordner.to_owned(),
        kennung,
        kunde: anfrage.kunde.map(str::to_owned),
        klasse_hoch: anfrage.klasse.to_owned(),
    });
    Ok(Some(marken))
}

fn kaputt<T>(_: T) -> Fehler {
    Fehler::OrdnerMarkeKaputt
}

/// The state folder: a real folder (no link) of the own user with mode exactly 0700.
fn ordner_oeffnen(zustand: &Path) -> Result<OwnedFd, Fehler> {
    let fd = rustix::fs::open(
        zustand,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(kaputt)?;
    let st = rustix::fs::fstat(&fd).map_err(kaputt)?;
    let uid = rustix::process::geteuid().as_raw();
    if st.st_uid != uid || crate::dateien::modus_von(&st) & 0o7777 != 0o700 {
        return Err(Fehler::OrdnerMarkeKaputt);
    }
    Ok(fd)
}

/// Whether a failed step is tried again: the faults of a lost race or a signal, and only while time is left.
///
/// macOS answers `ENOENT` to an `openat` with `O_CREAT` on a name that another process or thread creates at the same
/// moment (measured: 11 of 16 simultaneous starts in one process). The folder is open and the name is made by this very
/// call, so `ENOENT` is a lost race here, not a missing path.
fn wiederholen(fehler: rustix::io::Errno, vor_ende: bool) -> bool {
    vor_ende && matches!(fehler, rustix::io::Errno::NOENT | rustix::io::Errno::INTR)
}

fn sperr_datei(ordner: &OwnedFd) -> rustix::io::Result<OwnedFd> {
    rustix::fs::openat(
        ordner,
        SPERR_DATEI,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
}

/// Takes the exclusive lock, waiting at most `frist`. A lost race at the open of the lock file is retried
/// ([`wiederholen`]), a busy lock as well.
fn sperren(ordner: &OwnedFd, frist: Duration) -> Result<OwnedFd, Fehler> {
    let ende = Instant::now() + frist;
    let sperre = loop {
        match sperr_datei(ordner) {
            Ok(fd) => break fd,
            Err(e) if wiederholen(e, Instant::now() < ende) => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return Err(Fehler::OrdnerMarkeKaputt),
        }
    };
    loop {
        match rustix::fs::flock(&sperre, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(sperre),
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR)
                if Instant::now() < ende =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return Err(Fehler::OrdnerMarkeKaputt),
        }
    }
}

/// Reads the file under the lock: `ENOENT` is "no mark yet", every other fault is broken.
fn datei_lesen(ordner: &OwnedFd, klassen: &[String]) -> Result<Vec<Marke>, Fehler> {
    let fd = match rustix::fs::openat(
        ordner,
        MARKEN_DATEI,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(Vec::new()),
        Err(_) => return Err(Fehler::OrdnerMarkeKaputt),
    };
    let st = rustix::fs::fstat(&fd).map_err(kaputt)?;
    let uid = rustix::process::geteuid().as_raw();
    if !crate::dateien::start_modus_gut(
        rustix::fs::FileType::from_raw_mode(st.st_mode),
        st.st_uid,
        crate::dateien::modus_von(&st),
        uid,
    ) {
        return Err(Fehler::OrdnerMarkeKaputt);
    }
    let mut text = Vec::new();
    File::from(fd)
        .take(MARKEN_MAX + 1)
        .read_to_end(&mut text)
        .map_err(kaputt)?;
    lesen_marken(&text, klassen)
}

/// Where a write of the list stopped (see [`datei_schreiben_mit`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schreibfehler {
    /// The new file cannot be made, written or `fsync`ed; nothing changed.
    Datei,
    /// The step between the `fsync` and the `rename` failed; the new file is removed, nothing changed.
    Zwischen,
    /// The `rename` failed; the new file is removed, the old list stands.
    Umbenennen,
    /// The `fsync` of the folder after the `rename` failed; the new list stands.
    Ordner,
}

/// Writes the new list: a new file in the same folder, `fsync`, then `zwischen`, then `rename` over the old one,
/// `fsync` of the folder. `zwischen` is the place of the log line of the command: it comes after the new file is on
/// disk and before the list changes, so a line without effect is possible and an effect without a line is not.
fn datei_schreiben_mit(
    ordner: &OwnedFd,
    marken: &[Marke],
    zwischen: impl FnOnce() -> Result<(), Fehler>,
) -> Result<(), Schreibfehler> {
    let neu = format!("{MARKEN_DATEI}.{}.neu", std::process::id());
    let _ = rustix::fs::unlinkat(ordner, neu.as_str(), rustix::fs::AtFlags::empty());
    let fd = rustix::fs::openat(
        ordner,
        neu.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| Schreibfehler::Datei)?;
    let mut datei = File::from(fd);
    let ergebnis = datei
        .write_all(schreiben_marken(marken).as_bytes())
        .and_then(|()| datei.sync_all())
        .map_err(|_| Schreibfehler::Datei)
        .and_then(|()| zwischen().map_err(|_| Schreibfehler::Zwischen))
        .and_then(|()| {
            rustix::fs::renameat(ordner, neu.as_str(), ordner, MARKEN_DATEI)
                .map_err(|_| Schreibfehler::Umbenennen)
        });
    if ergebnis.is_err() {
        let _ = rustix::fs::unlinkat(ordner, neu.as_str(), rustix::fs::AtFlags::empty());
        return ergebnis;
    }
    rustix::fs::fsync(ordner.as_fd()).map_err(|_| Schreibfehler::Ordner)
}

/// The write of step 5b: no step in between; every fault is `lauf.ordner_marke_kaputt`.
fn datei_schreiben(ordner: &OwnedFd, marken: &[Marke]) -> Result<(), Fehler> {
    datei_schreiben_mit(ordner, marken, || Ok(())).map_err(kaputt)
}

/// The state folder with the lock held: the command `werkbank-ordnermarke` reads and writes the list through it. The
/// lock ends with the value.
#[derive(Debug)]
pub struct Zustand {
    ordner: OwnedFd,
    _sperre: Option<OwnedFd>,
}

impl Zustand {
    /// Opens the state folder (a real folder of the own user, mode 0700) and takes the exclusive lock, waiting at most
    /// `frist`.
    ///
    /// # Errors
    /// `lauf.ordner_marke_kaputt`.
    pub fn sperren(zustand: &Path, frist: Duration) -> Result<Zustand, Fehler> {
        let ordner = ordner_oeffnen(zustand)?;
        let sperre = sperren(&ordner, frist)?;
        Ok(Zustand {
            ordner,
            _sperre: Some(sperre),
        })
    }

    /// Opens the state folder without the lock, for a command that only reads: the list is replaced as a whole by a
    /// `rename`, so a read sees the old or the new list, never a mixture, and no file is made.
    ///
    /// # Errors
    /// `lauf.ordner_marke_kaputt`.
    pub fn ohne_sperre(zustand: &Path) -> Result<Zustand, Fehler> {
        Ok(Zustand {
            ordner: ordner_oeffnen(zustand)?,
            _sperre: None,
        })
    }

    /// Reads and checks the list: `ENOENT` is the empty list.
    ///
    /// # Errors
    /// `lauf.ordner_marke_kaputt` or `lauf.ordner_marke_kennung`.
    pub fn lesen(&self, klassen: &[String]) -> Result<Vec<Marke>, Fehler> {
        datei_lesen(&self.ordner, klassen)
    }

    /// Replaces the list: new file, `fsync`, `zwischen`, `rename` (see [`datei_schreiben_mit`]).
    ///
    /// # Errors
    /// The [`Schreibfehler`] that stopped it; with `Datei`, `Zwischen` and `Umbenennen` the old list stands.
    pub fn ersetzen(
        &self,
        marken: &[Marke],
        zwischen: impl FnOnce() -> Result<(), Fehler>,
    ) -> Result<(), Schreibfehler> {
        datei_schreiben_mit(&self.ordner, marken, zwischen)
    }
}

/// Step 5b: checks the marks for the request and writes the mark of the folder, all under the lock.
///
/// # Errors
/// `lauf.ordner_marke_kaputt`, `lauf.ordner_anderer_kunde` or `lauf.ordner_hoeher_markiert`.
pub fn pruefen_und_markieren(
    zustand: &Path,
    anfrage: Anfrage<'_>,
    klassen: &[String],
    frist: Duration,
) -> Result<(), Fehler> {
    let ordner = ordner_oeffnen(zustand)?;
    let _sperre = sperren(&ordner, frist)?;
    let marken = datei_lesen(&ordner, klassen)?;
    if let Some(neu) = entscheiden(marken, anfrage, klassen, &mut zufall_kennung)? {
        datei_schreiben(&ordner, &neu)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
