//! The observation of the program in the checks of the network sockets (Kernabbild, step 2a). The program shields its memory from core images
//! (`PR_SET_DUMPABLE 0`) and is then invisible to `lsof` of the same user under Linux; the checks therefore start it in its own user
//! namespace without capabilities and begin with a positive control: `lsof` must show the Unix socket of the program, else the check is red.
//!
//! Counter-checks (each must be red): a program with an additional TCP socket; a program that `lsof` does not see (Linux: started without the
//! namespace, the blind setup). The same through the Python check client of the contract (`tests/werkzeug`, the runner `test_aussen.py`).
//! The tests need `python3`, `lsof` and, under Linux, `unshare` and `setpriv`; without them they are red, not skipped.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![cfg(feature = "pruefbau")] // the fixed child programs `--kind` exist in the test build only; `naht.rs` runs in both builds

mod gemeinsam;

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gemeinsam::{DIENST, Prozess, Wegwerf, beobachtbar, netz_sockets_pruefen, startdatei};
use serde_json::json;

/// A program that opens a TCP socket on 127.0.0.1 and then becomes `wb-dienst` (the socket stays open in the same process).
fn tcp_dienst(w: &Wegwerf) -> String {
    let pfad = w.0.join("tcp_dienst.py");
    let text = format!(
        "#!/usr/bin/env python3\nimport os, socket, sys\ns = socket.socket()\ns.bind((\"127.0.0.1\", 0))\ns.listen(1)\nos.set_inheritable(s.fileno(), True)\nos.execv({DIENST:?}, [{DIENST:?}] + sys.argv[1:])\n"
    );
    std::fs::write(&pfad, text).expect("write the wrapper");
    std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    pfad.to_string_lossy().into_owned()
}

fn start(w: &Wegwerf) -> serde_json::Value {
    startdatei(w, &["/bin/sleep", "60"], &json!({}))
}

#[test]
fn der_dienst_ist_fuer_lsof_sichtbar_und_hat_keinen_netz_socket() {
    let w = Wegwerf::neu("beob-gut");
    let p = Prozess::starten_beobachtbar(&w, &start(&w));
    assert_eq!(netz_sockets_pruefen(p.kind.id(), &w.socket()), Ok(()));
}

#[test]
fn ein_zusaetzlicher_tcp_socket_macht_die_pruefung_rot() {
    let w = Wegwerf::neu("beob-tcp");
    let mut befehl = beobachtbar();
    befehl.push(tcp_dienst(&w));
    let p = Prozess::starten_mit(&w, &start(&w), &befehl);
    let ergebnis = netz_sockets_pruefen(p.kind.id(), &w.socket());
    let grund = ergebnis.expect_err("a TCP socket must turn the check red");
    assert!(grund.starts_with("network sockets: "), "{grund}");
    assert!(grund.contains("127.0.0.1"), "{grund}");
}

#[cfg(target_os = "linux")]
#[test]
fn ohne_namensraum_ist_der_dienst_unsichtbar_und_die_positivkontrolle_ist_rot() {
    let w = Wegwerf::neu("beob-blind");
    let p = Prozess::starten(&w, &start(&w));
    let grund = netz_sockets_pruefen(p.kind.id(), &w.socket())
        .expect_err("a program that lsof does not see must turn the check red");
    assert!(grund.starts_with("positive control: "), "{grund}");
}

/// Runs the runner of the contract on the case `di1_start_gut` with the program `bin` and the extra environment; returns exit code and text.
fn runner(bin: &str, extra: &[(&str, &str)], name: &str) -> (Option<i32>, String) {
    let tests = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let werkzeug = tests.join("werkzeug");
    // below `/tmp` like `faelle.rs`: the socket path the check client builds under it must stay within the 100 bytes of the service;
    // the default TMPDIR of macOS (/var/folders/...) makes it about 108 bytes, and the service refuses the start with exit 2
    let basis = Path::new("/tmp").join(format!("dienst-beob-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&basis);
    std::fs::create_dir_all(&basis).expect("folder");
    std::fs::set_permissions(&basis, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    let mut befehl = Command::new("python3");
    befehl
        .args(["-I", "-B", "-u"])
        .arg(werkzeug.join("test_aussen.py"))
        .arg("di1_start_gut")
        .env("DI_BIN", bin)
        .env("DI_FAELLE", &tests)
        .env("DI_REF_DIR", werkzeug.join("referenz"))
        .env("DI_TMP", &basis)
        .env("DI_ZEIT", "60")
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
    let _ = std::fs::remove_dir_all(&basis);
    (status.code(), text)
}

#[test]
fn der_pruefclient_ist_gruen_mit_dem_dienst_und_rot_mit_einem_tcp_socket() {
    let (code, text) = runner(DIENST, &[], "gut");
    assert_eq!(code, Some(0), "{text}");
    let w = Wegwerf::neu("beob-client");
    let (code, text) = runner(&tcp_dienst(&w), &[], "tcp");
    assert_ne!(code, Some(0), "{text}");
    assert!(text.contains("ROT"), "{text}");
}

#[cfg(target_os = "linux")]
#[test]
fn der_pruefclient_ist_ohne_namensraum_rot_an_der_positivkontrolle() {
    let (code, text) = runner(DIENST, &[("DI_NS", "aus")], "blind");
    assert_ne!(code, Some(0), "{text}");
    assert!(text.contains("ROT"), "{text}");
}
