//! No input makes the library panic, and a rejected input changes nothing (contract rules RA10, RA12, RA14).
//!
//! Random bytes, mutated valid frames, random text and random JSON go in with a fixed seed; the tests check that
//! every call returns, that an error leaves the picture of the receiver and the model picture of the sender as
//! they were, and that an accepted text reads to a canonical line.

use rasterauszug::{Bild, Empfang, Fehler, Verbindung, kanonisch, zeile_lesen, zeile_schreiben};
use serde_json::{Value, json};

/// `SplitMix64`: a small deterministic generator.
struct Zufall(u64);

impl Zufall {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn bis(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).expect("fits")
    }

    fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[0]
    }
}

/// Valid frames of a short session without Deflate, to mutate.
fn gueltige_rahmen() -> Vec<Vec<u8>> {
    let mut sender = Verbindung::neu(false);
    let nachrichten = [
        json!({"art": "Abzug", "sitzung": "s", "stand": 1, "groesse": [12, 6], "invers": false,
               "cursor": [1, 2, true],
               "zeilen": ["\u{1b}[1;31mfett\u{1b}[0m rot", "zwei", "", "\u{1b}[38;2;1;2;3mx", "", "end"]}),
        json!({"art": "Rasteraenderung", "sitzung": "s", "von": 1, "stand": 2, "invers": true,
               "zeilen": {"0": "zwei", "1": "drei", "2": "vier", "3": "fuenf", "4": "sechs", "5": "x"}}),
        json!({"art": "Rasteraenderung", "sitzung": "s", "von": 2, "stand": 3, "invers": true,
               "cursor": [4, 5, false], "zeilen": {"1": "\u{1b}[7mrev"}}),
    ];
    nachrichten
        .iter()
        .filter_map(|m| sender.nimm(0, m).expect("valid message"))
        .collect()
}

fn mutieren(z: &mut Zufall, rahmen: &[u8]) -> Vec<u8> {
    let mut b = rahmen.to_vec();
    for _ in 0..=z.bis(3) {
        match z.bis(5) {
            0 if !b.is_empty() => {
                let i = z.bis(b.len());
                b[i] ^= 1 << z.bis(8);
            }
            1 if !b.is_empty() => {
                let i = z.bis(b.len());
                b[i] = z.byte();
            }
            2 => b.truncate(z.bis(b.len() + 1)),
            3 => b.push(z.byte()),
            _ => {
                let i = z.bis(b.len() + 1);
                b.insert(i, z.byte());
            }
        }
    }
    b
}

fn abbilder(e: &Empfang) -> Vec<Option<Bild>> {
    (0..=255).map(|nr| e.abbild(nr).cloned()).collect()
}

#[test]
fn random_and_mutated_frames_never_panic_and_errors_change_nothing() {
    let mut z = Zufall(0x5eed_0001);
    let gueltig = gueltige_rahmen();
    assert_eq!(gueltig.len(), 3);
    let (mut angenommen, mut abgelehnt) = (0, 0);
    for i in 0..30_000 {
        let mut e = Empfang::neu(false);
        // Bring the receiver into a state, then send the odd frame.
        e.nimm(&gueltig[0]).expect("valid frame");
        let vorher = abbilder(&e);
        let rahmen = if i % 3 == 0 {
            (0..z.bis(60)).map(|_| z.byte()).collect()
        } else {
            let wahl = z.bis(gueltig.len());
            mutieren(&mut z, &gueltig[wahl])
        };
        if let Ok((auszug, abbild)) = e.nimm(&rahmen) {
            angenommen += 1;
            assert_eq!(abbild.stand, auszug.stand);
            assert_eq!(abbild.lines.len(), abbild.zeilen);
            assert!(abbild.lines.iter().all(|l| kanonisch(l)));
        } else {
            abgelehnt += 1;
            assert_eq!(abbilder(&e), vorher, "frame {rahmen:?} changed the state");
        }
    }
    assert!(angenommen > 100, "only {angenommen} frames accepted");
    assert!(abgelehnt > 10_000, "only {abgelehnt} frames rejected");
}

#[test]
fn random_wire_bytes_on_a_deflate_connection_never_panic() {
    let mut z = Zufall(0x5eed_0002);
    let mut fehler: Vec<Fehler> = Vec::new();
    for _ in 0..3000 {
        let mut e = Empfang::neu(true);
        for _ in 0..3 {
            let wire: Vec<u8> = (0..z.bis(80)).map(|_| z.byte()).collect();
            if let Err(f) = e.nimm(&wire) {
                fehler.push(f);
            }
        }
    }
    assert!(fehler.contains(&Fehler::InflateFehler));
    assert!(fehler.contains(&Fehler::RahmenAbgeschnitten));
}

