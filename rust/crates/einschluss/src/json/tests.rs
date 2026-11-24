//! Tests of the strict JSON reader and of the writer.

use super::*;

fn ok(text: &str) -> Wert {
    lesen(text.as_bytes()).unwrap_or_else(|| panic!("should read: {text}"))
}

fn nein(text: &str) {
    assert_eq!(lesen(text.as_bytes()), None, "should refuse: {text}");
}

#[test]
fn the_values_of_the_description_are_read() {
    assert_eq!(ok("null"), Wert::Null);
    assert_eq!(ok("0"), Wert::Zahl(0));
    assert_eq!(ok("1000"), Wert::Zahl(1000));
    assert_eq!(ok("\"a\""), Wert::Text("a".to_owned()));
    assert_eq!(ok("[]"), Wert::Liste(vec![]));
    assert_eq!(
        ok(" [ \"a\" , null ] "),
        Wert::Liste(vec![Wert::Text("a".to_owned()), Wert::Null])
    );
    let Wert::Objekt(o) = ok("{\"k\":\"v\", \"n\":1}") else {
        panic!("object");
    };
    assert_eq!(o.get("k"), Some(&Wert::Text("v".to_owned())));
    assert_eq!(o.get("n"), Some(&Wert::Zahl(1)));
    assert_eq!(ok("{}"), Wert::Objekt(BTreeMap::new()));
}

#[test]
fn white_space_is_only_the_four_json_characters() {
    assert_eq!(ok("\t\r\n null \n"), Wert::Null);
    nein("\u{b}null");
    nein("\u{a0}null");
}

#[test]
fn strings_know_the_escapes_and_nothing_else() {
    assert_eq!(
        ok(r#""\" \\ \/ \b \f \n \r \t \u0041""#),
        Wert::Text("\" \\ / \u{8} \u{c} \n \r \t A".to_owned())
    );
    assert_eq!(ok(r#""\ud83d\ude00""#), Wert::Text("\u{1f600}".to_owned()));
    assert_eq!(ok("\"\u{e4}\""), Wert::Text("\u{e4}".to_owned()));
    nein(r#""\x""#);
    nein(r#""\u12""#);
    nein(r#""\u12G4""#);
    nein(r#""\ud83d""#);
    nein(r#""\ud83dx""#);
    nein(r#""\ud83d\u0041""#);
    nein(r#""\ude00""#);
    nein("\"a\nb\"");
    nein("\"a\u{1}b\"");
    nein("\"offen");
    nein("\"\\");
}

#[test]
fn invalid_utf8_is_refused() {
    assert_eq!(lesen(b"\"\xff\""), None);
    assert_eq!(lesen(b"\xef\xbb\xbfnull"), None);
}

#[test]
fn numbers_are_plain_unsigned_integers() {
    nein("-1");
    nein("+1");
    nein("01");
    nein("1.5");
    nein("1e3");
    nein("1E3");
    nein("1.");
    nein("18446744073709551616");
    assert_eq!(ok("18446744073709551615"), Wert::Zahl(u64::MAX));
}

#[test]
fn the_refusals_of_the_structure() {
    nein("");
    nein("   ");
    nein("nul");
    nein("nullx");
    nein("null null");
    nein("true");
    nein("false");
    nein("[");
    nein("[1");
    nein("[1,]");
    nein("[,1]");
    nein("[1 2]");
    nein("{");
    nein("{\"a\"}");
    nein("{\"a\":}");
    nein("{\"a\":1,}");
    nein("{a:1}");
    nein("{\"a\" 1}");
    nein("{\"a\":1 \"b\":2}");
    nein("{\"a\":1}x");
    nein("]");
}

#[test]
fn a_key_twice_is_refused() {
    nein("{\"a\":1,\"a\":2}");
    nein("{\"a\":1,\"a\":1}");
    ok("{\"a\":1,\"b\":1}");
}

#[test]
fn the_nesting_is_limited() {
    let tief = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
    assert!(lesen(tief(TIEFE_MAX).as_bytes()).is_some());
    assert_eq!(lesen(tief(TIEFE_MAX + 1).as_bytes()), None);
    let objekte = |n: usize| format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n));
    assert!(lesen(objekte(TIEFE_MAX).as_bytes()).is_some());
    assert_eq!(lesen(objekte(TIEFE_MAX + 1).as_bytes()), None);
}

#[test]
fn the_writer_escapes_and_the_reader_takes_it_back() {
    let mut s = String::new();
    text_schreiben(&mut s, "a\"b\\c\nd\u{1}e\u{7f}f\u{e4}");
    assert_eq!(s, "\"a\\\"b\\\\c\\u000ad\\u0001e\\u007ff\u{e4}\"");
    assert_eq!(
        ok(&s),
        Wert::Text("a\"b\\c\nd\u{1}e\u{7f}f\u{e4}".to_owned())
    );
    let mut leer = String::new();
    text_schreiben(&mut leer, "");
    assert_eq!(leer, "\"\"");
    let mut tab = String::new();
    text_schreiben(&mut tab, "\t");
    assert_eq!(tab, "\"\\u0009\"");
    let mut grenze = String::new();
    text_schreiben(&mut grenze, " ~");
    assert_eq!(grenze, "\" ~\"");
}
