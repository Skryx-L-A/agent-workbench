//! The manifest of the package: the seventeen stages of the contract, in the contract's order (section "Manifest").
//!
//! [`pruefen`] returns the code of the first violated stage (without the prefix `paket.`) or the parsed manifest.
//! The reader refuses a duplicate key on every level. The stages follow `rust/spec/paket/referenz.py`.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::ohne_doppel;

/// The largest manifest in bytes.
pub const GRENZE: usize = 1_048_576;

/// The only platform this program checks.
pub const PLATTFORM: &str = "ubuntu-26.04-x64";

/// The place of the manifest itself, relative to `/`.
pub const ORT: &str = "usr/share/werkbank/manifest.json";

const PLATTFORMEN: [&str; 3] = ["windows-x64", PLATTFORM, "macos-arm64"];
const ARTEN: [&str; 4] = ["programm", "dienst", "daten", "lizenz"];
const ORTE: [&str; 3] = ["manifest.json", ORT, "Library/Werkbank/manifest.json"];
const TOP: [&str; 8] = [
    "fassung",
    "paket",
    "version",
    "plattform",
    "bau",
    "merkmale",
    "dateien",
    "bibliotheken",
];
const DATEI: [&str; 4] = ["pfad", "groesse", "sha256", "art"];
const RESERVIERT: [&str; 4] = ["con", "prn", "aux", "nul"];

/// The files every Ubuntu manifest must list, with their kind.
pub const PFLICHT: [(&str, &str); 33] = [
    ("usr/bin/werkbank-probelauf", "programm"),
    ("usr/bin/werkbank-pruefen", "programm"),
    ("usr/bin/werkbank-ordnermarke", "programm"),
    ("usr/bin/werkbank-ca-zulassung", "programm"),
    ("usr/lib/werkbank/werkbank-lauf", "programm"),
    ("usr/lib/werkbank/werkbank-lauf-kind", "programm"),
    ("usr/lib/werkbank/werkbank-kern", "dienst"),
    ("usr/lib/werkbank/werkbank-bruecke", "dienst"),
    ("usr/lib/werkbank/werkbank-modellweg", "dienst"),
    ("usr/lib/werkbank/werkbank-halter", "dienst"),
    ("usr/lib/werkbank/rangkern", "programm"),
    ("usr/lib/werkbank/kette", "programm"),
    ("usr/lib/werkbank/such-mcp", "programm"),
    ("usr/lib/werkbank/probelauf/drehbuch.py", "programm"),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/ablauf.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/bericht.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/bruecke.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/fremd.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/konfig.py",
        "programm",
    ),
    ("usr/lib/werkbank/probelauf/drehbuchlib/lauf.py", "programm"),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/messen.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/modell.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/probelauf.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/schritte.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/seite.py",
        "programm",
    ),
    (
        "usr/lib/werkbank/probelauf/drehbuchlib/weiterreicher.py",
        "programm",
    ),
    ("usr/lib/werkbank/probelauf/drehbuchlib/ziel.py", "programm"),
    ("usr/share/werkbank/probelauf/einstellungen.json", "daten"),
    ("usr/share/werkbank/probelauf/konfig.json", "daten"),
    ("usr/lib/systemd/user/werkbank-kern.service", "daten"),
    ("usr/lib/systemd/user/werkbank-bruecke.service", "daten"),
    ("usr/lib/systemd/user/wb-halter.service", "daten"),
    ("usr/share/werkbank/sbom.cdx.json", "daten"),
];

/// The file of the `AppArmor` profile of form A (decision V1: the fixed starter carries it, `werkbank-lauf` has none).
pub const PROFIL_A: &str = "etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss";
/// The fixed starter of the enclosure; form A consists of its profile and this program.
pub const STARTER: &str = "usr/lib/werkbank/werkbank-einschluss";
/// The file of the `AppArmor` profile of form B.
pub const PROFIL_B: &str = "etc/apparmor.d/usr.lib.werkbank.bwrap";

/// One file of the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datei {
    /// Path relative to `/`.
    pub pfad: String,
    /// Size in bytes.
    pub groesse: u64,
    /// SHA-256 as 64 lower-case hex digits.
    pub sha256: String,
    /// `programm`, `dienst`, `daten` or `lizenz`.
    pub art: String,
}

