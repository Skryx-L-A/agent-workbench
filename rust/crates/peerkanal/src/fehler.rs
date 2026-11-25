//! Codes, reasons, the event of a refusal and the peer value (contract sections "Annehmen und Messen", "Peer" and
//! "Client verbinden").

use std::fmt;

use serde_json::{Value, json};

use crate::kennung::Kennung;

/// The codes of the contract. Text with the prefix `pk.`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Code {
    /// The settings are not in the form of the schema, or they do not fit the system.
    VorgabeUngueltig,
    /// Unix: the socket folder is missing, no folder, a link, not owned by the own uid or not exactly mode 0700.
    OrdnerRechte,
    /// Unix: `<ordner>/<name>.sock` does not fit into `sun_path`.
    PfadZuLang,
    /// The name or path is taken. No fallback.
    NameBelegt,
    /// Any other error of the system while creating the server.
    AnlegenFehlgeschlagen,
    /// Step 3: the hello did not come in time or not in its form.
    HalloUngueltig,
    /// Step 4: the identity of the other side could not be measured.
    PeerUnmessbar,
    /// Step 5: the bound process is not the one that connected.
    ProzessGewechselt,
    /// Step 6: the measured identity is not in the list.
    KennungNichtErlaubt,
    /// Step 7: the hook of the caller refused.
    AufruferAbgewiesen,
    /// Client: no server under the name, or every instance busy until the end of the time limit.
    NichtErreichbar,
    /// Client: the system refuses (Windows error 5).
    ZugriffVerweigert,
    /// Client: the identity of the server could not be read.
    ServerUnmessbar,
    /// Client: the owner of the pipe instance is not the expected SID. No byte was sent.
    FalscherServer,
    /// Client: the server closed after the hello without an answer, or the answer did not come in time.
    Abgewiesen,
    /// An error while reading or writing after the handshake, with `os_fehler`.
    Io,
    /// The call has no meaning on this system (for example [`crate::eigene_sid`] outside Windows).
    NichtUnterstuetzt,
}

impl Code {
    /// The text of the code, such as `pk.name_belegt`.
    #[must_use]
    pub fn als_str(self) -> &'static str {
        match self {
            Self::VorgabeUngueltig => "pk.vorgabe_ungueltig",
            Self::OrdnerRechte => "pk.ordner_rechte",
            Self::PfadZuLang => "pk.pfad_zu_lang",
            Self::NameBelegt => "pk.name_belegt",
            Self::AnlegenFehlgeschlagen => "pk.anlegen_fehlgeschlagen",
            Self::HalloUngueltig => "pk.hallo_ungueltig",
            Self::PeerUnmessbar => "pk.peer_unmessbar",
            Self::ProzessGewechselt => "pk.prozess_gewechselt",
            Self::KennungNichtErlaubt => "pk.kennung_nicht_erlaubt",
            Self::AufruferAbgewiesen => "pk.aufrufer_abgewiesen",
            Self::NichtErreichbar => "pk.nicht_erreichbar",
            Self::ZugriffVerweigert => "pk.zugriff_verweigert",
            Self::ServerUnmessbar => "pk.server_unmessbar",
            Self::FalscherServer => "pk.falscher_server",
            Self::Abgewiesen => "pk.abgewiesen",
            Self::Io => "pk.io",
            Self::NichtUnterstuetzt => "pk.nicht_unterstuetzt",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.als_str())
    }
}

/// An error of the library: a code and, where the system gave one, its error number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fehler {
    /// The code.
    pub code: Code,
    /// The error of the system (Windows: `GetLastError`), where there is one.
    pub os_fehler: Option<i32>,
}

impl Fehler {
    pub(crate) fn neu(code: Code) -> Self {
        Self {
            code,
            os_fehler: None,
        }
    }

    pub(crate) fn os(code: Code, os_fehler: i32) -> Self {
        Self {
            code,
            os_fehler: Some(os_fehler),
        }
    }
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.os_fehler {
            Some(n) => write!(f, "{} (os {n})", self.code),
            None => write!(f, "{}", self.code),
        }
    }
}

impl std::error::Error for Fehler {}

