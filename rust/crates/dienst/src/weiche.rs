//! The switch of the events (Nachtrag 27.11.2026, DI-E5): which kinds of the frame `Ereignis` a connection carries, and the list of
//! them that every viewer gets as its first frame.
//!
//! One function, [`weiche`], answers for every kind on every connection, after the table of DI-E5 point 4. The list of a connection
//! ([`ereignisliste`]) is the catalogue of all modes filtered with that same function, so the list never differs from the switch by
//! itself: a new kind that the switch takes is in the list at once. The kinds of DI3 (`ERLAUBT`) and the fixed reasons of the switch
//! are named only here (the test `nur_hier_wird_gewiesen`). No code of a system of its own.

use serde_json::{Value, json};

/// The kinds of event a viewer may give in every mode (DI3): the lines `Ansicht` of `VT` section 4 without `Verbinden` and
/// `Trennen`, with `OrdnerLesen` and `InhaltSuchen` of the way L9. They go to a core or to the way L9.
pub(crate) const ERLAUBT: [&str; 12] = [
    "Abo",
    "Groesse",
    "Text",
    "Taste",
    "Einfuegen",
    "Antwort",
    "Freigabe",
    "RouteWaehlen",
    "EinstellungSetzen",
    "RaumWechsel",
    "OrdnerLesen",
    "InhaltSuchen",
];

/// The events of the service with `lauf` (DI12 and DI-L3): the service handles them itself.
pub(crate) const IM_LAUF: [&str; 6] = [
    "Starten",
    "Beenden",
    "Umbenennen",
    "Loeschen",
    "Fortsetzen",
    "Wiederbeleben",
];

/// The catalogue of all modes: [`ERLAUBT`], then [`IM_LAUF`] (the order of the contract).
pub(crate) const KATALOG: [&str; 18] = {
    let mut k = [""; 18];
    let mut i = 0;
    while i < ERLAUBT.len() {
        k[i] = ERLAUBT[i];
        i += 1;
    }
    let mut j = 0;
    while j < IM_LAUF.len() {
        k[ERLAUBT.len() + j] = IM_LAUF[j];
        j += 1;
    }
    k
};

/// The error for a kind outside the catalogue of a connection (DI3); the connection ends. The test-mode frames of a service without
/// test mode get the same error.
pub(crate) const NICHT_ERLAUBT: &str = "art_nicht_erlaubt";
/// The fixed refusal of `RaumWechsel` and `EinstellungSetzen` with `lauf`.
const NICHT_IM_LAUF: &str = "nicht_im_lauf";
/// The fixed refusal of every `Freigabe` over `dienst.sock` (DI-E5 point 5).
const FREIGABE_NUR_LOKAL: &str = "freigabe_nur_lokal";
/// The fixed refusal of a `Freigabe` on the local way without `lauf.stelle` (DI-E5 point 5, DI-L6 point 5).
const STELLE_NICHT_ERREICHBAR: &str = "stelle_nicht_erreichbar";

/// The way of a connection: the socket of the bridge (`dienst.sock`) or the local way of DI-L11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Weg {
    Bruecke,
    Lokal,
}

/// What the switch looks at, fixed for a connection: the mode, the way and the key `lauf.stelle` of the start file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Verbindungsart {
    /// The start file has `lauf`.
    pub(crate) lauf: bool,
    pub(crate) weg: Weg,
    /// The start file has `lauf.stelle`.
    pub(crate) stelle: bool,
}

