//! The order of checks of section 7, stages 1 to 27, as one pure function over the bytes of a line and the view of the
//! holder ([`Sicht`]). Nothing here reaches a target, a file or a clock; the server fills the view from its state and
//! executes what passes (stages 28 to 31).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::b64;
use crate::code::Code;
use crate::ed25519;
use crate::form::{self, Op};
use crate::json;

/// Longest line without the LF (32 MiB).
pub const ZEILE_MAX: usize = 33_554_432;
/// Longest span of a ticket, `ablauf - ausgestellt` (section 6).
pub const SCHEIN_SPANNE_MAX: i64 = 86_400_000;
/// How far `ausgestellt` may lie after the own clock.
pub const SCHEIN_VORLAUF: i64 = 5000;

/// A role of section 2.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rolle {
    /// The MCP hub.
    Hub,
    /// The model path.
    Bruecke,
    /// The program in which the person enters a token.
    Oberflaeche,
}

impl Rolle {
    /// The role of a name in `halter.json`.
    #[must_use]
    pub fn aus_text(t: &str) -> Option<Rolle> {
        match t {
            "hub" => Some(Rolle::Hub),
            "bruecke" => Some(Rolle::Bruecke),
            "oberflaeche" => Some(Rolle::Oberflaeche),
            _ => None,
        }
    }

    /// `true` if the role may call `op` (table of section 2.1).
    #[must_use]
    pub fn erlaubt(self, op: Op) -> bool {
        match self {
            Rolle::Hub => matches!(op, Op::Vorhanden | Op::McpAufruf),
            Rolle::Bruecke => matches!(
                op,
                Op::Vorhanden
                    | Op::ModellAufruf
                    | Op::ModellStrom
                    | Op::StromTeil
                    | Op::StromEnde
                    | Op::ModellSuche
            ),
            Rolle::Oberflaeche => matches!(op, Op::Vorhanden | Op::Setzen | Op::Loeschen),
        }
    }
}

/// The measured caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aufrufer {
    /// The role, `None` if the peer check failed (stage 2).
    pub rolle: Option<Rolle>,
    /// The account (uid under Unix, SID under Windows).
    pub konto: String,
    /// A deputy acts for the person it names (Windows hub only).
    pub vertreter: bool,
}

/// A model target as far as the checks need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZielRegel {
    /// Name of the secret (`modell.*` or `anmeldung.*`).
    pub schluessel: String,
    /// Rules `<METHODE> <pfad>`.
    pub pfade: Vec<String>,
}

/// Class of a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Klasse {
    /// Reads only.
    Lesend,
    /// Writes; also every tool without a class.
    Schreibend,
}

/// An MCP server as far as the checks need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRegel {
    /// Name of the secret (`mcp.*`).
    pub schluessel: String,
    /// Option "release per write call" (default on).
    pub freigabe: bool,
    /// The class of each known tool.
    pub werkzeuge: BTreeMap<String, Klasse>,
}

/// One secret held by the holder: name and owner, never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eintrag {
    /// Name of the secret.
    pub name: String,
    /// Account of the owner, `None` in the area `maschine`.
    pub konto: Option<String>,
    /// Pseudonym of the person, `None` where none is known.
    pub person: Option<String>,
}

/// What the holder knows at the time of one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sicht {
    /// The caller of this connection.
    pub aufrufer: Aufrufer,
    /// The own clock, ms since 1970.
    pub jetzt_ms: i64,
    /// Time of the start (end of step 4).
    pub start_ms: i64,
    /// Public keys of the decision point per `kid`.
    pub kids: BTreeMap<String, [u8; 32]>,
    /// The redeemed `einmal` of all connections.
    pub benutzt: BTreeSet<String>,
    /// Result of the check of the human proof (OPEN 1).
    pub nachweis_gueltig: bool,
    /// Model targets of `halter.json`.
    pub ziele: BTreeMap<String, ZielRegel>,
    /// MCP servers of `halter.json`.
    pub server: BTreeMap<String, ServerRegel>,
    /// The secrets held.
    pub geheimnisse: Vec<Eintrag>,
    /// The `id` of the open stream of this connection.
    pub strom: Option<Value>,
}

/// A request that passed stages 1 to 27.
#[derive(Debug, Clone, PartialEq)]
pub struct Geprueft {
    /// The op.
    pub op: Op,
    /// The request object.
    pub anfrage: Map<String, Value>,
    /// The `id`.
    pub id: Value,
    /// For a write call with a ticket: `einmal` and `ablauf` to redeem before the call is sent.
    pub einloesen: Option<(String, i64)>,
    /// For `mcp.aufruf`: the own `digest`.
    pub digest: Option<String>,
}