/// The fixed reason of a refusal, in the order of the contract table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Grund {
    /// The connection or `hallo_frist` ended before 4 bytes came, and the bytes that came are a start of the hello (also no
    /// byte at all).
    HalloFehlt,
    /// The received bytes are no start of the hello `pk1` LF.
    HalloFalsch,
    /// Impersonation or reading the token failed.
    MessungGescheitert,
    /// `prozess_pflicht` holds and the process could not be bound.
    ProzessNichtOffen,
    /// A job is set and `IsProcessInJob` gave no answer.
    JobUnbekannt,
    /// The bound process has ended.
    ProzessBeendet,
    /// The bound process started after the moment of the connection.
    StartzeitWeichtAb,
    /// macOS: the pid version does not match a living process.
    PidversionWeichtAb,
    /// The token carries restricting SIDs.
    Eingeschraenkt,
    /// No permission with this SID.
    SidNichtErlaubt,
    /// No permission with this pair of SID and package SID.
    AppcontainerFalsch,
    /// Unix: no permission with this uid.
    UidNichtErlaubt,
    /// Unix: the primary gid differs.
    GidFalsch,
    /// The hook of the caller refused, with its own name: `aufrufer:<kennung>`.
    Aufrufer(Kennung),
}

impl Grund {
    /// The code group of the reason.
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            Self::HalloFehlt | Self::HalloFalsch => Code::HalloUngueltig,
            Self::MessungGescheitert | Self::ProzessNichtOffen | Self::JobUnbekannt => {
                Code::PeerUnmessbar
            }
            Self::ProzessBeendet | Self::StartzeitWeichtAb | Self::PidversionWeichtAb => {
                Code::ProzessGewechselt
            }
            Self::Eingeschraenkt
            | Self::SidNichtErlaubt
            | Self::AppcontainerFalsch
            | Self::UidNichtErlaubt
            | Self::GidFalsch => Code::KennungNichtErlaubt,
            Self::Aufrufer(_) => Code::AufruferAbgewiesen,
        }
    }

    /// The text of the reason, such as `pk.hallo_fehlt` or `aufrufer:sitzung-fremd`.
    #[must_use]
    pub fn als_text(&self) -> String {
        match self {
            Self::HalloFehlt => "pk.hallo_fehlt".to_owned(),
            Self::HalloFalsch => "pk.hallo_falsch".to_owned(),
            Self::MessungGescheitert => "pk.messung_gescheitert".to_owned(),
            Self::ProzessNichtOffen => "pk.prozess_nicht_offen".to_owned(),
            Self::JobUnbekannt => "pk.job_unbekannt".to_owned(),
            Self::ProzessBeendet => "pk.prozess_beendet".to_owned(),
            Self::StartzeitWeichtAb => "pk.startzeit_weicht_ab".to_owned(),
            Self::PidversionWeichtAb => "pk.pidversion_weicht_ab".to_owned(),
            Self::Eingeschraenkt => "pk.eingeschraenkt".to_owned(),
            Self::SidNichtErlaubt => "pk.sid_nicht_erlaubt".to_owned(),
            Self::AppcontainerFalsch => "pk.appcontainer_falsch".to_owned(),
            Self::UidNichtErlaubt => "pk.uid_nicht_erlaubt".to_owned(),
            Self::GidFalsch => "pk.gid_falsch".to_owned(),
            Self::Aufrufer(k) => format!("aufrufer:{k}"),
        }
    }
}

/// The operating system of a peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum System {
    /// Windows.
    Windows,
    /// Linux.
    Linux,
    /// macOS.
    Macos,
}

impl System {
    /// The system this build runs on. Other Unix systems count as Linux.
    #[must_use]
    pub const fn aktuell() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }

    /// `windows`, `linux` or `macos`.
    #[must_use]
    pub fn als_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Macos => "macos",
        }
    }

    /// Windows or not.
    #[must_use]
    pub fn ist_windows(self) -> bool {
        self == Self::Windows
    }
}

/// The code signature of a peer (macOS).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signatur {
    /// The signing identifier.
    pub kennung: String,
    /// The team id, if any.
    pub team_id: Option<String>,
    /// Whether `SecCodeCheckValidity` accepted it.
    pub gueltig: bool,
}

