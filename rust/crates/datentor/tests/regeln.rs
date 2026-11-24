//! Unit tests of the rules D2 to D15 and of the checks of `Vorgabe::neu` (contract sections 2 to 4). All data are invented.

use datentor::{
    Antwort, Art, Einheit, Freigabe, Grund, Kunde, Route, Vorgabe, VorgabeFehler, darf,
};

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

fn kunde(schluessel: &str, erlaubt: &[&str], verboten: &[&str]) -> Kunde {
    Kunde {
        schluessel: s(schluessel),
        erlaubt: erlaubt.iter().map(|x| s(x)).collect(),
        verboten: verboten.iter().map(|x| s(x)).collect(),
    }
}

fn freigabe(route: &str, kunde: &str) -> Freigabe {
    Freigabe {
        route: s(route),
        kunde: s(kunde),
    }
}

fn klassen() -> Vec<String> {
    vec![s("oeffentlich"), s("intern"), s("kundendaten")]
}

fn routen() -> Vec<Route> {
    vec![
        route("c-oef", Art::Cloud, Some("oeffentlich")),
        route("c-int", Art::Cloud, Some("intern")),
        route("c-kd", Art::Cloud, Some("kundendaten")),
        route("l-oef", Art::Lokal, Some("oeffentlich")),
        route("l-int", Art::Lokal, Some("intern")),
        route("l-kd", Art::Lokal, Some("kundendaten")),
        route("l-ohne", Art::Lokal, None),
        route("c-ohne", Art::Cloud, None),
    ]
}

fn kunden() -> Vec<Kunde> {
    vec![
        kunde("k-a", &["l-kd", "c-kd", "c-int", "l-int", "c-oef"], &[]),
        kunde("k-b", &["l-kd", "c-kd"], &[]),
        kunde("k-v", &["l-kd", "c-kd"], &["l-kd"]),
        kunde("k-vo", &[], &["c-kd"]),
        kunde("k-leer", &[], &[]),
        kunde("k-c", &["c-kd", "l-kd"], &[]),
    ]
}

fn vorgabe_mit(stufe: Option<&str>) -> Vorgabe {
    Vorgabe::neu(
        klassen(),
        stufe.map(s),
        routen(),
        kunden(),
        vec![freigabe("c-kd", "k-a"), freigabe("c-int", "k-a")],
    )
    .expect("valid settings")
}

fn vorgabe() -> Vorgabe {
    vorgabe_mit(None)
}

fn frage(v: &Vorgabe, klasse: Option<&str>, route: Option<&str>, kunde: Option<&str>) -> Antwort {
    darf(v, &Einheit { klasse }, route, kunde)
}

/// The answer must have this reason and this effective class.
#[track_caller]
fn erwarte(a: &Antwort, grund: Grund, klasse: Option<&str>) {
    assert_eq!(a.grund, grund, "reason of {a:?}");
    assert_eq!(a.klasse.as_deref(), klasse, "class of {a:?}");
    assert_eq!(a.darf(), grund == Grund::Frei);
}

#[test]
fn d2_route_fehlt() {
    let v = vorgabe();
    erwarte(
        &frage(&v, Some("intern"), None, None),
        Grund::RouteFehlt,
        Some("intern"),
    );
    erwarte(
        &frage(&v, Some("intern"), Some(""), None),
        Grund::RouteFehlt,
        Some("intern"),
    );
    // The route is not trimmed: a blank is a route that does not exist (D3, D14).
    erwarte(
        &frage(&v, Some("intern"), Some(" "), None),
        Grund::RouteOhneKlasse,
        Some("intern"),
    );
    // D2 comes before D3 and D4; the class is still given as far as it is valid.
    erwarte(
        &frage(&v, Some("geheim"), None, None),
        Grund::RouteFehlt,
        None,
    );
    erwarte(
        &frage(&v, None, None, None),
        Grund::RouteFehlt,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("oeffentlich"), None, Some("k-a")),
        Grund::RouteFehlt,
        Some("kundendaten"),
    );
}

