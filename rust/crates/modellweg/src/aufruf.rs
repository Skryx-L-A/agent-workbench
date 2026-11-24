//! The call of the program (contract 8, section 4): the options for every mode, `--betriebsart`, and the order of the check.
//! The options of a mode are read by the mode itself (section 9 for `openai-lokal`).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::betriebsart::{self, ArtOptionen, Betriebsart};

/// The options for every mode, except `--betriebsart`.
pub const GEMEINSAM: [&str; 5] = [
    "--socket",
    "--socket-rechte",
    "--sitzung",
    "--route",
    "--protokoll",
];

/// The option that chooses the mode; without it the mode is `openai-lokal`.
pub const BETRIEBSART: &str = "--betriebsart";

/// The switch of a trial run (section 9).
pub const PROBELAUF: &str = "--probelauf";
/// The markers of a trial run.
pub const MARKER: &str = "--marker";
/// The count file of a trial run.
pub const MARKER_ZAEHLUNG: &str = "--marker-zaehlung";
/// At most this many markers.
pub const MARKER_MAX: usize = 16;

/// Longest socket path in bytes (`sun_path` without the closing zero).
pub const SOCKET_MAX: usize = 107;
/// Longest path of a key file and of the protocol folder in bytes.
pub const PFAD_MAX: usize = 1024;

/// Why a call is refused: `mw.aufruf` or `mw.betriebsart_unbekannt` (both exit 64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aufruffehler {
    /// Unknown or doubled option, missing value, missing option or a wrong form (`mw.aufruf`).
    Aufruf,
    /// The mode is unknown or not built (`mw.betriebsart_unbekannt`).
    BetriebsartUnbekannt,
    /// The trial options are not all three, or a marker or the count file has a wrong form (`mw.probelauf_aufruf`).
    ProbelaufAufruf,
}

/// The options of a trial run: the markers in the order of the call and the count file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probelauf {
    /// The markers (1 to 16, different).
    pub marker: Vec<String>,
    /// The count file.
    pub datei: PathBuf,
}

/// A checked call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aufruf {
    /// The mode of the process.
    pub betriebsart: Betriebsart,
    /// Path of the socket.
    pub socket: PathBuf,
    /// Session from the checked start file.
    pub sitzung: String,
    /// Route from the checked start file.
    pub route: String,
    /// Folder of the chain.
    pub protokoll: PathBuf,
    /// The options of the mode.
    pub art: ArtOptionen,
    /// The trial run, if all three options are given.
    pub probelauf: Option<Probelauf>,
}

/// Reads the arguments after the program name in the order of section 4: all options (unknown or doubled:
/// `mw.aufruf`), then `--betriebsart`, then the trial options (`mw.probelauf_aufruf`), then completeness and form for
/// the mode (`mw.aufruf`).
///
/// # Errors
/// [`Aufruffehler`] as above.
pub fn lies(args: &[OsString]) -> Result<Aufruf, Aufruffehler> {
    let (mut werte, schalter) = optionen_lesen(args).ok_or(Aufruffehler::Aufruf)?;
    let art = match werte.remove(BETRIEBSART) {
        None => Betriebsart::OpenaiLokal,
        Some(name) => Betriebsart::aus_name(&name).ok_or(Aufruffehler::BetriebsartUnbekannt)?,
    };
    let probelauf = probelauf_lesen(
        schalter,
        werte.remove(MARKER),
        werte.remove(MARKER_ZAEHLUNG),
    )?;
    let [socket, rechte, sitzung, route, protokoll] = GEMEINSAM.map(|name| werte.remove(name));
    let art_optionen = betriebsart::optionen_lesen(art, &mut werte);
    if !werte.is_empty() {
        // An option of another mode.
        return Err(Aufruffehler::Aufruf);
    }
    let (
        Some(socket),
        Some(rechte),
        Some(sitzung),
        Some(route),
        Some(protokoll),
        Some(art_optionen),
    ) = (socket, rechte, sitzung, route, protokoll, art_optionen)
    else {
        return Err(Aufruffehler::Aufruf);
    };
    let gut = pfad_ok(&socket, SOCKET_MAX)
        && rechte == "0600"
        && kennung_ok(&sitzung)
        && kennung_ok(&route)
        && pfad_ok(&protokoll, PFAD_MAX);
    if !gut {
        return Err(Aufruffehler::Aufruf);
    }
    Ok(Aufruf {
        betriebsart: art,
        socket: PathBuf::from(socket),
        sitzung,
        route,
        protokoll: PathBuf::from(protokoll),
        art: art_optionen,
        probelauf,
    })
}

/// The trial options (section 9): all three or none; 1 to 16 markers `^[A-Z0-9-]{1,32}$` without a double, an
/// absolute path for the count file.
fn probelauf_lesen(
    schalter: bool,
    marker: Option<String>,
    datei: Option<String>,
) -> Result<Option<Probelauf>, Aufruffehler> {
    let (marker, datei) = match (schalter, marker, datei) {
        (false, None, None) => return Ok(None),
        (true, Some(m), Some(d)) => (m, d),
        _ => return Err(Aufruffehler::ProbelaufAufruf),
    };
    let liste: Vec<String> = marker.split(',').map(str::to_owned).collect();
    let mut sortiert = liste.clone();
    sortiert.sort();
    sortiert.dedup();
    let gut = liste.len() <= MARKER_MAX
        && sortiert.len() == liste.len()
        && liste.iter().all(|m| marker_ok(m))
        && pfad_ok(&datei, PFAD_MAX);
    if !gut {
        return Err(Aufruffehler::ProbelaufAufruf);
    }
    Ok(Some(Probelauf {
        marker: liste,
        datei: PathBuf::from(datei),
    }))
}

