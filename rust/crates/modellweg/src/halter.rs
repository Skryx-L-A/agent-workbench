//! The holder client of mode 2 (contract 8, section 5.1; holder contract, sections 3 to 5 and 7). For a bound target
//! the model path holds no token: it hands the checked request to the holder as `modell.aufruf` or `modell.strom` and
//! builds the answer of the harness out of `beginn`, `teil` and `ende`.
//!
//! The transport is one line of UTF-8 per request and per answer, closed with LF, the bytes in RFC 8785 in both
//! directions. There is no hello, one connection carries one request at a time, and the holder keeps the way out: the
//! model path opens no connection to a bound target, resolves no name and ends no TLS towards it.
//!
//! The endpoint is a fixed path ([`fester_pfad`]); it is never an option and never comes from the caller. The lines
//! themselves ([`modell_aufruf`] and the others) and the reading of an answer ([`Zeile`]) hold no I/O, so they are
//! checked on every system; the connection with the check of the counterparty is Linux, like mode 2 itself.

use std::io::{self, Read, Write};
#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::b64;
use crate::json::lies_streng;

/// Most payload bytes of one line `teil` or `strom.teil` (holder contract, section 4).
pub const TEIL_MAX: usize = 65_536;
/// Largest raw body of a request to a bound target (holder contract, section 4, H6): 23 MiB.
pub const KOERPER_MAX: u64 = 24_117_248;
/// Largest line in bytes without the LF (holder contract, section 3).
pub const ZEILE_MAX: usize = 33_554_432;
/// How long the model path waits for the line `beginn` (contract 8, section 5.1, H12).
pub const FRIST_KOPF: Duration = Duration::from_secs(600);
/// Most headers of a request and of an answer (holder contract, sections 4 and 5).
pub const KOPF_MAX: usize = 64;
/// Longest value of one header (holder contract, sections 4 and 5).
pub const WERT_MAX: usize = 8192;

/// The fixed path of the holder socket for `uid` (contract 8, section 5.1 and section 3a, duty 2).
///
/// `XDG_RUNTIME_DIR` is not read: the variable comes from the environment of the caller.
#[must_use]
pub fn fester_pfad(uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{uid}/wb-halter/halter.sock"))
}

/// The own code for a code of the holder (contract 8, section 5.1): `halter.ziel_komprimiert` becomes
/// `mw.antwort_komprimiert`, `halter.kein_schluessel` becomes `mw.halter_kein_schluessel`, every other code becomes
/// `mw.halter_fehler`. The code of the holder never goes to the harness.
#[must_use]
pub fn auf_mw(code: &str) -> &'static str {
    match code {
        "halter.ziel_komprimiert" => "mw.antwort_komprimiert",
        "halter.kein_schluessel" => "mw.halter_kein_schluessel",
        _ => "mw.halter_fehler",
    }
}

/// A header of a request to the holder: the name written small, the value as sent.
pub type Kopf = (String, String);

/// Whether the headers keep the form the holder takes (holder contract, section 4): at most 64 entries, every name
/// `^[a-z0-9-]{1,64}$` and every value printable ASCII without CR and LF, at most 8192 characters.
#[must_use]
pub fn kopf_ok(kopf: &[Kopf]) -> bool {
    kopf.len() <= KOPF_MAX
        && kopf.iter().all(|(n, w)| {
            (1..=64).contains(&n.len())
                && n.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                && w.len() <= WERT_MAX
                && w.bytes().all(|c| (0x20..=0x7e).contains(&c))
        })
}

/// Whether `abfrage` keeps the form of the field `abfrage` (holder contract, section 4; contract 8, section 5.1):
/// 1 to 1024 characters of the set, never a `#`.
#[must_use]
pub fn abfrage_ok(abfrage: &str) -> bool {
    (1..=1024).contains(&abfrage.len())
        && abfrage
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._~!$&'()*+,;=:@/?%-".contains(&c))
}

/// The headers as the object `kopf` of a line.
fn kopf_wert(kopf: &[Kopf]) -> Value {
    let mut o = Map::new();
    for (n, w) in kopf {
        o.insert(n.clone(), Value::String(w.clone()));
    }
    Value::Object(o)
}

/// One line in RFC 8785; an error can only come from a value the builders never make.
fn zeile(wert: &Value) -> Result<String, &'static str> {
    formate::kette::kanonisch(wert).map_err(|_| "mw.halter_fehler")
}

/// The line `modell.aufruf` (holder contract, section 4): path and query apart, the headers without any name the
/// holder forbids, the body as `koerper_b64` in one piece. The holder sets the header with the token itself.
///
/// # Errors
/// `mw.halter_fehler` if a value does not keep its form; the caller checks the form of a request before (contract 8,
/// section 13.2), so this is the closed door behind it.
pub fn modell_aufruf(
    id: u64,
    ziel: &str,
    methode: &str,
    pfad: &str,
    abfrage: Option<&str>,
    kopf: &[Kopf],
    koerper: &[u8],
) -> Result<String, &'static str> {
    if !(kopf_ok(kopf) && abfrage.is_none_or(abfrage_ok)) {
        return Err("mw.halter_fehler");
    }
    if u64::try_from(koerper.len()).unwrap_or(u64::MAX) > KOERPER_MAX {
        return Err("mw.halter_fehler");
    }
    zeile(&json!({
        "abfrage": abfrage,
        "fassung": 1,
        "id": id,
        "koerper_b64": b64::kodieren(koerper),
        "kopf": kopf_wert(kopf),
        "methode": methode,
        "op": "modell.aufruf",
        "pfad": pfad,
        "ziel": ziel,
    }))
}

