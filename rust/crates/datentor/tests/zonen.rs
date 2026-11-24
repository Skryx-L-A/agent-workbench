//! Unit tests of the model zones (partial contract `zonen/VERTRAG.md`, sections 2 to 6 and 8). All data are invented.

#![allow(
    clippy::many_single_char_names,
    clippy::option_option,
    reason = "short names for the test data"
)]

use datentor::zonen::{
    Ausnahme, Bedingung, Ebene, Klassensatz, Kontext, ProjektFreigabe, Protokoll, Regel, Zone,
    Zonen, ZonenAntwort, ZonenFehler, ZonenGrund, darf_zonen,
};
use datentor::{Art, Einheit, Freigabe, Grund, Kunde, Route, Vorgabe, darf};

fn s(x: &str) -> String {
    x.to_owned()
}

fn sv(x: &[&str]) -> Vec<String> {
    x.iter().map(|y| s(y)).collect()
}

fn route(schluessel: &str, art: Art, klasse: Option<&str>) -> Route {
    Route {
        schluessel: s(schluessel),
        art,
        klasse: klasse.map(s),
    }
}

const ROUTEN: [&str; 11] = [
    "l1", "l2", "c1", "c2", "c3", "c4", "cx", "c5", "c6", "c7", "l3",
];

fn vorgabe() -> Vorgabe {
    Vorgabe::neu(
        sv(&["a", "b", "c"]),
        None,
        vec![
            route("l1", Art::Lokal, None),
            route("l2", Art::Lokal, Some("a")),
            route("c1", Art::Cloud, None),
            route("c2", Art::Cloud, None),
            route("c3", Art::Cloud, None),
            route("c4", Art::Cloud, None),
            route("cx", Art::Cloud, Some("b")),
            route("c5", Art::Cloud, Some("c")),
            route("c6", Art::Cloud, None),
            route("c7", Art::Cloud, None),
            route("l3", Art::Lokal, Some("c")),
        ],
        vec![
            Kunde {
                schluessel: s("k"),
                erlaubt: sv(&ROUTEN),
                verboten: vec![],
            },
            Kunde {
                schluessel: s("k2"),
                erlaubt: sv(&ROUTEN),
                verboten: vec![],
            },
            Kunde {
                schluessel: s("kv"),
                erlaubt: sv(&ROUTEN),
                verboten: sv(&["c2"]),
            },
            Kunde {
                schluessel: s("kn"),
                erlaubt: vec![],
                verboten: vec![],
            },
        ],
        vec![
            Freigabe {
                route: s("c5"),
                kunde: s("k"),
            },
            Freigabe {
                route: s("c2"),
                kunde: s("k2"),
            },
        ],
    )
    .expect("valid settings")
}

fn zone(schluessel: &str, klassen: Klassensatz, bedingung: Bedingung, routen: &[&str]) -> Zone {
    Zone {
        schluessel: s(schluessel),
        klassen,
        bedingung,
        routen: sv(routen),
    }
}

fn zonen_liste() -> Vec<Zone> {
    vec![
        zone(
            "z1",
            Klassensatz::Alle,
            Bedingung::Immer,
            &["l1", "l2", "c5", "l3"],
        ),
        zone("z2", Klassensatz::Alle, Bedingung::Nachrangig, &["c1"]),
        zone("z3", Klassensatz::Alle, Bedingung::Freigabe, &["c2", "c6"]),
        zone("z4", Klassensatz::Niedrigste, Bedingung::Immer, &["c3"]),
        zone("z5", Klassensatz::Keine, Bedingung::Gesperrt, &["c4"]),
        zone("z6", Klassensatz::Alle, Bedingung::Gesperrt, &["c7"]),
    ]
}

fn freigabe(route: &str, kunde: &str, projekt: &str) -> ProjektFreigabe {
    ProjektFreigabe {
        route: s(route),
        kunde: s(kunde),
        projekt: s(projekt),
    }
}

