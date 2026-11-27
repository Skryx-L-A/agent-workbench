//! The program `datentor` (contract section 5): reads one case from stdin and writes the answers as JSON.
//!
//! Exit 0: stdout holds exactly one JSON object `{"antworten": [...]}`. Exit 2: the case is invalid (or the call is
//! wrong); stdout stays empty and stderr holds exactly one line. Exit 1: stdout could not be written. No input panics.

use std::io::{Read, Write};

use crate::ohne_doppel;
use datentor::{Antwort, Art, Einheit, Freigabe, Kunde, Route, Vorgabe, darf};
use serde_json::{Map, Value, json};

/// The largest input in bytes.
pub const MAX_BYTES: usize = 1_048_576;
/// The deepest nesting of arrays and objects.
pub const MAX_TIEFE: usize = 64;

/// Runs the program: `args` are the arguments after the program name. Returns the exit code.
pub fn lauf<R: Read, W: Write, E: Write>(
    args: &[String],
    stdin: R,
    stdout: &mut W,
    stderr: &mut E,
) -> u8 {
    match ablauf(args, stdin, stdout) {
        Ok(()) => 0,
        Err((code, text)) => {
            // A failing stderr leaves nothing more to report; the exit code still tells.
            let _ = writeln!(stderr, "datentor: {}", einzeilig(&text));
            code
        }
    }
}

/// The text as one line: a control character (a newline from an odd input) becomes a space.
fn einzeilig(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// The whole run; the error is the exit code and the text of the stderr line.
fn ablauf<R: Read, W: Write>(
    args: &[String],
    stdin: R,
    stdout: &mut W,
) -> Result<(), (u8, String)> {
    let (befehl, rest) = match args {
        [b, rest @ ..] if b == "klassen" || b == "zonen" || b == "person" => {
            (Some(b.as_str()), rest)
        }
        _ => (None, args),
    };
    match rest {
        [] => {}
        [a] if a == "-" => {}
        _ => {
            return Err((
                2,
                "call: datentor [klassen|zonen|person] [-] < case.json".to_owned(),
            ));
        }
    }
    let mut eingabe = Vec::new();
    stdin
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut eingabe)
        .map_err(|e| (2, format!("cannot read the input: {e}")))?;
    let antwort = match befehl {
        Some("zonen") => crate::programm_zonen::verarbeite(&eingabe),
        Some("person") => crate::programm_person::verarbeite(&eingabe),
        Some(_) => crate::programm_klassen::verarbeite(&eingabe),
        None => verarbeite(&eingabe),
    }
    .map_err(|text| (2, text))?;
    stdout
        .write_all(antwort.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|e| (1, format!("cannot write the output: {e}")))
}

/// Checks one case and returns the output, or the reason the case is invalid.
pub fn verarbeite(eingabe: &[u8]) -> Result<String, String> {
    let fall = fall_lesen(eingabe)?;
    let fall = fall.as_object().ok_or("case is not an object")?;
    felder(
        fall,
        &["name", "regeln", "beschreibung", "vorgabe", "anfragen"],
        "case",
    )?;
    let vorgabe = vorgabe_lesen(fall.get("vorgabe").ok_or("vorgabe is missing")?)?;
    let anfragen = anfragen_liste(fall)?;
    let anfragen = anfragen
        .iter()
        .map(anfrage_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let antworten: Vec<Value> = anfragen
        .iter()
        .map(|a| {
            let antwort = darf(
                &vorgabe,
                &Einheit {
                    klasse: a.klasse.as_deref(),
                },
                a.route.as_deref(),
                a.kunde.as_deref(),
            );
            antwort_json(&antwort, a.route.as_deref())
        })
        .collect();
    Ok(json!({ "antworten": antworten }).to_string())
}

/// Reads one case: size, depth, JSON without a duplicate key (shared by all subcommands).
pub fn fall_lesen(eingabe: &[u8]) -> Result<Value, String> {
    if eingabe.len() > MAX_BYTES {
        return Err(format!("input larger than {MAX_BYTES} bytes"));
    }
    // The depth is checked before parsing, so that no feature of the JSON library decides how deep the parser descends.
    if tiefe_ueber_grenze(eingabe) {
        return Err(format!("nesting deeper than {MAX_TIEFE} levels"));
    }
    ohne_doppel::lesen(eingabe).map_err(|e| format!("not JSON or not unique: {e}"))
}

/// The required list `anfragen` of a case.
pub fn anfragen_liste(fall: &Map<String, Value>) -> Result<&Vec<Value>, String> {
    fall.get("anfragen")
        .ok_or("anfragen is missing")?
        .as_array()
        .ok_or_else(|| "anfragen is not a list".to_owned())
}

/// True if the nesting of `[` and `{` outside strings exceeds [`MAX_TIEFE`]. Malformed input is left to the parser.
fn tiefe_ueber_grenze(eingabe: &[u8]) -> bool {
    let (mut tiefe, mut im_text, mut maskiert) = (0_usize, false, false);
    for &b in eingabe {
        if im_text {
            match (maskiert, b) {
                (true, _) => maskiert = false,
                (false, b'\\') => maskiert = true,
                (false, b'"') => im_text = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => im_text = true,
            b'[' | b'{' => {
                tiefe += 1;
                if tiefe > MAX_TIEFE {
                    return true;
                }
            }
            b']' | b'}' => tiefe = tiefe.saturating_sub(1),
            _ => {}
        }
    }
    false
}

/// The closed form: every field of `objekt` must be in `erlaubt`.
pub fn felder(objekt: &Map<String, Value>, erlaubt: &[&str], wo: &str) -> Result<(), String> {
    match objekt.keys().find(|k| !erlaubt.contains(&k.as_str())) {
        Some(k) => Err(format!("{wo}: unknown field {k:?}")),
        None => Ok(()),
    }
}

/// A required field of `objekt`.
pub fn pflicht<'a>(
    objekt: &'a Map<String, Value>,
    feld: &str,
    wo: &str,
) -> Result<&'a Value, String> {
    objekt
        .get(feld)
        .ok_or_else(|| format!("{wo}: {feld} is missing"))
}

