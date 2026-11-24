//! The loop: reading lines with a limit, writing one answer line each.

mod gemeinsam;

use std::io::{self, BufRead, Cursor, Read, Write};

use gemeinsam::Attrappe;
use serde_json::Value;
use such_mcp::zeilen::{MAX_ZEILE, Zeile, lauf, naechste};

#[allow(clippy::unnecessary_wraps)]
fn ganz(text: &str) -> Option<Zeile> {
    Some(Zeile::Ganz(text.as_bytes().to_vec()))
}

#[test]
fn the_limit_is_eight_mebibytes() {
    assert_eq!(MAX_ZEILE, 8 * 1024 * 1024);
}

#[test]
fn lines_are_cut_at_the_lf() {
    let mut eingabe = Cursor::new(b"ab\n\ncd\r\nef".to_vec());
    assert_eq!(naechste(&mut eingabe, 10).expect("read"), ganz("ab"));
    assert_eq!(naechste(&mut eingabe, 10).expect("read"), ganz(""));
    assert_eq!(naechste(&mut eingabe, 10).expect("read"), ganz("cd\r"));
    assert_eq!(naechste(&mut eingabe, 10).expect("read"), ganz("ef"));
    assert_eq!(naechste(&mut eingabe, 10).expect("read"), None);
    assert_eq!(naechste(&mut eingabe, 10).expect("read"), None);
}

#[test]
fn an_empty_input_has_no_line() {
    assert_eq!(
        naechste(&mut Cursor::new(Vec::new()), 10).expect("read"),
        None
    );
}

#[test]
fn a_line_of_exactly_the_limit_is_whole_and_one_byte_more_is_dropped() {
    for (eingabe, erwartet) in [
        ("abc\nnext\n", ganz("abc")),
        ("abc", ganz("abc")),
        ("abcd\nnext\n", Some(Zeile::ZuLang)),
        ("abcd", Some(Zeile::ZuLang)),
        ("abcdefgh\nnext\n", Some(Zeile::ZuLang)),
        ("abcd\n", Some(Zeile::ZuLang)),
    ] {
        let mut leser = Cursor::new(eingabe.as_bytes().to_vec());
        assert_eq!(
            naechste(&mut leser, 3).expect("read"),
            erwartet,
            "{eingabe:?}"
        );
    }
}

#[test]
fn a_dropped_line_is_read_to_its_end_and_the_next_line_is_intact() {
    for laenge in [
        4usize,
        5,
        64 * 1024 - 1,
        64 * 1024,
        64 * 1024 + 1,
        64 * 1024 + 4,
        200_000,
        1_000_000,
    ] {
        let mut daten = vec![b'x'; laenge];
        daten.extend_from_slice(b"\nnx\nlast");
        let mut leser = Cursor::new(daten);
        assert_eq!(
            naechste(&mut leser, 3).expect("read"),
            Some(Zeile::ZuLang),
            "{laenge}"
        );
        assert_eq!(
            naechste(&mut leser, 3).expect("read"),
            ganz("nx"),
            "{laenge}"
        );
        assert_eq!(
            naechste(&mut leser, 3).expect("read"),
            Some(Zeile::ZuLang),
            "{laenge}"
        );
        assert_eq!(naechste(&mut leser, 3).expect("read"), None, "{laenge}");
    }
    let mut daten = vec![b'x'; 300_000];
    daten.extend_from_slice(b"\nnext\nlast");
    let mut leser = Cursor::new(daten);
    assert_eq!(naechste(&mut leser, 10).expect("read"), Some(Zeile::ZuLang));
    assert_eq!(naechste(&mut leser, 10).expect("read"), ganz("next"));
    assert_eq!(naechste(&mut leser, 10).expect("read"), ganz("last"));
    assert_eq!(naechste(&mut leser, 10).expect("read"), None);
}

#[test]
fn a_dropped_line_without_lf_ends_the_input() {
    let mut leser = Cursor::new(vec![b'x'; 300_000]);
    assert_eq!(naechste(&mut leser, 10).expect("read"), Some(Zeile::ZuLang));
    assert_eq!(naechste(&mut leser, 10).expect("read"), None);
}

struct Zaehler {
    ausgabe: Vec<u8>,
    leerungen: usize,
}