#[test]
fn d3_route_ohne_klasse() {
    let v = vorgabe();
    erwarte(
        &frage(&v, Some("intern"), Some("gibt-es-nicht"), None),
        Grund::RouteOhneKlasse,
        Some("intern"),
    );
    erwarte(
        &frage(&v, Some("intern"), Some("l-ohne"), None),
        Grund::RouteOhneKlasse,
        Some("intern"),
    );
    erwarte(
        &frage(&v, Some("intern"), Some("c-ohne"), None),
        Grund::RouteOhneKlasse,
        Some("intern"),
    );
    // D3 comes before D4.
    erwarte(
        &frage(&v, Some("geheim"), Some("l-ohne"), None),
        Grund::RouteOhneKlasse,
        None,
    );
    erwarte(
        &frage(&v, Some("geheim"), Some("gibt-es-nicht"), Some("k-a")),
        Grund::RouteOhneKlasse,
        None,
    );
    // D3 comes before D6 and D7..D11.
    erwarte(
        &frage(&v, Some("kundendaten"), Some("l-ohne"), None),
        Grund::RouteOhneKlasse,
        Some("kundendaten"),
    );
}

#[test]
fn d4_einheit_ohne_klasse() {
    let v = vorgabe();
    for k in [
        "geheim", "", "INTERN", " intern", "intern ", "intern\n", "inter", "internx",
    ] {
        erwarte(
            &frage(&v, Some(k), Some("l-kd"), None),
            Grund::EinheitOhneKlasse,
            None,
        );
        erwarte(
            &frage(&v, Some(k), Some("l-kd"), Some("k-a")),
            Grund::EinheitOhneKlasse,
            None,
        );
    }
}

#[test]
fn d5_wirksame_klasse() {
    let v = vorgabe();
    // Without a class: the highest class by default (A35).
    erwarte(
        &frage(&v, None, Some("l-kd"), Some("k-a")),
        Grund::Frei,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, None, Some("l-int"), None),
        Grund::KlasseUeberRoute,
        Some("kundendaten"),
    );
    // The vorgabestufe is used, wherever it stands.
    for (stufe, klasse) in [
        ("oeffentlich", "oeffentlich"),
        ("intern", "intern"),
        ("kundendaten", "kundendaten"),
    ] {
        let v = vorgabe_mit(Some(stufe));
        erwarte(
            &frage(&v, None, Some("c-oef"), None),
            if stufe == "oeffentlich" {
                Grund::Frei
            } else {
                Grund::KlasseUeberRoute
            },
            Some(klasse),
        );
    }
    let v = vorgabe_mit(Some("intern"));
    erwarte(
        &frage(&v, None, Some("l-int"), None),
        Grund::Frei,
        Some("intern"),
    );
    erwarte(
        &frage(&v, None, Some("l-kd"), None),
        Grund::Frei,
        Some("intern"),
    );
    // A customer lifts every class to the highest.
    erwarte(
        &frage(&v, None, Some("l-kd"), Some("k-a")),
        Grund::Frei,
        Some("kundendaten"),
    );
    for k in ["oeffentlich", "intern", "kundendaten"] {
        erwarte(
            &frage(&v, Some(k), Some("l-int"), Some("k-a")),
            Grund::KlasseUeberRoute,
            Some("kundendaten"),
        );
        erwarte(
            &frage(&v, Some(k), Some("l-kd"), Some("k-a")),
            Grund::Frei,
            Some("kundendaten"),
        );
        erwarte(
            &frage(&v, Some(k), Some("l-kd"), None),
            if k == "kundendaten" {
                Grund::KundeFehlt
            } else {
                Grund::Frei
            },
            Some(k),
        );
    }
    // The empty text is a customer, too: it lifts the class, and it has no entry (D8).
    erwarte(
        &frage(&v, Some("intern"), Some("l-kd"), Some("")),
        Grund::KundeOhneEintrag,
        Some("kundendaten"),
    );
}