/// The measurement of the other side (contract section "Peer"). What could not be measured is `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    /// The system of the measurement.
    pub system: System,
    /// Windows: `TokenUser` of the impersonated token; at the client the owner of the pipe instance.
    pub sid: Option<String>,
    /// Windows: `TokenAppContainerSid`, `None` outside a container.
    pub appcontainer_sid: Option<String>,
    /// Unix: uid.
    pub uid: Option<u32>,
    /// Unix: primary gid.
    pub gid: Option<u32>,
    /// The process id.
    pub pid: Option<u32>,
    /// Windows: `FILETIME` of the creation of the process.
    pub startzeit: Option<u64>,
    /// Windows: `IsProcessInJob` against the job of the settings.
    pub im_job: Option<bool>,
    /// The image path.
    pub programm: Option<String>,
    /// macOS: the code signature.
    pub signatur: Option<Signatur>,
    /// Windows: the token carries restricting SIDs.
    pub eingeschraenkt: Option<bool>,
    /// macOS: the process runs under the sandbox.
    pub sandbox: Option<bool>,
}

impl Peer {
    /// An empty measurement of the given system.
    #[must_use]
    pub fn leer(system: System) -> Self {
        Self {
            system,
            sid: None,
            appcontainer_sid: None,
            uid: None,
            gid: None,
            pid: None,
            startzeit: None,
            im_job: None,
            programm: None,
            signatur: None,
            eingeschraenkt: None,
            sandbox: None,
        }
    }

    /// The peer as JSON in the form of the schema (`$defs/peer`).
    #[must_use]
    pub fn als_json(&self) -> Value {
        json!({
            "system": self.system.als_str(),
            "sid": self.sid,
            "appcontainer_sid": self.appcontainer_sid,
            "uid": self.uid,
            "gid": self.gid,
            "pid": self.pid,
            "startzeit": self.startzeit,
            "im_job": self.im_job,
            "programm": self.programm,
            "signatur": self.signatur.as_ref().map(|s| json!({
                "kennung": s.kennung, "team_id": s.team_id, "gueltig": s.gueltig
            })),
            "eingeschraenkt": self.eingeschraenkt,
            "sandbox": self.sandbox,
        })
    }
}

/// A refusal after the system let the connection through. The caller writes it into its log; none stays silent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ereignis {
    /// The channel.
    pub kanal: Kennung,
    /// The fixed reason; its code is [`Grund::code`].
    pub grund: Grund,
    /// The measured peer, or `None` where the step came before the measurement of the identity.
    pub peer: Option<Peer>,
}

impl Ereignis {
    /// The code of the event.
    #[must_use]
    pub fn code(&self) -> Code {
        self.grund.code()
    }

    /// The event as JSON in the form of the schema (`$defs/ereignis`). The received bytes are never in it.
    #[must_use]
    pub fn als_json(&self) -> Value {
        json!({
            "ereignis": "abgewiesen",
            "kanal": self.kanal.als_str(),
            "code": self.code().als_str(),
            "grund": self.grund.als_text(),
            "peer": self.peer.as_ref().map(Peer::als_json),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texte_der_codes_fehler_und_systeme() {
        assert_eq!(Code::NameBelegt.to_string(), "pk.name_belegt");
        assert_eq!(Fehler::neu(Code::Io).to_string(), "pk.io");
        assert_eq!(
            Fehler::os(Code::NameBelegt, 5).to_string(),
            "pk.name_belegt (os 5)"
        );
        assert_eq!(System::Windows.als_str(), "windows");
        assert_eq!(System::Linux.als_str(), "linux");
        assert_eq!(System::Macos.als_str(), "macos");
    }

    #[test]
    fn peer_als_json_mit_jedem_feld() {
        let mut p = Peer::leer(System::Macos);
        p.uid = Some(501);
        p.signatur = Some(Signatur {
            kennung: "beispiel".to_owned(),
            team_id: None,
            gueltig: true,
        });
        assert_eq!(
            p.als_json(),
            json!({
                "system": "macos", "sid": null, "appcontainer_sid": null, "uid": 501, "gid": null, "pid": null,
                "startzeit": null, "im_job": null, "programm": null,
                "signatur": {"kennung": "beispiel", "team_id": null, "gueltig": true},
                "eingeschraenkt": null, "sandbox": null
            })
        );
    }
}