fn ausnahme(zone: &str, nutzer: &[&str], gruppen: &[&str], bis: u64) -> Ausnahme {
    Ausnahme {
        zone: s(zone),
        nutzer: sv(nutzer),
        gruppen: sv(gruppen),
        gesetzt_von: s("admin-1"),
        zweck: s("Evaluierung"),
        bis,
    }
}

fn regel(
    ebene: Ebene,
    wer: &str,
    zone: &str,
    klassen: Option<Klassensatz>,
    gesperrt: bool,
) -> Regel {
    Regel {
        ebene,
        wer: s(wer),
        zone: s(zone),
        klassen,
        gesperrt,
    }
}

fn zonen(an: bool) -> Zonen {
    Zonen::neu(
        &vorgabe(),
        an,
        zonen_liste(),
        vec![
            freigabe("c2", "k", "p1"),
            freigabe("c2", "k", "p1"),
            freigabe("c6", "k", "p2"),
        ],
        vec![
            ausnahme("z5", &["n1"], &[], 100),
            ausnahme("z5", &[], &["g1"], 200),
            ausnahme("z6", &["n4"], &[], 1000),
            ausnahme("z6", &["n1"], &[], 1000),
        ],
        vec![
            regel(Ebene::Nutzer, "n2", "z1", None, true),
            regel(
                Ebene::Gruppe,
                "g2",
                "z1",
                Some(Klassensatz::Niedrigste),
                false,
            ),
            regel(Ebene::Gruppe, "g3", "z1", Some(Klassensatz::Keine), false),
            regel(Ebene::Nutzer, "n5", "z1", Some(Klassensatz::Alle), false),
            regel(Ebene::Gruppe, "g4", "z6", None, true),
        ],
    )
    .expect("valid zones")
}

/// A request in the shape of the program: unit class (`None` = run check; `Some(None)` = unit without class).
struct A<'a> {
    einheit: Option<Option<&'a str>>,
    route: Option<&'a str>,
    kunde: Option<&'a str>,
    projekt: Option<&'a str>,
    nutzer: Option<&'a str>,
    gruppen: &'a [&'a str],
    jetzt: Option<u64>,
    grund: Option<&'a str>,
}

fn a<'a>(klasse: &'a str, route: &'a str) -> A<'a> {
    A {
        einheit: Some(Some(klasse)),
        route: Some(route),
        kunde: None,
        projekt: None,
        nutzer: Some("n1"),
        gruppen: &[],
        jetzt: Some(50),
        grund: None,
    }
}

fn frage(z: &Zonen, x: &A<'_>) -> ZonenAntwort {
    let v = vorgabe();
    let e = x.einheit.map(|k| Einheit { klasse: k });
    darf_zonen(
        &v,
        z,
        e.as_ref(),
        x.route,
        x.kunde,
        x.projekt,
        &Kontext {
            nutzer: x.nutzer,
            gruppen: x.gruppen,
            jetzt: x.jetzt,
            grund: x.grund,
        },
    )
}

fn grund(z: &Zonen, x: &A<'_>) -> ZonenGrund {
    frage(z, x).grund
}

use ZonenGrund as G;

#[test]
fn option_aus_ist_genau_darf() {
    let z = zonen(false);
    let v = vorgabe();
    for (klasse, route, kunde) in [
        (Some("a"), Some("l2"), None),
        (Some("b"), Some("l2"), None),
        (Some("c"), Some("c5"), Some("k")),
        (Some("c"), Some("l3"), None),
        (None, Some("l3"), Some("kn")),
        (Some("x"), Some("l2"), None),
        (Some("a"), None, None),
        (Some("a"), Some("l1"), None),
        (Some("a"), Some("c4"), None),
    ] {
        let e = Einheit { klasse };
        let soll = darf(&v, &e, route, kunde);
        let mut x = a("a", "l1");
        x.einheit = Some(klasse);
        x.route = route;
        x.kunde = kunde;
        x.projekt = Some("p1");
        x.grund = Some("g");
        let ist = frage(&z, &x);
        assert_eq!(ist.grund, G::Tor(soll.grund), "{klasse:?} {route:?}");
        assert_eq!(ist.klasse, soll.klasse);
        assert_eq!(ist.protokoll, None);
        assert_eq!(ist.darf(), soll.darf());
        assert_eq!(ist.meldung(route), soll.meldung(route));
    }
}

