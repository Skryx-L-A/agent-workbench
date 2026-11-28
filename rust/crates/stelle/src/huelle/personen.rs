//! The store of contract 9 (section 4), first stage: `oeffnen` and `person`, without text, information, deletion, key change
//! and comparison. Every access goes through descriptors relative to the open folder; no step checks a path text and opens
//! it afterwards. Any doubt is an error (fail closed): then the point does not start, or `person.pseudonym` gives no
//! pseudonym.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::sync::Mutex;

use formate::kette::kanonisch;
use rustix::fs::{
    AtFlags, CWD, FileType, Mode, OFlags, fstat, fsync, openat, renameat, statat, unlinkat,
};
use serde_json::{Map, Value, json};

use crate::Personenspeicher;
use crate::form;
use crate::pruefer::hmac_sha256;

/// The largest `zuordnung.json` in bytes.
pub const ZUORDNUNG_MAX: usize = 33_554_432;
/// The largest `schluessel.json` in bytes (many key changes, each about 100 bytes).
const SCHLUESSEL_MAX: usize = 1_048_576;

const ORDNER: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const LESEN: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
/// The lock file, made on the first opening of a store.
const SPERRE_NEU: OFlags = OFlags::RDWR
    .union(OFlags::CREATE)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
/// The lock file of an open store.
const SPERRE: OFlags = OFlags::RDWR.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC);
const NEU: OFlags = OFlags::WRONLY
    .union(OFlags::CREATE)
    .union(OFlags::EXCL)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// The errors of `oeffnen` (contract 9, section 4) and of a write.
pub mod fehler {
    /// The folder or a file is no folder or file, a link, of another owner, or open for the group or others.
    pub const UNSICHER: &str = "speicher_unsicher";
    /// The folder or one above it holds an entry `.git`.
    pub const IM_REPO: &str = "speicher_im_repo";
    /// Only `schluessel.json` is missing.
    pub const SCHLUESSEL_FEHLT: &str = "schluessel_fehlt";
    /// Only `zuordnung.json` is missing.
    pub const ZUORDNUNG_FEHLT: &str = "zuordnung_fehlt";
    /// A file is not canonical, not of the form, or violates a rule of the form.
    pub const UNGUELTIG: &str = "speicher_ungueltig";
    /// The store would grow beyond its limit.
    pub const VOLL: &str = "speicher_voll";
    /// The system failed (reading, writing, the lock, the randomness).
    pub const IO: &str = "speicher_io";
}

/// 32 random bytes, or `None` when the system gives none.
pub type Zufall = Box<dyn Fn() -> Option<[u8; 32]> + Send + Sync>;

/// The randomness of the system: 32 bytes from `/dev/urandom`, opened without following a link and checked as a character
/// device.
pub fn zufall_des_systems() -> Zufall {
    Box::new(|| {
        let fd = openat(CWD, "/dev/urandom", LESEN, Mode::empty()).ok()?;
        let st = fstat(&fd).ok()?;
        if FileType::from_raw_mode(st.st_mode) != FileType::CharacterDevice {
            return None;
        }
        let mut bytes = [0u8; 32];
        File::from(fd).read_exact(&mut bytes).ok()?;
        Some(bytes)
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut aus, b| {
        let _ = write!(aus, "{b:02x}");
        aus
    })
}

fn aus_hex(text: &str) -> Option<[u8; 32]> {
    if !form::hex(text, 64) {
        return None;
    }
    let mut aus = [0u8; 32];
    for (stelle, byte) in aus.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(stelle * 2..stelle * 2 + 2)?, 16).ok()?;
    }
    Some(aus)
}

/// The pseudonym of a secret under a key (contract 9, section 3).
#[must_use]
pub fn pseudonym(schluessel: &[u8; 32], geheimnis: &[u8; 32]) -> String {
    let mut nachricht = b"wb-person-pseudonym\n".to_vec();
    nachricht.extend_from_slice(geheimnis);
    let mac = hex(&hmac_sha256(schluessel, &nachricht));
    format!("ps_{}", &mac[..32])
}

