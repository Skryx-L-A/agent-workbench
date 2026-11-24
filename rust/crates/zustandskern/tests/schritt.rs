//! The step API on its own: every case of `tests/faelle/` run event by event gives the output of `laufen`; the
//! next deadline is checked on cases of its own (frame rate limit R5, catch-up R14, a request, a synchronised
//! update) and, over all cases, by a chain: a `Takt` at the deadline gives the effect the next event would give.
//!
//! All times in the cases are given in the cases; no test reads a clock.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use zustandskern::{Sitzungskern, Ungueltig, Wirkungen, laufen};

fn faelle() -> Vec<(String, Vec<u8>)> {
    let ordner = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("faelle");
    let mut namen: Vec<String> = fs::read_dir(&ordner)
        .expect("case folder exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "json"))
        .collect();
    namen.sort();
    namen
        .into_iter()
        .map(|n| {
            let bytes = fs::read(ordner.join(&n)).expect("case is readable");
            (n, bytes)
        })
        .collect()
}

/// The output as the program writes it, as bytes.
fn bytes(aus: &Value) -> Vec<u8> {
    serde_json::to_vec(aus).expect("serialises")
}

/// The parts of a valid case: its start and its events.
fn zerlegen(fall: &[u8]) -> Option<(Value, Vec<Value>)> {
    let Value::Object(fall) = serde_json::from_slice(fall).ok()? else {
        return None;
    };
    let Some(Value::Array(ereignisse)) = fall.get("ereignisse") else {
        return None;
    };
    let anfang = fall.get("anfang").cloned().unwrap_or_else(|| json!({}));
    Some((anfang, ereignisse.clone()))
}

/// What `laufen` makes of a case, built from the public API: one core, one `schritt` per event.
fn schrittweise(fall: &[u8]) -> Result<Value, Ungueltig> {
    let Some((anfang, ereignisse)) = zerlegen(fall) else {
        return Err("kein Fall".into());
    };
    let mut kern = Sitzungskern::neu(&anfang)?;
    let mut wirkungen = Wirkungen::default();
    for e in &ereignisse {
        let schritt = kern.schritt(e)?;
        wirkungen.anhaengen(schritt);
    }
    let mut aus = wirkungen.als_wert();
    aus["zustand"] = kern.zustand();
    Ok(aus)
}

#[test]
fn every_case_run_step_by_step_gives_the_output_of_laufen() {
    let alle = faelle();
    assert!(alle.len() >= 250, "only {} cases found", alle.len());
    let (mut gleich, mut ungueltig) = (0, 0);
    let mut rot = Vec::new();
    for (name, fall) in &alle {
        match (laufen(fall), schrittweise(fall)) {
            (Ok(soll), Ok(ist)) => {
                if bytes(&ist) == bytes(&soll) {
                    gleich += 1;
                } else {
                    rot.push(format!("{name}: Ausgabe weicht ab"));
                }
            }
            (Err(_), Err(_)) => ungueltig += 1,
            (Ok(_), Err(e)) => rot.push(format!("{name}: Schritte scheitern, laufen nicht: {e}")),
            (Err(e), Ok(_)) => rot.push(format!("{name}: laufen scheitert ({e}), Schritte nicht")),
        }
    }
    assert!(rot.is_empty(), "{} rot:\n{}", rot.len(), rot.join("\n"));
    assert_eq!(gleich + ungueltig, alle.len());
    assert!(
        gleich >= 250 && ungueltig >= 1,
        "{gleich} gleich, {ungueltig} ungueltig"
    );
}

#[test]
fn the_deadline_lies_after_the_last_event_in_every_case() {
    let mut mit_frist = 0;
    for (name, fall) in faelle() {
        let Some((anfang, ereignisse)) = zerlegen(&fall) else {
            continue;
        };
        let Ok(mut kern) = Sitzungskern::neu(&anfang) else {
            continue;
        };
        for e in &ereignisse {
            if kern.schritt(e).is_err() {
                break;
            }
            let t = e["t"].as_i64().expect("whole number t");
            if let Some(frist) = kern.naechste_frist() {
                mit_frist += 1;
                assert!(frist > t, "{name}: Frist {frist} nicht nach t {t}");
            }
        }
    }
    assert!(mit_frist > 100, "only {mit_frist} steps with a deadline");
}

/// Runs the events of a case as `laufen` does, but before each event feeds a `Takt` at every deadline that lies
/// before the event. Returns the effects and the end state, with the index of each rejection mapped back to the
/// index in the case.
fn mit_takten(anfang: &Value, ereignisse: &[Value]) -> Option<(Value, Value)> {
    let mut kern = Sitzungskern::neu(anfang).ok()?;
    let mut wirkungen = Wirkungen::default();
    let mut urspruenglich: Vec<usize> = Vec::new();
    let mut zuletzt = -1;
    for (i, e) in ereignisse.iter().enumerate() {
        let t = e["t"].as_i64()?;
        while let Some(frist) = kern.naechste_frist().filter(|&f| f < t) {
            // A deadline that does not move on would loop here without end; fail at once instead.
            assert!(
                frist > zuletzt,
                "deadline {frist} not after the last Takt {zuletzt}"
            );
            zuletzt = frist;
            wirkungen.anhaengen(kern.schritt(&json!({"t": frist, "art": "Takt"})).ok()?);
            urspruenglich.push(usize::MAX);
        }
        wirkungen.anhaengen(kern.schritt(e).ok()?);
        urspruenglich.push(i);
    }
    for ablehnung in &mut wirkungen.ablehnungen {
        let neu = ablehnung["ereignis"].as_u64().expect("index");
        let alt = urspruenglich[usize::try_from(neu).expect("fits")];
        assert_ne!(alt, usize::MAX, "a Takt was rejected");
        ablehnung["ereignis"] = json!(alt);
    }
    let mut aus = wirkungen.als_wert();
    let zustand = kern.zustand();
    aus["zustand"] = zustand.clone();
    Some((aus, zustand))
}

#[test]
fn a_takt_at_each_deadline_gives_the_effects_of_the_case() {
    let mut mit_einschub = 0;
    for (name, fall) in faelle() {
        let Some((anfang, ereignisse)) = zerlegen(&fall) else {
            continue;
        };
        let Ok(soll) = laufen(&fall) else { continue };
        let (ist, _) = mit_takten(&anfang, &ereignisse).expect("steps run");
        assert_eq!(bytes(&ist), bytes(&soll), "{name}");
        // Count the cases where at least one deadline fell between two events.
        let Some(mut kern) = Sitzungskern::neu(&anfang).ok() else {
            continue;
        };
        let mut gesehen = false;
        for e in &ereignisse {
            let t = e["t"].as_i64().expect("t");
            gesehen |= kern.naechste_frist().is_some_and(|f| f < t);
            kern.schritt(e).expect("valid");
        }
        mit_einschub += i32::from(gesehen);
    }
    assert!(
        mit_einschub >= 20,
        "only {mit_einschub} cases had a deadline between events"
    );
}

// ---------- cases of its own for the deadline ----------

/// A start with one room `r`, one session `s` and the frame rate limit 10 per second (distance 100 ms).
fn anfang_bild() -> Value {
    json!({"personen": {"p": {}},
           "sitzungen": [{"id": "s", "kundenraum": "r", "groesse": [10, 4], "zeilen": ["", "", "", ""]}],
           "regeln": {"bildrate_max": {"firma": 10}}})
}

fn verbinden(kern: &mut Sitzungskern) {
    kern.schritt(
        &json!({"t": 0, "art": "Verbinden", "betrachter": "b", "person": "p", "raum": "r"}),
    )
    .expect("valid");
    kern.schritt(&json!({"t": 0, "art": "Abo", "betrachter": "b", "sitzung": "s", "spalten": 10, "zeilen": 4}))
        .expect("valid");
}

fn takt(kern: &mut Sitzungskern, t: i64) -> Wirkungen {
    kern.schritt(&json!({"t": t, "art": "Takt"}))
        .expect("valid")
}

/// The messages to viewer `b` of one kind.
fn art<'a>(w: &'a Wirkungen, art: &str) -> Vec<&'a Value> {
    w.nachrichten
        .get("b")
        .map(|liste| liste.iter().filter(|n| n["art"] == art).collect())
        .unwrap_or_default()
}

