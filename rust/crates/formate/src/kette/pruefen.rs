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

/// The fields of a line `modellweg.aufruf` of mode 1 `openai-lokal`, in the order of the canonical bytes
/// (contract 8, sections 8 and 11).
const MODELLWEG_1: [&str; 15] = [
    "aktionsname",
    "betriebsart",
    "ergebnis",
    "fassung",
    "grund",
    "host",
    "modell",
    "pfad",
    "route",
    "sitzung",
    "status",
    "token_aus",
    "token_ein",
    "vorher",
    "zeit",
];

/// The fields of a line `modellweg.aufruf` of mode 2 `kopf-einsetzen` (contract 8, sections 8 and 13.4).
const MODELLWEG_2: [&str; 16] = [
    "aktionsname",
    "betriebsart",
    "ergebnis",
    "fassung",
    "grund",
    "host",
    "kopf",
    "methode",
    "pfad",
    "pfadregel",
    "route",
    "sitzung",
    "status",
    "vorher",
    "zeit",
    "ziel",
];

/// The results of a line `modellweg.aufruf` (contract 8, section 8).
const ERGEBNISSE_MODELLWEG: [&str; 4] = [
    "weitergereicht",
    "abgelehnt",
    "ziel_fehler",
    "leck_gesperrt",
];

/// A value of the field `pfad` (contract 8, section 8): `*`, `/`, or `/` and segments that are each `*` or of the
/// form `[A-Za-z0-9._~-]+`, in all at most 512 bytes. So no query, no fragment and no percent encoding gets into a
/// line, in neither form of the field.
fn pfad_feld_ok(wert: &str) -> bool {
    if wert == "*" || wert == "/" {
        return true;
    }
    wert.len() <= 512
        && wert.strip_prefix('/').is_some_and(|rest| {
            rest.split('/').all(|teil| {
                teil == "*"
                    || (!teil.is_empty()
                        && teil
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"._~-".contains(&c)))
            })
        })
}

/// A path of a rule of the target list (contract 8, section 12): `/*`, or segments `/[A-Za-z0-9._~-]+` with an
/// optional `/*` at the end. So the path of a request never stands in the field `pfadregel`.
fn regel_pfad_feld_ok(wert: &str) -> bool {
    if wert == "/*" {
        return true;
    }
    let rest = wert.strip_suffix("/*").unwrap_or(wert);
    !rest.is_empty()
        && rest.strip_prefix('/').is_some_and(|ohne| {
            ohne.split('/').all(|teil| {
                !teil.is_empty()
                    && teil != "."
                    && teil != ".."
                    && teil
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"._~-".contains(&c))
            })
        })
}

/// A host of the target list (contract 8, section 12): lower case, with a dot, without a port.
fn host_feld_ok(wert: &str) -> bool {
    let label = |l: &str| {
        let b = l.as_bytes();
        let rand = |c: Option<&u8>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        b.len() <= 63
            && rand(b.first())
            && rand(b.last())
            && b.iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
    };
    wert.len() <= 253 && wert.contains('.') && wert.split('.').all(label)
}

/// A field that is a text of a form or `null`.
fn text_oder_null(map: &Map<String, Value>, schluessel: &str, form: impl Fn(&str) -> bool) -> bool {
    match map.get(schluessel) {
        Some(Value::Null) => true,
        Some(Value::String(wert)) => form(wert),
        _ => false,
    }
}

