//! The diagnosis of the answer form of the first start (section 4, "Antwortform im Erststart"; decision of the lead,
//! 25.11.2026): for a path that `erststart_antwortform` of a target in `halter.json` names explicitly, the holder writes
//! one journal line with the field names of the JSON answer, never a value. A name with `token`, `secret`, `key`,
//! `session` or `credential` (any case, also as part of a word) marks the path as "stays closed" (`"zu":true`).
//!
//! Values (decision of the lead, 26.11.2026, finding of the Copilot start): the line also carries the values of exactly
//! two fields, `data[].id` of `GET /models` and `error.code` of an error answer (status 400 or above). A value stands in
//! the line only if it is not token-like ([`tokenaehnlich`], checked first) and fits `^[A-Za-z0-9._-]{1,64}$`
//! ([`wortform`]); every other value of the two fields only raises `wert_verworfen`. No other field gives a value.
//!
//! Without the switch there is no diagnosis, in every build. The answer itself goes on through the leak lock as before;
//! this module only reads a copy of the body.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::geheim::Geheim;
use crate::json;
use crate::leck;

/// Most bytes of a body the diagnosis reads; a longer body gives the form `zu_gross` and no names.
pub const KOERPER_MAX: usize = 1_048_576;
/// Most names in one line; more mark the line `abgeschnitten`.
pub const NAMEN_MAX: usize = 256;
/// Most bytes of one name in path form; a longer name is left out and marks the line `abgeschnitten`.
pub const NAME_MAX: usize = 256;
/// Deepest level of objects and lists the names are taken from; deeper levels mark the line `abgeschnitten`.
pub const TIEFE_MAX: usize = 32;
/// The words that keep a path closed (any case, also as part of a name).
pub const WOERTER: [&str; 5] = ["token", "secret", "key", "session", "credential"];
/// The start of the journal line.
pub const PRAEFIX: &str = "halter.antwortform: ";
/// Most model names (`data[].id`) in one line; more mark the line `abgeschnitten`, and no further name appears.
pub const WERTE_MAX: usize = 200;
/// Most bytes of one value in the line.
pub const WERT_MAX: usize = 64;
/// Starts of a token (any case): the forms of GitHub (`gho_`, `ghu_`, `ghp_`, `ghs_`, `ghr_`, `github_pat_`) and `sk-`.
pub const TOKEN_PRAEFIXE: [&str; 7] =
    ["gho_", "ghu_", "ghp_", "ghs_", "ghr_", "github_pat_", "sk-"];
/// Longest part between two of `.`, `_` and `-` in a value that is not token-like; a longer part looks like a token.
pub const TEIL_MAX: usize = 19;
/// The two fields whose values the line carries, in path form.
pub const WERT_FELDER: [&str; 2] = ["data[].id", "error.code"];

/// `true` if `wert` fits `^[A-Za-z0-9._-]{1,64}$` as a whole.
#[must_use]
pub fn wortform(wert: &str) -> bool {
    (1..=WERT_MAX).contains(&wert.len())
        && wert
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// `true` if `wert` looks like a token: a start of [`TOKEN_PRAEFIXE`] (any case), a word of [`WOERTER`], a part
/// between `.`, `_` and `-` longer than [`TEIL_MAX`], or a held secret in any form of the leak lock.
#[must_use]
pub fn tokenaehnlich(geheim: &[Geheim], wert: &str) -> bool {
    let klein = wert.to_ascii_lowercase();
    TOKEN_PRAEFIXE.iter().any(|p| klein.starts_with(p))
        || bleibt_zu(wert)
        || wert.split(['.', '_', '-']).any(|t| t.len() > TEIL_MAX)
        || leck::trifft(geheim, wert.as_bytes())
}

/// `true` only if `wert` may stand in the line: the token check first, then the word form. Every other case, also a
/// value that is no string, is `false`; there is no way that writes a value when a check fails.
#[must_use]
pub fn zulaessig(geheim: &[Geheim], wert: &Value) -> bool {
    let Some(w) = wert.as_str() else {
        return false;
    };
    if tokenaehnlich(geheim, w) {
        return false;
    }
    wortform(w)
}

/// The values of the two fields of one answer.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Werte {
    /// `data[].id` of `GET /models`, sorted, each once, at most [`WERTE_MAX`].
    pub ids: Vec<String>,
    /// `error.code` of an error answer.
    pub code: Option<String>,
    /// How many values of the two fields did not pass [`zulaessig`].
    pub verworfen: u64,
    /// `true` if more than [`WERTE_MAX`] model names passed.
    pub abgeschnitten: bool,
}

