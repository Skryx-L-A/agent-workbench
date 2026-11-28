//! The channel of the service to the decision point (contract, addendum "Kanal zur Stelle (L6, Nr. 98)").
//!
//! SECURITY, fail closed: the service speaks to the point only as a client of `peerkanal` with the expected identity of the
//! point (`lauf.stelle.erwartet`); `peerkanal` measures the server before a payload byte goes. A failure of the connection,
//! of that check, of a deadline or of the form of the answer is never a release: it is `stelle_nicht_erreichbar` or
//! `stelle_antwort_ungueltig`, and nothing changes. The reason of a human (`begruendung`) never goes to the point: the input
//! of `mcp.entscheiden` is built from exactly three values. No value of this module goes to stderr.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use peerkanal::{ClientVorgabe, Erwartet, Kennung};
use serde_json::{Map, Value, json};

/// The point did not answer, or not through a checked channel: the reason of the switch for the local way without
/// `lauf.stelle` (`weiche.rs` names it).
pub(crate) const NICHT_ERREICHBAR: &str = crate::weiche::STELLE_NICHT_ERREICHBAR;
/// The point answered, but not in the fixed form.
pub(crate) const ANTWORT_UNGUELTIG: &str = "stelle_antwort_ungueltig";
/// The point refused; the code of the point goes with it.
pub(crate) const ABGELEHNT: &str = "stelle_abgelehnt";
/// A second request of a run for a worker before the answer to the first.
pub(crate) const ANFORDERN_OFFEN: &str = "anfordern_offen";

/// The deadline of the connection (with the check of the other side and the hello).
const VERBINDEN_FRIST: Duration = Duration::from_secs(2);
/// The deadline of the answer.
const ANTWORT_FRIST: Duration = Duration::from_secs(5);
/// The largest frame and answer (contract `stelle`, section 13.3).
const RAHMEN_MAX: usize = 2_097_152;
/// How often the service asks `mcp.offene` while a viewer is connected (DI-L6 point 3).
pub(crate) const OFFENE_TAKT: Duration = Duration::from_secs(1);

/// The wall clock in milliseconds since 1970 (the deadlines of the point are in it); 0 when the clock lies before.
pub(crate) fn wanduhr_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

/// The largest line of the report path for a worker.
pub(crate) const MELDEZEILE_MAX: usize = 256;

/// The channels of the point (contract `stelle`, section 14.1).
pub(crate) const KANAL_MENSCH: &str = "stelle-mensch";
/// See [`KANAL_MENSCH`].
pub(crate) const KANAL_KERN: &str = "stelle-kern";

/// Where the point is and who it must be: `lauf.stelle` of the start file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ort {
    /// The folder of the sockets (Unix), `None` under Windows.
    pub ordner: Option<PathBuf>,
    /// The expected identity of the point: uid (Unix) or SID (Windows).
    pub erwartet: Erwartet,
    /// Where the service gets the proof of a release (addendum point 4): the key file and the pseudonym of each account.
    pub nachweis: Option<Nachweisort>,
}

/// The part `nachweis` of `lauf.stelle`: the key of `h1` (a file with the mode 0600, never read before a release) and the
/// pseudonym of each measured account (`uid-<uid>` or `sid-<SID>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nachweisort {
    /// The absolute path of the key file.
    pub schluessel: PathBuf,
    /// Account to pseudonym.
    pub konten: std::collections::BTreeMap<String, String>,
}

/// The most accounts `lauf.stelle.nachweis.konten` names.
const KONTEN_MAX: usize = 256;

