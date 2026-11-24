//! The check of a chain: files in the given order as one chain, the first break ends it.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use serde_json::{Map, Value, json};

use super::datum::text_zeit;
use super::hash::sha256_hex;
use super::{
    ANZAHL_PROBELAUF_MAX, PROGRAMME_PROBELAUF, grund_abgewiesen_ok, ist_sha256, kanonisch,
    marke_ok, ordnermarke_kopplung, schluessel_ok, zeit_muster,
};

/// The longest line, in bytes without the line feed, that the check reads.
pub const ZEILE_MAX: usize = 4_194_304;

/// How many bytes one read takes: a line of `ZEILE_MAX` bytes, its line feed and one byte more, so that a too long
/// line shows (the unit test `read_limit_is_the_line_limit_plus_two` ties it to `ZEILE_MAX`).
const LESE_GRENZE: usize = 4_194_306;

/// The place of the first break.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bruch {
    /// The name of the file, without folder.
    pub datei: String,
    /// The code of the reason, such as `kette_gebrochen`.
    pub grund: String,
    /// The line, counted from 1 inside the file; `None` when the file is unreadable or `letzte_falsch` has no line.
    pub zeile: Option<u64>,
}

/// The result of a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pruefung {
    /// The lines that were good before the break (or all lines), over all files.
    pub zeilen: u64,
    /// The sha256 of the last line when the chain is whole and has lines, else `None`.
    pub letzte: Option<String>,
    /// The break, or `None` when the chain is whole.
    pub bruch: Option<Bruch>,
}

impl Pruefung {
    /// True when the chain is whole.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.bruch.is_none()
    }

    /// The output of `kette pruefe`: one line in RFC 8785 and a line feed.
    #[must_use]
    pub fn ausgabe(&self) -> String {
        let bruch = self.bruch.as_ref().map_or(
            Value::Null,
            |b| json!({"datei": b.datei, "grund": b.grund, "zeile": b.zeile}),
        );
        let wert = json!({
            "bruch": bruch,
            "letzte": self.letzte,
            "ok": self.ok(),
            "zeilen": self.zeilen,
        });
        let mut text = kanonisch(&wert).unwrap_or_default();
        text.push('\n');
        text
    }
}

/// What one read gave.
pub(super) enum Gelesen {
    /// No byte left.
    Ende,
    /// A line with its line feed, at most `ZEILE_MAX` bytes; the bytes are in the buffer.
    Zeile,
    /// A line with its line feed, longer than `ZEILE_MAX` bytes.
    ZuLang,
    /// Bytes up to the end of the file without a line feed.
    Unvollstaendig,
}

/// Reads one line without its line feed into `puffer`. Past `ZEILE_MAX` bytes the line is not kept: the rest of it is
/// read past, only to learn whether a line feed follows (the caller stops at the break, so the position after it
/// does not matter).
pub(super) fn zeile_lesen<R: BufRead>(leser: &mut R, puffer: &mut Vec<u8>) -> io::Result<Gelesen> {
    puffer.clear();
    let grenze = u64::try_from(LESE_GRENZE).unwrap_or(u64::MAX);
    let n = leser.by_ref().take(grenze).read_until(b'\n', puffer)?;
    if n == 0 {
        return Ok(Gelesen::Ende);
    }
    if puffer.last() == Some(&b'\n') {
        puffer.pop();
        return Ok(if puffer.len() > ZEILE_MAX {
            Gelesen::ZuLang
        } else {
            Gelesen::Zeile
        });
    }
    loop {
        let stueck = leser.fill_buf()?;
        if stueck.is_empty() {
            return Ok(Gelesen::Unvollstaendig);
        }
        if stueck.contains(&b'\n') {
            return Ok(Gelesen::ZuLang);
        }
        let alles = stueck.len();
        leser.consume(alles);
    }
}

/// `JJJJ-MM` with a month from 01 to 12.
pub(super) fn jahr_monat(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 7
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && matches!(&b[5..], [b'0', b'1'..=b'9'] | [b'1', b'0'..=b'2'])
}

