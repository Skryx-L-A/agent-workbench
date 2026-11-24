//! The command `werkbank-ordnermarke` (contract 1, "Lösen und Senken nur durch einen Menschen"):
//!
//! ```text
//! werkbank-ordnermarke zeigen <ordner>
//! werkbank-ordnermarke loesen <ordner>
//! werkbank-ordnermarke senken <ordner> --klasse <klasse>
//! ```
//!
//! It changes only the entry with exactly the resolved path, through the same lock and the same atomic write as the
//! start ([`crate::marke`]). Each change writes one line `ordnermarke.geloest` or `ordnermarke.gesenkt` into
//! `<protokoll>/ordnermarke-<JJJJ-MM>.jsonl` (contract 5, section 6.4) with the key of the mark, customer and class
//! before and after, and never a path or a hash of a path. The order is fixed: lock, read and check, write the new
//! file and `fsync`, append the line, then `rename`. A fault before the `rename` removes the new file and changes
//! nothing; the mark never changes without its line.
//!
//! The command reads the own configuration of "Lauf starten" (`/etc/werkbank/lauf.json`; the test build the file next
//! to the program): the fields `zustand`, `protokoll` and `einstellungen`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use formate::kette::{Ordnermarke, anhaengen_ordnermarke};

use crate::fehler::Fehler;
use crate::marke::{self, Marke, Schreibfehler, Zustand};

/// The source of the chain file of the lines.
pub const QUELLE: &str = "ordnermarke";

/// Why the command ends with a fault. Each variant is one row of the table of contract 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BefehlFehler {
    /// Wrong arguments; for `senken` also a class the settings do not know or one that is not below the mark.
    Aufruf,
    /// No entry with exactly this path.
    KeineMarke,
    /// The file of marks is unreadable or broken (also a wrong key), or the state folder cannot be used.
    MarkeKaputt,
    /// The log line or the `rename` failed; the mark stays.
    Protokoll,
    /// The own configuration file does not exist.
    KonfigFehlt,
    /// The own configuration file or the settings file is unreadable or invalid.
    KonfigKaputt,
}

impl BefehlFehler {
    /// The code of the table.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            BefehlFehler::Aufruf => "ordnermarke.aufruf",
            BefehlFehler::KeineMarke => "ordnermarke.keine_marke",
            BefehlFehler::MarkeKaputt => "ordnermarke.marke_kaputt",
            BefehlFehler::Protokoll => "ordnermarke.protokoll",
            BefehlFehler::KonfigFehlt => "ordnermarke.konfig_fehlt",
            BefehlFehler::KonfigKaputt => "ordnermarke.konfig_kaputt",
        }
    }

    /// The fixed reason (ASCII, no path, no content).
    #[must_use]
    pub fn grund(self) -> &'static str {
        match self {
            BefehlFehler::Aufruf => "Aufruf ungueltig",
            BefehlFehler::KeineMarke => "Keine Marke fuer diesen Ordner",
            BefehlFehler::MarkeKaputt => "Ordnermarken unlesbar oder beschaedigt",
            BefehlFehler::Protokoll => "Protokollzeile nicht schreibbar",
            BefehlFehler::KonfigFehlt => "Konfigurationsdatei fehlt",
            BefehlFehler::KonfigKaputt => "Konfigurationsdatei nicht lesbar oder ungueltig",
        }
    }

    /// The exit code (`sysexits.h`).
    #[must_use]
    pub fn exitcode(self) -> u8 {
        match self {
            BefehlFehler::Aufruf => 64,
            BefehlFehler::KeineMarke => 66,
            BefehlFehler::Protokoll => 74,
            BefehlFehler::MarkeKaputt | BefehlFehler::KonfigFehlt | BefehlFehler::KonfigKaputt => {
                78
            }
        }
    }

    /// The one line on standard error: `<code>: <grund>` and LF.
    #[must_use]
    pub fn zeile(self) -> String {
        format!("{}: {}\n", self.code(), self.grund())
    }
}

/// The three commands with their arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Befehl {
    /// `zeigen <ordner>`.
    Zeigen(PathBuf),
    /// `loesen <ordner>`.
    Loesen(PathBuf),
    /// `senken <ordner> --klasse <klasse>`.
    Senken(PathBuf, String),
}

impl Befehl {
    fn ordner(&self) -> &Path {
        match self {
            Befehl::Zeigen(p) | Befehl::Loesen(p) | Befehl::Senken(p, _) => p,
        }
    }
}