/// `ps_` and 32 hex digits in lower case.
fn pseudonym_form(ps: &str) -> bool {
    ps.len() == 35
        && ps.starts_with("ps_")
        && ps[3..]
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// 64 hex digits in lower case (a sha256).
fn sha256_form(t: &str) -> bool {
    t.len() == 64
        && t.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// The account in the form of the measurement: `uid-<uid>` (decimal, no leading zero) under Unix, `sid-<SID>` under Windows.
fn konto_form(t: &str, windows: bool) -> bool {
    if windows {
        return t
            .strip_prefix("sid-")
            .is_some_and(|sid| peerkanal::Sid::neu(sid).is_ok());
    }
    t.strip_prefix("uid-")
        .and_then(|n| n.parse::<u32>().ok().filter(|u| u.to_string() == n))
        .is_some()
}

/// Reads `lauf.stelle.nachweis`: exactly `{"schluessel": <absolute path>, "konten": {<account>: <pseudonym>, ..}}`.
fn nachweis_lesen(wert: &Value, windows: bool) -> Option<Nachweisort> {
    let o = wert.as_object().filter(|o| o.len() == 2)?;
    let schluessel = o
        .get("schluessel")?
        .as_str()
        .filter(|t| !t.contains('\0'))
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())?;
    let konten = o
        .get("konten")?
        .as_object()
        .filter(|k| k.len() <= KONTEN_MAX)?;
    let konten = konten
        .iter()
        .map(|(k, v)| {
            let ps = v.as_str().filter(|ps| pseudonym_form(ps))?;
            konto_form(k, windows).then(|| (k.clone(), ps.to_owned()))
        })
        .collect::<Option<_>>()?;
    Some(Nachweisort { schluessel, konten })
}

fn kennung_form(t: &str) -> bool {
    let b = t.as_bytes();
    (1..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// Reads `lauf.stelle`: under Unix exactly `{"ordner": <absolute path>, "erwartet": {"uid": <number>}}`, under Windows exactly
/// `{"erwartet": {"sid": <SID>}}`, each with an optional `nachweis`. `erwartet` is mandatory: without it the service would not
/// check the other side.
pub fn ort_lesen(wert: &Value) -> Option<Ort> {
    // the system as `peerkanal` names it (the Windows calls stay in that crate)
    ort_lesen_fuer(wert, peerkanal::System::aktuell().ist_windows())
}

/// [`ort_lesen`] for the system `windows` or Unix; both forms are checked on every system.
fn ort_lesen_fuer(wert: &Value, windows: bool) -> Option<Ort> {
    let o = wert.as_object()?;
    let erwartet = o.get("erwartet")?.as_object().filter(|e| e.len() == 1)?;
    let nachweis = match o.get("nachweis") {
        None => None,
        Some(n) => Some(nachweis_lesen(n, windows)?),
    };
    let felder = o.len() - usize::from(nachweis.is_some());
    if windows {
        if felder != 1 {
            return None;
        }
        let sid = peerkanal::Sid::neu(erwartet.get("sid")?.as_str()?).ok()?;
        return Some(Ort {
            ordner: None,
            erwartet: Erwartet::Windows { sid },
            nachweis,
        });
    }
    if felder != 2 {
        return None;
    }
    let ordner = o
        .get("ordner")?
        .as_str()
        .filter(|t| t.starts_with('/') && !t.contains('\0'))?;
    let uid = u32::try_from(erwartet.get("uid")?.as_u64()?).ok()?;
    Some(Ort {
        ordner: Some(PathBuf::from(ordner)),
        erwartet: Erwartet::Unix { uid },
        nachweis,
    })
}

/// One request on its own connection: connect with the check of the point, one frame, one answer, close. Any failure is
/// [`NICHT_ERREICHBAR`]; an answer that is no JSON object is [`ANTWORT_UNGUELTIG`].
pub(crate) fn fragen(
    ort: &Ort,
    kanal: &str,
    vorgang: &str,
    eingabe: &Value,
) -> Result<Map<String, Value>, &'static str> {
    let v = ClientVorgabe {
        name: Kennung::neu(kanal).map_err(|_| NICHT_ERREICHBAR)?,
        erwartet: ort.erwartet.clone(),
        frist: VERBINDEN_FRIST,
        unix_ordner: ort.ordner.clone(),
    };
    let mut verbindung = peerkanal::verbinden(&v).map_err(|_| NICHT_ERREICHBAR)?;
    let mut zeile = json!({"vorgang": vorgang, "eingabe": eingabe})
        .to_string()
        .into_bytes();
    zeile.push(b'\n');
    if zeile.len() > RAHMEN_MAX {
        return Err(NICHT_ERREICHBAR);
    }
    verbindung.write_all(&zeile).map_err(|_| NICHT_ERREICHBAR)?;
    verbindung.lesefrist(Some(ANTWORT_FRIST));
    // the answer with its LF is at most RAHMEN_MAX bytes, as the frame
    let mut leser = BufReader::new(verbindung).take(u64::try_from(RAHMEN_MAX).unwrap_or(0));
    let mut antwort = Vec::new();
    let n = leser
        .read_until(b'\n', &mut antwort)
        .map_err(|_| NICHT_ERREICHBAR)?;
    if n == 0 || antwort.pop() != Some(b'\n') {
        return Err(NICHT_ERREICHBAR);
    }
    match serde_json::from_slice::<Value>(&antwort) {
        Ok(Value::Object(o)) => Ok(o),
        _ => Err(ANTWORT_UNGUELTIG),
    }
}

/// A refusal of the point `{"ok": false, "code": "stelle.*", "grund": <text>}`, else `None`.
fn ablehnung_der_stelle(o: &Map<String, Value>) -> Option<String> {
    let code = o.get("code")?.as_str()?;
    let gut = o.len() == 3
        && o.get("ok") == Some(&json!(false))
        && o.get("grund").is_some_and(Value::is_string)
        && code.strip_prefix("stelle.").is_some_and(kennung_form);
    gut.then(|| code.to_owned())
}

fn genau(o: &Map<String, Value>, namen: &[&str]) -> bool {
    o.len() == namen.len() && namen.iter().all(|n| o.contains_key(*n))
}

/// The outcome of a request: the answer of the point, or the reason of the service with the code of the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fehlschlag {
    /// [`NICHT_ERREICHBAR`] or [`ANTWORT_UNGUELTIG`].
    Dienst(&'static str),
    /// [`ABGELEHNT`] with the code of the point.
    Stelle(String),
}

fn auswerten<T>(
    antwort: Result<Map<String, Value>, &'static str>,
    gut: impl FnOnce(&Map<String, Value>) -> Option<T>,
) -> Result<T, Fehlschlag> {
    let o = antwort.map_err(Fehlschlag::Dienst)?;
    if let Some(code) = ablehnung_der_stelle(&o) {
        return Err(Fehlschlag::Stelle(code));
    }
    gut(&o).ok_or(Fehlschlag::Dienst(ANTWORT_UNGUELTIG))
}

/// The decision of the point on a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entschieden {
    /// `erteilt`, with the end of the redeem deadline.
    Erteilt,
    /// `abgelehnt`.
    Abgelehnt,
}

/// The form of the answer of `mcp.entscheiden` (addendum, point 6): exactly these keys, the sent request.
pub(crate) fn entscheidung_lesen(o: &Map<String, Value>, anfrage: &str) -> Option<Entschieden> {
    let eigene = o.get("anfrage").and_then(Value::as_str) == Some(anfrage)
        && o.get("ok") == Some(&json!(true));
    match o.get("ergebnis").and_then(Value::as_str) {
        Some("erteilt")
            if eigene
                && genau(o, &["ok", "ergebnis", "anfrage", "einloesen_bis_ms"])
                && o.get("einloesen_bis_ms")
                    .is_some_and(|v| v.as_i64().is_some()) =>
        {
            Some(Entschieden::Erteilt)
        }
        Some("abgelehnt") if eigene && genau(o, &["ok", "ergebnis", "anfrage"]) => {
            Some(Entschieden::Abgelehnt)
        }
        _ => None,
    }
}

/// `mcp.entscheiden` with exactly `anfrage`, `entscheidung` and `nachweis` (the reason stays with the service).
pub(crate) fn entscheiden(
    ort: &Ort,
    anfrage: &str,
    entscheidung: &str,
    nachweis: &str,
) -> Result<Entschieden, Fehlschlag> {
    let eingabe = json!({"anfrage": anfrage, "entscheidung": entscheidung, "nachweis": nachweis});
    auswerten(
        fragen(ort, KANAL_MENSCH, "mcp.entscheiden", &eingabe),
        |o| entscheidung_lesen(o, anfrage),
    )
}

/// An open request as `mcp.offene` names it (contract `stelle`, sections 4 and 14.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Offen {
    pub(crate) anfrage: String,
    pub(crate) sitzung: String,
    pub(crate) werkzeug: String,
    pub(crate) vorschau: Option<String>,
    pub(crate) frist_bis_ms: i64,
    /// The binding of the call and of the preview the point names; the challenge of the proof takes them from here, never from
    /// the event of the view.
    pub(crate) digest: String,
    pub(crate) vorschau_sha256: Option<String>,
}