/// The line is a closed `modellweg.aufruf` line of version 2 (contract 8, sections 8, 11 and 13.4). The mode names
/// the form: 15 fields for `openai-lokal`, 16 for `kopf-einsetzen`. A line of an older model without `pfad` and
/// `host` keeps the form it had (13 and 14 fields), so a chain written before them stays whole; the coupling of the
/// fields belongs to the writer (crate `modellweg`), not to this check.
pub(super) fn modellweg_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    let Some(art @ ("openai-lokal" | "kopf-einsetzen")) = text("betriebsart") else {
        return false;
    };
    let neu = map.contains_key("pfad") || map.contains_key("host");
    let felder: &[&str] = if art == "openai-lokal" {
        &MODELLWEG_1
    } else {
        &MODELLWEG_2
    };
    let ohne: Vec<&str> = felder
        .iter()
        .copied()
        .filter(|f| neu || !["pfad", "host"].contains(f))
        .collect();
    if map.len() != ohne.len() || !ohne.iter().all(|f| map.contains_key(*f)) {
        return false;
    }
    let status_ok = match map.get("status") {
        Some(Value::Null) => art == "kopf-einsetzen",
        Some(zahl) => zahl.as_u64().is_some_and(|s| (100..=599).contains(&s)),
        None => false,
    };
    let zahl_oder_null = |schluessel: &str| match map.get(schluessel) {
        Some(Value::Null) => true,
        Some(zahl) => zahl
            .as_u64()
            .is_some_and(|n| u32::try_from(n).is_ok() && zahl.is_u64()),
        None => false,
    };
    let eigen = if art == "openai-lokal" {
        // This mode has exactly one target from `--ziel`, so its line never names a host.
        text("modell").is_some_and(|m| (1..=128).contains(&m.len()))
            && zahl_oder_null("token_ein")
            && zahl_oder_null("token_aus")
            && (!neu || map.get("host") == Some(&Value::Null))
    } else {
        text_oder_null(map, "ziel", schluessel_ok)
            && text_oder_null(map, "methode", |m| {
                ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"].contains(&m)
            })
            && text_oder_null(map, "pfadregel", regel_pfad_feld_ok)
            && text_oder_null(map, "kopf", |k| {
                ["ersetzt", "keiner", "abgewiesen", "halter"].contains(&k)
            })
    };
    eigen
        && status_ok
        && text("ergebnis").is_some_and(|e| ERGEBNISSE_MODELLWEG.contains(&e))
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && text_oder_null(map, "grund", schluessel_ok)
        && ["route", "sitzung"]
            .iter()
            .all(|k| text(k).is_some_and(schluessel_ok))
        && (!neu
            || (text_oder_null(map, "pfad", pfad_feld_ok)
                && text_oder_null(map, "host", host_feld_ok)))
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// The fields of a line `mcp.schreibaufruf` (contract 5, section 6.8; contract 6, section 7.1).
const SCHREIBAUFRUF_FELDER: [&str; 14] = [
    "aktionsname",
    "anfrage",
    "digest",
    "entscheidend",
    "ergebnis",
    "fassung",
    "person",
    "server",
    "sitzung",
    "vorher",
    "werkzeug",
    "zeit",
    "zeit_anfrage",
    "zeit_entscheidung",
];

/// The fields of a line `sitzung.gestartet` or `sitzung.abgelehnt` (contract 5, section 6.8; contract 6,
/// section 7.2).
const SITZUNG_FELDER: [&str; 10] = [
    "aktionsname",
    "ergebnis",
    "fassung",
    "grund",
    "harness",
    "kunde",
    "route",
    "sitzung",
    "vorher",
    "zeit",
];

/// The fields of a line `vorgang.abgelehnt` (contract 5, section 6.8; contract 6, section 7.3).
const VORGANG_FELDER: [&str; 9] = [
    "aktionsname",
    "anfrage",
    "aufrufer",
    "ergebnis",
    "fassung",
    "grund",
    "vorgang",
    "vorher",
    "zeit",
];

/// The kinds of caller of the point (contract 6, section 3).
const AUFRUFER_STELLE: [&str; 6] = ["kern", "hub", "mensch", "agent", "takt", "unbekannt"];

/// The operations whose refusal is a line `vorgang.abgelehnt` (contract 6, section 4). A refused
/// `sitzung.starten` is a line `sitzung.abgelehnt`, so its name never stands in this field.
const VORGAENGE_STELLE: [&str; 6] = [
    "mcp.anfragen",
    "mcp.entscheiden",
    "mcp.einloesen",
    "mcp.offene",
    "mcp.stand",
    "ablauf.pruefen",
];

/// The results of a line `mcp.schreibaufruf` (contract 6, section 7.1).
const ERGEBNISSE_SCHREIBAUFRUF: [&str; 4] =
    ["eingeloest", "abgelehnt", "abgelaufen", "ohne_freigabe"];