/// The line `modell.strom` for an upgrade (holder contract, section 4): no method and no body; the holder sets
/// `upgrade`, `connection`, `accept-encoding: identity` and the header with the token itself.
///
/// # Errors
/// `mw.halter_fehler` if a value does not keep its form.
pub fn modell_strom(
    id: u64,
    ziel: &str,
    pfad: &str,
    abfrage: Option<&str>,
    kopf: &[Kopf],
) -> Result<String, &'static str> {
    if !(kopf_ok(kopf) && abfrage.is_none_or(abfrage_ok)) {
        return Err("mw.halter_fehler");
    }
    zeile(&json!({
        "abfrage": abfrage,
        "fassung": 1,
        "id": id,
        "kopf": kopf_wert(kopf),
        "op": "modell.strom",
        "pfad": pfad,
        "ziel": ziel,
    }))
}

/// The line `strom.teil`: the bytes of the harness in an open stream, at most `TEIL_MAX` per line.
///
/// # Errors
/// `mw.halter_fehler` for more than `TEIL_MAX` bytes.
pub fn strom_teil(id: u64, daten: &[u8]) -> Result<String, &'static str> {
    if daten.len() > TEIL_MAX {
        return Err("mw.halter_fehler");
    }
    zeile(&json!({"daten_b64": b64::kodieren(daten), "fassung": 1, "id": id, "op": "strom.teil"}))
}

/// The line `strom.ende`: the model path ends the stream.
///
/// # Errors
/// `mw.halter_fehler` (never reached for this line).
pub fn strom_ende(id: u64) -> Result<String, &'static str> {
    zeile(&json!({"fassung": 1, "id": id, "op": "strom.ende"}))
}

/// The line `modell.suche` (contract 8, section 9a, step 3a): the paths of the call for the secret of `ziel`.
///
/// # Errors
/// `mw.halter_fehler` for no path, more than 16 or a path outside the form.
pub fn modell_suche(id: u64, ziel: &str, pfade: &[String]) -> Result<String, &'static str> {
    let form_ok = |p: &String| {
        (1..=4096).contains(&p.len())
            && p.starts_with('/')
            && !p.bytes().any(|c| c <= 0x1f)
            && !p.split('/').any(|s| s == "." || s == "..")
    };
    if !((1..=16).contains(&pfade.len()) && pfade.iter().all(form_ok)) {
        return Err("mw.halter_fehler");
    }
    zeile(&json!({"fassung": 1, "id": id, "op": "modell.suche", "pfade": pfade, "ziel": ziel}))
}

/// The line `schluessel.vorhanden` for the bound target `ziel` (holder contract, section 4): the field `ziel`, never
/// `name`. The holder takes the name of the secret from `schluessel` of the target in his `halter.json`; the model path
/// does not know it (contract 8, section 5.1).
///
/// # Errors
/// `mw.halter_fehler` (never reached for this line).
pub fn schluessel_vorhanden(id: u64, ziel: &str) -> Result<String, &'static str> {
    zeile(&json!({"fassung": 1, "id": id, "op": "schluessel.vorhanden", "ziel": ziel}))
}

/// How long step 3 of the start waits for all answers of `schluessel.vorhanden` together (contract 8, section 13.5):
/// 5 s. "Lauf starten" waits 10 s for the ready line (contract 1, step 7a); a longer wait here would end unseen.
pub const FRIST_START: Duration = Duration::from_secs(5);

/// Whether `code` has the form `^halter\.[a-z_]{1,40}$`: the only form of a code of the holder that the start may
/// repeat on stderr (contract 8, section 5.1). Nothing else of an answer goes there.
#[must_use]
pub fn halter_code_form(code: &str) -> bool {
    code.strip_prefix("halter.").is_some_and(|rest| {
        (1..=40).contains(&rest.len()) && rest.bytes().all(|c| c.is_ascii_lowercase() || c == b'_')
    })
}

/// The refusal of step 3 for one target: the code of the model path and, if the holder answered with a code of the
/// form [`halter_code_form`], that code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Absage {
    /// The code of the model path (`mw.halter_*`).
    pub code: &'static str,
    /// The code of the holder, only if the holder answered with one of the closed form; never any other part of the
    /// answer.
    pub halter_code: Option<String>,
}

impl From<&'static str> for Absage {
    fn from(code: &'static str) -> Self {
        Self {
            code,
            halter_code: None,
        }
    }
}

/// Step 3 of the start for one bound target (contract 8, section 13.5): asks the holder on `k` with
/// `schluessel.vorhanden` and `ziel`, and waits for the answer at most until `bis`. Fail closed: only the answer
/// `{"vorhanden": true}` lets the start go on.
///
/// # Errors
/// `mw.halter_kein_schluessel` for `{"vorhanden": false}` and for `halter.ziel_unbekannt`; every other code of the
/// holder after [`auf_mw`]; `mw.halter_frist` if no answer came before `bis`; `mw.halter_fehler` for a torn
/// connection, a line outside the closed form or an answer without `vorhanden`.
pub fn schluessel_da(
    k: &mut Klient,
    ziel: &str,
    bis: std::time::Instant,
) -> Result<(), &'static str> {
    schluessel_da_mit_code(k, ziel, bis).map_err(|a| a.code)
}

