//! Reading and searching the working folder of a session (contract `dienst`, "Nachtrag 26.11.2026: Ordner lesen und durchsuchen (L9)").
//!
//! The events `OrdnerLesen {sitzung, pfad}` and `InhaltSuchen {sitzung, muster}` are the service's own: the core never sees them. The
//! answers are the messages `Ordnerinhalt` and `Suchtreffer`; every refusal is a frame `Ablehnung` with one of the fixed reasons below.
//!
//! SECURITY (fail closed):
//! - `pfad` is relative to the working folder of the session and is checked as text first ([`glieder`]): no absolute path, no member
//!   `..` or `.`, no empty member, no NUL, no backslash, no drive letter. The text check is not the lock: the file system part
//!   (`arbeitsordner_fs.rs`) opens the root once and every member relative to the open folder without following a link.
//! - There is no way in the service to learn the data class of a session or what a viewer may read ([`klasse_lesbar`]): every read
//!   and every search is refused with `klasse_gesperrt` until the contract names a source.
//! - Every limit stands in [`GRENZEN`]. A limit that is reached marks the answer (`abgeschnitten`) or refuses it (`grenze`); nothing
//!   is left out without a mark. An error while reading is a refusal or `abgeschnitten`, never an empty list without a mark.
//! - Nothing of a path, a name, a pattern or a hit goes to the log (stderr).

// Windows refuses before the file system part (`sys/windows.rs`), so the parts only it uses are unused there.
#![cfg_attr(windows, allow(dead_code))]

use std::path::Path;

use serde_json::{Map, Value, json};

/// The event that lists one folder below the working folder.
pub(crate) const ORDNER_LESEN: &str = "OrdnerLesen";
/// The event that searches the content of the working folder.
pub(crate) const INHALT_SUCHEN: &str = "InhaltSuchen";

/// The event has another form (an unknown key, a key missing, a value of the wrong type).
pub(crate) const FORM_UNGUELTIG: &str = "form_ungueltig";
/// The session is not known (the room core has none).
pub(crate) const SITZUNG_UNBEKANNT: &str = "sitzung_unbekannt";
/// The session is in another customer room than the viewer (the reason the core gives for `Abo`).
pub(crate) const FREMDER_KUNDENRAUM: &str = "fremder_kundenraum";
/// `pfad` breaks a rule of the text (absolute, `..`, `.`, empty member, NUL, backslash, drive letter).
pub(crate) const PFAD_UNGUELTIG: &str = "pfad_ungueltig";
/// `muster` is empty or holds NUL, a line feed or a carriage return.
pub(crate) const MUSTER_UNGUELTIG: &str = "muster_ungueltig";
/// A limit of the request itself: `pfad` or `muster` too long, too many members.
pub(crate) const GRENZE: &str = "grenze";
/// The data class of the session is not readable for the viewer, or not known to be.
pub(crate) const KLASSE_GESPERRT: &str = "klasse_gesperrt";
/// A member of the path is a symbolic link (or, under Windows, a reparse point).
pub(crate) const PFAD_AUSSERHALB: &str = "pfad_ausserhalb";
/// The folder cannot be opened or listed: it is missing, it is no folder, or there are no rights.
pub(crate) const NICHT_LESBAR: &str = "nicht_lesbar";
/// The system has no safe way to open the members relative to a held folder (Windows, for now).
#[cfg_attr(unix, allow(dead_code))]
pub(crate) const PLATTFORM_GESPERRT: &str = "plattform_gesperrt";

/// The limits of reading and searching, all in one place (the values stand in the contract).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Grenzen {
    /// The most entries of one folder that are read (listing and search).
    pub(crate) eintraege: usize,
    /// The most hits of one search.
    pub(crate) treffer: usize,
    /// The deepest folder a search enters (the working folder is depth 0).
    pub(crate) tiefe: usize,
    /// The most files a search opens.
    pub(crate) dateien: usize,
    /// The most bytes read of one file.
    pub(crate) datei_bytes: usize,
    /// The most bytes read in one search.
    pub(crate) gesamt_bytes: usize,
    /// The longest a search runs, in milliseconds of the clock of the service.
    pub(crate) laufzeit_ms: u64,
    /// The longest excerpt of a hit in bytes.
    pub(crate) auszug_bytes: usize,
    /// The most bytes the hits of one search may take in the answer, counted as [`treffer_bytes`] (every byte escaped, the worst
    /// case), so that the frame stays far below `RAHMEN_MAX`.
    pub(crate) antwort_bytes: usize,
    /// The longest pattern in bytes.
    pub(crate) muster_bytes: usize,
    /// The longest `pfad` in bytes.
    pub(crate) pfad_bytes: usize,
    /// The most members of `pfad`.
    pub(crate) glieder: usize,
}