/// A request identifier of the point (contract 6, section 4): `^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$`.
fn anfrage_kennung_ok(wert: &str) -> bool {
    let ziffern = |text: &str, hoechstens: usize| {
        (1..=hoechstens).contains(&text.len()) && text.bytes().all(|c| c.is_ascii_digit())
    };
    wert.strip_prefix("mf-")
        .and_then(|rest| rest.split_once('-'))
        .is_some_and(|(laden, n)| ziffern(laden, 15) && ziffern(n, 10) && !n.starts_with('0'))
}

/// A tool name (contract 6, section 2): `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`.
fn werkzeug_ok(wert: &str) -> bool {
    let bytes = wert.as_bytes();
    bytes.len() <= 128
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(c))
}

/// The line holds exactly these fields, no further one and none missing.
fn felder_genau(map: &Map<String, Value>, felder: &[&str]) -> bool {
    map.len() == felder.len() && felder.iter().all(|f| map.contains_key(*f))
}

/// The line is a closed `mcp.schreibaufruf` line of version 2 (contract 5, section 6.8): fourteen fields, the
/// person only as a pseudonym, an argument only as `digest`, and the decider coupled to the time of the decision.
pub(super) fn schreibaufruf_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    let entscheidend = match map.get("entscheidend") {
        Some(Value::Null) => false,
        Some(Value::String(wert)) if ist_pseudonym(wert) => true,
        _ => return false,
    };
    let zeit_entscheidung = match map.get("zeit_entscheidung") {
        Some(Value::Null) => false,
        Some(Value::String(wert)) if zeit_muster(wert) => true,
        _ => return false,
    };
    // A redeemed or refused request names its decider, one without a release names none; a request that ran out
    // may name one (released, not redeemed) or none (nobody decided).
    let kopplung = match text("ergebnis") {
        Some("eingeloest" | "abgelehnt") => entscheidend,
        Some("ohne_freigabe") => !entscheidend,
        Some("abgelaufen") => true,
        _ => false,
    };
    felder_genau(map, &SCHREIBAUFRUF_FELDER)
        && text("aktionsname") == Some("mcp.schreibaufruf")
        && text("ergebnis").is_some_and(|e| ERGEBNISSE_SCHREIBAUFRUF.contains(&e))
        && kopplung
        && entscheidend == zeit_entscheidung
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && text("anfrage").is_some_and(anfrage_kennung_ok)
        && text("digest").is_some_and(ist_sha256)
        && text("person").is_some_and(ist_pseudonym)
        && ["server", "sitzung"]
            .iter()
            .all(|k| text(k).is_some_and(schluessel_ok))
        && text("werkzeug").is_some_and(werkzeug_ok)
        && text("zeit_anfrage").is_some_and(zeit_muster)
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// The line is a closed `sitzung.gestartet` or `sitzung.abgelehnt` line of version 2 (contract 5, section 6.8):
/// ten fields; `grund` is `null` exactly when the session started.
pub(super) fn sitzung_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    let Some(art @ ("sitzung.gestartet" | "sitzung.abgelehnt")) = text("aktionsname") else {
        return false;
    };
    let gestartet = art == "sitzung.gestartet";
    let grund_ok = match map.get("grund") {
        Some(Value::Null) => gestartet,
        Some(Value::String(wert)) => !gestartet && schluessel_ok(wert),
        _ => false,
    };
    let ergebnis = if gestartet { "gestartet" } else { "abgelehnt" };
    felder_genau(map, &SITZUNG_FELDER)
        && text("ergebnis") == Some(ergebnis)
        && grund_ok
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && ["harness", "kunde", "route", "sitzung"]
            .iter()
            .all(|k| schluessel_oder_null(map, k))
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// The line is a closed `vorgang.abgelehnt` line of version 2 (contract 5, section 6.8): nine fields, the caller
/// only as a fixed kind, the operation only as one of the six names of contract 6.
pub(super) fn vorgang_form(map: &Map<String, Value>) -> bool {
    let text = |schluessel: &str| map.get(schluessel).and_then(Value::as_str);
    felder_genau(map, &VORGANG_FELDER)
        && text("aktionsname") == Some("vorgang.abgelehnt")
        && text("ergebnis") == Some("abgelehnt")
        && map.get("fassung").and_then(Value::as_u64) == Some(2)
        && text_oder_null(map, "anfrage", anfrage_kennung_ok)
        && text("aufrufer").is_some_and(|a| AUFRUFER_STELLE.contains(&a))
        && text("grund").is_some_and(schluessel_ok)
        && text("vorgang").is_some_and(|v| VORGAENGE_STELLE.contains(&v))
        && text("vorher").is_some_and(ist_sha256)
        && text("zeit").is_some_and(zeit_muster)
}

