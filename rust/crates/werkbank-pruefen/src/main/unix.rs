//! The program `werkbank-pruefen`. It takes no argument (exit 64 otherwise) and runs the seven checks. Two exceptions: `--vor-entfernen`,
//! called by `prerm` on `apt remove`, only reports a key file that stays and never fails; `--programmordner <heim> <ordner>`, called by
//! `postinst`, applies the rule of the enclosure for program folders to one folder (exit 0 allowed, 1 refused).
//!
//! Test seam, only in the test build (feature `pruefbau`): with `WB_TESTUMGEBUNG=1` and `WB_PRUEFEN_LAGE=<file>` the program
//! reads a `Lage` from the file (the `lage` of a case of the contract) instead of the machine. Without `WB_TESTUMGEBUNG=1` the
//! variable is ignored. The delivered build has no seam and never reads either variable.

use std::io::Write;
use std::process::ExitCode;

use werkbank_pruefen::bewerte::{Ergebnis, Strom, aufruf_falsch, bewerte, intern, vor_entfernen};
use werkbank_pruefen::lage::Lage;
use werkbank_pruefen::ohne_doppel;
use werkbank_pruefen::programmordner;
use werkbank_pruefen::sammeln::{Echt, Umgebung, aufrufer, reale_uid};

fn aus_datei(pfad: &str) -> Result<Lage, String> {
    let roh = std::fs::read(pfad).map_err(|e| e.to_string())?;
    let wert = ohne_doppel::lesen(&roh).map_err(|e| e.to_string())?;
    Lage::aus_json(&wert)
}

fn ausgeben(e: &Ergebnis) -> ExitCode {
    let mut text = String::new();
    for z in &e.zeilen {
        text.push_str(z);
        text.push('\n');
    }
    let geschrieben = match e.strom {
        Strom::Stdout => std::io::stdout().write_all(text.as_bytes()),
        Strom::Stderr => std::io::stderr().write_all(text.as_bytes()),
    };
    if geschrieben.is_err() && e.rc == 0 {
        return ExitCode::from(70);
    }
    ExitCode::from(e.rc)
}

/// The state of the key files of all users below `/var/lib/werkbank`: `ok` when one exists, `fehlt` when none does,
/// `nicht_pruefbar` when the folder cannot be read.
fn schluessel_zustand(datei: &str) -> &'static str {
    let Ok(ordner) = std::fs::read_dir("/var/lib/werkbank") else {
        return if std::path::Path::new("/var/lib/werkbank").exists() {
            "nicht_pruefbar"
        } else {
            "fehlt"
        };
    };
    for eintrag in ordner {
        let Ok(eintrag) = eintrag else {
            return "nicht_pruefbar";
        };
        if std::fs::symlink_metadata(eintrag.path().join(datei)).is_ok() {
            return "ok";
        }
    }
    "fehlt"
}

/// `--programmordner <heim> <ordner>`: the rule of the enclosure for one program folder (called by `postinst` before it writes a folder to
/// `lauf.json`). Exit 0: allowed, no output. Exit 1: refused, one fixed line on standard output. Exit 64: another form of the call.
fn programmordner_aufruf(mut args: impl Iterator<Item = std::ffi::OsString>) -> ExitCode {
    let (Some(heim), Some(ordner), None) = (args.next(), args.next(), args.next()) else {
        return ausgeben(&aufruf_falsch());
    };
    let (heim, ordner) = (std::path::Path::new(&heim), std::path::Path::new(&ordner));
    if !heim.is_absolute() || !ordner.is_absolute() {
        return ausgeben(&aufruf_falsch());
    }
    if programmordner::verboten(ordner, heim) {
        return ausgeben(&Ergebnis {
            rc: 1,
            strom: Strom::Stdout,
            zeilen: vec![programmordner::SATZ_VERBOTEN.to_owned()],
        });
    }
    ExitCode::SUCCESS
}

/// Test build (feature `pruefbau`): the file of the seam, only with `WB_TESTUMGEBUNG=1`.
#[cfg(feature = "pruefbau")]
fn lage_naht() -> Option<String> {
    if std::env::var("WB_TESTUMGEBUNG").is_ok_and(|v| v == "1") {
        std::env::var("WB_PRUEFEN_LAGE").ok()
    } else {
        None
    }
}

/// Delivered build: no seam, the environment is not read for it.
#[cfg(not(feature = "pruefbau"))]
fn lage_naht() -> Option<String> {
    None
}

pub fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    if let Some(erstes) = args.next() {
        if erstes == "--programmordner" {
            return programmordner_aufruf(args);
        }
        if erstes != "--vor-entfernen" || args.next().is_some() {
            return ausgeben(&aufruf_falsch());
        }
        return match lage_naht() {
            Some(pfad) => ausgeben(
                &aus_datei(&pfad)
                    .map_or_else(|_| intern(), |l| vor_entfernen(&l.schluessel, &l.copilot)),
            ),
            None => ausgeben(&vor_entfernen(
                schluessel_zustand("modellweg.schluessel"),
                schluessel_zustand("copilot.schluessel"),
            )),
        };
    }
    let ergebnis = if let Some(pfad) = lage_naht() {
        aus_datei(&pfad).map_or_else(|_| intern(), |l| bewerte(&l))
    } else {
        let (nutzer, root) = aufrufer(
            std::env::var("SUDO_USER").ok().as_deref(),
            reale_uid(),
            std::path::Path::new("/"),
        );
        let umgebung = Umgebung {
            wurzel: "/".into(),
            nutzer,
            root,
            system: &Echt,
        };
        bewerte(&umgebung.lage())
    };
    ausgeben(&ergebnis)
}
