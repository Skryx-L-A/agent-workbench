//! Unit tests of the read class of the person (partial contract `person/VERTRAG.md`, sections 3, 4, 5 and 8, and the
//! Nachtrag 27.11.2026: read without a route, limit of the entries). All data are invented.

use datentor::person::{
    LESEKLASSEN_MAX, Leseeintrag, Person, PersonAntwort, PersonEbene, PersonGrund, Personen,
    PersonenFehler, darf_person, lesen_person,
};
use datentor::{Art, Einheit, Freigabe, Grund, Kunde, Route, Vorgabe, darf};

fn s(x: &str) -> String {
    x.to_owned()
}

fn route(schluessel: &str, art: Art, klasse: Option<&str>) -> Route {
    Route {
        schluessel: s(schluessel),
        art,
        klasse: klasse.map(s),
    }
}

/// Three classes a < b < c; routes of every class and kind, one without a class; customers with every list.
fn vorgabe() -> Vorgabe {
    Vorgabe::neu(
        vec![s("a"), s("b"), s("c")],
        Some(s("b")),
        vec![
            route("la", Art::Lokal, Some("a")),
            route("lb", Art::Lokal, Some("b")),
            route("lc", Art::Lokal, Some("c")),
            route("cc", Art::Cloud, Some("c")),
            route("ca", Art::Cloud, Some("a")),
            route("lo", Art::Lokal, None),
        ],
        vec![
            Kunde {
                schluessel: s("k"),
                erlaubt: vec![s("lc"), s("cc")],
                verboten: vec![],
            },
            Kunde {
                schluessel: s("kv"),
                erlaubt: vec![s("lc"), s("cc")],
                verboten: vec![s("cc")],
            },
            Kunde {
                schluessel: s("kn"),
                erlaubt: vec![],
                verboten: vec![],
            },
            Kunde {
                schluessel: s("kf"),
                erlaubt: vec![s("cc")],
                verboten: vec![],
            },
        ],
        vec![Freigabe {
            route: s("cc"),
            kunde: s("k"),
        }],
    )
    .expect("valid settings")
}

fn e(ebene: PersonEbene, wer: &str, klasse: &str) -> Leseeintrag {
    Leseeintrag {
        ebene,
        wer: s(wer),
        klasse: s(klasse),
    }
}

use PersonEbene::{Gruppe, Nutzer};

fn personen(v: &Vorgabe) -> Personen {
    Personen::neu(
        v,
        vec![
            e(Nutzer, "pa", "a"),
            e(Nutzer, "pb", "b"),
            e(Nutzer, "pc", "c"),
            e(Gruppe, "ga", "a"),
            e(Gruppe, "gb", "b"),
            e(Gruppe, "gc", "c"),
            e(Nutzer, "tief", "a"),
            // the same key as a user and as a group: two entries, not a duplicate
            e(Gruppe, "pa", "c"),
        ],
    )
    .expect("valid read classes")
}

fn person<'a>(nutzer: Option<&'a str>, gruppen: &'a [&'a str]) -> Person<'a> {
    Person { nutzer, gruppen }
}

const KLASSEN: [Option<&str>; 6] = [None, Some("a"), Some("b"), Some("c"), Some("x"), Some("")];
const ROUTEN: [Option<&str>; 9] = [
    None,
    Some(""),
    Some("la"),
    Some("lb"),
    Some("lc"),
    Some("cc"),
    Some("ca"),
    Some("lo"),
    Some("zz"),
];
const KUNDEN: [Option<&str>; 7] = [
    None,
    Some("k"),
    Some("kv"),
    Some("kn"),
    Some("kf"),
    Some("zz"),
    Some(""),
];