/// `gateway-<JJJJ-MM>.jsonl` (version 1g) or `audit.jsonl`, `audit.<JJJJ-MM>[.<N>].jsonl` (version 1s):
/// the files whose lines may carry no `fassung`.
fn name_ohne_fassung(name: &str) -> bool {
    if let Some(mitte) = name
        .strip_prefix("gateway-")
        .and_then(|rest| rest.strip_suffix(".jsonl"))
    {
        return jahr_monat(mitte);
    }
    if name == "audit.jsonl" {
        return true;
    }
    let Some(mitte) = name
        .strip_prefix("audit.")
        .and_then(|rest| rest.strip_suffix(".jsonl"))
    else {
        return false;
    };
    match mitte.split_once('.') {
        Some((monat, nummer)) => {
            jahr_monat(monat) && !nummer.is_empty() && nummer.bytes().all(|c| c.is_ascii_digit())
        }
        None => jahr_monat(mitte),
    }
}

/// A field that is a key (`^[a-z0-9][a-z0-9._-]{0,63}$`) or `null`.
fn schluessel_oder_null(map: &Map<String, Value>, schluessel: &str) -> bool {
    match map.get(schluessel) {
        Some(Value::Null) => true,
        Some(Value::String(wert)) => schluessel_ok(wert),
        _ => false,
    }
}

