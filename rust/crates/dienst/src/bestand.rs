//! The Bestand (contract, DI-L5 points 3 and 4): the list of the sessions of `Starten` in the file `lauf.bestand`, so that the ended
//! sessions of a run are still in the list after a restart of the service.
//!
//! SECURITY: the file is written new after every change of the list and never in place: a new file `<bestand>.neu.<pid>` exclusive
//! with mode 0600, without following a link, `fsync`, rename onto `<bestand>`, `fsync` of the folder. A rest of a crashed run stays.
//! The file is read once at the start; a file the service cannot read is never replaced (exit code 2, fail closed). It holds no
//! line, title, input, clipboard or path of the start file. A start structure of the Bestand passes every check of `Starten` again
//! at `Fortsetzen`; the Bestand gives no content more right than a new request.

use serde_json::{Map, Value, json};

use crate::dienst::Dienst;
use crate::fehler::StartFehler;
use crate::startdatei::Startdatei;
use crate::verwaltung::projekt_von;

/// The largest Bestand in bytes (DI-L5 point 3).
pub(crate) const BESTAND_MAX: u64 = 4_194_304;
/// The most entries of a Bestand that the service reads: the 8 running sessions and the 256 ended ones of the list (P26).
const EINTRAEGE_MAX: usize = 264;
/// The texts of an entry.
const TEXTE: [&str; 7] = [
    "harness", "modell", "route", "kunde", "name", "ordner", "projekt",
];

/// What the start reads from the Bestand: the number of identifiers handed out and the entries in their stored order.
pub(crate) struct Bestand {
    /// `gestartet`, at least the highest number of the identifiers.
    pub(crate) gestartet: usize,
    /// The entries; each is an object with the fields checked here.
    pub(crate) eintraege: Vec<Map<String, Value>>,
}

/// `true` for a content of at most [`BESTAND_MAX`] bytes (DI-L5 point 3).
fn passt(roh: &[u8]) -> bool {
    u64::try_from(roh.len()).is_ok_and(|n| n <= BESTAND_MAX)
}

pub(crate) fn ungueltig() -> StartFehler {
    StartFehler::neu(2, "lauf.bestand ungueltig")
}

/// The number `n` of an identifier `s-<n>` (`^s-[1-9][0-9]{0,8}$`).
fn kennung_nummer(id: &str) -> Option<usize> {
    let ziffern = id.strip_prefix("s-")?;
    let gut = (1..=9).contains(&ziffern.len())
        && !ziffern.starts_with('0')
        && ziffern.bytes().all(|b| b.is_ascii_digit());
    gut.then(|| ziffern.parse().ok()).flatten()
}

/// A whole number of JSON, not a truth value and not a fraction.
fn ganz(wert: Option<&Value>) -> Option<i64> {
    wert.and_then(Value::as_i64)
}

/// The form of the content (DI-L5 point 4). `belegt` names the identifiers that exist already (the session of `kind`).
///
/// # Errors
/// Exit code 2: no object of fassung 1, `gestartet` no whole number from 0, too many entries, or an entry of a wrong form.
pub(crate) fn form(roh: &[u8], belegt: &[&str]) -> Result<Bestand, StartFehler> {
    let wert: Value = std::str::from_utf8(roh)
        .ok()
        .and_then(|t| serde_json::from_str(t).ok())
        .ok_or_else(ungueltig)?;
    let o = wert.as_object().ok_or_else(ungueltig)?;
    let gestartet = ganz(o.get("gestartet"))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(ungueltig)?;
    let liste = o
        .get("sitzungen")
        .and_then(Value::as_array)
        .filter(|l| l.len() <= EINTRAEGE_MAX)
        .filter(|_| ganz(o.get("fassung")) == Some(1))
        .ok_or_else(ungueltig)?;
    let mut hoechste = 0;
    let mut gesehen: Vec<String> = belegt.iter().map(|s| (*s).to_owned()).collect();
    let mut eintraege = Vec::new();
    for x in liste {
        let x = x.as_object().ok_or_else(ungueltig)?;
        let id = x.get("id").and_then(Value::as_str).unwrap_or_default();
        let n = kennung_nummer(id).ok_or_else(ungueltig)?;
        let gut = TEXTE
            .iter()
            .all(|k| x.get(*k).is_some_and(Value::is_string))
            && matches!(
                x.get("zustand").and_then(Value::as_str),
                Some("laeuft" | "beendet")
            )
            && x.get("start").is_some_and(Value::is_object)
            && !gesehen.iter().any(|g| g == id);
        if !gut {
            return Err(ungueltig());
        }
        gesehen.push(id.to_owned());
        hoechste = hoechste.max(n);
        eintraege.push(x.clone());
    }
    Ok(Bestand {
        gestartet: gestartet.max(hoechste),
        eintraege,
    })
}