/// [`schluessel_da`] with the code of the holder: if the holder answered with a code, the refusal carries it, but only
/// in the form [`halter_code_form`]. For a torn connection, an unreadable line, an answer without `vorhanden`,
/// `{"vorhanden": false}` and a deadline there is no code of the holder.
///
/// # Errors
/// As [`schluessel_da`].
pub fn schluessel_da_mit_code(
    k: &mut Klient,
    ziel: &str,
    bis: std::time::Instant,
) -> Result<(), Absage> {
    let rest = bis
        .checked_duration_since(std::time::Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or("mw.halter_frist")?;
    k.frist(Some(rest)).map_err(|_| "mw.halter_fehler")?;
    let id = k.naechste_id();
    k.senden(&schluessel_vorhanden(id, ziel)?)?;
    match k.naechste_zeile()? {
        Zeile::Antwort {
            code: None,
            vorhanden: Some(true),
            ..
        } => Ok(()),
        Zeile::Antwort {
            code: None,
            vorhanden: Some(false),
            ..
        } => Err("mw.halter_kein_schluessel".into()),
        Zeile::Antwort { code: Some(c), .. } => Err(Absage {
            // An unknown target is an error of the configuration, not a missing key; the start ends the same way.
            code: if c == "halter.ziel_unbekannt" {
                "mw.halter_kein_schluessel"
            } else {
                auf_mw(&c)
            },
            halter_code: halter_code_form(&c).then_some(c),
        }),
        _ => Err("mw.halter_fehler".into()),
    }
}

/// The four counts of an answer `suche` (holder contract, section 4; contract 8, section 9a, S5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Suche {
    /// Regular files read.
    pub dateien: u64,
    /// Files and folders that could not be read.
    pub nicht_lesbar: u64,
    /// Occurrences found.
    pub treffer: u64,
    /// Entries skipped (link, other kind of file).
    pub uebersprungen: u64,
}

/// One line of the holder (holder contract, section 5). The form is closed; a line outside it is not readable, and
/// the model path answers `mw.halter_fehler` (contract 8, section 5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zeile {
    /// `beginn`: the status and the headers of the target after the block list of the holder.
    Beginn {
        /// 100 to 599; 101 only for `modell.strom`.
        status: u16,
        /// The headers, in their order.
        kopf: Vec<Kopf>,
    },
    /// `teil`: payload bytes of the target.
    Teil(Vec<u8>),
    /// `ende`: the end of a stream, with the code of the holder if it failed.
    Ende(Option<String>),
    /// `antwort` of a call without a stream: the code if it failed, else the counts of a search.
    Antwort {
        /// The code of the holder, `None` if the call succeeded.
        code: Option<String>,
        /// The counts of `modell.suche`.
        suche: Option<Suche>,
        /// The answer of `schluessel.vorhanden`.
        vorhanden: Option<bool>,
    },
}

