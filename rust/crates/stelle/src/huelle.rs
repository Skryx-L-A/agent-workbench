//! The shell around the core (contract 6, sections 13 and 14), Unix side: the installation configuration, one `peerkanal`
//! channel per kind of caller, the strict frame, the clock, the chain of the source `stelle` (first the lines, then state
//! and answer) and the store of contract 9 behind `person.pseudonym`.
//!
//! SECURITY: the kind of a caller is the kind of the channel on which `peerkanal` accepted it after checking its identity
//! against `erlaubt` of that kind; never a field of a frame. A channel that cannot be set up stops the whole point.

pub mod personen;
pub mod vorgabe;

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use formate::kette::{anhaengen_zeilen, kanonisch, pruefe_quelle};
use peerkanal::{Annahme, Erlaubnis, Kennung, Server, ServerVorgabe, SocketModus, UnixOrt};
use serde_json::{Map, Value, json};

use crate::{
    Antwort, Aufrufer, Code, Ereignis, PrueferH1, Vorgang, Zustand, entscheide, form, laden,
};

/// The largest frame and answer in bytes (section 13.3).
pub const RAHMEN_MAX: usize = 2_097_152;
/// The time a connection may take for one frame (section 14.1, point 5).
pub const RAHMEN_FRIST: Duration = Duration::from_secs(30);
/// The first line of the chain of a new source.
const NULL64: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The kinds with a channel, and their channel names (section 14.1, point 1).
pub const KANAELE: [(Aufrufer, &str); 3] = [
    (Aufrufer::Kern, "stelle-kern"),
    (Aufrufer::Hub, "stelle-hub"),
    (Aufrufer::Mensch, "stelle-mensch"),
];

/// The installation configuration (section 14.1, point 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Konfig {
    /// The folder of the sockets (0700, of the account of the point).
    pub ordner: PathBuf,
    /// The permissions per kind, in the order of [`KANAELE`].
    pub erlaubt: [Vec<Erlaubnis>; 3],
    /// The settings file of contract 2.
    pub einstellungen: PathBuf,
    /// The key file of the checker `h1` (32 bytes, 0600).
    pub schluessel_h1: PathBuf,
    /// The folder of the chain.
    pub protokoll: PathBuf,
    /// The folder of the store of contract 9.
    pub personenbezug: PathBuf,
    /// The default of the admins for workers (root's file).
    pub vorgabe: PathBuf,
}

fn pfad(o: &Map<String, Value>, k: &str) -> Option<PathBuf> {
    let t = o.get(k)?.as_str()?;
    let p = PathBuf::from(t);
    (p.is_absolute() && !t.contains('\0')).then_some(p)
}

fn erlaubnisse(wert: &Value) -> Option<Vec<Erlaubnis>> {
    let liste = wert.as_array().filter(|l| (1..=64).contains(&l.len()))?;
    liste
        .iter()
        .map(|e| {
            let e = e.as_object().filter(|e| e.len() == 1)?;
            let uid = u32::try_from(e.get("uid")?.as_u64()?).ok()?;
            Some(Erlaubnis::Unix { uid, gid: None })
        })
        .collect()
}

/// Reads the configuration strictly: exactly these keys, absolute paths, 1 to 64 permissions `{"uid": n}` per kind.
///
/// # Errors
/// `konfig_ungueltig` for any other form.
pub fn konfig_lesen(bytes: &[u8]) -> Result<Konfig, &'static str> {
    const FEHLER: &str = "konfig_ungueltig";
    let o = form::objekt_lesen(bytes).ok_or(FEHLER)?;
    let schluessel = [
        "einstellungen",
        "erlaubt",
        "fassung",
        "ordner",
        "personenbezug",
        "protokoll",
        "schluessel_h1",
        "vorgabe",
    ];
    if o.len() != schluessel.len()
        || !schluessel.iter().all(|k| o.contains_key(*k))
        || o["fassung"] != json!(1)
    {
        return Err(FEHLER);
    }
    let e = o["erlaubt"]
        .as_object()
        .filter(|e| e.len() == 3)
        .ok_or(FEHLER)?;
    let je = |k: &str| e.get(k).and_then(erlaubnisse).ok_or(FEHLER);
    Ok(Konfig {
        ordner: pfad(&o, "ordner").ok_or(FEHLER)?,
        erlaubt: [je("kern")?, je("hub")?, je("mensch")?],
        einstellungen: pfad(&o, "einstellungen").ok_or(FEHLER)?,
        schluessel_h1: pfad(&o, "schluessel_h1").ok_or(FEHLER)?,
        protokoll: pfad(&o, "protokoll").ok_or(FEHLER)?,
        personenbezug: pfad(&o, "personenbezug").ok_or(FEHLER)?,
        vorgabe: pfad(&o, "vorgabe").ok_or(FEHLER)?,
    })
}