#[test]
fn d6_klassenordnung() {
    let v = vorgabe();
    let namen = ["oeffentlich", "intern", "kundendaten"];
    let routen = ["l-oef", "l-int", "l-kd"];
    for (u, einheit) in namen.iter().enumerate() {
        for (r, route) in routen.iter().enumerate() {
            let a = frage(&v, Some(einheit), Some(route), None);
            let grund = if u > r {
                Grund::KlasseUeberRoute
            } else if u == 2 {
                Grund::KundeFehlt
            } else {
                Grund::Frei
            };
            erwarte(&a, grund, Some(einheit));
        }
    }
    // A single class: unit and route are on the same step.
    let eine = Vorgabe::neu(
        vec![s("nur")],
        None,
        vec![route("r", Art::Lokal, Some("nur"))],
        vec![kunde("k", &["r"], &[])],
        vec![],
    )
    .expect("valid settings");
    erwarte(
        &frage(&eine, Some("nur"), Some("r"), Some("k")),
        Grund::Frei,
        Some("nur"),
    );
    erwarte(
        &frage(&eine, Some("nur"), Some("r"), None),
        Grund::KundeFehlt,
        Some("nur"),
    );
    erwarte(
        &frage(&eine, None, Some("r"), Some("k")),
        Grund::Frei,
        Some("nur"),
    );
}

#[test]
fn d6_vier_klassen_haengt_an_der_stellung() {
    // The gate compares positions and knows no class name (A29).
    let v = Vorgabe::neu(
        vec![s("a"), s("b"), s("c"), s("d")],
        Some(s("b")),
        vec![
            route("rb", Art::Lokal, Some("b")),
            route("rc", Art::Lokal, Some("c")),
        ],
        vec![],
        vec![],
    )
    .expect("valid settings");
    erwarte(
        &frage(&v, Some("c"), Some("rb"), None),
        Grund::KlasseUeberRoute,
        Some("c"),
    );
    erwarte(
        &frage(&v, Some("b"), Some("rc"), None),
        Grund::Frei,
        Some("b"),
    );
    erwarte(&frage(&v, None, Some("rb"), None), Grund::Frei, Some("b"));
    erwarte(
        &frage(&v, Some("d"), Some("rc"), None),
        Grund::KlasseUeberRoute,
        Some("d"),
    );
    erwarte(
        &frage(&v, Some("a"), Some("rb"), Some("k")),
        Grund::KlasseUeberRoute,
        Some("d"),
    );
}

#[test]
fn d7_kunde_fehlt() {
    let v = vorgabe();
    erwarte(
        &frage(&v, Some("kundendaten"), Some("l-kd"), None),
        Grund::KundeFehlt,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, None, Some("l-kd"), None),
        Grund::KundeFehlt,
        Some("kundendaten"),
    );
    // Not customer data: no customer needed, whatever route.
    erwarte(
        &frage(&v, Some("intern"), Some("l-kd"), None),
        Grund::Frei,
        Some("intern"),
    );
    erwarte(
        &frage(&v, Some("intern"), Some("c-kd"), None),
        Grund::Frei,
        Some("intern"),
    );
}

#[test]
fn d8_bis_d10_kundenvorgabe() {
    let v = vorgabe();
    erwarte(
        &frage(&v, Some("kundendaten"), Some("l-kd"), Some("k-x")),
        Grund::KundeOhneEintrag,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("intern"), Some("l-kd"), Some("k-x")),
        Grund::KundeOhneEintrag,
        Some("kundendaten"),
    );
    // D9: forbidden goes before allowed, and before D10.
    erwarte(
        &frage(&v, Some("kundendaten"), Some("l-kd"), Some("k-v")),
        Grund::RouteFuerKundeVerboten,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("kundendaten"), Some("c-kd"), Some("k-vo")),
        Grund::RouteFuerKundeVerboten,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("kundendaten"), Some("l-kd"), Some("k-vo")),
        Grund::RouteFuerKundeNichtErlaubt,
        Some("kundendaten"),
    );
    // D10: a customer without a list allows nothing; a route not in the list is not allowed.
    erwarte(
        &frage(&v, Some("kundendaten"), Some("l-kd"), Some("k-leer")),
        Grund::RouteFuerKundeNichtErlaubt,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("kundendaten"), Some("c-kd"), Some("k-leer")),
        Grund::RouteFuerKundeNichtErlaubt,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("kundendaten"), Some("l-kd"), Some("k-b")),
        Grund::Frei,
        Some("kundendaten"),
    );
    // The list of another customer does not count.
    erwarte(
        &frage(&v, Some("oeffentlich"), Some("l-kd"), Some("k-leer")),
        Grund::RouteFuerKundeNichtErlaubt,
        Some("kundendaten"),
    );
}