/// A refused ticket, for the protocol line (section 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheinAbgelehnt {
    /// The own `digest`.
    pub digest: String,
    /// `lauf` of the request.
    pub lauf: String,
    /// `server` of the request.
    pub server: String,
    /// `werkzeug` of the request.
    pub werkzeug: String,
}

/// A request that failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Abgelehnt {
    /// The code of the first violated stage.
    pub code: Code,
    /// The `id` if the line was an object with a valid `id`, else `None` (`null` in the answer).
    pub id: Option<Value>,
    /// Set if a ticket was refused in stages 18 to 26.
    pub schein: Option<ScheinAbgelehnt>,
    /// Set if a path was refused in stage 15 (`halter.pfad_verboten`), for the chain line of section 9.
    pub pfad: Option<PfadAbgelehnt>,
}

/// What the chain line of a refused path carries (section 9): the op, the target, the method and the path of the
/// request, never the query, a head or a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PfadAbgelehnt {
    /// `modell.aufruf` or `modell.strom`.
    pub op: String,
    /// `ziel` of the request (a target in `halter.json`).
    pub ziel: String,
    /// `GET` or `POST` (`GET` for `modell.strom`).
    pub methode: String,
    /// `pfad` of the request, without the query.
    pub pfad: String,
}

fn ab(code: Code, id: Option<&Value>) -> Abgelehnt {
    Abgelehnt {
        code,
        id: id.cloned(),
        schein: None,
        pfad: None,
    }
}

/// `digest` = sha256 hex over the RFC 8785 bytes of `{argumente, person, server, vorschau_sha256, werkzeug}`.
#[must_use]
pub fn digest(o: &Map<String, Value>) -> Option<String> {
    let teil = json!({
        "argumente": o.get("argumente")?,
        "person": o.get("person")?,
        "server": o.get("server")?,
        "vorschau_sha256": o.get("vorschau_sha256")?,
        "werkzeug": o.get("werkzeug")?,
    });
    json::kanonisch(&teil).map(|t| formate::kette::sha256_hex(t.as_bytes()))
}

/// The path check before stage 15: `//`, a segment `.` or `..`, `%2e`, `%2f`, `%5c` in any case, `\`.
#[must_use]
pub fn pfad_unsicher(pfad: &str) -> bool {
    let klein = pfad.to_ascii_lowercase();
    pfad.contains("//")
        || pfad.contains('\\')
        || ["%2e", "%2f", "%5c"].iter().any(|k| klein.contains(k))
        || pfad.split('/').skip(1).any(|s| s == "." || s == "..")
}

/// A rule `<METHODE> <pfad>`: `/*` matches every path, `/x/*` the subtree `/x/...`, else exactly.
#[must_use]
pub fn regel_passt(regel: &str, methode: &str, pfad: &str) -> bool {
    let Some((r_methode, r_pfad)) = regel.split_once(' ') else {
        return false;
    };
    if r_methode != methode {
        return false;
    }
    if r_pfad == "/*" {
        return true;
    }
    if let Some(stamm) = r_pfad.strip_suffix('*')
        && stamm.ends_with('/')
    {
        return pfad.starts_with(stamm);
    }
    pfad == r_pfad
}

fn s<'a>(o: &'a Map<String, Value>, k: &str) -> &'a str {
    o.get(k).and_then(Value::as_str).unwrap_or("")
}

