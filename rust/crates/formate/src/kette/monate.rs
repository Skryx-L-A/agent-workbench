//! Months, anchors and the switch (contract 5, section 5): the files of a source, the anchor line of a closed
//! month, and writing under one lock on the anchor file, which also covers the switch from one month to the next.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use serde_json::{Map, Value, json};

use super::datum::text_zeit;
use super::hash::sha256_hex;
use super::pruefen::{
    Dateiwerte, Gelesen, ZEILE_MAX, ist_zwei, jahr_monat, objekt_lesen, zeile_lesen, zeile_pruefen,
    zeilenart_ok,
};
use super::schreiber::{ein_ausgabe, letzte_zeile};
use super::{
    Abgewiesen, Fehler, Gesperrt, NULL_HASH, Ordnermarke, Probelauf, abschnitt, felder_abgewiesen,
    felder_ordnermarke, felder_probelauf, felder_pruefen, ist_sha256, kanonisch, quelle_ok,
    zeile_gesperrt, zeit_text,
};

/// The month `JJJJ-MM` of a file name `<quelle>-JJJJ-MM.jsonl`, or `None` for any other name.
pub(super) fn monat_aus_name<'a>(quelle: &str, name: &'a str) -> Option<&'a str> {
    let rest = name
        .strip_prefix(quelle)?
        .strip_prefix('-')?
        .strip_suffix(".jsonl")?;
    jahr_monat(rest).then_some(rest)
}

/// The name of the anchor file of a source.
pub(super) fn anker_name(quelle: &str) -> String {
    format!("{quelle}.anker.jsonl")
}

/// The names of the month files of a source in the folder, ascending.
pub(super) fn monatsnamen(ordner: &Path, quelle: &str) -> io::Result<Vec<String>> {
    let mut namen = Vec::new();
    for eintrag in fs::read_dir(ordner)? {
        let name = eintrag?.file_name();
        if let Some(name) = name
            .to_str()
            .filter(|n| monat_aus_name(quelle, n).is_some())
        {
            namen.push(name.to_owned());
        }
    }
    namen.sort();
    Ok(namen)
}