/// A random JSON value from a small vocabulary that hits the fields of the messages.
fn zufalls_json(z: &mut Zufall, tiefe: usize) -> Value {
    const SCHLUESSEL: &[&str] = &[
        "art", "sitzung", "stand", "von", "groesse", "invers", "zeilen", "cursor", "0", "1", "2",
        "01", "-1", "x", "",
    ];
    const TEXTE: &[&str] = &[
        "Abzug",
        "Rasteraenderung",
        "abzug",
        "s",
        "",
        "a",
        "\u{1b}[1mx",
        "\u{1b}",
        "\u{1b}[99m",
        "\u{1b}[38;5;999mq",
        "\u{7f}",
        "\u{85}",
    ];
    match z.bis(if tiefe == 0 { 6 } else { 9 }) {
        0 => Value::Null,
        1 => Value::Bool(z.bis(2) == 0),
        2 => json!(i64::try_from(z.bis(30)).expect("fits") - 3),
        3 => json!([i64::MAX, i64::MIN, 1000, 1001, 0, 3][z.bis(6)]),
        4 => json!(TEXTE[z.bis(TEXTE.len())]),
        5 => json!(0.5),
        6 => Value::Array((0..z.bis(5)).map(|_| zufalls_json(z, tiefe - 1)).collect()),
        _ => Value::Object(
            (0..z.bis(6))
                .map(|_| {
                    (
                        SCHLUESSEL[z.bis(SCHLUESSEL.len())].to_owned(),
                        zufalls_json(z, tiefe - 1),
                    )
                })
                .collect(),
        ),
    }
}

/// A message that has the shape of one of the two messages, with random parts. `vorbild` is the model picture the
/// sender holds, so that a change often fits it.
fn zufalls_nachricht(z: &mut Zufall, vorbild: Option<&Bild>) -> Value {
    const SGR: &[&str] = &[
        "a",
        "",
        "\u{1b}[1mb",
        "c d",
        "\u{1b}[31;42mxy\u{1b}[0m",
        "\u{1b}[1J",
    ];
    let mut m = match vorbild {
        Some(b) if z.bis(4) != 0 => {
            let zeilen: serde_json::Map<String, Value> = (0..z.bis(5))
                .map(|_| {
                    (
                        z.bis(b.zeilen + 1).to_string(),
                        json!(SGR[z.bis(SGR.len())]),
                    )
                })
                .collect();
            let von = if z.bis(8) == 0 { b.stand + 1 } else { b.stand };
            json!({"art": "Rasteraenderung", "sitzung": "s", "von": von, "stand": von + z.bis(3) as u64,
                   "invers": z.bis(2) == 0, "zeilen": zeilen})
        }
        _ => {
            let (sp, ze) = (1 + z.bis(7), 1 + z.bis(7));
            let zeilen: Vec<&str> = (0..ze).map(|_| SGR[z.bis(SGR.len())]).collect();
            json!({"art": "Abzug", "sitzung": "s", "stand": z.bis(6), "groesse": [sp, ze],
                   "invers": z.bis(2) == 0, "zeilen": zeilen})
        }
    };
    if z.bis(2) == 0 {
        m["cursor"] = json!([z.bis(8), z.bis(8), z.bis(2) == 0]);
    }
    if z.bis(3) == 0 {
        let name = [
            "art", "sitzung", "stand", "von", "groesse", "invers", "zeilen", "cursor",
        ][z.bis(8)];
        m[name] = zufalls_json(z, 2);
    }
    m
}

#[test]
fn random_messages_never_panic_and_errors_change_nothing() {
    let mut z = Zufall(0x5eed_0003);
    let (mut angenommen, mut abgelehnt) = (0, 0);
    for _ in 0..4000 {
        let mut v = Verbindung::neu(false);
        for _ in 0..8 {
            let nr = u8::try_from(z.bis(3)).expect("fits");
            let vorher = v.vorbild(nr).cloned();
            let nachricht = if z.bis(5) == 0 {
                zufalls_json(&mut z, 3)
            } else {
                zufalls_nachricht(&mut z, vorher.as_ref())
            };
            if v.nimm(nr, &nachricht).is_ok() {
                angenommen += 1;
            } else {
                abgelehnt += 1;
                assert_eq!(
                    v.vorbild(nr).cloned(),
                    vorher,
                    "{nachricht} changed the state"
                );
            }
        }
    }
    assert!(angenommen > 500, "only {angenommen} messages accepted");
    assert!(abgelehnt > 5000, "only {abgelehnt} messages rejected");
}

#[test]
fn random_text_never_panics_and_reads_to_a_canonical_line() {
    const TEIL: &[&str] = &[
        "\u{1b}",
        "[",
        "m",
        ";",
        "0",
        "1",
        "3",
        "8",
        "5",
        "2",
        "9",
        "7",
        "a",
        " ",
        "\u{e9}",
        "\u{65e5}",
        "\u{7f}",
        "\u{85}",
        "\u{0}",
        "?",
        "J",
        "~",
        "@",
        "\u{1f600}",
        "\u{a0}",
    ];
    let mut z = Zufall(0x5eed_0004);
    let (mut gelesen, mut abgelehnt) = (0, 0);
    for _ in 0..40_000 {
        let text: String = (0..z.bis(14)).map(|_| TEIL[z.bis(TEIL.len())]).collect();
        match zeile_lesen(&text) {
            Ok(zeile) => {
                gelesen += 1;
                assert!(kanonisch(&zeile), "{text:?}");
                assert_eq!(zeile_lesen(&zeile_schreiben(&zeile)), Ok(zeile), "{text:?}");
            }
            Err(_) => abgelehnt += 1,
        }
    }
    assert!(gelesen > 3000, "only {gelesen} texts read");
    assert!(abgelehnt > 3000, "only {abgelehnt} texts rejected");
}
