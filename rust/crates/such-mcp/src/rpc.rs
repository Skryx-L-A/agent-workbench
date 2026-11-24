//! JSON-RPC 2.0 and MCP: one input line in, at most one output line out.

use crate::suche::Befrager;
use serde_json::{Map, Value};

/// The name of the one tool.
pub const WERKZEUG: &str = "brain_suche";

/// The newest version of the protocol; the answer for a client with an unknown version.
pub const FASSUNG_NEU: &str = "2025-11-25";

/// The versions of the protocol that `initialize` answers with the client's own version.
pub const FASSUNGEN: [&str; 2] = [FASSUNG_NEU, "2025-06-18"];

/// The text of a tool result when the arguments are invalid. The socket is not asked then.
pub const TEXT_UNGUELTIG: &str = "Ungültige Anfrage";

/// The text of a tool result when the socket gives no usable answer.
pub const TEXT_NICHT_ERREICHBAR: &str = "Suche nicht erreichbar";

/// The most `k` a search may ask for.
pub const K_MAX: i64 = 50;

const SCHEMA: &str = r#"{"type":"object","properties":{"query":{"type":"string"},"k":{"type":"integer","minimum":1,"maximum":50}},"required":["query"],"additionalProperties":false}"#;

const BESCHREIBUNG: &str = "Durchsucht die Wissensnotizen dieser Sitzung. Gibt eine JSON-Zeile mit fallback, hits und gesperrt zurück. \
gesperrt ist die Zahl der Treffer, die diese Sitzung nicht sehen darf; sie werden nicht ausgegeben.";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// The answer line for one input line (without LF), or `None` when the message gets no answer.
///
/// `befrager` is asked only for a valid call of the tool.
pub fn antworte<B: Befrager>(zeile: &[u8], befrager: &B) -> Option<String> {
    // An empty line is skipped without an answer (contract 4, clarification 9).
    if zeile.is_empty() {
        return None;
    }
    let Ok(text) = std::str::from_utf8(zeile) else {
        return Some(fehler(&Value::Null, PARSE_ERROR, "Parse error"));
    };
    let Ok(wert) = serde_json::from_str::<Value>(text) else {
        return Some(fehler(&Value::Null, PARSE_ERROR, "Parse error"));
    };
    verarbeite(&wert, befrager)
}

/// The answer to a line that is too long: the line was dropped, the connection goes on.
#[must_use]
pub fn zu_lang() -> String {
    fehler(&Value::Null, INVALID_REQUEST, "Message too long")
}

fn verarbeite<B: Befrager>(wert: &Value, befrager: &B) -> Option<String> {
    let Value::Object(objekt) = wert else {
        return Some(fehler(&Value::Null, INVALID_REQUEST, "Invalid Request"));
    };
    let id = match objekt.get("id") {
        None => None,
        Some(id @ (Value::String(_) | Value::Number(_))) => Some(id),
        Some(_) => return Some(fehler(&Value::Null, INVALID_REQUEST, "Invalid Request")),
    };
    // An answer of the client to a request: this program sends no request, so there is nothing to match.
    if !objekt.contains_key("method")
        && (objekt.contains_key("result") || objekt.contains_key("error"))
    {
        return None;
    }
    let methode = match (objekt.get("jsonrpc"), objekt.get("method")) {
        (Some(Value::String(version)), Some(Value::String(methode))) if version == "2.0" => {
            methode.as_str()
        }
        _ => {
            return Some(fehler(
                id.unwrap_or(&Value::Null),
                INVALID_REQUEST,
                "Invalid Request",
            ));
        }
    };
    // A message without id is a notification: it never gets an answer, not even an error.
    let id = id?;
    Some(match methode {
        "initialize" => initialisiere(id, objekt.get("params")),
        "ping" => ergebnis(id, "{}"),
        "tools/list" => ergebnis(id, &werkzeugliste()),
        "tools/call" => rufe(id, objekt.get("params"), befrager),
        _ => fehler(id, METHOD_NOT_FOUND, "Method not found"),
    })
}