#[test]
fn p4_nutzer_vor_gruppe_und_hoechste_gruppe() {
    let v = vorgabe();
    let p = personen(&v);
    assert_eq!(p.leseklasse(&person(Some("pb"), &[])), Some("b"));
    // F2: the own entry counts also below a group
    assert_eq!(p.leseklasse(&person(Some("tief"), &["gc"])), Some("a"));
    // F1: the highest class of the groups, in any order
    assert_eq!(
        p.leseklasse(&person(Some("x"), &["ga", "gc", "gb"])),
        Some("c")
    );
    assert_eq!(p.leseklasse(&person(Some("x"), &["gb", "ga"])), Some("b"));
    assert_eq!(p.leseklasse(&person(Some("x"), &["ga"])), Some("a"));
    // unknown groups, a user key as a group, a group key as a user: none
    assert_eq!(p.leseklasse(&person(Some("x"), &["gz", "pc", "GC"])), None);
    assert_eq!(p.leseklasse(&person(Some("gc"), &[])), None);
    // the user "pa" has its own entry; the group "pa" counts only as a group
    assert_eq!(p.leseklasse(&person(Some("pa"), &["pa"])), Some("a"));
    assert_eq!(p.leseklasse(&person(Some("x"), &["pa"])), Some("c"));
    // byte for byte
    assert_eq!(p.leseklasse(&person(Some("PB"), &[])), None);
    assert_eq!(p.leseklasse(&person(Some("pb "), &[])), None);
    // no person: no read class, also with a group that would count
    assert_eq!(p.leseklasse(&person(None, &["gc"])), None);
    assert_eq!(p.leseklasse(&person(Some(""), &["gc"])), None);
}

#[test]
fn p0_ladefehler() {
    let v = vorgabe();
    let neu = |l: Vec<Leseeintrag>| Personen::neu(&v, l).map(|_| ());
    assert_eq!(neu(vec![]), Ok(()));
    assert_eq!(
        neu(vec![e(Nutzer, "pa", "B")]),
        Err(PersonenFehler::KlasseUnbekannt {
            wer: s("pa"),
            klasse: s("B")
        })
    );
    assert_eq!(
        neu(vec![e(Nutzer, "pa", "")]),
        Err(PersonenFehler::KlasseUnbekannt {
            wer: s("pa"),
            klasse: s("")
        })
    );
    for falsch in ["", "Pa", "-a", "a b", "ä", &"a".repeat(65)] {
        assert_eq!(
            neu(vec![e(Gruppe, falsch, "a")]),
            Err(PersonenFehler::SchluesselForm(s(falsch))),
            "{falsch:?}"
        );
    }
    assert_eq!(neu(vec![e(Gruppe, &"a".repeat(64), "a")]), Ok(()));
    assert_eq!(
        neu(vec![e(Nutzer, "ps_0123456789abcdef0123456789abcdef", "a")]),
        Ok(())
    );
    assert_eq!(
        neu(vec![e(Nutzer, "pa", "a"), e(Nutzer, "pa", "b")]),
        Err(PersonenFehler::Doppelt(s("pa")))
    );
    assert_eq!(
        neu(vec![
            e(Gruppe, "g", "a"),
            e(Nutzer, "g", "a"),
            e(Gruppe, "g", "a")
        ]),
        Err(PersonenFehler::Doppelt(s("g")))
    );
    // the first violation decides
    assert_eq!(
        neu(vec![
            e(Nutzer, "pa", "a"),
            e(Nutzer, "X", "z"),
            e(Nutzer, "pa", "a")
        ]),
        Err(PersonenFehler::SchluesselForm(s("X")))
    );
    assert_eq!(
        neu(vec![e(Nutzer, "pa", "z"), e(Nutzer, "pa", "a")]),
        Err(PersonenFehler::KlasseUnbekannt {
            wer: s("pa"),
            klasse: s("z")
        })
    );
}

