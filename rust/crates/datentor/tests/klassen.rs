//! Unit tests of the class depth (partial contract `klassen/VERTRAG.md`, sections 2 to 5 and 10). All data are invented.

#![allow(
    clippy::many_single_char_names,
    reason = "short names for the test data"
)]

use datentor::klassen::{
    Angabe, Befund, Ebene, Gruppe, Herkunft, Inhalt, Klassenergebnis, Lage, Modell, NIE_ARTEN,
    NUR_MIT_MODELL, NutzerSenkung, Senkung, Senkungsangaben, Tiefe, TiefeFehler, VORGABE_ARTEN,
    ganze_klasse, wirksame_klasse,
};
use datentor::{Art, Einheit, Grund, Kunde, Route, Vorgabe, darf, wirksam};

fn s(x: &str) -> String {
    x.to_owned()
}

fn sv(x: &[&str]) -> Vec<String> {
    x.iter().map(|y| s(y)).collect()
}

/// Four classes s0 < s1 < s2 < s3, one local route per class, customer k-a allowed on r3.
fn v4() -> Vorgabe {
    Vorgabe::neu(
        sv(&["s0", "s1", "s2", "s3"]),
        Some(s("s1")),
        ["r0", "r1", "r2", "r3"]
            .iter()
            .zip(["s0", "s1", "s2", "s3"])
            .map(|(r, k)| Route {
                schluessel: s(r),
                art: Art::Lokal,
                klasse: Some(s(k)),
            })
            .collect(),
        vec![Kunde {
            schluessel: s("k-a"),
            erlaubt: sv(&["r3"]),
            verboten: vec![],
        }],
        vec![],
    )
    .expect("valid")
}

const B0: Befund = Befund {
    geheimnis: false,
    personenbezug: false,
    interner_name: false,
    host: false,
    pfad: false,
    kundenkennung: false,
    konfiguration: false,
    zweifel: false,
};

fn lage(geerbt: Angabe<'static>) -> Lage<'static> {
    Lage {
        geerbt,
        gruppe: None,
        nutzer: NutzerSenkung::Folgt,
        bestaetigt: false,
    }
}

/// A content the lowering may lower: kind regel, no finding, no model.
fn senkbar() -> Inhalt<'static> {
    Inhalt {
        klasse: Angabe::Keine,
        kunde: None,
        art: Some("regel"),
        befund: Some(B0),
        modell: None,
    }
}

fn leer() -> Inhalt<'static> {
    Inhalt {
        klasse: Angabe::Keine,
        kunde: None,
        art: None,
        befund: None,
        modell: None,
    }
}

fn an() -> Tiefe {
    Tiefe::neu(true, true, sv(&VORGABE_ARTEN), vec![]).expect("valid")
}

fn gruppen() -> Tiefe {
    Tiefe::neu(
        true,
        true,
        sv(&["regel", "glossar"]),
        vec![
            Gruppe {
                schluessel: s("g-a"),
                streicht: sv(&["glossar"]),
            },
            Gruppe {
                schluessel: s("g-b"),
                streicht: sv(&["faq", "faq"]),
            },
        ],
    )
    .expect("valid")
}

fn kurz(e: &Klassenergebnis) -> (Option<&str>, Herkunft, Senkung) {
    (e.klasse.as_deref(), e.herkunft, e.senkung)
}

const S2: Angabe<'static> = Angabe::Text("s2");

#[test]
fn rang_und_wirksam() {
    let v = v4();
    assert_eq!(v.rang("s0"), Some(0));
    assert_eq!(v.rang("s3"), Some(3));
    assert_eq!(v.rang("S3"), None);
    assert_eq!(v.rang(""), None);
    assert_eq!(wirksam(&v, &Einheit { klasse: Some("s2") }, None), Ok("s2"));
    assert_eq!(
        wirksam(&v, &Einheit { klasse: Some("s0") }, Some("")),
        Ok("s3")
    );
    assert_eq!(wirksam(&v, &Einheit { klasse: None }, None), Ok("s1"));
    assert_eq!(wirksam(&v, &Einheit { klasse: None }, Some("k")), Ok("s3"));
    assert_eq!(
        wirksam(&v, &Einheit { klasse: Some("x") }, Some("k")),
        Err(Grund::EinheitOhneKlasse)
    );
}

