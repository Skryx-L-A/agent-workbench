//! The check of the peer that the search process makes itself under Windows, after `peerkanal` accepted the identity and before
//! the hello goes back (addendum for Windows, section 4, the last three steps): the platform service names the run whose job
//! holds the process, and only the agent of this session gets an answer.
//!
//! The request and the decision are pure functions, so they are tested on every system. The question itself goes to
//! `\\.\pipe\wb-plattform` (contract `windows-einschluss`, control pipe) and needs the check of the owner of that pipe before
//! the first byte; see [`fragen`].

use serde_json::Value;

/// The control pipe of the platform service.
pub const DIENST_PIPE: &str = r"\\.\pipe\wb-plattform";

/// The time the service has for its answer (addendum for Windows, section 4).
pub const DIENST_FRIST_MS: u64 = 500;

/// The service is not reachable, the owner of its pipe is not the service, no answer in time, or an error other than
/// `dienst.peer_unbekannt`.
pub const GRUND_STUMM: &str = "such.dienst_stumm";
/// The service knows no run of the process, or pid, start time or package SID of its answer differ from the measurement.
pub const GRUND_LAUF_UNBEKANNT: &str = "such.lauf_unbekannt";
/// The run of the process belongs to another session.
pub const GRUND_SITZUNG_FREMD: &str = "such.sitzung_fremd";

/// What `peerkanal` measured of the client: the bound process and the package SID of its token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Messung {
    /// The pid of the bound process.
    pub pid: u32,
    /// Its start time as a FILETIME number, if it could be read.
    pub startzeit: Option<u64>,
    /// The package SID of the token, `None` outside a container.
    pub paket_sid: Option<String>,
}

/// The request line `{"op":"peer","pid":<pid>}` with its LF.
#[must_use]
pub fn anfrage(pid: u32) -> Vec<u8> {
    format!("{{\"op\":\"peer\",\"pid\":{pid}}}\n").into_bytes()
}

/// The decision about one answer of the service: `None` accepts, `Some(grund)` turns the connection away. `antwort` is the
/// answer line without its LF, or `None` when no answer came (not reachable, wrong owner, no answer within the time).
#[must_use]
pub fn entscheiden(
    antwort: Option<&[u8]>,
    messung: &Messung,
    sitzung: &str,
) -> Option<&'static str> {
    let Some(zeile) = antwort else {
        return Some(GRUND_STUMM);
    };
    let Ok(Value::Object(felder)) = serde_json::from_slice::<Value>(zeile) else {
        return Some(GRUND_STUMM);
    };
    match felder.get("ok") {
        Some(Value::Bool(true)) => {}
        Some(Value::Bool(false)) => {
            return match felder.get("code").and_then(Value::as_str) {
                Some("dienst.peer_unbekannt") => Some(GRUND_LAUF_UNBEKANNT),
                _ => Some(GRUND_STUMM),
            };
        }
        _ => return Some(GRUND_STUMM),
    }
    let (Some(pid), Some(start), Some(sid), Some(lauf)) = (
        felder.get("pid").and_then(Value::as_u64),
        felder.get("start").and_then(Value::as_u64),
        felder.get("sid").and_then(Value::as_str),
        felder.get("sitzung").and_then(Value::as_str),
    ) else {
        return Some(GRUND_STUMM);
    };
    let gleich = pid == u64::from(messung.pid)
        && messung.startzeit == Some(start)
        && messung.paket_sid.as_deref() == Some(sid);
    if !gleich {
        return Some(GRUND_LAUF_UNBEKANNT);
    }
    (lauf != sitzung).then_some(GRUND_SITZUNG_FREMD)
}

/// Asks the platform service about the process `pid`: the answer line without its LF, or `None`.
///
/// Before the first byte the owner of the pipe instance must be the SID of `NT SERVICE\wb-plattform` (A115); a server with
/// another owner must not learn the pid and must not be able to name a session. `peerkanal` (version 1) checks the owner only
/// inside `verbinden`, which also sends its hello, and the control pipe speaks no hello. Without that check the question is not
/// asked: the answer is `None`, so every connection that reaches this step is turned away with [`GRUND_STUMM`] (fail
/// closed) until `peerkanal` offers a check of the owner without its hello.
#[must_use]
pub fn fragen(_pid: u32) -> Option<Vec<u8>> {
    None
}
