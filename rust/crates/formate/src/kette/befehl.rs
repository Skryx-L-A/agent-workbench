//! The commands `kette pruefe [--vorher <64 hex>] [--letzte <64 hex>] <datei>...` and
//! `kette pruefe-quelle [--vorher <64 hex>] [--letzte <64 hex>] <ordner> <quelle>` as a function: arguments in,
//! exit code and output out, so that a test needs no process.

use std::ffi::OsString;
use std::path::PathBuf;

use super::quelle::pruefe_quelle_geprueft;
use super::{NULL_HASH, ist_sha256, pruefe, quelle_ok};

/// Exit code, standard output and standard error of one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ausgang {
    /// 0 the chain is whole; 1 a break; 64 a wrong call.
    pub exit: u8,
    /// One line in RFC 8785 and a line feed; empty with exit 64.
    pub stdout: String,
    /// A message for a person; empty unless the call was wrong.
    pub stderr: String,
}

/// What a call asks for.
enum Ziel {
    /// The files, in the order given, as one chain.
    Dateien(Vec<PathBuf>),
    /// The folder of one source.
    Quelle(PathBuf, String),
}

struct Aufruf {
    vorher: String,
    letzte: Option<String>,
    ziel: Ziel,
}

const HILFE: &str = "Aufruf: kette pruefe [--vorher <64 hex>] [--letzte <64 hex>] <datei>...\n       kette pruefe-quelle [--vorher <64 hex>] [--letzte <64 hex>] <ordner> <quelle>";

/// The value of a switch: 64 lower-case hex digits.
fn schalterwert(wert: Option<&OsString>, schalter: &str) -> Result<String, String> {
    let text = wert
        .ok_or_else(|| format!("{schalter} braucht einen Wert"))?
        .to_str()
        .filter(|t| ist_sha256(t))
        .ok_or_else(|| format!("{schalter} braucht 64 Hexziffern in Kleinbuchstaben"))?;
    Ok(text.to_owned())
}

fn aufruf(args: &[OsString]) -> Result<Aufruf, String> {
    let (unterbefehl, rest) = args
        .split_first()
        .ok_or_else(|| "kein Unterbefehl".to_owned())?;
    let nur_quelle = unterbefehl == "pruefe-quelle";
    if unterbefehl != "pruefe" && !nur_quelle {
        return Err("unbekannter Unterbefehl".to_owned());
    }
    let mut vorher: Option<String> = None;
    let mut letzte: Option<String> = None;
    let mut angaben = Vec::new();
    let mut folgende = rest.iter();
    while let Some(arg) = folgende.next() {
        match arg.to_str() {
            Some(schalter @ ("--vorher" | "--letzte")) => {
                let ziel = if schalter == "--vorher" {
                    &mut vorher
                } else {
                    &mut letzte
                };
                if ziel.is_some() {
                    return Err(format!("{schalter} steht mehrfach"));
                }
                *ziel = Some(schalterwert(folgende.next(), schalter)?);
            }
            Some(schalter) if schalter.starts_with("--") => {
                return Err(format!("unbekannter Schalter {schalter}"));
            }
            _ => angaben.push(PathBuf::from(arg)),
        }
    }
    let ziel = if nur_quelle {
        let [ordner, quelle] = <[PathBuf; 2]>::try_from(angaben)
            .map_err(|_| "Ordner und Quelle nötig, sonst nichts".to_owned())?;
        let quelle = quelle
            .to_str()
            .filter(|q| quelle_ok(q))
            .ok_or_else(|| "Quelle: gateway, stelle, brain, halter oder ordnermarke".to_owned())?
            .to_owned();
        Ziel::Quelle(ordner, quelle)
    } else if angaben.is_empty() {
        return Err("keine Datei".to_owned());
    } else {
        Ziel::Dateien(angaben)
    };
    Ok(Aufruf {
        vorher: vorher.unwrap_or_else(|| NULL_HASH.to_owned()),
        letzte,
        ziel,
    })
}

/// Runs `kette` for the arguments after the program name.
///
/// Exit 0 when the chain is whole, 1 on a break, 64 on a wrong call (no sub-command, unknown sub-command or
/// switch, no file, for `pruefe-quelle` not exactly a folder and a source, a source that is none of the five, a
/// value that is not 64 lower-case hex digits, a switch without a value or given twice); with 64 the standard
/// output stays empty. No file is changed and no clock is read.
#[must_use]
pub fn befehl(args: &[OsString]) -> Ausgang {
    match aufruf(args) {
        Err(meldung) => Ausgang {
            exit: 64,
            stdout: String::new(),
            stderr: format!("kette: {meldung}\n{HILFE}\n"),
        },
        Ok(aufruf) => {
            let letzte = aufruf.letzte.as_deref();
            let ergebnis = match &aufruf.ziel {
                Ziel::Dateien(dateien) => pruefe(dateien, &aufruf.vorher, letzte),
                Ziel::Quelle(ordner, quelle) => {
                    pruefe_quelle_geprueft(ordner, quelle, &aufruf.vorher, letzte)
                }
            };
            Ausgang {
                exit: u8::from(!ergebnis.ok()),
                stdout: ergebnis.ausgabe(),
                stderr: String::new(),
            }
        }
    }
}
