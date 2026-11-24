use serde_json::json;

use super::*;

#[test]
fn kennung_grenzen() {
    assert!(kennung("a"));
    assert!(kennung("0-a._b"));
    assert!(kennung(&"a".repeat(64)));
    assert!(!kennung(&"a".repeat(65)));
    assert!(!kennung(""));
    assert!(!kennung("-a"));
    assert!(!kennung(".a"));
    assert!(!kennung("A"));
    assert!(!kennung("a b"));
    assert!(!kennung("aB"));
}

#[test]
fn werkzeug_grenzen() {
    assert!(werkzeug("A"));
    assert!(werkzeug("brain_suche"));
    assert!(werkzeug("Get.Item-2"));
    assert!(werkzeug(&"W".repeat(128)));
    assert!(!werkzeug(&"W".repeat(129)));
    assert!(!werkzeug(""));
    assert!(!werkzeug("_a"));
    assert!(!werkzeug("a b"));
    assert!(!werkzeug("a/b"));
}

#[test]
fn pseudonym_und_hex() {
    assert!(pseudonym(&format!("ps_{}", "0".repeat(32))));
    assert!(pseudonym(&format!("ps_{}", "af09".repeat(8))));
    assert!(!pseudonym(&format!("ps_{}", "0".repeat(31))));
    assert!(!pseudonym(&format!("ps_{}", "0".repeat(33))));
    assert!(!pseudonym(&format!("ps_{}", "A".repeat(32))));
    assert!(!pseudonym(&format!("ps_{}g", "0".repeat(31))));
    assert!(!pseudonym(&format!("px_{}", "0".repeat(32))));
    assert!(hex("09af", 4));
    assert!(!hex("09af", 3));
    assert!(!hex("09aF", 4));
}

#[test]
fn anfrage_grenzen() {
    assert!(anfrage("mf-0-1"));
    assert!(anfrage("mf-1795179540000-1"));
    assert!(anfrage(&format!("mf-{}-1", "9".repeat(15))));
    assert!(!anfrage(&format!("mf-{}-1", "9".repeat(16))));
    assert!(anfrage(&format!("mf-1-1{}", "0".repeat(9))));
    assert!(!anfrage(&format!("mf-1-1{}", "0".repeat(10))));
    assert!(!anfrage("mf-1-0"));
    assert!(!anfrage("mf-1-01"));
    assert!(!anfrage("mf--1"));
    assert!(!anfrage("mf-1-"));
    assert!(!anfrage("mf-1"));
    assert!(!anfrage("mf-a-1"));
    assert!(!anfrage("mf-1-1a"));
    assert!(!anfrage("mf-1-1-1"));
    assert!(!anfrage("mg-1-1"));
}

#[test]
fn ganzzahlen() {
    assert!(ganzzahl_in(&json!(1), 1, 3));
    assert!(ganzzahl_in(&json!(3), 1, 3));
    assert!(!ganzzahl_in(&json!(0), 1, 3));
    assert!(!ganzzahl_in(&json!(4), 1, 3));
    assert!(!ganzzahl_in(&json!(2.0), 1, 3));
}