/// The values of `data[].id` (only if `modelle`) and of `error.code` (only if `fehler`) in `v`.
#[must_use]
pub fn werte(v: &Value, geheim: &[Geheim], modelle: bool, fehler: bool) -> Werte {
    let mut w = Werte::default();
    if modelle && let Some(liste) = v.get("data").and_then(Value::as_array) {
        let mut ids = BTreeSet::new();
        for id in liste.iter().filter_map(|e| e.as_object()?.get("id")) {
            match id.as_str() {
                Some(t) if zulaessig(geheim, id) => {
                    ids.insert(t.to_owned());
                }
                _ => w.verworfen += 1,
            }
        }
        w.abgeschnitten = ids.len() > WERTE_MAX;
        w.ids = ids.into_iter().take(WERTE_MAX).collect();
    }
    if fehler && let Some(code) = v.get("error").and_then(|e| e.as_object()?.get("code")) {
        match code.as_str() {
            Some(t) if zulaessig(geheim, code) => w.code = Some(t.to_owned()),
            _ => w.verworfen += 1,
        }
    }
    w
}

/// `true` if `name` holds one of [`WOERTER`], in any case.
#[must_use]
pub fn bleibt_zu(name: &str) -> bool {
    let klein = name.to_ascii_lowercase();
    WOERTER.iter().any(|w| klein.contains(w))
}

/// The names of a JSON value in path form: a key below the root is `k`, below `p` it is `p.k`; the elements of a list
/// below `p` stand under `p[]` (`data[].id`). Sorted, each once. `true` as second value if names were left out
/// (more than [`NAMEN_MAX`], a name longer than [`NAME_MAX`] or a level deeper than [`TIEFE_MAX`]).
#[must_use]
pub fn namen(wert: &Value) -> (Vec<String>, bool) {
    let mut alle = BTreeSet::new();
    let mut ab = false;
    sammeln(wert, "", 0, &mut alle, &mut ab);
    if alle.len() > NAMEN_MAX {
        ab = true;
    }
    (alle.into_iter().take(NAMEN_MAX).collect(), ab)
}

fn sammeln(wert: &Value, praefix: &str, tiefe: usize, alle: &mut BTreeSet<String>, ab: &mut bool) {
    let kinder = match wert {
        Value::Object(_) | Value::Array(_) if tiefe >= TIEFE_MAX => {
            *ab = true;
            return;
        }
        Value::Object(o) => o
            .iter()
            .map(|(k, v)| {
                let name = if praefix.is_empty() {
                    k.clone()
                } else {
                    format!("{praefix}.{k}")
                };
                (Some(name), v)
            })
            .collect::<Vec<_>>(),
        Value::Array(l) => l.iter().map(|v| (None, v)).collect(),
        _ => return,
    };
    for (name, v) in kinder {
        let unter = match name {
            Some(n) if n.len() > NAME_MAX => {
                *ab = true;
                continue;
            }
            Some(n) => {
                alle.insert(n.clone());
                n
            }
            None => format!("{praefix}[]"),
        };
        sammeln(v, &unter, tiefe + 1, alle, ab);
    }
}

/// The copy of the body of one answer, at most [`KOERPER_MAX`] bytes.
#[derive(Debug, Default)]
pub struct Sammler {
    daten: Vec<u8>,
    zu_gross: bool,
}

impl Sammler {
    /// An empty copy.
    #[must_use]
    pub fn neu() -> Self {
        Sammler::default()
    }