fn initialisiere(id: &Value, params: Option<&Value>) -> String {
    let Some(Value::String(gewuenscht)) = params.and_then(|p| p.get("protocolVersion")) else {
        return fehler(id, INVALID_PARAMS, "Invalid params");
    };
    let fassung = FASSUNGEN
        .iter()
        .copied()
        .find(|f| f == gewuenscht)
        .unwrap_or(FASSUNG_NEU);
    let name = Value::String("such-mcp".to_owned());
    let version = Value::String(env!("CARGO_PKG_VERSION").to_owned());
    ergebnis(
        id,
        &format!(
            r#"{{"protocolVersion":{},"capabilities":{{"tools":{{}}}},"serverInfo":{{"name":{name},"version":{version}}}}}"#,
            Value::String(fassung.to_owned()),
        ),
    )
}

fn werkzeugliste() -> String {
    format!(
        r#"{{"tools":[{{"name":{},"description":{},"inputSchema":{SCHEMA}}}]}}"#,
        Value::String(WERKZEUG.to_owned()),
        Value::String(BESCHREIBUNG.to_owned()),
    )
}

fn rufe<B: Befrager>(id: &Value, params: Option<&Value>, befrager: &B) -> String {
    let Some(Value::Object(params)) = params else {
        return fehler(id, INVALID_PARAMS, "Invalid params");
    };
    if params.get("name").and_then(Value::as_str) != Some(WERKZEUG) {
        return fehler(id, INVALID_PARAMS, "Unknown tool");
    }
    let Some(anfrage) = anfrage_aus(params.get("arguments")) else {
        return werkzeug_ergebnis(id, TEXT_UNGUELTIG, true);
    };
    match befrager.frage(&anfrage) {
        Ok(antwort) => {
            // The search always answers with a JSON object. An answer that is no JSON object (no JSON, a list, a number, a
            // string) comes from a search that is out of order (fail closed, decision of 23.11.2026), as does an object with a
            // field `error`. Only an object without `error` is no error.
            let fehlerhaft = match serde_json::from_str::<Value>(&antwort) {
                Ok(Value::Object(o)) => o.contains_key("error"),
                _ => true,
            };
            werkzeug_ergebnis(id, &antwort, fehlerhaft)
        }
        Err(_) => werkzeug_ergebnis(id, TEXT_NICHT_ERREICHBAR, true),
    }
}

/// The request line for the socket, or `None` when the arguments are invalid.
///
/// Valid are an object with a text `query` and, at most, a whole number `k` from 1 to 50; every other field makes it
/// invalid, `route` included. The line is built from the two checked values only: `{"op":"search","query":…}` with
/// `k` only when it was given.
fn anfrage_aus(argumente: Option<&Value>) -> Option<String> {
    let Value::Object(argumente) = argumente? else {
        return None;
    };
    if argumente.keys().any(|name| name != "query" && name != "k") {
        return None;
    }
    let query = argumente.get("query")?.as_str()?;
    let k = match argumente.get("k") {
        None => String::new(),
        Some(k) => {
            let k = k.as_i64().filter(|k| (1..=K_MAX).contains(k))?;
            format!(r#","k":{k}"#)
        }
    };
    Some(format!(
        r#"{{"op":"search","query":{}{k}}}"#,
        Value::String(query.to_owned())
    ))
}

fn werkzeug_ergebnis(id: &Value, text: &str, ist_fehler: bool) -> String {
    ergebnis(
        id,
        &format!(
            r#"{{"content":[{{"type":"text","text":{}}}],"isError":{ist_fehler}}}"#,
            Value::String(text.to_owned()),
        ),
    )
}

fn ergebnis(id: &Value, ergebnis: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{ergebnis}}}"#)
}

fn fehler(id: &Value, code: i64, meldung: &str) -> String {
    let mut fehler = Map::new();
    fehler.insert("code".to_owned(), Value::from(code));
    fehler.insert("message".to_owned(), Value::String(meldung.to_owned()));
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"error":{}}}"#,
        Value::Object(fehler)
    )
}
