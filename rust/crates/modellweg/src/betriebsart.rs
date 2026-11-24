//! The modes of the model path (contract 8, section 2). A process has exactly one. The common part (start, socket, key
//! file, target, TLS, leak lock, line) does not depend on the mode; the rules of a mode stand in its own module.
//! Built are `openai-lokal` (part B) and, on Linux, `kopf-einsetzen` (part C; it needs OpenSSL for its own TLS server).

use std::collections::BTreeMap;

use crate::codes::Code;
use crate::http::Anfragekopf;
use crate::schluessel::Schluessel;
use crate::tokens::Zaehler;
use crate::ziel::Ziel;

pub mod kopf_einsetzen;
pub mod openai_lokal;
pub mod zielliste;

/// A mode of the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Betriebsart {
    /// Mode 1: OpenAI-compatible requests of a local harness to a self-hosted model (part B).
    OpenaiLokal,
    /// Mode 2: a forwarder for CONNECT that inserts the key header for named targets (part C). Linux only.
    #[cfg(target_os = "linux")]
    KopfEinsetzen,
}

impl Betriebsart {
    /// The name in the call and in the line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenaiLokal => "openai-lokal",
            #[cfg(target_os = "linux")]
            Self::KopfEinsetzen => "kopf-einsetzen",
        }
    }

    /// The mode of a name, if it is built; `None` is `mw.betriebsart_unbekannt`.
    #[must_use]
    pub fn aus_name(name: &str) -> Option<Self> {
        match name {
            "openai-lokal" => Some(Self::OpenaiLokal),
            #[cfg(target_os = "linux")]
            "kopf-einsetzen" => Some(Self::KopfEinsetzen),
            _ => None,
        }
    }
}

/// The options of the mode of a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtOptionen {
    /// The options of `openai-lokal` (section 9).
    OpenaiLokal(openai_lokal::Optionen),
    /// The options of `kopf-einsetzen` (section 13.5).
    #[cfg(target_os = "linux")]
    KopfEinsetzen(kopf_einsetzen::Optionen),
}

/// The option names of every mode.
const ALLE_OPTIONEN: [&str; 6] = [
    openai_lokal::OPTIONEN[0],
    openai_lokal::OPTIONEN[1],
    openai_lokal::OPTIONEN[2],
    kopf_einsetzen::OPTIONEN[0],
    kopf_einsetzen::OPTIONEN[1],
    kopf_einsetzen::OPTIONEN[2],
];

/// The option names of every built mode (known names, so the call can refuse unknown ones before the mode is known).
pub(crate) fn alle_optionen() -> &'static [&'static str] {
    &ALLE_OPTIONEN
}

/// Takes the options of the mode out of `werte`; `None` if one is missing or has a wrong form (`mw.aufruf`).
pub(crate) fn optionen_lesen(
    art: Betriebsart,
    werte: &mut BTreeMap<String, String>,
) -> Option<ArtOptionen> {
    match art {
        Betriebsart::OpenaiLokal => {
            openai_lokal::Optionen::lies(werte).map(ArtOptionen::OpenaiLokal)
        }
        #[cfg(target_os = "linux")]
        Betriebsart::KopfEinsetzen => {
            kopf_einsetzen::Optionen::lies(werte).map(ArtOptionen::KopfEinsetzen)
        }
    }
}

/// The rules of a mode for one request: the stages that belong to the mode, the bytes to the target and the tokens.
pub(crate) trait Regeln: Send + Sync {
    /// Stages 4 and 5: path and method.
    fn pruefe_kopf(&self, kopf: &Anfragekopf) -> Result<(), Code>;
    /// Stages 9 and 10: the body.
    fn pruefe_koerper(&self, koerper: &[u8]) -> Result<(), Code>;
    /// The exact bytes to the target for a request that passed every stage.
    fn an_ziel(
        &self,
        kopf: &Anfragekopf,
        koerper: &[u8],
        ziel: &Ziel,
        schluessel: &Schluessel,
    ) -> Vec<u8>;
    /// A counter of the tokens of one answer.
    fn zaehler(&self) -> Zaehler;
}
