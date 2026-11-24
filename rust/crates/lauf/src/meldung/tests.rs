use super::{GRUND_MAX, grund_gut, zeile, zeile_gut};
use crate::fehler::Fehler;

/// The keys a `code()` gives, read from the source: every quoted text after `=> "` that starts with `prefix`.
fn schluessel(quelle: &str, prefix: &str) -> Vec<String> {
    let mut alle = Vec::new();
    for teil in quelle.split("=> \"").skip(1) {
        let Some((wort, _)) = teil.split_once('"') else {
            continue;
        };
        if wort.starts_with(prefix) {
            alle.push(wort.to_owned());
        }
    }
    alle
}

/// Every key of `lauf/src/fehler.rs` and of the codes of contracts 1 and 2 it passes on (`start.*`, `einst.*`) has the
/// form of the report (agreed with the service), and so has every line the program can write.
#[test]
fn every_key_of_the_faults_has_the_form_of_the_report() {
    let quellen = [
        (include_str!("../fehler.rs"), "lauf."),
        (include_str!("../../../start/src/fehler.rs"), "start."),
        (
            include_str!("../../../einstellungen/src/fehler.rs"),
            "einst.",
        ),
    ];
    let mut zahl = 0;
    for (quelle, prefix) in quellen {
        let alle = schluessel(quelle, prefix);
        // positive control: the reading finds the keys at all
        assert!(alle.len() > 10, "{prefix}: {alle:?}");
        for s in &alle {
            assert!(grund_gut(s), "{s}");
            assert!(zeile_gut(format!("255 {s}\n").as_bytes()), "{s}");
        }
        zahl += alle.len();
    }
    assert!(zahl >= 75, "{zahl}");
    // the lines of the variants the program builds itself
    for f in [
        Fehler::MeldungFehlt,
        Fehler::HarnessFehlt,
        Fehler::Intern,
        Fehler::PaketCache("verweis"),
        Fehler::PaketEntpacken {
            rc: Some(1),
            wort: "kein-platz",
        },
        Fehler::Start(start::Fehler::ZuGross),
        Fehler::Einst(einstellungen::Fehler::ZuGross),
    ] {
        assert!(zeile_gut(zeile(&f).as_bytes()), "{}", zeile(&f));
    }
    assert_eq!(zeile(&Fehler::HarnessFehlt), "69 lauf.harness_fehlt\n");
    assert_eq!(zeile(&Fehler::MeldungFehlt), "69 lauf.meldung_fehlt\n");
}

#[test]
fn the_form_of_the_line_is_closed() {
    let lang = "a".repeat(GRUND_MAX);
    for gut in [
        "69 lauf.harness_fehlt\n".to_owned(),
        "1 a\n".to_owned(),
        "255 x_9.y\n".to_owned(),
        format!("70 {lang}\n"),
    ] {
        assert!(zeile_gut(gut.as_bytes()), "{gut}");
    }
    let zu_lang = format!("70 {lang}a\n");
    for schlecht in [
        "",
        "\n",
        "69 lauf.harness_fehlt",
        "69 lauf.harness_fehlt\n\n",
        "69 lauf.harness_fehlt\nx",
        "69  lauf.harness_fehlt\n",
        "69 lauf.harness_fehlt \n",
        "69 lauf.harness_fehlt: Harness nicht startbar\n",
        "69 Lauf.harness_fehlt\n",
        "69 lauf-harness\n",
        "0 lauf.intern\n",
        "256 lauf.intern\n",
        "069 lauf.intern\n",
        "+69 lauf.intern\n",
        "-1 lauf.intern\n",
        "69\n",
        "69 \n",
        " lauf.intern\n",
        "69 lauf.intern\r\n",
        zu_lang.as_str(),
    ] {
        assert!(!zeile_gut(schlecht.as_bytes()), "{schlecht:?}");
    }
    let mut gross = format!("70 {lang}\n").into_bytes();
    gross.resize(129, b'a');
    assert!(!zeile_gut(&gross));
    assert!(!zeile_gut(b"69 lauf.\xff\n"));
}