#[test]
fn k1_grundklasse() {
    let v = v4();
    let t = Tiefe::default();
    let mut i = leer();
    assert_eq!(
        kurz(&wirksame_klasse(&v, &t, &lage(Angabe::Keine), &i)),
        (Some("s1"), Herkunft::Vorgabestufe, Senkung::Aus)
    );
    assert_eq!(
        kurz(&wirksame_klasse(&v, &t, &lage(S2), &i)),
        (Some("s2"), Herkunft::Geerbt, Senkung::Aus)
    );
    i.klasse = Angabe::Text("s0");
    assert_eq!(
        kurz(&wirksame_klasse(&v, &t, &lage(S2), &i)),
        (Some("s0"), Herkunft::Eigen, Senkung::Aus)
    );
    assert_eq!(
        kurz(&wirksame_klasse(&v, &t, &lage(Angabe::Keine), &i)),
        (Some("s0"), Herkunft::Eigen, Senkung::Aus)
    );
    let ung = (None, Herkunft::Ungueltig, Senkung::KlasseUngueltig);
    for (eigen, geerbt) in [
        (Angabe::Text("x"), S2),
        (Angabe::KeinText, S2),
        (Angabe::Text("s0"), Angabe::Text("S2")),
        (Angabe::Text("s0"), Angabe::KeinText),
        (Angabe::Keine, Angabe::KeinText),
        (Angabe::Text(""), Angabe::Keine),
    ] {
        i.klasse = eigen;
        i.kunde = Some("k-a");
        let e = wirksame_klasse(&v, &an(), &lage(geerbt), &i);
        assert_eq!(kurz(&e), ung, "{eigen:?} {geerbt:?}");
        assert_eq!(e.kunde.as_deref(), Some("k-a"));
        assert_eq!((e.vorschlag.clone(), e.protokoll.clone()), (None, None));
        assert_eq!(e.einheit(), Einheit { klasse: Some("") });
        assert_eq!(
            darf(&v, &e.einheit(), Some("r3"), e.kunde.as_deref()).grund,
            Grund::EinheitOhneKlasse
        );
    }
}

#[test]
fn k2_kunde_hebt() {
    let v = v4();
    let mut i = leer();
    i.kunde = Some("");
    i.klasse = Angabe::Text("s0");
    let e = wirksame_klasse(&v, &an(), &lage(S2), &i);
    assert_eq!(
        kurz(&e),
        (Some("s3"), Herkunft::Kunde, Senkung::NichtGeerbt)
    );
    assert_eq!(e.kunde.as_deref(), Some(""));
    let mut j = senkbar();
    j.kunde = Some("k-a");
    let e = wirksame_klasse(&v, &an(), &lage(S2), &j);
    assert_eq!(
        kurz(&e),
        (Some("s3"), Herkunft::Kunde, Senkung::Kundendaten)
    );
    assert_eq!(e.einheit(), Einheit { klasse: Some("s3") });
}