    /// Takes the next bytes of the body; above [`KOERPER_MAX`] the copy is dropped and the form is `zu_gross`.
    pub fn schieben(&mut self, b: &[u8]) {
        if self.zu_gross {
            return;
        }
        if self.daten.len() + b.len() > KOERPER_MAX {
            self.zu_gross = true;
            self.daten = Vec::new();
            return;
        }
        self.daten.extend_from_slice(b);
    }

    /// The journal line for the answer of `methode` `pfad` at `ziel` with `status`: [`PRAEFIX`] and the canonical JSON
    /// object with `abgeschnitten`, `form` (`json`, `kein_json`, `zu_gross` or `geheimnis`), `methode`, `namen`,
    /// `pfad`, `status`, `wert_verworfen`, `werte` (exactly `data[].id` and `error.code`), `ziel` and `zu`, then LF.
    /// `zu` is `true` for every form but `json`, for a cut list and for a name after [`bleibt_zu`]. A held secret in the
    /// names (in any form of the leak lock) gives the form `geheimnis`, no names and no values. Values only for the form
    /// `json`: `data[].id` only for `GET /models`, `error.code` only for a status of 400 or above.
    #[must_use]
    pub fn zeile(
        &self,
        werte: &[Geheim],
        ziel: &str,
        methode: &str,
        pfad: &str,
        status: u16,
    ) -> String {
        let (form, liste, ab, w) = if self.zu_gross {
            ("zu_gross", Vec::new(), false, Werte::default())
        } else {
            match serde_json::from_slice::<Value>(&self.daten) {
                Err(_) => ("kein_json", Vec::new(), false, Werte::default()),
                Ok(v) => {
                    let (n, ab) = namen(&v);
                    if leck::trifft(werte, n.join("\n").as_bytes()) {
                        ("geheimnis", Vec::new(), false, Werte::default())
                    } else {
                        let modelle = methode == "GET" && pfad == "/models";
                        let w = self::werte(&v, werte, modelle, status >= 400);
                        let ab = ab || w.abgeschnitten;
                        ("json", n, ab, w)
                    }
                }
            }
        };
        let zu = form != "json" || ab || liste.iter().any(|n| bleibt_zu(n));
        let wert = json!({
            "abgeschnitten": ab,
            "form": form,
            "methode": methode,
            "namen": liste,
            "pfad": pfad,
            "status": status,
            "wert_verworfen": w.verworfen,
            "werte": {
                "data[].id": w.ids,
                "error.code": w.code,
            },
            "ziel": ziel,
            "zu": zu,
        });
        let text = json::kanonisch(&wert).unwrap_or_default();
        format!("{PRAEFIX}{text}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKE: &str = "hgx7-markenwert-4711";

    fn zeile_von(body: &[u8], werte: &[Geheim]) -> Value {
        let mut s = Sammler::neu();
        s.schieben(body);
        let z = s.zeile(werte, "copilot", "POST", "/auto", 200);
        assert!(z.ends_with('\n') && z.matches('\n').count() == 1, "{z}");
        let json = z.strip_prefix(PRAEFIX).expect("prefix");
        serde_json::from_str(json.trim_end()).expect("line is JSON")
    }

    /// Target 4 of the order: `{"data":[{"id":..}],"access_token":..}` gives `data`, `data[].id` and `access_token` and the
    /// mark; the values stand nowhere in the line (counter check with a mark value).
    #[test]
    fn namen_ohne_werte_und_die_markierung() {
        let body = format!(r#"{{"data":[{{"id":"{MARKE}"}}],"access_token":"{MARKE}"}}"#);
        let z = zeile_von(body.as_bytes(), &[]);
        assert_eq!(z["namen"], json!(["access_token", "data", "data[].id"]));
        assert_eq!(z["zu"], json!(true));
        assert_eq!(z["form"], "json");
        assert_eq!(z["abgeschnitten"], json!(false));
        assert_eq!(z["methode"], "POST");
        assert_eq!(z["pfad"], "/auto");
        assert_eq!(z["status"], 200);
        assert_eq!(z["ziel"], "copilot");
        let text = z.to_string();
        assert!(!text.contains(MARKE), "{text}");
        assert!(!text.contains("hgx7"), "{text}");
        let mut felder: Vec<&String> = z.as_object().expect("object").keys().collect();
        felder.sort();
        assert_eq!(
            felder,
            [
                "abgeschnitten",
                "form",
                "methode",
                "namen",
                "pfad",
                "status",
                "wert_verworfen",
                "werte",
                "ziel",
                "zu"
            ]
        );
    }

    /// Counter check: without a marked name the path is not kept closed.
    #[test]
    fn ohne_wort_bleibt_der_pfad_nicht_zu() {
        let z = zeile_von(
            br#"{"data":[{"id":"gpt","name":"x"}],"object":"list"}"#,
            &[],
        );
        assert_eq!(
            z["namen"],
            json!(["data", "data[].id", "data[].name", "object"])
        );
        assert_eq!(z["zu"], json!(false));
    }

    #[test]
    fn jedes_wort_in_jeder_schreibweise_als_teilwort() {
        for n in [
            "token",
            "Access_Token",
            "client_secret",
            "apiKey",
            "monkey",
            "SESSION_id",
            "credentials",
            "x.Credential",
        ] {
            assert!(bleibt_zu(n), "{n}");
        }
        for n in ["id", "data", "tok_en", "sess", "cred", "ke", ""] {
            assert!(!bleibt_zu(n), "{n}");
        }
        // a word deep in the tree marks the path, also as a key below a list
        let z = zeile_von(br#"{"a":[{"b":{"refresh_TOKEN":1}}]}"#, &[]);
        assert_eq!(z["namen"], json!(["a", "a[].b", "a[].b.refresh_TOKEN"]));
        assert_eq!(z["zu"], json!(true));
    }

    #[test]
    fn listen_auf_der_wurzel_und_in_listen() {
        let (n, ab) = namen(&json!([{"id": 1}, [{"x": 2}], 3]));
        assert_eq!(n, ["[].id", "[][].x"]);
        assert!(!ab);
        let (n, ab) = namen(&json!("nur ein wert"));
        assert!(n.is_empty() && !ab);
    }

    #[test]
    fn kein_json_und_zu_gross_bleiben_zu() {
        let z = zeile_von(b"kein json", &[]);
        assert_eq!(z["form"], "kein_json");
        assert_eq!(z["namen"], json!([]));
        assert_eq!(z["zu"], json!(true));
        let z = zeile_von(b"", &[]);
        assert_eq!(z["form"], "kein_json");
        // exactly at the limit the copy is read, one byte more drops it
        let mut genau = vec![b' '; KOERPER_MAX - 2];
        genau.splice(0..0, *b"{}");
        let z = zeile_von(&genau, &[]);
        assert_eq!(z["form"], "json");
        assert_eq!(z["zu"], json!(false));
        let mut s = Sammler::neu();
        s.schieben(&genau);
        assert!(!s.zu_gross);
        // one byte above the limit, in a second piece, drops the copy
        s.schieben(b" ");
        assert!(s.zu_gross);
        // a later piece does not bring the copy back
        s.schieben(b"{}");
        let z = s.zeile(&[], "copilot", "GET", "/models", 200);
        assert!(z.contains(r#""form":"zu_gross""#), "{z}");
        assert!(z.contains(r#""zu":true"#), "{z}");
        assert_eq!(s.daten.len(), 0);
    }

    #[test]
    fn grenzen_schneiden_ab_und_halten_zu() {
        // NAMEN_MAX names pass, one more cuts
        let mut obj = serde_json::Map::new();
        for i in 0..NAMEN_MAX {
            obj.insert(format!("n{i:03}"), json!(1));
        }
        let (liste, ab) = namen(&Value::Object(obj.clone()));
        assert_eq!(liste.len(), NAMEN_MAX);
        assert!(!ab);
        obj.insert("n999".into(), json!(1));
        let (liste, ab) = namen(&Value::Object(obj.clone()));
        assert_eq!(liste.len(), NAMEN_MAX);
        assert!(ab);
        let z = zeile_von(Value::Object(obj).to_string().as_bytes(), &[]);
        assert_eq!(z["abgeschnitten"], json!(true));
        assert_eq!(z["zu"], json!(true));
        // a name of NAME_MAX bytes passes, one byte more is left out
        let lang = "a".repeat(NAME_MAX);
        let (liste, ab) = namen(&json!({ lang.clone(): 1 }));
        assert_eq!(liste, [lang.as_str()]);
        assert!(!ab);
        let (liste, ab) = namen(&json!({ format!("{lang}a"): 1, "b": 2 }));
        assert_eq!(liste, ["b"]);
        assert!(ab);
        // TIEFE_MAX levels pass, one more cuts
        let mut tief = json!(1);
        for _ in 0..TIEFE_MAX {
            tief = json!({ "k": tief });
        }
        let (liste, ab) = namen(&tief);
        assert_eq!(liste.len(), TIEFE_MAX);
        assert!(!ab);
        let (liste, ab) = namen(&json!({ "k": tief }));
        assert_eq!(liste.len(), TIEFE_MAX);
        assert!(ab);
        let (_, ab) = namen(&json!({ "l": tief.clone() }));
        assert!(ab);
        let mut liste2 = json!(1);
        for _ in 0..=TIEFE_MAX {
            liste2 = json!([liste2]);
        }
        assert!(namen(&liste2).1);
    }

    /// A held secret as a name never stands in the line: the form `geheimnis`, no names, the path stays closed.
    #[test]
    fn ein_gehaltenes_geheimnis_als_name_steht_nie_in_der_zeile() {
        let werte = [Geheim::neu(MARKE.as_bytes().to_vec())];
        let body = format!(r#"{{"ok":{{"{MARKE}":1}}}}"#);
        let mut s = Sammler::neu();
        s.schieben(body.as_bytes());
        let z = s.zeile(&werte, "copilot", "GET", "/models", 200);
        assert!(!z.contains(MARKE), "{z}");
        assert!(z.contains(r#""form":"geheimnis""#), "{z}");
        assert!(z.contains(r#""namen":[]"#), "{z}");
        assert!(z.contains(r#""zu":true"#), "{z}");
        // counter check: the same body without a held secret shows the names
        let z = zeile_von(body.as_bytes(), &[]);
        assert_eq!(z["form"], "json");
        // a value equal to the secret is no name: only names are searched
        let werte = [Geheim::neu(b"wert-geheim".to_vec())];
        let mut s = Sammler::neu();
        s.schieben(br#"{"a":"wert-geheim"}"#);
        let z = s.zeile(&werte, "copilot", "GET", "/models", 200);
        assert!(z.contains(r#""form":"json""#), "{z}");
        assert!(!z.contains("wert-geheim"), "{z}");
    }

    /// A name with LF or a quote stays inside the one line (canonical JSON escapes it).
    #[test]
    fn ein_name_bricht_die_zeile_nie() {
        let z = zeile_von(br#"{"a\nhalter.kette: x":1,"b\"c":2}"#, &[]);
        assert_eq!(z["namen"], json!(["a\nhalter.kette: x", "b\"c"]));
    }

    /// The line of `GET /models` (or another `methode` `pfad`) with `status` for `body`, as JSON, and its text.
    fn zeile_bei(
        body: &[u8],
        geheim: &[Geheim],
        methode: &str,
        pfad: &str,
        status: u16,
    ) -> (Value, String) {
        let mut s = Sammler::neu();
        s.schieben(body);
        let z = s.zeile(geheim, "copilot", methode, pfad, status);
        let json = z.strip_prefix(PRAEFIX).expect("prefix");
        (
            serde_json::from_str(json.trim_end()).expect("line is JSON"),
            z,
        )
    }

    /// The body of `GET /models` with these ids.
    fn modelle(ids: &[&str]) -> Vec<u8> {
        let data: Vec<Value> = ids.iter().map(|i| json!({ "id": i })).collect();
        json!({ "data": data, "object": "list" })
            .to_string()
            .into_bytes()
    }

    /// Case: real model names stand in the line; the line has exactly the two value fields; no other value.
    #[test]
    fn echte_modellnamen_stehen_in_der_zeile() {
        let body = br#"{"data":[{"id":"gpt-6-luna","name":"luna-name","vendor":"openai-x"},{"id":"claude-sonnet-5.5"}],"object":"list-x"}"#;
        let (z, text) = zeile_bei(body, &[], "GET", "/models", 200);
        assert_eq!(
            z["werte"],
            json!({"data[].id": ["claude-sonnet-5.5", "gpt-6-luna"], "error.code": null})
        );
        assert_eq!(z["wert_verworfen"], 0);
        assert_eq!(z["abgeschnitten"], json!(false));
        assert_eq!(z["zu"], json!(false));
        assert_eq!(z["form"], "json");
        // other fields keep only their names, never a value, even one in word form
        for fremd in ["luna-name", "openai-x", "list-x"] {
            assert!(!text.contains(fremd), "{text}");
        }
        assert_eq!(
            z["namen"],
            json!([
                "data",
                "data[].id",
                "data[].name",
                "data[].vendor",
                "object"
            ])
        );
        assert_eq!(z["werte"].as_object().expect("object").len(), 2);
    }

    /// Case: a value in another field (`data[].name`, `access_token`, `error.message`, `id` at the root) gives no
    /// value, only its name; `data[].id` only for `GET /models`, `error.code` only for an error answer.
    #[test]
    fn ein_anderes_feld_gibt_keinen_wert() {
        let body = br#"{"access_token":"tok-wert-1","id":"wurzel-id","data":[{"name":"name-wert","id":"gpt-6-luna"}],"error":{"message":"text-wert","code":"code-wert","x":{"code":"tief-wert"}}}"#;
        let (z, text) = zeile_bei(body, &[], "GET", "/models", 200);
        assert_eq!(
            z["werte"],
            json!({"data[].id": ["gpt-6-luna"], "error.code": null})
        );
        for fremd in [
            "tok-wert-1",
            "wurzel-id",
            "name-wert",
            "text-wert",
            "code-wert",
            "tief-wert",
        ] {
            assert!(!text.contains(fremd), "{fremd}: {text}");
        }
        assert_eq!(z["wert_verworfen"], 0);
        // `data[].id` of another path is no model name
        for (m, p) in [("POST", "/models"), ("GET", "/models/x"), ("POST", "/auto")] {
            let (z, text) = zeile_bei(body, &[], m, p, 200);
            assert_eq!(
                z["werte"],
                json!({"data[].id": [], "error.code": null}),
                "{m} {p}"
            );
            assert!(!text.contains("gpt-6-luna"), "{text}");
        }
        // `error.code` below a list or deeper is not the field
        let (z, _) = zeile_bei(
            br#"{"error":[{"code":"c-1"}],"e":{"error":{"code":"c-2"}}}"#,
            &[],
            "GET",
            "/models",
            400,
        );
        assert_eq!(z["werte"]["error.code"], Value::Null);
        assert_eq!(z["wert_verworfen"], 0);
    }

    /// Case: a value with a space, with a slash and with 65 bytes is dropped and counted; 64 bytes pass.
    #[test]
    fn falsche_form_wird_verworfen_und_gezaehlt() {
        let lang64: String = "abcdefghij-".repeat(6)[..64].to_owned();
        let lang65 = format!("{lang64}k");
        assert_eq!((lang64.len(), lang65.len()), (64, 65));
        let werte = [
            "gpt 6",
            "openai/gpt-6",
            lang65.as_str(),
            "",
            "gpt-6\n",
            "gpt-6\u{e4}",
            "gpt:6",
            "ok-1",
        ];
        let (z, text) = zeile_bei(&modelle(&werte), &[], "GET", "/models", 200);
        assert_eq!(z["werte"]["data[].id"], json!(["ok-1"]));
        assert_eq!(z["wert_verworfen"], 7);
        for w in ["gpt 6", "openai/gpt-6", "gpt:6", lang65.as_str()] {
            assert!(!text.contains(w), "{w}: {text}");
        }
        // exactly 64 bytes pass
        let (z, _) = zeile_bei(&modelle(&[lang64.as_str()]), &[], "GET", "/models", 200);
        assert_eq!(z["werte"]["data[].id"], json!([lang64]));
        assert_eq!(z["wert_verworfen"], 0);
        // an id that is no string counts too and never stands in the line
        let (z, text) = zeile_bei(
            br#"{"data":[{"id":4711},{"id":["gpt-x"]},{"id":null},"gpt-y"]}"#,
            &[],
            "GET",
            "/models",
            200,
        );
        assert_eq!(z["werte"]["data[].id"], json!([]));
        assert_eq!(z["wert_verworfen"], 3);
        assert!(!text.contains("4711") && !text.contains("gpt-x"), "{text}");
        assert!(wortform("a") && wortform(&lang64) && !wortform(&lang65) && !wortform(""));
        // the check itself fails closed for a value that is no string
        for v in [
            json!(4711),
            json!(null),
            json!(true),
            json!(["gpt-x"]),
            json!({"a": "b"}),
        ] {
            assert!(!zulaessig(&[], &v), "{v}");
        }
        assert!(zulaessig(&[], &json!("gpt-x")));
    }

    /// Case: a token-like value is dropped although it fits the word form: the token check comes first. Each rule of
    /// the token check alone, with a counter check that passes.
    #[test]
    fn tokenaehnlich_vor_wortform() {
        let gho = format!("gho_{}", "a1b2c3".repeat(6));
        assert_eq!(gho.len(), 40);
        assert!(wortform(&gho), "the value fits the word form");
        let geheim = [Geheim::neu(MARKE.as_bytes().to_vec())];
        let teil20 = "0123456789abcdef0123";
        let fall = [
            gho.as_str(),
            "ghu_ab12",
            "GHP_x",
            "ghs_1",
            "ghr_1",
            "github_pat_1",
            "sk-ant-1",
            "Gho_x",
            "model-apikey",
            "my-token-1",
            "x-session",
            "secret.1",
            "credential",
            teil20,
            MARKE,
        ];
        for w in fall {
            assert!(tokenaehnlich(&geheim, w), "{w}");
            assert!(!zulaessig(&geheim, &json!(w)), "{w}");
        }
        let (z, text) = zeile_bei(&modelle(&fall), &geheim, "GET", "/models", 200);
        assert_eq!(z["werte"]["data[].id"], json!([]));
        assert_eq!(z["wert_verworfen"], fall.len());
        for w in fall {
            assert!(!text.contains(w), "{w}: {text}");
        }
        // counter checks: a part of 19 bytes, a prefix inside the name and the mark without the held secret pass
        let frei = ["0123456789abcdef012", "x-gho_1", "a-sk-1", MARKE];
        for w in frei {
            assert!(zulaessig(&[], &json!(w)), "{w}");
        }
        let (z, _) = zeile_bei(&modelle(&frei), &[], "GET", "/models", 200);
        assert_eq!(z["wert_verworfen"], 0);
        assert_eq!(z["werte"]["data[].id"].as_array().expect("list").len(), 4);
        // the held secret in another form of the leak lock (Base64) is token-like too
        let kurz = [Geheim::neu(b"ab-cd".to_vec())];
        let b64 = crate::b64::kodieren(b"ab-cd");
        let b64 = b64.trim_end_matches('=');
        assert!(wortform(b64) && !tokenaehnlich(&[], b64), "{b64}");
        assert!(tokenaehnlich(&kurz, b64), "{b64}");
    }

    /// Case: `error.code` of an error answer stands in the line; the same body with status 200 gives no value.
    #[test]
    fn fehler_code_einer_fehlerantwort() {
        let body = br#"{"error":{"code":"model_not_supported","message":"The model gpt-6-luna is not supported"}}"#;
        let (z, text) = zeile_bei(body, &[], "POST", "/chat/completions", 400);
        assert_eq!(
            z["werte"],
            json!({"data[].id": [], "error.code": "model_not_supported"})
        );
        assert_eq!(z["wert_verworfen"], 0);
        assert!(
            !text.contains("is not supported") && !text.contains("gpt-6-luna"),
            "{text}"
        );
        let (z, _) = zeile_bei(body, &[], "GET", "/models", 401);
        assert_eq!(z["werte"]["error.code"], "model_not_supported");
        for status in [200, 204, 399] {
            let (z, _) = zeile_bei(body, &[], "POST", "/chat/completions", status);
            assert_eq!(z["werte"]["error.code"], Value::Null, "{status}");
        }
        // a code in a wrong form or token-like is dropped and counted
        for code in [r#""bad code""#, r#""ghu_ab12""#, "401", r#"{"x":1}"#] {
            let b = format!(r#"{{"error":{{"code":{code}}}}}"#);
            let (z, text) = zeile_bei(b.as_bytes(), &[], "POST", "/chat/completions", 403);
            assert_eq!(z["werte"]["error.code"], Value::Null, "{code}");
            assert_eq!(z["wert_verworfen"], 1, "{code}");
            assert!(
                !text.contains("bad code") && !text.contains("ghu_ab12"),
                "{text}"
            );
        }
    }

    /// Case: 200 model names pass; with 201 the line is `abgeschnitten` (and so `zu`) and holds the first 200 in byte
    /// order, nothing further.
    #[test]
    fn grenze_200_modellnamen() {
        let ids: Vec<String> = (0..=WERTE_MAX).map(|i| format!("m-{i:03}")).collect();
        let alle: Vec<&str> = ids.iter().map(String::as_str).collect();
        let (z, _) = zeile_bei(&modelle(&alle[..WERTE_MAX]), &[], "GET", "/models", 200);
        assert_eq!(z["werte"]["data[].id"].as_array().expect("list").len(), 200);
        assert_eq!(z["abgeschnitten"], json!(false));
        assert_eq!(z["zu"], json!(false));
        let (z, text) = zeile_bei(&modelle(&alle), &[], "GET", "/models", 200);
        let liste = z["werte"]["data[].id"].as_array().expect("list");
        assert_eq!(liste.len(), 200);
        assert_eq!(liste[199], "m-199");
        assert!(!text.contains("m-200"), "{text}");
        assert_eq!(z["abgeschnitten"], json!(true));
        assert_eq!(z["zu"], json!(true));
        assert_eq!(z["wert_verworfen"], 0);
        // a twice given name counts once
        let (z, _) = zeile_bei(&modelle(&["a-1", "a-1"]), &[], "GET", "/models", 200);
        assert_eq!(z["werte"]["data[].id"], json!(["a-1"]));
    }

    /// No value without the form `json`: a held secret in a name, no JSON and a too large body give no value.
    #[test]
    fn ohne_form_json_kein_wert() {
        let geheim = [Geheim::neu(MARKE.as_bytes().to_vec())];
        let body = format!(r#"{{"data":[{{"id":"gpt-6-luna"}}],"{MARKE}":1}}"#);
        let (z, text) = zeile_bei(body.as_bytes(), &geheim, "GET", "/models", 200);
        assert_eq!(z["form"], "geheimnis");
        assert_eq!(z["werte"], json!({"data[].id": [], "error.code": null}));
        assert!(!text.contains("gpt-6-luna"), "{text}");
        let (z, _) = zeile_bei(b"gpt-6-luna", &[], "GET", "/models", 200);
        assert_eq!(z["werte"], json!({"data[].id": [], "error.code": null}));
        assert_eq!(z["wert_verworfen"], 0);
    }
}
