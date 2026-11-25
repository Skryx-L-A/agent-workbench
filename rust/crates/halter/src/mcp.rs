//! `mcp.aufruf` to a remote server (section 4): one JSON-RPC 2.0 request `tools/call` over HTTP POST with the token
//! of the person in the head named in `halter.json`. The answer is the result object, or the number of a JSON-RPC
//! error without the text of the server. An answer as `text/event-stream` is read up to the first event with a JSON
//! value.

use serde_json::{Map, Value, json};

use crate::code::Code;
use crate::geheim::Geheim;
use crate::json;
use crate::konfig::Server;
use crate::leck;
use crate::ziel::{self, FRIST_KOPF, Koerper};

/// Largest answer of a server in bytes.
pub const ANTWORT_MAX: usize = 33_554_432;

/// The outcome of a call.
#[derive(Debug, Clone, PartialEq)]
pub enum Ausgang {
    /// The result object.
    Ergebnis(Map<String, Value>),
    /// A JSON-RPC error with its number.
    Fehler(i64),
}

fn aus_text(text: &str, sse: bool) -> Option<Value> {
    if !sse {
        return json::streng_lesen(text.as_bytes());
    }
    let mut daten = String::new();
    for zeile in text.split('\n') {
        let zeile = zeile.strip_suffix('\r').unwrap_or(zeile);
        if let Some(d) = zeile.strip_prefix("data:") {
            daten.push_str(d.strip_prefix(' ').unwrap_or(d));
            daten.push('\n');
        } else if zeile.is_empty() && !daten.is_empty() {
            if let Some(w) = json::streng_lesen(daten.trim_end().as_bytes()) {
                return Some(w);
            }
            daten.clear();
        }
    }
    json::streng_lesen(daten.trim_end().as_bytes())
}

/// The outcome of the body of an answer.
#[must_use]
pub fn auswerten(text: &str, sse: bool) -> Option<Ausgang> {
    let w = aus_text(text, sse)?;
    let o = w.as_object()?;
    if o.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return None;
    }
    if let Some(e) = o.get("error") {
        let n = e.get("code")?.as_i64()?;
        return Some(Ausgang::Fehler(
            n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)),
        ));
    }
    o.get("result")?.as_object().cloned().map(Ausgang::Ergebnis)
}

/// Sends the call and reads the answer. `werte` are all secrets held, for the leak lock over the result.
///
/// # Errors
/// `halter.ziel_fehler`, `halter.ziel_komprimiert` or `halter.leck_gesperrt`.
pub fn aufrufen(
    s: &Server,
    werkzeug: &str,
    argumente: &Value,
    token: &Geheim,
    werte: &[Geheim],
) -> Result<Ausgang, Code> {
    let koerper = json::kanonisch(&json!({
        "id": 1,
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {"arguments": argumente, "name": werkzeug},
    }))
    .ok_or(Code::Intern)?;
    let mut kopf = Map::new();
    kopf.insert(
        "content-type".into(),
        Value::String("application/json".into()),
    );
    kopf.insert(
        "accept".into(),
        Value::String("application/json, text/event-stream".into()),
    );
    let anfrage = ziel::anfrage(
        "POST",
        &s.pfad,
        &s.adresse,
        &kopf,
        token.bytes(),
        Some(koerper.as_bytes()),
        false,
    );
    let mut l = ziel::verbinden(&s.adresse)?;
    l.tcp()
        .set_write_timeout(Some(FRIST_KOPF))
        .map_err(|_| Code::ZielFehler)?;
    std::io::Write::write_all(&mut l, &anfrage).map_err(|_| Code::ZielFehler)?;
    drop(anfrage);
    let (k, vorrat) = ziel::kopf_lesen(l.as_mut(), FRIST_KOPF)?;
    if k.komprimiert() {
        return Err(Code::ZielKomprimiert);
    }
    if !(200..300).contains(&k.status) {
        return Err(Code::ZielFehler);
    }
    let sse = k
        .wert("content-type")
        .is_some_and(|t| t.to_ascii_lowercase().starts_with("text/event-stream"));
    let mut koerper = Koerper::neu(l.as_mut(), &k, vorrat, "POST")?;
    let mut alles = Vec::new();
    while let Some(b) = koerper.naechstes()? {
        alles.extend_from_slice(&b);
        if alles.len() > ANTWORT_MAX {
            return Err(Code::ZielFehler);
        }
    }
    let text = String::from_utf8(alles).map_err(|_| Code::ZielFehler)?;
    let ausgang = auswerten(&text, sse).ok_or(Code::ZielFehler)?;
    if let Ausgang::Ergebnis(e) = &ausgang {
        let bytes = json::kanonisch(&Value::Object(e.clone())).ok_or(Code::ZielFehler)?;
        if leck::trifft(werte, bytes.as_bytes()) {
            return Err(Code::LeckGesperrt);
        }
    }
    Ok(ausgang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antworten_lesen() {
        let e = auswerten(r#"{"id":1,"jsonrpc":"2.0","result":{"content":[]}}"#, false);
        assert!(matches!(e, Some(Ausgang::Ergebnis(_))));
        let f = auswerten(
            r#"{"error":{"code":-32603,"message":"token abc"},"id":1,"jsonrpc":"2.0"}"#,
            false,
        );
        assert_eq!(f, Some(Ausgang::Fehler(-32603)));
        let s = auswerten(
            "event: message\ndata: {\"id\":1,\"jsonrpc\":\"2.0\",\"result\":{}}\n\n",
            true,
        );
        assert!(matches!(s, Some(Ausgang::Ergebnis(_))));
        assert_eq!(auswerten(r#"{"id":1,"result":{}}"#, false), None);
        assert_eq!(auswerten(r#"{"jsonrpc":"2.0","result":[1]}"#, false), None);
    }
}