#[test]
fn h0_eine_stufe_mit_protokoll() {
    let v = v4();
    let e = wirksame_klasse(&v, &an(), &lage(S2), &senkbar());
    assert_eq!(kurz(&e), (Some("s1"), Herkunft::Gesenkt, Senkung::Gesenkt));
    assert_eq!(e.vorschlag, None);
    assert_eq!(e.kunde, None);
    assert_eq!(
        e.protokoll,
        Some(Senkungsangaben {
            art: s("regel"),
            erkannt_durch: sv(&["typ:regel", "filter:frei"]),
            regel_ebene: Ebene::Firma,
            klasse_alt: s("s2"),
            klasse_neu: s("s1"),
        })
    );
    // Always from the inherited class: s1 sinks to s0, the same call gives the same answer.
    let e1 = wirksame_klasse(&v, &an(), &lage(Angabe::Text("s1")), &senkbar());
    assert_eq!(kurz(&e1), (Some("s0"), Herkunft::Gesenkt, Senkung::Gesenkt));
    assert_eq!(e, wirksame_klasse(&v, &an(), &lage(S2), &senkbar()));
    for (m, mit) in [
        (Some(Modell::Frei), true),
        (Some(Modell::Senken), true),
        (None, false),
    ] {
        let mut i = senkbar();
        i.modell = m;
        i.art = Some("bedienwissen");
        let p = wirksame_klasse(&v, &an(), &lage(S2), &i)
            .protokoll
            .expect("lowered");
        let mut soll = sv(&["typ:bedienwissen", "filter:frei"]);
        if mit {
            soll.push(s("modell:frei"));
        }
        assert_eq!(p.erkannt_durch, soll);
        assert_eq!(p.art, "bedienwissen");
    }
    let mut l = lage(S2);
    l.gruppe = Some("g-a");
    let p = wirksame_klasse(&v, &gruppen(), &l, &senkbar())
        .protokoll
        .expect("lowered");
    assert_eq!(p.regel_ebene, Ebene::Gruppe);
}

#[test]
fn h1_bis_h5() {
    let v = v4();
    let t = an();
    assert_eq!(
        wirksame_klasse(&v, &Tiefe::default(), &lage(S2), &senkbar()).senkung,
        Senkung::Aus
    );
    let aus = Tiefe::neu(true, false, sv(&["regel"]), vec![]).expect("valid");
    assert_eq!(
        kurz(&wirksame_klasse(&v, &aus, &lage(S2), &senkbar())),
        (Some("s2"), Herkunft::Geerbt, Senkung::Aus)
    );
    let mut eigen = senkbar();
    eigen.klasse = Angabe::Text("s2");
    assert_eq!(
        kurz(&wirksame_klasse(&v, &t, &lage(S2), &eigen)),
        (Some("s2"), Herkunft::Eigen, Senkung::NichtGeerbt)
    );
    assert_eq!(
        kurz(&wirksame_klasse(&v, &t, &lage(Angabe::Keine), &senkbar())),
        (Some("s1"), Herkunft::Vorgabestufe, Senkung::NichtGeerbt)
    );
    assert_eq!(
        kurz(&wirksame_klasse(
            &v,
            &t,
            &lage(Angabe::Text("s3")),
            &senkbar()
        )),
        (Some("s3"), Herkunft::Geerbt, Senkung::Kundendaten)
    );
    assert_eq!(
        kurz(&wirksame_klasse(
            &v,
            &t,
            &lage(Angabe::Text("s0")),
            &senkbar()
        )),
        (Some("s0"), Herkunft::Geerbt, Senkung::Niedrigste)
    );
    let mut i = senkbar();
    i.art = None;
    assert_eq!(
        kurz(&wirksame_klasse(&v, &t, &lage(S2), &i)),
        (Some("s2"), Herkunft::Geerbt, Senkung::ArtUnbekannt)
    );
}

#[test]
fn h6_h7_gruppen_und_arten() {
    let v = v4();
    let t = gruppen();
    let mit = |gruppe: Option<&'static str>, art: &'static str| {
        let mut l = lage(S2);
        l.gruppe = gruppe;
        let mut i = senkbar();
        i.art = Some(art);
        wirksame_klasse(&v, &t, &l, &i).senkung
    };
    assert_eq!(mit(Some("g-x"), "regel"), Senkung::GruppeOhneEintrag);
    assert_eq!(mit(Some("G-A"), "regel"), Senkung::GruppeOhneEintrag);
    assert_eq!(mit(None, "glossar"), Senkung::Gesenkt);
    assert_eq!(mit(None, "regel"), Senkung::Gesenkt);
    assert_eq!(mit(None, "bedienwissen"), Senkung::ArtNichtHerabstufbar);
    assert_eq!(mit(None, "Regel"), Senkung::ArtNichtHerabstufbar);
    assert_eq!(mit(Some("g-a"), "glossar"), Senkung::ArtNichtHerabstufbar);
    assert_eq!(mit(Some("g-a"), "regel"), Senkung::Gesenkt);
    assert_eq!(mit(Some("g-b"), "faq"), Senkung::ArtNichtHerabstufbar);
    assert_eq!(mit(Some("g-b"), "glossar"), Senkung::Gesenkt);
    let leer = Tiefe::neu(true, true, vec![], vec![]).expect("valid");
    assert_eq!(
        wirksame_klasse(&v, &leer, &lage(S2), &senkbar()).senkung,
        Senkung::ArtNichtHerabstufbar
    );
}