#[test]
fn option_aus_laufpruefung_nur_d2_und_d3() {
    let z = zonen(false);
    for (route, soll) in [
        (None, Grund::RouteFehlt),
        (Some(""), Grund::RouteFehlt),
        (Some("q"), Grund::RouteOhneKlasse),
        (Some("l1"), Grund::RouteOhneKlasse),
        (Some("l2"), Grund::Frei),
        (Some("c5"), Grund::Frei),
    ] {
        let mut x = a("a", "l1");
        x.einheit = None;
        x.route = route;
        x.nutzer = None;
        let ist = frage(&z, &x);
        assert_eq!(ist.grund, G::Tor(soll), "{route:?}");
        assert_eq!(ist.klasse, None);
        assert_eq!(ist.protokoll, None);
    }
}

#[test]
fn reihenfolge_route_nutzer_zone_einheit() {
    let z = zonen(true);
    let mut x = a("a", "l1");
    x.route = None;
    x.nutzer = None;
    assert_eq!(grund(&z, &x), G::Tor(Grund::RouteFehlt));
    x.route = Some("");
    assert_eq!(grund(&z, &x), G::Tor(Grund::RouteFehlt));
    x.route = Some("q");
    assert_eq!(grund(&z, &x), G::NutzerFehlt);
    x.nutzer = Some("");
    assert_eq!(grund(&z, &x), G::NutzerFehlt);
    x.nutzer = Some("n1");
    assert_eq!(grund(&z, &x), G::RouteOhneZone);
    x.route = Some("cx");
    assert_eq!(grund(&z, &x), G::RouteOhneZone);
    x.route = Some("l1");
    x.einheit = Some(Some("x"));
    assert_eq!(grund(&z, &x), G::Tor(Grund::EinheitOhneKlasse));
    assert_eq!(frage(&z, &x).klasse, None);
    // D3 does not hold with the option on: l1 has no class but its zone admits all.
    x.einheit = Some(Some("a"));
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    assert_eq!(frage(&z, &x).klasse.as_deref(), Some("a"));
    // A unit without class gets the vorgabestufe (here the highest class).
    x.einheit = Some(None);
    assert_eq!(grund(&z, &x), G::Tor(Grund::KundeFehlt));
    assert_eq!(frage(&z, &x).klasse.as_deref(), Some("c"));
}

#[test]
fn regel_sperrt_vor_ausnahme_und_grund() {
    let z = zonen(true);
    let mut x = a("a", "l1");
    x.nutzer = Some("n2");
    assert_eq!(grund(&z, &x), G::ZoneDurchRegelGesperrt);
    x.einheit = None;
    assert_eq!(grund(&z, &x), G::ZoneDurchRegelGesperrt);
    // The user rule names only n2; a group with the same key does not count.
    x.nutzer = Some("n1");
    x.gruppen = &["n2"];
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    // A group rule blocks even where an exception holds.
    let mut y = a("a", "c7");
    y.nutzer = Some("n4");
    assert_eq!(grund(&z, &y), G::Tor(Grund::Frei));
    y.gruppen = &["g4"];
    assert_eq!(grund(&z, &y), G::ZoneDurchRegelGesperrt);
    assert!(frage(&z, &y).protokoll.expect("on").ausnahme.is_some());
    // A group rule for g4 holds only in its zone z6.
    let mut w = a("a", "l1");
    w.gruppen = &["g4"];
    assert_eq!(grund(&z, &w), G::Tor(Grund::Frei));
    // A user rule n2 holds only in its zone z1.
    let mut u = a("a", "c3");
    u.nutzer = Some("n2");
    assert_eq!(grund(&z, &u), G::Tor(Grund::Frei));
}