/// The operation of a frame `{"vorgang": <name>, "eingabe": <value>}`, read strictly (section 13.3), or `None`.
#[must_use]
pub fn rahmen_lesen(bytes: &[u8]) -> Option<Vorgang> {
    if bytes.len() > RAHMEN_MAX {
        return None;
    }
    let mut o = form::objekt_lesen(bytes)?;
    if o.len() != 2 {
        return None;
    }
    let eingabe = o.remove("eingabe")?;
    let name = o.remove("vorgang")?;
    Vorgang::aus_name(name.as_str()?, eingabe)
}

/// The answer line of a refused frame (section 13.3): `stelle.eingabe_ungueltig`, no line in the chain.
#[must_use]
pub fn rahmenfehler() -> Vec<u8> {
    antwort_zeile(&Antwort::Nein(Code::EingabeUngueltig))
}

fn antwort_zeile(a: &Antwort) -> Vec<u8> {
    let mut z = kanonisch(&a.json()).unwrap_or_default().into_bytes();
    z.push(b'\n');
    z
}

/// The point: the state of the core and the last line of its chain.
pub struct Stelle {
    zustand: Zustand,
    protokoll: PathBuf,
    letzte: String,
}

impl Stelle {
    /// A point over a loaded state and the folder of its chain. The chain must be whole; its last line is the start
    /// (64 zeros for an empty one).
    ///
    /// # Errors
    /// `protokoll_gebrochen` when the chain has a break or cannot be read.
    pub fn neu(zustand: Zustand, protokoll: &Path) -> Result<Self, &'static str> {
        let p =
            pruefe_quelle(protokoll, "stelle", NULL64, None).map_err(|_| "protokoll_gebrochen")?;
        if !p.ok() {
            return Err("protokoll_gebrochen");
        }
        Ok(Self {
            zustand,
            protokoll: protokoll.to_path_buf(),
            letzte: p.letzte.unwrap_or_else(|| NULL64.to_owned()),
        })
    }

    /// One event (section 13.1): decide, write the lines in one append under the lock, and only then take the state and
    /// answer. A failed write keeps the old state and answers `stelle.protokoll_fehler`: there is no release then.
    pub fn ereignis(&mut self, aufrufer: Aufrufer, vorgang: Vorgang, jetzt_ms: i64) -> Vec<u8> {
        let e = Ereignis { aufrufer, vorgang };
        let r = entscheide(&self.zustand, &e, jetzt_ms);
        if !r.zeilen.is_empty() {
            let geschrieben = r.zeilen_texte(&self.letzte).ok().and_then(|texte| {
                anhaengen_zeilen(&self.protokoll, "stelle", &self.letzte, &texte).ok()
            });
            let Some(neu) = geschrieben else {
                return antwort_zeile(&Antwort::Nein(Code::ProtokollFehler));
            };
            self.letzte = neu;
        }
        let zeile = antwort_zeile(&r.antwort);
        self.zustand = r.zustand;
        zeile
    }
}

/// The time now in milliseconds since 1970, 0 when the clock lies before.
#[must_use]
pub fn jetzt_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

/// A file of the own user: read only, no link followed, closed on `exec`.
const EIGENE_DATEI: rustix::fs::OFlags = rustix::fs::OFlags::RDONLY
    .union(rustix::fs::OFlags::NOFOLLOW)
    .union(rustix::fs::OFlags::CLOEXEC);

/// Reads a small file of the own user that is closed to the group and others (the key of `h1`, the settings).
fn eigene_datei(pfad: &Path, max: usize) -> Result<Vec<u8>, &'static str> {
    use rustix::fs::{CWD, FileType, Mode, fstat, openat};
    let fd = openat(CWD, pfad, EIGENE_DATEI, Mode::empty()).map_err(|_| "datei_unsicher")?;
    let st = fstat(&fd).map_err(|_| "datei_unsicher")?;
    let euid = rustix::process::geteuid().as_raw();
    if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile
        || st.st_uid != euid
        || st.st_mode & 0o077 != 0
    {
        return Err("datei_unsicher");
    }
    let mut bytes = Vec::new();
    std::fs::File::from(fd)
        .take(u64::try_from(max).map_err(|_| "datei_unsicher")? + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "datei_unsicher")?;
    if bytes.len() > max {
        return Err("datei_unsicher");
    }
    Ok(bytes)
}

