//! The program as a whole: exit codes, stdout carries only protocol, and the promises of the contract:
//! it reads no environment variable, writes no file and opens no network connection.

#![cfg(unix)]

mod gemeinsam;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use gemeinsam::{StandIn, Wegwerf};
use serde_json::Value;

const PROGRAMM: &str = env!("CARGO_BIN_EXE_such-mcp");

fn lauf(args: &[&str], eingabe: &[u8], arbeitsordner: &Path, umgebung: &[(&str, &str)]) -> Output {
    let mut kind = Command::new(PROGRAMM)
        .args(args)
        .current_dir(arbeitsordner)
        .env_clear()
        .envs(umgebung.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the program");
    let mut stdin = kind.stdin.take().expect("stdin");
    let eingabe = eingabe.to_vec();
    let schreiber = std::thread::spawn(move || {
        let _ = stdin.write_all(&eingabe);
    });
    let ausgang = kind.wait_with_output().expect("wait");
    schreiber.join().expect("writer");
    ausgang
}

fn zeilen(ausgang: &Output) -> Vec<Value> {
    String::from_utf8(ausgang.stdout.clone())
        .expect("UTF-8")
        .lines()
        .map(|z| serde_json::from_str(z).expect("every stdout line is JSON"))
        .collect()
}

#[test]
fn a_wrong_call_exits_with_64_and_writes_nothing() {
    let wegwerf = Wegwerf::neu("aufruf");
    for args in [
        &["--route", "lokal"][..],
        &["--socket"],
        &["--socket", "a", "b"],
        &["--socket", "a", "--socket", "b"],
        &["--socket", ""],
        &["--socket=a"],
        &["-s", "a"],
        &["--help"],
        &["--version"],
        &["x"],
    ] {
        let ausgang = lauf(
            args,
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n",
            wegwerf.ordner(),
            &[],
        );
        assert_eq!(ausgang.status.code(), Some(64), "{args:?}");
        assert!(ausgang.stdout.is_empty(), "{args:?}");
        assert!(ausgang.stderr.is_empty(), "{args:?}");
    }
}

/// A read error of stdin (a folder as stdin: `read` gives `EISDIR`) ends the program with 74 (`EX_IOERR`), writes nothing and
/// leaves no message on standard output.
#[test]
fn a_read_error_of_stdin_exits_with_74() {
    let wegwerf = Wegwerf::neu("lesefehler");
    let pfad = wegwerf.pfad("s.sock");
    let ausgang = Command::new(PROGRAMM)
        .args(["--socket", pfad.to_str().expect("UTF-8")])
        .current_dir(wegwerf.ordner())
        .env_clear()
        .stdin(fs::File::open(wegwerf.ordner()).expect("a folder opens for reading"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run");
    assert_eq!(ausgang.status.code(), Some(74));
    assert_eq!(ausgang.stdout, b"", "nothing on standard output");
}

/// A write error of stdout (`/dev/full` refuses every write with `ENOSPC`) ends the program with 74.
#[cfg(target_os = "linux")]
#[test]
fn a_write_error_of_stdout_exits_with_74() {
    let wegwerf = Wegwerf::neu("schreibfehler");
    let pfad = wegwerf.pfad("s.sock");
    let voll = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full opens");
    let mut kind = Command::new(PROGRAMM)
        .args(["--socket", pfad.to_str().expect("UTF-8")])
        .current_dir(wegwerf.ordner())
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(voll)
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the program");
    let mut stdin = kind.stdin.take().expect("stdin");
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .expect("request");
    // stdin is closed before the wait: a program that ignored the write error would end at the end of stdin with 0 (red)
    // instead of waiting for more input for ever (review B13).
    drop(stdin);
    let ausgang = kind.wait_with_output().expect("wait");
    assert_eq!(ausgang.status.code(), Some(74));
}

const LEERE_ZEILEN: &[u8] = b"\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n\n[]\n\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n";

fn leere_zeilen_pruefen(ausgang: &Output) {
    assert_eq!(ausgang.status.code(), Some(0));
    let antworten = zeilen(ausgang);
    let ids: Vec<&Value> = antworten.iter().map(|a| &a["id"]).collect();
    assert_eq!(ids, [&Value::from(1), &Value::Null, &Value::from(2)]);
    assert_eq!(antworten[1]["error"]["code"], -32600);
}

/// The macOS counterpart of the call without an argument (addendum to contract 4, section 6): the option is required,
/// absolute and at most 103 bytes long; every other call is exit 64 with nothing written.
#[cfg(target_os = "macos")]
#[test]
fn under_macos_a_call_without_the_socket_option_exits_with_64_and_writes_nothing() {
    let wegwerf = Wegwerf::neu("mac-aufruf");
    let zu_lang = format!("/tmp/{}", "a".repeat(99));
    assert_eq!(zu_lang.len(), 104);
    for args in [
        &[][..],
        &["--socket", "such.sock"],
        &["--socket", "./such.sock"],
        &["--socket", zu_lang.as_str()],
    ] {
        let ausgang = lauf(
            args,
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n",
            wegwerf.ordner(),
            &[],
        );
        assert_eq!(ausgang.status.code(), Some(64), "{args:?}");
        assert!(ausgang.stdout.is_empty(), "{args:?}");
        assert!(ausgang.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn empty_lines_on_stdin_are_skipped_without_an_answer() {
    let wegwerf = Wegwerf::neu("leer");
    let pfad = wegwerf.pfad("s.sock");
    let ausgang = lauf(
        &["--socket", pfad.to_str().expect("UTF-8")],
        LEERE_ZEILEN,
        wegwerf.ordner(),
        &[],
    );
    leere_zeilen_pruefen(&ausgang);
}

/// The call without an argument (the fixed path of the enclosure) exists under Linux only; under macOS it is a usage error
/// (see `under_macos_a_call_without_the_socket_option_exits_with_64_and_writes_nothing`).
#[cfg(target_os = "linux")]
#[test]
fn empty_lines_on_stdin_are_skipped_without_an_answer_with_the_path_of_the_enclosure() {
    let wegwerf = Wegwerf::neu("leer-fest");
    let ausgang = lauf(&[], LEERE_ZEILEN, wegwerf.ordner(), &[]);
    leere_zeilen_pruefen(&ausgang);
}

#[test]
fn the_end_of_stdin_ends_the_program_with_exit_0() {
    let wegwerf = Wegwerf::neu("ende");
    let pfad = wegwerf.pfad("s.sock");
    let pfad = pfad.to_str().expect("UTF-8");
    let faelle = [
        (vec!["--socket", pfad], &b""[..]),
        (
            vec!["--socket", pfad],
            &b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}"[..],
        ),
    ]
    .into_iter()
    // Without an argument the fixed path of the enclosure is used: under Linux only (under macOS it is a usage error).
    // `cfg!` instead of `#[cfg]` on a `push`: the list needs no `mut` that only one system uses (clippy on the Mac).
    .chain(cfg!(target_os = "linux").then(|| (vec![], &b""[..])));
    for (args, eingabe) in faelle {
        let ausgang = lauf(&args, eingabe, wegwerf.ordner(), &[]);
        assert_eq!(ausgang.status.code(), Some(0), "{args:?}");
    }
    let ausgang = lauf(
        &["--socket", pfad],
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}",
        wegwerf.ordner(),
        &[],
    );
    assert_eq!(zeilen(&ausgang).len(), 1);
}

#[test]
fn stdout_carries_only_protocol_lines() {
    let wegwerf = Wegwerf::neu("protokoll");
    let pfad = wegwerf.pfad("s.sock");
    let _stand_in = StandIn::start(&pfad, |_, mut strom| {
        let _ = strom.write_all(b"{\"hits\":[],\"gesperrt\":0}\n");
    });
    let eingabe = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"brain_suche\",\"arguments\":{\"query\":\"x\"}}}\n",
        "kein json\n",
        "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"ping\"}\n",
    );
    let ausgang = lauf(
        &["--socket", pfad.to_str().expect("UTF-8")],
        eingabe.as_bytes(),
        wegwerf.ordner(),
        &[],
    );
    assert_eq!(ausgang.status.code(), Some(0));
    let antworten = zeilen(&ausgang);
    let ids: Vec<&Value> = antworten.iter().map(|a| &a["id"]).collect();
    assert_eq!(
        ids,
        [
            &Value::from(1),
            &Value::from(2),
            &Value::from(3),
            &Value::Null,
            &Value::from(4)
        ]
    );
    assert!(antworten.iter().all(|a| a["jsonrpc"] == "2.0"));
    assert_eq!(
        antworten[2]["result"]["content"][0]["text"],
        r#"{"hits":[],"gesperrt":0}"#
    );
}

#[test]
fn a_line_over_the_limit_is_dropped_and_the_program_goes_on() {
    let wegwerf = Wegwerf::neu("grenze");
    let pfad = wegwerf.pfad("s.sock");
    let mut eingabe = vec![b'x'; 8 * 1024 * 1024 + 1];
    eingabe.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"ping\"}\n");
    let ausgang = lauf(
        &["--socket", pfad.to_str().expect("UTF-8")],
        &eingabe,
        wegwerf.ordner(),
        &[],
    );
    assert_eq!(ausgang.status.code(), Some(0));
    let antworten = zeilen(&ausgang);
    assert_eq!(antworten.len(), 2);
    assert_eq!(antworten[0]["error"]["code"], -32600);
    assert_eq!(antworten[1]["id"], 5);
}

#[test]
fn a_line_of_exactly_the_limit_is_read() {
    let wegwerf = Wegwerf::neu("genau");
    let pfad = wegwerf.pfad("s.sock");
    let json = b"{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"ping\"}";
    let mut eingabe = vec![b' '; 8 * 1024 * 1024 - json.len()];
    eingabe.extend_from_slice(json);
    assert_eq!(eingabe.len(), 8 * 1024 * 1024);
    eingabe.push(b'\n');
    let ausgang = lauf(
        &["--socket", pfad.to_str().expect("UTF-8")],
        &eingabe,
        wegwerf.ordner(),
        &[],
    );
    let antworten = zeilen(&ausgang);
    assert_eq!(antworten.len(), 1);
    assert_eq!(antworten[0]["id"], 6);
}

#[test]
fn the_environment_changes_nothing_and_no_file_is_written() {
    let wegwerf = Wegwerf::neu("umgebung");
    let a = wegwerf.pfad("a.sock");
    let b = wegwerf.pfad("b.sock");
    let stand_in_a = StandIn::start(&a, |_, mut strom| {
        let _ = strom.write_all(b"{\"von\":\"a\"}\n");
    });
    let stand_in_b = StandIn::start(&b, |_, mut strom| {
        let _ = strom.write_all(b"{\"von\":\"b\"}\n");
    });
    let arbeit = wegwerf.pfad("arbeit");
    let heim = wegwerf.pfad("heim");
    fs::create_dir(&arbeit).expect("mkdir");
    fs::create_dir(&heim).expect("mkdir");
    let (b_text, heim_text, arbeit_text) = (
        b.to_str().expect("UTF-8"),
        heim.to_str().expect("UTF-8"),
        wegwerf.ordner().to_str().expect("UTF-8"),
    );
    let umgebung = [
        ("SUCH_SOCKET", b_text),
        ("SUCH_MCP_SOCKET", b_text),
        ("BRAIN_SOCKET", b_text),
        ("BRAIN_SUCH_SOCKET", b_text),
        ("MCP_SOCKET", b_text),
        ("SOCKET", b_text),
        ("XDG_RUNTIME_DIR", arbeit_text),
        ("TMPDIR", arbeit_text),
        ("HOME", heim_text),
        ("ROUTE", "lokal"),
        ("RUST_LOG", "trace"),
        ("RUST_BACKTRACE", "full"),
    ];
    let eingabe = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"brain_suche\",\"arguments\":{\"query\":\"x\"}}}\n";
    let mit = lauf(
        &["--socket", a.to_str().expect("UTF-8")],
        eingabe,
        &arbeit,
        &umgebung,
    );
    let ohne = lauf(
        &["--socket", a.to_str().expect("UTF-8")],
        eingabe,
        &arbeit,
        &[],
    );
    assert_eq!(mit.stdout, ohne.stdout);
    assert_eq!(mit.status.code(), Some(0));
    assert!(mit.stderr.is_empty() && ohne.stderr.is_empty());
    assert_eq!(
        zeilen(&mit)[0]["result"]["content"][0]["text"],
        r#"{"von":"a"}"#
    );
    assert_eq!(stand_in_a.empfangen().len(), 2);
    assert!(
        stand_in_b.empfangen().is_empty(),
        "the socket of the environment was asked"
    );
    assert_eq!(
        fs::read_dir(&arbeit).expect("ls").count(),
        0,
        "a file appeared in the working folder"
    );
    assert_eq!(
        fs::read_dir(&heim).expect("ls").count(),
        0,
        "a file appeared in the home folder"
    );
    let mut namen: Vec<String> = fs::read_dir(wegwerf.ordner())
        .expect("ls")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    assert_eq!(namen, ["a.sock", "arbeit", "b.sock", "heim"]);
}

// ---- the source holds no call for the environment, files or the network ---------------------------------

const VERBOTEN: [&str; 21] = [
    "env::var",
    "env::vars",
    "env::set_var",
    "env::remove_var",
    "env::current_dir",
    "env::temp_dir",
    "env::home_dir",
    "std::fs",
    "File::",
    "OpenOptions",
    "std::net",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "ToSocketAddrs",
    "std::process",
    "Command::",
    "UnixListener",
    "UnixDatagram",
    "libc",
    "reqwest",
];

fn verbotenes(text: &str) -> Vec<&'static str> {
    // `std::process::ExitCode` is the one allowed use of std::process (the exit code of main).
    let text = text
        .replace("std::process::ExitCode", "")
        .replace("use std::process::ExitCode;", "");
    VERBOTEN
        .iter()
        .copied()
        .filter(|wort| text.contains(wort))
        .collect()
}

fn quellen(ordner: &Path, treffer: &mut Vec<std::path::PathBuf>) {
    for eintrag in fs::read_dir(ordner).expect("ls") {
        let pfad = eintrag.expect("entry").path();
        if pfad.is_dir() {
            quellen(&pfad, treffer);
        } else if pfad.extension().is_some_and(|e| e == "rs") {
            treffer.push(pfad);
        }
    }
}

#[test]
fn the_source_calls_no_environment_file_process_or_network_function() {
    let mut dateien = Vec::new();
    quellen(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut dateien,
    );
    assert!(dateien.len() >= 6, "the scan found {dateien:?}");
    for datei in dateien {
        let text = fs::read_to_string(&datei).expect("read");
        assert_eq!(verbotenes(&text), Vec::<&str>::new(), "{}", datei.display());
    }
}

#[test]
fn the_scan_finds_what_it_looks_for() {
    for wort in VERBOTEN {
        assert!(
            verbotenes(&format!("let x = {wort}(y);")).contains(&wort),
            "{wort}"
        );
    }
    assert_eq!(
        verbotenes("let a = std::os::unix::net::UnixStream::connect(p);"),
        Vec::<&str>::new()
    );
    assert_eq!(
        verbotenes("std::env::args_os().skip(1)"),
        Vec::<&str>::new()
    );
}

/// The names of the dependency section `kopf` of the manifest (until the next section).
fn abhaengigkeiten<'a>(manifest: &'a str, kopf: &str) -> Vec<&'a str> {
    manifest
        .split(kopf)
        .skip(1)
        .flat_map(|abschnitt| {
            abschnitt
                .split("\n[")
                .next()
                .expect("section")
                .lines()
                .filter(|z| !z.trim().is_empty() && !z.trim_start().starts_with('#'))
                .map(|z| z.split('=').next().expect("a name").trim())
        })
        .collect()
}

/// Until 23.11.2026 this test was `the_manifest_names_serde_json_as_the_only_dependency` and read only `[dependencies]`; a
/// dependency in a target section went unseen (review B13). Now every dependency section counts: `serde_json` on every system,
/// `nix` only under macOS, nothing else.
#[test]
fn the_manifest_names_serde_json_everywhere_and_nix_only_under_macos() {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).expect("read");
    assert_eq!(
        abhaengigkeiten(&manifest, "\n[dependencies]"),
        ["serde_json"]
    );
    assert_eq!(
        abhaengigkeiten(
            &manifest,
            "\n[target.'cfg(target_os = \"macos\")'.dependencies]"
        ),
        ["nix"]
    );
    let abschnitte: Vec<&str> = manifest
        .lines()
        .filter(|z| z.starts_with('[') && z.contains("dependencies"))
        .collect();
    assert_eq!(
        abschnitte,
        [
            "[dependencies]",
            "[target.'cfg(target_os = \"macos\")'.dependencies]"
        ],
        "no other dependency section"
    );
}