#[test]
fn a_new_core_has_no_deadline() {
    let kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    assert_eq!(kern.naechste_frist(), None);
    let mut kern = kern;
    verbinden(&mut kern);
    assert_eq!(kern.naechste_frist(), None);
}

#[test]
fn the_deadline_of_a_dispatch_is_the_end_of_the_distance_and_a_takt_there_sends() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    let erster = kern
        .schritt(
            &json!({"t": 0, "art": "Ausgabe", "sitzung": "s", "stand": 1, "zeilen": {"0": "a"}}),
        )
        .expect("valid");
    assert_eq!(
        art(&erster, "Rasteraenderung").len(),
        1,
        "the first change goes at once"
    );
    assert_eq!(kern.naechste_frist(), None);
    let zweiter = kern
        .schritt(
            &json!({"t": 5, "art": "Ausgabe", "sitzung": "s", "stand": 2, "zeilen": {"1": "b"}}),
        )
        .expect("valid");
    assert!(
        art(&zweiter, "Rasteraenderung").is_empty(),
        "the second waits"
    );
    assert_eq!(kern.naechste_frist(), Some(100));
    let davor = takt(&mut kern, 99);
    assert!(davor.ist_leer(), "nothing before the deadline");
    assert_eq!(
        kern.naechste_frist(),
        Some(100),
        "the deadline does not move"
    );
    let dann = takt(&mut kern, 100);
    let aenderung = art(&dann, "Rasteraenderung");
    assert_eq!(aenderung.len(), 1);
    assert_eq!(aenderung[0]["t"], 100);
    assert_eq!(aenderung[0]["von"], 1);
    assert_eq!(aenderung[0]["stand"], 2);
    assert_eq!(aenderung[0]["zeilen"], json!({"1": "b"}));
    assert_eq!(kern.naechste_frist(), None, "the dispatch is done");
}