#[test]
fn p0_grenze_der_eintraege() {
    let v = vorgabe();
    let liste = |n: usize| -> Vec<Leseeintrag> {
        (0..n).map(|i| e(Gruppe, &format!("g{i}"), "a")).collect()
    };
    assert_eq!(LESEKLASSEN_MAX, 4096);
    let voll = Personen::neu(&v, liste(LESEKLASSEN_MAX)).expect("4096 entries are valid");
    assert_eq!(voll.leseklasse(&person(Some("x"), &["g4095"])), Some("a"));
    assert_eq!(
        Personen::neu(&v, liste(LESEKLASSEN_MAX + 1)).map(|_| ()),
        Err(PersonenFehler::ZuViele(4097))
    );
    // the limit comes before every other check
    let mut falsch = liste(LESEKLASSEN_MAX);
    falsch.push(e(Nutzer, "X", "z"));
    assert_eq!(
        Personen::neu(&v, falsch).map(|_| ()),
        Err(PersonenFehler::ZuViele(4097))
    );
}

/// The reason `darf_person` must give by section 4, written from the table: D2, P1, P2, D3, D4, D6, P3, D7 to D11.
fn erwartet(
    v: &Vorgabe,
    tor: Grund,
    lese: Option<&str>,
    wirksam: Option<&str>,
    nutzer: Option<&str>,
) -> PersonGrund {
    if tor == Grund::RouteFehlt {
        return PersonGrund::Tor(tor);
    }
    if nutzer.is_none_or(str::is_empty) {
        return PersonGrund::PersonFehlt;
    }
    let Some(lese) = lese else {
        return PersonGrund::PersonOhneLeseklasse;
    };
    if matches!(
        tor,
        Grund::RouteOhneKlasse | Grund::EinheitOhneKlasse | Grund::KlasseUeberRoute
    ) {
        return PersonGrund::Tor(tor);
    }
    let w = wirksam
        .and_then(|k| v.rang(k))
        .expect("a known class after D4");
    if w > v.rang(lese).expect("known read class") {
        return PersonGrund::KlasseUeberPerson;
    }
    PersonGrund::Tor(tor)
}

const PERSONEN: [(Option<&str>, &[&str]); 10] = [
    (None, &[]),
    (Some(""), &["gc"]),
    (Some("pa"), &[]),
    (Some("pb"), &["ga"]),
    (Some("pc"), &[]),
    (Some("tief"), &["gc"]),
    (Some("x"), &["gb", "ga"]),
    (Some("x"), &["gc"]),
    (Some("x"), &[]),
    (Some("x"), &["zz"]),
];

#[test]
fn p5_reihenfolge_ueber_alle_eingaben() {
    let v = vorgabe();
    let p = personen(&v);
    let mut je_grund = std::collections::BTreeMap::new();
    for k in KLASSEN {
        for r in ROUTEN {
            for ku in KUNDEN {
                for (n, g) in PERSONEN {
                    let pe = person(n, g);
                    let einheit = Einheit { klasse: k };
                    let tor = darf(&v, &einheit, r, ku);
                    let a = darf_person(&v, &p, &einheit, r, ku, &pe);
                    let lese = p.leseklasse(&pe);
                    let soll = erwartet(&v, tor.grund, lese, tor.klasse.as_deref(), n);
                    assert_eq!(a.grund, soll, "{k:?} {r:?} {ku:?} {n:?} {g:?}");
                    assert_eq!(a.klasse, tor.klasse, "klasse is the effective class");
                    assert_eq!(a.leseklasse.as_deref(), lese);
                    assert_eq!(a.darf(), soll == PersonGrund::Tor(Grund::Frei));
                    *je_grund.entry(a.grund.code()).or_insert(0) += 1;
                }
            }
        }
    }
    // every reason of the table occurs in the sweep, so the sweep tests every branch
    for code in [
        "frei",
        "route-fehlt",
        "person-fehlt",
        "person-ohne-leseklasse",
        "route-ohne-klasse",
        "einheit-ohne-klasse",
        "klasse-ueber-route",
        "klasse-ueber-person",
        "kunde-fehlt",
        "kunde-ohne-eintrag",
        "route-fuer-kunde-verboten",
        "route-fuer-kunde-nicht-erlaubt",
        "kundendaten-an-cloud",
    ] {
        assert!(je_grund.contains_key(code), "{code} never occurs");
    }
}

