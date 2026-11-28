//! The diagnosis of the answer form of the first start (section 4, "Antwortform im Erststart"; decision of the lead,
//! 25.11.2026): for a path that `erststart_antwortform` of a target in `halter.json` names explicitly, the holder writes
//! one journal line with the field names of the JSON answer, never a value. A name with `token`, `secret`, `key`,
//! `session` or `credential` (any case, also as part of a word) marks the path as "stays closed" (`"zu":true`).
//!
//! Values (decision of the lead, 26.11.2026, finding of the Copilot start): the line also carries the values of exactly
//! two fields, `data[].id` of `GET /models` and `error.code` of an error answer (status 400 or above). A value stands in
//! the line only if it is not token-like ([`tokenaehnlich`], checked first) and fits `^[A-Za-z0-9._-]{1,64}$`
//! ([`wortform`]); every other value of the two fields only raises `wert_verworfen`. No other field gives a value
//! of its own; the fields of the next paragraph give only a value of a fixed list.
//!
//! Effort and model list (decision of the operator, 27.11.2026): the line names the reasoning effort of the request
//! (`reasoning_effort` and `reasoning.effort` at the root of a JSON body) and, for `GET /models`, `policy.state` and
//! `capabilities.supports.reasoning_effort` of each named model. These values are never copied: each is mapped onto a
//! fixed list ([`STUFEN`], [`ZUSTAENDE`]), and every other present value becomes [`ANDERER`].
//!
//! Without the switch there is no diagnosis, in every build. The answer itself goes on through the leak lock as before;
//! this module only reads a copy of the body.

use std::collections::{BTreeMap, BTreeSet};

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

/// The levels of a reasoning effort the line names: the union of `capabilities.supports.reasoning_effort` in the measured
/// answer of `GET /models` (decision of the operator, 27.11.2026), in this order.
pub const STUFEN: [&str; 6] = ["none", "low", "medium", "high", "xhigh", "max"];
/// The states of `policy.state` the line names.
pub const ZUSTAENDE: [&str; 2] = ["enabled", "disabled"];
/// The value for a present value that is not on the fixed list, or a field of an unexpected form.
pub const ANDERER: &str = "anderer";

/// The value of the list `liste` that `wert` equals, else [`ANDERER`]; never the text of `wert` itself.
fn fest(liste: &[&'static str], wert: &Value) -> &'static str {
    wert.as_str()
        .and_then(|w| liste.iter().find(|l| **l == w))
        .copied()
        .unwrap_or(ANDERER)
}

/// The value at the path `teile` below `v`: `Ok(None)` if a key is missing, `Err(())` if a level above the last one
/// is no object (an unexpected form).
fn bei<'a>(v: &'a Value, teile: &[&str]) -> Result<Option<&'a Value>, ()> {
    let mut jetzt = v;
    for t in teile {
        match jetzt.as_object() {
            Some(o) => match o.get(*t) {
                Some(n) => jetzt = n,
                None => return Ok(None),
            },
            None => return Err(()),
        }
    }
    Ok(Some(jetzt))
}

/// One value of the fixed list `liste` at the path `teile`: `None` if the field is missing, [`ANDERER`] for another
/// value or an unexpected form.
fn fest_bei(v: &Value, teile: &[&str], liste: &[&'static str]) -> Option<&'static str> {
    match bei(v, teile) {
        Ok(None) => None,
        Ok(Some(w)) => Some(fest(liste, w)),
        Err(()) => Some(ANDERER),
    }
}

/// The reasoning effort of one request body.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Anfrage {
    /// `reasoning_effort` at the root: a level of [`STUFEN`], [`ANDERER`], or `None` if missing.
    pub oben: Option<&'static str>,
    /// `reasoning.effort`: a level of [`STUFEN`], [`ANDERER`], or `None` if missing.
    pub unter: Option<&'static str>,
}

/// The reasoning effort of the request body `koerper`. A body that is no JSON object names nothing.
#[must_use]
pub fn anfrage(koerper: &[u8]) -> Anfrage {
    match serde_json::from_slice::<Value>(koerper) {
        Ok(v) if v.is_object() => Anfrage {
            oben: fest_bei(&v, &["reasoning_effort"], &STUFEN),
            unter: fest_bei(&v, &["reasoning", "effort"], &STUFEN),
        },
        _ => Anfrage::default(),
    }
}

/// The levels of `capabilities.supports.reasoning_effort` of one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stufen {
    /// The field is missing.
    Fehlt,
    /// The field is no list, or a level above it is no object.
    Anders,
    /// The levels of [`STUFEN`] in the list, in that order, each once; [`ANDERER`] last, once, for any other element.
    Liste(Vec<&'static str>),
}

/// The values of one model of `GET /models`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modell {
    /// `policy.state`: a state of [`ZUSTAENDE`], [`ANDERER`], or `None` if missing.
    pub zustand: Option<&'static str>,
    /// `capabilities.supports.reasoning_effort`.
    pub stufen: Stufen,
}