/// The limits of the service.
pub(crate) const GRENZEN: Grenzen = Grenzen {
    eintraege: 1000,
    treffer: 200,
    tiefe: 16,
    dateien: 5000,
    datei_bytes: 1_048_576,
    gesamt_bytes: 33_554_432,
    laufzeit_ms: 500,
    auszug_bytes: 200,
    antwort_bytes: 8_388_608,
    muster_bytes: 256,
    pfad_bytes: 4096,
    glieder: 64,
};

/// The kind of an entry, read without following a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Art {
    /// A regular file.
    Datei,
    /// A folder.
    Ordner,
    /// Anything else: a link, a FIFO, a socket, a device.
    Anderes,
}

impl Art {
    fn wort(self) -> &'static str {
        match self {
            Self::Datei => "datei",
            Self::Ordner => "ordner",
            Self::Anderes => "anderes",
        }
    }
}

/// One entry of a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Eintrag {
    pub(crate) name: String,
    pub(crate) art: Art,
    /// The size in bytes of a regular file; `None` for every other kind.
    pub(crate) groesse: Option<u64>,
}

/// The content of one folder.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Inhalt {
    pub(crate) eintraege: Vec<Eintrag>,
    pub(crate) abgeschnitten: bool,
}

/// One hit: the path below the working folder, the line (from 1) and an excerpt of the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Treffer {
    pub(crate) pfad: String,
    pub(crate) zeile: u64,
    pub(crate) auszug: String,
}

/// The result of one search.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Suche {
    pub(crate) treffer: Vec<Treffer>,
    pub(crate) abgeschnitten: bool,
}

/// What an event asks for, after the check of its form.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Anfrage<'a> {
    /// `OrdnerLesen`: the session and the path as sent.
    Lesen(&'a str, &'a str),
    /// `InhaltSuchen`: the session and the pattern as sent.
    Suchen(&'a str, &'a str),
}

impl<'a> Anfrage<'a> {
    /// The session the event names.
    pub(crate) fn sitzung(&self) -> &'a str {
        match self {
            Self::Lesen(s, _) | Self::Suchen(s, _) => s,
        }
    }
}

/// Whether `art` is one of the two events of this module.
pub(crate) fn ist_eigen(art: &str) -> bool {
    art == ORDNER_LESEN || art == INHALT_SUCHEN
}

/// The form of the event: exactly `art`, `sitzung` and `pfad` (or `muster`), all texts. Anything else is [`FORM_UNGUELTIG`].
pub(crate) fn form(ereignis: &Map<String, Value>) -> Result<Anfrage<'_>, &'static str> {
    let art = ereignis.get("art").and_then(Value::as_str);
    let feld = match art {
        Some(ORDNER_LESEN) => "pfad",
        Some(INHALT_SUCHEN) => "muster",
        _ => return Err(FORM_UNGUELTIG),
    };
    if ereignis.len() != 3 {
        return Err(FORM_UNGUELTIG);
    }
    let (Some(sitzung), Some(wert)) = (
        ereignis.get("sitzung").and_then(Value::as_str),
        ereignis.get(feld).and_then(Value::as_str),
    ) else {
        return Err(FORM_UNGUELTIG);
    };
    Ok(if feld == "pfad" {
        Anfrage::Lesen(sitzung, wert)
    } else {
        Anfrage::Suchen(sitzung, wert)
    })
}

/// The members of `pfad`; the empty text is the working folder itself (no member). The rules of the text are checked here; the
/// file system part never follows a link, so a member that passes here can still be refused there.
pub(crate) fn glieder<'a>(pfad: &'a str, g: &Grenzen) -> Result<Vec<&'a str>, &'static str> {
    if pfad.len() > g.pfad_bytes {
        return Err(GRENZE);
    }
    if pfad.is_empty() {
        return Ok(Vec::new());
    }
    if pfad.starts_with('/') || pfad.contains(['\0', '\\']) {
        return Err(PFAD_UNGUELTIG);
    }
    let glieder: Vec<&str> = pfad.split('/').collect();
    if glieder.iter().any(|x| !glied_gut(x)) {
        return Err(PFAD_UNGUELTIG);
    }
    if glieder.len() > g.glieder {
        return Err(GRENZE);
    }
    Ok(glieder)
}