/// Reads the arguments (without the program name): exactly the three forms above, nothing else.
///
/// # Errors
/// `ordnermarke.aufruf`.
pub fn aufruf_lesen(args: &[OsString]) -> Result<Befehl, BefehlFehler> {
    let ordner = |a: &OsString| {
        if a.is_empty() {
            Err(BefehlFehler::Aufruf)
        } else {
            Ok(PathBuf::from(a))
        }
    };
    match args {
        [b, o] if b == "zeigen" => Ok(Befehl::Zeigen(ordner(o)?)),
        [b, o] if b == "loesen" => Ok(Befehl::Loesen(ordner(o)?)),
        [b, o, f, k] if b == "senken" && f == "--klasse" => {
            let klasse = k.to_str().ok_or(BefehlFehler::Aufruf)?;
            Ok(Befehl::Senken(ordner(o)?, klasse.to_owned()))
        }
        _ => Err(BefehlFehler::Aufruf),
    }
}

/// The path as the marks hold it: `realpath`. A folder that is gone has no link left to follow, so the part that
/// exists is resolved and the rest appended (a mark of a deleted folder can still be released). `None` when the
/// result is not a path of the form of `arbeitsordner` or the rest has a `.` or `..`.
#[must_use]
pub fn aufloesen(pfad: &Path) -> Option<String> {
    let mut rest: Vec<OsString> = Vec::new();
    let mut hier = if pfad.is_absolute() {
        pfad.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(pfad)
    };
    let echt = loop {
        match std::fs::canonicalize(&hier) {
            Ok(e) => break e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let name = hier.file_name()?.to_owned();
                rest.push(name);
                hier = hier.parent()?.to_path_buf();
            }
            Err(_) => return None,
        }
    };
    let mut ganz = echt;
    ganz.extend(rest.iter().rev());
    let text = ganz.to_str()?;
    crate::ordner_form(text).then(|| text.to_owned())
}

/// The data the command needs from the own configuration and the settings.
#[derive(Debug, Clone)]
pub struct Lage {
    /// The folder of `ordnermarken.json`.
    pub zustand: PathBuf,
    /// The log folder.
    pub protokoll: PathBuf,
    /// The names of the classes, lowest first.
    pub klassen: Vec<String>,
}

/// Reads the own configuration and the settings file.
///
/// # Errors
/// `ordnermarke.konfig_fehlt`, `ordnermarke.konfig_kaputt`.
pub fn lage_lesen() -> Result<Lage, BefehlFehler> {
    let pfad = crate::konfig::pfad().ok_or(BefehlFehler::KonfigFehlt)?;
    let konfig = crate::konfig::lesen_datei(&pfad).map_err(|f| match f {
        Fehler::KonfigFehlt => BefehlFehler::KonfigFehlt,
        _ => BefehlFehler::KonfigKaputt,
    })?;
    let einst = crate::ablauf::einstellungen_lesen(&konfig.einstellungen)
        .map_err(|_| BefehlFehler::KonfigKaputt)?;
    Ok(Lage {
        zustand: konfig.zustand,
        protokoll: konfig.protokoll,
        klassen: einst.klassen().to_vec(),
    })
}

fn rang(klassen: &[String], klasse: &str) -> Option<usize> {
    klassen.iter().position(|k| k == klasse)
}

/// The text `zeigen` prints: one JSON object with the key, the customer (`null` is "no customer") and the highest class;
/// never the path.
#[must_use]
pub fn zeigen_text(m: &Marke) -> String {
    let kunde = m
        .kunde
        .as_deref()
        .map_or_else(|| "null".to_owned(), einstellungen::json::text_schreiben);
    format!(
        "{{\"kennung\":{},\"kunde\":{kunde},\"klasse_hoch\":{}}}\n",
        einstellungen::json::text_schreiben(&m.kennung),
        einstellungen::json::text_schreiben(&m.klasse_hoch),
    )
}

/// What a change does to the list and what its log line says (no path).
struct Aenderung {
    neu: Vec<Marke>,
    zeile: Ordnermarke,
}

fn jetzt_ms() -> Result<i64, BefehlFehler> {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| BefehlFehler::Protokoll)?;
    i64::try_from(d.as_millis()).map_err(|_| BefehlFehler::Protokoll)
}