/// The form of the answer of `mcp.offene`; one entry out of form makes the whole answer invalid.
pub(crate) fn offene_lesen(o: &Map<String, Value>) -> Option<Vec<Offen>> {
    if !genau(o, &["ok", "ergebnis", "offene"])
        || o.get("ok") != Some(&json!(true))
        || o.get("ergebnis") != Some(&json!("offene"))
    {
        return None;
    }
    o.get("offene")?
        .as_array()?
        .iter()
        .map(|e| {
            let e = e.as_object()?;
            let text = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_owned);
            let vorschau = match e.get("vorschau")? {
                Value::Null => None,
                Value::String(t) => Some(t.clone()),
                _ => return None,
            };
            let vorschau_sha256 = match e.get("vorschau_sha256")? {
                Value::Null => None,
                Value::String(t) if sha256_form(t) => Some(t.clone()),
                _ => return None,
            };
            Some(Offen {
                anfrage: text("anfrage")?,
                sitzung: text("sitzung").filter(|s| kennung_form(s))?,
                werkzeug: text("werkzeug")?,
                vorschau,
                frist_bis_ms: e.get("frist_bis_ms")?.as_i64()?,
                digest: text("digest").filter(|d| sha256_form(d))?,
                vorschau_sha256,
            })
        })
        .collect()
}