/// One member: not empty, not `.` or `..`, no drive letter at its start (`C:`), as Windows would read it.
fn glied_gut(glied: &str) -> bool {
    let b = glied.as_bytes();
    let laufwerk = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    !glied.is_empty() && glied != "." && glied != ".." && !laufwerk
}

/// The pattern of a search: not empty, at most [`Grenzen::muster_bytes`], without NUL and without a line break (the search goes by
/// line, a line break never matches).
pub(crate) fn muster_pruefen(muster: &str, g: &Grenzen) -> Result<(), &'static str> {
    if muster.len() > g.muster_bytes {
        return Err(GRENZE);
    }
    if muster.is_empty() || muster.contains(['\0', '\n', '\r']) {
        return Err(MUSTER_UNGUELTIG);
    }
    Ok(())
}

/// SECURITY: whether the viewer may read the data class of the session. The service has no source for either: the start structure
/// names the route and the customer, but the service neither knows the class of what lies in the working folder nor the read class
/// of the viewer (contract `datentor person`, which no crate builds yet, and the settings carry no read classes). So the answer is
/// always no; it never assumes "readable". A source comes with a decision and a case of the contract.
pub(crate) const fn klasse_lesbar() -> bool {
    false
}

/// The answer to a request whose session is known and in the room of the viewer: the checks of the text, the class, then the file
/// system below `wurzel` (`None`: the session has no working folder). `uhr` is the clock of the service in milliseconds.
pub(crate) fn beantworten(
    anfrage: &Anfrage<'_>,
    wurzel: Option<&Path>,
    g: &Grenzen,
    uhr: &mut dyn FnMut() -> u64,
) -> Result<Value, &'static str> {
    match anfrage {
        Anfrage::Lesen(sitzung, pfad) => {
            let glieder = glieder(pfad, g)?;
            if !klasse_lesbar() {
                return Err(KLASSE_GESPERRT);
            }
            let wurzel = wurzel.ok_or(NICHT_LESBAR)?;
            let inhalt = crate::sys::ordner_lesen(wurzel, &glieder, g)?;
            Ok(ordnerinhalt(sitzung, pfad, &inhalt))
        }
        Anfrage::Suchen(sitzung, muster) => {
            muster_pruefen(muster, g)?;
            if !klasse_lesbar() {
                return Err(KLASSE_GESPERRT);
            }
            let wurzel = wurzel.ok_or(NICHT_LESBAR)?;
            let suche = crate::sys::inhalt_suchen(wurzel, muster, g, uhr)?;
            Ok(suchtreffer(sitzung, muster, &suche))
        }
    }
}

/// The message `Ordnerinhalt` (without `t`, which the service sets).
pub(crate) fn ordnerinhalt(sitzung: &str, pfad: &str, inhalt: &Inhalt) -> Value {
    let eintraege: Vec<Value> = inhalt
        .eintraege
        .iter()
        .map(|e| json!({"name": e.name, "art": e.art.wort(), "groesse": e.groesse}))
        .collect();
    json!({"art": "Ordnerinhalt", "sitzung": sitzung, "pfad": pfad, "eintraege": eintraege,
           "abgeschnitten": inhalt.abgeschnitten})
}

/// The message `Suchtreffer` (without `t`, which the service sets).
pub(crate) fn suchtreffer(sitzung: &str, muster: &str, suche: &Suche) -> Value {
    let treffer: Vec<Value> = suche
        .treffer
        .iter()
        .map(|t| json!({"pfad": t.pfad, "zeile": t.zeile, "auszug": t.auszug}))
        .collect();
    json!({"art": "Suchtreffer", "sitzung": sitzung, "muster": muster, "treffer": treffer,
           "abgeschnitten": suche.abgeschnitten})
}

/// What one hit may take in the answer at most: path and excerpt with every byte escaped (`\u00xx`, six bytes) and 64 bytes for
/// the keys, the line and the punctuation.
pub(crate) fn treffer_bytes(pfad: &str, auszug: &str) -> usize {
    (pfad.len() + auszug.len()) * 6 + 64
}

