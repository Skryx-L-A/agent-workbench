//! Stage 0 of the start (contract 8, sections 3 and 4, F1): every case of `faelle/umgebung.json` as a real start of the
//! program with exactly the environment of the case. A refused start writes the fixed line, nothing on stdout, and
//! touches no file; a passing stage 0 goes on to step 1 (here the incomplete call, `mw.aufruf`).
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use gemeinsam::{SCHLUESSEL, Wegwerf, argv, fall_datei, programm, text};
use serde_json::Value;

/// Runs the program with an empty environment plus `umgebung`; returns exit code, stdout and stderr.
fn lauf(args: &[OsString], umgebung: &[(String, String)]) -> (i32, Vec<u8>, Vec<u8>) {
    lauf_als(None, args, umgebung)
}

/// [`lauf`] with `name` as argv\[0\] where it is given (section 3a, duty 3: the mark of the image after the emptying).
fn lauf_als(
    name: Option<&str>,
    args: &[OsString],
    umgebung: &[(String, String)],
) -> (i32, Vec<u8>, Vec<u8>) {
    let mut cmd = Command::new(programm());
    if let Some(n) = name {
        cmd.arg0(n);
    }
    cmd.args(args).env_clear().stdin(Stdio::null());
    for (k, v) in umgebung {
        cmd.env(k, v);
    }
    let aus = cmd.output().expect("start the program");
    (
        aus.status.code().expect("exit code"),
        aus.stdout,
        aus.stderr,
    )
}

fn umgebung(fall: &Value) -> Vec<(String, String)> {
    fall["umgebung"]
        .as_object()
        .expect("environment")
        .iter()
        .map(|(k, v)| (k.clone(), text(v).to_owned()))
        .collect()
}

/// The line of step 1 when stage 0 passes and the call holds only `--betriebsart <art>`.
fn schritt_1(art: &str) -> &'static [u8] {
    if art == "kopf-einsetzen" && !cfg!(target_os = "linux") {
        b"mw.betriebsart_unbekannt: Betriebsart unbekannt oder nicht gebaut\n"
    } else {
        b"mw.aufruf: Aufruf ungueltig\n"
    }
}

#[test]
fn umgebung_cases() {
    let f = fall_datei("umgebung.json");
    let faelle = f["faelle"].as_array().expect("cases");
    assert_eq!(faelle.len(), 34);
    let mit_marke = faelle.iter().filter(|f| f.get("argv0").is_some()).count();
    assert_eq!(mit_marke, 10);
    for fall in faelle {
        let name = text(&fall["name"]);
        let art = text(&fall["betriebsart"]);
        let args: Vec<OsString> = match fall["argv"].as_array() {
            Some(a) => a.iter().map(|x| OsString::from(text(x))).collect(),
            None => vec!["--betriebsart".into(), art.into()],
        };
        let env = umgebung(fall);
        let name0 = fall
            .get("argv0")
            .map(|n| text(n).replace("<ELTERN>", &std::process::id().to_string()));
        let (exit, out, err) = lauf_als(name0.as_deref(), &args, &env);
        // duty 3 of section 3a holds under Linux only; a case with argv0 says what it expects there
        let e = match fall.get("erwartet_linux") {
            Some(linux) if cfg!(target_os = "linux") => linux,
            _ => &fall["erwartet"],
        };
        if e["bis_schritt"] == "umgebung_gut" {
            assert_eq!(exit, 64, "{name}");
            assert_eq!(err, schritt_1(art), "{name}");
            continue;
        }
        assert_eq!(i64::from(exit), e["exit"].as_i64().expect("exit"), "{name}");
        assert_eq!(err, text(&e["stderr"]).as_bytes(), "{name}");
        assert_eq!(out, text(&e["stdout"]).as_bytes(), "{name}");
        for (k, v) in &env {
            assert!(
                !gemeinsam::enthaelt(&err, k.as_bytes()),
                "{name}: name in stderr"
            );
            assert!(
                v.is_empty() || !gemeinsam::enthaelt(&err, v.as_bytes()),
                "{name}: value in stderr"
            );
        }
    }
}

/// Stage 0 comes before every file: with a valid call and the variable set, the key file is not read (it would be
/// refused), the protocol folder stays empty and no socket appears.
#[test]
fn stage_0_comes_before_every_file_and_the_socket() {
    for n in ["SSL_CERT_FILE", "SSL_CERT_DIR", "OPENSSL_CONF"] {
        let w = Wegwerf::neu("umgebung");
        let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
        fs::set_permissions(&schluessel, fs::Permissions::from_mode(0o644)).expect("chmod");
        let socket = w.pfad("m.sock");
        let a = argv(
            &socket,
            "http://127.0.0.1:9/v1",
            &schluessel,
            &w.protokoll(),
        );
        let (exit, out, err) = lauf(&a, &[(n.to_owned(), String::new())]);
        assert_eq!(
            (exit, out.as_slice(), err.as_slice()),
            (
                78,
                &b""[..],
                &b"mw.umgebung_unsicher: Umgebung unsicher\n"[..]
            ),
            "{n}"
        );
        assert!(!socket.exists(), "{n}");
        assert_eq!(
            fs::read_dir(w.protokoll()).expect("folder").count(),
            0,
            "{n}"
        );
        let (exit, _, err) = lauf(&a, &[]);
        assert_eq!(
            (exit, err.as_slice()),
            (
                78,
                &b"mw.schluessel_datei: Schluesseldatei nicht eigen, nicht 0600 oder keine Datei\n"
                    [..]
            ),
            "{n}: without the variable the start reaches the key file"
        );
    }
}
