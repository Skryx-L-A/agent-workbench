//! The protocol of the holder (section 9): one line per refused ticket in its own chain (source `halter`,
//! `halter-<JJJJ-MM>.jsonl`, anchor `halter.anker.jsonl`), written through `formate`. The line carries the own `digest`,
//! `lauf`, `server`, `werkzeug` and a reason of the fixed list; never arguments, a person, an `id` or a value.
//!
//! Stage 18 and the leak lock have no reason in the fixed list (OPEN 2); for them no line is written.

use std::path::Path;

use serde_json::json;

use crate::code::Code;
use crate::pruefung::ScheinAbgelehnt;

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

#[cfg(test)]
mod tests {
    use super::*;

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