/// The values of one element of `data`.
fn modell(e: &Value) -> Modell {
    let stufen = match bei(e, &["capabilities", "supports", "reasoning_effort"]) {
        Ok(None) => Stufen::Fehlt,
        Ok(Some(Value::Array(l))) => {
            let mut s: Vec<&'static str> = STUFEN
                .iter()
                .copied()
                .filter(|st| l.iter().any(|x| x.as_str() == Some(*st)))
                .collect();
            if l.iter().any(|x| fest(&STUFEN, x) == ANDERER) {
                s.push(ANDERER);
            }
            Stufen::Liste(s)
        }
        Ok(Some(_)) | Err(()) => Stufen::Anders,
    };
    Modell {
        zustand: fest_bei(e, &["policy", "state"], &ZUSTAENDE),
        stufen,
    }
}

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

/// The values of the fields of one answer.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Werte {
    /// For each name of `ids`: the values of its first element in `data`.
    pub modelle: BTreeMap<String, Modell>,
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
        let mut modelle = BTreeMap::new();
        for e in liste {
            let Some(id) = e.as_object().and_then(|o| o.get("id")) else {
                continue;
            };
            match id.as_str() {
                Some(t) if zulaessig(geheim, id) => {
                    ids.insert(t.to_owned());
                    modelle.entry(t.to_owned()).or_insert_with(|| modell(e));
                }
                _ => w.verworfen += 1,
            }
        }
        w.abgeschnitten = ids.len() > WERTE_MAX;
        w.ids = ids.into_iter().take(WERTE_MAX).collect();
        modelle.retain(|id, _| w.ids.binary_search(id).is_ok());
        w.modelle = modelle;
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
    anfrage: Anfrage,
}

impl Sammler {
    /// An empty copy.
    #[must_use]
    pub fn neu() -> Self {
        Sammler::default()
    }

    /// An empty copy for the request whose reasoning effort is `anfrage`.
    #[must_use]
    pub fn mit_anfrage(anfrage: Anfrage) -> Self {
        Sammler {
            anfrage,
            ..Sammler::default()
        }
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
    /// `pfad`, `status`, `wert_verworfen`, `werte` (exactly `anfrage.reasoning.effort`, `anfrage.reasoning_effort`, `data[].id`,
    /// `error.code` and `modelle`), `ziel` and `zu`, then LF. The two values of the request stand in every form.
    /// `zu` is `true` for every form but `json`, for a cut list and for a name after [`bleibt_zu`]. A held secret in the
    /// names (in any form of the leak lock) gives the form `geheimnis`, no names and no values of the answer. Values of the
    /// answer only for the form `json`: `data[].id` only for `GET /models`, `error.code` only for a status of 400 or above.
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
                "anfrage.reasoning.effort": self.anfrage.unter,
                "anfrage.reasoning_effort": self.anfrage.oben,
                "data[].id": w.ids,
                "error.code": w.code,
                "modelle": modelle_json(&w.modelle),
            },
            "ziel": ziel,
            "zu": zu,
        });
        let text = json::kanonisch(&wert).unwrap_or_default();
        format!("{PRAEFIX}{text}\n")
    }
}