fn i(o: &Map<String, Value>, k: &str) -> i64 {
    o.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// The model secret of a target: `modell.*` in the area `maschine` (no account), `anmeldung.*` of the caller's account.
fn modell_geheimnis(sicht: &Sicht, name: &str) -> bool {
    let konto = if name.starts_with("modell.") {
        None
    } else {
        Some(sicht.aufrufer.konto.as_str())
    };
    sicht
        .geheimnisse
        .iter()
        .any(|g| g.name == name && g.konto.as_deref() == konto)
}

/// Stages 19 to 26.
fn schein_stufen(
    o: &Map<String, Value>,
    sicht: &Sicht,
    eigen: &str,
) -> Result<(String, i64), Code> {
    let schein = o
        .get("schein")
        .and_then(Value::as_object)
        .ok_or(Code::Intern)?;
    let daten = schein
        .get("daten")
        .and_then(Value::as_object)
        .ok_or(Code::Intern)?;
    let oeff = sicht.kids.get(s(daten, "kid")).ok_or(Code::ScheinKid)?;
    let sig = b64::dekodieren(s(schein, "signatur")).ok_or(Code::ScheinSignatur)?;
    let sig: [u8; 64] = sig.try_into().map_err(|_| Code::ScheinSignatur)?;
    let bytes = json::kanonisch(&Value::Object(daten.clone())).ok_or(Code::ScheinSignatur)?;
    if !ed25519::pruefen(oeff, bytes.as_bytes(), &sig) {
        return Err(Code::ScheinSignatur);
    }
    if s(daten, "art") != "mcp-schein" || daten.get("fassung").and_then(Value::as_u64) != Some(1) {
        return Err(Code::ScheinArt);
    }
    if s(daten, "lauf") != s(o, "lauf") {
        return Err(Code::ScheinLauf);
    }
    if s(daten, "digest") != eigen {
        return Err(Code::ScheinDigest);
    }
    let (ausgestellt, ablauf) = (i(daten, "ausgestellt"), i(daten, "ablauf"));
    let spanne = ablauf - ausgestellt;
    if !(1..=SCHEIN_SPANNE_MAX).contains(&spanne) || ausgestellt > sicht.jetzt_ms + SCHEIN_VORLAUF {
        return Err(Code::ScheinZeit);
    }
    if ausgestellt < sicht.start_ms || sicht.jetzt_ms >= ablauf {
        return Err(Code::ScheinAbgelaufen);
    }
    let einmal = s(daten, "einmal");
    if sicht.benutzt.contains(einmal) {
        return Err(Code::ScheinBenutzt);
    }
    Ok((einmal.to_owned(), ablauf))
}

/// Stages 13 to 27 for a request in its closed form.
fn nach_form(
    op: Op,
    o: &Map<String, Value>,
    sicht: &Sicht,
) -> Result<Geprueft, (Code, Option<ScheinAbgelehnt>)> {
    let id = o.get("id").cloned().unwrap_or(Value::Null);
    let fertig = |einloesen, digest| Geprueft {
        op,
        anfrage: o.clone(),
        id: id.clone(),
        einloesen,
        digest,
    };
    match op {
        Op::Setzen | Op::Loeschen => {
            if s(o, "name").starts_with("modell.") {
                return Err((Code::Bereich, None));
            }
            if !sicht.nachweis_gueltig {
                return Err((Code::Nachweis, None));
            }
            Ok(fertig(None, None))
        }
        Op::Vorhanden => match o.get("ziel") {
            Some(z) if !sicht.ziele.contains_key(z.as_str().unwrap_or("")) => {
                Err((Code::ZielUnbekannt, None))
            }
            _ => Ok(fertig(None, None)),
        },
        Op::StromTeil | Op::StromEnde => Ok(fertig(None, None)),
        Op::ModellAufruf | Op::ModellStrom | Op::ModellSuche => {
            let ziel = sicht
                .ziele
                .get(s(o, "ziel"))
                .ok_or((Code::ZielUnbekannt, None))?;
            if op != Op::ModellSuche {
                let methode = if op == Op::ModellAufruf {
                    s(o, "methode")
                } else {
                    "GET"
                };
                let pfad = s(o, "pfad");
                if pfad_unsicher(pfad) || !ziel.pfade.iter().any(|r| regel_passt(r, methode, pfad))
                {
                    return Err((Code::PfadVerboten, None));
                }
            }
            if !modell_geheimnis(sicht, &ziel.schluessel) {
                return Err((Code::KeinSchluessel, None));
            }
            Ok(fertig(None, None))
        }
        Op::McpAufruf => {
            let server = sicht
                .server
                .get(s(o, "server"))
                .ok_or((Code::ZielUnbekannt, None))?;
            let eintrag = sicht
                .geheimnisse
                .iter()
                .rev()
                .find(|g| {
                    g.name == server.schluessel && g.person.as_deref() == Some(s(o, "person"))
                })
                .ok_or((Code::KeinSchluessel, None))?;
            if eintrag.konto.as_deref() != Some(sicht.aufrufer.konto.as_str())
                && !sicht.aufrufer.vertreter
            {
                return Err((Code::Person, None));
            }
            let eigen = digest(o).ok_or((Code::Intern, None))?;
            let abgelehnt = || ScheinAbgelehnt {
                digest: eigen.clone(),
                lauf: s(o, "lauf").to_owned(),
                server: s(o, "server").to_owned(),
                werkzeug: s(o, "werkzeug").to_owned(),
            };
            let klasse = server
                .werkzeuge
                .get(s(o, "werkzeug"))
                .copied()
                .unwrap_or(Klasse::Schreibend);
            let braucht = klasse == Klasse::Schreibend && server.freigabe;
            let hat = o.contains_key("schein");
            if braucht && !hat {
                return Err((Code::ScheinFehlt, Some(abgelehnt())));
            }
            if !braucht && hat {
                return Err((Code::ScheinUnerwartet, Some(abgelehnt())));
            }
            if !braucht {
                return Ok(fertig(None, Some(eigen)));
            }
            match schein_stufen(o, sicht, &eigen) {
                Ok(e) => Ok(fertig(Some(e), Some(eigen))),
                Err(c) => Err((c, Some(abgelehnt()))),
            }
        }
    }
}

/// Stages 1 to 27 for the bytes of one line (without the LF).
///
/// # Errors
/// [`Abgelehnt`] with the code of the first violated stage.
#[allow(
    clippy::result_large_err,
    reason = "one refusal per line; it is answered at once"
)]
pub fn pruefen(zeile: &[u8], sicht: &Sicht) -> Result<Geprueft, Abgelehnt> {
    if zeile.len() > ZEILE_MAX {
        return Err(ab(Code::ZuGross, None));
    }
    let Some(rolle) = sicht.aufrufer.rolle else {
        return Err(ab(Code::Peer, None));
    };
    let Some(wert) = json::streng_lesen(zeile) else {
        return Err(ab(Code::JsonUngueltig, None));
    };
    let Value::Object(o) = &wert else {
        return Err(ab(Code::JsonUngueltig, None));
    };
    let id = o.get("id").filter(|v| form::id_gueltig(v));
    if !json::ist_kanonisch(&wert, zeile) {
        return Err(ab(Code::NichtKanonisch, id));
    }
    if o.get("fassung").and_then(Value::as_u64) != Some(1) || !o["fassung"].is_u64() {
        return Err(ab(Code::FassungUnbekannt, id));
    }
    let Some(op) = o.get("op").and_then(Value::as_str).and_then(Op::aus_text) else {
        return Err(ab(Code::OpUnbekannt, id));
    };
    // `schluessel.vorhanden` by `ziel` is for the role `bruecke` only (section 4)
    if !rolle.erlaubt(op)
        || (op == Op::Vorhanden && o.contains_key("ziel") && rolle != Rolle::Bruecke)
    {
        return Err(ab(Code::Rolle, id));
    }
    match (&sicht.strom, op) {
        (Some(offen), Op::StromTeil | Op::StromEnde) if o.get("id") == Some(offen) => {}
        (None, Op::StromTeil | Op::StromEnde) | (Some(_), _) => return Err(ab(Code::Strom, id)),
        (None, _) => {}
    }
    form::pruefen(o, op).map_err(|c| ab(c, id))?;
    nach_form(op, o, sicht).map_err(|(code, schein)| Abgelehnt {
        code,
        id: id.cloned(),
        schein,
        pfad: (code == Code::PfadVerboten).then(|| PfadAbgelehnt {
            op: s(o, "op").to_owned(),
            ziel: s(o, "ziel").to_owned(),
            methode: if op == Op::ModellStrom {
                "GET"
            } else {
                s(o, "methode")
            }
            .to_owned(),
            pfad: s(o, "pfad").to_owned(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regeln() {
        assert!(regel_passt("GET /*", "GET", "/x/y"));
        assert!(regel_passt(
            "GET /copilot_internal/*",
            "GET",
            "/copilot_internal/user"
        ));
        assert!(!regel_passt(
            "GET /copilot_internal/*",
            "GET",
            "/copilot_internalx"
        ));
        assert!(!regel_passt(
            "GET /copilot_internal/*",
            "GET",
            "/copilot_internal"
        ));
        assert!(!regel_passt("POST /a", "GET", "/a"));
        assert!(!regel_passt("POST /a", "POST", "/a/b"));
        assert!(regel_passt("POST /a", "POST", "/a"));
        assert!(!regel_passt("POST/a", "POST", "/a"));
    }

    #[test]
    fn unsichere_pfade() {
        for p in [
            "//a", "/a//b", "/a/./b", "/a/..", "/a/%2E", "/a%2f", "/%5C", "/a\\b",
        ] {
            assert!(pfad_unsicher(p), "{p}");
        }
        for p in ["/", "/a/b", "/a/...", "/a.b"] {
            assert!(!pfad_unsicher(p), "{p}");
        }
    }
}
