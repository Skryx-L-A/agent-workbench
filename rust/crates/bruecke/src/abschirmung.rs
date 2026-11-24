//! The first step of the program: the memory of the process is shielded from core images (the crate `kernabbild`, decision of 21.11.2026).
//! The crate returns reasons and notes, no text and no exit codes; this file turns them into one line on standard error and the exit
//! code 5. No other path of the program ends with 5 (the codes of the start are 1 to 3).

use std::io::Write;

/// The exit code of a start that cannot shield the memory of the process from core images.
pub const KERNABBILD_CODE: u8 = 5;

/// Turns the result of `kernabbild::abschirmen` into the behaviour of the program: `None` goes on (a note goes to `fehler` when the
/// hard limit for core was 0 already and the core pattern is a file pattern), `Some(5)` ends the program with one line on `fehler`.
pub fn abschirmen(
    ergebnis: Result<kernabbild::Abgeschirmt, kernabbild::Abbruch>,
    fehler: &mut impl Write,
) -> Option<u8> {
    match ergebnis {
        Ok(abgeschirmt) => {
            if let Some(hinweis) = abgeschirmt.hinweis {
                let _ = writeln!(fehler, "wb-bruecke: {}", hinweis.text());
            }
            None
        }
        Err(grund) => {
            let _ = writeln!(
                fehler,
                "wb-bruecke: Kernabbild nicht abschirmbar: {} ({})",
                grund.grund(),
                grund.abhilfe()
            );
            Some(KERNABBILD_CODE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernabbild::{Abbruch, Abgeschirmt, Hinweis};

    fn zeilen(f: impl FnOnce(&mut Vec<u8>) -> Option<u8>) -> (Option<u8>, Vec<String>) {
        let mut aus = Vec::new();
        let code = f(&mut aus);
        let text = String::from_utf8(aus).expect("utf8");
        (code, text.lines().map(str::to_owned).collect())
    }

    #[test]
    fn nichts_zu_melden_geht_weiter() {
        let (code, aus) = zeilen(|f| abschirmen(Ok(Abgeschirmt { hinweis: None }), f));
        assert_eq!(code, None);
        assert!(aus.is_empty(), "{aus:?}");
    }

    #[test]
    fn ein_hinweis_geht_als_eine_zeile_hinaus_und_es_geht_weiter() {
        for hinweis in [Hinweis::MusterDatei, Hinweis::MusterFehlt] {
            let (code, aus) = zeilen(|f| {
                abschirmen(
                    Ok(Abgeschirmt {
                        hinweis: Some(hinweis),
                    }),
                    f,
                )
            });
            assert_eq!(code, None);
            assert_eq!(aus, [format!("wb-bruecke: {}", hinweis.text())]);
        }
    }

    #[test]
    fn jeder_abbruch_endet_mit_5_und_einer_zeile_mit_grund_und_abhilfe() {
        for grund in [
            Abbruch::AbzugSperren,
            Abbruch::GrenzeSetzen,
            Abbruch::MusterPipe,
            Abbruch::MusterNichtLesbar,
        ] {
            let (code, aus) = zeilen(|f| abschirmen(Err(grund), f));
            assert_eq!(code, Some(5), "{grund:?}");
            assert_eq!(
                aus,
                [format!(
                    "wb-bruecke: Kernabbild nicht abschirmbar: {} ({})",
                    grund.grund(),
                    grund.abhilfe()
                )]
            );
        }
    }

    #[test]
    fn der_code_5_gehoert_nur_diesem_schritt() {
        assert_eq!(KERNABBILD_CODE, 5);
        assert!((1..=3).all(|c| c != KERNABBILD_CODE));
    }
}
