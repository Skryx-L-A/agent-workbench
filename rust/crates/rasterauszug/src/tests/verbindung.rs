//! The sender and the receiver of one connection: the messages of the core, the model picture and the forgetting
//! (RA5, RA6, RA8, RA12, RA13).

use serde_json::{Value, json};

use super::zeile;
use crate::{Cursor, Empfang, Fehler, Schub, Verbindung};

/// A message `Abzug` of 10 columns and 4 rows at stand 5.
fn abzug() -> Value {
    json!({"art": "Abzug", "sitzung": "s", "stand": 5, "groesse": [10, 4], "invers": false,
           "zeilen": ["\u{1b}[1mfett", "zwei", "", "vier"]})
}

fn aenderung(von: i64, stand: i64, zeilen: &Value) -> Value {
    json!({"art": "Rasteraenderung", "sitzung": "s", "von": von, "stand": stand, "invers": false, "zeilen": zeilen})
}

/// A connection without Deflate whose session 0 holds `abzug()`.
fn bereit() -> (Verbindung, Empfang) {
    let mut v = Verbindung::neu(false);
    let mut e = Empfang::neu(false);
    let wire = v.nimm(0, &abzug()).expect("valid").expect("a frame");
    e.nimm(&wire).expect("valid frame");
    (v, e)
}

/// Sends a message and returns what the receiver read, or the error of the sender.
fn schicken(
    v: &mut Verbindung,
    e: &mut Empfang,
    nr: u8,
    m: &Value,
) -> Result<Option<crate::Auszug>, Fehler> {
    match v.nimm(nr, m)? {
        None => Ok(None),
        Some(wire) => Ok(Some(e.nimm(&wire).expect("the receiver reads the frame").0)),
    }
}

#[test]
fn a_message_gives_a_frame_or_nothing() {
    let mut v = Verbindung::neu(false);
    let wire = v.nimm(3, &abzug()).expect("valid").expect("a frame");
    assert_eq!(
        &wire[..3],
        &[5, 3, 4],
        "frame art 5, session 3, flags: size"
    );
    let mut w = Verbindung::neu(false);
    assert_eq!(w.nimm_roh(3, &abzug()).expect("valid"), Some(wire));
    assert_eq!(v.nimm(3, &aenderung(5, 6, &json!({}))), Ok(None));
    assert_eq!(
        v.nimm_roh(3, &aenderung(6, 7, &json!({"0": "\u{1b}[1mfett"}))),
        Ok(None)
    );
}

#[test]
fn with_deflate_the_wire_differs_from_the_frame_and_the_receiver_reads_it() {
    let mut v = Verbindung::neu(true);
    let mut e = Empfang::neu(true);
    let roh = Verbindung::neu(false)
        .nimm_roh(0, &abzug())
        .expect("valid")
        .expect("a frame");
    let wire = v.nimm(0, &abzug()).expect("valid").expect("a frame");
    assert_ne!(wire, roh);
    let (auszug, abbild) = e.nimm(&wire).expect("reads");
    assert_eq!(auszug.groesse, Some((10, 4)));
    assert_eq!(abbild.lines[0], zeile(&[(0, 0, 1, "fett")]));
    // verpacken on its own is the same step the message takes.
    let mut v2 = Verbindung::neu(true);
    let roh2 = v2.nimm_roh(0, &abzug()).expect("valid").expect("a frame");
    assert_eq!(v2.verpacken(&roh2), wire);
    assert_eq!(Verbindung::neu(false).verpacken(&roh2), roh2);
}

#[test]
fn the_model_picture_is_what_the_message_said() {
    let (v, e) = bereit();
    let b = v.vorbild(0).expect("a picture");
    assert_eq!((b.spalten, b.zeilen, b.stand, b.invers), (10, 4, 5, false));
    assert_eq!(
        b.lines,
        vec![
            zeile(&[(0, 0, 1, "fett")]),
            zeile(&[(0, 0, 0, "zwei")]),
            Vec::new(),
            zeile(&[(0, 0, 0, "vier")])
        ]
    );
    assert_eq!(b.cursor, Cursor::default());
    assert!(b.gleicht(e.abbild(0).expect("a picture")));
    assert!(v.vorbild(1).is_none());
    assert!(e.abbild(1).is_none());
}

