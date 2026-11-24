//! The request of the search socket (contract 4, section "Transport und Anfragen").
//!
//! A request has the fields `op`, `id`, `query`, `k` and `space`, and no other. The route, the customer, the session and the
//! class are never fields of a request: they come from the start of the process.

use serde_json::{Map, Value};

use crate::serve::{Fields, Invalid, is_id, k_of, read_fields, text_or_null};

/// The fields a request may have.
const FIELDS: [&str; 5] = ["op", "id", "query", "k", "space"];
/// The number of hits when a request gives no `k`.
const K_DEFAULT: usize = 5;
/// The largest `k`.
const K_MAX: usize = 50;

/// What a valid request asks for.
#[derive(Debug, PartialEq)]
pub enum Op {
    /// `ping`.
    Ping,
    /// `search`.
    Search {
        /// The question.
        query: String,
        /// The number of hits, 1 to 50.
        k: usize,
        /// A space or a comma list of spaces, as in `rangkern serve`.
        space: Option<String>,
    },
}

/// A valid request.
#[derive(Debug, PartialEq)]
pub struct Anfrage {
    /// The `id`, to be returned unchanged.
    pub id: Option<Value>,
    /// The operation.
    pub op: Op,
}

/// The operation of the fields of a line that has no repeated and no unknown key.
fn op_von(fields: &Map<String, Value>, k_text: Option<&str>) -> Result<Op, Invalid> {
    if fields.get("id").is_some_and(|id| !is_id(id)) {
        return Err(Invalid);
    }
    let op = fields.get("op").and_then(Value::as_str).ok_or(Invalid)?;
    match op {
        "ping" if fields.keys().all(|key| key == "op" || key == "id") => Ok(Op::Ping),
        "search" => {
            let k = match k_text {
                None => K_DEFAULT,
                Some(text) => k_of(text).filter(|k| *k <= K_MAX).ok_or(Invalid)?,
            };
            let query = fields.get("query").and_then(Value::as_str).ok_or(Invalid)?;
            let space = fields.get("space").map_or(Ok(None), text_or_null)?;
            Ok(Op::Search {
                query: query.to_owned(),
                k,
                space,
            })
        }
        _ => Err(Invalid),
    }
}

/// Reads one line as a request. `Err` carries the `id` for the answer `bad_request`: the `id` of the line if it is a JSON
/// object with exactly one valid `id`.
///
/// # Errors
///
/// The `id` (if any) of a line that is no valid request. `quit` is not valid: the starter ends the session, not the agent.
pub fn lesen(zeile: &[u8]) -> Result<Anfrage, Option<Value>> {
    let Fields { id, values, k_text } = read_fields(zeile, &FIELDS)?;
    match op_von(&values, k_text.as_deref()) {
        Ok(op) => Ok(Anfrage { id, op }),
        Err(Invalid) => Err(id),
    }
}