#[test]
fn a_takt_after_the_deadline_gives_the_dispatch_at_the_time_of_the_deadline() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    for (t, stand) in [(0, 1), (5, 2)] {
        kern.schritt(&json!({"t": t, "art": "Ausgabe", "sitzung": "s", "stand": stand, "zeilen": {"0": "x"}}))
            .expect("valid");
    }
    let wirkung = takt(&mut kern, 400);
    assert_eq!(art(&wirkung, "Rasteraenderung")[0]["t"], 100);
}

#[test]
fn the_deadline_of_a_catch_up_is_the_end_of_the_distance_and_a_takt_there_catches_up() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    let schritt = |kern: &mut Sitzungskern, e: Value| kern.schritt(&e).expect("valid");
    schritt(
        &mut kern,
        json!({"t": 0, "art": "Ausgabe", "sitzung": "s", "stand": 1, "zeilen": {"0": "a"}}),
    );
    schritt(
        &mut kern,
        json!({"t": 1, "art": "Socket", "betrachter": "b", "ausstehend": 20000}),
    );
    schritt(
        &mut kern,
        json!({"t": 2, "art": "Ausgabe", "sitzung": "s", "stand": 2, "zeilen": {"1": "b"}}),
    );
    assert_eq!(kern.naechste_frist(), Some(100));
    let verfall = takt(&mut kern, 100);
    assert!(
        art(&verfall, "Rasteraenderung").is_empty(),
        "the viewer cannot take the image"
    );
    assert_eq!(kern.naechste_frist(), None);
    let nachgeholt = schritt(
        &mut kern,
        json!({"t": 150, "art": "Socket", "betrachter": "b", "ausstehend": 0}),
    );
    assert_eq!(
        art(&nachgeholt, "Rasteraenderung")[0]["stand"],
        2,
        "caught up at once"
    );
    schritt(
        &mut kern,
        json!({"t": 160, "art": "Ausgabe", "sitzung": "s", "stand": 3, "zeilen": {"2": "c"}}),
    );
    assert_eq!(
        kern.naechste_frist(),
        Some(200),
        "dispatch after the distance since t=100"
    );
    let versand = takt(&mut kern, 200);
    assert!(
        art(&versand, "Rasteraenderung").is_empty(),
        "inside the distance of the catch-up"
    );
    assert_eq!(
        kern.naechste_frist(),
        Some(250),
        "catch-up after the distance since t=150"
    );
    assert!(takt(&mut kern, 249).ist_leer());
    let holt = takt(&mut kern, 250);
    let aenderung = art(&holt, "Rasteraenderung");
    assert_eq!(aenderung.len(), 1);
    assert_eq!(aenderung[0]["t"], 250);
    assert_eq!(aenderung[0]["von"], 2);
    assert_eq!(aenderung[0]["stand"], 3);
    assert_eq!(kern.naechste_frist(), None);
}

