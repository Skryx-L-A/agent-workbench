//! The frame: exact bytes of the encoder and every check of the decoder (RA9, RA10).

use std::collections::HashMap;

use super::zeile;
use crate::{Abbild, Auszug, Bild, Cursor, Fehler, Schub, rahmen_dekodieren, rahmen_kodieren};

/// A varint, written independently of the library.
fn vu(mut n: u64) -> Vec<u8> {
    let mut aus = Vec::new();
    loop {
        let gruppe = u8::try_from(n & 0x7f).expect("fits");
        n >>= 7;
        if n == 0 {
            aus.push(gruppe);
            return aus;
        }
        aus.push(gruppe | 0x80);
    }
}

/// A run of a hand-made frame: style number and text bytes.
type Lauf = (u64, Vec<u8>);

/// A frame put together by hand.
#[derive(Clone)]
struct Rahmen {
    art: u8,
    nr: u8,
    flags: u8,
    stand: Vec<u8>,
    groesse: Vec<u8>,
    schub: Vec<u8>,
    cursor: Vec<u8>,
    stile: Vec<(u64, u64, u64)>,
    stile_zahl: Option<u64>,
    /// Lines: index and runs (style number, text bytes).
    zeilen: Vec<(u64, Vec<Lauf>)>,
    zeilen_zahl: Option<u64>,
    rest: Vec<u8>,
}

impl Rahmen {
    /// A frame that is a change at stand 8 without lines.
    fn aenderung() -> Rahmen {
        Rahmen {
            art: 5,
            nr: 1,
            flags: 0,
            stand: vu(8),
            groesse: Vec::new(),
            schub: Vec::new(),
            cursor: Vec::new(),
            stile: Vec::new(),
            stile_zahl: None,
            zeilen: Vec::new(),
            zeilen_zahl: None,
            rest: Vec::new(),
        }
    }

    /// A full picture of the given size at stand 3.
    fn voll(spalten: u64, zeilen: u64) -> Rahmen {
        let mut r = Rahmen::aenderung();
        r.flags = 4;
        r.stand = vu(3);
        r.groesse = [vu(spalten), vu(zeilen)].concat();
        r
    }

    fn mit_schub(mut self, o: u64, u: u64, n: u64) -> Rahmen {
        self.flags |= 1;
        self.schub = [vu(o), vu(u), vu(n)].concat();
        self
    }

    fn mit_cursor(mut self, z: u64, s: u64, v: u64) -> Rahmen {
        self.flags |= 2;
        self.cursor = [vu(z), vu(s), vu(v)].concat();
        self
    }

    fn mit_zeile(mut self, index: u64, laeufe: &[(u64, &str)]) -> Rahmen {
        self.zeilen.push((
            index,
            laeufe
                .iter()
                .map(|(s, t)| (*s, t.as_bytes().to_vec()))
                .collect(),
        ));
        self
    }

    fn bytes(&self) -> Vec<u8> {
        let mut b = vec![self.art, self.nr, self.flags];
        b.extend(&self.stand);
        b.extend(&self.groesse);
        b.extend(&self.schub);
        b.extend(&self.cursor);
        b.extend(vu(self.stile_zahl.unwrap_or(self.stile.len() as u64)));
        for (fg, bg, fl) in &self.stile {
            b.extend([vu(*fg), vu(*bg), vu(*fl)].concat());
        }
        b.extend(vu(self.zeilen_zahl.unwrap_or(self.zeilen.len() as u64)));
        for (index, laeufe) in &self.zeilen {
            b.extend(vu(*index));
            b.extend(vu(laeufe.len() as u64));
            for (stilnr, text) in laeufe {
                b.extend(vu(*stilnr));
                b.extend(vu(text.len() as u64));
                b.extend(text);
            }
        }
        b.extend(&self.rest);
        b
    }
}

/// The receiver state: session 1 holds a picture of 10 columns and 5 rows at stand 7, with lines `a` to `e`,
/// the cursor at (2, 3) shown and `invers` set.
fn zustand() -> HashMap<u8, Abbild> {
    let mut b = Bild::leer(10, 5);
    b.stand = 7;
    b.invers = true;
    b.cursor = Cursor {
        zeile: 2,
        spalte: 3,
        sichtbar: true,
    };
    for (i, t) in ["a", "b", "c", "d", "e"].iter().enumerate() {
        b.lines[i] = zeile(&[(0, 0, 0, t)]);
    }
    HashMap::from([(1, b)])
}

