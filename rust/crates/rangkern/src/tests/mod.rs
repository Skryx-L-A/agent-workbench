//! Unit tests. They pin the behaviour of the Python model: the expected values in `tests/data`
//! come from the model itself (`tools/gen_unit_data.py`), the other cases are exact values.

// Style lints that would only make the assertions of the tests longer: `assert!(x.is_empty())` and
// a comparison of floats that are exact by construction (Python's `repr` round-trips them).
#![allow(clippy::assert_is_empty, clippy::manual_assert_eq, clippy::float_cmp)]

mod abschirmung;
mod bm25;
mod cli;
mod embed;
mod fulltext;
mod hit;
mod notes;
mod pystr;
mod search;
mod serve;
#[cfg(unix)]
mod sitzung_anfrage;
#[cfg(unix)]
mod sitzung_einst;
#[cfg(unix)]
mod sitzung_konfig;
#[cfg(unix)]
mod sitzung_loeschen;
#[cfg(unix)]
mod sitzung_mac;
#[cfg(unix)]
mod sitzung_probelauf;
#[cfg(unix)]
mod sitzung_tor;
#[cfg(windows)]
mod sitzung_windows;
mod snippet;
mod stamp;
mod stem;
mod token;
mod validity;
mod vault;
mod vdb;
mod warm;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

/// A temporary directory that is removed when it goes out of scope.
pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("rangkern-unit-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("temporary directory is created");
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// Writes a file below the directory, creating parents, and returns its path.
    pub(crate) fn write(&self, rel: &str, content: &[u8]) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().expect("file has a parent")).expect("parent is created");
        fs::write(&path, content).expect("file is written");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs `work` in a thread and gives it up after ten seconds. A loop that does not end because of
/// a bug then fails the test instead of hanging it.
pub(crate) fn guarded<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(work());
    });
    receive
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the work ends in time")
}

/// A data file of `tests/data` as JSON.
pub(crate) fn data(name: &str) -> Value {
    let text = match name {
        "bm25" => include_str!("../../tests/data/bm25.json"),
        "chars" => include_str!("../../tests/data/chars.json"),
        "cosine" => include_str!("../../tests/data/cosine.json"),
        "fuse" => include_str!("../../tests/data/fuse.json"),
        "links" => include_str!("../../tests/data/links.json"),
        "lowermap" => include_str!("../../tests/data/lowermap.json"),
        "lower" => include_str!("../../tests/data/lower.json"),
        "snippets" => include_str!("../../tests/data/snippets.json"),
        "sums" => include_str!("../../tests/data/sums.json"),
        "tokens" => include_str!("../../tests/data/tokens.json"),
        "validity" => include_str!("../../tests/data/validity.json"),
        other => panic!("no data file {other}"),
    };
    serde_json::from_str(text).expect("data file is JSON")
}

/// A float written as text by Python's `repr`.
pub(crate) fn float(value: &Value) -> f64 {
    value
        .as_str()
        .expect("float is text")
        .parse()
        .expect("float parses")
}

/// The comparison of two floats against the data files: the same bits, no tolerance. The files
/// come from Python, and `float_roundtrip` makes the program read numbers as Python does.
pub(crate) fn identical(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

/// A list of JSON texts as strings.
pub(crate) fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("list")
        .iter()
        .map(|item| item.as_str().expect("text").to_string())
        .collect()
}

/// Decimal texts of 17 digits made up for this test, with the bits of the `f64` that Python's `float()` reads
/// from each text (`struct.pack(">d", float(text)).hex()`). Without the feature `float_roundtrip` of `serde_json`
/// every one of them is read one ULP away. They are no numbers of a real database.
pub(crate) const ROUNDING_CASES: [(&str, u64); 8] = [
    ("0.0015667258415858435", 0x3f59_ab53_1017_16ed),
    ("0.11945560216203431", 0x3fbe_94a4_709c_23ff),
    ("-0.0072351154082460267", 0xbf7d_a291_80fa_af57),
    ("0.010908590668029123", 0x3f86_573e_4151_2c9d),
    ("0.0038431740859626163", 0x3f6f_7bb8_6062_aa5f),
    ("-0.020083013003700946", 0xbf94_90a4_3039_621a),
    ("0.00079283916361580086", 0x3f49_fad1_23ad_06da),
    ("-0.0014242812491837235", 0xbf57_55de_58ac_da68),
];