#[test]
fn gesperrte_zone_und_ausnahme() {
    let z = zonen(true);
    let mut x = a("a", "c4");
    x.einheit = None;
    x.nutzer = Some("n3");
    assert_eq!(grund(&z, &x), G::ZoneGesperrt);
    // n1 before the deadline: the run may start, the unit stays blocked by the class set keine.
    x.nutzer = Some("n1");
    x.jetzt = Some(99);
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    let p = frage(&z, &x).protokoll.expect("on");
    assert_eq!(p.ausnahme, Some(ausnahme("z5", &["n1"], &[], 100)));
    assert_eq!(p.zone.as_deref(), Some("z5"));
    x.einheit = Some(Some("a"));
    assert_eq!(grund(&z, &x), G::KlasseUeberZone);
    // At the deadline the exception has expired.
    x.einheit = None;
    x.jetzt = Some(100);
    assert_eq!(grund(&z, &x), G::ZoneGesperrt);
    assert_eq!(frage(&z, &x).protokoll.expect("on").ausnahme, None);
    x.jetzt = None;
    assert_eq!(grund(&z, &x), G::ZoneGesperrt);
    // A group of the exception counts; the first exception that holds wins.
    x.nutzer = Some("n9");
    x.gruppen = &["g0", "g1"];
    x.jetzt = Some(150);
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    assert_eq!(
        frage(&z, &x).protokoll.expect("on").ausnahme,
        Some(ausnahme("z5", &[], &["g1"], 200))
    );
    x.nutzer = Some("n1");
    x.jetzt = Some(10);
    assert_eq!(
        frage(&z, &x).protokoll.expect("on").ausnahme,
        Some(ausnahme("z5", &["n1"], &[], 100))
    );
    // An exception of another zone does not hold.
    x.nutzer = Some("n4");
    x.gruppen = &[];
    assert_eq!(grund(&z, &x), G::ZoneGesperrt);
    // Without a user the answer is Z1, but an exception through a group stands in the log (addendum 2).
    x.nutzer = Some("");
    x.gruppen = &["g1"];
    assert_eq!(grund(&z, &x), G::NutzerFehlt);
    let mit_gruppe = Some(ausnahme("z5", &[], &["g1"], 200));
    assert_eq!(frage(&z, &x).protokoll.expect("on").ausnahme, mit_gruppe);
    x.nutzer = None;
    assert_eq!(frage(&z, &x).protokoll.expect("on").ausnahme, mit_gruppe);
    // An empty user never matches a user list.
    x.gruppen = &[];
    assert_eq!(frage(&z, &x).protokoll.expect("on").ausnahme, None);
    // An exception lifts only Z4: in z6 (all classes) the unit passes, in z5 (no class) it does not.
    let mut y = a("b", "c7");
    y.nutzer = Some("n1");
    assert_eq!(grund(&z, &y), G::Tor(Grund::Frei));
    // An exception lists only the user of the exception.
    y.nutzer = Some("n9");
    assert_eq!(grund(&z, &y), G::ZoneGesperrt);
}

#[test]
fn nachrangige_zone_braucht_einen_grund() {
    let z = zonen(true);
    let mut x = a("a", "c1");
    for g in [None, Some(""), Some(" \t\n\r")] {
        x.grund = g;
        assert_eq!(grund(&z, &x), G::GrundFehlt, "{g:?}");
        assert_eq!(frage(&z, &x).protokoll.expect("on").grund, None);
    }
    for g in ["x", " x ", "\u{a0}"] {
        x.grund = Some(g);
        assert_eq!(grund(&z, &x), G::Tor(Grund::Frei), "{g:?}");
        assert_eq!(
            frage(&z, &x).protokoll.expect("on").grund.as_deref(),
            Some(g)
        );
    }
    x.einheit = None;
    x.grund = None;
    assert_eq!(grund(&z, &x), G::GrundFehlt);
    // The reason stands only for a zone of lower precedence.
    let mut y = a("a", "l1");
    y.grund = Some("x");
    assert_eq!(frage(&z, &y).protokoll.expect("on").grund, None);
}