#[test]
fn forgetting_removes_the_picture_on_each_side() {
    let (mut v, mut e) = bereit();
    v.vergessen(0);
    assert!(v.vorbild(0).is_none());
    assert!(e.abbild(0).is_some());
    assert_eq!(
        v.nimm(0, &aenderung(5, 6, &json!({}))),
        Err(Fehler::KeinAbzug)
    );
    e.vergessen(0);
    assert!(e.abbild(0).is_none());
    // Forgetting a number without a picture is no error and touches no other number.
    v.vergessen(200);
    e.vergessen(200);
    let (mut v, mut e) = bereit();
    let wire = v.nimm(1, &abzug()).expect("valid").expect("a frame");
    e.nimm(&wire).expect("reads");
    v.vergessen(0);
    e.vergessen(0);
    assert!(v.vorbild(1).is_some());
    assert!(e.abbild(1).is_some());
    // A change frame for a forgotten number is rejected by the receiver.
    let (mut v, mut e) = bereit();
    let mut v_zweit = Verbindung::neu(false);
    v_zweit.nimm(0, &abzug()).expect("valid");
    let wire = v_zweit
        .nimm(0, &aenderung(5, 6, &json!({"0": "neu"})))
        .expect("valid")
        .expect("a frame");
    e.vergessen(0);
    assert_eq!(e.nimm(&wire).map(|_| ()), Err(Fehler::KeinAbzug));
    v.vergessen(0);
}