#[test]
fn doppelte_schluessel() {
    let doppelt = |t: &str| schluessel_doppelt(t.as_bytes());
    assert!(doppelt(r#"{"a":1,"a":2}"#));
    assert!(doppelt(r#"{"a":1,"a":2}"#));
    assert!(doppelt(r#"{"x":{"a":1,"a":2}}"#));
    assert!(doppelt(r#"[{"a":1,"a":2}]"#));
    assert!(doppelt(r#"{"a":[1,{"b":2}],"a":3}"#));
    assert!(doppelt(r#"{"a\"":1,"b":{},"a\"":2}"#));
    assert!(!doppelt(r#"{"a":1,"b":2}"#));
    assert!(!doppelt(r#"{"a":"b","b":"a"}"#));
    assert!(!doppelt(r#"[{"a":1},{"a":1}]"#));
    assert!(!doppelt(r#"{"a":{"a":1}}"#));
    assert!(!doppelt(r#"{"a":[{"b":1}],"b":2}"#));
    assert!(!doppelt(r#"{"a":["b","b"],"b":1}"#));
    assert!(!doppelt(r#"{"a\\":1,"a":2}"#));
    assert!(!doppelt(r#"["a","a"]"#));
    assert!(!doppelt(r#"{"a":{},"b":[],"c":1}"#));
}

#[test]
fn objekt_lesen_streng() {
    assert!(objekt_lesen(br#"{"a":1}"#).is_some());
    assert!(objekt_lesen(b" {\"a\":1}\n").is_some());
    assert!(objekt_lesen(br#"{"a":1,"a":1}"#).is_none());
    assert!(objekt_lesen(b"\xef\xbb\xbf{}").is_none());
    assert!(objekt_lesen(b"{} x").is_none());
    assert!(objekt_lesen(b"[]").is_none());
    assert!(objekt_lesen(b"{\"a\":\"\xff\"}").is_none());
}

/// The frame of the shell (contract 6, section 13.3): one case per kind that the strict reading refuses.
/// The shell is built later; the reading it takes is this one, so the kinds are fixed here.
#[test]
fn rahmen_streng_gelesen() {
    let gut = br#"{"vorgang":"mcp.anfragen","eingabe":{"server":"tickets","werkzeug":"update"}}"#;
    let felder = objekt_lesen(gut).expect("the frame is read");
    assert_eq!(felder.len(), 2);
    assert_eq!(felder["vorgang"], json!("mcp.anfragen"));
    let abgewiesen: [(&str, &[u8]); 11] = [
        ("kein JSON", b"vorgang=mcp.anfragen"),
        ("kein Objekt", br#"["mcp.anfragen",{}]"#),
        (
            "Schluessel doppelt, oberste Ebene",
            br#"{"vorgang":"mcp.anfragen","eingabe":{},"eingabe":{}}"#,
        ),
        (
            "Schluessel doppelt, in der Eingabe",
            br#"{"vorgang":"mcp.anfragen","eingabe":{"argumente":1,"argumente":2}}"#,
        ),
        (
            "Zahl ausserhalb von JSON: NaN",
            br#"{"vorgang":"mcp.anfragen","eingabe":{"frist_ms":NaN}}"#,
        ),
        (
            "Zahl ausserhalb von JSON: Infinity",
            br#"{"vorgang":"mcp.anfragen","eingabe":{"frist_ms":Infinity}}"#,
        ),
        (
            "Zahl ausserhalb von JSON: 1e999",
            br#"{"vorgang":"mcp.anfragen","eingabe":{"frist_ms":1e999}}"#,
        ),
        (
            "Komma am Ende",
            br#"{"vorgang":"mcp.anfragen","eingabe":{},}"#,
        ),
        (
            "Text nach dem Objekt",
            br#"{"vorgang":"mcp.anfragen","eingabe":{}} {"vorgang":"mcp.offene","eingabe":{}}"#,
        ),
        (
            "BOM vor dem Objekt",
            b"\xef\xbb\xbf{\"vorgang\":\"mcp.offene\",\"eingabe\":{}}",
        ),
        (
            "kein UTF-8",
            b"{\"vorgang\":\"mcp.anfragen\",\"eingabe\":{\"a\":\"\xff\"}}",
        ),
    ];
    for (art, bytes) in abgewiesen {
        assert!(objekt_lesen(bytes).is_none(), "{art}");
    }
}

#[test]
fn base64_lesen() {
    assert_eq!(base64(""), Some(Vec::new()));
    assert_eq!(base64("QQ=="), Some(b"A".to_vec()));
    assert_eq!(base64("QUI="), Some(b"AB".to_vec()));
    assert_eq!(base64("QUJD"), Some(b"ABC".to_vec()));
    assert_eq!(base64("QUJDRA"), Some(b"ABCD".to_vec()));
    assert_eq!(base64("+/+/"), Some(vec![0xfb, 0xff, 0xbf]));
    assert_eq!(base64("az09"), Some(vec![0x6b, 0x3d, 0x3d]));
    assert_eq!(base64("QQ==="), None);
    assert_eq!(base64("Q=Q="), None);
    assert_eq!(base64("QU I"), None);
    assert_eq!(base64("QU-_"), None);
}