#[test]
fn grenze_der_zone_und_der_regeln() {
    let z = zonen(true);
    let mut x = a("b", "c3");
    assert_eq!(grund(&z, &x), G::KlasseUeberZone);
    x.einheit = Some(Some("a"));
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    x.einheit = None;
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    assert_eq!(frage(&z, &x).klasse, None);
    // A customer raises the class to the highest (D5).
    let mut k = a("a", "c3");
    k.kunde = Some("k");
    assert_eq!(grund(&z, &k), G::KlasseUeberZone);
    assert_eq!(frage(&z, &k).klasse.as_deref(), Some("c"));
    // Group rules lower the limit; the lowest value counts.
    let mut y = a("b", "l1");
    y.gruppen = &["g2"];
    assert_eq!(grund(&z, &y), G::KlasseUeberZone);
    y.einheit = Some(Some("a"));
    assert_eq!(grund(&z, &y), G::Tor(Grund::Frei));
    y.gruppen = &["g2", "g3"];
    assert_eq!(grund(&z, &y), G::KlasseUeberZone);
    y.gruppen = &["g3"];
    assert_eq!(grund(&z, &y), G::KlasseUeberZone);
    // A rule equal to the zone (alle) changes nothing.
    let mut w = a("b", "l1");
    w.nutzer = Some("n5");
    assert_eq!(grund(&z, &w), G::Tor(Grund::Frei));
}

#[test]
fn eigene_klasse_der_route_gilt_auch() {
    let z = zonen(true);
    let mut x = a("b", "l2");
    assert_eq!(grund(&z, &x), G::Tor(Grund::KlasseUeberRoute));
    x.einheit = Some(Some("a"));
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    let mut y = a("c", "l3");
    assert_eq!(grund(&z, &y), G::Tor(Grund::KundeFehlt));
    y.kunde = Some("k");
    assert_eq!(grund(&z, &y), G::Tor(Grund::Frei));
}

#[test]
fn kundendaten_in_zonen() {
    let z = zonen(true);
    let mut x = a("c", "l1");
    assert_eq!(grund(&z, &x), G::Tor(Grund::KundeFehlt));
    x.kunde = Some("q");
    assert_eq!(grund(&z, &x), G::Tor(Grund::KundeOhneEintrag));
    x.kunde = Some("kn");
    assert_eq!(grund(&z, &x), G::Tor(Grund::RouteFuerKundeNichtErlaubt));
    x.kunde = Some("k");
    assert_eq!(grund(&z, &x), G::Tor(Grund::Frei));
    // A unit below the highest class with a customer is customer data, too.
    x.einheit = Some(Some("a"));
    x.kunde = Some("kn");
    assert_eq!(grund(&z, &x), G::Tor(Grund::RouteFuerKundeNichtErlaubt));
    // Zone with freigabe: route, customer and project must match one release.
    let mut y = a("c", "c2");
    y.kunde = Some("kv");
    y.projekt = Some("p1");
    assert_eq!(grund(&z, &y), G::Tor(Grund::RouteFuerKundeVerboten));
    y.kunde = Some("k");
    assert_eq!(grund(&z, &y), G::Tor(Grund::Frei));
    y.projekt = Some("p2");
    assert_eq!(grund(&z, &y), G::ZoneOhneFreigabe);
    y.projekt = None;
    assert_eq!(grund(&z, &y), G::ZoneOhneFreigabe);
    // k2 has a release of contract 3 for c2; it is not enough in a zone with freigabe.
    y.kunde = Some("k2");
    y.projekt = Some("p1");
    assert_eq!(grund(&z, &y), G::ZoneOhneFreigabe);
    let mut w = a("c", "c6");
    w.kunde = Some("k");
    w.projekt = Some("p2");
    assert_eq!(grund(&z, &w), G::Tor(Grund::Frei));
    w.projekt = Some("p1");
    assert_eq!(grund(&z, &w), G::ZoneOhneFreigabe);
    // Other zones: a cloud route needs the release of contract 3, a local route does not.
    let mut c = a("c", "c1");
    c.grund = Some("g");
    c.kunde = Some("k");
    assert_eq!(grund(&z, &c), G::Tor(Grund::KundendatenAnCloud));
    let mut f = a("c", "c5");
    f.kunde = Some("k");
    assert_eq!(grund(&z, &f), G::Tor(Grund::Frei));
    f.kunde = Some("k2");
    assert_eq!(grund(&z, &f), G::Tor(Grund::KundendatenAnCloud));
}