fn dekodieren(r: &Rahmen) -> Result<(Auszug, Abbild), Fehler> {
    rahmen_dekodieren(&r.bytes(), &zustand())
}

fn fehler(r: &Rahmen) -> Fehler {
    dekodieren(r).expect_err("the frame is invalid")
}

/// A change frame with one line and one style, which every test may break in one place.
fn gut() -> Rahmen {
    let mut r = Rahmen::aenderung().mit_zeile(1, &[(0, "xy")]);
    r.stile = vec![(1, 0, 0)];
    r
}

#[test]
fn the_encoder_writes_the_bytes_of_the_contract() {
    let auszug = Auszug {
        nr: 3,
        stand: 300,
        groesse: Some((80, 24)),
        invers: true,
        schub: Some(Schub {
            oben: 2,
            unten: 10,
            n: 3,
        }),
        cursor: Some(Cursor {
            zeile: 5,
            spalte: 7,
            sichtbar: true,
        }),
        zeilen: vec![(0, zeile(&[(1, 0, 0, "ab")]))],
    };
    assert_eq!(
        rahmen_kodieren(&auszug),
        [
            5, 3, 15, 0xac, 0x02, 80, 24, 2, 10, 3, 5, 7, 1, 1, 1, 0, 0, 1, 0, 1, 0, 2, 0x61, 0x62
        ]
    );
    let leer = Auszug {
        nr: 0,
        stand: 0,
        groesse: None,
        invers: false,
        schub: None,
        cursor: Some(Cursor {
            zeile: 0,
            spalte: 0,
            sichtbar: false,
        }),
        zeilen: Vec::new(),
    };
    assert_eq!(rahmen_kodieren(&leer), [5, 0, 2, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn the_style_table_follows_the_first_appearance() {
    let auszug = Auszug {
        nr: 1,
        stand: 1,
        groesse: None,
        invers: false,
        schub: None,
        cursor: None,
        zeilen: vec![
            (0, zeile(&[(9, 0, 0, "a"), (2, 0, 0, "b")])),
            (1, zeile(&[(2, 0, 0, "c"), (9, 0, 0, "d")])),
        ],
    };
    let b = rahmen_kodieren(&auszug);
    assert_eq!(
        b,
        [
            5, 1, 0, 1, // header
            2, 9, 0, 0, 2, 0, 0, // two styles: 9 then 2
            2, // two lines
            0, 2, 0, 1, 0x61, 1, 1, 0x62, // line 0: style 0 "a", style 1 "b"
            1, 2, 1, 1, 0x63, 0, 1, 0x64, // line 1: style 1 "c", style 0 "d"
        ]
    );
}

#[test]
fn varints_are_written_in_their_shortest_form() {
    for (n, soll) in [
        (0_u64, vec![0_u8]),
        (1, vec![1]),
        (127, vec![0x7f]),
        (128, vec![0x80, 0x01]),
        (300, vec![0xac, 0x02]),
        (16383, vec![0xff, 0x7f]),
        (16384, vec![0x80, 0x80, 0x01]),
        (
            u64::MAX,
            vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01],
        ),
    ] {
        let auszug = Auszug {
            nr: 0,
            stand: n,
            groesse: None,
            invers: false,
            schub: None,
            cursor: None,
            zeilen: Vec::new(),
        };
        let b = rahmen_kodieren(&auszug);
        assert_eq!(&b[3..3 + soll.len()], soll.as_slice(), "{n}");
        assert_eq!(b.len(), 3 + soll.len() + 2, "{n}");
        let mut zustand = zustand();
        zustand.clear();
        // The decoder reads it back as the stand of a full picture.
        let voll = Rahmen {
            stand: vu(n),
            ..Rahmen::voll(2, 1)
        };
        let (a, _) = rahmen_dekodieren(&voll.bytes(), &zustand).expect("valid");
        assert_eq!(a.stand, n);
    }
}

#[test]
fn a_valid_frame_is_read_into_extract_and_picture() {
    let mut r = Rahmen::aenderung()
        .mit_schub(1, 4, 1)
        .mit_cursor(4, 9, 0)
        .mit_zeile(0, &[(1, "neu"), (0, "x")])
        .mit_zeile(4, &[]);
    r.flags |= 8;
    r.stile = vec![(1, 0, 0), (0x0100_0001, 256, 127)];
    let (a, neu) = dekodieren(&r).expect("valid");
    assert_eq!(a.nr, 1);
    assert_eq!(a.stand, 8);
    assert_eq!(a.groesse, None);
    assert!(a.invers);
    assert_eq!(
        a.schub,
        Some(Schub {
            oben: 1,
            unten: 4,
            n: 1
        })
    );
    assert_eq!(
        a.cursor,
        Some(Cursor {
            zeile: 4,
            spalte: 9,
            sichtbar: false
        })
    );
    assert_eq!(
        a.zeilen,
        vec![
            (0, zeile(&[(0x0100_0001, 256, 127, "neu"), (1, 0, 0, "x")])),
            (4, Vec::new())
        ]
    );
    // The scroll moves b, c, d up by one: [a, c, d, "", e]; then the lines 0 and 4 are set.
    assert_eq!(
        neu.lines,
        vec![
            zeile(&[(0x0100_0001, 256, 127, "neu"), (1, 0, 0, "x")]),
            zeile(&[(0, 0, 0, "c")]),
            zeile(&[(0, 0, 0, "d")]),
            Vec::new(),
            Vec::new(),
        ]
    );
    assert_eq!(neu.stand, 8);
    assert!(neu.invers);
    assert_eq!(
        neu.cursor,
        Cursor {
            zeile: 4,
            spalte: 9,
            sichtbar: false
        }
    );
    assert_eq!((neu.spalten, neu.zeilen), (10, 5));
}

#[test]
fn a_change_keeps_what_it_does_not_name() {
    let (a, neu) = dekodieren(&Rahmen::aenderung()).expect("valid");
    assert!(!a.invers);
    assert_eq!(a.cursor, None);
    assert_eq!(a.schub, None);
    assert_eq!(a.zeilen, Vec::new());
    let z = zustand();
    let alt = &z[&1];
    assert_eq!(neu.lines, alt.lines);
    assert_eq!(neu.cursor, alt.cursor);
    assert!(!neu.invers, "invers comes from every frame");
    assert_eq!(neu.stand, 8);
}

#[test]
fn a_full_picture_starts_from_an_empty_one() {
    let r = Rahmen::voll(4, 2).mit_zeile(1, &[(0, "z")]);
    let mut r = r;
    r.stile = vec![(0, 0, 0)];
    // Session 1 holds a picture of the same size as well; nothing of it survives.
    let mut z = zustand();
    z.insert(1, {
        let mut b = Bild::leer(4, 2);
        b.lines[0] = zeile(&[(0, 0, 0, "alt")]);
        b.cursor = Cursor {
            zeile: 1,
            spalte: 1,
            sichtbar: true,
        };
        b.stand = 99;
        b
    });
    let (a, neu) = rahmen_dekodieren(&r.bytes(), &z).expect("a full picture takes any stand");
    assert_eq!(a.groesse, Some((4, 2)));
    assert_eq!(neu.lines, vec![Vec::new(), zeile(&[(0, 0, 0, "z")])]);
    assert_eq!(neu.cursor, Cursor::default());
    assert_eq!((neu.spalten, neu.zeilen, neu.stand), (4, 2, 3));
    assert!(!neu.invers);
    // A full picture of session 9 needs no earlier picture.
    let mut r9 = Rahmen::voll(2, 2);
    r9.nr = 9;
    assert!(rahmen_dekodieren(&r9.bytes(), &HashMap::new()).is_ok());
}

#[test]
fn the_art_and_the_flags_are_checked() {
    assert_eq!(
        rahmen_dekodieren(&[], &zustand()),
        Err(Fehler::RahmenAbgeschnitten)
    );
    assert_eq!(
        rahmen_dekodieren(&[4], &zustand()),
        Err(Fehler::ArtUngueltig)
    );
    assert_eq!(
        rahmen_dekodieren(&[6, 1, 0, 8], &zustand()),
        Err(Fehler::ArtUngueltig)
    );
    assert_eq!(
        rahmen_dekodieren(&[5], &zustand()),
        Err(Fehler::RahmenAbgeschnitten)
    );
    assert_eq!(
        rahmen_dekodieren(&[5, 1], &zustand()),
        Err(Fehler::RahmenAbgeschnitten)
    );
    assert_eq!(
        rahmen_dekodieren(&[5, 1, 0], &zustand()),
        Err(Fehler::RahmenAbgeschnitten)
    );
    for bit in [16_u8, 32, 64, 128, 0xf0, 0xff] {
        let mut r = Rahmen::aenderung();
        r.flags = bit;
        assert_eq!(fehler(&r), Fehler::FlagsUnbekannt, "flags {bit}");
    }
}

#[test]
fn the_stand_is_a_shortest_varint_within_64_bits() {
    let mut r = Rahmen::voll(2, 1);
    r.stand = vec![0x80, 0x00];
    assert_eq!(fehler(&r), Fehler::VuUngueltig);
    r.stand = vec![0xff, 0x80, 0x00];
    assert_eq!(fehler(&r), Fehler::VuUngueltig);
    assert_eq!(
        rahmen_dekodieren(&[5, 1, 4, 0x80], &zustand()),
        Err(Fehler::RahmenAbgeschnitten)
    );
    r.stand = vec![0xff; 9];
    r.stand.push(0x02);
    assert_eq!(fehler(&r), Fehler::VuUngueltig);
    r.stand = vec![0xff; 10];
    assert_eq!(fehler(&r), Fehler::VuUngueltig);
    r.stand = vec![0xff; 11];
    assert_eq!(fehler(&r), Fehler::VuUngueltig);
    r.stand = [vec![0xff; 9], vec![0x01]].concat();
    assert_eq!(dekodieren(&r).expect("2^64 - 1").0.stand, u64::MAX);
    r.stand = vec![0x81, 0x00];
    assert_eq!(fehler(&r), Fehler::VuUngueltig);
    r.stand = vec![0xff, 0x7f];
    assert_eq!(dekodieren(&r).expect("16383").0.stand, 16383);
}

#[test]
fn the_size_of_a_full_picture_is_2_to_1000_columns_and_1_to_1000_rows() {
    for (sp, ze, ok) in [
        (2, 1, true),
        (1000, 1000, true),
        (1000, 1, true),
        (2, 1000, true),
        // one column is refused: the carrier cannot hold one (`alacritty_terminal` issue 9112)
        (1, 1, false),
        (1, 5, false),
        (1, 1000, false),
        (0, 5, false),
        (5, 0, false),
        (1001, 5, false),
        (5, 1001, false),
        (u64::MAX, 5, false),
        (5, u64::MAX, false),
    ] {
        let r = Rahmen::voll(sp, ze);
        match dekodieren(&r) {
            Ok((a, neu)) => {
                assert!(ok, "{sp} x {ze}");
                assert_eq!(
                    a.groesse,
                    Some((
                        usize::try_from(sp).expect("fits"),
                        usize::try_from(ze).expect("fits")
                    ))
                );
                assert_eq!(neu.lines.len(), usize::try_from(ze).expect("fits"));
            }
            Err(f) => {
                assert!(!ok, "{sp} x {ze}");
                assert_eq!(f, Fehler::GroesseUngueltig);
            }
        }
    }
    assert_eq!(
        fehler(&Rahmen::voll(5, 5).mit_schub(0, 5, 1)),
        Fehler::RahmenUngueltig
    );
    // The size is read before the conflict with the scroll shows.
    assert_eq!(
        fehler(&Rahmen::voll(0, 5).mit_schub(0, 5, 1)),
        Fehler::GroesseUngueltig
    );
}

#[test]
fn a_change_needs_a_picture_and_a_rising_stand() {
    let mut r = Rahmen::aenderung();
    r.nr = 2;
    assert_eq!(fehler(&r), Fehler::KeinAbzug);
    for (stand, ok) in [
        (0, false),
        (6, false),
        (7, false),
        (8, true),
        (9, true),
        (u64::MAX, true),
    ] {
        let mut r = Rahmen::aenderung();
        r.stand = vu(stand);
        match dekodieren(&r) {
            Ok(_) => assert!(ok, "stand {stand}"),
            Err(f) => {
                assert!(!ok, "stand {stand}");
                assert_eq!(f, Fehler::StandAlt);
            }
        }
    }
}

#[test]
fn the_scroll_must_fit_the_picture() {
    for ((o, u, n), ok) in [
        ((0, 5, 1), true),
        ((0, 5, 5), true),
        ((2, 5, 3), true),
        ((1, 4, 1), true),
        ((4, 5, 1), true),
        ((0, 5, 6), false),
        ((1, 5, 5), false),
        ((0, 6, 1), false),
        ((0, 5, 0), false),
        ((3, 2, 1), false),
        ((5, 5, 1), false),
        ((u64::MAX, u64::MAX, 1), false),
        ((u64::MAX - 1, u64::MAX, 2), false),
        ((0, u64::MAX, 1), false),
    ] {
        let r = Rahmen::aenderung().mit_schub(o, u, n);
        match dekodieren(&r) {
            Ok(_) => assert!(ok, "scroll {o} {u} {n}"),
            Err(f) => {
                assert!(!ok, "scroll {o} {u} {n}");
                assert_eq!(f, Fehler::SchubUngueltig);
            }
        }
    }
}

#[test]
fn the_scroll_moves_the_region_up_and_empties_its_end() {
    let (_, neu) = dekodieren(&Rahmen::aenderung().mit_schub(1, 5, 2)).expect("valid");
    let t = |s: &str| zeile(&[(0, 0, 0, s)]);
    assert_eq!(
        neu.lines,
        vec![t("a"), t("d"), t("e"), Vec::new(), Vec::new()]
    );
    let (_, neu) = dekodieren(&Rahmen::aenderung().mit_schub(0, 3, 3)).expect("valid");
    assert_eq!(
        neu.lines,
        vec![Vec::new(), Vec::new(), Vec::new(), t("d"), t("e")]
    );
    let (_, neu) = dekodieren(&Rahmen::aenderung().mit_schub(2, 3, 1)).expect("valid");
    assert_eq!(neu.lines, vec![t("a"), t("b"), Vec::new(), t("d"), t("e")]);
}

#[test]
fn the_cursor_must_lie_in_the_picture() {
    for ((z, s, v), ok) in [
        ((0, 0, 0), true),
        ((4, 9, 1), true),
        ((4, 0, 0), true),
        ((0, 9, 1), true),
        ((5, 0, 0), false),
        ((0, 10, 0), false),
        ((5, 10, 1), false),
        ((0, 0, 2), false),
        ((0, 0, 255), false),
        ((u64::MAX, 0, 0), false),
        ((0, u64::MAX, 0), false),
    ] {
        let r = Rahmen::aenderung().mit_cursor(z, s, v);
        match dekodieren(&r) {
            Ok((a, neu)) => {
                assert!(ok, "cursor {z} {s} {v}");
                let c = a.cursor.expect("cursor");
                assert_eq!(
                    (c.zeile as u64, c.spalte as u64, c.sichtbar),
                    (z, s, v == 1)
                );
                assert_eq!(neu.cursor, c);
            }
            Err(f) => {
                assert!(!ok, "cursor {z} {s} {v}");
                assert_eq!(f, Fehler::CursorUngueltig);
            }
        }
    }
    // A full picture checks the cursor against its own size.
    assert!(dekodieren(&Rahmen::voll(3, 2).mit_cursor(1, 2, 1)).is_ok());
    assert_eq!(
        fehler(&Rahmen::voll(3, 2).mit_cursor(2, 0, 1)),
        Fehler::CursorUngueltig
    );
    assert_eq!(
        fehler(&Rahmen::voll(3, 2).mit_cursor(0, 3, 1)),
        Fehler::CursorUngueltig
    );
}

#[test]
fn styles_are_checked_and_counted_against_the_bytes_left() {
    let mut r = gut();
    r.stile = vec![(257, 0, 0)];
    assert_eq!(fehler(&r), Fehler::StilUngueltig);
    r.stile = vec![(0, 0x0200_0000, 0)];
    assert_eq!(fehler(&r), Fehler::StilUngueltig);
    r.stile = vec![(0, 0, 128)];
    assert_eq!(fehler(&r), Fehler::StilUngueltig);
    r.stile = vec![(0x00ff_ffff, 0, 0)];
    assert_eq!(fehler(&r), Fehler::StilUngueltig);
    r.stile = vec![(256, 0x01ff_ffff, 127)];
    assert!(dekodieren(&r).is_ok());
    // More styles announced than bytes follow.
    let mut r = gut();
    r.stile_zahl = Some(1000);
    assert_eq!(fehler(&r), Fehler::RahmenAbgeschnitten);
    // A count that would hide a later error is caught first: 100 lines announced, 4 bytes left, and the first line
    // would be outside the picture.
    let mut r = Rahmen::aenderung();
    r.zeilen_zahl = Some(100);
    r.rest = vec![7, 0, 0, 0];
    assert_eq!(fehler(&r), Fehler::RahmenAbgeschnitten);
    // Exactly as many bytes as the count: it passes the count check and fails at its element.
    let mut r = Rahmen::aenderung();
    r.zeilen_zahl = Some(4);
    r.rest = vec![7, 0, 0, 0];
    assert_eq!(fehler(&r), Fehler::ZeileAusserhalb);
}

#[test]
fn lines_are_checked_one_by_one() {
    let mut r = gut();
    r.zeilen[0].0 = 5;
    assert_eq!(fehler(&r), Fehler::ZeileAusserhalb);
    r.zeilen[0].0 = u64::MAX;
    assert_eq!(fehler(&r), Fehler::ZeileAusserhalb);
    r.zeilen[0].0 = 4;
    assert!(dekodieren(&r).is_ok());
    let mut r = gut();
    r.zeilen.push((1, vec![(0, b"q".to_vec())]));
    assert_eq!(fehler(&r), Fehler::ZeileReihenfolge);
    r.zeilen[1].0 = 0;
    assert_eq!(fehler(&r), Fehler::ZeileReihenfolge);
    r.zeilen[1].0 = 2;
    assert!(dekodieren(&r).is_ok());
    r.zeilen[0].0 = 0;
    r.zeilen[1].0 = 0;
    assert_eq!(fehler(&r), Fehler::ZeileReihenfolge);
    let mut r = gut();
    r.zeilen[0].1[0].0 = 1;
    assert_eq!(fehler(&r), Fehler::StilnrUngueltig);
    r.zeilen[0].1[0].0 = u64::MAX;
    assert_eq!(fehler(&r), Fehler::StilnrUngueltig);
    let mut r = gut();
    r.zeilen[0].1[0].1 = vec![0xff];
    assert_eq!(fehler(&r), Fehler::Utf8Ungueltig);
    r.zeilen[0].1[0].1 = vec![0xc3];
    assert_eq!(fehler(&r), Fehler::Utf8Ungueltig);
    r.zeilen[0].1[0].1 = vec![b'a', 0x01];
    assert_eq!(fehler(&r), Fehler::ZeichenUngueltig);
    r.zeilen[0].1[0].1 = vec![0xc2, 0x85];
    assert_eq!(fehler(&r), Fehler::ZeichenUngueltig);
    r.zeilen[0].1[0].1 = vec![0x7f];
    assert_eq!(fehler(&r), Fehler::ZeichenUngueltig);
    r.zeilen[0].1[0].1 = vec![0xc2, 0xa0];
    assert!(dekodieren(&r).is_ok());
}

#[test]
fn lines_must_be_canonical() {
    let mut r = gut();
    r.zeilen[0].1 = vec![(0, Vec::new())];
    assert_eq!(fehler(&r), Fehler::ZeileNichtKanonisch);
    let mut r = gut();
    r.zeilen[0].1 = vec![(0, b"a".to_vec()), (0, b"b".to_vec())];
    assert_eq!(fehler(&r), Fehler::ZeileNichtKanonisch);
    let mut r = gut();
    r.stile = vec![(1, 0, 0), (0, 0, 0)];
    r.zeilen[0].1 = vec![(0, b"a".to_vec()), (1, b"b ".to_vec())];
    assert_eq!(fehler(&r), Fehler::ZeileNichtKanonisch);
    r.zeilen[0].1 = vec![(0, b"a ".to_vec()), (1, b"b".to_vec())];
    assert!(dekodieren(&r).is_ok());
    // Text longer than the bytes left.
    let mut r = gut();
    r.zeilen[0].1[0].1 = b"xy".to_vec();
    let mut b = r.bytes();
    let n = b.len();
    b[n - 3] = 9;
    assert_eq!(
        rahmen_dekodieren(&b, &zustand()),
        Err(Fehler::RahmenAbgeschnitten)
    );
}

#[test]
fn nothing_may_follow_the_last_line_and_every_prefix_is_an_error() {
    let mut r = gut();
    r.rest = vec![0];
    assert_eq!(fehler(&r), Fehler::RahmenRest);
    let r = Rahmen::aenderung()
        .mit_cursor(1, 1, 1)
        .mit_schub(0, 5, 1)
        .mit_zeile(2, &[(0, "abc"), (1, "d")])
        .mit_zeile(3, &[(1, "\u{e9}")]);
    let mut r = r;
    r.flags |= 8;
    r.stile = vec![(1, 2, 3), (0, 0, 0)];
    let b = r.bytes();
    assert!(rahmen_dekodieren(&b, &zustand()).is_ok());
    for n in 0..b.len() {
        assert!(
            rahmen_dekodieren(&b[..n], &zustand()).is_err(),
            "prefix of {n} bytes"
        );
    }
}

#[test]
fn a_rejected_frame_leaves_the_state_alone() {
    let z = zustand();
    let vorher = z.clone();
    let mut r = gut();
    r.zeilen[0].1[0].1 = vec![0xff];
    assert!(rahmen_dekodieren(&r.bytes(), &z).is_err());
    assert_eq!(z, vorher);
}