/// `mcp.offene`.
pub(crate) fn offene(ort: &Ort) -> Result<Vec<Offen>, Fehlschlag> {
    auswerten(
        fragen(ort, KANAL_MENSCH, "mcp.offene", &json!({})),
        offene_lesen,
    )
}

/// `person.pseudonym` with the identifier `uid-<uid>` or `sid-<SID>`; the name of the account never goes. `None` for every
/// failure and every value that is no pseudonym (then the viewer is never admin).
pub(crate) fn pseudonym(ort: &Ort, kennung: &str) -> Option<String> {
    let o = fragen(
        ort,
        KANAL_KERN,
        "person.pseudonym",
        &json!({"kennung": kennung}),
    )
    .ok()?;
    pseudonym_lesen(&o)
}

/// The form of the answer of `person.pseudonym`: exactly these keys, `ps_` and 32 hex digits in lower case.
fn pseudonym_lesen(o: &Map<String, Value>) -> Option<String> {
    if !genau(o, &["ok", "ergebnis", "pseudonym"]) || o.get("ergebnis") != Some(&json!("pseudonym"))
    {
        return None;
    }
    let ps = o.get("pseudonym")?.as_str()?;
    pseudonym_form(ps).then(|| ps.to_owned())
}

/// A request for a worker from the report path of a run (addendum, point 10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Anforderung {
    pub(crate) rolle: String,
    pub(crate) harness: String,
    pub(crate) modell: String,
    pub(crate) effort: String,
}

/// The grammar `anfordern <rolle> <harness> <modell> <effort>` with LF, each field in the form of an identifier, at most 256
/// bytes; anything else is `None`.
pub(crate) fn meldezeile_lesen(zeile: &[u8]) -> Option<Anforderung> {
    if zeile.len() > MELDEZEILE_MAX {
        return None;
    }
    let text = std::str::from_utf8(zeile.strip_suffix(b"\n")?).ok()?;
    let teile: Vec<&str> = text.split(' ').collect();
    let [kopf, rolle, harness, modell, effort] = teile.as_slice() else {
        return None;
    };
    if *kopf != "anfordern"
        || ![rolle, harness, modell, effort]
            .iter()
            .all(|t| kennung_form(t))
    {
        return None;
    }
    Some(Anforderung {
        rolle: (*rolle).to_owned(),
        harness: (*harness).to_owned(),
        modell: (*modell).to_owned(),
        effort: (*effort).to_owned(),
    })
}