/// A file of the own user, regular, closed to the group and others.
#[allow(
    clippy::verbose_bit_mask,
    reason = "the mask names the bits of the group and others"
)]
fn datei_gut(fd: &OwnedFd, euid: u32) -> bool {
    fstat(fd).is_ok_and(|st| {
        FileType::from_raw_mode(st.st_mode) == FileType::RegularFile
            && st.st_uid == euid
            && st.st_mode & 0o077 == 0
    })
}

/// Reads a file of the folder, or `Ok(None)` when it does not exist.
fn lesen(
    ordner: BorrowedFd<'_>,
    name: &str,
    max: usize,
    euid: u32,
) -> Result<Option<Vec<u8>>, &'static str> {
    let fd = match openat(ordner, name, LESEN, Mode::empty()) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => {
            // nothing there, or a dangling link: a link is never a missing file
            return match statat(ordner, name, AtFlags::SYMLINK_NOFOLLOW) {
                Err(rustix::io::Errno::NOENT) => Ok(None),
                _ => Err(fehler::UNSICHER),
            };
        }
        Err(_) => return Err(fehler::UNSICHER),
    };
    if !datei_gut(&fd, euid) {
        return Err(fehler::UNSICHER);
    }
    let mut bytes = Vec::new();
    let grenze = u64::try_from(max).map_err(|_| fehler::IO)? + 1;
    File::from(fd)
        .take(grenze)
        .read_to_end(&mut bytes)
        .map_err(|_| fehler::IO)?;
    if bytes.len() > max {
        return Err(fehler::UNGUELTIG);
    }
    Ok(Some(bytes))
}

/// Writes a file of the folder atomically: a new file with `O_EXCL`, everything, `fsync`, `rename`, `fsync` of the folder.
/// On any error the old file stays and the new one is removed.
fn ersetzen(
    ordner: BorrowedFd<'_>,
    name: &str,
    bytes: &[u8],
    zufall: &Zufall,
) -> Result<(), &'static str> {
    let marke = zufall().ok_or(fehler::IO)?;
    let neu = format!(".{name}.{}", hex(&marke[..8]));
    let fd =
        openat(ordner, neu.as_str(), NEU, Mode::from_raw_mode(0o600)).map_err(|_| fehler::IO)?;
    let mut datei = File::from(fd);
    let geschrieben = datei
        .write_all(bytes)
        .and_then(|()| datei.sync_all())
        .map_err(|_| fehler::IO)
        .and_then(|()| renameat(ordner, neu.as_str(), ordner, name).map_err(|_| fehler::IO));
    if geschrieben.is_err() {
        let _ = unlinkat(ordner, neu.as_str(), AtFlags::empty());
        return geschrieben;
    }
    fsync(ordner).map_err(|_| fehler::IO)
}

/// Whether a new `zuordnung.json` of `laenge` bytes fits the store (contract 9: at most [`ZUORDNUNG_MAX`]).
fn passt(laenge: usize) -> bool {
    laenge <= ZUORDNUNG_MAX
}

fn text(wert: &Value) -> Result<Vec<u8>, &'static str> {
    let mut t = kanonisch(wert).map_err(|_| fehler::UNGUELTIG)?.into_bytes();
    t.push(b'\n');
    Ok(t)
}

/// The bytes are exactly the canonical form of their value with one LF; then the value.
fn kanonisch_lesen(bytes: &[u8]) -> Result<Map<String, Value>, &'static str> {
    let objekt = form::objekt_lesen(bytes).ok_or(fehler::UNGUELTIG)?;
    if text(&Value::Object(objekt.clone()))? != bytes {
        return Err(fehler::UNGUELTIG);
    }
    Ok(objekt)
}

fn genau(objekt: &Map<String, Value>, namen: &[&str]) -> bool {
    objekt.len() == namen.len() && namen.iter().all(|n| objekt.contains_key(*n))
}