/// The line is a closed `kanal.abgewiesen` line of version 2 (contract section 6).
pub(super) fn abgewiesen_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    map.len() == 9
        && text("aktionsname") == Some("kanal.abgewiesen")
        && text("ergebnis") == Some("abgewiesen")
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && matches!((text("kanal"), text("grund")), (Some(kanal), Some(grund)) if grund_abgewiesen_ok(kanal, grund))
        && text("kanal").is_some_and(schluessel_ok)
        && schluessel_oder_null(map, "route")
        && schluessel_oder_null(map, "sitzung")
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// The line is a closed `probelauf.zaehlung` line of version 2 (contract section 6): eight fields, no marker.
pub(super) fn probelauf_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    map.len() == 8
        && text("aktionsname") == Some("probelauf.zaehlung")
        && text("ergebnis") == Some("an")
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && map
            .get("anzahl")
            .and_then(Value::as_u64)
            .is_some_and(|a| (1..=u64::from(ANZAHL_PROBELAUF_MAX)).contains(&a))
        && text("programm").is_some_and(|p| PROGRAMME_PROBELAUF.contains(&p))
        && text("sitzung").is_some_and(schluessel_ok)
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// The line is a closed `ordnermarke.geloest` or `ordnermarke.gesenkt` line of version 2 (contract section 6.4):
/// ten fields, flat, the four values keys or `null`, `marke` 32 lower-case hex digits, the coupling of the kind.
pub(super) fn ordnermarke_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    let Some(art @ ("ordnermarke.geloest" | "ordnermarke.gesenkt")) = text("aktionsname") else {
        return false;
    };
    map.len() == 10
        && text("ergebnis") == Some("ausgefuehrt")
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && text("marke").is_some_and(marke_ok)
        && ["klasse_alt", "klasse_neu", "kunde_alt", "kunde_neu"]
            .iter()
            .all(|k| schluessel_oder_null(map, k))
        && ordnermarke_kopplung(
            art,
            text("klasse_alt"),
            text("klasse_neu"),
            text("kunde_alt"),
            text("kunde_neu"),
        )
        .is_none()
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// A pseudonym of a person: `ps_` and 32 lower-case hex digits (contract 6 and 9).
fn ist_pseudonym(text: &str) -> bool {
    text.strip_prefix("ps_").is_some_and(|hex| {
        hex.len() == 32 && hex.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// `handelnd` of a line of the stelle: a pseudonym, or `null` when `null_ok`.
fn handelnd_ok(map: &Map<String, Value>, null_ok: bool) -> bool {
    match map.get("handelnd") {
        Some(Value::Null) => null_ok,
        Some(Value::String(wert)) => ist_pseudonym(wert),
        _ => false,
    }
}

/// The line is a closed `person.geloescht` line of version 2 (contract 9, section 8): seven fields.
pub(super) fn person_geloescht_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    let marke_ok = match (text("ergebnis"), map.get("loeschmarke")) {
        (Some("geloescht"), Some(Value::String(marke))) => ist_sha256(marke),
        (Some("unbekannt"), Some(Value::Null)) => true,
        _ => false,
    };
    map.len() == 7
        && text("aktionsname") == Some("person.geloescht")
        && marke_ok
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && handelnd_ok(map, true)
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// The line is a closed `person.auskunft` line of version 2 (contract 9, section 8): seven fields.
pub(super) fn person_auskunft_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    let zeilen = map.get("zeilen").and_then(Value::as_u64);
    let zeilen_ok = match text("ergebnis") {
        Some("geliefert") => zeilen.is_some_and(|z| u32::try_from(z).is_ok()),
        Some("unbekannt") => zeilen == Some(0),
        _ => false,
    };
    map.len() == 7
        && text("aktionsname") == Some("person.auskunft")
        && zeilen_ok
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && handelnd_ok(map, false)
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// Whether a line of a closed kind (`ausgabe.gesperrt`, `kanal.abgewiesen`, `probelauf.zaehlung`,
/// `ordnermarke.geloest`, `ordnermarke.gesenkt`, `person.geloescht`, `person.auskunft`) has the closed form of version 2; every other kind passes.
pub(super) fn zeilenart_ok(map: &Map<String, Value>, fassung_2: bool) -> bool {
    match map.get("aktionsname").and_then(Value::as_str) {
        Some("ausgabe.gesperrt") => fassung_2 && gesperrt_form(map),
        Some("kanal.abgewiesen") => fassung_2 && abgewiesen_form(map),
        Some("probelauf.zaehlung") => fassung_2 && probelauf_form(map),
        Some("ordnermarke.geloest" | "ordnermarke.gesenkt") => fassung_2 && ordnermarke_form(map),
        Some("person.geloescht") => fassung_2 && person_geloescht_form(map),
        Some("person.auskunft") => fassung_2 && person_auskunft_form(map),
        _ => true,
    }
}

/// The line is a closed `ausgabe.gesperrt` line of version 2 (contract section 1).
fn gesperrt_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    let zahl = |schluessel: &str| map.get(schluessel).and_then(Value::as_u64);
    let oder_null = |schluessel: &str| schluessel_oder_null(map, schluessel);
    map.len() == 11
        && text("aktionsname") == Some("ausgabe.gesperrt")
        && text("ergebnis") == Some("gesperrt")
        && zahl("fassung") == Some(2)
        && zahl("anzahl").is_some_and(|a| (1..=u64::from(u32::MAX)).contains(&a))
        && ["grund", "route", "sitzung"]
            .iter()
            .all(|k| text(k).is_some_and(schluessel_ok))
        && oder_null("klasse")
        && oder_null("kunde")
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// The line as a JSON object, or `None` when it is no UTF-8, no JSON, no object, or holds a lone surrogate
/// (`serde_json` refuses the escapes `\ud800` to `\udfff` that stand alone).
pub(super) fn objekt_lesen(inhalt: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(inhalt).ok()?;
    let wert: Value = serde_json::from_str(text).ok()?;
    wert.is_object().then_some(wert)
}

/// The number 2 (as a double, so `2.0` counts too; the canonical form then refuses it).
pub(super) fn ist_zwei(wert: &Value) -> bool {
    matches!(wert, Value::Number(n) if n.as_f64().is_some_and(|f| f.to_bits() == 2f64.to_bits()))
}

/// The steps of the contract (section 3) for one line, in order. `Err` carries the code of the first that applies;
/// `Ok` carries the milliseconds of `zeit` when the field is a text in the pattern with a date since 1970, and
/// the line as a value.
pub(super) fn zeile_pruefen(
    inhalt: &[u8],
    name: &str,
    erwartet: &str,
) -> Result<(Option<i64>, Value), String> {
    let wert = objekt_lesen(inhalt).ok_or("zeile_unlesbar")?;
    let Value::Object(map) = &wert else {
        return Err("zeile_unlesbar".to_owned());
    };
    let fassung_2 = match map.get("fassung") {
        None => {
            if !name_ohne_fassung(name) {
                return Err("fassung_fehlt".to_owned());
            }
            false
        }
        Some(f) if ist_zwei(f) => true,
        Some(_) => return Err("form_falsch".to_owned()),
    };
    if let Some(bruch) = map.get("kettenbruch") {
        return Err(match bruch {
            Value::String(text) => format!("angekuendigt:{text}"),
            _ => "form_falsch".to_owned(),
        });
    }
    let vorher = map.get("vorher").ok_or("vorher_fehlt")?;
    if vorher.as_str() != Some(erwartet) {
        return Err("kette_gebrochen".to_owned());
    }
    if fassung_2 {
        if !kanonisch(&wert).is_ok_and(|bytes| bytes.as_bytes() == inhalt) {
            return Err("nicht_kanonisch".to_owned());
        }
        let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
        if !(text("zeit").is_some_and(zeit_muster)
            && text("aktionsname").is_some()
            && text("ergebnis").is_some())
        {
            return Err("form_falsch".to_owned());
        }
    }
    if !zeilenart_ok(map, fassung_2) {
        return Err("form_falsch".to_owned());
    }
    let zeit = map.get("zeit").and_then(Value::as_str).and_then(text_zeit);
    Ok((zeit, wert))
}

/// The state while the files are read.
pub(super) struct Zustand {
    pub(super) zeilen: u64,
    /// The `vorher` the next line must carry.
    pub(super) erwartet: String,
    /// File and number of the last good line.
    letzte: Option<(String, u64)>,
}

/// What the lines of one file with at least one line gave (for the anchor of the month).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Dateiwerte {
    /// The `vorher` of the first line.
    pub(super) erste_vorher: String,
    /// Milliseconds of `zeit` of the first and of the last line, when readable.
    pub(super) von: Option<i64>,
    pub(super) bis: Option<i64>,
    /// The sha256 of the last line.
    pub(super) letzte: String,
}

pub(super) fn bruch(datei: &str, zeile: Option<u64>, grund: &str) -> Bruch {
    Bruch {
        datei: datei.to_owned(),
        grund: grund.to_owned(),
        zeile,
    }
}

impl Zustand {
    pub(super) fn neu(vorher: &str) -> Self {
        Self {
            zeilen: 0,
            erwartet: vorher.to_owned(),
            letzte: None,
        }
    }

    /// Checks the lines of one file. `Err` is the first break; `Ok(None)` is a file without a line.
    pub(super) fn datei(&mut self, pfad: &Path, name: &str) -> Result<Option<Dateiwerte>, Bruch> {
        self.datei_mit(pfad, name, &mut |_, _| {})
    }

    /// As [`Zustand::datei`]; `sicht` sees every good line (the bytes and the value) after its check.
    pub(super) fn datei_mit(
        &mut self,
        pfad: &Path,
        name: &str,
        sicht: &mut dyn FnMut(&[u8], &Value),
    ) -> Result<Option<Dateiwerte>, Bruch> {
        let nicht_lesbar = |_: io::Error| bruch(name, None, "datei_nicht_lesbar");
        let mut leser = BufReader::new(File::open(pfad).map_err(nicht_lesbar)?);
        let mut puffer = Vec::new();
        let mut nummer: u64 = 0;
        let mut werte: Option<Dateiwerte> = None;
        loop {
            nummer += 1;
            let ergebnis = match zeile_lesen(&mut leser, &mut puffer).map_err(nicht_lesbar)? {
                Gelesen::Ende => return Ok(werte),
                Gelesen::Unvollstaendig => Err("letzte_zeile_unvollstaendig".to_owned()),
                Gelesen::ZuLang => Err("zeile_unlesbar".to_owned()),
                Gelesen::Zeile => zeile_pruefen(&puffer, name, &self.erwartet),
            };
            let (zeit, wert) = match ergebnis {
                Ok(geprueft) => geprueft,
                Err(grund) => return Err(bruch(name, Some(nummer), &grund)),
            };
            sicht(&puffer, &wert);
            let erste_vorher = werte
                .as_ref()
                .map_or_else(|| self.erwartet.clone(), |w| w.erste_vorher.clone());
            let von = werte.as_ref().map_or(zeit, |w| w.von);
            self.erwartet = sha256_hex(&puffer);
            werte = Some(Dateiwerte {
                erste_vorher,
                von,
                bis: zeit,
                letzte: self.erwartet.clone(),
            });
            self.zeilen += 1;
            self.letzte = Some((name.to_owned(), nummer));
        }
    }

    /// The end of a check: the result of the chain, or `letzte_falsch` when the sha256 of the last line is not
    /// the expected one. `letzter_name` is the file named when there was no good line.
    pub(super) fn abschluss(self, letzter_name: String, letzte: Option<&str>) -> Pruefung {
        let hash = self.letzte.as_ref().map(|_| self.erwartet.clone());
        if letzte.is_some_and(|soll| hash.as_deref() != Some(soll)) {
            let (datei, zeile) = match self.letzte {
                Some((datei, nummer)) => (datei, Some(nummer)),
                None => (letzter_name, None),
            };
            return Pruefung {
                zeilen: self.zeilen,
                letzte: None,
                bruch: Some(Bruch {
                    datei,
                    grund: "letzte_falsch".to_owned(),
                    zeile,
                }),
            };
        }
        Pruefung {
            zeilen: self.zeilen,
            letzte: hash,
            bruch: None,
        }
    }
}

pub(super) fn dateiname(pfad: &Path) -> String {
    pfad.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Checks the files in the given order as one chain.
///
/// `vorher` is the expected `vorher` of the first line ([`super::NULL_HASH`] for a whole chain). `letzte` is
/// the expected sha256 of the last line; without it a changed LAST line stays unseen, because no line follows
/// it. The function changes no file and reads no clock.
#[must_use]
pub fn pruefe<P: AsRef<Path>>(dateien: &[P], vorher: &str, letzte: Option<&str>) -> Pruefung {
    let mut zustand = Zustand::neu(vorher);
    let mut letzter_name = String::new();
    for pfad in dateien {
        letzter_name = dateiname(pfad.as_ref());
        if let Err(gebrochen) = zustand.datei(pfad.as_ref(), &letzter_name) {
            return Pruefung {
                zeilen: zustand.zeilen,
                letzte: None,
                bruch: Some(gebrochen),
            };
        }
    }
    zustand.abschluss(letzter_name, letzte)
}

#[cfg(test)]
mod tests {
    use super::{LESE_GRENZE, ZEILE_MAX, jahr_monat, name_ohne_fassung};

    #[test]
    fn read_limit_is_the_line_limit_plus_two() {
        assert_eq!(LESE_GRENZE, ZEILE_MAX + 2);
    }

    #[test]
    fn year_month() {
        for ok in ["2026-01", "2026-09", "2026-10", "2026-12", "0000-01"] {
            assert!(jahr_monat(ok), "{ok}");
        }
        for falsch in [
            "", "2026-00", "2026-13", "2026-1", "2026-011", "202a-01", "2026_01", "2026-0a",
            "2026-1a", "2026-20", "+026-01",
        ] {
            assert!(!jahr_monat(falsch), "{falsch}");
        }
    }

    #[test]
    fn names_that_may_carry_no_version() {
        for ok in [
            "gateway-2026-11.jsonl",
            "audit.jsonl",
            "audit.2026-11.jsonl",
            "audit.2026-11.1.jsonl",
            "audit.2026-11.12.jsonl",
        ] {
            assert!(name_ohne_fassung(ok), "{ok}");
        }
        for falsch in [
            "",
            "brain-2026-11.jsonl",
            "stelle-2026-11.jsonl",
            "gateway-2026-13.jsonl",
            "gateway-2026-11.json",
            "gateway-2026-11.jsonl.bak",
            "xgateway-2026-11.jsonl",
            "gateway.jsonl",
            "audit",
            "audit.json",
            "audit.2026-13.jsonl",
            "audit.2026-11..jsonl",
            "audit.2026-11.a.jsonl",
            "audit.2026-11.1.2.jsonl",
            "audit-2026-11.jsonl",
            "audit..jsonl",
        ] {
            assert!(!name_ohne_fassung(falsch), "{falsch}");
        }
    }
}
