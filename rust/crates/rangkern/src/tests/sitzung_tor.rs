//! The gate in front of the output: the judgement of a note, the cut after the gate, the groups of the log, and the
//! references from an allowed hit to a blocked note.

use std::collections::HashMap;

use serde_json::json;

use crate::cli::hit_json;
use crate::hit::SearchHit;
use crate::notes::{NoteRec, note_rec};
use crate::sitzung::einst;
use crate::sitzung::tor::{Durchgelassen, Gruppe, Paar, Tor};

/// The settings of the tests: `eng` sees `oeffentlich`, `cloud` sees `intern`, `lokal` sees everything for `k-nord`.
fn tor(route: &str) -> Tor {
    let einstellungen = json!({
        "fassung": 1,
        "klassen": ["oeffentlich", "intern", "kundendaten"],
        "vorgabestufe": "kundendaten",
        "routen": [
            {"schluessel": "eng", "art": "cloud", "klasse": "oeffentlich"},
            {"schluessel": "cloud", "art": "cloud", "klasse": "intern"},
            {"schluessel": "lokal", "art": "lokal", "klasse": "kundendaten"}
        ],
        "kunden": [{"schluessel": "k-nord", "erlaubt": ["lokal"], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    });
    let einst = einst::lesen(einstellungen.to_string().as_bytes()).expect("valid settings");
    Tor::new(einst.vorgabe, route.to_string())
}

fn note(rel: &str, front: &str) -> NoteRec {
    note_rec(rel, format!("---\ntitle: T\n{front}\n---\nText\n"))
}

fn urteil(route: &str, front: &str) -> (&'static str, Option<String>, Option<String>) {
    let n = note("a.md", front);
    let u = tor(route).urteil(Some(&n));
    (u.grund, u.klasse, u.kunde)
}

#[allow(clippy::unnecessary_wraps)]
fn s(text: &str) -> Option<String> {
    Some(text.to_string())
}

#[test]
fn the_judgement_follows_class_and_customer_of_the_note() {
    assert_eq!(
        urteil("cloud", "datenklasse: intern"),
        ("frei", s("intern"), None)
    );
    assert_eq!(
        urteil("cloud", "datenklasse: kundendaten"),
        ("klasse-ueber-route", s("kundendaten"), None)
    );
    // No class, or a class without a value: the vorgabestufe of the settings, the highest class.
    assert_eq!(
        urteil("cloud", "x: 1"),
        ("klasse-ueber-route", s("kundendaten"), None)
    );
    assert_eq!(
        urteil("cloud", "datenklasse:"),
        ("klasse-ueber-route", s("kundendaten"), None)
    );
    assert_eq!(
        urteil("cloud", "datenklasse: null"),
        ("klasse-ueber-route", s("kundendaten"), None)
    );
    // A class that is no text, an empty text and an unknown class: blocked, with no class in the log.
    for front in [
        "datenklasse: [intern]",
        "datenklasse: 5",
        "datenklasse: true",
        "datenklasse: \"\"",
        "datenklasse: geheim",
        "datenklasse: Intern",
    ] {
        assert_eq!(
            urteil("lokal", front),
            ("einheit-ohne-klasse", None, None),
            "{front}"
        );
    }
    // A customer makes the effective class the highest: not for `cloud`, for `lokal` if the customer allows it.
    assert_eq!(
        urteil("cloud", "datenklasse: intern\nkunde: k-nord"),
        ("klasse-ueber-route", s("kundendaten"), s("k-nord"))
    );
    assert_eq!(
        urteil("lokal", "datenklasse: oeffentlich\nkunde: k-nord"),
        ("frei", s("kundendaten"), s("k-nord"))
    );
    // A customer without an entry: the log keeps its key if it has the key form, and nothing if it has not.
    assert_eq!(
        urteil("lokal", "datenklasse: intern\nkunde: k-x"),
        ("kunde-ohne-eintrag", s("kundendaten"), s("k-x"))
    );
    for front in [
        "datenklasse: intern\nkunde: K-Nord",
        "datenklasse: intern\nkunde: [k-nord]",
        "datenklasse: intern\nkunde: 5",
        "datenklasse: intern\nkunde: \"\"",
        "datenklasse: intern\nkunde: \"k nord\"",
    ] {
        assert_eq!(
            urteil("lokal", front),
            ("kunde-ohne-eintrag", s("kundendaten"), None),
            "{front}"
        );
    }
    // `kunde:` without a value or `null` is no customer.
    assert_eq!(
        urteil("lokal", "datenklasse: intern\nkunde:"),
        ("frei", s("intern"), None)
    );
    assert_eq!(
        urteil("lokal", "datenklasse: intern\nkunde: null"),
        ("frei", s("intern"), None)
    );
    // Customer data without a customer.
    assert_eq!(
        urteil("lokal", "datenklasse: kundendaten"),
        ("kunde-fehlt", s("kundendaten"), None)
    );
    // The field `class` of the vault has nothing to do with it.
    assert_eq!(
        urteil("cloud", "datenklasse: intern\nclass: source"),
        ("frei", s("intern"), None)
    );
}

#[test]
fn a_note_that_is_not_in_the_index_is_blocked() {
    let u = tor("cloud").urteil(None);
    assert!(!u.darf());
    assert_eq!(
        (u.grund, u.klasse, u.kunde),
        ("einheit-ohne-klasse", None, None)
    );
    let frei = note("a.md", "datenklasse: oeffentlich");
    assert!(tor("cloud").urteil(Some(&frei)).darf());
}

/// A list of hits `rel` with their notes: a name that starts with `b` is blocked on `cloud`, one with `a` is allowed.
struct Lage {
    notes: Vec<NoteRec>,
}

impl Lage {
    fn new(eintraege: &[(&str, &str)]) -> Self {
        Self {
            notes: eintraege
                .iter()
                .map(|(rel, front)| note(rel, front))
                .collect(),
        }
    }

    fn map(&self) -> HashMap<&str, &NoteRec> {
        self.notes.iter().map(|n| (n.rel.as_str(), n)).collect()
    }

    fn liste(rels: &[&str]) -> Vec<SearchHit> {
        rels.iter()
            .enumerate()
            .map(|(i, rel)| {
                SearchHit::new(
                    rel,
                    10.0 - f64::from(u8::try_from(i).expect("a short list")),
                )
            })
            .collect()
    }

    fn durch(&self, route: &str, rels: &[&str], k: usize, paare: &[Paar]) -> Durchgelassen {
        tor(route).durchlassen(Self::liste(rels), k, &self.map(), paare)
    }
}

fn rels(durch: &Durchgelassen) -> Vec<&str> {
    durch.hits.iter().map(|h| h.rel.as_str()).collect()
}

const ERLAUBT: &str = "datenklasse: intern";
const GESPERRT: &str = "datenklasse: kundendaten\nkunde: k-nord";

#[test]
fn the_gate_acts_before_the_cut_and_blocked_hits_take_no_place() {
    let lage = Lage::new(&[
        ("b1.md", GESPERRT),
        ("a1.md", ERLAUBT),
        ("b2.md", GESPERRT),
        ("a2.md", ERLAUBT),
        ("a3.md", ERLAUBT),
    ]);
    let liste = ["b1.md", "a1.md", "b2.md", "a2.md", "a3.md"];
    let erwartet: [(usize, &[&str], usize); 6] = [
        (1, &["a1.md"], 1),
        (2, &["a1.md", "a2.md"], 1),
        (3, &["a1.md", "a2.md", "a3.md"], 2),
        (4, &["a1.md", "a2.md", "a3.md"], 2),
        (5, &["a1.md", "a2.md", "a3.md"], 2),
        (50, &["a1.md", "a2.md", "a3.md"], 2),
    ];
    for (k, soll, gesperrt) in erwartet {
        let durch = lage.durch("cloud", &liste, k, &[]);
        assert_eq!(rels(&durch), soll, "k {k}");
        assert_eq!(durch.gesperrt(), gesperrt, "k {k}");
    }
    // A hit keeps its place in the order of the list.
    let durch = lage.durch("cloud", &["a3.md", "a1.md", "a2.md"], 2, &[]);
    assert_eq!(rels(&durch), ["a3.md", "a1.md"]);
    assert_eq!(durch.gesperrt(), 0);
    assert!(durch.gruppen.is_empty());
    // An empty list gives nothing.
    let durch = lage.durch("cloud", &[], 5, &[]);
    assert!(durch.hits.is_empty() && durch.gruppen.is_empty());
    assert_eq!(durch.gesperrt(), 0);
}

#[test]
fn the_blocked_hits_form_groups_by_class_customer_and_reason_in_the_order_of_the_list() {
    let lage = Lage::new(&[
        // klasse-ueber-route, kundendaten, no customer (the class is missing, so the vorgabestufe is taken).
        ("g1.md", "x: 1"),
        // The same group again.
        ("g1b.md", "x: 2"),
        // klasse-ueber-route, kundendaten, customer k-nord.
        ("g2.md", GESPERRT),
        // einheit-ohne-klasse.
        ("g3.md", "datenklasse: geheim"),
        // The same reason and class as g2, no customer.
        ("g4.md", "datenklasse: kundendaten"),
        // klasse-ueber-route with the class `intern` on the route `eng`.
        ("g5.md", "datenklasse: intern"),
        ("g6.md", "datenklasse: kundendaten"),
        ("g2b.md", GESPERRT),
        ("a.md", "datenklasse: oeffentlich"),
    ]);
    let alle = [
        "g1.md", "g1b.md", "g2.md", "g3.md", "g4.md", "g5.md", "g6.md", "g2b.md", "a.md",
    ];
    // On `cloud`: g5 is allowed (intern).
    let durch = lage.durch("cloud", &alle, 50, &[]);
    assert_eq!(rels(&durch), ["g5.md", "a.md"]);
    let gruppe =
        |klasse: Option<&str>, kunde: Option<&str>, grund: &'static str, anzahl: u32| Gruppe {
            klasse: klasse.map(str::to_string),
            kunde: kunde.map(str::to_string),
            grund,
            anzahl,
        };
    assert_eq!(
        durch.gruppen,
        [
            gruppe(Some("kundendaten"), None, "klasse-ueber-route", 4),
            gruppe(Some("kundendaten"), Some("k-nord"), "klasse-ueber-route", 2),
            gruppe(None, None, "einheit-ohne-klasse", 1),
        ]
    );
    assert_eq!(durch.gesperrt(), 7);
    // On `eng`: g5 (intern) is blocked with the class `intern`, a group of its own.
    let durch = lage.durch("eng", &alle, 50, &[]);
    assert_eq!(rels(&durch), ["a.md"]);
    assert_eq!(
        durch.gruppen,
        [
            gruppe(Some("kundendaten"), None, "klasse-ueber-route", 4),
            gruppe(Some("kundendaten"), Some("k-nord"), "klasse-ueber-route", 2),
            gruppe(None, None, "einheit-ohne-klasse", 1),
            gruppe(Some("intern"), None, "klasse-ueber-route", 1),
        ]
    );
    // Only the first `k` of the list count: g1, g1b, g2.
    let durch = lage.durch("eng", &alle, 3, &[]);
    assert_eq!(
        durch.gruppen,
        [
            gruppe(Some("kundendaten"), None, "klasse-ueber-route", 2),
            gruppe(Some("kundendaten"), Some("k-nord"), "klasse-ueber-route", 1),
        ]
    );
    assert_eq!(durch.gesperrt(), 3);
    assert_eq!(durch.hits.len(), 1);
}

#[test]
fn groups_differ_by_the_reason_alone() {
    let lage = Lage::new(&[
        ("g1.md", "datenklasse: kundendaten"),
        ("g2.md", "datenklasse: kundendaten\nkunde: K-X"),
    ]);
    let durch = lage.durch("lokal", &["g1.md", "g2.md"], 5, &[]);
    assert!(durch.hits.is_empty());
    let gruende: Vec<&str> = durch.gruppen.iter().map(|g| g.grund).collect();
    assert_eq!(gruende, ["kunde-fehlt", "kunde-ohne-eintrag"]);
    assert!(durch.gruppen.iter().all(|g| g.anzahl == 1));
}

#[test]
fn a_hit_without_a_note_in_the_index_is_blocked_and_logged_without_class() {
    let lage = Lage::new(&[("a.md", ERLAUBT)]);
    let durch = lage.durch("cloud", &["weg.md", "a.md"], 5, &[]);
    assert_eq!(rels(&durch), ["a.md"]);
    assert_eq!(
        durch.gruppen,
        [Gruppe {
            klasse: None,
            kunde: None,
            grund: "einheit-ohne-klasse",
            anzahl: 1
        }]
    );
}

fn mit_nachfolgern(rel: &str, aufgeloest: &[&str], geschrieben: &[&str]) -> SearchHit {
    let mut hit = SearchHit::new(rel, 1.0);
    hit.status = "superseded".to_string();
    hit.superseded_by_rel = aufgeloest.iter().map(ToString::to_string).collect();
    hit.superseded_by = geschrieben.iter().map(ToString::to_string).collect();
    hit
}

#[test]
fn a_successor_that_is_blocked_does_not_appear_in_the_hit() {
    let lage = Lage::new(&[
        ("alt.md", ERLAUBT),
        ("neu-a.md", ERLAUBT),
        ("neu-b.md", GESPERRT),
    ]);
    let gezeigt = |hit: SearchHit| -> serde_json::Value {
        let durch = tor("cloud").durchlassen(vec![hit], 5, &lage.map(), &[]);
        assert_eq!(durch.hits.len(), 1);
        hit_json(&durch.hits[0])
    };
    // Resolved paths: only the allowed ones stay.
    let json = gezeigt(mit_nachfolgern(
        "alt.md",
        &["neu-b.md", "neu-a.md"],
        &["[[neu-b]]", "[[neu-a]]"],
    ));
    assert_eq!(json["abgeloest_durch"], json!(["neu-a.md"]));
    assert_eq!(json["status"], "superseded");
    // Only blocked ones: the field is gone, the status stays, and the text as written does not come back.
    let json = gezeigt(mit_nachfolgern("alt.md", &["neu-b.md"], &["[[neu-b]]"]));
    assert!(json.get("abgeloest_durch").is_none());
    assert_eq!(json["status"], "superseded");
    assert!(!json.to_string().contains("neu-b"));
    // Resolved paths of notes the index does not hold are not shown either.
    let json = gezeigt(mit_nachfolgern("alt.md", &["weg.md"], &["[[weg]]"]));
    assert!(json.get("abgeloest_durch").is_none());
    // Without resolved paths the text as written would show; it is no allowed note, so it is dropped.
    let json = gezeigt(mit_nachfolgern("alt.md", &[], &["[[neu-a]]"]));
    assert!(json.get("abgeloest_durch").is_none());
    // Nothing to filter: the allowed successor stays.
    let json = gezeigt(mit_nachfolgern("alt.md", &["neu-a.md"], &["[[neu-a]]"]));
    assert_eq!(json["abgeloest_durch"], json!(["neu-a.md"]));
    // A hit without a successor has no field.
    let json = gezeigt(SearchHit::new("alt.md", 1.0));
    assert!(json.get("abgeloest_durch").is_none());
}

fn paar(a: &str, b: &str) -> Paar {
    (Some(a.to_string()), Some(b.to_string()))
}

#[test]
fn a_contradiction_stays_only_with_an_allowed_partner() {
    let lage = Lage::new(&[
        ("a.md", ERLAUBT),
        ("a2.md", ERLAUBT),
        ("b.md", GESPERRT),
        ("c.md", ERLAUBT),
    ]);
    let markiert = |paare: &[Paar]| -> bool {
        let mut hit = SearchHit::new("a.md", 1.0);
        hit.contradiction = true;
        let durch = tor("cloud").durchlassen(vec![hit], 5, &lage.map(), paare);
        durch.hits[0].contradiction
    };
    // The partner is allowed, on either side of the finding.
    assert!(markiert(&[paar("a.md", "a2.md")]));
    assert!(markiert(&[paar("a2.md", "a.md")]));
    // The partner is blocked, or not in the index, or the finding names none.
    assert!(!markiert(&[paar("a.md", "b.md")]));
    assert!(!markiert(&[paar("b.md", "a.md")]));
    assert!(!markiert(&[paar("a.md", "weg.md")]));
    assert!(!markiert(&[(Some("a.md".to_string()), None)]));
    assert!(!markiert(&[(None, Some("a.md".to_string()))]));
    // Another finding with an allowed note does not count: only a finding of this hit.
    assert!(!markiert(&[paar("a.md", "b.md"), paar("c.md", "a2.md")]));
    // One allowed partner is enough, in any finding.
    assert!(markiert(&[paar("a.md", "b.md"), paar("c.md", "a.md")]));
    assert!(markiert(&[paar("a.md", "b.md"), paar("a.md", "c.md")]));
    // No finding at all although the hit was marked: the mark goes.
    assert!(!markiert(&[]));
    // A hit that was not marked stays unmarked.
    let durch = tor("cloud").durchlassen(
        vec![SearchHit::new("a.md", 1.0)],
        5,
        &lage.map(),
        &[paar("a.md", "a2.md")],
    );
    assert!(!durch.hits[0].contradiction);
}