/// `schluessel.json`: the running key is the last; `ab_ms` rises strictly; no key twice.
fn schluessel_lesen(bytes: &[u8]) -> Result<[u8; 32], &'static str> {
    let o = kanonisch_lesen(bytes)?;
    let liste = o
        .get("schluessel")
        .and_then(Value::as_array)
        .filter(|_| genau(&o, &["fassung", "schluessel"]) && o["fassung"] == json!(1))
        .ok_or(fehler::UNGUELTIG)?;
    let mut vorher: Option<i64> = None;
    let mut gesehen = Vec::new();
    for e in liste {
        let e = e
            .as_object()
            .filter(|e| genau(e, &["ab_ms", "wert"]))
            .ok_or(fehler::UNGUELTIG)?;
        let ab = e["ab_ms"]
            .as_i64()
            .filter(|ab| vorher.is_none_or(|v| *ab > v))
            .ok_or(fehler::UNGUELTIG)?;
        let wert = e["wert"]
            .as_str()
            .and_then(aus_hex)
            .ok_or(fehler::UNGUELTIG)?;
        if gesehen.contains(&wert) {
            return Err(fehler::UNGUELTIG);
        }
        gesehen.push(wert);
        vorher = Some(ab);
    }
    gesehen.last().copied().ok_or(fehler::UNGUELTIG)
}

/// The state of `zuordnung.json` and the secrets by identifier.
type Zuordnung = (Map<String, Value>, BTreeMap<String, [u8; 32]>);

/// `zuordnung.json`: each person with a secret (unique), `seit_ms` and `texte`; the secrets by identifier.
fn zuordnung_lesen(bytes: &[u8]) -> Result<Zuordnung, &'static str> {
    let o = kanonisch_lesen(bytes)?;
    let personen = o
        .get("personen")
        .and_then(Value::as_object)
        .filter(|_| genau(&o, &["fassung", "personen"]) && o["fassung"] == json!(1))
        .ok_or(fehler::UNGUELTIG)?;
    let mut geheimnisse = BTreeMap::new();
    for (kennung, p) in personen {
        let p = p
            .as_object()
            .filter(|p| genau(p, &["geheimnis", "seit_ms", "texte"]) && form::kennung(kennung))
            .ok_or(fehler::UNGUELTIG)?;
        let geheimnis = p["geheimnis"]
            .as_str()
            .and_then(aus_hex)
            .ok_or(fehler::UNGUELTIG)?;
        if p["seit_ms"].as_i64().is_none() || !p["texte"].is_object() {
            return Err(fehler::UNGUELTIG);
        }
        if geheimnisse.values().any(|g| *g == geheimnis) {
            return Err(fehler::UNGUELTIG);
        }
        geheimnisse.insert(kennung.clone(), geheimnis);
    }
    Ok((o, geheimnisse))
}

/// The folder and every folder above it, on the resolved path, hold no entry `.git`.
fn ausserhalb_eines_repos(ordner: &Path) -> Result<(), &'static str> {
    let echt = ordner.canonicalize().map_err(|_| fehler::UNSICHER)?;
    for stufe in echt.ancestors() {
        match statat(CWD, stufe.join(".git"), AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => {}
            _ => return Err(fehler::IM_REPO),
        }
    }
    Ok(())
}

/// The open store.
pub struct Speicher {
    ordner: OwnedFd,
    schluessel: [u8; 32],
    euid: u32,
    zufall: Zufall,
    /// One change at a time in this process; between processes the lock on `zuordnung.sperre`.
    sperre: Mutex<()>,
}