/// DI-L5 point 4: reads the Bestand once at the start. A missing file gives an empty list (`None`).
///
/// # Errors
/// Exit code 2: the file is a link, no regular file, owned by another user, not mode 0600, larger than [`BESTAND_MAX`], cannot be
/// read or has a wrong form (`bestand_datei.rs`).
pub(crate) fn lesen(cfg: &Startdatei, belegt: &[&str]) -> Result<Option<Bestand>, StartFehler> {
    let Some(pfad) = cfg.lauf.as_ref().and_then(|l| l.bestand.as_deref()) else {
        return Ok(None);
    };
    match crate::bestand_datei::lesen(pfad)? {
        Some(roh) if passt(&roh) => form(&roh, belegt).map(Some),
        Some(_) => Err(ungueltig()),
        None => Ok(None),
    }
}

impl Dienst {
    /// The content of the Bestand (DI-L5 point 3): every session of `Starten` that is still in the list, in its order.
    pub(crate) fn bestand_inhalt(&self) -> Value {
        let sitzungen: Vec<Value> = self
            .sitzungen
            .iter()
            .filter(|s| s.eintrag.is_some())
            .filter(|s| !s.sid.as_ref().is_some_and(|sid| self.entfernt.contains(sid)))
            .filter_map(|s| {
                let sid = s.sid.as_ref()?;
                let start = s.angaben.start.as_ref()?;
                let a = &s.angaben;
                let mut x = json!({"id": sid, "harness": s.meta[0], "modell": s.meta[1], "route": s.meta[2],
                                   "kunde": s.meta[3], "name": a.name, "ordner": a.ordner,
                                   "projekt": projekt_von(&a.ordner),
                                   "zustand": if s.ist_beendet() { "beendet" } else { "laeuft" }, "start": start});
                if let Some(code) = s.ende_code() {
                    x["code"] = json!(code);
                }
                let wahl = [
                    ("grund", s.grund.as_ref()),
                    ("orchestrator", a.orchestrator.as_ref()),
                    ("rolle", a.rolle.as_ref()),
                    ("fortsetzung_von", a.fortsetzung_von.as_ref()),
                    ("fortgesetzt_als", a.fortgesetzt_als.as_ref()),
                ];
                for (k, v) in wahl {
                    if let Some(v) = v {
                        x[k] = json!(v);
                    }
                }
                if s.ist_beendet() {
                    x["abgestuerzt"] = json!(s.abgestuerzt());
                }
                Some(x)
            })
            .collect();
        json!({"fassung": 1, "gestartet": self.naechste, "sitzungen": sitzungen})
    }