#[test]
fn d11_kundendaten_an_cloud() {
    let v = vorgabe();
    // Release for exactly (c-kd, k-a).
    erwarte(
        &frage(&v, Some("kundendaten"), Some("c-kd"), Some("k-a")),
        Grund::Frei,
        Some("kundendaten"),
    );
    // Same route, another customer who allows it: no release.
    erwarte(
        &frage(&v, Some("kundendaten"), Some("c-kd"), Some("k-b")),
        Grund::KundendatenAnCloud,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("kundendaten"), Some("c-kd"), Some("k-c")),
        Grund::KundendatenAnCloud,
        Some("kundendaten"),
    );
    // The release does not lift the class: (c-int, k-a) is released, but the route sees only `intern` (D6).
    erwarte(
        &frage(&v, Some("kundendaten"), Some("c-int"), Some("k-a")),
        Grund::KlasseUeberRoute,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&v, Some("intern"), Some("c-int"), Some("k-a")),
        Grund::KlasseUeberRoute,
        Some("kundendaten"),
    );
    // Customer allows the route and the route has the class, but there is no release for the pair.
    let ohne = Vorgabe::neu(klassen(), None, routen(), kunden(), vec![]).expect("valid settings");
    erwarte(
        &frage(&ohne, Some("kundendaten"), Some("c-kd"), Some("k-a")),
        Grund::KundendatenAnCloud,
        Some("kundendaten"),
    );
    // A local route needs no release.
    erwarte(
        &frage(&ohne, Some("kundendaten"), Some("l-kd"), Some("k-a")),
        Grund::Frei,
        Some("kundendaten"),
    );
    // A release for the pair is not a release for the route with another customer, nor for the customer with another route.
    let kreuz = Vorgabe::neu(
        klassen(),
        None,
        vec![
            route("c1", Art::Cloud, Some("kundendaten")),
            route("c2", Art::Cloud, Some("kundendaten")),
        ],
        vec![
            kunde("k1", &["c1", "c2"], &[]),
            kunde("k2", &["c1", "c2"], &[]),
        ],
        vec![freigabe("c1", "k1"), freigabe("c2", "k2")],
    )
    .expect("valid settings");
    erwarte(
        &frage(&kreuz, Some("kundendaten"), Some("c1"), Some("k1")),
        Grund::Frei,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&kreuz, Some("kundendaten"), Some("c2"), Some("k2")),
        Grund::Frei,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&kreuz, Some("kundendaten"), Some("c1"), Some("k2")),
        Grund::KundendatenAnCloud,
        Some("kundendaten"),
    );
    erwarte(
        &frage(&kreuz, Some("kundendaten"), Some("c2"), Some("k1")),
        Grund::KundendatenAnCloud,
        Some("kundendaten"),
    );
    // D10 comes before D11: a cloud route the customer does not allow ends in D10, with or without release.
    let nicht_erlaubt = Vorgabe::neu(
        klassen(),
        None,
        vec![route("c", Art::Cloud, Some("kundendaten"))],
        vec![kunde("k", &[], &[])],
        vec![freigabe("c", "k")],
    )
    .expect("valid settings");
    erwarte(
        &frage(&nicht_erlaubt, Some("kundendaten"), Some("c"), Some("k")),
        Grund::RouteFuerKundeNichtErlaubt,
        Some("kundendaten"),
    );
}

#[test]
fn d11_gilt_nur_fuer_kundendaten() {
    // Not customer data: a cloud route needs no release and no customer entry.
    let ohne = Vorgabe::neu(klassen(), None, routen(), kunden(), vec![]).expect("valid settings");
    erwarte(
        &frage(&ohne, Some("intern"), Some("c-int"), None),
        Grund::Frei,
        Some("intern"),
    );
    erwarte(
        &frage(&ohne, Some("oeffentlich"), Some("c-oef"), None),
        Grund::Frei,
        Some("oeffentlich"),
    );
    erwarte(
        &frage(&ohne, Some("oeffentlich"), Some("c-kd"), None),
        Grund::Frei,
        Some("oeffentlich"),
    );
}