/// `worker.anfordern` for the orchestrator `sitzung` (the session of the run, never a value of the line): `Ok` only for
/// `frei` with the same session.
pub(crate) fn worker_anfordern(
    ort: &Ort,
    sitzung: &str,
    a: &Anforderung,
    laufend: u64,
) -> Result<(), Fehlschlag> {
    let eingabe = json!({"sitzung": sitzung, "rolle": a.rolle, "harness": a.harness, "modell": a.modell,
                         "effort": a.effort, "laufend": laufend});
    auswerten(fragen(ort, KANAL_KERN, "worker.anfordern", &eingabe), |o| {
        let gut = genau(o, &["ok", "ergebnis", "sitzung"])
            && o.get("ok") == Some(&json!(true))
            && o.get("ergebnis") == Some(&json!("frei"))
            && o.get("sitzung").and_then(Value::as_str) == Some(sitzung);
        gut.then_some(())
    })
}

/// The answer line to the run: `frei <sitzung>` or `abgelehnt <code>`; a failure is never `frei`.
pub(crate) fn meldeantwort(ergebnis: &Result<(), Fehlschlag>, sitzung: &str) -> Vec<u8> {
    match ergebnis {
        Ok(()) => format!("frei {sitzung}\n"),
        Err(Fehlschlag::Dienst(grund)) => format!("abgelehnt {grund}\n"),
        Err(Fehlschlag::Stelle(code)) => format!("abgelehnt {code}\n"),
    }
    .into_bytes()
}

/// The rejection to the sender of a `Freigabe` (DI-L6 point 5 and the addendum, point 5): `stelle_abgelehnt` with the code
/// of the point, or one of the two reasons of the service with code and fixed text. The core stays as it is.
pub(crate) fn ablehnung(f: &Fehlschlag) -> Value {
    match f {
        Fehlschlag::Stelle(code) => json!({"grund": ABGELEHNT, "code": code}),
        Fehlschlag::Dienst(grund) => {
            let text = if *grund == ANTWORT_UNGUELTIG {
                "Antwort der Stelle ungueltig"
            } else {
                "Stelle nicht erreichbar"
            };
            json!({"grund": grund, "code": grund, "text": text})
        }
    }
}

/// The answer to a second request of a run before the answer to its first: no question to the point.
pub(crate) fn zweite_anforderung() -> Vec<u8> {
    format!("abgelehnt {ANFORDERN_OFFEN}\n").into_bytes()
}

/// The reasons of a release of the service after the checks of DI-L6 point 4 (addendum points 10 and 11), in the order of the
/// checks. `freigabe_nur_lokal` and `stelle_nicht_erreichbar` before them belong to the switch (`weiche.rs`, DI-E5 point 5).
pub(crate) const KONTO_OHNE_PERSON: &str = "konto_ohne_person";
/// See [`KONTO_OHNE_PERSON`].
pub(crate) const SCHLUESSEL_FEHLT: &str = "schluessel_fehlt";
/// The fields of an event that would name or prove who decides: the service takes them from the connection and the
/// configuration, never from the view (DI-L11 point 7). Each is the error `feld_verboten`, as `t` and `betrachter` (DI3).
pub(crate) const VERBOTEN: [&str; 6] = ["nachweis", "wer", "konto", "person", "uid", "sid"];

/// The measured account of a viewer as `uid-<uid>` or `sid-<SID>`, from the account of the local way (DI-L11 point 2: the partner,
/// measured by `peerkanal`; `lokal::Wer` in its form `{"system", "uid", "sid", "name"}`). Exactly one of `uid` and `sid` names
/// the account; anything else is `None`, and the release ends with [`KONTO_OHNE_PERSON`]. Nothing of the event and nothing of the
/// start file is an account, and a viewer of `dienst.sock` has none.
pub(crate) fn konto_aus(wer: &Value) -> Option<String> {
    let uid = wer.get("uid").filter(|v| !v.is_null());
    let sid = wer.get("sid").filter(|v| !v.is_null());
    match (uid, sid) {
        (Some(u), None) => u32::try_from(u.as_u64()?).ok().map(|u| format!("uid-{u}")),
        (None, Some(s)) => {
            let s = s.as_str()?;
            peerkanal::Sid::neu(s).ok().map(|_| format!("sid-{s}"))
        }
        _ => None,
    }
}