#[test]
fn protokoll_steht_auch_bei_darf_nicht() {
    let z = zonen(true);
    let mut x = a("c", "c1");
    x.grund = Some("Grund");
    x.kunde = Some("k");
    let ant = frage(&z, &x);
    assert!(!ant.darf());
    assert_eq!(
        ant.protokoll,
        Some(Protokoll {
            zone: Some(s("z2")),
            grund: Some(s("Grund")),
            ausnahme: None,
        })
    );
    x.route = Some("cx");
    assert_eq!(
        frage(&z, &x).protokoll,
        Some(Protokoll {
            zone: None,
            grund: None,
            ausnahme: None,
        })
    );
    x.route = None;
    assert_eq!(frage(&z, &x).protokoll.expect("on").zone, None);
}

#[test]
fn codes_und_meldungen() {
    let r = Some("r-x");
    let faelle = [
        (G::NutzerFehlt, "nutzer-fehlt", "Anfrage ohne Nutzer"),
        (
            G::RouteOhneZone,
            "route-ohne-zone",
            "Route r-x hat keine Zone",
        ),
        (
            G::ZoneDurchRegelGesperrt,
            "zone-durch-regel-gesperrt",
            "Zone der Route r-x ist durch eine Regel gesperrt",
        ),
        (
            G::ZoneGesperrt,
            "zone-gesperrt",
            "Zone der Route r-x ist gesperrt",
        ),
        (
            G::GrundFehlt,
            "grund-fehlt",
            "Route r-x braucht einen Grund für die nachrangige Zone",
        ),
        (
            G::KlasseUeberZone,
            "klasse-ueber-zone",
            "Klasse der Einheit liegt über der Zone der Route r-x",
        ),
        (
            G::ZoneOhneFreigabe,
            "zone-ohne-freigabe",
            "Kundendaten an Route r-x ohne Freigabe für Kunde und Projekt",
        ),
        (
            G::Tor(Grund::KundendatenAnCloud),
            "kundendaten-an-cloud",
            "Kundendaten an Cloud-Route r-x ohne Freigabe",
        ),
        (G::Tor(Grund::Frei), "frei", "frei"),
    ];
    for (g, code, meldung) in faelle {
        let ant = ZonenAntwort {
            grund: g,
            klasse: None,
            protokoll: None,
        };
        assert_eq!(g.code(), code);
        assert_eq!(ant.meldung(r), meldung);
        assert_eq!(ant.darf(), g == G::Tor(Grund::Frei));
    }
    let ant = ZonenAntwort {
        grund: G::RouteOhneZone,
        klasse: None,
        protokoll: None,
    };
    assert_eq!(ant.meldung(None), "Route  hat keine Zone");
}

fn neu(
    zonen: Vec<Zone>,
    freigaben: Vec<ProjektFreigabe>,
    ausnahmen: Vec<Ausnahme>,
    regeln: Vec<Regel>,
) -> Result<Zonen, ZonenFehler> {
    Zonen::neu(&vorgabe(), true, zonen, freigaben, ausnahmen, regeln)
}

