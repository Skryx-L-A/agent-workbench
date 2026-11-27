//! The form of the company default as far as the helper checks it (contract 15.12.3, stages H4, H8 and H10), the form of
//! the request (H8) and the comparison outside `leseklassen` (H10). Classes and groups are not checked here; that is
//! the work of `setzen` in the service.

use serde_json::{Map, Value};

/// Name of the only key the helper may change.
pub const LESEKLASSEN: &str = "leseklassen";
/// Name of the list of the admin accounts.
pub const ADMINS: &str = "admins";
/// Most entries of `leseklassen`.
pub const LESEKLASSEN_MAX: usize = 4096;

/// `true` for an account name of the form `[a-z_][a-z0-9_-]*` (as `KONTO_FORM` of the reference, without a line end).
#[must_use]
pub fn ist_konto(s: &str) -> bool {
    let mut zeichen = s.bytes();
    let Some(erstes) = zeichen.next() else {
        return false;
    };
    (erstes.is_ascii_lowercase() || erstes == b'_')
        && zeichen.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// `true` for 64 lower-case hex digits.
#[must_use]
pub fn ist_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn liste_der_admins(a: &Value) -> Option<Vec<&str>> {
    let liste = a.as_array()?;
    let mut namen: Vec<&str> = Vec::with_capacity(liste.len());
    for k in liste {
        let k = k.as_str()?;
        if !ist_konto(k) || namen.contains(&k) {
            return None;
        }
        namen.push(k);
    }
    Some(namen)
}

fn leseklassen_gut(l: &Value) -> bool {
    let Some(liste) = l.as_array() else {
        return false;
    };
    if liste.len() > LESEKLASSEN_MAX {
        return false;
    }
    let mut paare: Vec<(&str, &str)> = Vec::with_capacity(liste.len());
    for e in liste {
        let Some(e) = e.as_object() else {
            return false;
        };
        if e.len() != 3 {
            return false;
        }
        let (Some(ebene), Some(wer), Some(_)) = (
            e.get("ebene").and_then(Value::as_str),
            e.get("wer").and_then(Value::as_str),
            e.get("klasse").and_then(Value::as_str),
        ) else {
            return false;
        };
        if !matches!(ebene, "nutzer" | "gruppe") || paare.contains(&(ebene, wer)) {
            return false;
        }
        paare.push((ebene, wer));
    }
    true
}

/// The admin accounts of `inhalt` if it has the form of the company default (H4): an object with at least `admins`
/// (distinct accounts) and `leseklassen` (at most 4096 objects with exactly `ebene` `nutzer` or `gruppe`, `wer` and
/// `klasse` as strings, each pair of `ebene` and `wer` once). `None` otherwise.
#[must_use]
pub fn admins_der_vorgabe(inhalt: &Value) -> Option<Vec<&str>> {
    let o = inhalt.as_object()?;
    let admins = liste_der_admins(o.get(ADMINS)?)?;
    leseklassen_gut(o.get(LESEKLASSEN)?).then_some(admins)
}

/// A request in the form of H8.
#[derive(Debug, Clone, PartialEq)]
pub struct Anfrage {
    /// sha256 (hex, lower case) of the bytes the client read last.
    pub basis: String,
    /// The whole new company default.
    pub inhalt: Value,
}

/// The request if `wert` is an object with exactly `art` `Ersetzen`, `fassung` `1` (an integer), `basis` (64 hex digits,
/// lower case) and `inhalt` in the form of H4 (H8). `None` otherwise.
#[must_use]
pub fn anfrage(wert: Value) -> Option<Anfrage> {
    let Value::Object(mut o) = wert else {
        return None;
    };
    if o.len() != 4 {
        return None;
    }
    if o.get("art").and_then(Value::as_str) != Some("Ersetzen") {
        return None;
    }
    if o.get("fassung").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let basis = o.get("basis").and_then(Value::as_str)?.to_owned();
    if !ist_hex64(&basis) {
        return None;
    }
    let inhalt = o.remove("inhalt")?;
    admins_der_vorgabe(&inhalt)?;
    Some(Anfrage { basis, inhalt })
}

fn ohne_leseklassen(o: &Map<String, Value>) -> Map<String, Value> {
    o.iter()
        .filter(|(k, _)| k.as_str() != LESEKLASSEN)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// `true` if `neu` and `alt` are objects that are equal outside `leseklassen` (H10): the list of the admins, every
/// other key and the set of keys.
#[must_use]
pub fn nur_leseklassen_anders(alt: &Value, neu: &Value) -> bool {
    match (alt.as_object(), neu.as_object()) {
        (Some(a), Some(n)) => ohne_leseklassen(a) == ohne_leseklassen(n),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn le(ebene: &str, wer: &str) -> Value {
        json!({"ebene": ebene, "wer": wer, "klasse": "intern"})
    }

    #[test]
    fn konto_und_hex() {
        for gut in ["a", "_", "konto-admin", "k0_9-x", "_a"] {
            assert!(ist_konto(gut), "{gut}");
        }
        for schlecht in [
            "",
            "-a",
            "0a",
            "A",
            "konto admin",
            "konto\n",
            "kö",
            "a/b",
            "a.",
        ] {
            assert!(!ist_konto(schlecht), "{schlecht:?}");
        }
        assert!(ist_hex64(&"0123456789abcdef".repeat(4)));
        assert!(!ist_hex64(&"0123456789abcdef".repeat(4).to_uppercase()));
        assert!(!ist_hex64(&"0".repeat(63)));
        assert!(!ist_hex64(&"0".repeat(65)));
        assert!(!ist_hex64(&format!("{}g", "0".repeat(63))));
        assert!(!ist_hex64(&format!("{}/", "0".repeat(63))));
        assert!(!ist_hex64(&format!("{}`", "0".repeat(63))));
    }

    #[test]
    fn form_der_vorgabe() {
        let gut = json!({"admins": ["konto-a", "konto-b"], "leseklassen": [le("nutzer", "x"), le("gruppe", "x")]});
        assert_eq!(admins_der_vorgabe(&gut), Some(vec!["konto-a", "konto-b"]));
        let mehr = json!({"admins": [], "leseklassen": [], "weiteres": 1});
        assert_eq!(
            admins_der_vorgabe(&mehr),
            Some(vec![]),
            "a further key is no load error"
        );
        for schlecht in [
            json!([]),
            json!({"admins": []}),
            json!({"leseklassen": []}),
            json!({"admins": "konto-a", "leseklassen": []}),
            json!({"admins": ["konto-a", "konto-a"], "leseklassen": []}),
            json!({"admins": ["Konto"], "leseklassen": []}),
            json!({"admins": [1], "leseklassen": []}),
            json!({"admins": [], "leseklassen": {}}),
            json!({"admins": [], "leseklassen": [1]}),
            json!({"admins": [], "leseklassen": [le("nutzer", "x"), le("nutzer", "x")]}),
            json!({"admins": [], "leseklassen": [le("firma", "x")]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": "x"}]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": "x", "klasse": 1}]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": 1, "klasse": "a"}]}),
            json!({"admins": [], "leseklassen": [{"ebene": 1, "wer": "x", "klasse": "a"}]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": "x", "klasse": "a", "k": []}]}),
            json!({"admins": [], "leseklassen": [{"ebene": "nutzer", "wer": "x", "kl": "a", "k": []}]}),
        ] {
            assert_eq!(admins_der_vorgabe(&schlecht), None, "{schlecht}");
        }
    }

    #[test]
    fn grenze_der_leseklassen() {
        let liste: Vec<Value> = (0..LESEKLASSEN_MAX)
            .map(|i| le("nutzer", &format!("k{i}")))
            .collect();
        let mut v = json!({"admins": [], "leseklassen": liste});
        assert!(admins_der_vorgabe(&v).is_some(), "4096 entries");
        v["leseklassen"]
            .as_array_mut()
            .expect("list")
            .push(le("nutzer", "eins-mehr"));
        assert!(admins_der_vorgabe(&v).is_none(), "4097 entries");
    }

    #[test]
    fn form_der_anfrage() {
        let inhalt = json!({"admins": ["konto-a"], "leseklassen": []});
        let basis = "a".repeat(64);
        let gut = json!({"art": "Ersetzen", "fassung": 1, "basis": basis, "inhalt": inhalt});
        assert_eq!(
            anfrage(gut.clone()),
            Some(Anfrage {
                basis: basis.clone(),
                inhalt: inhalt.clone()
            })
        );
        let mut falsch = Vec::new();
        for (k, v) in [
            ("art", json!("Lesen")),
            ("art", json!(1)),
            ("fassung", json!(2)),
            ("fassung", json!(1.0)),
            ("fassung", json!(true)),
            ("fassung", json!("1")),
            ("basis", json!("alt")),
            ("basis", json!(1)),
            ("inhalt", json!(["kein", "objekt"])),
            ("inhalt", json!({"admins": ["konto-a"]})),
        ] {
            let mut x = gut.clone();
            x[k] = v;
            falsch.push(x);
        }
        let mut weiter = gut.clone();
        weiter["noch"] = json!(1);
        falsch.push(weiter);
        for k in ["art", "fassung", "basis", "inhalt"] {
            let mut x = gut.clone();
            x.as_object_mut().expect("object").remove(k);
            falsch.push(x);
        }
        falsch.push(json!([gut.clone()]));
        for x in falsch {
            assert_eq!(anfrage(x.clone()), None, "{x}");
        }
    }

    #[test]
    fn nur_leseklassen() {
        let alt = json!({"admins": ["a"], "leseklassen": [], "x": 1});
        assert!(nur_leseklassen_anders(&alt, &alt));
        assert!(nur_leseklassen_anders(
            &alt,
            &json!({"admins": ["a"], "leseklassen": [le("nutzer", "n")], "x": 1})
        ));
        for neu in [
            json!({"admins": ["a", "b"], "leseklassen": [], "x": 1}),
            json!({"admins": [], "leseklassen": [], "x": 1}),
            json!({"admins": ["a"], "leseklassen": []}),
            json!({"admins": ["a"], "leseklassen": [], "x": 2}),
            json!({"admins": ["a"], "leseklassen": [], "x": 1, "y": 1}),
            json!(["a"]),
        ] {
            assert!(!nur_leseklassen_anders(&alt, &neu), "{neu}");
        }
        assert!(!nur_leseklassen_anders(&json!([]), &json!([])));
    }
}