#[test]
fn d14_byte_genau() {
    let v = vorgabe();
    for r in [
        "C-KD",
        "c-kd ",
        " c-kd",
        "c-kd\n",
        "c\u{2011}kd",
        "c-k\u{64}\u{301}",
    ] {
        erwarte(
            &frage(&v, Some("intern"), Some(r), None),
            Grund::RouteOhneKlasse,
            Some("intern"),
        );
    }
    for k in ["K-A", "k-a ", " k-a", "k-a\n", "k\u{2011}a"] {
        erwarte(
            &frage(&v, Some("kundendaten"), Some("l-kd"), Some(k)),
            Grund::KundeOhneEintrag,
            Some("kundendaten"),
        );
    }
    for k in ["Intern", "intern\u{0}", "inte\u{301}rn"] {
        erwarte(
            &frage(&v, Some(k), Some("l-kd"), None),
            Grund::EinheitOhneKlasse,
            None,
        );
    }
}

#[test]
fn d15_rein_und_ohne_panik() {
    let v = vorgabe();
    let lang = "x".repeat(100_000);
    let seltsam = [
        "",
        " ",
        "\0",
        "\u{10FFFF}",
        "ä",
        "k-a",
        "c-kd",
        "intern",
        "kundendaten",
        lang.as_str(),
    ];
    for klasse in std::iter::once(None).chain(seltsam.iter().map(|x| Some(*x))) {
        for route in std::iter::once(None).chain(seltsam.iter().map(|x| Some(*x))) {
            for kunde in std::iter::once(None).chain(seltsam.iter().map(|x| Some(*x))) {
                let a = frage(&v, klasse, route, kunde);
                assert_eq!(a, frage(&v, klasse, route, kunde));
                // The message never carries the class or the customer.
                let m = a.meldung(route);
                assert!(!m.contains("k-a") || route == Some("k-a"));
            }
        }
    }
}

#[test]
fn die_antwort_nennt_den_kunden_nie() {
    let v = vorgabe();
    for r in ["l-kd", "c-kd", "c-int", "l-int", "l-ohne", "gibt-es-nicht"] {
        for k in ["k-a", "k-v", "k-vo", "k-x"] {
            let a = frage(&v, Some("kundendaten"), Some(r), Some(k));
            assert!(!a.meldung(Some(r)).contains(k), "{a:?} names the customer");
        }
    }
}

#[test]
fn codes_und_meldungen() {
    let alle = [
        (Grund::Frei, "frei", "frei"),
        (Grund::RouteFehlt, "route-fehlt", "Anfrage ohne Route"),
        (
            Grund::RouteOhneKlasse,
            "route-ohne-klasse",
            "Route r-1 hat keine Klasse",
        ),
        (
            Grund::EinheitOhneKlasse,
            "einheit-ohne-klasse",
            "Einheit ohne gültige Klasse",
        ),
        (
            Grund::KlasseUeberRoute,
            "klasse-ueber-route",
            "Klasse der Einheit liegt über Route r-1",
        ),
        (Grund::KundeFehlt, "kunde-fehlt", "Kundendaten ohne Kunde"),
        (
            Grund::KundeOhneEintrag,
            "kunde-ohne-eintrag",
            "Kunde ohne Eintrag in der Kundenvorgabe",
        ),
        (
            Grund::RouteFuerKundeVerboten,
            "route-fuer-kunde-verboten",
            "Route r-1 ist für diesen Kunden verboten",
        ),
        (
            Grund::RouteFuerKundeNichtErlaubt,
            "route-fuer-kunde-nicht-erlaubt",
            "Route r-1 ist für diesen Kunden nicht erlaubt",
        ),
        (
            Grund::KundendatenAnCloud,
            "kundendaten-an-cloud",
            "Kundendaten an Cloud-Route r-1 ohne Freigabe",
        ),
    ];
    for (grund, code, meldung) in alle {
        assert_eq!(grund.code(), code);
        let a = Antwort {
            grund,
            klasse: None,
        };
        assert_eq!(a.meldung(Some("r-1")), meldung);
        assert_eq!(a.darf(), grund == Grund::Frei);
    }
    // Without a route the placeholder is empty.
    let a = Antwort {
        grund: Grund::RouteOhneKlasse,
        klasse: None,
    };
    assert_eq!(a.meldung(None), "Route  hat keine Klasse");
}