    /// Writes the Bestand after a change of the list, if `lauf.bestand` is set. On an error the old file stays and one line without
    /// content goes to stderr.
    pub(crate) fn bestand_schreiben(&self) {
        let Some(pfad) = self.cfg.lauf.as_ref().and_then(|l| l.bestand.as_deref()) else {
            return;
        };
        let roh = crate::starten::kanon(&self.bestand_inhalt());
        let ergebnis = if passt(&roh) {
            crate::bestand_datei::schreiben(pfad, &roh)
        } else {
            Err("groesse".to_owned())
        };
        if let Err(code) = ergebnis {
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr(),
                "wb-dienst: bestand nicht geschrieben: {code}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eintrag(id: &str) -> Value {
        json!({"id": id, "harness": "claude", "modell": "m", "route": "r", "kunde": "", "name": "", "ordner": "/a/b",
               "projekt": "b", "zustand": "beendet", "start": {}})
    }

    fn roh(wert: &Value) -> Vec<u8> {
        serde_json::to_vec(wert).expect("json")
    }

    #[test]
    fn the_form_takes_a_good_bestand_and_counts_the_highest_identifier() {
        let b = form(
            &roh(&json!({"fassung": 1, "gestartet": 2, "sitzungen": [eintrag("s-7"), eintrag("s-3")]})),
            &[],
        )
        .expect("good");
        assert_eq!((b.gestartet, b.eintraege.len()), (7, 2));
        let b = form(
            &roh(&json!({"fassung": 1, "gestartet": 9, "sitzungen": []})),
            &[],
        )
        .expect("good");
        assert_eq!(b.gestartet, 9);
    }

    #[test]
    fn the_form_refuses_every_wrong_part() {
        let rot = |w: &Value| form(&roh(w), &["s-1"]).err().map(|f| f.code);
        let gut = json!({"fassung": 1, "gestartet": 0, "sitzungen": [eintrag("s-2")]});
        assert_eq!(rot(&gut), None);
        for (k, v) in [
            ("fassung", json!(2)),
            ("gestartet", json!(-1)),
            ("gestartet", json!(true)),
            ("gestartet", json!(1.5)),
            ("sitzungen", json!({})),
        ] {
            let mut w = gut.clone();
            w[k] = v;
            assert_eq!(rot(&w), Some(2), "{k}");
        }
        let viele: Vec<Value> = (1..=265)
            .map(|n| eintrag(&format!("s-{}", n + 1)))
            .collect();
        assert_eq!(
            rot(&json!({"fassung": 1, "gestartet": 0, "sitzungen": viele})),
            Some(2)
        );
        let ziel: Vec<Value> = (1..=264)
            .map(|n| eintrag(&format!("s-{}", n + 1)))
            .collect();
        assert_eq!(
            rot(&json!({"fassung": 1, "gestartet": 0, "sitzungen": ziel})),
            None
        );
        for id in ["s-0", "s-01", "s-1234567890", "x-1", "s-", "s-1"] {
            assert_eq!(
                rot(&json!({"fassung": 1, "gestartet": 0, "sitzungen": [eintrag(id)]})),
                Some(2),
                "{id}"
            );
        }
        for (k, v) in [
            ("harness", json!(1)),
            ("projekt", Value::Null),
            ("zustand", json!("offen")),
            ("start", json!([])),
        ] {
            let mut e = eintrag("s-2");
            e[k] = v;
            assert_eq!(
                rot(&json!({"fassung": 1, "gestartet": 0, "sitzungen": [e]})),
                Some(2),
                "{k}"
            );
        }
        let doppelt =
            json!({"fassung": 1, "gestartet": 0, "sitzungen": [eintrag("s-2"), eintrag("s-2")]});
        assert_eq!(rot(&doppelt), Some(2));
        assert_eq!(form(b"{", &[]).err().map(|f| f.code), Some(2));
        assert_eq!(form(&[0xff], &[]).err().map(|f| f.code), Some(2));
        assert_eq!(form(b"[]", &[]).err().map(|f| f.code), Some(2));
    }

    #[test]
    fn the_limit_is_4_mib() {
        let max = usize::try_from(BESTAND_MAX).expect("size");
        assert!(passt(&vec![b' '; max]));
        assert!(!passt(&vec![b' '; max + 1]));
        assert!(passt(b""));
    }

    #[test]
    fn the_number_of_an_identifier() {
        assert_eq!(kennung_nummer("s-1"), Some(1));
        assert_eq!(kennung_nummer("s-999999999"), Some(999_999_999));
        assert_eq!(kennung_nummer("s-1a"), None);
    }
}