#[test]
fn h8_bis_h11() {
    let v = v4();
    let t = an();
    let mit = |f: &dyn Fn(&mut Lage<'static>, &mut Inhalt<'static>)| {
        let mut l = lage(S2);
        let mut i = senkbar();
        f(&mut l, &mut i);
        wirksame_klasse(&v, &t, &l, &i)
    };
    assert_eq!(mit(&|_, i| i.befund = None).senkung, Senkung::Befund);
    for b in [
        Befund {
            geheimnis: true,
            ..B0
        },
        Befund {
            personenbezug: true,
            ..B0
        },
        Befund {
            interner_name: true,
            ..B0
        },
        Befund { host: true, ..B0 },
        Befund { pfad: true, ..B0 },
        Befund {
            kundenkennung: true,
            ..B0
        },
        Befund {
            konfiguration: true,
            ..B0
        },
        Befund {
            zweifel: true,
            ..B0
        },
    ] {
        assert!(!b.frei(), "{b:?}");
        assert_eq!(
            mit(&|_, i| i.befund = Some(b)).senkung,
            Senkung::Befund,
            "{b:?}"
        );
    }
    assert!(B0.frei());
    assert_eq!(
        mit(&|_, i| i.modell = Some(Modell::Verhindert)).senkung,
        Senkung::ModellVerhindert
    );
    assert_eq!(
        mit(&|l, _| l.nutzer = NutzerSenkung::Aus).senkung,
        Senkung::NutzerAus
    );
    let e = mit(&|l, _| l.nutzer = NutzerSenkung::Bestaetigung);
    assert_eq!(
        kurz(&e),
        (Some("s2"), Herkunft::Geerbt, Senkung::BestaetigungFehlt)
    );
    assert_eq!(e.vorschlag.as_deref(), Some("s1"));
    assert_eq!(e.protokoll, None);
    let e = mit(&|l, _| {
        l.nutzer = NutzerSenkung::Bestaetigung;
        l.bestaetigt = true;
    });
    assert_eq!(kurz(&e), (Some("s1"), Herkunft::Gesenkt, Senkung::Gesenkt));
    assert_eq!(e.vorschlag, None);
    // bestaetigt without the setting bestaetigung changes nothing.
    assert_eq!(mit(&|l, _| l.bestaetigt = true).senkung, Senkung::Gesenkt);
    let e = mit(&|l, _| {
        l.nutzer = NutzerSenkung::Aus;
        l.bestaetigt = true;
    });
    assert_eq!(e.senkung, Senkung::NutzerAus);
}

/// H9b (addendum 26.11.2026): the two kinds the filter core cannot judge sink only with a model verdict.
#[test]
fn h9b_modell_pflicht() {
    let v = v4();
    let t = an();
    let mit = |f: &dyn Fn(&mut Inhalt<'static>)| {
        let mut i = senkbar();
        f(&mut i);
        wirksame_klasse(&v, &t, &lage(S2), &i)
    };
    for art in NUR_MIT_MODELL {
        assert_eq!(
            mit(&|i| i.art = Some(art)).senkung,
            Senkung::ModellFehlt,
            "{art}"
        );
        for m in [Modell::Frei, Modell::Senken] {
            assert_eq!(
                mit(&|i| {
                    i.art = Some(art);
                    i.modell = Some(m);
                })
                .senkung,
                Senkung::Gesenkt,
                "{art} {m:?}"
            );
        }
        assert_eq!(
            mit(&|i| {
                i.art = Some(art);
                i.modell = Some(Modell::Verhindert);
            })
            .senkung,
            Senkung::ModellVerhindert,
            "{art}"
        );
    }
    for art in ["regel", "bedienwissen"] {
        assert_eq!(
            mit(&|i| i.art = Some(art)).senkung,
            Senkung::Gesenkt,
            "{art}"
        );
    }
}