impl Zeile {
    /// Reads one line of the holder; `None` for a line outside the closed form.
    #[must_use]
    pub fn lies(bytes: &[u8]) -> Option<Self> {
        let Value::Object(o) = lies_streng(bytes)? else {
            return None;
        };
        if o.get("fassung") != Some(&json!(1)) {
            return None;
        }
        let ok = o.get("ok").and_then(Value::as_bool);
        let code = o.get("code").and_then(Value::as_str).map(str::to_owned);
        match o.get("art").and_then(Value::as_str)? {
            "beginn" => {
                let status = u16::try_from(o.get("status")?.as_u64()?).ok()?;
                let mut kopf = Vec::new();
                for (n, w) in o.get("kopf")?.as_object()? {
                    kopf.push((n.clone(), w.as_str()?.to_owned()));
                }
                // Fail closed at the reading: these headers go to the harness. A name or a value with CR, LF or a
                // control character would hang an own header line into the answer there, and a name with a `:` or
                // white space would turn a value into a name. The holder sends nothing of that kind (his contract
                // allows printable ASCII without CR and LF, at most 64 headers, each value up to 8192 characters),
                // but the model path does not rely on it: a line outside this form is not readable and ends the
                // request.
                let kopf_gut = kopf.len() <= KOPF_MAX
                    && kopf.iter().all(|(n, w)| {
                        (1..=64).contains(&n.len())
                            && n.bytes().all(|c| (0x21..=0x7e).contains(&c) && c != b':')
                            && w.len() <= WERT_MAX
                            && w.bytes().all(|c| (0x20..=0x7e).contains(&c))
                    });
                ((100..=599).contains(&status) && kopf_gut).then_some(Self::Beginn { status, kopf })
            }
            "teil" => {
                let daten = b64::dekodieren(o.get("daten_b64")?.as_str()?)?;
                (daten.len() <= TEIL_MAX).then_some(Self::Teil(daten))
            }
            "ende" => match (ok, &code) {
                (Some(true), None) => Some(Self::Ende(None)),
                (Some(false), Some(_)) => Some(Self::Ende(code)),
                _ => None,
            },
            "antwort" => {
                let suche = match o.get("suche") {
                    None => None,
                    Some(w) => {
                        let w = w.as_object()?;
                        let zahl = |n: &str| w.get(n).and_then(Value::as_u64);
                        Some(Suche {
                            dateien: zahl("dateien")?,
                            nicht_lesbar: zahl("nicht_lesbar")?,
                            treffer: zahl("treffer")?,
                            uebersprungen: zahl("uebersprungen")?,
                        })
                    }
                };
                let vorhanden = o.get("vorhanden").map(Value::as_bool);
                let vorhanden = match vorhanden {
                    None => None,
                    Some(w) => Some(w?),
                };
                match (ok, &code) {
                    (Some(true), None) => Some(Self::Antwort {
                        code: None,
                        suche,
                        vorhanden,
                    }),
                    (Some(false), Some(_)) => Some(Self::Antwort {
                        code,
                        suche: None,
                        vorhanden: None,
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// One connection to the holder. On a connection one request runs at a time: the next line goes out only after the
/// last line of the answer before it (contract 8, section 5.1). A stream keeps its connection until `ende`.
pub struct Klient {
    strom: std::os::unix::net::UnixStream,
    vorrat: Vec<u8>,
    /// How far `vorrat` was already searched for the LF. Without it every read would search the whole store again,
    /// and a holder that sends a line of many bytes without its LF would cost the model path time by the square of
    /// the bytes (measured at the build: 32 MiB took 171 s with the search from the start, under a second with it).
    gesucht: usize,
    naechste_id: u64,
}

impl Klient {
    /// A client on an open connection. The check of the counterparty belongs to `Klient::verbinde` (Linux), which is the
    /// only way the program itself takes.
    #[must_use]
    pub fn auf(strom: std::os::unix::net::UnixStream) -> Self {
        Self {
            strom,
            vorrat: Vec::new(),
            gesucht: 0,
            naechste_id: 1,
        }
    }

    /// Connects to the holder at `pfad` and checks the counterparty before the first line (contract 8, section 5.1):
    /// the folder is 0700 and the socket 0600 of the own uid, and the uid of the server out of `SO_PEERCRED` is the
    /// own one. There is no hello.
    ///
    /// # Errors
    /// `mw.halter_fehlt` if the endpoint does not answer, `mw.halter_unecht` if the check of the counterparty fails.
    #[cfg(target_os = "linux")]
    pub fn verbinde(pfad: &Path, eigene_uid: u32) -> Result<Self, &'static str> {
        use std::os::unix::fs::MetadataExt;

        let ordner = pfad.parent().ok_or("mw.halter_unecht")?;
        let eigen =
            |m: &std::fs::Metadata, modus: u32| m.uid() == eigene_uid && m.mode() & 0o7777 == modus;
        let o = std::fs::symlink_metadata(ordner).map_err(|_| "mw.halter_fehlt")?;
        if !(o.is_dir() && eigen(&o, 0o700)) {
            return Err("mw.halter_unecht");
        }
        let s = std::fs::symlink_metadata(pfad).map_err(|_| "mw.halter_fehlt")?;
        if !eigen(&s, 0o600) {
            return Err("mw.halter_unecht");
        }
        let strom = std::os::unix::net::UnixStream::connect(pfad).map_err(|_| "mw.halter_fehlt")?;
        let cred = rustix::net::sockopt::socket_peercred(&strom).map_err(|_| "mw.halter_unecht")?;
        if cred.uid.as_raw() != eigene_uid {
            return Err("mw.halter_unecht");
        }
        Ok(Self::auf(strom))
    }

    /// The `id` of the next request; it comes back unchanged in every line of its answer.
    pub fn naechste_id(&mut self) -> u64 {
        let id = self.naechste_id;
        self.naechste_id = self.naechste_id.saturating_add(1);
        id
    }

    /// Sets how long a read of an answer waits.
    ///
    /// # Errors
    /// The error of the socket.
    pub fn frist(&self, frist: Option<Duration>) -> io::Result<()> {
        self.strom.set_read_timeout(frist)
    }

    /// Sends one line and its LF.
    ///
    /// # Errors
    /// `mw.halter_fehler`: the connection broke.
    pub fn senden(&mut self, zeile: &str) -> Result<(), &'static str> {
        let fehler = |_| "mw.halter_fehler";
        self.strom.write_all(zeile.as_bytes()).map_err(fehler)?;
        self.strom.write_all(b"\n").map_err(fehler)?;
        self.strom.flush().map_err(fehler)
    }

    /// Reads the next line of the holder; `Ok(None)` when the deadline of the read passed without a line. The bridge
    /// of a stream polls both ways with a short deadline and takes `None` as „nothing yet".
    ///
    /// # Errors
    /// `mw.halter_fehler` for a torn connection, a line above `ZEILE_MAX` or a line outside the closed form.
    pub fn zeile_oder_nichts(&mut self) -> Result<Option<Zeile>, &'static str> {
        loop {
            // Only the bytes that came after the last search are searched again.
            if let Some(i) = self.vorrat[self.gesucht..]
                .iter()
                .position(|c| *c == b'\n')
                .map(|i| self.gesucht + i)
            {
                let zeile: Vec<u8> = self.vorrat.drain(..=i).collect();
                self.gesucht = 0;
                return Zeile::lies(&zeile[..i]).map(Some).ok_or("mw.halter_fehler");
            }
            self.gesucht = self.vorrat.len();
            if self.vorrat.len() > ZEILE_MAX {
                return Err("mw.halter_fehler");
            }
            let mut block = [0_u8; 16_384];
            match self.strom.read(&mut block) {
                Ok(0) => return Err("mw.halter_fehler"),
                Ok(n) => self.vorrat.extend_from_slice(&block[..n]),
                Err(e) => match lesefolge(&e) {
                    Lesefolge::Nichts => return Ok(None),
                    Lesefolge::Nochmal => {}
                    Lesefolge::Fehler => return Err("mw.halter_fehler"),
                },
            }
        }
    }
}

impl Klient {
    /// Reads the next line of the holder and takes a passed deadline as a failure of the holder.
    ///
    /// # Errors
    /// `mw.halter_frist` if nothing came in time, else as [`Klient::zeile_oder_nichts`].
    pub fn naechste_zeile(&mut self) -> Result<Zeile, &'static str> {
        self.zeile_oder_nichts()?.ok_or("mw.halter_frist")
    }
}

/// Whether a read ended because its deadline passed.
fn frist_vorbei(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// What a failed read means for the reader of a line.
#[derive(Debug, PartialEq, Eq)]
enum Lesefolge {
    /// The deadline passed; no line came, and the caller decides whether it waits again.
    Nichts,
    /// A signal broke the read; the same read is taken up again.
    Nochmal,
    /// The connection is unusable; the request ends.
    Fehler,
}

/// Reads the meaning of a failed read. It stands apart from the loop so that every one of the three ways is pinned by
/// a test: a read error other than a deadline is not producible on a Unix socket of a test.
fn lesefolge(e: &io::Error) -> Lesefolge {
    if frist_vorbei(e) {
        Lesefolge::Nichts
    } else if e.kind() == io::ErrorKind::Interrupted {
        Lesefolge::Nochmal
    } else {
        Lesefolge::Fehler
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::time::Duration;

    use super::{
        Klient, Lesefolge, Suche, Zeile, abfrage_ok, auf_mw, fester_pfad, kopf_ok, lesefolge,
        modell_aufruf, modell_strom, modell_suche, schluessel_vorhanden, strom_ende, strom_teil,
    };

    fn kopf(paare: &[(&str, &str)]) -> Vec<(String, String)> {
        paare
            .iter()
            .map(|(n, w)| ((*n).to_owned(), (*w).to_owned()))
            .collect()
    }

    /// Every one of the three ways of a failed read is pinned here, because only a deadline is producible over a
    /// socket of a test. A deadline means „nothing yet", a signal means „read again", every other error ends the
    /// request.
    #[test]
    fn a_failed_read_is_read_as_a_deadline_a_signal_or_the_end() {
        for art in [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut] {
            assert_eq!(
                lesefolge(&io::Error::from(art)),
                Lesefolge::Nichts,
                "{art:?}"
            );
        }
        assert_eq!(
            lesefolge(&io::Error::from(io::ErrorKind::Interrupted)),
            Lesefolge::Nochmal
        );
        for art in [
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::UnexpectedEof,
            io::ErrorKind::Other,
        ] {
            assert_eq!(
                lesefolge(&io::Error::from(art)),
                Lesefolge::Fehler,
                "{art:?}"
            );
        }
    }

    #[test]
    fn the_endpoint_is_a_fixed_path_without_the_variable() {
        assert_eq!(
            fester_pfad(1000).to_str(),
            Some("/run/user/1000/wb-halter/halter.sock")
        );
        assert_eq!(
            fester_pfad(0).to_str(),
            Some("/run/user/0/wb-halter/halter.sock")
        );
    }

    #[test]
    fn every_code_of_the_holder_gets_an_own_code() {
        assert_eq!(auf_mw("halter.ziel_komprimiert"), "mw.antwort_komprimiert");
        assert_eq!(
            auf_mw("halter.kein_schluessel"),
            "mw.halter_kein_schluessel"
        );
        for fremd in [
            "halter.pfad_verboten",
            "halter.kopf_verboten",
            "halter.ziel_unbekannt",
            "halter.rolle",
            "halter.peer",
            "halter.person",
            "halter.strom",
            "halter.wert_form",
            "halter.zu_gross",
            "halter.nicht_kanonisch",
            "halter.ziel_fehler",
            "halter.leck_gesperrt",
            "halter.intern",
            "halter.erfunden",
        ] {
            assert_eq!(auf_mw(fremd), "mw.halter_fehler", "{fremd}");
        }
    }

    #[test]
    fn the_line_of_a_call_is_canonical_and_keeps_path_and_query_apart() {
        let z = modell_aufruf(
            7,
            "api.github.com",
            "GET",
            "/copilot_internal/user",
            None,
            &kopf(&[("accept", "application/json")]),
            b"",
        )
        .expect("line");
        assert_eq!(
            z,
            r#"{"abfrage":null,"fassung":1,"id":7,"koerper_b64":"","kopf":{"accept":"application/json"},"methode":"GET","op":"modell.aufruf","pfad":"/copilot_internal/user","ziel":"api.github.com"}"#
        );
        let z = modell_aufruf(
            1,
            "a.b",
            "POST",
            "/chat/completions",
            Some("x=1&y=2"),
            &[],
            b"hi",
        )
        .expect("line");
        assert!(z.contains(r#""abfrage":"x=1&y=2""#), "{z}");
        assert!(z.contains(r#""koerper_b64":"aGk=""#), "{z}");
        assert!(!z.contains('?'), "{z}");
    }

    #[test]
    fn a_stream_has_its_own_op_without_method_and_body() {
        let z = modell_strom(
            2,
            "a.b",
            "/responses",
            None,
            &kopf(&[("sec-websocket-version", "13")]),
        )
        .expect("line");
        assert_eq!(
            z,
            r#"{"abfrage":null,"fassung":1,"id":2,"kopf":{"sec-websocket-version":"13"},"op":"modell.strom","pfad":"/responses","ziel":"a.b"}"#
        );
        assert_eq!(
            strom_teil(2, b"\x81\x03abc").expect("line"),
            r#"{"daten_b64":"gQNhYmM=","fassung":1,"id":2,"op":"strom.teil"}"#
        );
        assert_eq!(
            strom_ende(2).expect("line"),
            r#"{"fassung":1,"id":2,"op":"strom.ende"}"#
        );
        assert!(strom_teil(2, &vec![0; 65_537]).is_err());
        assert!(strom_teil(2, &vec![0; 65_536]).is_ok());
    }

    #[test]
    fn the_search_and_the_question_for_a_key_have_their_lines() {
        assert_eq!(
            modell_suche(3, "a.b", &["/arbeit".to_owned(), "/log".to_owned()]).expect("line"),
            r#"{"fassung":1,"id":3,"op":"modell.suche","pfade":["/arbeit","/log"],"ziel":"a.b"}"#
        );
        assert!(modell_suche(3, "a.b", &[]).is_err());
        assert!(modell_suche(3, "a.b", &vec!["/a".to_owned(); 17]).is_err());
        assert!(modell_suche(3, "a.b", &["a".to_owned()]).is_err());
        assert!(modell_suche(3, "a.b", &["/a/../b".to_owned()]).is_err());
        assert!(modell_suche(3, "a.b", &["/./a".to_owned()]).is_err());
        // The limits themselves hold: 16 paths and a path of 4096 characters go out, one more does not.
        assert!(modell_suche(3, "a.b", &vec!["/a".to_owned(); 16]).is_ok());
        let grenze = format!("/{}", "a".repeat(4095));
        assert_eq!(grenze.len(), 4096);
        assert!(modell_suche(3, "a.b", std::slice::from_ref(&grenze)).is_ok());
        assert!(modell_suche(3, "a.b", &[format!("{grenze}a")]).is_err());
        // A control character in a path never goes out (the form of the holder takes none).
        assert!(modell_suche(3, "a.b", &["/a\u{1}b".to_owned()]).is_err());
        assert!(modell_suche(3, "a.b", &["/a\nb".to_owned()]).is_err());
        // The question goes by the target, never by the name of the secret (contract 8, section 13.5, step 3).
        assert_eq!(
            schluessel_vorhanden(4, "api.individual.githubcopilot.com").expect("line"),
            r#"{"fassung":1,"id":4,"op":"schluessel.vorhanden","ziel":"api.individual.githubcopilot.com"}"#
        );
    }

    #[test]
    fn a_value_outside_the_form_of_the_holder_never_goes_out() {
        let lang = "a".repeat(8193);
        assert!(!kopf_ok(&kopf(&[("a", lang.as_str())])));
        assert!(kopf_ok(&kopf(&[("a", "a".repeat(8192).as_str())])));
        assert!(!kopf_ok(&kopf(&[("A", "x")])));
        assert!(!kopf_ok(&kopf(&[("a_b", "x")])));
        assert!(!kopf_ok(&kopf(&[("", "x")])));
        assert!(!kopf_ok(&kopf(&[("a", "x\ty")])));
        assert!(!kopf_ok(&vec![("a".to_owned(), "x".to_owned()); 65]));
        assert!(kopf_ok(&vec![("a".to_owned(), "x".to_owned()); 64]));
        assert!(modell_aufruf(1, "a.b", "GET", "/a", None, &kopf(&[("A", "x")]), b"").is_err());
        assert!(modell_aufruf(1, "a.b", "GET", "/a", Some("a#b"), &[], b"").is_err());
        assert!(modell_aufruf(1, "a.b", "GET", "/a", Some(""), &[], b"").is_err());
        assert!(abfrage_ok("x=1"));
        assert!(!abfrage_ok(&"x".repeat(1025)));
        assert!(abfrage_ok(&"x".repeat(1024)));
        // The limit itself still goes out, one byte more does not (H6).
        assert!(modell_aufruf(1, "a.b", "POST", "/a", None, &[], &vec![0; 24_117_248]).is_ok());
        assert!(modell_aufruf(1, "a.b", "POST", "/a", None, &[], &vec![0; 24_117_249]).is_err());
        // `modell.strom` checks both forms as well, each on its own: a head outside the form with a good query, and a
        // query outside the form with a good head.
        assert!(modell_strom(1, "a.b", "/a", Some("x=1"), &kopf(&[("A", "x")])).is_err());
        assert!(modell_strom(1, "a.b", "/a", Some("a#b"), &kopf(&[("a", "x")])).is_err());
        assert!(modell_strom(1, "a.b", "/a", Some(""), &[]).is_err());
        assert!(modell_strom(1, "a.b", "/a", Some("x=1"), &kopf(&[("a", "x")])).is_ok());
    }

    /// Every request gets a fresh `id`, and it comes back unchanged in every line of its answer (holder contract,
    /// section 4). A connection may carry one request after the other, so two requests never share an `id`.
    #[test]
    fn every_request_gets_a_fresh_id() {
        let (a, _b) = std::os::unix::net::UnixStream::pair().expect("pair");
        let mut k = Klient::auf(a);
        let erste = k.naechste_id();
        let zweite = k.naechste_id();
        let dritte = k.naechste_id();
        assert_eq!((erste, zweite, dritte), (1, 2, 3));
        assert!(erste < zweite && zweite < dritte);
        // The `id` of the line is the one that was handed out.
        let z = modell_aufruf(zweite, "a.b", "GET", "/x", None, &[], b"").expect("line");
        assert!(z.contains(r#""id":2"#), "{z}");
    }

    /// A holder that sends more than one line may hold (32 MiB) without ever sending the LF: the request ends with
    /// `mw.halter_fehler`, and it ends **before** the whole flood is read, so the memory of the model path stays
    /// bounded. The writer keeps its end open, so the end only comes from the limit.
    #[test]
    fn a_line_above_the_limit_ends_the_request_before_its_end() {
        use std::io::Write as _;

        let (a, mut b) = std::os::unix::net::UnixStream::pair().expect("pair");
        let schreiber = std::thread::spawn(move || {
            let block = vec![b'x'; 1 << 20];
            for _ in 0..33 {
                if b.write_all(&block).is_err() {
                    return;
                }
            }
            // No LF and no end of file: the reader may only end at its own limit.
            std::thread::sleep(Duration::from_secs(2));
        });
        let mut k = Klient::auf(a);
        k.frist(Some(Duration::from_secs(10))).expect("deadline");
        assert_eq!(k.naechste_zeile().err(), Some("mw.halter_fehler"));
        drop(k);
        let _ = schreiber.join();
    }

    /// The limit of a line has to cut off a flooding holder however his bytes arrive. One byte of its own ahead of
    /// the flood moves every later store off the multiple of the read block, so a check that fires only at the exact
    /// limit would never fire and the store of the model path would grow without an end.
    #[test]
    fn a_flood_whose_bytes_never_land_on_the_limit_is_cut_off_too() {
        use std::io::Write as _;

        let (a, mut b) = std::os::unix::net::UnixStream::pair().expect("pair");
        let schreiber = std::thread::spawn(move || {
            if b.write_all(b"x").is_err() || b.flush().is_err() {
                return;
            }
            // The reader takes this byte alone, so the store is 1 plus a multiple of the read block from here on.
            std::thread::sleep(Duration::from_millis(100));
            let block = vec![b'x'; 1 << 20];
            for _ in 0..40 {
                if b.write_all(&block).is_err() {
                    return;
                }
            }
            // No LF and no end of file: the reader may only end at its own limit, never at a deadline.
            std::thread::sleep(Duration::from_secs(3));
        });
        let mut k = Klient::auf(a);
        k.frist(Some(Duration::from_millis(500))).expect("deadline");
        assert_eq!(k.zeile_oder_nichts().err(), Some("mw.halter_fehler"));
        drop(k);
        let _ = schreiber.join();
    }

    /// A line of exactly `ZEILE_MAX` bytes before its LF is still a line, as at the holder, whose reader takes the same
    /// limit: the limit cuts off only what lies above it, also while the LF has not come yet.
    #[test]
    fn a_line_of_exactly_the_limit_is_still_read() {
        use std::io::Write as _;

        let kopf = br#"{"art":"ende","code":""#;
        let fuss = br#"","fassung":1,"ok":false}"#;
        let code = super::ZEILE_MAX - kopf.len() - fuss.len();
        let mut zeile = Vec::with_capacity(super::ZEILE_MAX + 1);
        zeile.extend_from_slice(kopf);
        zeile.resize(kopf.len() + code, b'x');
        zeile.extend_from_slice(fuss);
        assert_eq!(zeile.len(), super::ZEILE_MAX);
        let (a, mut b) = std::os::unix::net::UnixStream::pair().expect("pair");
        let schreiber = std::thread::spawn(move || {
            if b.write_all(&zeile).is_err() {
                return;
            }
            // The reader holds all bytes of the line and waits past its deadline before the LF comes.
            std::thread::sleep(Duration::from_secs(1));
            let _ = b.write_all(b"\n");
            std::thread::sleep(Duration::from_secs(1));
        });
        let mut k = Klient::auf(a);
        k.frist(Some(Duration::from_millis(200))).expect("deadline");
        let ende = std::time::Instant::now() + Duration::from_secs(20);
        let gelesen = loop {
            match k.zeile_oder_nichts() {
                Ok(Some(gelesen)) => break gelesen,
                Ok(None) => assert!(std::time::Instant::now() < ende, "no line"),
                Err(fehler) => panic!("a line of exactly the limit: {fehler}"),
            }
        };
        assert!(matches!(gelesen, Zeile::Ende(Some(wert)) if wert.len() == code));
        drop(k);
        let _ = schreiber.join();
    }

    #[test]
    fn the_lines_of_the_holder_are_read_in_their_closed_form() {
        let b = |s: &str| Zeile::lies(s.as_bytes());
        assert_eq!(
            b(
                r#"{"art":"beginn","fassung":1,"id":1,"kopf":{"content-type":"text/plain"},"status":200}"#
            ),
            Some(Zeile::Beginn {
                status: 200,
                kopf: vec![("content-type".to_owned(), "text/plain".to_owned())],
            })
        );
        assert_eq!(
            b(r#"{"art":"teil","daten_b64":"aGk=","fassung":1,"id":1}"#),
            Some(Zeile::Teil(b"hi".to_vec()))
        );
        assert_eq!(
            b(r#"{"art":"ende","fassung":1,"id":1,"ok":true}"#),
            Some(Zeile::Ende(None))
        );
        assert_eq!(
            b(
                r#"{"art":"ende","code":"halter.ziel_fehler","fassung":1,"grund":"Ziel nicht erreichbar","id":1,"ok":false}"#
            ),
            Some(Zeile::Ende(Some("halter.ziel_fehler".to_owned())))
        );
        assert_eq!(
            b(
                r#"{"art":"antwort","fassung":1,"id":1,"ok":true,"suche":{"dateien":2,"nicht_lesbar":0,"treffer":0,"uebersprungen":1}}"#
            ),
            Some(Zeile::Antwort {
                code: None,
                suche: Some(Suche {
                    dateien: 2,
                    nicht_lesbar: 0,
                    treffer: 0,
                    uebersprungen: 1,
                }),
                vorhanden: None,
            })
        );
        assert_eq!(
            b(r#"{"art":"antwort","fassung":1,"id":1,"ok":true,"vorhanden":false}"#),
            Some(Zeile::Antwort {
                code: None,
                suche: None,
                vorhanden: Some(false),
            })
        );
        assert_eq!(
            b(
                r#"{"art":"antwort","code":"halter.kein_schluessel","fassung":1,"grund":"kein Schluessel hinterlegt","id":1,"ok":false}"#
            ),
            Some(Zeile::Antwort {
                code: Some("halter.kein_schluessel".to_owned()),
                suche: None,
                vorhanden: None,
            })
        );
    }

    /// The headers of the line `beginn` go to the harness; a line with a name or a value outside the form is
    /// therefore not readable (fail closed against a header line hung into the answer).
    #[test]
    fn a_head_of_the_holder_that_could_hang_a_line_into_the_answer_is_not_readable() {
        let mit = |kopf: &str| {
            format!(r#"{{"art":"beginn","fassung":1,"id":1,"kopf":{kopf},"status":200}}"#)
        };
        for falsch in [
            r#"{"x":"a
set-cookie: b"}"#,
            r#"{"x":"a
b"}"#,
            r#"{"x":"a	b"}"#,
            r#"{"x:y":"a"}"#,
            r#"{"x y":"a"}"#,
            r#"{"":"a"}"#,
            r#"{"x":"a\u0000"}"#,
        ] {
            assert_eq!(Zeile::lies(mit(falsch).as_bytes()), None, "{falsch}");
        }
        let lang = format!(r#"{{"x":"{}"}}"#, "v".repeat(8193));
        assert_eq!(Zeile::lies(mit(&lang).as_bytes()), None);
        let grenze = format!(r#"{{"x":"{}"}}"#, "v".repeat(8192));
        assert!(Zeile::lies(mit(&grenze).as_bytes()).is_some());
        let mut viele = String::from("{");
        for i in 0..65 {
            if i > 0 {
                viele.push(',');
            }
            viele.push_str(r#""x-"#);
            viele.push_str(&i.to_string());
            viele.push_str(r#"":"v""#);
        }
        viele.push('}');
        assert_eq!(Zeile::lies(mit(&viele).as_bytes()), None);
        // An ordinary header of the target stays readable, also with capitals in its name.
        assert!(
            Zeile::lies(mit(r#"{"Content-Type":"text/plain; charset=utf-8"}"#).as_bytes())
                .is_some()
        );
    }

    #[test]
    fn a_line_outside_the_form_is_not_readable() {
        for falsch in [
            r#"{"art":"beginn","fassung":1,"id":1,"kopf":{},"status":99}"#,
            r#"{"art":"beginn","fassung":1,"id":1,"kopf":{},"status":600}"#,
            r#"{"art":"beginn","fassung":1,"id":1,"kopf":{"a":1},"status":200}"#,
            r#"{"art":"beginn","fassung":1,"id":1,"status":200}"#,
            r#"{"art":"teil","daten_b64":"nicht base64","fassung":1,"id":1}"#,
            r#"{"art":"ende","fassung":1,"id":1,"ok":false}"#,
            r#"{"art":"ende","code":"halter.intern","fassung":1,"id":1,"ok":true}"#,
            r#"{"art":"ende","fassung":2,"id":1,"ok":true}"#,
            r#"{"art":"erfunden","fassung":1,"id":1}"#,
            r#"{"fassung":1,"id":1,"ok":true}"#,
            r#"{"art":"antwort","fassung":1,"id":1,"ok":true,"suche":{"dateien":2}}"#,
            r#"{"art":"antwort","fassung":1,"id":1,"ok":true,"vorhanden":"ja"}"#,
            r#"{"art":"antwort","fassung":1,"id":1}"#,
            r#"{"art":"ende","fassung":1,"id":1,"ok":true,"ok":true}"#,
            "nicht json",
            "",
        ] {
            assert_eq!(Zeile::lies(falsch.as_bytes()), None, "{falsch}");
        }
    }
}