impl Write for Zaehler {
    fn write(&mut self, daten: &[u8]) -> io::Result<usize> {
        self.ausgabe.extend_from_slice(daten);
        Ok(daten.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.leerungen += 1;
        Ok(())
    }
}

#[test]
fn every_answer_is_one_line_followed_by_a_flush_and_notifications_write_nothing() {
    let eingabe = concat!(
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n",
        "kein json\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}"
    );
    let mut ausgabe = Zaehler {
        ausgabe: Vec::new(),
        leerungen: 0,
    };
    lauf(
        &mut Cursor::new(eingabe.as_bytes().to_vec()),
        &mut ausgabe,
        &Attrappe::mit("{}"),
        1000,
    )
    .expect("run");
    let text = String::from_utf8(ausgabe.ausgabe).expect("UTF-8");
    assert!(text.ends_with('\n'));
    let zeilen: Vec<Value> = text
        .lines()
        .map(|z| serde_json::from_str(z).expect("JSON"))
        .collect();
    assert_eq!(zeilen.len(), 3);
    assert_eq!(zeilen[0]["id"], 1);
    assert_eq!(zeilen[1]["error"]["code"], -32700);
    assert_eq!(zeilen[2]["id"], 2);
    assert_eq!(ausgabe.leerungen, 3);
}

#[test]
fn a_too_long_line_gets_an_error_and_the_loop_goes_on() {
    let mut eingabe = vec![b'x'; 5000];
    eingabe.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n");
    let mut ausgabe = Zaehler {
        ausgabe: Vec::new(),
        leerungen: 0,
    };
    lauf(
        &mut Cursor::new(eingabe),
        &mut ausgabe,
        &Attrappe::mit("{}"),
        1000,
    )
    .expect("run");
    let text = String::from_utf8(ausgabe.ausgabe).expect("UTF-8");
    let zeilen: Vec<Value> = text
        .lines()
        .map(|z| serde_json::from_str(z).expect("JSON"))
        .collect();
    assert_eq!(zeilen.len(), 2);
    assert_eq!(zeilen[0]["error"]["code"], -32600);
    assert_eq!(zeilen[0]["id"], Value::Null);
    assert_eq!(zeilen[1]["id"], 7);
}

#[test]
fn empty_lines_are_skipped_without_an_answer_and_the_loop_goes_on() {
    let eingabe = "\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n\n\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n\n";
    let mut ausgabe = Zaehler {
        ausgabe: Vec::new(),
        leerungen: 0,
    };
    lauf(
        &mut Cursor::new(eingabe.as_bytes().to_vec()),
        &mut ausgabe,
        &Attrappe::mit("{}"),
        1000,
    )
    .expect("run");
    let text = String::from_utf8(ausgabe.ausgabe).expect("UTF-8");
    let ids: Vec<Value> = text
        .lines()
        .map(|z| serde_json::from_str::<Value>(z).expect("JSON")["id"].clone())
        .collect();
    assert_eq!(ids, [Value::from(1), Value::from(2)]);
}

#[test]
fn an_empty_input_writes_nothing() {
    let mut ausgabe = Zaehler {
        ausgabe: Vec::new(),
        leerungen: 0,
    };
    lauf(
        &mut Cursor::new(Vec::new()),
        &mut ausgabe,
        &Attrappe::mit("{}"),
        1000,
    )
    .expect("run");
    assert_eq!(ausgabe.ausgabe, Vec::<u8>::new());
    assert_eq!(ausgabe.leerungen, 0);
}

struct KaputterLeser;

impl Read for KaputterLeser {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read error"))
    }
}

impl BufRead for KaputterLeser {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Err(io::Error::other("read error"))
    }
    fn consume(&mut self, _: usize) {}
}

struct KaputterSchreiber;

impl Write for KaputterSchreiber {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("write error"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_read_error_and_a_write_error_end_the_loop_with_an_error() {
    let mut ausgabe = Zaehler {
        ausgabe: Vec::new(),
        leerungen: 0,
    };
    assert!(lauf(&mut KaputterLeser, &mut ausgabe, &Attrappe::mit("{}"), 1000).is_err());
    let eingabe = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n".to_vec();
    assert!(
        lauf(
            &mut Cursor::new(eingabe),
            &mut KaputterSchreiber,
            &Attrappe::mit("{}"),
            1000
        )
        .is_err()
    );
}