#[test]
fn the_deadline_of_a_request_is_its_end_and_a_takt_there_lets_it_run_out() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    kern.schritt(&json!({"t": 5, "art": "Anfrage", "id": "a", "sitzung": "s", "werkzeug": "w", "vorschau": "v", "frist": 1000}))
        .expect("valid");
    assert_eq!(kern.naechste_frist(), Some(1000));
    assert!(takt(&mut kern, 999).ist_leer());
    let aus = takt(&mut kern, 1000);
    let melden = art(&aus, "Anfrage");
    assert_eq!(melden.len(), 1);
    assert_eq!(melden[0]["zustand"], "abgelaufen");
    assert_eq!(kern.zustand()["anfragen"]["a"], "abgelaufen");
    assert_eq!(kern.naechste_frist(), None);
}

#[test]
fn the_deadline_of_a_synchronised_update_is_150_ms_after_its_start() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    kern.schritt(&json!({"t": 10, "art": "SyncBeginn", "sitzung": "s"}))
        .expect("valid");
    assert_eq!(kern.naechste_frist(), Some(160));
    let gehalten = kern
        .schritt(
            &json!({"t": 20, "art": "Ausgabe", "sitzung": "s", "stand": 1, "zeilen": {"0": "a"}}),
        )
        .expect("valid");
    assert!(
        art(&gehalten, "Rasteraenderung").is_empty(),
        "held until the end"
    );
    let aus = takt(&mut kern, 160);
    assert_eq!(art(&aus, "Rasteraenderung").len(), 1);
    assert_eq!(kern.naechste_frist(), None);
}

#[test]
fn with_several_timers_the_deadline_is_the_smallest_and_moves_on() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    kern.schritt(&json!({"t": 1, "art": "Anfrage", "id": "a", "sitzung": "s", "werkzeug": "w", "vorschau": "v", "frist": 1000}))
        .expect("valid");
    for (t, stand) in [(2, 1), (3, 2)] {
        kern.schritt(&json!({"t": t, "art": "Ausgabe", "sitzung": "s", "stand": stand, "zeilen": {"0": "x"}}))
            .expect("valid");
    }
    assert_eq!(kern.naechste_frist(), Some(102));
    takt(&mut kern, 102);
    assert_eq!(kern.naechste_frist(), Some(1000));
}

// ---------- the step on its own ----------

#[test]
fn a_rejected_event_counts_with_its_index_over_all_steps() {
    let mut kern = Sitzungskern::neu(&json!({})).expect("valid");
    assert!(takt(&mut kern, 3).ist_leer());
    let a = kern.schritt(&json!({"t": 3, "art": "Nix"})).expect("valid");
    assert_eq!(
        a.ablehnungen,
        vec![json!({"t": 3, "ereignis": 1, "grund": "ereignis_unbekannt"})]
    );
    let b = kern.schritt(&json!({"t": 9, "art": "Nix"})).expect("valid");
    assert_eq!(
        b.ablehnungen,
        vec![json!({"t": 9, "ereignis": 2, "grund": "ereignis_unbekannt"})]
    );
}

#[test]
fn a_step_returns_only_its_own_effects() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    let erst = kern
        .schritt(
            &json!({"t": 0, "art": "Verbinden", "betrachter": "b", "person": "p", "raum": "r"}),
        )
        .expect("valid");
    assert!(!erst.ist_leer());
    let zweit = kern
        .schritt(&json!({"t": 1, "art": "Titel", "sitzung": "s", "text": "neu"}))
        .expect("valid");
    assert_eq!(zweit.nachrichten["b"].len(), 1, "only the list of sessions");
    assert_eq!(zweit.nachrichten["b"][0]["art"], "Sitzungsliste");
    assert!(takt(&mut kern, 2).ist_leer());
}

#[test]
fn an_invalid_event_is_an_error_and_leaves_the_core_as_it_was() {
    let mut kern = Sitzungskern::neu(&json!({})).expect("valid");
    takt(&mut kern, 5);
    let vorher = kern.zustand();
    for schlecht in [
        json!({"t": 4, "art": "Takt"}),
        json!({"art": "Takt"}),
        json!({"t": 6.5, "art": "Takt"}),
        json!({"t": "6", "art": "Takt"}),
        json!(3),
        json!(null),
        json!([{"t": 6}]),
    ] {
        let fehler = kern.schritt(&schlecht).expect_err("invalid");
        assert!(fehler.0.contains("Ereignis 1"), "{}", fehler.0);
    }
    assert_eq!(kern.zustand(), vorher);
    let a = kern.schritt(&json!({"t": 5, "art": "Nix"})).expect("valid");
    assert_eq!(
        a.ablehnungen[0]["ereignis"], 1,
        "the invalid events were not counted"
    );
}