#[test]
fn p6_hoechste_leseklasse_ist_darf() {
    let v = vorgabe();
    let p = personen(&v);
    for k in KLASSEN {
        for r in ROUTEN {
            for ku in KUNDEN {
                let einheit = Einheit { klasse: k };
                let tor = darf(&v, &einheit, r, ku);
                let a = darf_person(&v, &p, &einheit, r, ku, &person(Some("pc"), &[]));
                assert_eq!(a.grund, PersonGrund::Tor(tor.grund), "{k:?} {r:?} {ku:?}");
                assert_eq!(a.klasse, tor.klasse);
                assert_eq!(a.meldung(r), tor.meldung(r));
            }
        }
    }
}

#[test]
fn p3_gegenprobe_a34() {
    let v = vorgabe();
    let p = personen(&v);
    let b = Einheit { klasse: Some("b") };
    let gesperrt = darf_person(&v, &p, &b, Some("lb"), None, &person(Some("pa"), &[]));
    assert_eq!(gesperrt.grund, PersonGrund::KlasseUeberPerson);
    assert!(!gesperrt.darf());
    assert_eq!(gesperrt.klasse.as_deref(), Some("b"));
    assert_eq!(gesperrt.leseklasse.as_deref(), Some("a"));
    let frei = darf_person(&v, &p, &b, Some("lb"), None, &person(Some("pb"), &[]));
    assert!(frei.darf());
    // a unit without class gets the vorgabestufe b
    let ohne = Einheit { klasse: None };
    assert_eq!(
        darf_person(&v, &p, &ohne, Some("lc"), None, &person(Some("pa"), &[])).grund,
        PersonGrund::KlasseUeberPerson
    );
    assert!(darf_person(&v, &p, &ohne, Some("lc"), None, &person(Some("pb"), &[])).darf());
}

/// The reason `lesen_person` must give by P7: P1, P2, D4, P3, D1.
fn erwartet_lesen(
    v: &Vorgabe,
    wirksam: Option<&str>,
    lese: Option<&str>,
    nutzer: Option<&str>,
) -> PersonGrund {
    if nutzer.is_none_or(str::is_empty) {
        return PersonGrund::PersonFehlt;
    }
    let Some(lese) = lese else {
        return PersonGrund::PersonOhneLeseklasse;
    };
    let Some(w) = wirksam else {
        return PersonGrund::Tor(Grund::EinheitOhneKlasse);
    };
    if v.rang(w) > v.rang(lese) {
        return PersonGrund::KlasseUeberPerson;
    }
    PersonGrund::Tor(Grund::Frei)
}

#[test]
fn p7_lesen_ohne_route() {
    let v = vorgabe();
    let p = personen(&v);
    let mut gesehen = std::collections::BTreeSet::new();
    for k in KLASSEN {
        for ku in KUNDEN {
            for (n, g) in PERSONEN {
                let pe = person(n, g);
                let einheit = Einheit { klasse: k };
                let a = lesen_person(&v, &p, &einheit, ku, &pe);
                let wirksam = datentor::wirksam(&v, &einheit, ku).ok();
                let soll = erwartet_lesen(&v, wirksam, p.leseklasse(&pe), n);
                assert_eq!(a.grund, soll, "{k:?} {ku:?} {n:?} {g:?}");
                assert_eq!(a.klasse.as_deref(), wirksam);
                assert_eq!(a.leseklasse.as_deref(), p.leseklasse(&pe));
                assert_eq!(a.darf(), soll == PersonGrund::Tor(Grund::Frei));
                // below the highest class and without a customer, the read is darf_person on a local route of the highest
                // class; at the highest class the route would add D7 (customer data without customer), the read does not
                if ku.is_none() && wirksam != Some("c") {
                    let ueber_route = darf_person(&v, &p, &einheit, Some("lc"), None, &pe);
                    assert_eq!(a, ueber_route, "{k:?} {n:?} {g:?}");
                }
                gesehen.insert(a.grund.code());
            }
        }
    }
    assert_eq!(
        gesehen.into_iter().collect::<Vec<_>>(),
        [
            "einheit-ohne-klasse",
            "frei",
            "klasse-ueber-person",
            "person-fehlt",
            "person-ohne-leseklasse"
        ]
    );
    // customer content has the highest class; no customer rule applies to a read, also not D7 at the highest class
    let c = Einheit { klasse: Some("c") };
    assert!(lesen_person(&v, &p, &c, None, &person(Some("pc"), &[])).darf());
    let a = Einheit { klasse: Some("a") };
    assert!(lesen_person(&v, &p, &a, Some("zz"), &person(Some("pc"), &[])).darf());
    assert_eq!(
        lesen_person(&v, &p, &a, Some("k"), &person(Some("pb"), &[])).grund,
        PersonGrund::KlasseUeberPerson
    );
    assert_eq!(
        lesen_person(&v, &p, &a, Some("k"), &person(Some("pb"), &[]))
            .klasse
            .as_deref(),
        Some("c")
    );
}

