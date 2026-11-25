//! The protocol of the holder (section 9): one line per refused ticket in its own chain (source `halter`,
//! `halter-<JJJJ-MM>.jsonl`, anchor `halter.anker.jsonl`), written through `formate`. The line carries the own `digest`,
//! `lauf`, `server`, `werkzeug` and a reason of the fixed list; never arguments, a person, an `id` or a value.
//!
//! A hit of the leak lock writes the fixed line `halter.leck_gesperrt` to the journal (stderr of the unit) and the chain
//! line `halter.leck_gesperrt` (reason `leck`) with the op and the target only. Stage 18 still has no reason in the
//! fixed list (OPEN 2); for it no chain line is written.

use std::io::Write;
use std::path::Path;

use serde_json::json;

use crate::code::Code;
use crate::pruefung::{PfadAbgelehnt, ScheinAbgelehnt};

/// The fixed line of the journal when the chain cannot be continued (finding F16): no path, no content.
pub const ANKER_ZEILE: &str = "halter.kette_anker: Anker der Kette beschaedigt";
/// See [`ANKER_ZEILE`]: any other failure of the chain.
pub const KETTE_ZEILE: &str = "halter.kette: Kette nicht fortgeschrieben";

/// Appends the line `mcp.schein_abgelehnt` for `code`, if the fixed list has a reason for it. `Ok(true)` if a line was
/// written, `Ok(false)` if the list has no reason.
///
/// # Errors
/// The fixed line for the journal ([`ANKER_ZEILE`] or [`KETTE_ZEILE`]) when the chain cannot be continued.
pub fn schein_abgelehnt(
    ordner: &Path,
    zeit_ms: i64,
    code: Code,
    s: &ScheinAbgelehnt,
) -> Result<bool, &'static str> {
    let Some(grund) = code.kettengrund() else {
        return Ok(false);
    };
    let zeile = json!({
        "aktionsname": "mcp.schein_abgelehnt",
        "digest": s.digest,
        "ergebnis": "abgelehnt",
        "grund": grund,
        "lauf": s.lauf,
        "server": s.server,
        "werkzeug": s.werkzeug,
    });
    match formate::kette::anhaengen_zeile(ordner, "halter", zeit_ms, &zeile) {
        Ok(_) => Ok(true),
        Err(formate::kette::Fehler::AnkerFalsch) => Err(ANKER_ZEILE),
        Err(_) => Err(KETTE_ZEILE),
    }
}

/// Appends the line `halter.leck_gesperrt` of a hit of the leak lock (section 9): the fields `aktionsname`, `ergebnis`
/// (`abgelehnt`), `grund` (`leck`), `op` and `ziel` (the name of the target or server in `halter.json`); `fassung`,
/// `vorher` and `zeit` come from `formate`. Never a value, a path, a head, a body, an `id`, a `lauf` or a part of the hit.
///
/// # Errors
/// The fixed line for the journal ([`ANKER_ZEILE`] or [`KETTE_ZEILE`]) when the chain cannot be continued.
pub fn leck_zeile(ordner: &Path, zeit_ms: i64, op: &str, ziel: &str) -> Result<(), &'static str> {
    let zeile = json!({
        "aktionsname": "halter.leck_gesperrt",
        "ergebnis": "abgelehnt",
        "grund": Code::LeckGesperrt.kettengrund(),
        "op": op,
        "ziel": ziel,
    });
    match formate::kette::anhaengen_zeile(ordner, "halter", zeit_ms, &zeile) {
        Ok(_) => Ok(()),
        Err(formate::kette::Fehler::AnkerFalsch) => Err(ANKER_ZEILE),
        Err(_) => Err(KETTE_ZEILE),
    }
}

/// The line `halter.pfad_verboten` of a path refused in stage 15 (section 9): the fields `aktionsname`, `ergebnis`
/// (`abgelehnt`), `grund` (`pfad`), `methode`, `op`, `pfad` and `ziel`; `fassung`, `vorher` and `zeit` come from
/// `formate`. Never the query, a head, a body, an `id` or a value.
#[must_use]
pub fn pfad_wert(p: &PfadAbgelehnt) -> serde_json::Value {
    json!({
        "aktionsname": "halter.pfad_verboten",
        "ergebnis": "abgelehnt",
        "grund": "pfad",
        "methode": p.methode,
        "op": p.op,
        "pfad": p.pfad,
        "ziel": p.ziel,
    })
}