/// A manifest that passed all stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Version `a.b.c`.
    pub version: String,
    /// `true` for `bau: test`.
    pub testbau: bool,
    /// The listed files.
    pub dateien: Vec<Datei>,
}

fn ganz(v: &Value) -> bool {
    v.is_i64() || v.is_u64()
}

fn kennung(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(c))
}

/// The form `a.b.c` without leading zeros; `a` and `b` up to 255, `c` up to 65535.
fn version_gut(s: &str) -> bool {
    let teile: Vec<&str> = s.split('.').collect();
    let [a, b, c] = teile.as_slice() else {
        return false;
    };
    let zahl = |t: &str, hoechst: u32, stellen: usize| {
        !t.is_empty()
            && t.len() <= stellen
            && t.bytes().all(|x| x.is_ascii_digit())
            && (t == "0" || !t.starts_with('0'))
            && t.parse::<u32>().is_ok_and(|n| n <= hoechst)
    };
    zahl(a, 255, 3) && zahl(b, 255, 3) && zahl(c, 65535, 5)
}

fn pfad_gut(p: &str, plattform: &str) -> bool {
    let mac = plattform == "macos-arm64";
    let eigene: Vec<&str> = if PLATTFORMEN.contains(&plattform) {
        vec![
            ORTE[PLATTFORMEN
                .iter()
                .position(|x| *x == plattform)
                .unwrap_or(0)],
        ]
    } else {
        ORTE.to_vec()
    };
    if p.len() > 200 || p.is_empty() || eigene.contains(&p) {
        return false;
    }
    p.split('/').all(|seg| {
        !seg.is_empty()
            && seg.bytes().all(|c| {
                let buchstabe = if mac {
                    c.is_ascii_alphabetic()
                } else {
                    c.is_ascii_lowercase()
                };
                buchstabe || c.is_ascii_digit() || b"._-".contains(&c)
            })
            && seg != "."
            && seg != ".."
            && !seg.ends_with('.')
            && !reserviert(seg.split('.').next().unwrap_or(""))
    })
}

fn reserviert(name: &str) -> bool {
    RESERVIERT.contains(&name)
        || ["com", "lpt"].iter().any(|p| {
            name.strip_prefix(p)
                .is_some_and(|n| n.len() == 1 && (b'1'..=b'9').contains(&n.as_bytes()[0]))
        })
}

fn sha_gut(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn text_feld<'a>(o: &'a serde_json::Map<String, Value>, k: &str) -> &'a str {
    o.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Stages 11 to 17 on the file entries: paths, hashes, sizes, doubles, forbidden features, required files.
fn dateien_pruefen(
    objekte: &[&serde_json::Map<String, Value>],
    plattform: &str,
    bau: &str,
    merkmale: &[&str],
) -> Result<(), &'static str> {
    if objekte
        .iter()
        .any(|d| !pfad_gut(text_feld(d, "pfad"), plattform))
    {
        return Err("pfad_form");
    }
    if objekte.iter().any(|d| !sha_gut(text_feld(d, "sha256"))) {
        return Err("pruefsumme_form");
    }
    if objekte.iter().any(|d| {
        !d["groesse"]
            .as_i64()
            .is_some_and(|g| (0..=2_147_483_647).contains(&g))
    }) {
        return Err("groesse_bereich");
    }
    let pfade: BTreeSet<String> = objekte
        .iter()
        .map(|d| text_feld(d, "pfad").to_lowercase())
        .collect();
    if pfade.len() != objekte.len() {
        return Err("pfad_doppelt");
    }
    if merkmale.iter().collect::<BTreeSet<_>>().len() != merkmale.len() {
        return Err("merkmal_doppelt");
    }
    if bau == "auslieferung"
        && merkmale
            .iter()
            .any(|k| *k == "attrappe" || k.starts_with("test"))
    {
        return Err("merkmal_verboten");
    }
    let da: BTreeSet<(&str, &str)> = objekte
        .iter()
        .map(|d| (text_feld(d, "pfad"), text_feld(d, "art")))
        .collect();
    if plattform == PLATTFORM
        && (PFLICHT.iter().any(|p| !da.contains(p))
            || !((da.contains(&(PROFIL_A, "daten")) && da.contains(&(STARTER, "programm")))
                || da.contains(&(PROFIL_B, "daten"))))
    {
        return Err("pflichtdatei_fehlt");
    }
    Ok(())
}