#[test]
fn leseklasse_fremder_vorgabe_sperrt() {
    // read classes built for other settings: a class the settings of the call do not know counts as none (fail closed)
    let v = vorgabe();
    let andere = Vorgabe::neu(
        vec![s("a"), s("z")],
        None,
        vec![route("la", Art::Lokal, Some("z"))],
        vec![],
        vec![],
    )
    .expect("valid settings");
    let p = Personen::neu(&andere, vec![e(Nutzer, "p", "z"), e(Nutzer, "q", "a")]).expect("valid");
    let einheit = Einheit { klasse: Some("a") };
    let pe = person(Some("p"), &[]);
    assert_eq!(
        lesen_person(&v, &p, &einheit, None, &pe).grund,
        PersonGrund::PersonOhneLeseklasse
    );
    assert_eq!(
        darf_person(&v, &p, &einheit, Some("la"), None, &pe).grund,
        PersonGrund::PersonOhneLeseklasse
    );
    assert_eq!(lesen_person(&v, &p, &einheit, None, &pe).leseklasse, None);
    assert!(lesen_person(&v, &p, &einheit, None, &person(Some("q"), &[])).darf());
}

#[test]
fn codes_und_meldungen() {
    let antwort = |grund| PersonAntwort {
        grund,
        klasse: None,
        leseklasse: None,
    };
    let faelle = [
        (
            PersonGrund::PersonFehlt,
            "person-fehlt",
            "Anfrage ohne Person",
        ),
        (
            PersonGrund::PersonOhneLeseklasse,
            "person-ohne-leseklasse",
            "Person ohne Leseklasse",
        ),
        (
            PersonGrund::KlasseUeberPerson,
            "klasse-ueber-person",
            "Klasse der Einheit liegt über der Leseklasse der Person",
        ),
        (PersonGrund::Tor(Grund::Frei), "frei", "frei"),
        (
            PersonGrund::Tor(Grund::RouteOhneKlasse),
            "route-ohne-klasse",
            "Route r-x hat keine Klasse",
        ),
    ];
    for (g, code, meldung) in faelle {
        assert_eq!(g.code(), code);
        assert_eq!(antwort(g).meldung(Some("r-x")), meldung);
        assert_eq!(antwort(g).darf(), code == "frei");
    }
}

#[test]
fn fehler_als_text() {
    let texte = [
        (
            PersonenFehler::ZuViele(4097),
            "4097 entries, more than 4096",
        ),
        (
            PersonenFehler::SchluesselForm(s("X")),
            "key \"X\" does not have the key form",
        ),
        (
            PersonenFehler::KlasseUnbekannt {
                wer: s("p"),
                klasse: s("z"),
            },
            "class \"z\" of \"p\" is not in klassen",
        ),
        (
            PersonenFehler::Doppelt(s("p")),
            "\"p\" stands twice on the same level",
        ),
    ];
    for (f, t) in texte {
        assert_eq!(f.to_string(), t);
    }
}
