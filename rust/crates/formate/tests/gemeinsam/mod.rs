//! Helpers of the integration tests: a throw-away folder with a unique name, valid lines, and a check from text.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use formate::kette::{Gesperrt, NULL_HASH, Pruefung, pruefe, sha256_hex, zeile_gesperrt};
use serde_json::Value;

/// A folder below the system's temp folder, removed again when the value is dropped.
pub struct Wegwerf(PathBuf);

impl Wegwerf {
    /// A new empty folder; the name holds the process id, a counter and `name`, so that no two tests share one.
    pub fn neu(name: &str) -> Self {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let nummer = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let pfad = std::env::temp_dir().join(format!(
            "formate-test-{}-{nummer}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("create the test folder");
        Self(pfad)
    }

    pub fn pfad(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn ordner(&self) -> &Path {
        &self.0
    }

    /// Writes a file and returns its path.
    pub fn schreibe(&self, name: &str, inhalt: &[u8]) -> PathBuf {
        let pfad = self.pfad(name);
        fs::write(&pfad, inhalt).expect("write the test file");
        pfad
    }
}

impl Drop for Wegwerf {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn muster() -> Gesperrt {
    Gesperrt {
        zeit_ms: 1_795_179_600_000,
        route: "cloud-beispiel".to_owned(),
        sitzung: "s-0001".to_owned(),
        grund: "kundendaten-an-cloud".to_owned(),
        klasse: Some("kundendaten".to_owned()),
        kunde: Some("k-beispiel".to_owned()),
        anzahl: 1,
    }
}

/// `n` valid lines of one chain, the first with 64 zeros.
pub fn zeilen(n: u64) -> Vec<String> {
    let mut aus: Vec<String> = Vec::new();
    let mut vorher = NULL_HASH.to_owned();
    for i in 0..n {
        let mut g = muster();
        g.anzahl = u32::try_from(i + 1).expect("small");
        g.zeit_ms += i64::try_from(i).expect("small") * 1000;
        let zeile = zeile_gesperrt(&g, &vorher).expect("valid line");
        vorher = sha256_hex(zeile.as_bytes());
        aus.push(zeile);
    }
    aus
}

/// The lines, each with a line feed.
pub fn als_datei(zeilen: &[String]) -> String {
    let mut aus = String::new();
    for zeile in zeilen {
        aus.push_str(zeile);
        aus.push('\n');
    }
    aus
}

/// Checks one file with the given content from the start of a chain; no `letzte`.
pub fn pruefe_datei(ordner: &Wegwerf, name: &str, inhalt: &[u8]) -> Pruefung {
    let pfad = ordner.schreibe(name, inhalt);
    pruefe(&[pfad], NULL_HASH, None)
}

/// The break of a check as `(file, reason, line)`.
pub fn bruch_von(p: &Pruefung) -> Option<(String, String, Option<u64>)> {
    p.bruch
        .as_ref()
        .map(|b| (b.datei.clone(), b.grund.clone(), b.zeile))
}

/// A file of the visible cases (`rust/spec/protokollzeile/faelle/`), read as JSON.
pub fn fall_datei(name: &str) -> Value {
    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/protokollzeile/faelle")
        .join(name);
    let text = fs::read_to_string(&pfad).expect("case file is readable");
    serde_json::from_str(&text).expect("case file is JSON")
}

pub fn liste<'a>(wert: &'a Value, schluessel: &str) -> &'a [Value] {
    wert[schluessel].as_array().expect("a list")
}

pub fn text<'a>(wert: &'a Value, schluessel: &str) -> &'a str {
    wert[schluessel].as_str().expect("a text")
}

/// The data of a line `ausgabe.gesperrt` from the input of a case.
pub fn gesperrt_aus(eingabe: &Value) -> Gesperrt {
    let option = |schluessel: &str| eingabe[schluessel].as_str().map(str::to_owned);
    Gesperrt {
        zeit_ms: eingabe["zeit_ms"].as_i64().expect("zeit_ms"),
        route: text(eingabe, "route").to_owned(),
        sitzung: text(eingabe, "sitzung").to_owned(),
        grund: text(eingabe, "grund").to_owned(),
        klasse: option("klasse"),
        kunde: option("kunde"),
        anzahl: u32::try_from(eingabe["anzahl"].as_u64().expect("anzahl")).expect("u32"),
    }
}

/// The bytes of a standard base64 text.
pub fn base64(eingabe: &str) -> Vec<u8> {
    let wert = |c: u8| match c {
        b'A'..=b'Z' => u32::from(c - b'A'),
        b'a'..=b'z' => u32::from(c - b'a') + 26,
        b'0'..=b'9' => u32::from(c - b'0') + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {c}"),
    };
    let rein: Vec<u8> = eingabe.bytes().filter(|c| *c != b'=').collect();
    let mut aus = Vec::new();
    for gruppe in rein.chunks(4) {
        let mut summe = 0u32;
        for (i, c) in gruppe.iter().enumerate() {
            summe |= wert(*c) << (18 - 6 * i);
        }
        let bytes = summe.to_be_bytes();
        aus.extend_from_slice(&bytes[1..gruppe.len()]);
    }
    aus
}
