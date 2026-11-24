//! The first step of `rangkern serve` and `rangkern sitzung`: the memory of the process is shielded from core images (the crate
//! `kernabbild`, decision of 21.11.2026). The crate returns reasons and notes, no text and no exit codes; this file turns them into one line on
//! the error output and the exit code 5. The other commands (`search`, `tokens`, `snippet`, `--licenses`) hold no secret and do not take
//! the step. No other path of the two commands ends with 5 (their codes are 0, 2 or 64, 3, 74 and 78).
//!
//! The step is taken by `main`, never by `run_with_input` or `sitzung::run`: a test that calls them in its own process must not leave
//! that process with the limit 1/1 and without a dump.

use std::io::Write;

/// The exit code of a start that cannot shield the memory of the process from core images.
pub const EXIT_ABSCHIRMUNG: u8 = 5;

/// The commands that take the step: those that hold the contents of the vault in memory for a long time.
#[must_use]
pub fn betrifft(befehl: &str) -> bool {
    matches!(befehl, "serve" | "sitzung")
}

/// Turns the result of `kernabbild::abschirmen` into the behaviour of the command `befehl`: `None` goes on (a note goes to `fehler`
/// when the hard limit for core was 0 already and the core pattern is a file pattern or missing), `Some(5)` ends the program with one
/// line on `fehler`.
pub fn abbilden(
    befehl: &str,
    ergebnis: Result<kernabbild::Abgeschirmt, kernabbild::Abbruch>,
    fehler: &mut impl Write,
) -> Option<u8> {
    match ergebnis {
        Ok(abgeschirmt) => {
            if let Some(hinweis) = abgeschirmt.hinweis {
                let _ = writeln!(fehler, "rangkern {befehl}: {}", hinweis.text());
            }
            None
        }
        Err(grund) => {
            let _ = writeln!(
                fehler,
                "rangkern {befehl}: Kernabbild nicht abschirmbar: {} ({})",
                grund.grund(),
                grund.abhilfe()
            );
            Some(EXIT_ABSCHIRMUNG)
        }
    }
}

/// The step for this process: `None` goes on, `Some(code)` ends the program. Unix only; under Windows the step does not exist.
#[cfg(unix)]
pub fn schritt(befehl: &str, fehler: &mut impl Write) -> Option<u8> {
    abbilden(befehl, kernabbild::abschirmen(), fehler)
}

/// Under Windows there is no `kernabbild::abschirmen`: the step is missing and the program goes on.
#[cfg(not(unix))]
pub fn schritt(_befehl: &str, _fehler: &mut impl Write) -> Option<u8> {
    None
}
