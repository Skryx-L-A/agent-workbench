//! The rules of the admins for workers (contract 6, sections 14.5 and 15.3): read from the bytes of the default, closed form.
//! Reading the file itself, on descriptors and only from a default of root, is the shell's (`huelle::vorgabe`).

use std::collections::BTreeSet;

use serde_json::Value;

use crate::form;

/// The largest default in bytes.
pub const VORGABE_MAX: usize = 65_536;
/// The largest entry count of a list and the largest highest count.
const LISTE_MAX: usize = 64;
/// The largest value of `laufend` in the input of `worker.anfordern`.
pub(crate) const LAUFEND_MAX: i64 = 1000;

/// The rules for workers: the highest count of running workers of an orchestrator, and the allowed harnesses,
/// models and effort levels. What is not in a list is not allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRegeln {
    pub(crate) hoechstzahl: i64,
    pub(crate) harness: BTreeSet<String>,
    pub(crate) modelle: BTreeSet<String>,
    pub(crate) effort: BTreeSet<String>,
}

/// A list of 1 up to 64 entries without a double, each of the form `gilt`.
fn liste(wert: Option<&Value>, gilt: fn(&str) -> bool) -> Option<BTreeSet<String>> {
    let liste = wert?.as_array()?;
    if !(1..=LISTE_MAX).contains(&liste.len()) {
        return None;
    }
    let mut menge = BTreeSet::new();
    for eintrag in liste {
        let text = eintrag.as_str().filter(|t| gilt(t))?;
        if !menge.insert(text.to_owned()) {
            return None;
        }
    }
    Some(menge)
}

fn schluessel_genau(objekt: &serde_json::Map<String, Value>, namen: &[&str]) -> bool {
    objekt.len() == namen.len() && namen.iter().all(|n| objekt.contains_key(*n))
}

/// Reads the default `{"fassung": 1, "worker": {"hoechstzahl", "harness", "modelle", "effort"}}`. Any other form,
/// size or value gives `None`: then there are no rules, and every request is refused (fail closed).
#[must_use]
pub fn worker_regeln_lesen(bytes: &[u8]) -> Option<WorkerRegeln> {
    if bytes.len() > VORGABE_MAX {
        return None;
    }
    let oben = form::objekt_lesen(bytes)?;
    if !schluessel_genau(&oben, &["fassung", "worker"]) || oben.get("fassung")?.as_i64() != Some(1)
    {
        return None;
    }
    let w = oben.get("worker")?.as_object()?;
    if !schluessel_genau(w, &["hoechstzahl", "harness", "modelle", "effort"]) {
        return None;
    }
    let hoechstzahl = w.get("hoechstzahl")?;
    // `2.0` is no integer: `as_i64` of a float is `None`.
    if !form::ganzzahl_in(hoechstzahl, 1, 64) {
        return None;
    }
    Some(WorkerRegeln {
        hoechstzahl: hoechstzahl.as_i64()?,
        harness: liste(w.get("harness"), form::kennung)?,
        // section 15.3: the form of `modell` of contract 1, not an identifier
        modelle: liste(w.get("modelle"), form::modell)?,
        effort: liste(w.get("effort"), form::kennung)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regel(text: &str) -> Option<WorkerRegeln> {
        worker_regeln_lesen(text.as_bytes())
    }

    const GUT: &str = r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#;

    #[test]
    fn die_gute_vorgabe_wird_gelesen() {
        let r = regel(GUT).expect("rules");
        assert_eq!(r.hoechstzahl, 2);
        assert!(r.harness.contains("claude"));
        assert!(r.modelle.contains("m-1"));
        assert!(r.effort.contains("mittel"));
    }

    #[test]
    fn jede_andere_form_ergibt_keine_regeln() {
        for text in [
            "",
            "[]",
            r#"{"fassung":2,"worker":{"hoechstzahl":2,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]},"x":1}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":["claude"],"modelle":["m-1"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"],"x":1}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":0,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":65,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2.0,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":[],"modelle":["m-1"],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":["claude","claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":["Claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"worker":{"hoechstzahl":2,"harness":["claude"],"modelle":[7],"effort":["mittel"]}}"#,
            r#"{"fassung":1,"fassung":1,"worker":{"hoechstzahl":2,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
        ] {
            assert_eq!(regel(text), None, "{text}");
        }
    }

    #[test]
    fn modelle_haben_die_form_aus_vertrag_1() {
        let mit = |liste: &str, feld: &str| {
            let text = GUT.replace(&format!(r#""{feld}":["#), &format!(r#""{feld}":[{liste},"#));
            regel(&text)
        };
        let r = mit(r#""qwen3:8b","claude-sonnet-5.5","a/b@c+d""#, "modelle").expect("models");
        assert!(r.modelle.contains("qwen3:8b"));
        assert!(r.modelle.contains("claude-sonnet-5.5"));
        assert!(r.modelle.contains("a/b@c+d"));
        assert!(mit(&format!("\"{}\"", "m".repeat(128)), "modelle").is_some());
        for falsch in [
            "\"qwen3 8b\"",
            "\"\"",
            &format!("\"{}\"", "m".repeat(129)),
            "\"m\u{e4}\"",
        ] {
            assert_eq!(mit(falsch, "modelle"), None, "{falsch}");
        }
        // the other lists stay identifiers
        assert_eq!(mit(r#""qwen3:8b""#, "harness"), None);
        assert_eq!(mit(r#""Mittel""#, "effort"), None);
    }

    #[test]
    fn grenzen_der_listen_und_der_groesse() {
        let viele = |n: usize| {
            (0..n)
                .map(|i| format!("\"h{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        };
        let mit = |n: usize| {
            format!(
                r#"{{"fassung":1,"worker":{{"hoechstzahl":64,"harness":[{}],"modelle":["m-1"],"effort":["mittel"]}}}}"#,
                viele(n)
            )
        };
        assert!(regel(&mit(64)).is_some());
        assert_eq!(regel(&mit(65)), None);
        assert_eq!(regel(&mit(1)).map(|r| r.hoechstzahl), Some(64));
        let gross = format!("{}{}", GUT, " ".repeat(VORGABE_MAX - GUT.len()));
        assert!(regel(&gross).is_some());
        let zu_gross = format!("{gross} ");
        assert_eq!(regel(&zu_gross), None);
    }
}