/// The change of one entry. `senken` needs a class strictly below the mark (contract 1, open point of the contract).
fn aendern(
    befehl: &Befehl,
    ordner: &str,
    marken: &[Marke],
    klassen: &[String],
    zeit_ms: i64,
) -> Result<Aenderung, BefehlFehler> {
    let i = marken
        .iter()
        .position(|m| m.ordner == ordner)
        .ok_or(BefehlFehler::KeineMarke)?;
    let alt = &marken[i];
    let mut neu = marken.to_vec();
    let zeile = match befehl {
        Befehl::Loesen(_) => {
            neu.remove(i);
            Ordnermarke {
                zeit_ms,
                aktionsname: "ordnermarke.geloest".to_owned(),
                marke: alt.kennung.clone(),
                klasse_alt: Some(alt.klasse_hoch.clone()),
                klasse_neu: None,
                kunde_alt: alt.kunde.clone(),
                kunde_neu: None,
            }
        }
        Befehl::Senken(_, klasse) => {
            let ziel = rang(klassen, klasse).ok_or(BefehlFehler::Aufruf)?;
            let hoch = rang(klassen, &alt.klasse_hoch).ok_or(BefehlFehler::MarkeKaputt)?;
            if ziel >= hoch {
                return Err(BefehlFehler::Aufruf);
            }
            neu[i].klasse_hoch.clone_from(klasse);
            Ordnermarke {
                zeit_ms,
                aktionsname: "ordnermarke.gesenkt".to_owned(),
                marke: alt.kennung.clone(),
                klasse_alt: Some(alt.klasse_hoch.clone()),
                klasse_neu: Some(klasse.clone()),
                kunde_alt: alt.kunde.clone(),
                kunde_neu: alt.kunde.clone(),
            }
        }
        Befehl::Zeigen(_) => return Err(BefehlFehler::Aufruf),
    };
    Ok(Aenderung { neu, zeile })
}

fn marke_fehler(f: &Fehler) -> BefehlFehler {
    let _ = f;
    BefehlFehler::MarkeKaputt
}

/// Runs the command. Returns the text for standard output on success.
///
/// # Errors
/// The rows of the table of contract 1 and the two rows of the configuration.
pub fn ausfuehren(befehl: &Befehl, lage: &Lage) -> Result<String, BefehlFehler> {
    if let Befehl::Senken(_, klasse) = befehl
        && rang(&lage.klassen, klasse).is_none()
    {
        return Err(BefehlFehler::Aufruf);
    }
    let ordner = aufloesen(befehl.ordner()).ok_or(BefehlFehler::Aufruf)?;
    if matches!(befehl, Befehl::Zeigen(_)) {
        let zustand = Zustand::ohne_sperre(&lage.zustand).map_err(|f| marke_fehler(&f))?;
        let marken = zustand.lesen(&lage.klassen).map_err(|f| marke_fehler(&f))?;
        let m = marken
            .iter()
            .find(|m| m.ordner == ordner)
            .ok_or(BefehlFehler::KeineMarke)?;
        return Ok(zeigen_text(m));
    }
    let zustand =
        Zustand::sperren(&lage.zustand, marke::SPERR_FRIST).map_err(|f| marke_fehler(&f))?;
    let marken = zustand.lesen(&lage.klassen).map_err(|f| marke_fehler(&f))?;
    let aenderung = aendern(befehl, &ordner, &marken, &lage.klassen, jetzt_ms()?)?;
    let zeile = aenderung.zeile;
    zustand
        .ersetzen(&aenderung.neu, || {
            anhaengen_ordnermarke(&lage.protokoll, QUELLE, &zeile)
                .map(|_| ())
                .map_err(|_| Fehler::Intern)
        })
        .map_err(|f| match f {
            Schreibfehler::Datei => BefehlFehler::MarkeKaputt,
            Schreibfehler::Zwischen | Schreibfehler::Umbenennen | Schreibfehler::Ordner => {
                BefehlFehler::Protokoll
            }
        })?;
    Ok(String::new())
}

/// The whole command: arguments, configuration, execution. Returns the text for standard output.
///
/// # Errors
/// As [`aufruf_lesen`], [`lage_lesen`] and [`ausfuehren`], in this order.
pub fn befehl(args: &[OsString]) -> Result<String, BefehlFehler> {
    let befehl = aufruf_lesen(args)?;
    let lage = lage_lesen()?;
    ausfuehren(&befehl, &lage)
}

#[cfg(test)]
mod tests;