/// Appends [`pfad_wert`] to the chain (the kind is in the register of contract 5, `REGISTER_FREMD`).
///
/// # Errors
/// The fixed line for the journal ([`ANKER_ZEILE`] or [`KETTE_ZEILE`]) when the chain cannot be continued.
pub fn pfad_zeile(ordner: &Path, zeit_ms: i64, p: &PfadAbgelehnt) -> Result<(), &'static str> {
    match formate::kette::anhaengen_zeile(ordner, "halter", zeit_ms, &pfad_wert(p)) {
        Ok(_) => Ok(()),
        Err(formate::kette::Fehler::AnkerFalsch) => Err(ANKER_ZEILE),
        Err(_) => Err(KETTE_ZEILE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N1: the chain line of a refused path carries the op, the target, the method and the path, and nothing else.
    #[test]
    fn die_zeile_eines_abgelehnten_pfads_traegt_methode_und_pfad() {
        let p = PfadAbgelehnt {
            op: "modell.aufruf".to_owned(),
            ziel: "copilot".to_owned(),
            methode: "POST".to_owned(),
            pfad: "/v1/fremd".to_owned(),
        };
        assert_eq!(
            pfad_wert(&p),
            json!({"aktionsname": "halter.pfad_verboten", "ergebnis": "abgelehnt", "grund": "pfad",
                "methode": "POST", "op": "modell.aufruf", "pfad": "/v1/fremd", "ziel": "copilot"})
        );
        let weg = std::env::temp_dir().join(format!("halter-pfad-fehlt-{}", std::process::id()));
        assert_eq!(pfad_zeile(&weg, 1_795_000_000_000, &p), Err(KETTE_ZEILE));
        // the register holds the kind: the line goes into the chain, and the chain stays whole
        let o = ordner("pfad");
        assert_eq!(pfad_zeile(&o, 1_795_000_000_000, &p), Ok(()));
        let geprueft = formate::kette::pruefe_quelle(&o, "halter", formate::kette::NULL_HASH, None)
            .expect("source");
        assert!(geprueft.ok(), "{:?}", geprueft.bruch);
        assert_eq!(geprueft.zeilen, 1);
        let anker = ordner("pfad-anker");
        std::fs::write(anker.join("halter.anker.jsonl"), b"kaputt\n").expect("anchor");
        assert_eq!(pfad_zeile(&anker, 1_795_000_000_000, &p), Err(ANKER_ZEILE));
        for d in [o, anker] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    fn schein() -> ScheinAbgelehnt {
        ScheinAbgelehnt {
            digest: "ab".repeat(32),
            lauf: "l-beispiel".to_owned(),
            server: "tickets".to_owned(),
            werkzeug: "update".to_owned(),
        }
    }

    fn ordner(name: &str) -> std::path::PathBuf {
        let o = std::env::temp_dir().join(format!("halter-kette-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::create_dir_all(&o).expect("folder");
        o
    }

    /// F16: a broken anchor and a folder that is not there give their fixed lines; a working chain takes the line, and
    /// a code without a reason of the fixed list writes nothing.
    #[test]
    fn feste_zeile_bei_kaputter_kette() {
        let gut = ordner("gut");
        assert_eq!(
            schein_abgelehnt(&gut, 1_795_000_000_000, Code::ScheinBenutzt, &schein()),
            Ok(true)
        );
        assert_eq!(
            schein_abgelehnt(&gut, 1_795_000_000_000, Code::ScheinFehlt, &schein()),
            Ok(false)
        );
        let anker = ordner("anker");
        std::fs::write(anker.join("halter.anker.jsonl"), b"kaputt\n").expect("anchor");
        let r = schein_abgelehnt(&anker, 1_795_000_000_000, Code::ScheinBenutzt, &schein());
        assert_eq!(r, Err(ANKER_ZEILE));
        let weg = gut.join("fehlt");
        assert_eq!(
            schein_abgelehnt(&weg, 1_795_000_000_000, Code::ScheinBenutzt, &schein()),
            Err(KETTE_ZEILE)
        );
        for o in [gut, anker] {
            let _ = std::fs::remove_dir_all(o);
        }
    }
}

/// The line of the journal for a hit of the leak lock: `<code>: <grund>` and LF, fixed text without any value.
#[must_use]
pub fn leck_journalzeile() -> String {
    let (code, grund) = Code::LeckGesperrt.paar();
    format!("{code}: {grund}\n")
}

/// A hit of the leak lock (section 9): writes [`leck_journalzeile`] to `journal`, never a value, a path, a head, a body,
/// an `id` or an argument.
pub fn leck_gesperrt(journal: &mut dyn Write) {
    let _ = journal.write_all(leck_journalzeile().as_bytes());
}

#[cfg(test)]
mod leck_tests {
    use super::*;

    /// F4: a hit writes one chain line `halter.leck_gesperrt` with exactly the fields of the contract (section 9), and
    /// the check of `formate` takes the chain as whole (the kind is in its register). Counter check: a kind with a typo
    /// written the same way breaks the chain, so the check really reads the kind.
    #[test]
    fn leck_schreibt_die_kettenzeile_mit_den_feldern_des_vertrags() {
        let o = std::env::temp_dir().join(format!("halter-leck-kette-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::create_dir_all(&o).expect("folder");
        assert_eq!(
            leck_zeile(&o, 1_795_000_000_000, "modell.aufruf", "copilot"),
            Ok(())
        );
        assert_eq!(
            leck_zeile(&o, 1_795_000_000_001, "mcp.aufruf", "tickets"),
            Ok(())
        );
        let datei = o.join("halter-2026-11.jsonl");
        let text = std::fs::read_to_string(&datei).expect("chain file");
        let zeilen: Vec<serde_json::Value> = text
            .lines()
            .map(|z| serde_json::from_str(z).expect("line"))
            .collect();
        assert_eq!(zeilen.len(), 2);
        let mut felder: Vec<&str> = zeilen[0]
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        felder.sort_unstable();
        assert_eq!(
            felder,
            [
                "aktionsname",
                "ergebnis",
                "fassung",
                "grund",
                "op",
                "vorher",
                "zeit",
                "ziel"
            ]
        );
        assert_eq!(zeilen[0]["aktionsname"], "halter.leck_gesperrt");
        assert_eq!(zeilen[0]["ergebnis"], "abgelehnt");
        assert_eq!(zeilen[0]["fassung"], 2);
        assert_eq!(zeilen[0]["grund"], "leck");
        assert_eq!(zeilen[0]["op"], "modell.aufruf");
        assert_eq!(zeilen[0]["ziel"], "copilot");
        assert_eq!(zeilen[0]["vorher"], formate::kette::NULL_HASH);
        assert_eq!(zeilen[1]["op"], "mcp.aufruf");
        assert_eq!(zeilen[1]["ziel"], "tickets");
        let p = formate::kette::pruefe_quelle(&o, "halter", formate::kette::NULL_HASH, None)
            .expect("source");
        assert!(p.ok(), "{:?}", p.bruch);
        assert_eq!(p.zeilen, 2);
        // counter check: the same line with another kind is refused by the register of version 2
        let falsch = o.join("falsch");
        std::fs::create_dir_all(&falsch).expect("folder");
        let z = json!({"aktionsname": "halter.leck_gesperr", "ergebnis": "abgelehnt", "grund": "leck",
            "op": "modell.aufruf", "ziel": "copilot"});
        let geschrieben = formate::kette::anhaengen_zeile(&falsch, "halter", 1_795_000_000_000, &z);
        let p = formate::kette::pruefe_quelle(&falsch, "halter", formate::kette::NULL_HASH, None)
            .expect("source");
        assert!(
            geschrieben.is_err() || !p.ok(),
            "a kind outside the register passed"
        );
        // a broken anchor gives the fixed line, no path
        let anker = o.join("anker");
        std::fs::create_dir_all(&anker).expect("folder");
        std::fs::write(anker.join("halter.anker.jsonl"), b"kaputt\n").expect("anchor");
        assert_eq!(
            leck_zeile(&anker, 1_795_000_000_000, "modell.aufruf", "copilot"),
            Err(ANKER_ZEILE)
        );
        assert_eq!(
            leck_zeile(
                &o.join("fehlt"),
                1_795_000_000_000,
                "modell.aufruf",
                "copilot"
            ),
            Err(KETTE_ZEILE)
        );
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn leck_schreibt_genau_die_feste_journalzeile() {
        let mut journal = Vec::new();
        leck_gesperrt(&mut journal);
        assert_eq!(
            journal,
            b"halter.leck_gesperrt: Antwort enthielt einen Schluessel\n"
        );
        leck_gesperrt(&mut journal);
        assert_eq!(
            journal.len(),
            2 * leck_journalzeile().len(),
            "one line per hit"
        );
    }
}
