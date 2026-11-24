//! The calls of the program `einschluss-attrappe` that are refused, and its exit codes (feature `attrappe`).
#![cfg(feature = "attrappe")]

mod gemeinsam;

use std::process::Command;
use std::time::Duration;

use gemeinsam::{ATTRAPPE, lauf, testort};

fn aufruf(args: &[&str]) -> (Option<i32>, String, String) {
    let mut c = Command::new(ATTRAPPE);
    c.args(args).env_clear();
    lauf(&mut c, Duration::from_secs(10))
}

#[test]
fn a_wrong_call_ends_with_exit_code_2_and_a_usage_line() {
    for args in [
        &[][..],
        &["harness"][..],
        &["harness", "nichts"][..],
        &["such"][..],
        &["such", "/x"][..],
        &["such", "/x", "r", "zuviel"][..],
        &["weiter"][..],
        &["weiter", "/x"][..],
        &["weiter", "/x", "a", "zuviel"][..],
        &["cloud"][..],
        &["cloud", "/x"][..],
        &["cloud", "/x", "p"][..],
        &["cloud", "/x", "p", "m", "zuviel"][..],
        &["unbekannt"][..],
    ] {
        let (status, aus, fehler) = aufruf(args);
        assert_eq!(status, Some(2), "{args:?}");
        assert_eq!(aus, "", "{args:?}");
        assert!(
            fehler.starts_with("einschluss-attrappe: "),
            "{args:?}: {fehler}"
        );
    }
}

#[test]
fn a_probe_writes_its_line_and_exits_with_0() {
    let (status, aus, _) = aufruf(&["harness", "kennung"]);
    assert_eq!(status, Some(0));
    assert!(
        aus.starts_with("kennung: OK einschluss-attrappe-marker-"),
        "{aus}"
    );
}

/// A throw-away folder that goes away when the test ends, also when an assertion fails.
struct Weg(std::path::PathBuf);

impl Drop for Weg {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_taken_socket_path_ends_the_stand_ins_with_exit_code_1() {
    let ordner = testort().join(format!("ei-prog-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("folder");
    let _weg = Weg(ordner.clone());
    let pfad = ordner.join("belegt");
    std::fs::write(&pfad, "x").expect("file");
    let p = pfad.to_string_lossy().into_owned();
    let (status, _, fehler) = aufruf(&["such", &p, "r"]);
    assert_eq!(status, Some(1));
    assert!(fehler.contains("search:"), "{fehler}");
    let (status, _, fehler) = aufruf(&["weiter", &p, "127.0.0.1:1"]);
    assert_eq!(status, Some(1));
    assert!(fehler.contains("relay:"), "{fehler}");
}