#[test]
fn the_first_event_must_not_fall_below_zero() {
    let mut kern = Sitzungskern::neu(&json!({})).expect("valid");
    assert!(kern.schritt(&json!({"t": -1, "art": "Takt"})).is_err());
    assert!(kern.schritt(&json!({"t": 0, "art": "Takt"})).is_ok());
}

#[test]
fn the_start_must_be_an_object_and_well_formed() {
    for schlecht in [json!(null), json!(3), json!([]), json!("x")] {
        let fehler = Sitzungskern::neu(&schlecht).err().expect("invalid");
        assert!(fehler.0.contains("anfang"), "{}", fehler.0);
    }
    assert!(Sitzungskern::neu(&json!({"routen": [{}]})).is_err());
    assert!(Sitzungskern::neu(&json!({"sitzungen": [{"id": "s", "groesse": [1001, 1]}]})).is_err());
    assert!(Sitzungskern::neu(&json!({})).is_ok());
}

#[test]
fn the_limit_for_the_messages_counts_per_step_and_spends_the_core() {
    let anfang = json!({"personen": {"p": {}}, "sitzungen": [{"id": "s", "kundenraum": "r"}]});
    let verbinden =
        json!({"t": 0, "art": "Verbinden", "betrachter": "b", "person": "p", "raum": "r"});
    let titel = |t: i64| json!({"t": t, "art": "Titel", "sitzung": "s", "text": "neu"});

    // The bytes the connecting needs, as the first step spends them.
    let mut probe = Sitzungskern::neu(&anfang).expect("valid");
    let summe: usize = probe
        .schritt(&verbinden)
        .expect("valid")
        .nachrichten
        .values()
        .flatten()
        .map(|n| serde_json::to_vec(n).expect("serialises").len())
        .sum();
    assert!(summe > 100, "{summe}");

    // Exactly the limit passes, and the limit starts anew with every step.
    let mut kern = Sitzungskern::neu_mit(&anfang, summe).expect("valid");
    kern.schritt(&verbinden).expect("at the limit");
    for t in 1..4 {
        let _ = kern
            .schritt(&titel(t))
            .expect("a small step under the limit");
    }

    // One byte less: the step is an error, and the core refuses every later step.
    let mut kern = Sitzungskern::neu_mit(&anfang, summe - 1).expect("valid");
    let fehler = kern.schritt(&verbinden).expect_err("over the limit");
    assert!(fehler.0.contains(&format!("{}", summe - 1)), "{}", fehler.0);
    let spaeter = kern
        .schritt(&json!({"t": 1, "art": "Takt"}))
        .expect_err("spent");
    assert!(spaeter.0.contains("verbraucht"), "{}", spaeter.0);

    // A limit of 0 passes while no message is made.
    let mut leer = Sitzungskern::neu_mit(&anfang, 0).expect("valid");
    assert!(leer.schritt(&json!({"t": 0, "art": "Takt"})).is_ok());
}

#[test]
fn the_default_limit_is_the_limit_of_the_contract() {
    assert_eq!(zustandskern::NACHRICHTEN_MAX, 33_554_432);
    let anfang = json!({"personen": {"p": {}}, "sitzungen": [{"id": "s", "kundenraum": "r"}]});
    let mut kern = Sitzungskern::neu(&anfang).expect("valid");
    let w = kern
        .schritt(
            &json!({"t": 0, "art": "Verbinden", "betrachter": "b", "person": "p", "raum": "r"}),
        )
        .expect("valid");
    assert!(!w.ist_leer());
}