/// An excerpt of a line: the bytes as UTF-8 (an invalid sequence becomes U+FFFD), without a carriage return at the end, cut to at
/// most `max` bytes at the end of a character.
pub(crate) fn auszug(zeile: &[u8], max: usize) -> String {
    let zeile = zeile.strip_suffix(b"\r").unwrap_or(zeile);
    let text = String::from_utf8_lossy(zeile);
    let mut ende = text.len().min(max);
    while !text.is_char_boundary(ende) {
        ende -= 1;
    }
    text[..ende].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(v: &Value) -> &Map<String, Value> {
        v.as_object().expect("object")
    }

    #[test]
    fn die_form_ist_geschlossen() {
        let gut = json!({"art": "OrdnerLesen", "sitzung": "s-1", "pfad": "a/b"});
        assert_eq!(form(obj(&gut)), Ok(Anfrage::Lesen("s-1", "a/b")));
        let suche = json!({"art": "InhaltSuchen", "sitzung": "s-1", "muster": "x"});
        assert_eq!(form(obj(&suche)), Ok(Anfrage::Suchen("s-1", "x")));
        assert_eq!(form(obj(&suche)).map(|a| a.sitzung()), Ok("s-1"));
        assert_eq!(form(obj(&gut)).map(|a| a.sitzung()), Ok("s-1"));
        for v in [
            json!({"art": "OrdnerLesen", "sitzung": "s-1"}),
            json!({"art": "OrdnerLesen", "sitzung": "s-1", "pfad": "a", "x": 1}),
            json!({"art": "OrdnerLesen", "sitzung": "s-1", "muster": "a"}),
            json!({"art": "OrdnerLesen", "sitzung": 1, "pfad": "a"}),
            json!({"art": "OrdnerLesen", "sitzung": "s-1", "pfad": null}),
            json!({"art": "InhaltSuchen", "sitzung": "s-1", "pfad": "a"}),
            json!({"art": "InhaltSuchen", "sitzung": "s-1", "muster": ["a"]}),
            json!({"art": "Abo", "sitzung": "s-1", "pfad": "a"}),
        ] {
            assert_eq!(form(obj(&v)), Err(FORM_UNGUELTIG), "{v}");
        }
        assert!(ist_eigen("OrdnerLesen") && ist_eigen("InhaltSuchen"));
        assert!(!ist_eigen("Abo") && !ist_eigen("Ordnerinhalt"));
    }

    #[test]
    fn der_pfadtext_folgt_den_regeln() {
        let g = GRENZEN;
        assert_eq!(glieder("", &g), Ok(vec![]));
        assert_eq!(glieder("a", &g), Ok(vec!["a"]));
        assert_eq!(glieder("a/b c/d.txt", &g), Ok(vec!["a", "b c", "d.txt"]));
        assert_eq!(glieder("..a/a..", &g), Ok(vec!["..a", "a.."]));
        assert_eq!(glieder("ab:/1:/:a", &g), Ok(vec!["ab:", "1:", ":a"]));
        for p in [
            "/",
            "/etc",
            "..",
            "../x",
            "a/..",
            "a/../b",
            ".",
            "./a",
            "a/.",
            "a//b",
            "a/",
            "/a",
            "a\0b",
            "a\\b",
            "\\\\srv\\x",
            "C:",
            "c:/x",
            "a/C:x",
            "Z:\\",
            "a:b",
        ] {
            assert_eq!(glieder(p, &g), Err(PFAD_UNGUELTIG), "{p:?}");
        }
    }

    #[test]
    fn pfadgrenzen() {
        let g = Grenzen {
            pfad_bytes: 7,
            glieder: 3,
            ..GRENZEN
        };
        assert_eq!(glieder("a/b/c", &g), Ok(vec!["a", "b", "c"]));
        assert_eq!(glieder("a/b/c/d", &g), Err(GRENZE));
        assert_eq!(glieder("abcdefg", &g), Ok(vec!["abcdefg"]));
        assert_eq!(glieder("abcdefgh", &g), Err(GRENZE));
        assert_eq!(glieder("/bcdefgh", &g), Err(GRENZE));
    }

    #[test]
    fn das_muster_folgt_den_regeln() {
        let g = Grenzen {
            muster_bytes: 3,
            ..GRENZEN
        };
        assert_eq!(muster_pruefen("abc", &g), Ok(()));
        assert_eq!(muster_pruefen("a", &g), Ok(()));
        assert_eq!(muster_pruefen("abcd", &g), Err(GRENZE));
        for m in ["", "a\0", "a\n", "\r"] {
            assert_eq!(muster_pruefen(m, &g), Err(MUSTER_UNGUELTIG), "{m:?}");
        }
    }

    #[test]
    fn die_klasse_ist_nie_lesbar_und_sperrt_vor_dem_dateisystem() {
        assert!(!klasse_lesbar());
        let mut uhr = || 0;
        let g = GRENZEN;
        // the root does not exist: the class refuses first, the file system is never asked
        let nirgends = Path::new("/nirgends/da");
        let lesen = Anfrage::Lesen("s-1", "a");
        assert_eq!(
            beantworten(&lesen, Some(nirgends), &g, &mut uhr),
            Err(KLASSE_GESPERRT)
        );
        let suchen = Anfrage::Suchen("s-1", "x");
        assert_eq!(
            beantworten(&suchen, Some(nirgends), &g, &mut uhr),
            Err(KLASSE_GESPERRT)
        );
        assert_eq!(
            beantworten(&suchen, None, &g, &mut uhr),
            Err(KLASSE_GESPERRT)
        );
        // the text is checked before the class
        let schlecht = Anfrage::Lesen("s-1", "../x");
        assert_eq!(
            beantworten(&schlecht, Some(nirgends), &g, &mut uhr),
            Err(PFAD_UNGUELTIG)
        );
        let leer = Anfrage::Suchen("s-1", "");
        assert_eq!(
            beantworten(&leer, Some(nirgends), &g, &mut uhr),
            Err(MUSTER_UNGUELTIG)
        );
    }

    #[test]
    fn die_antworten_haben_die_felder_des_vertrags() {
        let inhalt = Inhalt {
            eintraege: vec![
                Eintrag {
                    name: "a.txt".into(),
                    art: Art::Datei,
                    groesse: Some(3),
                },
                Eintrag {
                    name: "b".into(),
                    art: Art::Ordner,
                    groesse: None,
                },
                Eintrag {
                    name: "c".into(),
                    art: Art::Anderes,
                    groesse: None,
                },
            ],
            abgeschnitten: true,
        };
        assert_eq!(
            ordnerinhalt("s-1", "x", &inhalt),
            json!({"art": "Ordnerinhalt", "sitzung": "s-1", "pfad": "x", "abgeschnitten": true, "eintraege": [
                {"name": "a.txt", "art": "datei", "groesse": 3},
                {"name": "b", "art": "ordner", "groesse": null},
                {"name": "c", "art": "anderes", "groesse": null}]})
        );
        let suche = Suche {
            treffer: vec![Treffer {
                pfad: "a/b".into(),
                zeile: 7,
                auszug: "zeile".into(),
            }],
            abgeschnitten: false,
        };
        assert_eq!(
            suchtreffer("s-1", "ei", &suche),
            json!({"art": "Suchtreffer", "sitzung": "s-1", "muster": "ei", "abgeschnitten": false,
                   "treffer": [{"pfad": "a/b", "zeile": 7, "auszug": "zeile"}]})
        );
    }

    #[test]
    fn ein_treffer_zaehlt_jedes_byte_sechsfach() {
        assert_eq!(treffer_bytes("", ""), 64);
        assert_eq!(treffer_bytes("ab", "c"), 82);
        // the worst case: a control character, escaped as six bytes
        let t =
            json!({"pfad": "\u{1}", "zeile": 18_446_744_073_709_551_615_u64, "auszug": "\u{1}"});
        // the JSON of the hit and the comma in front of it in the list
        let laenge = serde_json::to_vec(&t).expect("json").len() + 1;
        assert!(laenge <= treffer_bytes("\u{1}", "\u{1}"), "{laenge}");
    }

    #[test]
    fn der_auszug_endet_an_einem_zeichen() {
        assert_eq!(auszug(b"abc\r", 10), "abc");
        assert_eq!(auszug(b"abc", 2), "ab");
        assert_eq!(auszug(b"abc", 3), "abc");
        assert_eq!(auszug("aä".as_bytes(), 2), "a");
        assert_eq!(auszug("aä".as_bytes(), 3), "aä");
        assert_eq!(auszug(b"a\xffb", 10), "a\u{fffd}b");
        assert_eq!(auszug(b"", 10), "");
    }
}
