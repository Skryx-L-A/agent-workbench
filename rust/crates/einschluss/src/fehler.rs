//! The errors of the enclosure and the fixed texts of the contract (contract 1, step 5 and 6).

use std::fmt;

/// Why the enclosure cannot start. Every variant but [`Fehler::ArbeitsordnerFehlt`] maps to the code
/// `lauf.einschluss_fehlt`; the variant tells a person in the log what was wrong.
///
/// The detail text of [`fmt::Display`] holds no path of the session and no file content, with one exception:
/// [`Fehler::Probelauf`] carries the first bytes of the standard error of `bwrap`, which may name a path. That text
/// belongs in the log of the service and never on the terminal of the session (contract 1: the line on standard error
/// is the fixed reason only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fehler {
    /// The platform has no enclosure (every system but Linux).
    NichtVerfuegbar,
    /// The description is malformed; the text names the field.
    BeschreibungUngueltig(&'static str),
    /// A mounted path is or covers a secret or a login store, or is a system path.
    Verboten(&'static str),
    /// A mounted path overlaps a protected folder (vault, log, folder of the start file) or another mount.
    Ueberlappt(&'static str),
    /// The working folder does not exist or is not a folder.
    ArbeitsordnerFehlt,
    /// A mount source is missing, is not what it must be (socket, empty folder, own folder) or is a link.
    QuelleUnbrauchbar(&'static str),
    /// `bwrap` is not at the fixed path.
    BwrapFehlt,
    /// `bwrap` is there but cannot be run.
    BwrapNichtAusfuehrbar,
    /// The kernel lets a process push keystrokes into the terminal (`legacy_tiocsti` is not `0`, or the kernel is older
    /// than 6.2), and the profile has no `--new-session`.
    TiocstiOffen,
    /// The system refuses the user namespace the profile needs. On Ubuntu this is the `AppArmor` restriction of
    /// unprivileged user namespaces (the profile of the starter `werkbank-einschluss` is not loaded, or the call does
    /// not come through the starter); on other systems the kernel switch is off. A profile for `/usr/bin/bwrap` is
    /// never the remedy: only the starter carries one. The test run failed with a message that names the namespace (`Failed RTM_NEWADDR`, `creating new namespace`,
    /// `setting up uid map`).
    NamensraumGesperrt,
    /// The test run of the finished profile with `true` failed.
    Probelauf {
        /// Exit status, or `None` when `bwrap` was killed by a signal, did not start or timed out.
        status: Option<i32>,
        /// The first bytes of standard error, ASCII only.
        meldung: String,
    },
}

impl Fehler {
    /// The code of contract 1.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Fehler::ArbeitsordnerFehlt => "lauf.ordner_fehlt",
            _ => "lauf.einschluss_fehlt",
        }
    }

    /// The fixed reason of contract 1 for the line on standard error.
    #[must_use]
    pub fn grund(&self) -> &'static str {
        match self {
            Fehler::ArbeitsordnerFehlt => "Arbeitsordner fehlt oder ist kein Ordner",
            _ => "Einschluss nicht verfuegbar",
        }
    }
}

impl Fehler {
    /// The fixed word of the second line of the starter (`lauf.einschluss_teil: <exit code> <word>`): which of the checks
    /// refused. ASCII, never a path and never a file content; the variant alone decides it, not the detail text.
    #[must_use]
    pub fn teil(&self) -> &'static str {
        match self {
            Fehler::NichtVerfuegbar => "keine-plattform",
            Fehler::BeschreibungUngueltig(_) => "beschreibung-ungueltig",
            Fehler::Verboten(_) => "verboten",
            Fehler::Ueberlappt(_) => "ueberlappt",
            Fehler::ArbeitsordnerFehlt => "arbeitsordner-fehlt",
            Fehler::QuelleUnbrauchbar(_) => "quelle-unbrauchbar",
            Fehler::BwrapFehlt => "bwrap-fehlt",
            Fehler::BwrapNichtAusfuehrbar => "bwrap-nicht-ausfuehrbar",
            Fehler::TiocstiOffen => "tiocsti-offen",
            Fehler::NamensraumGesperrt => "namensraum-gesperrt",
            Fehler::Probelauf { .. } => "probelauf",
        }
    }
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fehler::NichtVerfuegbar => write!(f, "enclosure not available on this platform"),
            Fehler::BeschreibungUngueltig(was) => write!(f, "description invalid: {was}"),
            Fehler::Verboten(was) => write!(f, "forbidden mount: {was}"),
            Fehler::Ueberlappt(was) => write!(f, "overlap: {was}"),
            Fehler::ArbeitsordnerFehlt => write!(f, "working folder missing or not a folder"),
            Fehler::QuelleUnbrauchbar(was) => write!(f, "mount source unusable: {was}"),
            Fehler::BwrapFehlt => write!(f, "bwrap missing at the fixed path"),
            Fehler::BwrapNichtAusfuehrbar => write!(f, "bwrap cannot be run"),
            Fehler::NamensraumGesperrt => write!(
                f,
                "user namespaces are refused (the werkbank AppArmor profile for werkbank-einschluss is not loaded, or the kernel switch is off)"
            ),
            Fehler::TiocstiOffen => write!(f, "terminal injection (TIOCSTI) not closed"),
            Fehler::Probelauf { status, meldung } => {
                write!(f, "test run failed (status {status:?}): {meldung}")
            }
        }
    }
}

impl std::error::Error for Fehler {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_working_folder_has_its_own_code() {
        assert_eq!(Fehler::ArbeitsordnerFehlt.code(), "lauf.ordner_fehlt");
        assert_eq!(
            Fehler::ArbeitsordnerFehlt.grund(),
            "Arbeitsordner fehlt oder ist kein Ordner"
        );
        for f in [
            Fehler::NichtVerfuegbar,
            Fehler::BeschreibungUngueltig("x"),
            Fehler::Verboten("x"),
            Fehler::Ueberlappt("x"),
            Fehler::QuelleUnbrauchbar("x"),
            Fehler::BwrapFehlt,
            Fehler::BwrapNichtAusfuehrbar,
            Fehler::NamensraumGesperrt,
            Fehler::TiocstiOffen,
            Fehler::Probelauf {
                status: Some(1),
                meldung: String::new(),
            },
        ] {
            assert_eq!(f.code(), "lauf.einschluss_fehlt", "{f:?}");
            assert_eq!(f.grund(), "Einschluss nicht verfuegbar", "{f:?}");
        }
    }

    #[test]
    fn the_namespace_refusal_names_apparmor() {
        let text = Fehler::NamensraumGesperrt.to_string();
        assert!(
            text.contains("werkbank AppArmor profile for werkbank-einschluss")
                && text.contains("not loaded")
                && !text.contains("for /usr/bin/bwrap"),
            "{text}"
        );
    }

    #[test]
    fn the_detail_text_names_the_cause() {
        assert!(
            Fehler::Verboten("arbeitsordner")
                .to_string()
                .contains("arbeitsordner")
        );
        let probe = Fehler::Probelauf {
            status: Some(1),
            meldung: "kaputt".to_owned(),
        };
        let text = probe.to_string();
        assert!(
            text.contains("kaputt") && text.contains("Some(1)"),
            "{text}"
        );
    }
}