// ---------------------------------------------------------------- Vorgabe::neu

fn schluessel_form_soll(s: &str) -> bool {
    // Written out independently of the library: ^[a-z0-9][a-z0-9._-]{0,63}$
    let b: Vec<char> = s.chars().collect();
    if b.is_empty() || b.len() > 64 {
        return false;
    }
    let anfang = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    let rest = |c: char| anfang(c) || c == '.' || c == '_' || c == '-';
    anfang(b[0]) && b[1..].iter().all(|&c| rest(c))
}

fn mit_klasse(k: &str) -> Result<Vorgabe, VorgabeFehler> {
    Vorgabe::neu(vec![s(k)], None, vec![], vec![], vec![])
}

#[test]
fn schluesselform_jedes_ascii_zeichen() {
    for code in 0_u8..=127 {
        let c = char::from(code);
        // As the first character and as a later one; the key also gets a valid partner so only the form matters.
        for k in [format!("{c}"), format!("a{c}"), format!("{c}a")] {
            let ok = mit_klasse(&k).is_ok();
            assert_eq!(ok, schluessel_form_soll(&k), "key {k:?}");
            if !ok {
                assert_eq!(
                    mit_klasse(&k).err(),
                    Some(VorgabeFehler::SchluesselForm(k.clone()))
                );
            }
        }
    }
}

#[test]
fn schluesselform_laenge_und_nicht_ascii() {
    let a63 = "a".repeat(63);
    let a64 = "a".repeat(64);
    let a65 = "a".repeat(65);
    let strich64 = format!("a{}", "-".repeat(63));
    let strich65 = format!("a{}", "-".repeat(64));
    for (k, ok) in [
        ("", false),
        ("a", true),
        ("0", true),
        ("-a", false),
        (".a", false),
        ("_a", false),
        ("a.b_c-d", true),
        (a63.as_str(), true),
        (a64.as_str(), true),
        (a65.as_str(), false),
        (strich64.as_str(), true),
        (strich65.as_str(), false),
        ("ä", false),
        ("aä", false),
        ("a\u{301}", false),
        ("a b", false),
        ("a\n", false),
        ("A", false),
        ("aZ", false),
    ] {
        assert_eq!(mit_klasse(k).is_ok(), ok, "key {k:?}");
    }
}

#[test]
fn neu_ohne_klassen_und_doppelte_klassen() {
    assert_eq!(
        Vorgabe::neu(vec![], None, vec![], vec![], vec![]).err(),
        Some(VorgabeFehler::KlassenLeer)
    );
    assert_eq!(
        Vorgabe::neu(vec![s("a"), s("b"), s("a")], None, vec![], vec![], vec![]).err(),
        Some(VorgabeFehler::SchluesselDoppelt(s("a")))
    );
    assert!(Vorgabe::neu(vec![s("a"), s("b")], None, vec![], vec![], vec![]).is_ok());
}

#[test]
fn neu_vorgabestufe() {
    let f = |stufe: Option<&str>| {
        Vorgabe::neu(vec![s("a"), s("b")], stufe.map(s), vec![], vec![], vec![]).err()
    };
    assert_eq!(
        f(Some("c")),
        Some(VorgabeFehler::VorgabestufeUnbekannt(s("c")))
    );
    assert_eq!(
        f(Some("A")),
        Some(VorgabeFehler::VorgabestufeUnbekannt(s("A")))
    );
    assert_eq!(
        f(Some("")),
        Some(VorgabeFehler::VorgabestufeUnbekannt(s("")))
    );
    assert_eq!(f(Some("a")), None);
    assert_eq!(f(Some("b")), None);
    assert_eq!(f(None), None);
}