/// A marker `^[A-Z0-9-]{1,32}$`.
#[must_use]
pub fn marker_ok(m: &str) -> bool {
    (1..=32).contains(&m.len())
        && m.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'-')
}

/// Reads the options: `--probelauf` is a switch without a value, every other option has exactly one value. Every
/// name known (for every mode or for a mode), none twice, every part UTF-8. Returns the values and the switch.
fn optionen_lesen(args: &[OsString]) -> Option<(BTreeMap<String, String>, bool)> {
    let mut werte = BTreeMap::new();
    let mut schalter = false;
    let mut rest = args;
    while let Some((name, danach)) = rest.split_first() {
        let name = name.to_str()?;
        if name == PROBELAUF {
            if schalter {
                return None;
            }
            schalter = true;
            rest = danach;
            continue;
        }
        let bekannt = [BETRIEBSART, MARKER, MARKER_ZAEHLUNG].contains(&name)
            || GEMEINSAM.contains(&name)
            || betriebsart::alle_optionen().contains(&name);
        let (wert, danach) = danach.split_first()?;
        if !bekannt {
            return None;
        }
        if werte
            .insert(name.to_owned(), wert.to_str()?.to_owned())
            .is_some()
        {
            return None;
        }
        rest = danach;
    }
    Some((werte, schalter))
}

/// An absolute path without a segment `.` or `..`, without an empty segment and without a control character
/// (U+0000 to U+001F, U+007F to U+009F), at most `max` bytes.
#[must_use]
pub fn pfad_ok(pfad: &str, max: usize) -> bool {
    pfad.len() <= max
        && pfad.strip_prefix('/').is_some_and(|rest| {
            rest.split('/').all(|teil| {
                !teil.is_empty() && teil != "." && teil != ".." && !teil.chars().any(steuerzeichen)
            })
        })
}

fn steuerzeichen(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}')
}

/// A name `^[a-z0-9][a-z0-9._-]{0,63}$` (session and route).
#[must_use]
pub fn kennung_ok(s: &str) -> bool {
    let klein = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    s.len() <= 64
        && s.as_bytes().first().is_some_and(klein)
        && s.bytes()
            .all(|c| klein(&c) || matches!(c, b'.' | b'_' | b'-'))
}

/// A model `^[A-Za-z0-9._:/@+-]{1,128}$` (as `modell` in contract 1).
#[must_use]
pub fn modell_ok(s: &str) -> bool {
    (1..=128).contains(&s.len())
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._:/@+-".contains(&c))
}

#[cfg(test)]
mod tests {
    use super::{kennung_ok, marker_ok, modell_ok, pfad_ok};

    #[test]
    fn a_marker_has_limits() {
        assert!(marker_ok("MK-0"));
        assert!(marker_ok(&"A".repeat(32)));
        assert!(!marker_ok(&"A".repeat(33)));
        assert!(!marker_ok(""));
        assert!(!marker_ok("mk"));
        assert!(!marker_ok("A_B"));
        assert!(!marker_ok("A B"));
    }

    #[test]
    fn a_path_has_limits() {
        assert!(pfad_ok("/a", 2));
        assert!(!pfad_ok("/ab", 2));
        assert!(pfad_ok(&format!("/{}", "a".repeat(106)), 107));
        assert!(!pfad_ok(&format!("/{}", "a".repeat(107)), 107));
        assert!(!pfad_ok("/", 9));
        assert!(!pfad_ok("a/b", 9));
        assert!(!pfad_ok("/a//b", 9));
        assert!(!pfad_ok("/a/", 9));
        assert!(!pfad_ok("/a/./b", 9));
        assert!(!pfad_ok("/a/../b", 9));
        assert!(pfad_ok("/a/.b/..c", 9));
        assert!(!pfad_ok("/a\u{1f}", 9));
        assert!(!pfad_ok("/a\u{0}", 9));
        assert!(!pfad_ok("/a\u{7f}", 9));
        assert!(!pfad_ok("/a\u{9f}", 9));
        assert!(pfad_ok("/a\u{a0} b", 9));
    }

    #[test]
    fn a_name_has_limits() {
        assert!(kennung_ok("s-2"));
        assert!(kennung_ok("0"));
        assert!(kennung_ok(&"a".repeat(64)));
        assert!(!kennung_ok(&"a".repeat(65)));
        assert!(!kennung_ok(""));
        assert!(!kennung_ok("-a"));
        assert!(!kennung_ok(".a"));
        assert!(!kennung_ok("S-2"));
        assert!(!kennung_ok("a/b"));
        assert!(kennung_ok("a._-9"));
    }

    #[test]
    fn a_model_has_limits() {
        assert!(modell_ok("beispiel/modell:1b"));
        assert!(modell_ok("A@b+c_d"));
        assert!(modell_ok(&"a".repeat(128)));
        assert!(!modell_ok(&"a".repeat(129)));
        assert!(!modell_ok(""));
        assert!(!modell_ok("a b"));
        assert!(!modell_ok("a\"b"));
    }
}
