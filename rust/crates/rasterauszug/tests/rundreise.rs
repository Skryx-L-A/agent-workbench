//! The two round-trip tests of the contract (section 11), each on at least 10 000 states with a fixed seed.
//!
//! 1. Line form: for every canonical line `z`, `lesen(schreiben(z)) == z`; for every valid text `s`,
//!    `lesen(schreiben(lesen(s))) == lesen(s)`.
//! 2. Encoder and decoder: every message of the core that the sender takes gives a frame that the receiver reads,
//!    and the receiver's picture equals the model picture of the sender; the frame equals the encoding of the
//!    extract the receiver read.

use rasterauszug::{
    Empfang, Lauf, Stil, Verbindung, Zeile, kanonisch, rahmen_kodieren, zeile_lesen,
    zeile_schreiben,
};
use serde_json::{Value, json};

/// `SplitMix64`: a small deterministic generator, so the tests need no crate and no clock.
struct Zufall(u64);

impl Zufall {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`.
    fn bis(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).expect("fits")
    }

    /// A number in `lo..=hi`.
    fn zw(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.bis(hi - lo + 1)
    }

    fn wahr(&mut self, eins_von: usize) -> bool {
        self.bis(eins_von) == 0
    }
}

/// Characters for texts: ASCII, spaces, wide, combining, no-break space and other allowed odd ones.
const ZEICHEN: &[char] = &[
    'a',
    'b',
    'Z',
    '0',
    '9',
    ' ',
    ' ',
    ' ',
    '.',
    '~',
    '[',
    ';',
    'm',
    '\u{e9}',
    '\u{65e5}',
    '\u{1f600}',
    '\u{a0}',
    '\u{2028}',
    '\u{200b}',
    '\u{feff}',
    '\u{301}',
    '\u{ff}',
    '\u{1fffd}',
];

fn zufallstext(z: &mut Zufall, max: usize) -> String {
    (0..z.zw(1, max))
        .map(|_| ZEICHEN[z.bis(ZEICHEN.len())])
        .collect()
}

fn zufallsfarbe(z: &mut Zufall) -> u32 {
    match z.bis(4) {
        0 | 1 => 0,
        2 => u32::try_from(z.zw(1, 256)).expect("fits"),
        _ => 0x0100_0000 | u32::try_from(z.bis(0x0100_0000)).expect("fits"),
    }
}

fn zufallsstil(z: &mut Zufall) -> Stil {
    Stil {
        fg: zufallsfarbe(z),
        bg: zufallsfarbe(z),
        fl: if z.wahr(2) {
            0
        } else {
            u8::try_from(z.bis(128)).expect("fits")
        },
    }
}

/// A random canonical line of up to `max_laeufe` runs.
fn kanonische_zeile(z: &mut Zufall, max_laeufe: usize) -> Zeile {
    let mut zeile: Zeile = Vec::new();
    for _ in 0..z.bis(max_laeufe + 1) {
        let stil = zufallsstil(z);
        if zeile.last().is_some_and(|l| l.stil == stil) {
            continue;
        }
        let mut text = zufallstext(z, 6);
        if stil == Stil::VORGABE {
            text.truncate(text.trim_end_matches(' ').len());
        }
        if !text.is_empty() {
            zeile.push(Lauf { stil, text });
        }
    }
    if zeile
        .last()
        .is_some_and(|l| l.stil == Stil::VORGABE && l.text.ends_with(' '))
    {
        zeile.pop();
    }
    zeile
}

/// A random SGR parameter list for one sequence.
fn zufalls_sgr(z: &mut Zufall) -> String {
    const EINFACH: &[u16] = &[
        0, 1, 2, 3, 4, 7, 8, 9, 22, 23, 24, 27, 28, 29, 39, 49, 30, 31, 37, 40, 47, 90, 97, 100,
        107,
    ];
    let mut ps: Vec<String> = Vec::new();
    for _ in 0..z.bis(5) {
        ps.push(match z.bis(8) {
            0 => String::new(),
            1 => format!("{:03}", EINFACH[z.bis(EINFACH.len())]),
            2 => format!("38;5;{}", z.bis(256)),
            3 => format!("48;5;{}", z.bis(256)),
            4 => format!("38;2;{};{};{}", z.bis(256), z.bis(256), z.bis(256)),
            5 => format!("48;2;{};{};{}", z.bis(256), z.bis(256), z.bis(256)),
            _ => EINFACH[z.bis(EINFACH.len())].to_string(),
        });
    }
    format!("\u{1b}[{}m", ps.join(";"))
}

/// A random valid line text: sequences and text mixed.
fn zufalls_sgr_text(z: &mut Zufall) -> String {
    let mut s = String::new();
    for _ in 0..z.bis(10) {
        if z.wahr(2) {
            s.push_str(&zufalls_sgr(z));
        } else {
            s.push_str(&zufallstext(z, 5));
        }
    }
    s
}

#[test]
fn line_form_reads_what_it_wrote() {
    let mut z = Zufall(20_261_119);
    let mut gelesen_nicht_leer = 0;
    for i in 0..10_000 {
        let zeile = kanonische_zeile(&mut z, 7);
        assert!(
            kanonisch(&zeile),
            "state {i}: generator made a non-canonical line"
        );
        let text = zeile_schreiben(&zeile);
        assert_eq!(
            zeile_lesen(&text).as_ref(),
            Ok(&zeile),
            "state {i}: {text:?}"
        );
    }
    for i in 0..10_000 {
        let s = zufalls_sgr_text(&mut z);
        let erste =
            zeile_lesen(&s).unwrap_or_else(|e| panic!("state {i}: {s:?} does not read: {e}"));
        assert!(
            kanonisch(&erste),
            "state {i}: {s:?} reads to a non-canonical line"
        );
        let kanon = zeile_schreiben(&erste);
        assert_eq!(
            zeile_lesen(&kanon).as_ref(),
            Ok(&erste),
            "state {i}: {s:?} via {kanon:?}"
        );
        assert_eq!(
            zeile_schreiben(&zeile_lesen(&kanon).expect("reads")),
            kanon,
            "state {i}"
        );
        if !erste.is_empty() {
            gelesen_nicht_leer += 1;
        }
    }
    assert!(
        gelesen_nicht_leer > 5000,
        "the generator makes too many empty lines"
    );
}

/// A random picture of the given size as lines of SGR text.
fn bildtexte(z: &mut Zufall, zeilen: usize) -> Vec<String> {
    (0..zeilen)
        .map(|_| {
            if z.wahr(3) {
                String::new()
            } else if z.wahr(2) {
                zeile_schreiben(&kanonische_zeile(z, 4))
            } else {
                zufalls_sgr_text(z)
            }
        })
        .collect()
}

fn cursor_feld(z: &mut Zufall, spalten: usize, zeilen: usize) -> Option<Value> {
    z.wahr(2)
        .then(|| json!([z.bis(zeilen), z.bis(spalten), z.wahr(2)]))
}

/// What the test knows about one session number: size, the texts of the lines and the stand.
#[derive(Clone)]
struct Sitzung {
    spalten: usize,
    zeilen: usize,
    texte: Vec<String>,
    stand: u64,
}

/// A message `Abzug` with a random size and picture.
fn abzug_nachricht(z: &mut Zufall) -> (Value, Sitzung) {
    let spalten = z.zw(2, 40);
    let zeilen = z.zw(1, 40);
    let texte = bildtexte(z, zeilen);
    let stand = z.bis(1000) as u64;
    let mut m = json!({
        "art": "Abzug", "sitzung": "s", "stand": stand, "groesse": [spalten, zeilen],
        "invers": z.wahr(2), "zeilen": texte,
    });
    if let Some(c) = cursor_feld(z, spalten, zeilen) {
        m["cursor"] = c;
    }
    (
        m,
        Sitzung {
            spalten,
            zeilen,
            texte,
            stand,
        },
    )
}

/// A message `Rasteraenderung` for a session: a scroll, a few changed lines or nothing, random cursor and `invers`.
fn aenderung_nachricht(z: &mut Zufall, alt: &Sitzung) -> (Value, Sitzung) {
    let mut neu = alt.texte.clone();
    match z.bis(3) {
        0 if alt.zeilen > 1 => {
            let sh = z.zw(1, alt.zeilen - 1);
            neu.rotate_left(sh);
            for t in neu.iter_mut().skip(alt.zeilen - sh) {
                *t = zufalls_sgr_text(z);
            }
        }
        1 => {
            for _ in 0..z.zw(0, 4) {
                let i = z.bis(alt.zeilen);
                neu[i] = zufalls_sgr_text(z);
            }
        }
        _ => {}
    }
    let mut zeilenobjekt = serde_json::Map::new();
    for (i, t) in neu.iter().enumerate() {
        if *t != alt.texte[i] || z.wahr(6) {
            zeilenobjekt.insert(i.to_string(), Value::String(t.clone()));
        }
    }
    let stand = alt.stand + 1 + z.bis(5) as u64;
    let mut m = json!({
        "art": "Rasteraenderung", "sitzung": "s", "von": alt.stand, "stand": stand,
        "invers": z.wahr(2), "zeilen": Value::Object(zeilenobjekt),
    });
    if let Some(c) = cursor_feld(z, alt.spalten, alt.zeilen) {
        m["cursor"] = c;
    }
    (
        m,
        Sitzung {
            texte: neu,
            stand,
            ..alt.clone()
        },
    )
}

/// How often the generator reached each branch.
#[derive(Default)]
struct Zaehler {
    zustaende: usize,
    mit_schub: usize,
    ohne_rahmen: usize,
    mit_cursor: usize,
    mit_deflate: usize,
}

/// Sends one message through sender and receiver and checks that both agree.
fn nachricht_pruefen(
    sender: &mut Verbindung,
    empfang: &mut Empfang,
    deflate: bool,
    nr: u8,
    nachricht: &Value,
    zaehler: &mut Zaehler,
) {
    let roh = sender
        .nimm_roh(nr, nachricht)
        .unwrap_or_else(|e| panic!("sender rejects {nachricht}: {e}"));
    zaehler.zustaende += 1;
    if nachricht.get("cursor").is_some() {
        zaehler.mit_cursor += 1;
    }
    let Some(roh) = roh else {
        zaehler.ohne_rahmen += 1;
        return;
    };
    let wire = sender.verpacken(&roh);
    if deflate {
        zaehler.mit_deflate += 1;
    }
    let (roh2, auszug, abbild) = empfang
        .nimm_roh(&wire)
        .unwrap_or_else(|e| panic!("receiver rejects the frame of {nachricht}: {e}"));
    assert_eq!(roh2, roh, "inflated frame differs for {nachricht}");
    assert_eq!(
        rahmen_kodieren(&auszug),
        roh,
        "re-encoding differs for {nachricht}"
    );
    let vorbild = sender.vorbild(nr).expect("the sender holds a picture");
    assert!(vorbild.gleicht(abbild), "picture differs for {nachricht}");
    assert_eq!(vorbild.stand, abbild.stand, "stand differs for {nachricht}");
    if auszug.schub.is_some() {
        zaehler.mit_schub += 1;
    }
}

#[test]
fn encoder_and_decoder_agree() {
    let mut z = Zufall(19_112_026);
    let mut zaehler = Zaehler::default();
    for _ in 0..2500 {
        let deflate = z.wahr(2);
        let mut sender = Verbindung::neu(deflate);
        let mut empfang = Empfang::neu(deflate);
        let mut sitzungen: Vec<Option<Sitzung>> = vec![None; 3];
        for _ in 0..8 {
            let nr = z.bis(3);
            let (nachricht, neu) = match sitzungen[nr].clone() {
                Some(alt) if !z.wahr(8) => aenderung_nachricht(&mut z, &alt),
                _ => abzug_nachricht(&mut z),
            };
            sitzungen[nr] = Some(neu);
            let nr8 = u8::try_from(nr).expect("fits");
            nachricht_pruefen(
                &mut sender,
                &mut empfang,
                deflate,
                nr8,
                &nachricht,
                &mut zaehler,
            );
        }
    }
    assert!(
        zaehler.zustaende >= 10_000,
        "only {} states",
        zaehler.zustaende
    );
    assert!(
        zaehler.mit_schub > 100,
        "only {} scrolls",
        zaehler.mit_schub
    );
    assert!(
        zaehler.ohne_rahmen > 100,
        "only {} messages without a frame",
        zaehler.ohne_rahmen
    );
    assert!(
        zaehler.mit_cursor > 1000,
        "only {} cursors",
        zaehler.mit_cursor
    );
    assert!(
        zaehler.mit_deflate > 1000,
        "only {} Deflate frames",
        zaehler.mit_deflate
    );
}