#[test]
#[allow(clippy::too_many_lines)] // one table of messages and the error each must give
fn the_message_is_checked_field_by_field() {
    let mut basis = abzug();
    basis["zeilen"] = json!(["", "", "", ""]);
    let aendere = |feld: &str, wert: Value| {
        let mut m = basis.clone();
        m[feld] = wert;
        m
    };
    let ohne = |feld: &str| {
        let mut m = basis.clone();
        m.as_object_mut().expect("object").remove(feld);
        m
    };
    let zu_gross = u64::try_from(i64::MAX).expect("fits") + 1;
    let faelle: Vec<(Value, Fehler)> = vec![
        (ohne("sitzung"), Fehler::NachrichtUngueltig),
        (ohne("stand"), Fehler::NachrichtUngueltig),
        (ohne("groesse"), Fehler::NachrichtUngueltig),
        (ohne("invers"), Fehler::NachrichtUngueltig),
        (ohne("zeilen"), Fehler::NachrichtUngueltig),
        (ohne("art"), Fehler::NachrichtUngueltig),
        (aendere("sitzung", json!(1)), Fehler::NachrichtUngueltig),
        (aendere("stand", json!(-1)), Fehler::NachrichtUngueltig),
        (aendere("stand", json!(1.5)), Fehler::NachrichtUngueltig),
        (aendere("stand", json!(3.0)), Fehler::NachrichtUngueltig),
        (aendere("stand", json!("1")), Fehler::NachrichtUngueltig),
        (aendere("stand", json!(true)), Fehler::NachrichtUngueltig),
        (aendere("stand", json!(null)), Fehler::NachrichtUngueltig),
        (
            aendere("stand", json!(zu_gross)),
            Fehler::NachrichtUngueltig,
        ),
        (aendere("invers", json!(0)), Fehler::NachrichtUngueltig),
        (aendere("invers", json!("true")), Fehler::NachrichtUngueltig),
        (aendere("invers", json!(null)), Fehler::NachrichtUngueltig),
        (aendere("zeilen", json!("abcd")), Fehler::NachrichtUngueltig),
        (aendere("zeilen", json!({})), Fehler::NachrichtUngueltig),
        (
            aendere("zeilen", json!(["", "", "", 1])),
            Fehler::NachrichtUngueltig,
        ),
        (
            aendere("zeilen", json!(["", "", ""])),
            Fehler::NachrichtUngueltig,
        ),
        (
            aendere("zeilen", json!(["", "", "", "", ""])),
            Fehler::NachrichtUngueltig,
        ),
        (aendere("groesse", json!([10])), Fehler::NachrichtUngueltig),
        (
            aendere("groesse", json!([10, 4, 1])),
            Fehler::NachrichtUngueltig,
        ),
        (aendere("groesse", json!("ab")), Fehler::NachrichtUngueltig),
        (
            aendere("groesse", json!([10.0, 4])),
            Fehler::NachrichtUngueltig,
        ),
        (
            aendere("groesse", json!([true, 4])),
            Fehler::NachrichtUngueltig,
        ),
        (
            aendere("groesse", json!([zu_gross, 4])),
            Fehler::NachrichtUngueltig,
        ),
        (aendere("groesse", json!([1, 4])), Fehler::GroesseUngueltig),
        (aendere("groesse", json!([1, 1])), Fehler::GroesseUngueltig),
        (aendere("groesse", json!([0, 4])), Fehler::GroesseUngueltig),
        (aendere("groesse", json!([10, 0])), Fehler::GroesseUngueltig),
        (
            aendere("groesse", json!([1001, 4])),
            Fehler::GroesseUngueltig,
        ),
        (
            aendere("groesse", json!([10, 1001])),
            Fehler::GroesseUngueltig,
        ),
        (aendere("groesse", json!([-1, 4])), Fehler::GroesseUngueltig),
        (
            aendere("groesse", json!([10, -4])),
            Fehler::GroesseUngueltig,
        ),
        (
            aendere("groesse", json!([i64::MAX, 4])),
            Fehler::GroesseUngueltig,
        ),
        (
            aendere("cursor", json!([4, 0, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([0, 10, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([-1, 0, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([0, -1, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([i64::MAX, 0, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([0, 0, 1])),
            Fehler::NachrichtUngueltig,
        ),
        (aendere("cursor", json!([0, 0])), Fehler::NachrichtUngueltig),
        (
            aendere("cursor", json!([0, 0, true, 1])),
            Fehler::NachrichtUngueltig,
        ),
        (
            aendere("cursor", json!([0.0, 0, true])),
            Fehler::NachrichtUngueltig,
        ),
        (aendere("cursor", json!("x")), Fehler::NachrichtUngueltig),
        (aendere("cursor", json!(null)), Fehler::NachrichtUngueltig),
        (
            aendere("zeilen", json!(["\u{1b}[1J", "", "", ""])),
            Fehler::ZeileUngueltig,
        ),
        (
            aendere("zeilen", json!(["", "", "\u{1}", ""])),
            Fehler::ZeileUngueltig,
        ),
        (aendere("art", json!("abzug")), Fehler::ArtUnbekannt),
        (aendere("art", json!("Abzug ")), Fehler::ArtUnbekannt),
        (aendere("art", json!("X")), Fehler::ArtUnbekannt),
        (aendere("art", json!("")), Fehler::ArtUnbekannt),
        (aendere("art", json!(5)), Fehler::NachrichtUngueltig),
        (aendere("art", json!(null)), Fehler::NachrichtUngueltig),
        (json!([]), Fehler::NachrichtUngueltig),
        (json!("Abzug"), Fehler::NachrichtUngueltig),
        (json!(null), Fehler::NachrichtUngueltig),
        (json!(5), Fehler::NachrichtUngueltig),
    ];
    for (m, soll) in faelle {
        let mut v = Verbindung::neu(false);
        assert_eq!(v.nimm(0, &m), Err(soll), "{m}");
        assert!(v.vorbild(0).is_none(), "{m} left a picture");
    }
    for gut in [
        aendere("stand", json!(0)),
        aendere("stand", json!(i64::MAX)),
        aendere("groesse", json!([2, 4])),
        aendere("groesse", json!([1000, 4])),
        aendere("cursor", json!([3, 9, false])),
        aendere("cursor", json!([0, 0, true])),
        aendere("sitzung", json!("")),
        aendere("invers", json!(true)),
    ] {
        assert!(Verbindung::neu(false).nimm(0, &gut).is_ok(), "{gut}");
    }
    let mut gross = abzug();
    gross["groesse"] = json!([1000, 1000]);
    gross["zeilen"] = json!(vec![""; 1000]);
    assert!(Verbindung::neu(false).nimm(0, &gross).is_ok());
    let mut eins = abzug();
    eins["groesse"] = json!([2, 1]);
    eins["zeilen"] = json!(["x"]);
    eins["cursor"] = json!([0, 0, true]);
    assert!(Verbindung::neu(false).nimm(0, &eins).is_ok());
    // one column is refused, with the rest of the message right
    let mut schmal = abzug();
    schmal["groesse"] = json!([1, 1]);
    schmal["zeilen"] = json!(["x"]);
    schmal["cursor"] = json!([0, 0, true]);
    assert_eq!(
        Verbindung::neu(false).nimm(0, &schmal),
        Err(Fehler::GroesseUngueltig)
    );
}

#[test]
fn a_change_is_checked_against_the_model_picture() {
    let zeilen = json!({"0": "neu"});
    let mut anderes_nr = Verbindung::neu(false);
    assert_eq!(
        anderes_nr.nimm(0, &aenderung(5, 6, &zeilen)),
        Err(Fehler::KeinAbzug)
    );
    let (mut v, _) = bereit();
    assert_eq!(v.nimm(1, &aenderung(5, 6, &zeilen)), Err(Fehler::KeinAbzug));
    for (von, stand, fehler) in [
        (4, 6, Some(Fehler::Luecke)),
        (6, 7, Some(Fehler::Luecke)),
        (0, 1, Some(Fehler::Luecke)),
        (5, 5, Some(Fehler::StandAlt)),
        (5, 4, Some(Fehler::StandAlt)),
        (5, 0, Some(Fehler::StandAlt)),
        (5, 6, None),
    ] {
        let (mut v, _) = bereit();
        match (v.nimm(0, &aenderung(von, stand, &zeilen)), fehler) {
            (Err(f), Some(soll)) => assert_eq!(f, soll, "von {von} stand {stand}"),
            (Ok(Some(_)), None) => {}
            (andere, soll) => panic!("von {von} stand {stand}: {andere:?} statt {soll:?}"),
        }
    }
}

#[test]
fn the_fields_of_a_change_are_checked() {
    let basis = aenderung(5, 6, &json!({"0": "neu"}));
    let aendere = |feld: &str, wert: Value| {
        let mut m = basis.clone();
        m[feld] = wert;
        m
    };
    let ohne = |feld: &str| {
        let mut m = basis.clone();
        m.as_object_mut().expect("object").remove(feld);
        m
    };
    let faelle: Vec<(Value, Fehler)> = vec![
        (ohne("sitzung"), Fehler::NachrichtUngueltig),
        (ohne("von"), Fehler::NachrichtUngueltig),
        (ohne("stand"), Fehler::NachrichtUngueltig),
        (ohne("invers"), Fehler::NachrichtUngueltig),
        (ohne("zeilen"), Fehler::NachrichtUngueltig),
        (aendere("sitzung", json!(null)), Fehler::NachrichtUngueltig),
        (aendere("von", json!(-1)), Fehler::NachrichtUngueltig),
        (aendere("von", json!(5.0)), Fehler::NachrichtUngueltig),
        (aendere("von", json!("5")), Fehler::NachrichtUngueltig),
        (aendere("stand", json!(-6)), Fehler::NachrichtUngueltig),
        (aendere("stand", json!(6.5)), Fehler::NachrichtUngueltig),
        (aendere("invers", json!(1)), Fehler::NachrichtUngueltig),
        (aendere("zeilen", json!([])), Fehler::NachrichtUngueltig),
        (aendere("zeilen", json!("0")), Fehler::NachrichtUngueltig),
        (
            aendere("zeilen", json!({"0": 1})),
            Fehler::NachrichtUngueltig,
        ),
        (
            aendere("zeilen", json!({"0": null})),
            Fehler::NachrichtUngueltig,
        ),
        (
            aendere("zeilen", json!({"0": "\u{1b}[1J"})),
            Fehler::ZeileUngueltig,
        ),
        (
            aendere("zeilen", json!({"3": "\u{1}"})),
            Fehler::ZeileUngueltig,
        ),
        (
            aendere("cursor", json!([4, 0, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([0, 10, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([-1, 0, true])),
            Fehler::CursorUngueltig,
        ),
        (
            aendere("cursor", json!([0, 0, 0])),
            Fehler::NachrichtUngueltig,
        ),
        (aendere("cursor", json!(null)), Fehler::NachrichtUngueltig),
        (
            aendere("art", json!("Rasteraenderung ")),
            Fehler::ArtUnbekannt,
        ),
        (
            aendere("art", json!("rasteraenderung")),
            Fehler::ArtUnbekannt,
        ),
    ];
    for (m, soll) in faelle {
        let (mut v, _) = bereit();
        let vorher = v.vorbild(0).cloned();
        assert_eq!(v.nimm(0, &m), Err(soll), "{m}");
        assert_eq!(v.vorbild(0).cloned(), vorher, "{m} changed the picture");
    }
    for gut in [
        aendere("cursor", json!([3, 9, true])),
        aendere("cursor", json!([0, 0, false])),
        aendere("zeilen", json!({"3": "x"})),
        aendere("zeilen", json!({"0": "a", "1": "b", "2": "c", "3": "d"})),
        aendere("sitzung", json!("andere")),
    ] {
        let (mut v, _) = bereit();
        assert!(v.nimm(0, &gut).is_ok(), "{gut}");
    }
}

#[test]
fn a_row_key_is_a_decimal_number_without_leading_zero_below_the_row_count() {
    for schluessel in [
        "-1",
        "00",
        "01",
        "+1",
        "1.0",
        " 1",
        "1 ",
        "",
        "\u{663}",
        "99999999999999999999999",
        "1e1",
        "4",
        "10",
        "x",
        "0x1",
        "\u{ff11}",
        "0.0",
    ] {
        let (mut v, _) = bereit();
        let mut zeilen = serde_json::Map::new();
        zeilen.insert(schluessel.to_owned(), json!("a"));
        assert_eq!(
            v.nimm(0, &aenderung(5, 6, &Value::Object(zeilen))),
            Err(Fehler::ZeileAusserhalb),
            "{schluessel:?}"
        );
    }
    for schluessel in ["0", "1", "2", "3"] {
        let (mut v, _) = bereit();
        let mut zeilen = serde_json::Map::new();
        zeilen.insert(schluessel.to_owned(), json!("a"));
        assert!(
            v.nimm(0, &aenderung(5, 6, &Value::Object(zeilen))).is_ok(),
            "{schluessel:?}"
        );
    }
}

#[test]
fn a_change_lists_only_the_lines_that_differ() {
    let (mut v, mut e) = bereit();
    let m = aenderung(
        5,
        6,
        &json!({"0": "\u{1b}[1mfett", "1": "neu", "2": "", "3": "\u{1b}[0mvier  "}),
    );
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert_eq!(a.nr, 0);
    assert_eq!(a.stand, 6);
    assert_eq!(a.groesse, None);
    assert_eq!(a.schub, None);
    assert_eq!(a.cursor, None);
    assert!(!a.invers);
    assert_eq!(a.zeilen, vec![(1, zeile(&[(0, 0, 0, "neu")]))]);
    // A line that becomes empty is listed with no runs; a changed style at equal text is a change.
    let m = aenderung(6, 7, &json!({"1": "", "0": "fett"}));
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert_eq!(
        a.zeilen,
        vec![(0, zeile(&[(0, 0, 0, "fett")])), (1, Vec::new())]
    );
    assert_eq!(v.vorbild(0).expect("picture").stand, 7);
}

#[test]
fn nothing_to_send_still_advances_the_stand() {
    let (mut v, mut e) = bereit();
    assert_eq!(
        schicken(&mut v, &mut e, 0, &aenderung(5, 6, &json!({"3": "vier"}))),
        Ok(None)
    );
    assert_eq!(v.vorbild(0).expect("picture").stand, 6);
    assert_eq!(
        schicken(&mut v, &mut e, 0, &aenderung(5, 7, &json!({}))),
        Err(Fehler::Luecke)
    );
    assert_eq!(
        schicken(&mut v, &mut e, 0, &aenderung(6, 7, &json!({}))),
        Ok(None)
    );
    let a = schicken(&mut v, &mut e, 0, &aenderung(7, 8, &json!({"2": "x"})))
        .expect("valid")
        .expect("frame");
    assert_eq!(a.stand, 8);
    // The receiver has seen stands 5 and 8 only: a gap on its side is fine, it asks only for a rising stand.
    assert_eq!(e.abbild(0).expect("picture").stand, 8);
}

#[test]
fn invers_alone_makes_a_frame_without_lines_and_the_same_value_makes_none() {
    let (mut v, mut e) = bereit();
    let mut m = aenderung(5, 6, &json!({}));
    m["invers"] = json!(true);
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert!(a.invers);
    assert_eq!(a.zeilen, Vec::new());
    assert_eq!((a.schub, a.cursor), (None, None));
    assert!(e.abbild(0).expect("picture").invers);
    let mut m = aenderung(6, 7, &json!({}));
    m["invers"] = json!(true);
    assert_eq!(schicken(&mut v, &mut e, 0, &m), Ok(None));
    let m = aenderung(7, 8, &json!({}));
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert!(!a.invers, "back to normal is a change");
    assert!(!v.vorbild(0).expect("picture").invers);
    // Every frame carries the value of its message.
    let mut m = aenderung(8, 9, &json!({"0": "x"}));
    m["invers"] = json!(true);
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert!(a.invers);
}

#[test]
fn the_cursor_travels_only_when_it_changes() {
    let (mut v, mut e) = bereit();
    // The picture had no cursor field: the cursor stands at (0, 0) hidden. The same value is no change.
    let mut m = aenderung(5, 6, &json!({}));
    m["cursor"] = json!([0, 0, false]);
    assert_eq!(schicken(&mut v, &mut e, 0, &m), Ok(None));
    let mut m = aenderung(6, 7, &json!({}));
    m["cursor"] = json!([0, 0, true]);
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert_eq!(
        a.cursor,
        Some(Cursor {
            zeile: 0,
            spalte: 0,
            sichtbar: true
        }),
        "only `sichtbar` differs"
    );
    let mut m = aenderung(7, 8, &json!({"1": "x"}));
    m["cursor"] = json!([0, 0, true]);
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert_eq!(a.cursor, None, "the same cursor is not repeated");
    let mut m = aenderung(8, 9, &json!({}));
    m["cursor"] = json!([3, 9, false]);
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert_eq!(
        a.cursor,
        Some(Cursor {
            zeile: 3,
            spalte: 9,
            sichtbar: false
        })
    );
    assert_eq!(
        v.vorbild(0).expect("picture").cursor,
        Cursor {
            zeile: 3,
            spalte: 9,
            sichtbar: false
        }
    );
    // No cursor field: the model picture keeps its cursor and the frame has none.
    let a = schicken(&mut v, &mut e, 0, &aenderung(9, 10, &json!({"1": "y"})))
        .expect("valid")
        .expect("a frame");
    assert_eq!(a.cursor, None);
    assert_eq!(
        v.vorbild(0).expect("picture").cursor,
        Cursor {
            zeile: 3,
            spalte: 9,
            sichtbar: false
        }
    );
    assert_eq!(
        e.abbild(0).expect("picture").cursor,
        Cursor {
            zeile: 3,
            spalte: 9,
            sichtbar: false
        }
    );
}

#[test]
fn a_full_picture_replaces_everything_and_resets_the_cursor() {
    let (mut sender, mut empfang) = bereit();
    let mut nachricht = abzug();
    nachricht["cursor"] = json!([2, 2, true]);
    nachricht["stand"] = json!(1);
    let auszug = schicken(&mut sender, &mut empfang, 0, &nachricht)
        .expect("valid")
        .expect("a frame");
    assert_eq!(
        auszug.cursor,
        Some(Cursor {
            zeile: 2,
            spalte: 2,
            sichtbar: true
        })
    );
    assert_eq!(auszug.stand, 1, "a full picture may take a lower stand");
    assert_eq!(sender.vorbild(0).expect("picture").cursor.zeile, 2);
    // The next full picture has no cursor: the model picture goes back to the default.
    let mut nachricht = abzug();
    nachricht["groesse"] = json!([3, 2]);
    nachricht["zeilen"] = json!(["a", ""]);
    nachricht["stand"] = json!(2);
    let auszug = schicken(&mut sender, &mut empfang, 0, &nachricht)
        .expect("valid")
        .expect("a frame");
    assert_eq!(auszug.groesse, Some((3, 2)));
    assert_eq!(auszug.cursor, None);
    let bild = sender.vorbild(0).expect("picture");
    assert_eq!(bild.cursor, Cursor::default());
    assert_eq!((bild.spalten, bild.zeilen, bild.stand), (3, 2, 2));
    // Only the lines that are not empty are listed.
    assert_eq!(auszug.zeilen, vec![(0, zeile(&[(0, 0, 0, "a")]))]);
    assert!(bild.gleicht(empfang.abbild(0).expect("picture")));
}

#[test]
fn a_scroll_in_a_change_is_found_and_applied() {
    let mut v = Verbindung::neu(false);
    let mut e = Empfang::neu(false);
    let mut m = abzug();
    m["groesse"] = json!([10, 8]);
    m["zeilen"] = json!(["k", "L1", "L2", "L3", "L4", "L5", "L6", "f"]);
    schicken(&mut v, &mut e, 0, &m).expect("valid");
    let m = aenderung(
        5,
        6,
        &json!({"1": "L2", "2": "L3", "3": "L4", "4": "L5", "5": "L6", "6": "neu"}),
    );
    let a = schicken(&mut v, &mut e, 0, &m)
        .expect("valid")
        .expect("a frame");
    assert_eq!(
        a.schub,
        Some(Schub {
            oben: 1,
            unten: 7,
            n: 1
        })
    );
    assert_eq!(a.zeilen, vec![(6, zeile(&[(0, 0, 0, "neu")]))]);
    assert!(
        v.vorbild(0)
            .expect("picture")
            .gleicht(e.abbild(0).expect("picture"))
    );
}

#[test]
fn connections_and_session_numbers_do_not_share_pictures() {
    let mut a = Verbindung::neu(false);
    let mut b = Verbindung::neu(false);
    a.nimm(0, &abzug()).expect("valid");
    assert!(b.vorbild(0).is_none());
    assert_eq!(
        b.nimm(0, &aenderung(5, 6, &json!({"0": "x"}))),
        Err(Fehler::KeinAbzug)
    );
    // The sessions of one connection interleave freely and keep their own stand.
    let mut ea = Empfang::neu(false);
    let mut m = abzug();
    m["stand"] = json!(50);
    for nr in [1, 2] {
        let wire = a.nimm(nr, &m).expect("valid").expect("a frame");
        ea.nimm(&wire).expect("reads");
    }
    assert_eq!(a.vorbild(0).expect("picture").stand, 5);
    assert_eq!(a.vorbild(1).expect("picture").stand, 50);
    let w = a
        .nimm(2, &aenderung(50, 51, &json!({"0": "x"})))
        .expect("valid")
        .expect("a frame");
    ea.nimm(&w).expect("reads");
    let w = a
        .nimm(1, &aenderung(50, 52, &json!({"0": "y"})))
        .expect("valid")
        .expect("a frame");
    ea.nimm(&w).expect("reads");
    assert_eq!(ea.abbild(1).expect("picture").stand, 52);
    assert_eq!(ea.abbild(2).expect("picture").stand, 51);
}

#[test]
fn a_receiver_is_strict_about_the_stand_of_what_it_gets() {
    let (mut v, mut e) = bereit();
    let w1 = v
        .nimm(0, &aenderung(5, 6, &json!({"0": "a"})))
        .expect("valid")
        .expect("frame");
    let w2 = v
        .nimm(0, &aenderung(6, 7, &json!({"0": "b"})))
        .expect("valid")
        .expect("frame");
    e.nimm(&w2).expect("a later frame is read");
    assert_eq!(e.nimm(&w1).map(|_| ()), Err(Fehler::StandAlt));
    assert_eq!(e.abbild(0).expect("picture").stand, 7);
    assert_eq!(e.nimm(&w2).map(|_| ()), Err(Fehler::StandAlt));
}

#[test]
fn the_receiver_checks_the_length_of_a_frame_without_deflate() {
    use crate::RAHMEN_MAX;
    let mut e = Empfang::neu(false);
    assert_eq!(
        e.nimm(&vec![0; RAHMEN_MAX]).map(|_| ()),
        Err(Fehler::ArtUngueltig)
    );
    assert_eq!(
        e.nimm(&vec![0; RAHMEN_MAX + 1]).map(|_| ()),
        Err(Fehler::RahmenZuGross)
    );
    assert_eq!(e.nimm(&[]).map(|_| ()), Err(Fehler::RahmenAbgeschnitten));
    let mut d = Empfang::neu(true);
    assert_eq!(d.nimm(&[]).map(|_| ()), Err(Fehler::RahmenAbgeschnitten));
}

#[test]
fn the_receiver_returns_the_frame_after_inflating() {
    let mut v = Verbindung::neu(true);
    let mut e = Empfang::neu(true);
    let roh = Verbindung::neu(false)
        .nimm_roh(0, &abzug())
        .expect("valid")
        .expect("frame");
    let wire = v.nimm(0, &abzug()).expect("valid").expect("frame");
    let (roh2, auszug, abbild) = e.nimm_roh(&wire).expect("reads");
    assert_eq!(roh2, roh);
    assert_eq!(auszug.nr, 0);
    assert_eq!(abbild.stand, 5);
}