#[test]
fn k4_ganze_einheit() {
    let v = v4();
    let l = lage(Angabe::Text("s0"));
    let mut a = leer();
    let mut b = leer();
    b.klasse = Angabe::Text("s2");
    let e = ganze_klasse(&v, &l, &[a, b]);
    assert_eq!(
        kurz(&e),
        (Some("s2"), Herkunft::Teile, Senkung::NichtAufgeteilt)
    );
    assert_eq!(
        (e.kunde.clone(), e.vorschlag.clone(), e.protokoll.clone()),
        (None, None, None)
    );
    assert_eq!(kurz(&ganze_klasse(&v, &l, &[b, a])).0, Some("s2"));
    assert_eq!(kurz(&ganze_klasse(&v, &l, &[a])).0, Some("s0"));
    a.kunde = Some("k-a");
    let e = ganze_klasse(&v, &l, &[a, b]);
    assert_eq!(
        (e.klasse.as_deref(), e.kunde.as_deref()),
        (Some("s3"), Some("k-a"))
    );
    b.kunde = Some("k-a");
    assert_eq!(ganze_klasse(&v, &l, &[a, b]).kunde.as_deref(), Some("k-a"));
    b.kunde = Some("k-b");
    let e = ganze_klasse(&v, &l, &[a, b, a]);
    assert_eq!(e.kunde.as_deref(), Some(""));
    assert_eq!(
        darf(&v, &e.einheit(), Some("r3"), e.kunde.as_deref()).grund,
        Grund::KundeOhneEintrag
    );
    let mut c = leer();
    c.klasse = Angabe::KeinText;
    let e = ganze_klasse(&v, &l, &[a, c]);
    assert_eq!(
        kurz(&e),
        (None, Herkunft::Ungueltig, Senkung::KlasseUngueltig)
    );
    assert_eq!(e.kunde.as_deref(), Some("k-a"));
    let e = ganze_klasse(&v, &lage(Angabe::Text("x")), &[leer()]);
    assert_eq!(
        kurz(&e),
        (None, Herkunft::Ungueltig, Senkung::KlasseUngueltig)
    );
    let e = ganze_klasse(&v, &l, &[]);
    assert_eq!(
        kurz(&e),
        (None, Herkunft::Ungueltig, Senkung::KlasseUngueltig)
    );
    assert_eq!(e.kunde, None);
}

#[test]
fn t1_nie_art_ist_ladefehler() {
    for art in NIE_ARTEN {
        assert_eq!(
            Tiefe::neu(true, false, sv(&["regel", art]), vec![]),
            Err(TiefeFehler::NieArt(s(art))),
            "{art}"
        );
        assert_eq!(
            Tiefe::neu(
                true,
                true,
                vec![],
                vec![Gruppe {
                    schluessel: s("g"),
                    streicht: sv(&[art]),
                }]
            ),
            Err(TiefeFehler::NieArt(s(art))),
            "{art}"
        );
    }
    assert_eq!(NIE_ARTEN.len(), 26);
    assert!(NIE_ARTEN.contains(&"personenbezug") && NIE_ARTEN.contains(&"entwurf"));
    // Addendum 26.11.2026, written out by hand.
    for art in [
        "angebot",
        "schaetzung",
        "kalkulation",
        "ticket",
        "testfall",
        "fehlerbericht",
        "sicherheitsbefund",
        "konfiguration",
        "lizenz",
        "kundenbezug",
        "vertragsregel",
    ] {
        assert!(NIE_ARTEN.contains(&art), "{art}");
    }
    assert!(Tiefe::neu(true, true, sv(&["glossar", "faq"]), vec![]).is_ok());
    // Byte for byte: variants of a never kind are other kinds (addendum 2).
    for art in ["vertraege", "vertrag.", "kunden-daten", "plan-a", "xplan"] {
        assert!(Tiefe::neu(true, true, sv(&[art]), vec![]).is_ok(), "{art}");
    }
}