#[test]
fn the_effects_append_in_order_and_show_as_the_keys_of_the_output() {
    let mut a = Wirkungen::default();
    assert!(a.ist_leer());
    let mut b = Wirkungen::default();
    b.nachrichten.insert("v1".into(), vec![json!(1)]);
    b.pty.push(json!("p"));
    b.groessen.push(json!("g"));
    b.ablehnungen.push(json!("a"));
    b.protokoll.push(json!("l"));
    b.an_stelle.push(json!("s"));
    b.abfragen.push(json!("f"));
    let mut c = Wirkungen::default();
    c.nachrichten.insert("v1".into(), vec![json!(2)]);
    c.nachrichten.insert("v2".into(), vec![json!(3)]);
    c.pty.push(json!("p2"));
    for (i, leer) in [
        Wirkungen {
            pty: vec![json!(1)],
            ..Wirkungen::default()
        },
        Wirkungen {
            groessen: vec![json!(1)],
            ..Wirkungen::default()
        },
        Wirkungen {
            ablehnungen: vec![json!(1)],
            ..Wirkungen::default()
        },
        Wirkungen {
            protokoll: vec![json!(1)],
            ..Wirkungen::default()
        },
        Wirkungen {
            an_stelle: vec![json!(1)],
            ..Wirkungen::default()
        },
        Wirkungen {
            abfragen: vec![json!(1)],
            ..Wirkungen::default()
        },
        Wirkungen {
            nachrichten: BTreeMap::from([("v".to_owned(), vec![])]),
            ..Wirkungen::default()
        },
    ]
    .iter()
    .enumerate()
    {
        assert!(!leer.ist_leer(), "field {i} counts");
    }
    a.anhaengen(b.clone());
    assert_eq!(a, b);
    a.anhaengen(c);
    assert!(!a.ist_leer());
    assert_eq!(
        a.als_wert(),
        json!({"nachrichten": {"v1": [1, 2], "v2": [3]}, "pty": ["p", "p2"], "groessen": ["g"],
               "ablehnungen": ["a"], "protokoll": ["l"], "an_stelle": ["s"], "abfragen": ["f"]})
    );
}

// ---------- the cursor in the step API (R15) ----------

/// The `Rasteraenderung` and `Abzug` messages to viewer `b`.
fn bilder(w: &Wirkungen) -> Vec<&Value> {
    w.nachrichten
        .get("b")
        .map(|liste| {
            liste
                .iter()
                .filter(|n| n["art"] == "Rasteraenderung" || n["art"] == "Abzug")
                .collect()
        })
        .unwrap_or_default()
}

fn cursor_schritt(kern: &mut Sitzungskern, t: i64, stand: i64, cursor: &Value) -> Wirkungen {
    kern.schritt(&json!({"t": t, "art": "Ausgabe", "sitzung": "s", "stand": stand, "zeilen": {}, "cursor": cursor}))
        .expect("valid")
}

#[test]
fn a_pure_cursor_step_goes_through_the_distance_of_the_frame_rate_limit() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    let erster = cursor_schritt(&mut kern, 0, 1, &json!([1, 2, true]));
    let bild = bilder(&erster);
    assert_eq!(bild.len(), 1, "the first step goes at once");
    assert_eq!(bild[0]["art"], "Rasteraenderung");
    assert_eq!(bild[0]["zeilen"], json!({}));
    assert_eq!(bild[0]["cursor"], json!([1, 2, true]));
    assert_eq!(kern.naechste_frist(), None);
    for (t, stand, spalte) in [(5, 2, 3), (6, 3, 4), (99, 4, 5)] {
        let w = cursor_schritt(&mut kern, t, stand, &json!([1, spalte, true]));
        assert!(
            bilder(&w).is_empty(),
            "a step at {t} waits for the distance"
        );
        assert_eq!(kern.naechste_frist(), Some(100));
    }
    let dann = takt(&mut kern, 100);
    let bild = bilder(&dann);
    assert_eq!(bild.len(), 1, "one change for the whole storm");
    assert_eq!(bild[0]["t"], 100);
    assert_eq!(bild[0]["von"], 1);
    assert_eq!(bild[0]["stand"], 4);
    assert_eq!(bild[0]["zeilen"], json!({}));
    assert_eq!(
        bild[0]["cursor"],
        json!([1, 5, true]),
        "the latest cursor counts"
    );
    assert_eq!(kern.naechste_frist(), None);
}

#[test]
fn a_new_core_takes_the_cursor_of_the_end_state_as_its_start() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    assert_eq!(kern.zustand()["sitzungen"][0].get("cursor"), None);
    verbinden(&mut kern);
    cursor_schritt(&mut kern, 0, 1, &json!([3, 9, false]));
    let zustand = kern.zustand();
    assert_eq!(zustand["sitzungen"][0]["cursor"], json!([3, 9, false]));
    let mut anfang = anfang_bild();
    anfang["sitzungen"][0]["cursor"] = zustand["sitzungen"][0]["cursor"].clone();
    let mut neu = Sitzungskern::neu(&anfang).expect("valid");
    assert_eq!(
        neu.zustand()["sitzungen"][0]["cursor"],
        json!([3, 9, false])
    );
    let abo = neu
        .schritt(&json!({"t": 0, "art": "Verbinden", "betrachter": "b", "person": "p", "raum": "r"}))
        .and_then(|_| neu.schritt(&json!({"t": 0, "art": "Abo", "betrachter": "b", "sitzung": "s", "spalten": 10, "zeilen": 4})))
        .expect("valid");
    assert_eq!(art(&abo, "Abzug")[0]["cursor"], json!([3, 9, false]));
}