#[test]
fn ungueltige_zonen() {
    use ZonenFehler as F;
    let z = zonen_liste;
    let mit = |f: fn(&mut Vec<Zone>)| {
        let mut l = z();
        f(&mut l);
        neu(l, vec![], vec![], vec![]).err()
    };
    assert!(neu(z(), vec![], vec![], vec![]).is_ok());
    assert!(neu(vec![], vec![], vec![], vec![]).is_ok());
    assert_eq!(
        mit(|l| l[1].schluessel = s("Z")),
        Some(F::SchluesselForm(s("Z")))
    );
    assert_eq!(
        mit(|l| l[2].schluessel = s("z1")),
        Some(F::ZoneDoppelt(s("z1")))
    );
    assert_eq!(
        mit(|l| l[0].bedingung = Bedingung::Nachrangig),
        Some(F::NachrangigZuerst(s("z1")))
    );
    assert_eq!(
        mit(|l| l[1].routen.push(s("q"))),
        Some(F::RouteUnbekannt {
            zone: s("z2"),
            route: s("q")
        })
    );
    assert_eq!(
        mit(|l| l[1].routen.push(s("c1"))),
        Some(F::RouteDoppelt(s("c1")))
    );
    assert_eq!(
        mit(|l| l[1].routen.push(s("l1"))),
        Some(F::RouteDoppelt(s("l1")))
    );
    // A route without a zone is allowed.
    assert_eq!(mit(|l| l[1].routen.clear()), None);
}

#[test]
fn ungueltige_freigaben_ausnahmen_und_regeln() {
    use Klassensatz::{Alle, Keine, Niedrigste};
    use ZonenFehler as F;
    let f = |x: ProjektFreigabe| neu(zonen_liste(), vec![x], vec![], vec![]).err();
    assert_eq!(f(freigabe("c2", "k", "p1")), None);
    assert_eq!(
        f(freigabe("cx", "k", "p1")),
        Some(F::FreigabeOhneZone(s("cx")))
    );
    assert_eq!(
        f(freigabe("c1", "k", "p1")),
        Some(F::FreigabeOhneBedingung(s("c1")))
    );
    assert_eq!(
        f(freigabe("c2", "q", "p1")),
        Some(F::FreigabeKundeUnbekannt(s("q")))
    );
    assert_eq!(
        f(freigabe("c2", "k", "P1")),
        Some(F::SchluesselForm(s("P1")))
    );

    let a = |x: Ausnahme| neu(zonen_liste(), vec![], vec![x], vec![]).err();
    assert_eq!(a(ausnahme("z5", &["n1"], &[], 0)), None);
    assert_eq!(a(ausnahme("z5", &[], &["g1"], u64::MAX)), None);
    assert_eq!(
        a(ausnahme("zq", &["n1"], &[], 1)),
        Some(F::AusnahmeZoneUnbekannt(s("zq")))
    );
    assert_eq!(
        a(ausnahme("z1", &["n1"], &[], 1)),
        Some(F::AusnahmeZoneNichtGesperrt(s("z1")))
    );
    assert_eq!(
        a(ausnahme("z5", &[], &[], 1)),
        Some(F::AusnahmeOhneWen(s("z5")))
    );
    assert_eq!(
        a(ausnahme("z5", &["N1"], &[], 1)),
        Some(F::SchluesselForm(s("N1")))
    );
    assert_eq!(
        a(ausnahme("z5", &["n1"], &["G1"], 1)),
        Some(F::SchluesselForm(s("G1")))
    );
    let mut x = ausnahme("z5", &["n1"], &[], 1);
    x.gesetzt_von = s("Admin");
    assert_eq!(a(x), Some(F::SchluesselForm(s("Admin"))));
    for leer in ["", " ", " \t\r\n"] {
        let mut x = ausnahme("z5", &["n1"], &[], 1);
        x.zweck = s(leer);
        assert_eq!(a(x), Some(F::ZweckLeer(s("z5"))), "{leer:?}");
    }
    let mut x = ausnahme("z5", &["n1"], &[], 1);
    x.zweck = s("\u{a0}");
    assert_eq!(a(x), None);

    let r = |x: Regel| neu(zonen_liste(), vec![], vec![], vec![x]).err();
    assert_eq!(r(regel(Ebene::Nutzer, "n1", "z1", None, true)), None);
    assert_eq!(
        r(regel(Ebene::Nutzer, "N1", "z1", None, true)),
        Some(F::SchluesselForm(s("N1")))
    );
    assert_eq!(
        r(regel(Ebene::Gruppe, "g1", "zq", None, true)),
        Some(F::RegelZoneUnbekannt(s("zq")))
    );
    assert_eq!(
        r(regel(Ebene::Gruppe, "g1", "z1", None, false)),
        Some(F::RegelOhneWirkung(s("g1")))
    );
    assert_eq!(r(regel(Ebene::Gruppe, "g1", "z1", Some(Keine), true)), None);
    let lockert = Some(F::RegelLockert {
        wer: s("g1"),
        zone: s("z4"),
    });
    assert_eq!(
        r(regel(Ebene::Gruppe, "g1", "z4", Some(Alle), false)),
        lockert
    );
    assert_eq!(
        r(regel(Ebene::Gruppe, "g1", "z4", Some(Niedrigste), false)),
        None
    );
    assert_eq!(
        r(regel(Ebene::Gruppe, "g1", "z4", Some(Keine), false)),
        None
    );
    assert!(r(regel(Ebene::Gruppe, "g1", "z5", Some(Niedrigste), false)).is_some());
    assert_eq!(
        r(regel(Ebene::Gruppe, "g1", "z5", Some(Keine), false)),
        None
    );
    assert_eq!(r(regel(Ebene::Gruppe, "g1", "z1", Some(Alle), false)), None);
}

