//! The picture: its empty form and the comparison the round trip uses.

use super::zeile;
use crate::{Bild, Cursor};

#[test]
fn an_empty_picture_is_empty_everywhere() {
    let b = Bild::leer(7, 3);
    assert_eq!((b.spalten, b.zeilen), (7, 3));
    assert_eq!(b.lines, vec![Vec::new(), Vec::new(), Vec::new()]);
    assert!(!b.invers);
    assert_eq!(b.cursor, Cursor::default());
    assert_eq!(
        b.cursor,
        Cursor {
            zeile: 0,
            spalte: 0,
            sichtbar: false
        }
    );
    assert_eq!(b.stand, 0);
}

#[test]
fn two_pictures_are_alike_when_everything_but_the_stand_is_alike() {
    let basis = {
        let mut b = Bild::leer(4, 2);
        b.lines[1] = zeile(&[(1, 0, 0, "x")]);
        b
    };
    assert!(basis.gleicht(&basis.clone()));
    let mut anderer_stand = basis.clone();
    anderer_stand.stand = 5;
    assert!(basis.gleicht(&anderer_stand), "the stand is not compared");

    let mut a = basis.clone();
    a.spalten = 5;
    assert!(!basis.gleicht(&a));
    let mut a = basis.clone();
    a.zeilen = 3;
    assert!(!basis.gleicht(&a));
    let mut a = basis.clone();
    a.invers = true;
    assert!(!basis.gleicht(&a));
    let mut a = basis.clone();
    a.cursor.sichtbar = true;
    assert!(!basis.gleicht(&a));
    let mut a = basis.clone();
    a.lines[1] = zeile(&[(1, 0, 0, "y")]);
    assert!(!basis.gleicht(&a));
    let mut a = basis.clone();
    a.lines[0] = zeile(&[(0, 0, 0, "y")]);
    assert!(!basis.gleicht(&a));
}
