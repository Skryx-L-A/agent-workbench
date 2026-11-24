//! Tokens of an answer (contract 8, section 10, mode `openai-lokal`): only `usage.prompt_tokens` and
//! `usage.completion_tokens`, from the JSON body or from the last SSE event with `usage`; else both null.

use serde_json::Value;

use crate::json::lies_streng;

/// Largest token count of the line.
const U32: u64 = 4_294_967_295;
/// Largest body kept to read it as one JSON value (the largest body of a request).
const JSON_MAX: usize = 33_554_432;
/// Longest SSE line read for `usage`; a longer line is skipped.
const ZEILE_MAX: usize = 1_048_576;

/// Counts the tokens of one answer while it passes, part by part.
pub(crate) struct Zaehler {
    ganz: Option<Vec<u8>>,
    zeile: Option<Vec<u8>>,
    letzte: Option<Value>,
}

impl Zaehler {
    pub(crate) fn neu() -> Self {
        Self {
            ganz: Some(Vec::new()),
            zeile: Some(Vec::new()),
            letzte: None,
        }
    }

    /// The next part of the body as it came from the target.
    pub(crate) fn teil(&mut self, teil: &[u8]) {
        if let Some(ganz) = &mut self.ganz {
            if ganz.len() + teil.len() > JSON_MAX {
                self.ganz = None;
            } else {
                ganz.extend_from_slice(teil);
            }
        }
        let mut stuecke = teil.split(|c| *c == b'\n');
        if let Some(erstes) = stuecke.next() {
            self.zeile_anhaengen(erstes);
        }
        for stueck in stuecke {
            self.zeile_fertig();
            self.zeile_anhaengen(stueck);
        }
    }

    fn zeile_anhaengen(&mut self, bytes: &[u8]) {
        if let Some(zeile) = &mut self.zeile {
            if zeile.len() + bytes.len() > ZEILE_MAX {
                self.zeile = None;
            } else {
                zeile.extend_from_slice(bytes);
            }
        }
    }

    fn zeile_fertig(&mut self) {
        if let Some(zeile) = self.zeile.replace(Vec::new())
            && let Some(usage) = sse_usage(&zeile)
        {
            self.letzte = Some(usage);
        }
    }

    /// `(token_ein, token_aus)` at the end of the answer, or `None`.
    pub(crate) fn ende(mut self) -> Option<(u64, u64)> {
        let ganz = self.ganz.take().and_then(|b| lies_streng(&b));
        let usage = if let Some(Value::Object(mut o)) = ganz {
            o.remove("usage")
        } else {
            self.zeile_fertig();
            self.letzte
        };
        let Some(Value::Object(usage)) = usage else {
            return None;
        };
        let zahl = |feld: &str| {
            usage
                .get(feld)
                .and_then(Value::as_u64)
                .filter(|n| *n <= U32)
        };
        Some((zahl("prompt_tokens")?, zahl("completion_tokens")?))
    }
}

/// The `usage` object of an SSE line `data: {…}` (with or without CR at its end); `data: [DONE]` and others have none.
fn sse_usage(zeile: &[u8]) -> Option<Value> {
    let daten = zeile.strip_prefix(b"data: ")?;
    let daten = daten.strip_suffix(b"\r").unwrap_or(daten);
    match lies_streng(daten)? {
        Value::Object(mut o) => o.remove("usage").filter(Value::is_object),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::Zaehler;

    fn zaehle(teile: &[&[u8]]) -> Option<(u64, u64)> {
        let mut z = Zaehler::neu();
        for t in teile {
            z.teil(t);
        }
        z.ende()
    }

    #[test]
    fn tokens_from_a_json_body() {
        assert_eq!(
            zaehle(&[
                br#"{"usage":{"prompt_tokens":9,"#,
                br#""completion_tokens":3}}"#
            ]),
            Some((9, 3))
        );
        assert_eq!(zaehle(&[br#"{"usage":{"prompt_tokens":9}}"#]), None);
        assert_eq!(
            zaehle(&[br#"{"usage":{"prompt_tokens":9,"completion_tokens":3.0}}"#]),
            None
        );
        assert_eq!(
            zaehle(&[br#"{"usage":{"prompt_tokens":-1,"completion_tokens":3}}"#]),
            None
        );
        assert_eq!(
            zaehle(&[br#"{"usage":{"prompt_tokens":4294967295,"completion_tokens":0}}"#]),
            Some((4_294_967_295, 0))
        );
        assert_eq!(
            zaehle(&[br#"{"usage":{"prompt_tokens":4294967296,"completion_tokens":0}}"#]),
            None
        );
        assert_eq!(zaehle(&[br#"{"usage":[1]}"#]), None);
        assert_eq!(zaehle(&[br#"{"x":1}"#]), None);
    }

    #[test]
    fn a_json_body_of_the_largest_size_is_still_read() {
        let mut koerper = br#"{"usage":{"prompt_tokens":1,"completion_tokens":2},"x":""#.to_vec();
        koerper.resize(33_554_432 - 2, b'a');
        koerper.extend_from_slice(b"\"}");
        assert_eq!(zaehle(&[&koerper[..10], &koerper[10..]]), Some((1, 2)));
        koerper.insert(70, b'a');
        assert_eq!(zaehle(&[&koerper]), None);
    }

    #[test]
    fn tokens_from_the_last_sse_event_with_usage() {
        let strom: &[&[u8]] = &[
            b"data: {\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2}}\n\ndata: {\"usa",
            b"ge\":{\"prompt_tokens\":11,\"completion_tokens\":4}}\r\n\ndata: {\"usage\":null}\n",
            b"data: {\"choices\":[]}\n\ndata: [DONE]\n\n",
        ];
        assert_eq!(zaehle(strom), Some((11, 4)));
        assert_eq!(
            zaehle(&[b"data: {\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2}}"]),
            Some((1, 2))
        );
        assert_eq!(
            zaehle(&[b"data:{\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2}}\n"]),
            None
        );
        assert_eq!(zaehle(&[b"data: [DONE]\n"]), None);
    }

    #[test]
    fn a_very_long_line_is_skipped() {
        let mut lang =
            b"data: {\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2},\"x\":\"".to_vec();
        lang.resize(1_048_576, b'a');
        lang.extend_from_slice(b"\"}\n");
        assert_eq!(zaehle(&[&lang]), None);
        lang.truncate(1_048_574);
        lang.extend_from_slice(b"\"}\n");
        assert_eq!(zaehle(&[&lang[..1000], &lang[1000..]]), Some((1, 2)));
    }
}