impl Speicher {
    /// `oeffnen(ordner, zufall, jetzt_ms)` of contract 9, section 4, in the order of its table.
    ///
    /// # Errors
    /// One of the codes in [`fehler`].
    pub fn oeffnen(ordner: &Path, zufall: Zufall, jetzt_ms: i64) -> Result<Self, &'static str> {
        let euid = rustix::process::geteuid().as_raw();
        let fd = openat(CWD, ordner, ORDNER, Mode::empty()).map_err(|_| fehler::UNSICHER)?;
        let st = fstat(&fd).map_err(|_| fehler::UNSICHER)?;
        if st.st_uid != euid || st.st_mode & 0o077 != 0 {
            return Err(fehler::UNSICHER);
        }
        ausserhalb_eines_repos(ordner)?;
        let schluessel = lesen(fd.as_fd(), "schluessel.json", SCHLUESSEL_MAX, euid)?;
        let zuordnung = lesen(fd.as_fd(), "zuordnung.json", ZUORDNUNG_MAX, euid)?;
        let schluessel = match (schluessel, zuordnung) {
            (None, None) => {
                let wert = zufall().ok_or(fehler::IO)?;
                let s =
                    json!({"fassung": 1, "schluessel": [{"ab_ms": jetzt_ms, "wert": hex(&wert)}]});
                ersetzen(fd.as_fd(), "schluessel.json", &text(&s)?, &zufall)?;
                let z = json!({"fassung": 1, "personen": {}});
                ersetzen(fd.as_fd(), "zuordnung.json", &text(&z)?, &zufall)?;
                wert
            }
            (None, Some(_)) => return Err(fehler::SCHLUESSEL_FEHLT),
            (Some(_), None) => return Err(fehler::ZUORDNUNG_FEHLT),
            (Some(s), Some(z)) => {
                let wert = schluessel_lesen(&s)?;
                zuordnung_lesen(&z)?;
                wert
            }
        };
        let sperre = openat(
            fd.as_fd(),
            "zuordnung.sperre",
            SPERRE_NEU,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| fehler::UNSICHER)?;
        if !datei_gut(&sperre, euid) {
            return Err(fehler::UNSICHER);
        }
        Ok(Self {
            ordner: fd,
            schluessel,
            euid,
            zufall,
            sperre: Mutex::new(()),
        })
    }

    /// `person(kennung, zufall, jetzt_ms)`: the pseudonym of a known person, or a new person with a new secret.
    ///
    /// # Errors
    /// `feld_ungueltig:kennung`, `geheimnis_doppelt`, `speicher_voll` and the codes in [`fehler`].
    pub fn person(&self, kennung: &str, jetzt_ms: i64) -> Result<(String, bool), &'static str> {
        if !form::kennung(kennung) {
            return Err("feld_ungueltig:kennung");
        }
        let _innen = self.sperre.lock().map_err(|_| fehler::IO)?;
        let datei = openat(
            self.ordner.as_fd(),
            "zuordnung.sperre",
            SPERRE,
            Mode::empty(),
        )
        .map_err(|_| fehler::UNSICHER)?;
        if !datei_gut(&datei, self.euid) {
            return Err(fehler::UNSICHER);
        }
        let datei = File::from(datei);
        datei.lock().map_err(|_| fehler::IO)?;
        let bytes = lesen(
            self.ordner.as_fd(),
            "zuordnung.json",
            ZUORDNUNG_MAX,
            self.euid,
        )?
        .ok_or(fehler::ZUORDNUNG_FEHLT)?;
        let (mut stand, geheimnisse) = zuordnung_lesen(&bytes)?;
        if let Some(g) = geheimnisse.get(kennung) {
            return Ok((pseudonym(&self.schluessel, g), false));
        }
        let geheimnis = (self.zufall)().ok_or(fehler::IO)?;
        if geheimnisse.values().any(|g| *g == geheimnis) {
            return Err("geheimnis_doppelt");
        }
        stand
            .get_mut("personen")
            .and_then(Value::as_object_mut)
            .ok_or(fehler::UNGUELTIG)?
            .insert(
                kennung.to_owned(),
                json!({"geheimnis": hex(&geheimnis), "seit_ms": jetzt_ms, "texte": {}}),
            );
        let neu = text(&Value::Object(stand))?;
        if !passt(neu.len()) {
            return Err(fehler::VOLL);
        }
        ersetzen(self.ordner.as_fd(), "zuordnung.json", &neu, &self.zufall)?;
        Ok((pseudonym(&self.schluessel, &geheimnis), true))
    }
}

impl Personenspeicher for Speicher {
    fn pseudonym(&self, kennung: &str, jetzt_ms: i64) -> Option<String> {
        self.person(kennung, jetzt_ms).ok().map(|(ps, _)| ps)
    }
}

#[cfg(test)]
mod tests;