/// Kinds of version 2 that contract 5 names but whose closed form has no check here yet (register group
/// „angemeldet, Form noch nicht geprüft", contract 5, section 3). Their line passes as a chain link; their form
/// comes with the build of their writer.
const REGISTER_ANGEMELDET: [&str; 3] = ["regel.geaendert", "regel.abgelehnt", "suche.rueckfall"];

/// Kinds of version 2 whose form another contract lays down (register group „in anderen Verträgen festgelegt",
/// contract 5, section 3). This check only knows that they exist, so a chain that holds them stays whole.
const REGISTER_FREMD: [&str; 15] = [
    "hersteller.zugang",
    "hersteller.zugriff",
    "pflicht.aenderung",
    "protokoll.einsicht",
    "vorgang.gedrosselt",
    "modell.aufruf",
    "fall.freitext",
    "mcp.schein_abgelehnt",
    "halter.leck_gesperrt",
    "halter.pfad_verboten",
    "wissen.geschrieben",
    "wissen.schreiben.abgelehnt",
    "klasse.erlaubt",
    "lauf.melden",
    "sitzung.abgemeldet",
];

/// Whether a line of a closed kind (`ausgabe.gesperrt`, `kanal.abgewiesen`, `probelauf.zaehlung`,
/// `ordnermarke.geloest`, `ordnermarke.gesenkt`, `person.geloescht`, `person.auskunft`, `modellweg.aufruf`,
/// `mcp.schreibaufruf`, `sitzung.gestartet`, `sitzung.abgelehnt`, `vorgang.abgelehnt`) has the closed form of
/// version 2, and whether a line of version 2 carries a kind of the register at all (contract 5, section 3,
/// decision of the operator, 23.11.2026: fail closed for version 2). A line of version 1g or 1s comes here with `fassung_2`
/// false and passes, because no register holds for it.
pub(super) fn zeilenart_ok(map: &Map<String, Value>, fassung_2: bool) -> bool {
    match map.get("aktionsname").and_then(Value::as_str) {
        Some("ausgabe.gesperrt") => fassung_2 && gesperrt_form(map),
        Some("kanal.abgewiesen") => fassung_2 && abgewiesen_form(map),
        Some("probelauf.zaehlung") => fassung_2 && probelauf_form(map),
        Some("ordnermarke.geloest" | "ordnermarke.gesenkt") => fassung_2 && ordnermarke_form(map),
        Some("person.geloescht") => fassung_2 && person_geloescht_form(map),
        Some("person.auskunft") => fassung_2 && person_auskunft_form(map),
        Some("modellweg.aufruf") => fassung_2 && modellweg_form(map),
        Some("mcp.schreibaufruf") => fassung_2 && schreibaufruf_form(map),
        Some("sitzung.gestartet" | "sitzung.abgelehnt") => fassung_2 && sitzung_form(map),
        Some("vorgang.abgelehnt") => fassung_2 && vorgang_form(map),
        // Register: a version 2 line whose kind contract 5 does not name is `form_falsch`. So a kind with a
        // typo, such as `mcp.schreibaufrufe` with a `vorschau`, no longer passes.
        Some(art) => {
            !fassung_2 || REGISTER_ANGEMELDET.contains(&art) || REGISTER_FREMD.contains(&art)
        }
        // No `aktionsname` of its own: a version 2 line fails that step before this one.
        None => true,
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
    use serde_json::{Map, Value, json};

    use super::{
        LESE_GRENZE, ZEILE_MAX, jahr_monat, modellweg_form, name_ohne_fassung, pfad_feld_ok,
        zeilenart_ok,
    };

    /// A line `modellweg.aufruf` of mode 1 with the 15 fields of the closed form.
    fn mw1() -> Map<String, Value> {
        let o = json!({
            "aktionsname": "modellweg.aufruf",
            "betriebsart": "openai-lokal",
            "ergebnis": "abgelehnt",
            "fassung": 2,
            "grund": "mw.pfad_unbekannt",
            "host": Value::Null,
            "modell": "lokal-beispiel/modell-a",
            "pfad": "/v1/models",
            "route": "lokal-beispiel",
            "sitzung": "s-2",
            "status": 404,
            "token_aus": Value::Null,
            "token_ein": Value::Null,
            "vorher": "0".repeat(64),
            "zeit": "2026-11-20T15:00:00.000Z",
        });
        o.as_object().expect("object").clone()
    }

    /// A line of mode 2 with the 16 fields of the closed form.
    fn mw2() -> Map<String, Value> {
        let o = json!({
            "aktionsname": "modellweg.aufruf",
            "betriebsart": "kopf-einsetzen",
            "ergebnis": "weitergereicht",
            "fassung": 2,
            "grund": Value::Null,
            "host": "api.beispiel.invalid",
            "kopf": "ersetzt",
            "methode": "GET",
            "pfad": "/models/session",
            "pfadregel": "/models/*",
            "route": "cloud-beispiel",
            "sitzung": "s-1",
            "status": 200,
            "vorher": "0".repeat(64),
            "zeit": "2026-11-20T16:00:00.000Z",
            "ziel": "ziel-1",
        });
        o.as_object().expect("object").clone()
    }

    #[test]
    fn a_line_of_the_model_path_keeps_its_closed_form() {
        assert!(modellweg_form(&mw1()));
        assert!(modellweg_form(&mw2()));
        for (feld, wert) in [
            ("betriebsart", json!("kopf-einsetzen")),
            ("ergebnis", json!("ok")),
            ("fassung", json!(1)),
            ("grund", json!("Mw.Pfad")),
            ("host", json!("api.beispiel.invalid")),
            ("modell", json!("")),
            ("pfad", json!("/v1/models?a=b")),
            ("route", json!("Lokal")),
            ("sitzung", json!(2)),
            ("status", json!(600)),
            ("token_ein", json!(-1)),
            ("vorher", json!("x")),
            ("zeit", json!("2026-11-20T15:00:00Z")),
        ] {
            let mut map = mw1();
            map.insert(feld.to_owned(), wert.clone());
            assert!(!modellweg_form(&map), "{feld} {wert}");
        }
        for (feld, wert) in [
            ("host", json!("api.beispiel.invalid:443")),
            ("host", json!("API.beispiel.invalid")),
            ("host", json!("ohnepunkt")),
            ("methode", json!("PROPFIND")),
            ("kopf", json!("ersetz")),
            ("pfadregel", json!("/user/repos?per_page=100")),
            ("pfadregel", json!("/a/../b")),
            ("status", json!(0)),
        ] {
            let mut map = mw2();
            map.insert(feld.to_owned(), wert.clone());
            assert!(!modellweg_form(&map), "{feld} {wert}");
        }
        // The status is null only in mode 2 (`mw.sni_falsch`).
        let mut ohne_status = mw2();
        ohne_status.insert("status".to_owned(), Value::Null);
        assert!(modellweg_form(&ohne_status));
        let mut eins_ohne_status = mw1();
        eins_ohne_status.insert("status".to_owned(), Value::Null);
        assert!(!modellweg_form(&eins_ohne_status));
        // A field more or a field less breaks the form.
        let mut mehr = mw1();
        mehr.insert("titel".to_owned(), json!("geheim"));
        assert!(!modellweg_form(&mehr));
        let mut weniger = mw2();
        weniger.remove("ziel");
        assert!(!modellweg_form(&weniger));
    }

    /// The register of the field `kopf` takes `halter` since 25.11.2026: a bound target of mode 2 on the holder
    /// path, whose header the holder sets (contract 8, section 13.4). A value outside the register stays refused, so
    /// the register keeps failing closed.
    #[test]
    fn the_register_of_the_head_takes_the_holder() {
        for wert in ["ersetzt", "keiner", "abgewiesen", "halter"] {
            let mut map = mw2();
            map.insert("kopf".to_owned(), json!(wert));
            assert!(modellweg_form(&map), "{wert}");
        }
        let mut ohne = mw2();
        ohne.insert("kopf".to_owned(), Value::Null);
        assert!(modellweg_form(&ohne));
        for wert in ["halters", "Halter", "halter ", "", "ersetz"] {
            let mut map = mw2();
            map.insert("kopf".to_owned(), json!(wert));
            assert!(!modellweg_form(&map), "{wert}");
        }
    }

    #[test]
    fn an_older_line_without_path_and_host_keeps_its_form() {
        for mut map in [mw1(), mw2()] {
            map.remove("pfad");
            map.remove("host");
            assert!(modellweg_form(&map), "{map:?}");
            map.insert("pfad".to_owned(), Value::Null);
            assert!(!modellweg_form(&map), "only the path: {map:?}");
            map.remove("pfad");
            map.insert("host".to_owned(), Value::Null);
            assert!(!modellweg_form(&map), "only the host: {map:?}");
        }
    }

    #[test]
    fn a_host_may_carry_digits_and_hyphens_inside_a_label() {
        for gut in [
            "api-2.beispiel.invalid",
            "a1b.c-d.invalid",
            "x9.beispiel.invalid",
        ] {
            let mut map = mw2();
            map.insert("host".to_owned(), json!(gut));
            assert!(modellweg_form(&map), "{gut}");
        }
        for falsch in [
            "-a.beispiel.invalid",
            "a-.beispiel.invalid",
            "a_b.beispiel.invalid",
        ] {
            let mut map = mw2();
            map.insert("host".to_owned(), json!(falsch));
            assert!(!modellweg_form(&map), "{falsch}");
        }
    }

    #[test]
    fn a_token_count_stays_within_u32() {
        for feld in ["token_ein", "token_aus"] {
            let mut ok = mw1();
            ok.insert(feld.to_owned(), json!(4_294_967_295_u64));
            assert!(modellweg_form(&ok), "{feld} u32::MAX");
            let mut zu_gross = mw1();
            zu_gross.insert(feld.to_owned(), json!(4_294_967_296_u64));
            assert!(!modellweg_form(&zu_gross), "{feld} u32::MAX + 1");
        }
    }

    #[test]
    fn a_path_in_a_line_carries_no_query_and_no_encoding() {
        for gut in [
            "*",
            "/",
            "/a",
            "/a/b",
            "/a/*",
            "/*/b",
            "/a/b/c/d",
            "/a.b~c-d_e",
        ] {
            assert!(pfad_feld_ok(gut), "{gut}");
        }
        for falsch in [
            "", "a", "/a?b=c", "/a#b", "/%2e", "/a b", "/a/", "/a//b", "**", "/a:b",
        ] {
            assert!(!pfad_feld_ok(falsch), "{falsch}");
        }
        let lang = format!("/{}", "a".repeat(511));
        assert!(pfad_feld_ok(&lang));
        assert!(!pfad_feld_ok(&format!("{lang}a")));
    }

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

    /// The kind of a hit of the leak lock of the holder is in the register (its form stands in the holder contract,
    /// section 9); a kind with a typo stays `form_falsch` in version 2, and version 1 lines pass without a register.
    #[test]
    fn die_art_des_leck_treffers_des_halters_steht_im_register() {
        let zeile = |art: &str| {
            json!({"aktionsname": art, "fassung": 2})
                .as_object()
                .expect("object")
                .clone()
        };
        assert!(zeilenart_ok(&zeile("halter.leck_gesperrt"), true));
        assert!(!zeilenart_ok(&zeile("halter.leck_gesperr"), true));
        assert!(zeilenart_ok(&zeile("halter.leck_gesperr"), false));
    }

    /// The kind of a request the holder refused for a path outside the rules is in the register (its form stands in
    /// the holder contract, section 9); a kind with a typo stays `form_falsch` in version 2.
    #[test]
    fn die_art_des_verbotenen_pfads_des_halters_steht_im_register() {
        let zeile = |art: &str| {
            json!({"aktionsname": art, "fassung": 2})
                .as_object()
                .expect("object")
                .clone()
        };
        assert!(zeilenart_ok(&zeile("halter.pfad_verboten"), true));
        assert!(!zeilenart_ok(&zeile("halter.pfad_verbote"), true));
        assert!(zeilenart_ok(&zeile("halter.pfad_verbote"), false));
    }
}