/// The key of `h1` for the service of the human (addendum of contract `dienst`, channel to the point, point 4): a regular file of
/// the own user with the mode exactly 0600 and exactly 32 bytes, opened without following a link and checked on the descriptor.
///
/// # Errors
/// `schluessel_fehlt` for every other file and every failure.
pub fn schluessel_lesen(pfad: &Path) -> Result<[u8; 32], &'static str> {
    use rustix::fs::{CWD, FileType, Mode, fstat, openat};
    const FEHLT: &str = "schluessel_fehlt";
    let fd = openat(CWD, pfad, EIGENE_DATEI, Mode::empty()).map_err(|_| FEHLT)?;
    let st = fstat(&fd).map_err(|_| FEHLT)?;
    if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile
        || st.st_uid != rustix::process::geteuid().as_raw()
        || st.st_mode & 0o7777 != 0o600
    {
        return Err(FEHLT);
    }
    let mut bytes = Vec::new();
    std::fs::File::from(fd)
        .take(33)
        .read_to_end(&mut bytes)
        .map_err(|_| FEHLT)?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| FEHLT)
}

/// Loads everything the point needs before a channel opens: settings, key, store, rules, chain. Any failure stops the
/// start (fail closed); the missing or unsafe default of the admins does not, then every `worker.anfordern` is refused.
///
/// # Errors
/// The code of the first failing part.
pub fn laden_aus(
    k: &Konfig,
    zufall: personen::Zufall,
    zweiter: Option<u32>,
) -> Result<Stelle, &'static str> {
    let jetzt = jetzt_ms();
    let einstellungen = eigene_datei(&k.einstellungen, 1_048_576)?;
    let schluessel: [u8; 32] = eigene_datei(&k.schluessel_h1, 32)?
        .try_into()
        .map_err(|_| "schluessel_ungueltig")?;
    let speicher = personen::Speicher::oeffnen(&k.personenbezug, zufall, jetzt)?;
    let regeln = vorgabe::lesen(&k.vorgabe, zweiter).and_then(|b| crate::worker_regeln_lesen(&b));
    let z = laden(&einstellungen, jetzt, Box::new(PrueferH1::neu(schluessel)))
        .map_err(Code::code)?
        .mit_personen(Arc::new(speicher))
        .mit_worker_regeln(regeln);
    Stelle::neu(z, &k.protokoll)
}

/// One request of a connection to the loop of the point.
pub struct Anfrage {
    /// The measured kind of the channel.
    pub aufrufer: Aufrufer,
    /// The operation of the frame.
    pub vorgang: Vorgang,
    /// The answer line goes back here.
    pub antwort: mpsc::Sender<Vec<u8>>,
}

/// Reads one frame up to LF, at most [`RAHMEN_MAX`] bytes; `None` at the end, on any error, on a frame too large.
fn zeile_lesen(leser: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut zeile = Vec::new();
    let grenze = u64::try_from(RAHMEN_MAX).ok()? + 1;
    let n = leser
        .by_ref()
        .take(grenze)
        .read_until(b'\n', &mut zeile)
        .ok()?;
    if n == 0 || zeile.pop() != Some(b'\n') {
        return None;
    }
    Some(zeile)
}

/// Serves one connection: frames in turn, each answered before the next is read. A frame error answers
/// `stelle.eingabe_ungueltig`, writes one line without content to stderr and closes; the end, an error or a frame
/// without LF in time close without an answer.
pub fn verbindung_bedienen<V: Read + Write>(
    verbindung: V,
    aufrufer: Aufrufer,
    schleife: &mpsc::Sender<Anfrage>,
) {
    let mut leser = BufReader::new(verbindung);
    while let Some(zeile) = zeile_lesen(&mut leser) {
        let Some(vorgang) = rahmen_lesen(&zeile) else {
            let _ = leser.get_mut().write_all(&rahmenfehler());
            #[allow(
                clippy::print_stderr,
                reason = "the operating log of the point is stderr (section 13.3)"
            )]
            {
                eprintln!("werkbank-stelle: rahmen abgewiesen");
            }
            break;
        };
        let (tx, rx) = mpsc::channel();
        if schleife
            .send(Anfrage {
                aufrufer,
                vorgang,
                antwort: tx,
            })
            .is_err()
        {
            break;
        }
        let Ok(antwort) = rx.recv() else { break };
        if leser.get_mut().write_all(&antwort).is_err() {
            break;
        }
    }
}