#[test]
fn lockerung_nach_position_bei_einer_klasse() {
    // With one class, niedrigste and alle are the same position: no loosening.
    let v = Vorgabe::neu(
        sv(&["nur"]),
        None,
        vec![route("r", Art::Lokal, None)],
        vec![],
        vec![],
    )
    .expect("valid");
    let z = Zonen::neu(
        &v,
        true,
        vec![zone("z", Klassensatz::Niedrigste, Bedingung::Immer, &["r"])],
        vec![],
        vec![],
        vec![regel(
            Ebene::Nutzer,
            "n",
            "z",
            Some(Klassensatz::Alle),
            false,
        )],
    );
    assert!(z.is_ok());
}

#[test]
fn fehlertexte() {
    use ZonenFehler as F;
    let faelle = [
        (
            F::SchluesselForm(s("X")),
            "key \"X\" does not have the key form",
        ),
        (F::ZoneDoppelt(s("z")), "zone \"z\" occurs twice"),
        (
            F::NachrangigZuerst(s("z")),
            "the first zone \"z\" has the condition nachrangig",
        ),
        (
            F::RouteUnbekannt {
                zone: s("z"),
                route: s("r"),
            },
            "zone \"z\" names the unknown route \"r\"",
        ),
        (
            F::RouteDoppelt(s("r")),
            "route \"r\" stands in more than one zone place",
        ),
        (
            F::FreigabeOhneZone(s("r")),
            "release names the route \"r\" without a zone",
        ),
        (
            F::FreigabeOhneBedingung(s("r")),
            "release names the route \"r\" whose zone is not freigabe",
        ),
        (
            F::FreigabeKundeUnbekannt(s("k")),
            "release names the unknown customer \"k\"",
        ),
        (
            F::AusnahmeZoneUnbekannt(s("z")),
            "exception names the unknown zone \"z\"",
        ),
        (
            F::AusnahmeZoneNichtGesperrt(s("z")),
            "exception names the zone \"z\" that is not gesperrt",
        ),
        (
            F::AusnahmeOhneWen(s("z")),
            "exception for zone \"z\" names neither users nor groups",
        ),
        (
            F::ZweckLeer(s("z")),
            "exception for zone \"z\" has an empty purpose",
        ),
        (
            F::RegelZoneUnbekannt(s("z")),
            "rule names the unknown zone \"z\"",
        ),
        (
            F::RegelOhneWirkung(s("w")),
            "rule of \"w\" has neither klassen nor gesperrt",
        ),
        (
            F::RegelLockert {
                wer: s("w"),
                zone: s("z"),
            },
            "rule of \"w\" loosens zone \"z\"",
        ),
    ];
    for (f, text) in faelle {
        assert_eq!(f.to_string(), text);
    }
}

#[test]
fn gleiche_eingabe_gleiche_antwort() {
    let z = zonen(true);
    let x = a("a", "c3");
    assert_eq!(frage(&z, &x), frage(&z, &x));
}