/// The answer of the switch for one kind of event on one connection (DI-E5 point 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Urteil {
    /// The event goes to its receiver and through its checks there.
    Angenommen,
    /// The error of DI3 (checked before `feld_verboten`); the connection ends.
    Fehler(&'static str),
    /// A fixed refusal (checked right after `feld_verboten`): the frame `Ablehnung` with this `grund`, only to the sender.
    Ablehnung(&'static str),
}

/// The table of DI-E5 point 4, one line per arm.
pub(crate) fn weiche(art: &str, v: Verbindungsart) -> Urteil {
    match (art, v.lauf, v.weg, v.stelle) {
        (
            "Abo" | "Groesse" | "Text" | "Taste" | "Einfuegen" | "Antwort" | "RouteWaehlen"
            | "OrdnerLesen" | "InhaltSuchen",
            _,
            _,
            _,
        )
        | ("RaumWechsel" | "EinstellungSetzen", false, _, _)
        | ("Freigabe", true, Weg::Lokal, true)
        | (
            "Starten" | "Beenden" | "Umbenennen" | "Loeschen" | "Fortsetzen" | "Wiederbeleben",
            true,
            _,
            _,
        ) => Urteil::Angenommen,
        ("RaumWechsel" | "EinstellungSetzen", true, _, _) => Urteil::Ablehnung(NICHT_IM_LAUF),
        ("Freigabe", _, Weg::Bruecke, _) => Urteil::Ablehnung(FREIGABE_NUR_LOKAL),
        ("Freigabe", _, Weg::Lokal, _) => Urteil::Ablehnung(STELLE_NICHT_ERREICHBAR),
        _ => Urteil::Fehler(NICHT_ERLAUBT),
    }
}

/// `true` for an event the service handles itself with `lauf` ([`IM_LAUF`]).
pub(crate) fn im_lauf(art: &str) -> bool {
    IM_LAUF.contains(&art)
}

/// The message `Ereignisliste` of a connection (DI-E5 points 1 and 3): the kinds of [`KATALOG`] that [`weiche`] takes, in byte order.
pub(crate) fn ereignisliste(v: Verbindungsart) -> Value {
    let mut liste: Vec<&str> = KATALOG
        .into_iter()
        .filter(|a| weiche(a, v) == Urteil::Angenommen)
        .collect();
    liste.sort_unstable();
    json!({"art": "Ereignisliste", "fassung": 1, "ereignisse": liste})
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Urteil = Urteil::Angenommen;

    fn art(lauf: bool, weg: Weg, stelle: bool) -> Verbindungsart {
        Verbindungsart { lauf, weg, stelle }
    }

    /// All eight kinds of connection (the two without `lauf` but with `stelle` cannot come from a start file; the switch still
    /// answers for them as for the local way without `lauf.stelle`).
    fn alle() -> Vec<Verbindungsart> {
        let mut v = Vec::new();
        for lauf in [false, true] {
            for weg in [Weg::Bruecke, Weg::Lokal] {
                for stelle in [false, true] {
                    v.push(art(lauf, weg, stelle));
                }
            }
        }
        v
    }

    fn liste(v: Verbindungsart) -> Vec<String> {
        let l = ereignisliste(v);
        assert_eq!(l.as_object().map(serde_json::Map::len), Some(3));
        assert_eq!(
            (&l["art"], &l["fassung"]),
            (&json!("Ereignisliste"), &json!(1))
        );
        l["ereignisse"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|e| e.as_str().expect("a text").to_owned())
            .collect()
    }

    #[test]
    fn katalog_ist_erlaubt_dann_die_arten_des_laufs() {
        let mut k = ERLAUBT.to_vec();
        k.extend(IM_LAUF);
        assert_eq!(KATALOG.to_vec(), k);
        for a in KATALOG {
            assert_eq!(KATALOG.iter().filter(|b| **b == a).count(), 1, "{a}");
            assert_eq!(im_lauf(a), IM_LAUF.contains(&a));
        }
    }

    /// DI-E5 point 3: in the list exactly when the switch takes the kind, also for kinds outside the catalogue.
    #[test]
    fn liste_gleicht_der_weiche() {
        let fremde = [
            "Verbinden",
            "Trennen",
            "Takt",
            "Kernfehler",
            "Uhr",
            "Stumm",
            "Zustand",
            "Ausgabe",
            "Socket",
            "abo",
            "",
        ];
        for v in alle() {
            let l = liste(v);
            for a in KATALOG.iter().chain(fremde.iter()) {
                assert_eq!(l.iter().any(|x| x == a), weiche(a, v) == A, "{a} {v:?}");
            }
            for a in fremde {
                assert_eq!(weiche(a, v), Urteil::Fehler(NICHT_ERLAUBT), "{a} {v:?}");
            }
        }
    }

    /// DI-E5 point 3: byte order, each once, at most 64 entries of the form `^[A-Z][A-Za-z]{0,31}$`.
    #[test]
    fn liste_sortiert_einmal_und_in_der_form() {
        for v in alle() {
            let l = liste(v);
            let mut s = l.clone();
            s.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            s.dedup();
            assert_eq!(s, l, "{v:?}");
            assert!(l.len() <= 64);
            for a in &l {
                let b = a.as_bytes();
                assert!(b[0].is_ascii_uppercase() && b.len() <= 32, "{a}");
                assert!(b.iter().all(u8::is_ascii_alphabetic), "{a}");
            }
        }
    }

    /// The table of DI-E5 point 4, line by line, in the five columns of the contract.
    #[test]
    fn tabelle_von_di_e5() {
        let spalten = [
            art(false, Weg::Bruecke, false),
            art(false, Weg::Lokal, false),
            art(true, Weg::Bruecke, true),
            art(true, Weg::Lokal, false),
            art(true, Weg::Lokal, true),
        ];
        let ab = Urteil::Ablehnung;
        let f = Urteil::Fehler(NICHT_ERLAUBT);
        let zeilen: [(&[&str], [Urteil; 5]); 5] = [
            (
                &[
                    "Abo",
                    "Groesse",
                    "Text",
                    "Taste",
                    "Einfuegen",
                    "Antwort",
                    "RouteWaehlen",
                    "OrdnerLesen",
                    "InhaltSuchen",
                ],
                [A; 5],
            ),
            (
                &["RaumWechsel", "EinstellungSetzen"],
                [
                    A,
                    A,
                    ab("nicht_im_lauf"),
                    ab("nicht_im_lauf"),
                    ab("nicht_im_lauf"),
                ],
            ),
            (
                &["Freigabe"],
                [
                    ab("freigabe_nur_lokal"),
                    ab("stelle_nicht_erreichbar"),
                    ab("freigabe_nur_lokal"),
                    ab("stelle_nicht_erreichbar"),
                    A,
                ],
            ),
            (
                &[
                    "Starten",
                    "Beenden",
                    "Umbenennen",
                    "Loeschen",
                    "Fortsetzen",
                    "Wiederbeleben",
                ],
                [f, f, A, A, A],
            ),
            (&["Verbinden", "Takt", "Uhr", "Gateway", "x"], [f; 5]),
        ];
        let mut gesehen = 0;
        for (arten, soll) in zeilen {
            for a in arten {
                for (v, s) in spalten.iter().zip(soll) {
                    assert_eq!(weiche(a, *v), s, "{a} {v:?}");
                }
                gesehen += usize::from(KATALOG.contains(a));
            }
        }
        assert_eq!(
            gesehen,
            KATALOG.len(),
            "every kind of the catalogue has its line"
        );
        // the column "mit lauf, dienst.sock" holds with and without lauf.stelle
        for a in KATALOG {
            assert_eq!(
                weiche(a, art(true, Weg::Bruecke, false)),
                weiche(a, art(true, Weg::Bruecke, true)),
                "{a}"
            );
        }
    }

    /// The lists of the cases `die5_*`, as the contract measured them with the reference.
    #[test]
    fn listen_der_faelle() {
        let ohne_lauf = [
            "Abo",
            "Antwort",
            "Einfuegen",
            "EinstellungSetzen",
            "Groesse",
            "InhaltSuchen",
            "OrdnerLesen",
            "RaumWechsel",
            "RouteWaehlen",
            "Taste",
            "Text",
        ];
        let lauf = [
            "Abo",
            "Antwort",
            "Beenden",
            "Einfuegen",
            "Fortsetzen",
            "Groesse",
            "InhaltSuchen",
            "Loeschen",
            "OrdnerLesen",
            "RouteWaehlen",
            "Starten",
            "Taste",
            "Text",
            "Umbenennen",
            "Wiederbeleben",
        ];
        let mut lauf_stelle = lauf.to_vec();
        lauf_stelle.insert(5, "Freigabe");
        assert_eq!(liste(art(false, Weg::Bruecke, false)), ohne_lauf);
        assert_eq!(liste(art(false, Weg::Lokal, false)), ohne_lauf);
        assert_eq!(liste(art(true, Weg::Bruecke, false)), lauf);
        assert_eq!(liste(art(true, Weg::Bruecke, true)), lauf);
        assert_eq!(liste(art(true, Weg::Lokal, false)), lauf);
        assert_eq!(liste(art(true, Weg::Lokal, true)), lauf_stelle);
    }

    /// The text `name` stands in `text` as a word of its own (not as a part of a longer name).
    fn als_wort(text: &str, name: &str) -> bool {
        let wort = |c: char| c.is_ascii_alphanumeric() || c == '_';
        text.match_indices(name).any(|(i, _)| {
            !text[..i].chars().next_back().is_some_and(wort)
                && !text[i + name.len()..].chars().next().is_some_and(wort)
        })
    }

    /// DI-E5 point 7: no other file of the crate names `ERLAUBT`, a fixed reason of the switch or builds an `Ereignisliste`. The
    /// files are read at test time from the source folder of the crate.
    #[test]
    fn nur_hier_wird_gewiesen() {
        let ordner = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let verboten = [
            "\"art_nicht_erlaubt\"",
            "\"nicht_im_lauf\"",
            "\"freigabe_nur_lokal\"",
            "\"stelle_nicht_erreichbar\"",
            "\"Ereignisliste\"",
        ];
        let mut gesehen = 0;
        let mut stapel = vec![ordner];
        while let Some(o) = stapel.pop() {
            for e in std::fs::read_dir(&o).expect("src") {
                let pfad = e.expect("entry").path();
                if pfad.is_dir() {
                    stapel.push(pfad);
                    continue;
                }
                let name = pfad.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let rust = pfad.extension().and_then(|e| e.to_str()) == Some("rs");
                if !rust || name == "weiche.rs" {
                    continue;
                }
                gesehen += 1;
                let text = std::fs::read_to_string(&pfad).expect("read");
                assert!(
                    !als_wort(&text, "ERLAUBT"),
                    "{} names ERLAUBT",
                    pfad.display()
                );
                for v in verboten {
                    assert!(!text.contains(v), "{} names {v}", pfad.display());
                }
            }
        }
        assert!(gesehen > 5, "only {gesehen} files seen");
    }

    #[test]
    fn als_wort_sieht_nur_ganze_namen() {
        assert!(als_wort("ERLAUBT.contains", "ERLAUBT"));
        assert!(als_wort("x ERLAUBT", "ERLAUBT"));
        assert!(!als_wort("NICHT_ERLAUBT", "ERLAUBT"));
        assert!(!als_wort("ERLAUBTE", "ERLAUBT"));
        assert!(!als_wort("", "ERLAUBT"));
    }
}