#[test]
fn tiefe_pruefung() {
    use TiefeFehler as F;
    let g = |k: &str, st: &[&str]| Gruppe {
        schluessel: s(k),
        streicht: sv(st),
    };
    assert_eq!(
        Tiefe::neu(true, true, sv(&["Regel"]), vec![]),
        Err(F::SchluesselForm(s("Regel")))
    );
    assert_eq!(
        Tiefe::neu(true, true, sv(&["regel", "regel"]), vec![]),
        Err(F::ArtDoppelt(s("regel")))
    );
    assert_eq!(
        Tiefe::neu(true, true, vec![], vec![g("G", &[])]),
        Err(F::SchluesselForm(s("G")))
    );
    assert_eq!(
        Tiefe::neu(
            true,
            true,
            vec![],
            vec![g("g", &[]), g("h", &[]), g("g", &[])]
        ),
        Err(F::GruppeDoppelt(s("g")))
    );
    assert_eq!(
        Tiefe::neu(true, true, vec![], vec![g("g", &["X"])]),
        Err(F::SchluesselForm(s("X")))
    );
    assert!(Tiefe::neu(false, true, vec![], vec![g("g", &["x", "x"]), g("h", &[])]).is_ok());
    let d = Tiefe::default();
    assert!(d.aufteilen());
    assert!(
        !Tiefe::neu(false, true, vec![], vec![])
            .expect("valid")
            .aufteilen()
    );
    assert_eq!(
        d,
        Tiefe::neu(true, false, sv(&VORGABE_ARTEN), vec![]).expect("valid")
    );
    assert_eq!(
        VORGABE_ARTEN,
        [
            "regel",
            "bedienwissen",
            "oeffentliche-quelle",
            "leere-vorlage"
        ]
    );
    for (f, text) in [
        (
            F::SchluesselForm(s("X")),
            "key \"X\" does not have the key form",
        ),
        (
            F::NieArt(s("plan")),
            "kind \"plan\" is on the never list and can never be lowered",
        ),
        (F::ArtDoppelt(s("a")), "kind \"a\" occurs twice in arten"),
        (F::GruppeDoppelt(s("g")), "group \"g\" occurs twice"),
    ] {
        assert_eq!(f.to_string(), text);
    }
}

#[test]
fn codes() {
    use Herkunft as H;
    use Senkung as S;
    for (h, c) in [
        (H::Eigen, "eigen"),
        (H::Geerbt, "geerbt"),
        (H::Vorgabestufe, "vorgabestufe"),
        (H::Gesenkt, "gesenkt"),
        (H::Kunde, "kunde"),
        (H::Teile, "teile"),
        (H::Ungueltig, "ungueltig"),
    ] {
        assert_eq!(h.code(), c);
    }
    for (x, c) in [
        (S::Gesenkt, "gesenkt"),
        (S::Aus, "aus"),
        (S::NichtGeerbt, "nicht-geerbt"),
        (S::Kundendaten, "kundendaten"),
        (S::Niedrigste, "niedrigste"),
        (S::ArtUnbekannt, "art-unbekannt"),
        (S::GruppeOhneEintrag, "gruppe-ohne-eintrag"),
        (S::ArtNichtHerabstufbar, "art-nicht-herabstufbar"),
        (S::Befund, "befund"),
        (S::ModellVerhindert, "modell-verhindert"),
        (S::ModellFehlt, "modell-fehlt"),
        (S::NutzerAus, "nutzer-aus"),
        (S::BestaetigungFehlt, "bestaetigung-fehlt"),
        (S::KlasseUngueltig, "klasse-ungueltig"),
        (S::NichtAufgeteilt, "nicht-aufgeteilt"),
    ] {
        assert_eq!(x.code(), c);
    }
}
