//! The diagnosis of the answer form of the first start (section 4, "Antwortform im Erststart"; decision of the lead,
//! 25.11.2026): for a path that `erststart_antwortform` of a target in `halter.json` names explicitly, the holder writes
//! one journal line with the field names of the JSON answer, never a value. A name with `token`, `secret`, `key`,
//! `session` or `credential` (any case, also as part of a word) marks the path as "stays closed" (`"zu":true`).
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
    /// `pfad`, `status`, `ziel` and `zu`, then LF. `zu` is `true` for every form but `json`, for a cut list and for a
    /// name after [`bleibt_zu`]. A held secret in the names (in any form of the leak lock) gives the form `geheimnis`
    /// and no names.
    #[must_use]
    pub fn zeile(
        &self,
        werte: &[Geheim],
        ziel: &str,
        methode: &str,
        pfad: &str,
        status: u16,
    ) -> String {
        let (form, liste, ab) = if self.zu_gross {
            ("zu_gross", Vec::new(), false)
        } else {
            match serde_json::from_slice::<Value>(&self.daten) {
                Err(_) => ("kein_json", Vec::new(), false),
                Ok(v) => {
                    let (n, ab) = namen(&v);
                    if leck::trifft(werte, n.join("\n").as_bytes()) {
                        ("geheimnis", Vec::new(), false)
                    } else {
                        ("json", n, ab)
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
}