#[test]
fn neu_routen() {
    let f =
        |routen: Vec<Route>| Vorgabe::neu(vec![s("a"), s("b")], None, routen, vec![], vec![]).err();
    assert_eq!(
        f(vec![route("R", Art::Cloud, None)]),
        Some(VorgabeFehler::SchluesselForm(s("R")))
    );
    assert_eq!(
        f(vec![
            route("r", Art::Cloud, None),
            route("r", Art::Lokal, Some("a"))
        ]),
        Some(VorgabeFehler::SchluesselDoppelt(s("r")))
    );
    assert_eq!(
        f(vec![route("r", Art::Cloud, Some("c"))]),
        Some(VorgabeFehler::RouteKlasseUnbekannt {
            route: s("r"),
            klasse: s("c")
        })
    );
    assert_eq!(
        f(vec![route("r", Art::Cloud, Some(""))]),
        Some(VorgabeFehler::RouteKlasseUnbekannt {
            route: s("r"),
            klasse: s("")
        })
    );
    assert_eq!(
        f(vec![
            route("r", Art::Cloud, None),
            route("q", Art::Lokal, Some("b"))
        ]),
        None
    );
}

#[test]
fn neu_kunden() {
    let r = || {
        vec![
            route("r1", Art::Lokal, Some("a")),
            route("r2", Art::Cloud, Some("a")),
        ]
    };
    let f = |kunden: Vec<Kunde>| Vorgabe::neu(vec![s("a")], None, r(), kunden, vec![]).err();
    assert_eq!(
        f(vec![kunde("K", &[], &[])]),
        Some(VorgabeFehler::SchluesselForm(s("K")))
    );
    assert_eq!(
        f(vec![kunde("k", &[], &[]), kunde("k", &[], &[])]),
        Some(VorgabeFehler::SchluesselDoppelt(s("k")))
    );
    assert_eq!(
        f(vec![kunde("k", &["r1", "r9"], &[])]),
        Some(VorgabeFehler::RouteUnbekannt {
            kunde: s("k"),
            route: s("r9")
        })
    );
    assert_eq!(
        f(vec![kunde("k", &[], &["r9"])]),
        Some(VorgabeFehler::RouteUnbekannt {
            kunde: s("k"),
            route: s("r9")
        })
    );
    assert_eq!(
        f(vec![kunde("k", &["R1"], &[])]),
        Some(VorgabeFehler::RouteUnbekannt {
            kunde: s("k"),
            route: s("R1")
        })
    );
    // The same route twice in one list is no violation of the contract.
    assert_eq!(f(vec![kunde("k", &["r1", "r1"], &["r2", "r2"])]), None);
    assert_eq!(
        f(vec![
            kunde("k", &["r1", "r2"], &["r1"]),
            kunde("l", &[], &[])
        ]),
        None
    );
}

#[test]
fn neu_freigaben() {
    let r = || {
        vec![
            route("c", Art::Cloud, Some("a")),
            route("l", Art::Lokal, Some("a")),
        ]
    };
    let k = || vec![kunde("k", &["c"], &[])];
    let f = |fr: Vec<Freigabe>| Vorgabe::neu(vec![s("a")], None, r(), k(), fr).err();
    assert_eq!(
        f(vec![freigabe("x", "k")]),
        Some(VorgabeFehler::FreigabeRouteUnbekannt(s("x")))
    );
    assert_eq!(
        f(vec![freigabe("l", "k")]),
        Some(VorgabeFehler::FreigabeRouteLokal(s("l")))
    );
    assert_eq!(
        f(vec![freigabe("c", "x")]),
        Some(VorgabeFehler::FreigabeKundeUnbekannt(s("x")))
    );
    assert_eq!(f(vec![freigabe("c", "k")]), None);
    // A pair twice is no violation of the contract.
    assert_eq!(f(vec![freigabe("c", "k"), freigabe("c", "k")]), None);
    // The first violation counts, in the order of the list.
    assert_eq!(
        f(vec![freigabe("c", "x"), freigabe("l", "k")]),
        Some(VorgabeFehler::FreigabeKundeUnbekannt(s("x")))
    );
    assert_eq!(
        f(vec![freigabe("c", "k"), freigabe("l", "k")]),
        Some(VorgabeFehler::FreigabeRouteLokal(s("l")))
    );
    // A release with a route that does not exist and a customer that does not exist names the route.
    assert_eq!(
        f(vec![freigabe("x", "y")]),
        Some(VorgabeFehler::FreigabeRouteUnbekannt(s("x")))
    );
}

