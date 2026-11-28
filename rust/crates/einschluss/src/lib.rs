//! The Linux enclosure of a session (part of the platform layer): a fixed `bwrap` profile that the start
//! builds from a typed description and runs by `exec`.
//!
//! What the enclosure holds is a property of the namespaces, not of a promise:
//! - the harness sees only the whitelist of the profile; the vault, the log and the start file do not exist inside
//!   (an access fails with "no such file", not with "denied");
//! - it has its own network space with `lo` only;
//! - it has its own process space: the search process runs under the same user but is not visible, cannot be signalled
//!   and its environment and memory cannot be read (property E3). The user is the same; the namespaces separate;
//! - the search socket stands at one fixed path, [`GAST_SUCH_SOCKET`]; its folder is not mounted;
//! - the environment is cleared and set again with five variables (the cloud harness gets three more, with fixed
//!   values);
//! - the cloud harness reaches the outside only through a forwarder on the own loopback that passes the bytes to one
//!   socket ([`weiter`]), and sees the bundle of certificates of the session at the place of the system bundle.
//!
//! There is no `seccomp` filter yet. A fault of the kernel breaks this enclosure.
//!
//! Use: describe ([`Beschreibung`]), build ([`Profil::bauen`], pure, runs on every platform), check the host
//! ([`Profil::vorpruefen`], Linux only: on another system it answers [`Fehler::NichtVerfuegbar`]), then run
//! [`Profil::befehl`]. The program "Lauf starten" (not part of this crate) calls these steps and replaces itself by the
//! command, so the pseudo-terminal stays the terminal of the harness.
//!
//! The stand-in harness for the outside tests exists only in test builds (feature `attrappe`, and the unit tests of
//! the crate).

mod beschreibung;
mod eingabe;
pub mod erweiterung;
mod fehler;
mod json;
pub mod paket;
pub mod pfad;
mod profil;
pub mod starter;
pub mod verbote;
mod vorpruefung;
#[cfg(unix)]
pub mod weiter;

// the unit tests of the crate check the stand-in too: they run without the feature (and so in the mutation run)
#[cfg(any(test, feature = "attrappe"))]
pub mod attrappe;

pub use beschreibung::{
    Beschreibung, COPILOT_PLATZHALTER, Cloud, DATEI_ZIELE, Datei, ETC_DATEIEN,
    GAST_ANFORDERN_SOCKET, GAST_MODELL_SOCKET, GAST_SUCH_SOCKET, GAST_WEITER, GESPRAECH_ORTE,
    HARNESS_COPILOT, HARNESS_OPENCODE, HARNESS_PI, LOKAL_PORT, OPENCODE_ORDNER, OPENCODE_UMGEBUNG,
    OPENCODE_ZUSTAND, ORDNERMARKE_PFAD, PAKET_GAST_CACHE, PAKET_MARKE, PAKET_PLATTFORMEN,
    PI_UMGEBUNG, Paket, System, Umgebung, Verteilung, WEITER_PORT, Zustand, fassung_gut,
    gespraech_ort, opencode_ordner, paket_pfade, paket_ziel,
};
pub use eingabe::{EINGABE_MAX, beschreibung_lesen, beschreibung_schreiben};
pub use fehler::Fehler;
pub use paket::{
    PaketMangel, Vorentpacken, paket_pruefen, vorentpacken_lesen, vorentpacken_schreiben,
};
pub use profil::{ANKER_ABBRUCH, ANKER_NAME, ANKER_SKRIPT, BWRAP_PFAD, LEINE_FD, Profil};
#[cfg(unix)]
pub use vorpruefung::vorentpacken_pruefen;