/// A release of the view in its form (DI-L6 point 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Freigabe {
    pub(crate) anfrage: String,
    pub(crate) entscheidung: &'static str,
    pub(crate) begruendung: Option<String>,
}

/// The form of `Freigabe` after zustandskern R-L6 points 1 and 2 (the same reasons), then the count: exactly `art`, `anfragen`
/// and the optional `entscheidung` (`erteilen` when missing) and `begruendung`.
pub(crate) fn freigabe_form(e: &Map<String, Value>) -> Result<Freigabe, &'static str> {
    const UNGUELTIG: &str = "ereignis_ungueltig";
    let erlaubt = ["art", "anfragen", "entscheidung", "begruendung"];
    if e.keys().any(|k| !erlaubt.contains(&k.as_str())) {
        return Err(UNGUELTIG);
    }
    let Some(Value::Array(ids)) = e.get("anfragen") else {
        return Err(UNGUELTIG);
    };
    let ids = ids
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<&str>>>()
        .ok_or(UNGUELTIG)?;
    let entscheidung = match e.get("entscheidung").map(Value::as_str) {
        None | Some(Some("erteilen")) => "erteilen",
        Some(Some("ablehnen")) => "ablehnen",
        Some(_) => return Err(UNGUELTIG),
    };
    let begruendung = match e.get("begruendung") {
        None => None,
        Some(Value::String(t)) => Some(t.clone()),
        Some(_) => return Err(UNGUELTIG),
    };
    if let Some(t) = &begruendung {
        if entscheidung == "erteilen" {
            return Err("begruendung_nur_bei_ablehnen");
        }
        let steuer = |c: char| c < '\u{20}' || ('\u{7f}'..='\u{9f}').contains(&c);
        if t.is_empty() || t.chars().count() > 500 || t.chars().any(steuer) {
            return Err("begruendung_ungueltig");
        }
    }
    let [anfrage] = ids.as_slice() else {
        return Err("sammelfreigabe");
    };
    Ok(Freigabe {
        anfrage: (*anfrage).to_owned(),
        entscheidung,
        begruendung,
    })
}

/// The state of the request after the form (DI-L6 point 4): a core of this service knows it (`bekannt`, and its `zustand`), the
/// viewer is in the customer room of its session, it is open and no release of it waits for the point.
pub(crate) fn freigabe_lage(
    bekannt: bool,
    im_raum: bool,
    zustand: Option<&str>,
    wartet: bool,
) -> Result<(), &'static str> {
    if !bekannt {
        return Err("anfrage_unbekannt");
    }
    if !im_raum {
        return Err("fremder_kundenraum");
    }
    match zustand {
        Some("offen") if !wartet => Ok(()),
        Some("abgelaufen") => Err("abgelaufen"),
        Some(_) => Err("nicht_offen"),
        None => Err("anfrage_unbekannt"),
    }
}

/// The proof of the service, right before `mcp.entscheiden` (addendum point 4): the pseudonym of the measured account from
/// `lauf.stelle.nachweis` ([`KONTO_OHNE_PERSON`]), the key from its file with the mode 0600 ([`SCHLUESSEL_FEHLT`]), then
/// `h1:<pseudonym>:<hex>` over the challenge of contract 6, section 6, with `digest` and `vorschau_sha256` of `mcp.offene`. The key
/// lives only in this call; it never goes to the view, into a line or into a log.
pub(crate) fn nachweis_bilden(
    ort: Option<&Nachweisort>,
    konto: &str,
    offen: &Offen,
    entscheidung: &str,
) -> Result<String, &'static str> {
    let ps = ort
        .and_then(|n| n.konten.get(konto))
        .ok_or(KONTO_OHNE_PERSON)?;
    let schluessel = ort
        .and_then(|n| stelle::schluessel_h1(&n.schluessel))
        .ok_or(SCHLUESSEL_FEHLT)?;
    let h = stelle::herausforderung(
        &offen.anfrage,
        &offen.digest,
        entscheidung,
        offen.vorschau_sha256.as_deref(),
    );
    Ok(stelle::nachweis_h1(&schluessel, ps, &h))
}