/// Milliseconds to seconds as a double (a division of two doubles, the same as `Date.parse(...) / 1000`).
#[allow(
    clippy::cast_precision_loss,
    reason = "a time up to 9999-12-31 in milliseconds is below 2^53, so the double is exact"
)]
fn sekunden(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

fn sekunden_wert(ms: Option<i64>) -> Value {
    ms.and_then(|m| serde_json::Number::from_f64(sekunden(m)))
        .map_or(Value::Null, Value::Number)
}

/// The anchor line of the closed month `name` (without line feed).
fn anker_zeile(
    name: &str,
    monat: &str,
    vorher_erste: Option<&str>,
    von: Option<i64>,
    bis: Option<i64>,
    letzte: &str,
) -> Result<String, Fehler> {
    kanonisch(&json!({
        "abschnitt": name,
        "bis": sekunden_wert(bis),
        "fassung": 2,
        "letzte": letzte,
        "monat": monat,
        "von": sekunden_wert(von),
        "vorher_erste": vorher_erste,
    }))
}

/// Same number: both `null`, or both the same double.
fn zahl_gleich(wert: Option<&Value>, soll: Option<i64>) -> bool {
    match (wert, soll) {
        (Some(Value::Null), None) => true,
        (Some(Value::Number(n)), Some(ms)) => n
            .as_f64()
            .is_some_and(|f| f.to_bits() == sekunden(ms).to_bits()),
        _ => false,
    }
}

/// Whether an anchor line fits the month file it names (contract 5.4). `roh` is the line as written,
/// `werte` what the lines of the file gave.
pub(super) fn anker_passt(
    roh: &[u8],
    wert: &Map<String, Value>,
    name: &str,
    quelle: &str,
    werte: &Dateiwerte,
) -> bool {
    let mut felder = wert.clone();
    if let Some(fassung) = felder.remove("fassung") {
        let kanonisch_ok =
            kanonisch(&Value::Object(wert.clone())).is_ok_and(|bytes| bytes.as_bytes() == roh);
        if !ist_zwei(&fassung) || !kanonisch_ok {
            return false;
        }
    }
    let text = |schluessel: &str| felder.get(schluessel).and_then(Value::as_str);
    felder.len() == 6
        && text("abschnitt") == Some(name)
        && text("monat") == monat_aus_name(quelle, name)
        && text("vorher_erste") == Some(werte.erste_vorher.as_str())
        && text("letzte") == Some(werte.letzte.as_str())
        && zahl_gleich(felder.get("von"), werte.von)
        && zahl_gleich(felder.get("bis"), werte.bis)
}

/// One line of the anchor file as the writer needs it.
struct Eintrag {
    abschnitt: String,
    letzte: Option<String>,
}

/// The anchor lines. The file must end with a line feed; every line must be a JSON object with a text `abschnitt`.
fn anker_lesen(datei: &mut File) -> Result<Vec<Eintrag>, Fehler> {
    let mut bytes = Vec::new();
    datei
        .seek(SeekFrom::Start(0))
        .and_then(|_| datei.read_to_end(&mut bytes))
        .map_err(|e| ein_ausgabe(&e))?;
    let Some((&letztes, ohne_ende)) = bytes.split_last() else {
        return Ok(Vec::new());
    };
    if letztes != b'\n' {
        return Err(Fehler::LetzteZeileUnvollstaendig);
    }
    ohne_ende
        .split(|b| *b == b'\n')
        .map(|zeile| match objekt_lesen(zeile) {
            Some(Value::Object(map)) => map
                .get("abschnitt")
                .and_then(Value::as_str)
                .map(|abschnitt| Eintrag {
                    abschnitt: abschnitt.to_owned(),
                    letzte: map.get("letzte").and_then(Value::as_str).map(str::to_owned),
                })
                .ok_or(Fehler::AnkerFalsch),
            _ => Err(Fehler::AnkerFalsch),
        })
        .collect()
}

/// `vorher` and `zeit` (milliseconds) of a line, or `(None, None)` when the line is unreadable or too long.
fn zeile_werte(roh: Option<&[u8]>) -> (Option<String>, Option<i64>) {
    let Some(Value::Object(map)) = roh.filter(|r| r.len() <= ZEILE_MAX).and_then(objekt_lesen)
    else {
        return (None, None);
    };
    (
        map.get("vorher").and_then(Value::as_str).map(str::to_owned),
        map.get("zeit").and_then(Value::as_str).and_then(text_zeit),
    )
}

/// The first line of a file when it fits the limit of a line, else `None`.
fn erste_zeile(datei: &mut File) -> Result<Option<Vec<u8>>, Fehler> {
    datei
        .seek(SeekFrom::Start(0))
        .map_err(|e| ein_ausgabe(&e))?;
    let mut leser = BufReader::new(datei);
    let mut puffer = Vec::new();
    match zeile_lesen(&mut leser, &mut puffer).map_err(|e| ein_ausgabe(&e))? {
        Gelesen::Zeile => Ok(Some(puffer)),
        _ => Ok(None),
    }
}

/// The sha256 of the last line and the anchor line of a closed month file with `laenge` bytes
/// (not empty, ends with a line feed).
fn anker_aus_datei(
    pfad: &Path,
    name: &str,
    quelle: &str,
    laenge: u64,
) -> Result<(String, String), Fehler> {
    let mut datei = File::open(pfad).map_err(|e| ein_ausgabe(&e))?;
    let letzte = letzte_zeile(&mut datei, laenge)?.unwrap_or_default();
    let erste = erste_zeile(&mut datei)?;
    let (vorher_erste, von) = zeile_werte(erste.as_deref());
    let (_, bis) = zeile_werte(Some(&letzte));
    let hash = sha256_hex(&letzte);
    let monat = monat_aus_name(quelle, name).unwrap_or_default();
    let zeile = anker_zeile(name, monat, vorher_erste.as_deref(), von, bis, &hash)?;
    Ok((hash, zeile))
}

/// Opens a month file for reading and appending (a new one with `create_new`, mode 0600 on Unix).
fn monat_oeffnen(pfad: &Path, neu: bool) -> io::Result<File> {
    let mut optionen = OpenOptions::new();
    optionen.read(true).append(true).create_new(neu);
    #[cfg(unix)]
    optionen.mode(0o600);
    optionen.open(pfad)
}

/// The error for a month file that could not be opened: one that exists already is `abschnitt_vorhanden`
/// (somebody made it between the look and the `create_new`), any other is `ein_ausgabe`.
fn oeffnen_fehler(fehler: &io::Error) -> Fehler {
    if fehler.kind() == io::ErrorKind::AlreadyExists {
        Fehler::AbschnittVorhanden
    } else {
        ein_ausgabe(fehler)
    }
}

/// Opens the anchor file, which is also the lock of the source (a new one is empty, mode 0600 on Unix).
fn anker_oeffnen(pfad: &Path) -> Result<File, Fehler> {
    let mut optionen = OpenOptions::new();
    optionen.read(true).append(true).create(true);
    #[cfg(unix)]
    optionen.mode(0o600);
    optionen.open(pfad).map_err(|e| ein_ausgabe(&e))
}

/// Appends all lines (each with a line feed) in one write and syncs. On an error the file goes back to `laenge`
/// bytes; `None` means the call made the file, and it is removed again (on Unix only if the name still shows that file).
fn zeilen_schreiben(
    datei: &mut File,
    pfad: &Path,
    laenge: Option<u64>,
    zeilen: &[String],
) -> Result<(), Fehler> {
    let mut bytes = Vec::new();
    for zeile in zeilen {
        bytes.extend_from_slice(zeile.as_bytes());
        bytes.push(b'\n');
    }
    if let Err(fehler) = datei.write_all(&bytes).and_then(|()| datei.sync_data()) {
        match laenge {
            Some(alt) => {
                let _ = datei.set_len(alt);
            }
            None => {
                // Only the file this call made, and only if the name still shows it (not another file, not a link).
                #[cfg(unix)]
                super::loeschen::eigene_datei_entfernen(datei, pfad);
                #[cfg(not(unix))]
                let _ = fs::remove_file(pfad);
            }
        }
        return Err(ein_ausgabe(&fehler));
    }
    Ok(())
}

/// What the switch decided under the lock.
struct Wechsel {
    vorher: String,
    /// The anchor line of the month that closes, when it has none yet.
    anker: Option<String>,
}

/// Steps 3 to 6 of contract 5.3: the closed month, the month before, and `vorher`.
fn wechsel_entscheiden(
    ordner: &Path,
    quelle: &str,
    name: &str,
    eintraege: &[Eintrag],
    dateien: &[(String, u64)],
) -> Result<Wechsel, Fehler> {
    let hat_anker = |datei: &str| eintraege.iter().any(|e| e.abschnitt == datei);
    if dateien.iter().any(|(d, _)| d.as_str() > name) || hat_anker(name) {
        return Err(Fehler::ZeitRueckwaerts);
    }
    let vorgaenger = dateien
        .iter()
        .rev()
        .find(|(d, l)| d.as_str() < name && *l > 0);
    if let Some((_, laenge)) = dateien.iter().find(|(d, l)| d == name && *l > 0) {
        if vorgaenger.is_some_and(|(p, _)| !hat_anker(p)) {
            return Err(Fehler::AbschnittVorhanden);
        }
        let mut datei = File::open(ordner.join(name)).map_err(|e| ein_ausgabe(&e))?;
        let letzte = letzte_zeile(&mut datei, *laenge)?.unwrap_or_default();
        return Ok(Wechsel {
            vorher: sha256_hex(&letzte),
            anker: None,
        });
    }
    let Some((p, laenge)) = vorgaenger else {
        return Ok(Wechsel {
            vorher: NULL_HASH.to_owned(),
            anker: None,
        });
    };
    let (hash, zeile) = anker_aus_datei(&ordner.join(p), p, quelle, *laenge)?;
    let alt: Vec<&Eintrag> = eintraege.iter().filter(|e| e.abschnitt == *p).collect();
    let anker = match alt.as_slice() {
        [] => Some(zeile),
        [ein] if ein.letzte.as_deref() == Some(hash.as_str()) => None,
        _ => return Err(Fehler::AnkerFalsch),
    };
    Ok(Wechsel {
        vorher: hash,
        anker,
    })
}

/// Builds the finished lines for a `vorher`.
type Bauen<'a> = &'a dyn Fn(&str) -> Result<Vec<String>, Fehler>;

/// The whole write under the lock on the anchor file.
fn unter_sperre(
    anker: &mut File,
    ordner: &Path,
    quelle: &str,
    name: &str,
    erwartet: Option<&str>,
    bauen: Bauen<'_>,
) -> Result<String, Fehler> {
    let eintraege = anker_lesen(anker)?;
    let dateien: Vec<(String, u64)> = monatsnamen(ordner, quelle)
        .and_then(|namen| {
            namen
                .into_iter()
                .map(|n| fs::metadata(ordner.join(&n)).map(|m| (n, m.len())))
                .collect()
        })
        .map_err(|e| ein_ausgabe(&e))?;
    let entscheidung = wechsel_entscheiden(ordner, quelle, name, &eintraege, &dateien)?;
    if erwartet.is_some_and(|soll| soll != entscheidung.vorher) {
        return Err(Fehler::VorherAbweichend);
    }
    let zeilen = bauen(&entscheidung.vorher)?;
    if let Some(zeile) = &entscheidung.anker {
        let laenge = anker.metadata().map_err(|e| ein_ausgabe(&e))?.len();
        let pfad = ordner.join(anker_name(quelle));
        zeilen_schreiben(anker, &pfad, Some(laenge), std::slice::from_ref(zeile))?;
    }
    let pfad = ordner.join(name);
    let eigene = dateien.iter().find(|(d, _)| d == name);
    let mut datei = monat_oeffnen(&pfad, eigene.is_none()).map_err(|e| oeffnen_fehler(&e))?;
    zeilen_schreiben(&mut datei, &pfad, eigene.map(|(_, l)| *l), &zeilen)?;
    Ok(sha256_hex(zeilen.last().map_or(&[][..], String::as_bytes)))
}

fn schreiben(
    ordner: &Path,
    quelle: &str,
    zeit_ms: i64,
    erwartet: Option<&str>,
    bauen: Bauen<'_>,
) -> Result<String, Fehler> {
    let name = abschnitt(quelle, zeit_ms)?;
    let mut anker = anker_oeffnen(&ordner.join(anker_name(quelle)))?;
    anker.lock().map_err(|e| ein_ausgabe(&e))?;
    let ergebnis = unter_sperre(&mut anker, ordner, quelle, &name, erwartet, bauen);
    // Closing the handle also releases the lock; the explicit call keeps the order of the contract.
    let _ = anker.unlock();
    ergebnis
}

/// Appends the line `ausgabe.gesperrt` to the month file of its time in the folder of the source and
/// returns the sha256 of the line. Contract 5, section 5.3.
///
/// # Errors
/// The fields as [`zeile_gesperrt`] (`feld_ungueltig:<feld>`), then `feld_ungueltig:quelle`, `zeit_ungueltig`;
/// under the lock `letzte_zeile_unvollstaendig`, `anker_falsch`, `zeit_rueckwaerts`, `abschnitt_vorhanden`;
/// `ein_ausgabe` when a file cannot be opened, locked, read or written.
pub fn anhaengen_gesperrt_in(ordner: &Path, quelle: &str, g: &Gesperrt) -> Result<String, Fehler> {
    felder_pruefen(g, NULL_HASH)?;
    abschnitt(quelle, g.zeit_ms)?;
    schreiben(ordner, quelle, g.zeit_ms, None, &|vorher| {
        zeile_gesperrt(g, vorher).map(|zeile| vec![zeile])
    })
}

/// The finished line of a closed object: `fassung`, `vorher` and `zeit` set, canonical.
fn objekt_zeile(objekt: &Map<String, Value>, zeit_ms: i64, vorher: &str) -> Result<String, Fehler> {
    let mut map = objekt.clone();
    map.insert("fassung".to_owned(), json!(2));
    map.insert("vorher".to_owned(), json!(vorher));
    map.insert("zeit".to_owned(), json!(zeit_text(zeit_ms)?));
    if !zeilenart_ok(&map, true) {
        return Err(Fehler::FeldUngueltig("zeile"));
    }
    let zeile = kanonisch(&Value::Object(map))?;
    if zeile.len() > ZEILE_MAX {
        return Err(Fehler::FeldUngueltig("zeile"));
    }
    Ok(zeile)
}

/// Appends a closed JSON object of a line type: the writer sets `fassung` (2), `vorher` and `zeit` (from
/// `zeit_ms`) and writes it canonical. Returns the sha256 of the line. Contract 5, section 5.3.
///
/// # Errors
/// `feld_ungueltig:zeile` (no object, or a line `ausgabe.gesperrt` outside of its closed form, or longer than
/// 4194304 bytes), `feld_ungueltig:aktionsname`, `…:ergebnis` (not a text), `…:fassung`, `…:vorher`, `…:zeit`
/// (already in the object), `feld_ungueltig:quelle`, `zeit_ungueltig`; under the lock as
/// [`anhaengen_gesperrt_in`].
pub fn anhaengen_zeile(
    ordner: &Path,
    quelle: &str,
    zeit_ms: i64,
    zeile: &Value,
) -> Result<String, Fehler> {
    let Value::Object(objekt) = zeile else {
        return Err(Fehler::FeldUngueltig("zeile"));
    };
    for feld in ["aktionsname", "ergebnis"] {
        if !objekt.get(feld).is_some_and(Value::is_string) {
            return Err(Fehler::FeldUngueltig(feld));
        }
    }
    for feld in ["fassung", "vorher", "zeit"] {
        if objekt.contains_key(feld) {
            return Err(Fehler::FeldUngueltig(feld));
        }
    }
    abschnitt(quelle, zeit_ms)?;
    objekt_zeile(objekt, zeit_ms, NULL_HASH)?;
    schreiben(ordner, quelle, zeit_ms, None, &|vorher| {
        objekt_zeile(objekt, zeit_ms, vorher).map(|z| vec![z])
    })
}

/// Appends the line `kanal.abgewiesen` through [`anhaengen_zeile`] (so the closed form of the line is checked
/// there too) and returns the sha256 of the line. Contract 5, section 6.
///
/// # Errors
/// The fields in the order `grund`, `kanal`, `route`, `sitzung` (`feld_ungueltig:<feld>`), then
/// `feld_ungueltig:quelle`, `zeit_ungueltig`; under the lock as [`anhaengen_gesperrt_in`].
pub fn anhaengen_abgewiesen(ordner: &Path, quelle: &str, a: &Abgewiesen) -> Result<String, Fehler> {
    felder_abgewiesen(a, NULL_HASH)?;
    anhaengen_zeile(
        ordner,
        quelle,
        a.zeit_ms,
        &json!({
            "aktionsname": "kanal.abgewiesen",
            "ergebnis": "abgewiesen",
            "grund": a.grund,
            "kanal": a.kanal,
            "route": a.route,
            "sitzung": a.sitzung,
        }),
    )
}

/// Appends the line `probelauf.zaehlung` through [`anhaengen_zeile`] (so the closed form of the line is checked
/// there too) and returns the sha256 of the line. Contract 5, section 6.
///
/// # Errors
/// The fields in the order `programm`, `sitzung`, `anzahl` (`feld_ungueltig:<feld>`), then
/// `feld_ungueltig:quelle`, `zeit_ungueltig`; under the lock as [`anhaengen_gesperrt_in`].
pub fn anhaengen_probelauf(ordner: &Path, quelle: &str, p: &Probelauf) -> Result<String, Fehler> {
    felder_probelauf(p, NULL_HASH)?;
    anhaengen_zeile(
        ordner,
        quelle,
        p.zeit_ms,
        &json!({
            "aktionsname": "probelauf.zaehlung",
            "anzahl": p.anzahl,
            "ergebnis": "an",
            "programm": p.programm,
            "sitzung": p.sitzung,
        }),
    )
}

/// Appends the line `ordnermarke.geloest` or `ordnermarke.gesenkt` through [`anhaengen_zeile`] (so the closed
/// form of the line is checked there too) and returns the sha256 of the line. Contract 5, section 6.4.
///
/// # Errors
/// The fields as in `zeile_ordnermarke` (`feld_ungueltig:<feld>`), then `feld_ungueltig:quelle`,
/// `zeit_ungueltig`; under the lock as [`anhaengen_gesperrt_in`].
pub fn anhaengen_ordnermarke(
    ordner: &Path,
    quelle: &str,
    o: &Ordnermarke,
) -> Result<String, Fehler> {
    felder_ordnermarke(o, NULL_HASH)?;
    anhaengen_zeile(
        ordner,
        quelle,
        o.zeit_ms,
        &json!({
            "aktionsname": o.aktionsname,
            "ergebnis": "ausgefuehrt",
            "klasse_alt": o.klasse_alt,
            "klasse_neu": o.klasse_neu,
            "kunde_alt": o.kunde_alt,
            "kunde_neu": o.kunde_neu,
            "marke": o.marke,
        }),
    )
}

/// Appends lines that are already chained (each without line feed, Fassung 2) in one write under one lock.
/// `vorher_erwartet` is the sha256 the caller expects as the last line of the chain (64 zeros for a new chain);
/// when the last line under the lock is another, nothing is written (`vorher_abweichend`). All lines must lie in
/// the same month. Returns the sha256 of the last line. Contract 5, section 5.3.
///
/// # Errors
/// `feld_ungueltig:zeilen` (empty list), `feld_ungueltig:vorher`, `feld_ungueltig:quelle`;
/// `zeile_ungueltig:<grund>` with the reason of the check of section 3 for the first bad line;
/// `zeit_ungueltig`, `feld_ungueltig:zeit` (lines in different months); under the lock as
/// [`anhaengen_gesperrt_in`] and `vorher_abweichend`.
pub fn anhaengen_zeilen(
    ordner: &Path,
    quelle: &str,
    vorher_erwartet: &str,
    zeilen: &[String],
) -> Result<String, Fehler> {
    if zeilen.is_empty() {
        return Err(Fehler::FeldUngueltig("zeilen"));
    }
    if !ist_sha256(vorher_erwartet) {
        return Err(Fehler::FeldUngueltig("vorher"));
    }
    if !quelle_ok(quelle) {
        return Err(Fehler::FeldUngueltig("quelle"));
    }
    let mut erwartet = vorher_erwartet.to_owned();
    let mut monate: Vec<(i64, String)> = Vec::new();
    for zeile in zeilen {
        let (zeit, _) = if zeile.len() > ZEILE_MAX {
            Err("zeile_unlesbar".to_owned())
        } else {
            zeile_pruefen(zeile.as_bytes(), "brain-2026-01.jsonl", &erwartet)
        }
        .map_err(Fehler::ZeileUngueltig)?;
        erwartet = sha256_hex(zeile.as_bytes());
        let ms = zeit.ok_or(Fehler::ZeitUngueltig)?;
        monate.push((ms, abschnitt(quelle, ms)?));
    }
    let (ms, name) = &monate[0];
    if monate.iter().any(|(_, m)| m != name) {
        return Err(Fehler::FeldUngueltig("zeit"));
    }
    schreiben(ordner, quelle, *ms, Some(vorher_erwartet), &|_| {
        Ok(zeilen.to_vec())
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    use serde_json::{Map, Value, json};

    use super::super::pruefen::Dateiwerte;
    use super::{
        Fehler, anker_passt, anker_zeile, monat_aus_name, oeffnen_fehler, sekunden, sekunden_wert,
        zahl_gleich, zeilen_schreiben,
    };

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const NAME: &str = "brain-2026-11.jsonl";

    fn werte() -> Dateiwerte {
        Dateiwerte {
            erste_vorher: HASH_A.to_owned(),
            von: Some(1_795_179_600_000),
            bis: Some(1_795_179_601_500),
            letzte: HASH_B.to_owned(),
        }
    }

    fn zeile() -> String {
        let w = werte();
        anker_zeile(
            NAME,
            "2026-11",
            Some(&w.erste_vorher),
            w.von,
            w.bis,
            &w.letzte,
        )
        .expect("line")
    }

    fn karte(zeile: &str) -> Map<String, Value> {
        match serde_json::from_str(zeile).expect("JSON") {
            Value::Object(map) => map,
            _ => panic!("no object"),
        }
    }

    fn passt(zeile: &str) -> bool {
        anker_passt(zeile.as_bytes(), &karte(zeile), NAME, "brain", &werte())
    }

    #[test]
    fn month_file_names() {
        for (quelle, name, monat) in [
            ("brain", "brain-2026-11.jsonl", "2026-11"),
            ("brain", "brain-0000-01.jsonl", "0000-01"),
            ("gateway", "gateway-2027-12.jsonl", "2027-12"),
            ("stelle", "stelle-2026-09.jsonl", "2026-09"),
        ] {
            assert_eq!(monat_aus_name(quelle, name), Some(monat), "{name}");
        }
        for (quelle, name) in [
            ("brain", "brain-2026-13.jsonl"),
            ("brain", "brain-2026-00.jsonl"),
            ("brain", "brain-2026-11.jsonl.bak"),
            ("brain", "brain-2026-11.json"),
            ("brain", "brain-2026-1.jsonl"),
            ("brain", "brain2026-11.jsonl"),
            ("brain", "xbrain-2026-11.jsonl"),
            ("brain", "stelle-2026-11.jsonl"),
            ("brain", "brain.anker.jsonl"),
            ("brain", "brain-2026-11-2.jsonl"),
            ("brain", ""),
            ("stelle", "brain-2026-11.jsonl"),
        ] {
            assert_eq!(monat_aus_name(quelle, name), None, "{name}");
        }
    }

    #[test]
    fn seconds_are_a_division_of_doubles() {
        assert!((sekunden(1500) - 1.5).abs() < f64::EPSILON);
        assert!((sekunden(0)).abs() < f64::EPSILON);
        assert_eq!(sekunden_wert(None), Value::Null);
        assert_eq!(
            sekunden_wert(Some(1_795_179_600_000)),
            json!(1_795_179_600.0)
        );
        assert_eq!(
            sekunden_wert(Some(1_795_179_601_500)),
            json!(1_795_179_601.5)
        );
    }

    #[test]
    fn numbers_equal_by_double() {
        assert!(zahl_gleich(Some(&json!(null)), None));
        assert!(!zahl_gleich(Some(&json!(null)), Some(0)));
        assert!(!zahl_gleich(None, None));
        assert!(zahl_gleich(Some(&json!(1.5)), Some(1500)));
        assert!(zahl_gleich(Some(&json!(2)), Some(2000)));
        assert!(zahl_gleich(Some(&json!(2.0)), Some(2000)));
        assert!(!zahl_gleich(Some(&json!(1.5)), Some(1501)));
        assert!(!zahl_gleich(Some(&json!(1.5)), None));
        assert!(!zahl_gleich(Some(&json!("1.5")), Some(1500)));
        assert!(!zahl_gleich(Some(&json!(true)), Some(1)));
    }

    #[test]
    fn an_anchor_line_has_the_closed_form() {
        assert_eq!(
            zeile(),
            format!(
                "{{\"abschnitt\":\"{NAME}\",\"bis\":1795179601.5,\"fassung\":2,\"letzte\":\"{HASH_B}\",\"monat\":\"2026-11\",\"von\":1795179600,\"vorher_erste\":\"{HASH_A}\"}}"
            )
        );
        let leer = anker_zeile(NAME, "2026-11", None, None, None, HASH_B).expect("line");
        assert!(leer.contains("\"bis\":null,"), "{leer}");
        assert!(leer.contains("\"von\":null,"), "{leer}");
        assert!(leer.ends_with("\"vorher_erste\":null}"), "{leer}");
    }

    #[test]
    fn an_anchor_fits_when_every_field_fits() {
        assert!(passt(&zeile()));
        // Without `fassung` (the old form): no check of the bytes, only of the fields.
        let mut alt = karte(&zeile());
        alt.remove("fassung");
        let text = format!("{}", Value::Object(alt.clone()));
        let leer = text.replace(',', ", ");
        assert!(anker_passt(leer.as_bytes(), &alt, NAME, "brain", &werte()));
    }

    #[test]
    fn an_anchor_that_differs_in_one_place_does_not_fit() {
        let ganz = karte(&zeile());
        let aendern = |schluessel: &str, wert: Value| {
            let mut map = ganz.clone();
            map.insert(schluessel.to_owned(), wert);
            let text = crate::kette::kanonisch(&Value::Object(map.clone())).expect("canonical");
            anker_passt(text.as_bytes(), &map, NAME, "brain", &werte())
        };
        assert!(!aendern("abschnitt", json!("brain-2026-12.jsonl")));
        assert!(!aendern("abschnitt", json!(5)));
        assert!(!aendern("monat", json!("2026-10")));
        assert!(!aendern("vorher_erste", json!(HASH_B)));
        assert!(!aendern("vorher_erste", json!(null)));
        assert!(!aendern("letzte", json!(HASH_A)));
        assert!(!aendern("von", json!(1_795_179_600.5)));
        assert!(!aendern("von", json!(null)));
        assert!(!aendern("bis", json!(1_795_179_601.0)));
        assert!(!aendern("bis", json!(null)));
        assert!(!aendern("bis", json!("1795179601.5")));
        assert!(!aendern("fassung", json!(3)));
        assert!(!aendern("fassung", json!("2")));
        assert!(!aendern("titel", json!("x")));
        assert!(aendern("fassung", json!(2)));
    }

    #[test]
    fn an_anchor_with_missing_fields_or_wrong_bytes_does_not_fit() {
        let ganz = karte(&zeile());
        for fehlt in ["abschnitt", "bis", "letzte", "monat", "von", "vorher_erste"] {
            let mut map = ganz.clone();
            map.remove(fehlt);
            let text = crate::kette::kanonisch(&Value::Object(map.clone())).expect("canonical");
            assert!(
                !anker_passt(text.as_bytes(), &map, NAME, "brain", &werte()),
                "{fehlt}"
            );
        }
        // A space: the bytes are no longer the canonical form of the value.
        let weit = zeile().replace("\"monat\"", " \"monat\"");
        assert!(!passt(&weit));
        // Another source: the month name does not follow `<quelle>-JJJJ-MM.jsonl`.
        assert!(!anker_passt(
            zeile().as_bytes(),
            &ganz,
            NAME,
            "stelle",
            &werte()
        ));
    }

    #[test]
    fn a_month_file_that_cannot_be_opened_is_an_error() {
        let vorhanden = io::Error::from(io::ErrorKind::AlreadyExists);
        assert_eq!(oeffnen_fehler(&vorhanden), Fehler::AbschnittVorhanden);
        let anders = io::Error::from(io::ErrorKind::PermissionDenied);
        assert_eq!(oeffnen_fehler(&anders).code(), "ein_ausgabe");
        let nicht = io::Error::from(io::ErrorKind::NotFound);
        assert_eq!(oeffnen_fehler(&nicht).code(), "ein_ausgabe");
    }

    fn wegwerf(name: &str) -> PathBuf {
        let pfad =
            std::env::temp_dir().join(format!("formate-monate-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("folder");
        pfad
    }

    #[test]
    fn a_write_goes_in_one_piece() {
        let ordner = wegwerf("schreiben");
        let pfad = ordner.join("m.jsonl");
        fs::write(&pfad, "alt\n").expect("write");
        let mut datei = fs::OpenOptions::new()
            .append(true)
            .open(&pfad)
            .expect("open");
        zeilen_schreiben(
            &mut datei,
            &pfad,
            Some(4),
            &["a".to_owned(), "b".to_owned()],
        )
        .expect("write");
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "alt\na\nb\n");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_failed_write_is_taken_back() {
        // /dev/full accepts every open and refuses every write with "no space left".
        let ordner = wegwerf("zurueck");
        let mut voll = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .expect("open /dev/full");
        // A file this call made is removed again. (The handle is read-only, so the write fails; it is the handle of the
        // file at the path, because on Unix only the file that the caller holds is removed.)
        let neu = ordner.join("neu.jsonl");
        fs::write(&neu, "").expect("write");
        #[cfg(unix)]
        let mut nur_lesen = fs::File::open(&neu).expect("open");
        #[cfg(unix)]
        let r = zeilen_schreiben(&mut nur_lesen, &neu, None, &["a".to_owned()]);
        #[cfg(not(unix))]
        let r = zeilen_schreiben(&mut voll, &neu, None, &["a".to_owned()]);
        assert_eq!(
            r.as_ref().map_err(Fehler::code),
            Err("ein_ausgabe".to_owned()),
            "{r:?}"
        );
        assert!(!neu.exists(), "the new file is removed");
        // A file that was there stays.
        let alt = ordner.join("alt.jsonl");
        fs::write(&alt, "alt\n").expect("write");
        let r = zeilen_schreiben(&mut voll, &alt, Some(4), &["a".to_owned()]);
        assert_eq!(
            r.as_ref().map_err(Fehler::code),
            Err("ein_ausgabe".to_owned()),
            "{r:?}"
        );
        assert!(alt.exists(), "the old file stays");
        let _ = fs::remove_dir_all(&ordner);
    }
}