#[test]
fn neu_erste_verletzung_in_der_reihenfolge_des_vertrags() {
    // classes before vorgabestufe before routes before customers before releases
    let r = vec![route("R", Art::Cloud, Some("zz"))];
    let k = vec![kunde("K", &["nix"], &[])];
    let fr = vec![freigabe("x", "y")];
    assert_eq!(
        Vorgabe::neu(vec![s("A")], Some(s("q")), r.clone(), k.clone(), fr.clone()).err(),
        Some(VorgabeFehler::SchluesselForm(s("A")))
    );
    assert_eq!(
        Vorgabe::neu(vec![s("a")], Some(s("q")), r.clone(), k.clone(), fr.clone()).err(),
        Some(VorgabeFehler::VorgabestufeUnbekannt(s("q")))
    );
    assert_eq!(
        Vorgabe::neu(vec![s("a")], None, r.clone(), k.clone(), fr.clone()).err(),
        Some(VorgabeFehler::SchluesselForm(s("R")))
    );
    let r = vec![route("r", Art::Cloud, Some("zz"))];
    assert_eq!(
        Vorgabe::neu(vec![s("a")], None, r, k.clone(), fr.clone()).err(),
        Some(VorgabeFehler::RouteKlasseUnbekannt {
            route: s("r"),
            klasse: s("zz")
        })
    );
    let r = vec![route("r", Art::Cloud, None)];
    assert_eq!(
        Vorgabe::neu(vec![s("a")], None, r.clone(), k, fr.clone()).err(),
        Some(VorgabeFehler::SchluesselForm(s("K")))
    );
    let k = vec![kunde("k", &["nix"], &[])];
    assert_eq!(
        Vorgabe::neu(vec![s("a")], None, r, k, fr).err(),
        Some(VorgabeFehler::RouteUnbekannt {
            kunde: s("k"),
            route: s("nix")
        })
    );
}

#[test]
fn fehlertexte() {
    let faelle = [
        (VorgabeFehler::KlassenLeer, "klassen is empty"),
        (
            VorgabeFehler::SchluesselForm(s("A")),
            "key \"A\" does not have the key form",
        ),
        (
            VorgabeFehler::SchluesselDoppelt(s("a")),
            "key \"a\" occurs twice",
        ),
        (
            VorgabeFehler::VorgabestufeUnbekannt(s("q")),
            "vorgabestufe \"q\" is not in klassen",
        ),
        (
            VorgabeFehler::RouteKlasseUnbekannt {
                route: s("r"),
                klasse: s("z"),
            },
            "class \"z\" of route \"r\" is not in klassen",
        ),
        (
            VorgabeFehler::RouteUnbekannt {
                kunde: s("k"),
                route: s("r"),
            },
            "customer \"k\" names the unknown route \"r\"",
        ),
        (
            VorgabeFehler::FreigabeRouteUnbekannt(s("r")),
            "release names the unknown route \"r\"",
        ),
        (
            VorgabeFehler::FreigabeRouteLokal(s("r")),
            "release names the local route \"r\"",
        ),
        (
            VorgabeFehler::FreigabeKundeUnbekannt(s("k")),
            "release names the unknown customer \"k\"",
        ),
    ];
    for (fehler, text) in faelle {
        assert_eq!(fehler.to_string(), text);
    }
}

#[test]
fn eine_klasse_vorgabestufe_ist_die_hoechste_ohne_angabe() {
    // vorgabestufe None means the highest class, not the first.
    let v = Vorgabe::neu(
        vec![s("a"), s("b"), s("c")],
        None,
        vec![route("r", Art::Lokal, Some("b"))],
        vec![],
        vec![],
    )
    .expect("valid settings");
    erwarte(
        &frage(&v, None, Some("r"), None),
        Grund::KlasseUeberRoute,
        Some("c"),
    );
    let v = Vorgabe::neu(
        vec![s("a"), s("b"), s("c")],
        Some(s("a")),
        vec![route("r", Art::Lokal, Some("b"))],
        vec![],
        vec![],
    )
    .expect("valid settings");
    erwarte(&frage(&v, None, Some("r"), None), Grund::Frei, Some("a"));
}
