//! The tests of the lock under Linux. The test program of the crate (`kernsperre-probe`) installs the lock and tries the calls; the tests
//! read its lines. Nothing is installed in the test process itself: a filter cannot be taken back, and it would hold for every later test.
//!
//! What each test measures stands in `rust/spec/VERTRAEGE.md`, section 7. The counter-test without the lock belongs to every case:
//! a refusal is only evidence when the same call succeeds without the filter.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::process::Command;

const PROBE: &str = env!("CARGO_BIN_EXE_kernsperre-probe");

/// Runs the test program with these steps and gives its lines and its exit code. The program never waits, so there is no watchdog here.
fn probe(schritte: &[&str]) -> (Vec<String>, i32) {
    let aus = Command::new(PROBE)
        .args(schritte)
        .output()
        .expect("the probe starts");
    let zeilen = String::from_utf8_lossy(&aus.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    (zeilen, aus.status.code().unwrap_or(-1))
}

/// The line that starts with this word, without the word.
fn feld(zeilen: &[String], name: &str) -> String {
    zeilen
        .iter()
        .find(|z| z.split(' ').next() == Some(name))
        .unwrap_or_else(|| panic!("a line {name} in {zeilen:?}"))
        .clone()
}

#[test]
fn mit_der_sperre_wird_das_senken_der_grenze_mit_eperm_verweigert() {
    let (zeilen, code) = probe(&["eins", "sperre", "senken", "grenze"]);
    assert_eq!(zeilen[0], "eins rc=0", "{zeilen:?}");
    assert_eq!(zeilen[1], "sperre rc=0", "{zeilen:?}");
    assert_eq!(zeilen[2], "senken rc=-1 fehler=1", "{zeilen:?}");
    assert!(
        zeilen[3].starts_with("grenze weich=1 "),
        "the limit still stands: {zeilen:?}"
    );
    assert_eq!(code, 0, "{zeilen:?}");
}

#[test]
fn ohne_die_sperre_gelingt_das_senken_der_grenze() {
    let (zeilen, code) = probe(&["eins", "senken", "grenze"]);
    assert_eq!(zeilen[0], "eins rc=0", "{zeilen:?}");
    assert_eq!(zeilen[1], "senken rc=0", "{zeilen:?}");
    assert!(
        zeilen[2].starts_with("grenze weich=0 "),
        "the limit was lowered: {zeilen:?}"
    );
    assert_eq!(code, 0, "{zeilen:?}");
}

#[test]
fn mit_der_sperre_bleibt_das_lesen_erlaubt_und_eine_andere_ressource_frei() {
    let (zeilen, code) = probe(&["eins", "grenze", "sperre", "grenze", "fremd"]);
    // the second `grenze` runs under the lock; in rustix it is `prlimit64(0, RLIMIT_CORE, NULL, &out)`
    assert!(zeilen[1].starts_with("grenze weich=1 "), "{zeilen:?}");
    assert_eq!(
        zeilen[3], zeilen[1],
        "reading gives the same under the lock"
    );
    assert_eq!(zeilen[4], "fremd rc=0", "another resource stays free");
    assert_eq!(code, 0, "{zeilen:?}");
}

#[test]
fn die_sperre_gilt_im_kind_eines_kindes() {
    // `kind` starts the program itself with the rest of the steps: two of them give a child of a child, each through fork and exec
    let (zeilen, code) = probe(&["eins", "sperre", "kind", "kind", "senken"]);
    assert_eq!(zeilen[0], "eins rc=0", "{zeilen:?}");
    assert_eq!(zeilen[1], "sperre rc=0", "{zeilen:?}");
    assert_eq!(
        zeilen[2], "senken rc=-1 fehler=1",
        "the grandchild is refused as well: {zeilen:?}"
    );
    assert_eq!(zeilen[3], "kind rc=0", "{zeilen:?}");
    assert_eq!(zeilen[4], "kind rc=0", "{zeilen:?}");
    assert_eq!(code, 0, "{zeilen:?}");
}

#[test]
fn ohne_die_sperre_gelingt_es_im_kind_eines_kindes() {
    let (zeilen, _) = probe(&["eins", "kind", "kind", "senken"]);
    assert_eq!(zeilen[1], "senken rc=0", "{zeilen:?}");
}

#[test]
fn die_lage_des_prozesses_nennt_filter_und_no_new_privs() {
    let (zeilen, _) = probe(&["sperre", "lage"]);
    assert_eq!(feld(&zeilen, "lage"), "lage seccomp=2 nnp=1", "{zeilen:?}");
}

#[test]
fn ein_feld_das_die_lage_nicht_hat_liest_sich_als_minus_eins() {
    // the reader of `/proc/self/status` falls back to -1; without this test no case reaches that fallback
    let (zeilen, code) = probe(&["fehlfeld"]);
    assert_eq!(feld(&zeilen, "fehlfeld"), "fehlfeld wert=-1", "{zeilen:?}");
    assert_eq!(code, 0);
}

#[test]
fn ein_fremdes_programm_kann_die_grenze_unter_der_sperre_nicht_senken() {
    // the shell of the system is a caller that this crate does not know; `ulimit -c 0` lowers the soft limit from 1
    let (mit, _) = probe(&["eins", "sperre", "schale"]);
    let (ohne, _) = probe(&["eins", "schale"]);
    assert_eq!(feld(&ohne, "schale"), "schale rc=0", "{ohne:?}");
    assert_ne!(
        feld(&mit, "schale"),
        "schale rc=0",
        "the shell must not get through: {mit:?}"
    );
}

#[test]
fn ein_unbekannter_schritt_endet_mit_vierundsechzig() {
    let (zeilen, code) = probe(&["gibt-es-nicht"]);
    assert_eq!(zeilen[0], "unbekannt gibt-es-nicht");
    assert_eq!(code, 64);
}