/// What the service knows of the requests of the point.
#[derive(Debug, Default)]
pub(crate) struct Stand {
    /// The requests the cores got as `Anfrage`, by identifier: the entry of `mcp.offene` (session, binding of call and preview).
    pub(crate) anfragen: std::collections::BTreeMap<String, Offen>,
    /// Releases sent to the point and not answered yet: identifier to viewer and reason (the reason stays here).
    pub(crate) wartend: std::collections::BTreeMap<String, (u64, Option<String>)>,
    /// When `mcp.offene` went last; `None` before the first time.
    pub(crate) offene_zuletzt: Option<std::time::Instant>,
}

/// A job for the thread of the channel; the loop of the service never waits for the point.
pub(crate) enum Auftrag {
    /// `Freigabe` of a viewer.
    Entscheiden {
        nr: u64,
        anfrage: String,
        entscheidung: String,
        nachweis: String,
    },
    /// `mcp.offene`.
    Offene,
}

/// What the thread hands back to the loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rueckmeldung {
    /// The outcome of a `Freigabe` of viewer `nr`.
    Entschieden {
        nr: u64,
        anfrage: String,
        ergebnis: Result<Entschieden, Fehlschlag>,
    },
    /// The open requests, or the failure (then no new request).
    Offene(Result<Vec<Offen>, Fehlschlag>),
}

/// The thread of the channel: jobs in, answers into a queue, and one byte on the wake socket per answer.
pub(crate) struct Faden {
    tx: mpsc::Sender<Auftrag>,
    /// `lauf.stelle.nachweis`, for the proof of a release.
    pub(crate) nachweis: Option<Nachweisort>,
    pub(crate) eingang: Arc<Mutex<VecDeque<Rueckmeldung>>>,
}

impl Faden {
    /// Starts the thread. `wecken` writes one byte to the wake socket of the loop.
    pub(crate) fn starten(ort: Ort, wecken: Box<dyn Fn() + Send>) -> Self {
        let nachweis = ort.nachweis.clone();
        let (tx, rx) = mpsc::channel::<Auftrag>();
        let eingang = Arc::new(Mutex::new(VecDeque::new()));
        let ziel = Arc::clone(&eingang);
        std::thread::spawn(move || {
            for auftrag in rx {
                let r = match auftrag {
                    Auftrag::Entscheiden {
                        nr,
                        anfrage,
                        entscheidung,
                        nachweis,
                    } => {
                        let ergebnis = entscheiden(&ort, &anfrage, &entscheidung, &nachweis);
                        Rueckmeldung::Entschieden {
                            nr,
                            anfrage,
                            ergebnis,
                        }
                    }
                    Auftrag::Offene => Rueckmeldung::Offene(offene(&ort)),
                };
                if let Ok(mut q) = ziel.lock() {
                    q.push_back(r);
                }
                wecken();
            }
        });
        Self {
            tx,
            nachweis,
            eingang,
        }
    }

    /// The thread for `lauf.stelle`, or none without it. The loop takes the answers in each turn; the wake socket with one
    /// kind per source comes with the local way (DI-L11).
    pub(crate) fn aus_lauf(ort: Option<Ort>) -> Option<Self> {
        ort.map(|o| Self::starten(o, Box::new(|| {})))
    }

    /// Hands a job to the thread; `false` when the thread is gone.
    pub(crate) fn auftrag(&self, a: Auftrag) -> bool {
        self.tx.send(a).is_ok()
    }

    /// Every answer that came in, in order.
    pub(crate) fn abholen(&self) -> Vec<Rueckmeldung> {
        self.eingang
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
}

// `cfg(test)` alone, so that the mutation tests leave the test file out; the file is Unix only (`#![cfg(unix)]` in it)
#[cfg(test)]
mod tests;