/// Sets up the three channels (section 14.1, point 3). Any failure stops the start.
///
/// # Errors
/// The code of `peerkanal` (`pk.ordner_rechte`, `pk.name_belegt`, …).
pub fn kanaele_anlegen(k: &Konfig) -> Result<Vec<(Aufrufer, Server)>, &'static str> {
    let mut server = Vec::new();
    for ((art, name), erlaubt) in KANAELE.iter().zip(k.erlaubt.iter()) {
        let kanal = Kennung::neu(name).map_err(|f| f.code.als_str())?;
        let mut v = ServerVorgabe::neu(kanal, erlaubt.clone(), 8);
        v.prozess_pflicht = true;
        v.unix = Some(UnixOrt {
            ordner: k.ordner.clone(),
            modus: SocketModus::M0600,
        });
        let mut s = Server::anlegen(v).map_err(|f| f.code.als_str())?;
        let kanalname = *name;
        // the event without identity: kind of the channel, code and reason of `peerkanal` (section 14.1, point 4)
        s.bei_abweisung(move |e| {
            #[allow(
                clippy::print_stderr,
                reason = "the operating log of the point is stderr (section 13.3)"
            )]
            {
                eprintln!(
                    "werkbank-stelle: {kanalname} abgewiesen: {} {}",
                    e.code().als_str(),
                    e.grund.als_text()
                );
            }
        });
        server.push((*art, s));
    }
    Ok(server)
}

/// Accepts connections of one channel for ever, each confirmed at once (the hook decides nothing more, section 14.1
/// point 4) and served in its own thread.
pub fn annehmen_schleife(server: &Server, aufrufer: Aufrufer, schleife: &mpsc::Sender<Anfrage>) {
    loop {
        let Ok(annahme) = server.annehmen() else {
            continue;
        };
        let Annahme::Vorlaeufig(v) = annahme else {
            continue;
        };
        let Ok(mut verbindung) = v.bestaetigen() else {
            continue;
        };
        verbindung.lesefrist(Some(RAHMEN_FRIST));
        let schleife = schleife.clone();
        std::thread::spawn(move || verbindung_bedienen(verbindung, aufrufer, &schleife));
    }
}

/// The clock of the point: `ablauf.pruefen` once a second.
const TAKT: Duration = Duration::from_secs(1);

/// The loop of the point: every request of every channel and the clock (`ablauf.pruefen` once a second), strictly in
/// turn over one state.
pub fn schleife(mut stelle: Stelle, rx: &mpsc::Receiver<Anfrage>) {
    let mut naechster_takt = std::time::Instant::now();
    loop {
        let warten = naechster_takt.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(warten) {
            Ok(a) => {
                let zeile = stelle.ereignis(a.aufrufer, a.vorgang, jetzt_ms());
                let _ = a.antwort.send(zeile);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = stelle.ereignis(
                    Aufrufer::Takt,
                    Vorgang::AblaufPruefen(json!({})),
                    jetzt_ms(),
                );
                let jetzt = std::time::Instant::now();
                naechster_takt = jetzt.checked_add(TAKT).unwrap_or(jetzt);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// The whole start: configuration, loading, channels, loop. Returns only on a failure of the start, with its exit code
/// (78 configuration, 74 a channel) and reason.
#[must_use]
pub fn laufen(konfig_pfad: &Path, zweiter: Option<u32>) -> (u8, &'static str) {
    let Ok(bytes) = std::fs::read(konfig_pfad) else {
        return (78, "konfig_unlesbar");
    };
    let k = match konfig_lesen(&bytes) {
        Ok(k) => k,
        Err(g) => return (78, g),
    };
    let stelle = match laden_aus(&k, personen::zufall_des_systems(), zweiter) {
        Ok(s) => s,
        Err(g) => return (78, g),
    };
    let server = match kanaele_anlegen(&k) {
        Ok(s) => s,
        Err(g) => return (74, g),
    };
    let (tx, rx) = mpsc::channel();
    for (art, s) in server {
        let s = Arc::new(s);
        let tx = tx.clone();
        std::thread::spawn(move || annehmen_schleife(&s, art, &tx));
    }
    drop(tx);
    schleife(stelle, &rx);
    (70, "schleife_beendet")
}

#[cfg(test)]
mod tests;
