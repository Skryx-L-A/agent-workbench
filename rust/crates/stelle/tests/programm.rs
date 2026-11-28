//! The program `werkbank-stelle`: the call, and the start that ends with the fixed reason and exit code of section 13
//! (64 for the call, 78 for the configuration), one line on stderr, nothing on stdout.
#![cfg(unix)] // the shell is Unix only; under Windows the program ends with `plattform_gesperrt`

use std::process::Command;

fn lauf(args: &[&str]) -> (Option<i32>, String, String) {
    let aus = Command::new(env!("CARGO_BIN_EXE_werkbank-stelle"))
        .args(args)
        .output()
        .expect("runs");
    (
        aus.status.code(),
        String::from_utf8_lossy(&aus.stdout).into_owned(),
        String::from_utf8_lossy(&aus.stderr).into_owned(),
    )
}

#[test]
fn der_aufruf_ist_genau_konfig_und_ein_pfad() {
    for args in [
        &[][..],
        &["--konfig"][..],
        &["--andere", "/srv/beispiel/stelle.json"][..],
        &["--konfig", "/srv/beispiel/stelle.json", "x"][..],
    ] {
        assert_eq!(
            lauf(args),
            (
                Some(64),
                String::new(),
                "werkbank-stelle: aufruf\n".to_owned()
            ),
            "{args:?}"
        );
    }
}

#[test]
fn ohne_lesbare_konfiguration_startet_die_stelle_nicht() {
    assert_eq!(
        lauf(&["--konfig", "/nicht/da/stelle.json"]),
        (
            Some(78),
            String::new(),
            "werkbank-stelle: konfig_unlesbar\n".to_owned()
        )
    );
}