#[test]
fn a_start_with_an_invalid_cursor_is_an_error() {
    let mit = |cursor: Value| {
        let mut anfang = anfang_bild();
        anfang["sitzungen"][0]["cursor"] = cursor;
        Sitzungskern::neu(&anfang).is_ok()
    };
    assert!(mit(json!([3, 9, true])), "the last row and column");
    assert!(!mit(json!([4, 0, true])));
    assert!(!mit(json!([0, 10, true])));
    assert!(!mit(json!([-1, 0, true])));
    assert!(!mit(json!([0, -1, true])));
    assert!(!mit(json!(null)));
    assert!(!mit(json!([0, 0])));
    assert!(!mit(json!([0, 0, 1])));
    assert!(!mit(json!([0.0, 0, true])));
    assert!(!mit(json!("x")));
}

#[test]
fn an_invalid_cursor_rejects_the_whole_event_before_the_stand_is_looked_at() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    cursor_schritt(&mut kern, 0, 5, &json!([0, 0, true]));
    let falsch = kern
        .schritt(&json!({"t": 1, "art": "Ausgabe", "sitzung": "s", "stand": 2, "zeilen": {"0": "x"}, "cursor": [4, 0, true]}))
        .expect("valid");
    assert_eq!(falsch.ablehnungen.len(), 1);
    assert_eq!(falsch.ablehnungen[0]["grund"], "ereignis_ungueltig");
    assert!(falsch.nachrichten.is_empty());
    let alt = cursor_schritt(&mut kern, 2, 2, &json!([0, 1, true]));
    assert_eq!(alt.ablehnungen[0]["grund"], "stand_alt");
    assert_eq!(
        kern.zustand()["sitzungen"][0]["cursor"],
        json!([0, 0, true])
    );
    assert_eq!(kern.zustand()["sitzungen"][0]["stand"], 5);
}

#[test]
fn a_size_change_clamps_the_cursor_before_the_full_image_goes_out() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    cursor_schritt(&mut kern, 0, 1, &json!([3, 9, true]));
    kern.schritt(&json!({"t": 1, "art": "Text", "betrachter": "b", "sitzung": "s", "text": "x"}))
        .expect("valid");
    let w = kern
        .schritt(&json!({"t": 2, "art": "Groesse", "betrachter": "b", "sitzung": "s", "spalten": 5, "zeilen": 2}))
        .expect("valid");
    let abzug = art(&w, "Abzug");
    assert_eq!(abzug.len(), 1);
    assert_eq!(abzug[0]["groesse"], json!([5, 2]));
    assert_eq!(abzug[0]["cursor"], json!([1, 4, true]));
    assert_eq!(
        kern.zustand()["sitzungen"][0]["cursor"],
        json!([1, 4, true])
    );
}

#[test]
fn a_cursor_held_in_a_synchronised_update_goes_out_at_the_release_only() {
    let mut kern = Sitzungskern::neu(&anfang_bild()).expect("valid");
    verbinden(&mut kern);
    cursor_schritt(&mut kern, 0, 1, &json!([0, 0, true]));
    kern.schritt(&json!({"t": 200, "art": "SyncBeginn", "sitzung": "s"}))
        .expect("valid");
    let still = cursor_schritt(&mut kern, 201, 2, &json!([2, 3, true]));
    assert!(
        bilder(&still).is_empty(),
        "nothing goes out inside DEC 2026"
    );
    assert_eq!(
        kern.zustand()["sitzungen"][0]["cursor"],
        json!([0, 0, true])
    );
    assert_eq!(kern.naechste_frist(), Some(350));
    let frei = takt(&mut kern, 350);
    let bild = bilder(&frei);
    assert_eq!(bild.len(), 1);
    assert_eq!(bild[0]["cursor"], json!([2, 3, true]));
    assert_eq!(bild[0]["zeilen"], json!({}));
}