/// Checks the bytes of a manifest. `Err` holds the code of the first violated stage, without the prefix `paket.`.
///
/// # Errors
/// The code of the first violated stage.
pub fn pruefen(roh: &[u8]) -> Result<Manifest, &'static str> {
    if roh.len() > GRENZE {
        return Err("zu_gross");
    }
    if roh.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err("json_ungueltig");
    }
    let wert = ohne_doppel::lesen(roh).map_err(|_| "json_ungueltig")?;
    let Value::Object(m) = &wert else {
        return Err("json_ungueltig");
    };
    let liste: &[Value] = m
        .get("dateien")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    let objekte: Vec<&serde_json::Map<String, Value>> =
        liste.iter().filter_map(Value::as_object).collect();
    if m.keys().any(|k| !TOP.contains(&k.as_str()))
        || objekte
            .iter()
            .any(|d| d.keys().any(|k| !DATEI.contains(&k.as_str())))
    {
        return Err("schluessel_unbekannt");
    }
    if TOP.iter().any(|k| !m.contains_key(*k))
        || objekte
            .iter()
            .any(|d| DATEI.iter().any(|k| !d.contains_key(*k)))
    {
        return Err("schluessel_fehlt");
    }
    let merkmale = m["merkmale"].as_array();
    let typ_ok = ganz(&m["fassung"])
        && ["paket", "version", "plattform", "bau"]
            .iter()
            .all(|k| m[*k].is_string())
        && merkmale.is_some_and(|l| l.iter().all(Value::is_string))
        && m["dateien"].is_array()
        && objekte.len() == liste.len()
        && ganz(&m["bibliotheken"])
        && objekte.iter().all(|d| {
            d["pfad"].is_string()
                && ganz(&d["groesse"])
                && d["sha256"].is_string()
                && d["art"].is_string()
        });
    if !typ_ok {
        return Err("typ_falsch");
    }
    if m["fassung"].as_i64() != Some(1) {
        return Err("fassung_unbekannt");
    }
    let merkmale: Vec<&str> =
        merkmale.map_or_else(Vec::new, |l| l.iter().filter_map(Value::as_str).collect());
    let bibliotheken = m["bibliotheken"].as_i64().unwrap_or(0);
    if liste.is_empty()
        || liste.len() > 2000
        || merkmale.len() > 16
        || !(1..=100_000).contains(&bibliotheken)
    {
        return Err("anzahl");
    }
    let version = text_feld(m, "version");
    if !version_gut(version) {
        return Err("version_form");
    }
    let plattform = text_feld(m, "plattform");
    let bau = text_feld(m, "bau");
    if text_feld(m, "paket") != "werkbank"
        || !PLATTFORMEN.contains(&plattform)
        || !["auslieferung", "test"].contains(&bau)
        || objekte
            .iter()
            .any(|d| !ARTEN.contains(&text_feld(d, "art")))
    {
        return Err("wert_unbekannt");
    }
    if merkmale.iter().any(|k| !kennung(k)) {
        return Err("kennung_form");
    }
    dateien_pruefen(&objekte, plattform, bau, &merkmale)?;
    if plattform != PLATTFORM {
        // Not a code of the contract: the other platforms have their own tables, this program reads only the Ubuntu manifest.
        return Err("plattform_fremd");
    }
    Ok(Manifest {
        version: version.to_owned(),
        testbau: bau == "test",
        dateien: objekte
            .iter()
            .map(|d| Datei {
                pfad: text_feld(d, "pfad").to_owned(),
                groesse: d["groesse"].as_u64().unwrap_or(0),
                sha256: text_feld(d, "sha256").to_owned(),
                art: text_feld(d, "art").to_owned(),
            })
            .collect(),
    })
}
