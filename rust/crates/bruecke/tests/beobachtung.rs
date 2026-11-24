//! The observation of the bridge in the check of its listener (Kernabbild, step 2a). The bridge shields its memory from core images
//! (`PR_SET_DUMPABLE 0`) and is then invisible to `lsof` of the same user under Linux (the check client would see no listener and be red).
//! The check client therefore starts the bridge in its own user namespace without capabilities and begins with a positive control: `lsof` must
//! show the listening TCP socket of the bridge, else the run ends red.
//!
//! The runner of the contract (`tests/aussen/test_aussen.py`) runs the case `br_401_starten` (it measures the listener): green with the bridge;
//! red with a program that listens on a second port (counter-check a); red where `lsof` does not see the bridge (Linux: `BR_NS=aus`, the blind
//! setup; counter-check b). Needs `python3`, `lsof` and, under Linux, `unshare` and `setpriv`; without them it is red, not skipped.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PROGRAMM: &str = env!("CARGO_BIN_EXE_wb-bruecke");

fn ordner() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/aussen")
}

/// A program that opens a second TCP socket on 127.0.0.1 and then becomes `wb-bruecke` (the socket stays open in the same process).
fn tcp_bruecke(basis: &Path) -> String {
    let pfad = basis.join("tcp_bruecke.py");
    let text = format!(
        "#!/usr/bin/env python3\nimport os, socket, sys\ns = socket.socket()\ns.bind((\"127.0.0.1\", 0))\ns.listen(1)\nos.set_inheritable(s.fileno(), True)\nos.execv({PROGRAMM:?}, [{PROGRAMM:?}] + sys.argv[1:])\n"
    );
    std::fs::write(&pfad, text).expect("write the wrapper");
    std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    pfad.to_string_lossy().into_owned()
}

/// Runs the runner of the contract on the case `br_401_starten` with the program `bin`; returns exit code and text.
fn runner(bin: &str, extra: &[(&str, &str)], basis: &Path) -> (Option<i32>, String) {
    let mut befehl = Command::new("python3");
    befehl
        .args(["-I", "-B", "-u", "test_aussen.py", "br_401_starten"])
        .current_dir(ordner())
        .env("BR_BIN", bin)
        .env("BR_FAELLE", ordner().join("aussentests"))
        .env("BR_TMP", basis)
        .env("BR_ZEIT", "120")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        befehl.env(k, v);
    }
    let mut kind = befehl
        .spawn()
        .expect("python3 starts (it is needed for this test)");
    let mut text = String::new();
    if let Some(aus) = kind.stdout.take() {
        for zeile in BufReader::new(aus).lines().map_while(Result::ok) {
            text.push_str(&zeile);
            text.push('\n');
        }
    }
    let status = kind.wait().expect("runner ends");
    (status.code(), text)
}

fn basis(name: &str) -> PathBuf {
    let b = PathBuf::from("/tmp").join(format!("wbbr-beob-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&b);
    std::fs::create_dir_all(&b).expect("folder");
    b
}

#[test]
fn der_pruefclient_ist_gruen_mit_der_bruecke_und_rot_mit_einem_zweiten_tcp_socket() {
    let b = basis("gut");
    let (code, text) = runner(PROGRAMM, &[], &b);
    assert_eq!(code, Some(0), "{text}");
    let b2 = basis("tcp");
    let (code, text) = runner(&tcp_bruecke(&b2), &[], &b2);
    assert_ne!(code, Some(0), "{text}");
    assert!(text.contains("ROT"), "{text}");
    let _ = std::fs::remove_dir_all(&b);
    let _ = std::fs::remove_dir_all(&b2);
}

#[cfg(target_os = "linux")]
#[test]
fn der_pruefclient_ist_ohne_namensraum_rot_an_der_positivkontrolle() {
    let b = basis("blind");
    let (code, text) = runner(PROGRAMM, &[("BR_NS", "aus")], &b);
    assert_ne!(code, Some(0), "{text}");
    assert!(text.contains("ROT"), "{text}");
    let _ = std::fs::remove_dir_all(&b);
}