/// The object `modelle` of the line: each name with exactly `capabilities.supports.reasoning_effort` (a list, `null` or
/// [`ANDERER`]) and `policy.state` (a state, `null` or [`ANDERER`]).
fn modelle_json(modelle: &BTreeMap<String, Modell>) -> Value {
    let mut m = serde_json::Map::new();
    for (id, e) in modelle {
        let stufen = match &e.stufen {
            Stufen::Fehlt => Value::Null,
            Stufen::Anders => json!(ANDERER),
            Stufen::Liste(l) => json!(l),
        };
        m.insert(
            id.clone(),
            json!({
                "capabilities.supports.reasoning_effort": stufen,
                "policy.state": e.zustand,
            }),
        );
    }
    Value::Object(m)
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

    /// The values of a model without `policy` and `capabilities`.
    const LEER: Value = Value::Null;

    /// The object `werte` of a line without a reasoning effort in the request; `LEER` as a model stands for both values
    /// `null`.
    fn ohne_anfrage(ids: Value, code: Value, modelle: Value) -> Value {
        let mut m = modelle;
        for (_, e) in m.as_object_mut().expect("object").iter_mut() {
            if e.is_null() {
                *e = json!({"capabilities.supports.reasoning_effort": null, "policy.state": null});
            }
        }
        let mut w = serde_json::Map::new();
        w.insert("anfrage.reasoning.effort".into(), Value::Null);
        w.insert("anfrage.reasoning_effort".into(), Value::Null);
        w.insert("data[].id".into(), ids);
        w.insert("error.code".into(), code);
        w.insert("modelle".into(), m);
        Value::Object(w)
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
            ohne_anfrage(
                json!(["claude-sonnet-5.5", "gpt-6-luna"]),
                Value::Null,
                json!({"claude-sonnet-5.5": LEER, "gpt-6-luna": LEER})
            )
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
        assert_eq!(z["werte"].as_object().expect("object").len(), 5);
    }

    /// Case: a value in another field (`data[].name`, `access_token`, `error.message`, `id` at the root) gives no
    /// value, only its name; `data[].id` only for `GET /models`, `error.code` only for an error answer.
    #[test]
    fn ein_anderes_feld_gibt_keinen_wert() {
        let body = br#"{"access_token":"tok-wert-1","id":"wurzel-id","data":[{"name":"name-wert","id":"gpt-6-luna"}],"error":{"message":"text-wert","code":"code-wert","x":{"code":"tief-wert"}}}"#;
        let (z, text) = zeile_bei(body, &[], "GET", "/models", 200);
        assert_eq!(
            z["werte"],
            ohne_anfrage(
                json!(["gpt-6-luna"]),
                Value::Null,
                json!({ "gpt-6-luna": LEER })
            )
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
                ohne_anfrage(json!([]), Value::Null, json!({})),
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
            ohne_anfrage(json!([]), json!("model_not_supported"), json!({}))
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
        assert_eq!(z["werte"], ohne_anfrage(json!([]), Value::Null, json!({})));
        assert!(!text.contains("gpt-6-luna"), "{text}");
        let (z, _) = zeile_bei(b"gpt-6-luna", &[], "GET", "/models", 200);
        assert_eq!(z["werte"], ohne_anfrage(json!([]), Value::Null, json!({})));
        assert_eq!(z["wert_verworfen"], 0);
    }
    /// The line of `POST /chat/completions` for the request body `anfrage` and the answer `body` with `status`.
    fn zeile_mit_anfrage(anfrage: &[u8], body: &[u8], status: u16) -> (Value, String) {
        let mut s = Sammler::mit_anfrage(self::anfrage(anfrage));
        s.schieben(body);
        let z = s.zeile(&[], "copilot", "POST", "/chat/completions", status);
        let json = z.strip_prefix(PRAEFIX).expect("prefix");
        (
            serde_json::from_str(json.trim_end()).expect("line is JSON"),
            z,
        )
    }

    /// Case: a known effort of the request stands in the line, at the root and below `reasoning`, each level of
    /// `STUFEN`; counter check: the other field stays `null`.
    #[test]
    fn bekannter_effort_steht_in_der_zeile() {
        for st in STUFEN {
            let b = format!(r#"{{"model":"gpt-6-luna","reasoning_effort":"{st}"}}"#);
            let (z, _) = zeile_mit_anfrage(b.as_bytes(), b"{}", 200);
            assert_eq!(z["werte"]["anfrage.reasoning_effort"], st, "{st}");
            assert_eq!(z["werte"]["anfrage.reasoning.effort"], Value::Null, "{st}");
            let b = format!(r#"{{"reasoning":{{"effort":"{st}","summary":"auto"}}}}"#);
            let (z, _) = zeile_mit_anfrage(b.as_bytes(), b"{}", 200);
            assert_eq!(z["werte"]["anfrage.reasoning.effort"], st, "{st}");
            assert_eq!(z["werte"]["anfrage.reasoning_effort"], Value::Null, "{st}");
        }
        assert_eq!(
            anfrage(br#"{"reasoning_effort":"low","reasoning":{"effort":"max"}}"#),
            Anfrage {
                oben: Some("low"),
                unter: Some("max")
            }
        );
        // the 400 of the first start becomes readable: the effort of the request and the code of the answer in one line
        let (z, _) = zeile_mit_anfrage(
            br#"{"reasoning_effort":"xhigh"}"#,
            br#"{"error":{"code":"invalid_reasoning_effort","message":"m"}}"#,
            400,
        );
        assert_eq!(z["werte"]["anfrage.reasoning_effort"], "xhigh");
        assert_eq!(z["werte"]["error.code"], "invalid_reasoning_effort");
    }

    /// Case: an unknown effort becomes `anderer` and its text stands nowhere in the line, also a value in word form, a
    /// level in another case, a token-like value, a held text, a number, `null`, a list and an object.
    #[test]
    fn unbekannter_effort_wird_anderer() {
        let fremd = [
            r#""minimal""#,
            r#""HIGH""#,
            r#""high ""#,
            r#""ghu_effort0815wert""#,
            r#""effort-wert-4711""#,
            "3",
            "null",
            r#"["high"]"#,
            r#"{"stufe":"high"}"#,
            "true",
        ];
        for f in fremd {
            for b in [
                format!(r#"{{"reasoning_effort":{f}}}"#),
                format!(r#"{{"reasoning":{{"effort":{f}}}}}"#),
            ] {
                let (z, text) = zeile_mit_anfrage(b.as_bytes(), b"{}", 200);
                let w = &z["werte"];
                let feld = if b.contains("reasoning_effort") {
                    "anfrage.reasoning_effort"
                } else {
                    "anfrage.reasoning.effort"
                };
                assert_eq!(w[feld], ANDERER, "{b}");
                for t in ["minimal", "HIGH", "ghu_", "effort-wert", "stufe"] {
                    assert!(!text.contains(t), "{t}: {text}");
                }
            }
        }
        // `reasoning` of another form than an object is `anderer` too
        for r in [r#""high""#, "1", r#"["high"]"#, "null"] {
            let b = format!(r#"{{"reasoning":{r}}}"#);
            assert_eq!(anfrage(b.as_bytes()).unter, Some(ANDERER), "{b}");
            assert_eq!(anfrage(b.as_bytes()).oben, None, "{b}");
        }
    }

    /// Case: a request without an effort names `null` in both fields; so does a body that is no JSON object. The two
    /// values of the request stand in every form of the answer.
    #[test]
    fn anfrage_ohne_effort() {
        for b in [
            &br#"{"model":"gpt-6-luna","messages":[{"role":"user","content":"reasoning_effort high"}]}"#[..],
            br#"{"reasoning":{"summary":"auto"}}"#,
            br#"{"x":{"reasoning_effort":"high"}}"#,
            br#"[{"reasoning_effort":"high"}]"#,
            br#""high""#,
            b"kein json",
            b"",
        ] {
            assert_eq!(anfrage(b), Anfrage::default(), "{}", String::from_utf8_lossy(b));
        }
        let (z, _) = zeile_mit_anfrage(br#"{"model":"m"}"#, b"{}", 200);
        assert_eq!(z["werte"]["anfrage.reasoning_effort"], Value::Null);
        assert_eq!(z["werte"]["anfrage.reasoning.effort"], Value::Null);
        // without the request (`Sammler::neu`) both stay `null`
        let (z, _) = zeile_bei(b"{}", &[], "POST", "/chat/completions", 200);
        assert_eq!(z["werte"]["anfrage.reasoning_effort"], Value::Null);
        // the values of the request also stand for an answer that is no JSON
        let (z, _) = zeile_mit_anfrage(br#"{"reasoning_effort":"medium"}"#, b"kein json", 502);
        assert_eq!(z["form"], "kein_json");
        assert_eq!(z["werte"]["anfrage.reasoning_effort"], "medium");
    }

    /// Case: the model list as measured names `policy.state` and the levels of each model; `data[].name` and the
    /// other fields give no value.
    #[test]
    fn modellliste_mit_zustand_und_stufen() {
        let body = br#"{"data":[
            {"id":"gpt-6-luna","name":"n-wert","policy":{"state":"enabled","terms":"t-wert"},
             "capabilities":{"family":"f-wert","supports":{"reasoning_effort":["none","low","medium","high","xhigh","max"],"streaming":true}}},
            {"id":"claude-opus-5.5","policy":{"state":"disabled"},
             "capabilities":{"supports":{"reasoning_effort":["max","low","medium","high","xhigh"]}}},
            {"id":"gpt-4.1","policy":{"state":"enabled"},"capabilities":{"supports":{"streaming":true}}},
            {"id":"gpt-4o"},
            {"id":"gemini-4-argon","capabilities":{"supports":{"reasoning_effort":["low","medium","high"]}}}
        ],"object":"list"}"#;
        let (z, text) = zeile_bei(body, &[], "GET", "/models", 200);
        assert_eq!(
            z["werte"]["modelle"],
            json!({
                "claude-opus-5.5": {"capabilities.supports.reasoning_effort": ["low","medium","high","xhigh","max"], "policy.state": "disabled"},
                "gemini-4-argon": {"capabilities.supports.reasoning_effort": ["low","medium","high"], "policy.state": null},
                "gpt-4.1": {"capabilities.supports.reasoning_effort": null, "policy.state": "enabled"},
                "gpt-4o": {"capabilities.supports.reasoning_effort": null, "policy.state": null},
                "gpt-6-luna": {"capabilities.supports.reasoning_effort": ["none","low","medium","high","xhigh","max"], "policy.state": "enabled"},
            })
        );
        assert_eq!(z["wert_verworfen"], 0);
        assert_eq!(z["zu"], json!(false));
        for fremd in ["n-wert", "t-wert", "f-wert"] {
            assert!(!text.contains(fremd), "{fremd}: {text}");
        }
        // counter check: the same body on another path gives no model values
        let (z, _) = zeile_bei(body, &[], "POST", "/chat/completions", 200);
        assert_eq!(z["werte"]["modelle"], json!({}));
    }

    /// Case: a model list of an unexpected form gives `anderer` or `null`, never a text of the body, and never a model
    /// whose name did not pass; a twice named model keeps its first values; beyond 200 names no further model.
    #[test]
    fn modellliste_mit_unerwarteter_form() {
        let body = br#"{"data":[
            {"id":"m-1","policy":"enabled","capabilities":"c-wert"},
            {"id":"m-2","policy":{"state":"ENABLED"},"capabilities":{"supports":["reasoning_effort"]}},
            {"id":"m-3","policy":{"state":4},"capabilities":{"supports":{"reasoning_effort":"high"}}},
            {"id":"m-4","policy":{"state":"z-wert"},"capabilities":{"supports":{"reasoning_effort":["high","minimal",3,null,"high"]}}},
            {"id":"m-5","policy":null,"capabilities":{"supports":{"reasoning_effort":[]}}},
            {"id":"m-1","policy":{"state":"disabled"}},
            {"id":"gho_abc","policy":{"state":"enabled"}},
            {"id":"m 6","policy":{"state":"enabled"}},
            "m-7",
            {"name":"m-8","policy":{"state":"enabled"}}
        ]}"#;
        let (z, text) = zeile_bei(body, &[], "GET", "/models", 200);
        assert_eq!(
            z["werte"]["data[].id"],
            json!(["m-1", "m-2", "m-3", "m-4", "m-5"])
        );
        assert_eq!(
            z["werte"]["modelle"],
            json!({
                "m-1": {"capabilities.supports.reasoning_effort": "anderer", "policy.state": "anderer"},
                "m-2": {"capabilities.supports.reasoning_effort": "anderer", "policy.state": "anderer"},
                "m-3": {"capabilities.supports.reasoning_effort": "anderer", "policy.state": "anderer"},
                "m-4": {"capabilities.supports.reasoning_effort": ["high", "anderer"], "policy.state": "anderer"},
                "m-5": {"capabilities.supports.reasoning_effort": [], "policy.state": "anderer"},
            })
        );
        assert_eq!(z["wert_verworfen"], 2);
        for fremd in [
            "c-wert", "z-wert", "ENABLED", "minimal", "gho_abc", "m 6", "m-7", "m-8",
        ] {
            assert!(!text.contains(fremd), "{fremd}: {text}");
        }
        // `data` that is no list, and elements that are no objects, give no model
        for b in [
            &br#"{"data":{"id":"m-1","policy":{"state":"enabled"}}}"#[..],
            br#"{"data":"m-1"}"#,
            br#"{"data":[["m-1"],1,null]}"#,
            br#"[{"id":"m-1"}]"#,
        ] {
            let (z, _) = zeile_bei(b, &[], "GET", "/models", 200);
            assert_eq!(
                z["werte"]["modelle"],
                json!({}),
                "{}",
                String::from_utf8_lossy(b)
            );
            assert_eq!(z["werte"]["data[].id"], json!([]));
        }
        // beyond 200 names the cut model has no values either
        let ids: Vec<String> = (0..=WERTE_MAX).map(|i| format!("m-{i:03}")).collect();
        let alle: Vec<&str> = ids.iter().map(String::as_str).collect();
        let (z, text) = zeile_bei(&modelle(&alle), &[], "GET", "/models", 200);
        let m = z["werte"]["modelle"].as_object().expect("object");
        assert_eq!(m.len(), WERTE_MAX);
        assert!(m.contains_key("m-199") && !m.contains_key("m-200"));
        assert!(!text.contains("m-200"), "{text}");
    }
}
