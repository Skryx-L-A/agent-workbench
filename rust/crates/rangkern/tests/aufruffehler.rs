//! The built program: every usage error ends with exit 64 (`EX_USAGE` of `sysexits.h`), writes the usage text to the
//! error output and nothing to the standard output. One code for the whole program since 23.11.2026.

use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_rangkern");

fn laufen<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> std::process::Output {
    let tmp = std::env::temp_dir();
    Command::new(BIN)
        .args(args)
        .current_dir(&tmp)
        .env("HOME", &tmp)
        .stdin(Stdio::null())
        .output()
        .expect("program runs")
}

#[test]
fn every_usage_error_of_the_program_is_exit_64_without_output() {
    let faelle: [&[&str]; 6] = [
        &[],
        &["zerlege", "x"],
        &["search", "vault"],
        &["serve"],
        &["tokens"],
        // `serve` with the options of `sitzung`: they are unknown options of `serve`
        &["serve", "vault", "--route", "cloud", "--sitzung", "s-1"],
    ];
    for args in faelle {
        let ende = laufen(args);
        assert_eq!(ende.status.code(), Some(64), "{args:?}");
        assert!(ende.stdout.is_empty(), "{args:?}");
        let fehler = String::from_utf8_lossy(&ende.stderr);
        assert!(fehler.starts_with("usage: rangkern "), "{args:?}: {fehler}");
    }
}

#[cfg(unix)]
#[test]
fn an_argument_that_is_no_utf8_is_exit_64_without_output() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    // `serve` and `sitzung` take the step against core images first (contract 4); it passes on this host, then the usage error.
    for befehl in ["tokens", "serve", "sitzung"] {
        let ende = laufen(&[OsStr::new(befehl), OsStr::from_bytes(b"\xff")]);
        assert_eq!(ende.status.code(), Some(64), "{befehl}");
        assert_eq!(ende.stdout, b"", "stdout of {befehl}");
    }
}