/// A text.
pub fn text(wert: &Value, wo: &str) -> Result<String, String> {
    wert.as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{wo} is not a text"))
}

/// A list of texts.
pub fn texte(wert: &Value, wo: &str) -> Result<Vec<String>, String> {
    let liste = wert
        .as_array()
        .ok_or_else(|| format!("{wo} is not a list"))?;
    liste.iter().map(|x| text(x, wo)).collect()
}

/// An object.
pub fn objekt<'a>(wert: &'a Value, wo: &str) -> Result<&'a Map<String, Value>, String> {
    wert.as_object()
        .ok_or_else(|| format!("{wo} is not an object"))
}

/// A list of objects.
pub fn objekte<'a>(wert: &'a Value, wo: &str) -> Result<Vec<&'a Map<String, Value>>, String> {
    let liste = wert
        .as_array()
        .ok_or_else(|| format!("{wo} is not a list"))?;
    liste.iter().map(|x| objekt(x, wo)).collect()
}

/// Reads the settings of section 2 and checks them through `Vorgabe::neu`.
pub fn vorgabe_lesen(wert: &Value) -> Result<Vorgabe, String> {
    let v = objekt(wert, "vorgabe")?;
    felder(
        v,
        &[
            "klassen",
            "vorgabestufe",
            "routen",
            "kunden",
            "kundendaten_an_cloud",
        ],
        "vorgabe",
    )?;
    let klassen = texte(pflicht(v, "klassen", "vorgabe")?, "klassen")?;
    let stufe = v
        .get("vorgabestufe")
        .map(|s| text(s, "vorgabestufe"))
        .transpose()?;
    let routen = objekte(pflicht(v, "routen", "vorgabe")?, "routen")?
        .into_iter()
        .map(route_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let kunden = objekte(pflicht(v, "kunden", "vorgabe")?, "kunden")?
        .into_iter()
        .map(kunde_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let freigaben = match v.get("kundendaten_an_cloud") {
        None => Vec::new(),
        Some(k) => freigaben_lesen(k)?,
    };
    Vorgabe::neu(klassen, stufe, routen, kunden, freigaben).map_err(|e| format!("vorgabe: {e}"))
}

/// Reads one route.
fn route_lesen(r: &Map<String, Value>) -> Result<Route, String> {
    felder(r, &["schluessel", "art", "klasse"], "route")?;
    let art = match text(pflicht(r, "art", "route")?, "art")?.as_str() {
        "cloud" => Art::Cloud,
        "lokal" => Art::Lokal,
        _ => return Err("route: art is neither cloud nor lokal".to_owned()),
    };
    let klasse = match r.get("klasse") {
        None | Some(Value::Null) => None,
        Some(k) => Some(text(k, "klasse of a route")?),
    };
    Ok(Route {
        schluessel: text(pflicht(r, "schluessel", "route")?, "schluessel of a route")?,
        art,
        klasse,
    })
}

/// Reads one customer; a missing list is empty.
fn kunde_lesen(k: &Map<String, Value>) -> Result<Kunde, String> {
    felder(k, &["schluessel", "erlaubt", "verboten"], "kunde")?;
    let liste = |feld: &str| {
        k.get(feld)
            .map_or_else(|| Ok(Vec::new()), |x| texte(x, feld))
    };
    Ok(Kunde {
        schluessel: text(
            pflicht(k, "schluessel", "kunde")?,
            "schluessel of a customer",
        )?,
        erlaubt: liste("erlaubt")?,
        verboten: liste("verboten")?,
    })
}

/// Reads `kundendaten_an_cloud`: `vorgabe` must be `"nie"`; a missing `freigaben` is empty.
fn freigaben_lesen(wert: &Value) -> Result<Vec<Freigabe>, String> {
    let k = objekt(wert, "kundendaten_an_cloud")?;
    felder(k, &["vorgabe", "freigaben"], "kundendaten_an_cloud")?;
    if pflicht(k, "vorgabe", "kundendaten_an_cloud")?.as_str() != Some("nie") {
        return Err("kundendaten_an_cloud: vorgabe is not \"nie\"".to_owned());
    }
    let Some(liste) = k.get("freigaben") else {
        return Ok(Vec::new());
    };
    objekte(liste, "freigaben")?
        .into_iter()
        .map(|f| {
            felder(f, &["route", "kunde"], "freigabe")?;
            Ok(Freigabe {
                route: text(pflicht(f, "route", "freigabe")?, "route of a release")?,
                kunde: text(pflicht(f, "kunde", "freigabe")?, "kunde of a release")?,
            })
        })
        .collect()
}

/// One request of the case.
struct Anfrage {
    /// The class of the unit. A class that is not a text becomes the empty text, which is no valid class (the form of a
    /// key has at least one character), so it ends in rule D4.
    klasse: Option<String>,
    route: Option<String>,
    kunde: Option<String>,
}

/// An optional text: missing and `null` are none, a text is some, anything else makes the case invalid.
pub fn text_oder_null(wert: Option<&Value>, wo: &str) -> Result<Option<String>, String> {
    match wert {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{wo} is neither a text nor null")),
    }
}

/// Reads one request.
fn anfrage_lesen(wert: &Value) -> Result<Anfrage, String> {
    let a = objekt(wert, "request")?;
    felder(a, &["einheit", "route", "kunde"], "request")?;
    let einheit = objekt(pflicht(a, "einheit", "request")?, "einheit")?;
    felder(einheit, &["klasse"], "einheit")?;
    let klasse = match einheit.get("klasse") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => Some(String::new()),
    };
    Ok(Anfrage {
        klasse,
        route: text_oder_null(a.get("route"), "route")?,
        kunde: text_oder_null(a.get("kunde"), "kunde")?,
    })
}

/// The answer as JSON: `darf`, `grund`, `klasse` and `meldung`.
pub fn antwort_json(antwort: &Antwort, route: Option<&str>) -> Value {
    json!({
        "darf": antwort.darf(),
        "grund": antwort.grund.code(),
        "klasse": antwort.klasse,
        "meldung": antwort.meldung(route),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader that always fails.
    struct Kaputt;
    impl Read for Kaputt {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken"))
        }
    }
    /// A writer that always fails.
    struct Voll;
    impl Write for Voll {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("full"))
        }
    }
    /// A writer that takes every byte and fails only at `flush`.
    struct NurFlushKaputt;
    impl Write for NurFlushKaputt {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("flush"))
        }
    }

    const FALL: &str = r#"{"vorgabe":{"klassen":["a","b"],"routen":[{"schluessel":"r","art":"lokal","klasse":"b"}],"kunden":[]},"anfragen":[{"einheit":{"klasse":"a"},"route":"r"}]}"#;

    fn lauf_mit(args: &[&str], stdin: &[u8]) -> (u8, String, String) {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let (mut aus, mut fehler) = (Vec::new(), Vec::new());
        let code = lauf(&args, stdin, &mut aus, &mut fehler);
        (
            code,
            String::from_utf8(aus).expect("UTF-8"),
            String::from_utf8(fehler).expect("UTF-8"),
        )
    }

    fn tief(n: usize) -> Vec<u8> {
        let mut t = vec![b'['; n];
        t.extend(vec![b']'; n]);
        t
    }

    #[test]
    fn tiefe_grenze_bei_64() {
        assert!(!tiefe_ueber_grenze(&tief(0)));
        assert!(!tiefe_ueber_grenze(&tief(64)));
        assert!(tiefe_ueber_grenze(&tief(65)));
        let mut objekte = b"{\"a\":".repeat(64);
        objekte.extend(b"1");
        assert!(!tiefe_ueber_grenze(&objekte));
        objekte = b"{\"a\":".repeat(65);
        assert!(tiefe_ueber_grenze(&objekte));
        // Arrays and objects count together.
        let mut gemischt = b"[{\"a\":".repeat(32);
        gemischt.extend(b"1");
        assert!(!tiefe_ueber_grenze(&gemischt));
        gemischt = b"[{\"a\":".repeat(33);
        assert!(tiefe_ueber_grenze(&gemischt));
    }

    #[test]
    fn tiefe_zaehlt_wieder_herunter() {
        // Closing brackets lower the depth: two groups of 64 are fine.
        let mut zwei = vec![b'['];
        zwei.extend(tief(63));
        zwei.extend(tief(63));
        zwei.push(b']');
        assert!(!tiefe_ueber_grenze(&zwei));
        // 64 open, 1 closed, 2 open: depth 65 again.
        let mut wieder = vec![b'['; 64];
        wieder.push(b']');
        wieder.extend(vec![b'['; 2]);
        assert!(tiefe_ueber_grenze(&wieder));
        // More closers than openers do not wrap around.
        let mut viele = vec![b']'; 10];
        viele.extend(vec![b'['; 64]);
        assert!(!tiefe_ueber_grenze(&viele));
        viele.push(b'[');
        assert!(tiefe_ueber_grenze(&viele));
    }

    #[test]
    fn klammern_in_texten_zaehlen_nicht() {
        let klammern = "[".repeat(100);
        assert!(!tiefe_ueber_grenze(format!("\"{klammern}\"").as_bytes()));
        // An escaped quote does not end the text.
        assert!(!tiefe_ueber_grenze(
            format!("\"\\\"{klammern}\"").as_bytes()
        ));
        // An escaped backslash does not escape the closing quote: the text ends, the brackets after it count.
        assert!(tiefe_ueber_grenze(format!("\"\\\\\"{klammern}").as_bytes()));
        // The escape ends after one character.
        assert!(tiefe_ueber_grenze(format!("\"\\n\"{klammern}").as_bytes()));
        // A text opens and closes: the brackets after it count again.
        assert!(tiefe_ueber_grenze(format!("\"x\"{klammern}").as_bytes()));
        assert!(tiefe_ueber_grenze(
            format!("\"x\" \"y\"{klammern}").as_bytes()
        ));
    }

    #[test]
    fn grenze_der_eingabe() {
        let mut text = FALL.as_bytes().to_vec();
        text.resize(MAX_BYTES, b' ');
        assert!(verarbeite(&text).is_ok());
        text.push(b' ');
        assert!(verarbeite(&text).is_err());
    }

    #[test]
    fn lauf_liest_hoechstens_die_grenze_plus_eins() {
        struct Zaehler(usize);
        impl Read for Zaehler {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.0 += buf.len();
                buf.fill(b' ');
                Ok(buf.len())
            }
        }
        let mut z = Zaehler(0);
        let (mut aus, mut fehler) = (Vec::new(), Vec::new());
        assert_eq!(lauf(&[], &mut z, &mut aus, &mut fehler), 2);
        assert!(z.0 <= MAX_BYTES + 1 + 65_536, "read {} bytes", z.0);
        assert_eq!(aus.len(), 0);
    }

    #[test]
    fn exit_codes_und_ausgabe() {
        let (code, aus, fehler) = lauf_mit(&[], FALL.as_bytes());
        assert_eq!(code, 0);
        assert_eq!(fehler, "");
        let wert: Value = serde_json::from_str(&aus).expect("JSON");
        assert_eq!(
            wert,
            json!({"antworten": [{"darf": true, "grund": "frei", "klasse": "a", "meldung": "frei"}]})
        );
        assert_eq!(lauf_mit(&["-"], FALL.as_bytes()).0, 0);

        let (code, aus, fehler) = lauf_mit(&[], b"{");
        assert_eq!((code, aus.as_str()), (2, ""));
        assert!(fehler.starts_with("datentor: not JSON"), "{fehler:?}");
        assert_eq!(fehler.lines().count(), 1);
        assert!(fehler.ends_with('\n'));

        for args in [&["x"][..], &["-", "-"], &["", "-"], &["--"]] {
            let (code, aus, fehler) = lauf_mit(args, FALL.as_bytes());
            assert_eq!((code, aus.as_str()), (2, ""), "{args:?}");
            assert_eq!(
                fehler,
                "datentor: call: datentor [klassen|zonen|person] [-] < case.json\n"
            );
        }
    }

    #[test]
    fn lesefehler_ist_exit_2_und_schreibfehler_exit_1() {
        let (mut aus, mut fehler) = (Vec::new(), Vec::new());
        assert_eq!(lauf(&[], Kaputt, &mut aus, &mut fehler), 2);
        assert_eq!(aus.len(), 0);
        assert_eq!(
            String::from_utf8(fehler).expect("UTF-8"),
            "datentor: cannot read the input: broken\n"
        );

        let mut fehler = Vec::new();
        assert_eq!(lauf(&[], FALL.as_bytes(), &mut Voll, &mut fehler), 1);
        assert_eq!(
            String::from_utf8(fehler).expect("UTF-8"),
            "datentor: cannot write the output: full\n"
        );

        let mut fehler = Vec::new();
        assert_eq!(
            lauf(&[], FALL.as_bytes(), &mut NurFlushKaputt, &mut fehler),
            1
        );
        assert_eq!(
            String::from_utf8(fehler).expect("UTF-8"),
            "datentor: cannot write the output: flush\n"
        );
        // A failing stderr changes nothing about the code.
        assert_eq!(lauf(&[], &b"{"[..], &mut Vec::new(), &mut Voll), 2);
    }

    #[test]
    fn einzeilig_ersetzt_steuerzeichen() {
        assert_eq!(einzeilig("a\nb\rc\td\u{0}e\u{7f}f"), "a b c d e f");
        assert_eq!(einzeilig("Ünï cödé ok"), "Ünï cödé ok");
    }

    fn ungueltig(fall: &str) -> String {
        verarbeite(fall.as_bytes()).expect_err("case must be invalid")
    }

    /// A case with the given `vorgabe` and `anfragen` text.
    fn fall(vorgabe: &str, anfragen: &str) -> String {
        format!("{{\"vorgabe\":{vorgabe},\"anfragen\":{anfragen}}}")
    }

    const V: &str = r#"{"klassen":["a","b"],"routen":[{"schluessel":"r","art":"lokal","klasse":"b"}],"kunden":[{"schluessel":"k","erlaubt":["r"]}]}"#;

    #[test]
    fn form_des_falls() {
        assert!(verarbeite(fall(V, "[]").as_bytes()).is_ok());
        assert!(verarbeite(format!("{{\"name\":1,\"regeln\":2,\"beschreibung\":[[3]],\"vorgabe\":{V},\"anfragen\":[]}}").as_bytes()).is_ok());
        for roh in ["[]", "null", "1", "\"x\"", "{}", "{\"anfragen\":[]}"] {
            assert!(verarbeite(roh.as_bytes()).is_err(), "{roh}");
        }
        assert!(ungueltig(&format!("{{\"vorgabe\":{V}}}")).contains("anfragen is missing"));
        assert!(ungueltig(&fall(V, "{}")).contains("anfragen is not a list"));
        assert!(ungueltig(&fall(V, "null")).contains("anfragen is not a list"));
        assert!(ungueltig("{\"anfragen\":[]}").contains("vorgabe is missing"));
        assert!(
            ungueltig(&format!("{{\"vorgabe\":{V},\"anfragen\":[],\"x\":1}}"))
                .contains("unknown field \"x\"")
        );
        assert!(
            ungueltig(&format!("{{\"vorgabe\":{V},\"anfragen\":[],\"Name\":1}}"))
                .contains("unknown field")
        );
    }

    #[test]
    fn form_der_anfragen() {
        let ok = |a: &str| verarbeite(fall(V, &format!("[{a}]")).as_bytes());
        assert!(ok(r#"{"einheit":{}}"#).is_ok());
        assert!(ok(r#"{"einheit":{"klasse":null},"route":null,"kunde":null}"#).is_ok());
        assert!(ok(r#"{"einheit":{"klasse":"a"},"route":"r","kunde":"k"}"#).is_ok());
        for roh in [
            "1",
            "null",
            "[]",
            "\"x\"",
            "{}",
            r#"{"einheit":null}"#,
            r#"{"einheit":"a"}"#,
            r#"{"einheit":[]}"#,
            r#"{"einheit":{},"extra":1}"#,
            r#"{"einheit":{"klasse":"a","titel":"x"}}"#,
            r#"{"einheit":{},"route":1}"#,
            r#"{"einheit":{},"route":true}"#,
            r#"{"einheit":{},"route":[]}"#,
            r#"{"einheit":{},"kunde":1}"#,
            r#"{"einheit":{},"kunde":{}}"#,
        ] {
            assert!(ok(roh).is_err(), "{roh}");
        }
        // One bad request among good ones makes the whole case invalid.
        assert!(verarbeite(fall(V, r#"[{"einheit":{}},1]"#).as_bytes()).is_err());
    }

    #[test]
    fn klasse_die_kein_text_ist_endet_in_d4() {
        for k in ["5", "true", "[\"a\"]", "{\"a\":1}", "1.5", "false", "0"] {
            let aus = verarbeite(
                fall(
                    V,
                    &format!(r#"[{{"einheit":{{"klasse":{k}}},"route":"r"}}]"#),
                )
                .as_bytes(),
            )
            .expect("valid case");
            let wert: Value = serde_json::from_str(&aus).expect("JSON");
            assert_eq!(wert["antworten"][0]["grund"], "einheit-ohne-klasse", "{k}");
            assert_eq!(wert["antworten"][0]["klasse"], Value::Null);
        }
    }

    #[test]
    fn die_ausgabe_nennt_die_route_wie_uebergeben() {
        let aus =
            verarbeite(fall(V, r#"[{"einheit":{"klasse":"a"},"route":"Rä\n\"x"}]"#).as_bytes())
                .expect("valid case");
        let wert: Value = serde_json::from_str(&aus).expect("JSON");
        assert_eq!(
            wert["antworten"][0]["meldung"],
            "Route Rä\n\"x hat keine Klasse"
        );
        assert_eq!(wert["antworten"][0]["darf"], false);
        assert_eq!(wert["antworten"][0]["grund"], "route-ohne-klasse");
        assert_eq!(wert["antworten"][0]["klasse"], "a");
        // Order and number of the answers follow the requests.
        let aus = verarbeite(fall(V, r#"[{"einheit":{"klasse":"a"},"route":"r"},{"einheit":{"klasse":"b"},"route":"r"},{"einheit":{"klasse":"b"},"route":"r","kunde":"k"},{"einheit":{}}]"#).as_bytes()).expect("valid case");
        let wert: Value = serde_json::from_str(&aus).expect("JSON");
        let gruende: Vec<&str> = wert["antworten"]
            .as_array()
            .expect("list")
            .iter()
            .map(|a| a["grund"].as_str().expect("text"))
            .collect();
        assert_eq!(gruende, ["frei", "kunde-fehlt", "frei", "route-fehlt"]);
        assert_eq!(wert.as_object().expect("object").len(), 1);
    }

    #[test]
    fn form_der_vorgabe() {
        let mit = |v: &str| verarbeite(fall(v, "[]").as_bytes());
        assert!(mit(V).is_ok());
        // Minimal: only the required fields; optional fields may be missing.
        assert!(mit(r#"{"klassen":["a"],"routen":[],"kunden":[]}"#).is_ok());
        assert!(mit(r#"{"klassen":["a"],"routen":[],"kunden":[],"vorgabestufe":"a","kundendaten_an_cloud":{"vorgabe":"nie"}}"#).is_ok());
        assert!(mit(r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud"}],"kunden":[{"schluessel":"k"}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[{"route":"r","kunde":"k"}]}}"#).is_ok());
        assert!(mit(r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud","klasse":null}],"kunden":[]}"#).is_ok());
        for v in [
            "null",
            "[]",
            "1",
            "{}",
            r#"{"routen":[],"kunden":[]}"#,
            r#"{"klassen":["a"],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[]}"#,
            r#"{"klassen":[],"routen":[],"kunden":[]}"#,
            r#"{"klassen":"a","routen":[],"kunden":[]}"#,
            r#"{"klassen":[1],"routen":[],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":{},"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[1],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":{}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[1]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"x":1}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"vorgabestufe":null}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"vorgabestufe":1}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"vorgabestufe":"b"}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r"}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"art":"cloud"}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"Cloud"}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"x"}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":1}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":1,"art":"cloud"}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud","klasse":1}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud","klasse":"z"}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud","x":1}],"kunden":[]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[{}]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[{"schluessel":1}]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[{"schluessel":"k","x":1}]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[{"schluessel":"k","erlaubt":null}]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[{"schluessel":"k","erlaubt":"r"}]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[{"schluessel":"k","verboten":[1]}]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[{"schluessel":"k","verboten":["r"]}]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":null}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":[]}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{"freigaben":[]}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"immer"}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"Nie"}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{"vorgabe":null}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","x":1}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":{}}}"#,
            r#"{"klassen":["a"],"routen":[],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[1]}}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud"}],"kunden":[{"schluessel":"k"}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[{"route":"r"}]}}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud"}],"kunden":[{"schluessel":"k"}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[{"kunde":"k"}]}}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud"}],"kunden":[{"schluessel":"k"}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[{"route":"r","kunde":"k","x":1}]}}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud"}],"kunden":[{"schluessel":"k"}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[{"route":1,"kunde":"k"}]}}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"cloud"}],"kunden":[{"schluessel":"k"}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[{"route":"r","kunde":1}]}}"#,
            r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"lokal"}],"kunden":[{"schluessel":"k"}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[{"route":"r","kunde":"k"}]}}"#,
        ] {
            assert!(mit(v).is_err(), "{v}");
        }
    }

    #[test]
    fn vorgabefehler_steht_in_der_zeile() {
        let e = ungueltig(&fall(r#"{"klassen":["A"],"routen":[],"kunden":[]}"#, "[]"));
        assert_eq!(e, "vorgabe: key \"A\" does not have the key form");
    }

    #[test]
    fn eine_leere_klasse_im_fall_ist_nie_gueltig() {
        // A key cannot be empty, so the empty text of a non-text class never matches a class of the settings.
        let aus = verarbeite(fall(V, r#"[{"einheit":{"klasse":""},"route":"r"}]"#).as_bytes())
            .expect("valid case");
        assert!(aus.contains("einheit-ohne-klasse"));
    }

    /// The valid case with one object replaced by a variant that repeats a key.
    fn doppel_fall(vorgabe: &str, anfragen: &str, vor: &str) -> String {
        format!("{{{vor}\"vorgabe\":{vorgabe},\"anfragen\":{anfragen}}}")
    }

    #[test]
    #[allow(clippy::too_many_lines, reason = "one long table of cases")]
    fn doppelter_schluessel_macht_den_fall_ungueltig_auf_jeder_ebene() {
        let vk = r#""klassen":["a","b"]"#;
        let r = r#"{"schluessel":"r","art":"lokal","klasse":"b"}"#;
        let routen = format!("\"routen\":[{r}]");
        let kunden = r#""kunden":[{"schluessel":"k","erlaubt":["r"]}]"#;
        let ok_v = format!("{{{vk},{routen},{kunden}}}");
        let ok_a = r#"[{"einheit":{"klasse":"a"},"route":"r"}]"#;
        // Without a duplicate the case is valid.
        assert!(verarbeite(doppel_fall(&ok_v, ok_a, "").as_bytes()).is_ok());
        let faelle = [
            // the case itself, also with a different escape of the same key
            doppel_fall(&ok_v, ok_a, r#""name":1,"name":2,"#),
            doppel_fall(&ok_v, ok_a, r#""name":1,"name":2,"#),
            format!("{{\"vorgabe\":{ok_v},\"anfragen\":{ok_a},\"anfragen\":[]}}"),
            format!("{{\"vorgabe\":{ok_v},\"vorgabe\":{ok_v},\"anfragen\":[]}}"),
            // a field that is not evaluated, on any level
            doppel_fall(&ok_v, ok_a, r#""beschreibung":{"x":1,"x":2},"#),
            doppel_fall(&ok_v, ok_a, r#""beschreibung":[[{"x":1,"x":2}]],"#),
            doppel_fall(&ok_v, ok_a, r#""regeln":[{"y":null,"y":null}],"#),
            // the settings
            doppel_fall(&format!("{{{vk},{vk},{routen},{kunden}}}"), "[]", ""),
            doppel_fall(&format!("{{{vk},{routen},{routen},{kunden}}}"), "[]", ""),
            doppel_fall(
                &format!(
                    "{{{vk},{routen},{kunden},\"vorgabestufe\":\"a\",\"vorgabestufe\":\"a\"}}"
                ),
                "[]",
                "",
            ),
            // a route
            doppel_fall(
                &format!(
                    "{{{vk},\"routen\":[{{\"schluessel\":\"r\",\"art\":\"lokal\",\"art\":\"lokal\"}}],{kunden}}}"
                ),
                "[]",
                "",
            ),
            doppel_fall(
                &format!(
                    "{{{vk},\"routen\":[{{\"schluessel\":\"r\",\"art\":\"lokal\",\"klasse\":\"a\",\"klasse\":\"b\"}}],{kunden}}}"
                ),
                "[]",
                "",
            ),
            // a customer
            doppel_fall(
                &format!(
                    "{{{vk},{routen},\"kunden\":[{{\"schluessel\":\"k\",\"schluessel\":\"k\"}}]}}"
                ),
                "[]",
                "",
            ),
            doppel_fall(
                &format!(
                    "{{{vk},{routen},\"kunden\":[{{\"schluessel\":\"k\",\"erlaubt\":[],\"erlaubt\":[\"r\"]}}]}}"
                ),
                "[]",
                "",
            ),
            doppel_fall(
                &format!(
                    "{{{vk},{routen},\"kunden\":[{{\"schluessel\":\"k\",\"verboten\":[\"r\"],\"ver\u{62}oten\":[]}}]}}"
                ),
                "[]",
                "",
            ),
            // kundendaten_an_cloud and a release
            doppel_fall(
                &format!(
                    "{{{vk},\"routen\":[{{\"schluessel\":\"c\",\"art\":\"cloud\"}}],{kunden},\"kundendaten_an_cloud\":{{\"vorgabe\":\"nie\",\"vorgabe\":\"nie\"}}}}"
                ),
                "[]",
                "",
            ),
            doppel_fall(
                &format!(
                    "{{{vk},\"routen\":[{{\"schluessel\":\"c\",\"art\":\"cloud\"}}],{kunden},\"kundendaten_an_cloud\":{{\"vorgabe\":\"nie\",\"freigaben\":[],\"freigaben\":[]}}}}"
                ),
                "[]",
                "",
            ),
            doppel_fall(
                &format!(
                    "{{{vk},\"routen\":[{{\"schluessel\":\"c\",\"art\":\"cloud\"}}],\"kunden\":[{{\"schluessel\":\"k\",\"erlaubt\":[\"c\"]}}],\"kundendaten_an_cloud\":{{\"vorgabe\":\"nie\",\"freigaben\":[{{\"route\":\"c\",\"kunde\":\"k\",\"kunde\":\"k\"}}]}}}}"
                ),
                "[]",
                "",
            ),
            // a request and the unit
            doppel_fall(
                &ok_v,
                r#"[{"einheit":{"klasse":"a"},"route":"r","route":"r"}]"#,
                "",
            ),
            doppel_fall(
                &ok_v,
                r#"[{"einheit":{"klasse":"a"},"einheit":{"klasse":"a"},"route":"r"}]"#,
                "",
            ),
            doppel_fall(
                &ok_v,
                r#"[{"einheit":{"klasse":"a","klasse":"b"},"route":"r"}]"#,
                "",
            ),
            doppel_fall(
                &ok_v,
                r#"[{"einheit":{"klasse":"a"},"route":"r","kunde":null,"kunde":"k"}]"#,
                "",
            ),
            doppel_fall(
                &ok_v,
                r#"[{"einheit":{"klasse":"a"},"route":"r"},{"einheit":{},"einheit":{}}]"#,
                "",
            ),
            // after the first of two requests: the second is checked, too
            doppel_fall(
                &ok_v,
                r#"[{"einheit":{}},{"einheit":{},"route":null,"route":null}]"#,
                "",
            ),
        ];
        for text in &faelle {
            let e = ungueltig(text);
            assert!(e.contains("duplicate key"), "{text}: {e}");
            // the program says it in one line, with the exit code 2
            let (code, aus, fehler) = lauf_mit(&[], text.as_bytes());
            assert_eq!((code, aus.as_str()), (2, ""), "{text}");
            assert_eq!(fehler.lines().count(), 1, "{text}");
        }
    }

    #[test]
    fn gleiche_namen_in_verschiedenen_objekten_sind_kein_doppel() {
        // The key `klasse` appears in the settings and in the unit, `schluessel` in every route and customer.
        let aus = verarbeite(
            fall(
                r#"{"klassen":["a"],"routen":[{"schluessel":"r","art":"lokal","klasse":"a"},{"schluessel":"q","art":"lokal","klasse":"a"}],"kunden":[{"schluessel":"k"},{"schluessel":"l"}]}"#,
                r#"[{"einheit":{"klasse":"a"},"route":"r"},{"einheit":{"klasse":"a"},"route":"q"}]"#,
            )
            .as_bytes(),
        );
        assert!(aus.is_ok(), "{aus:?}");
    }
}
