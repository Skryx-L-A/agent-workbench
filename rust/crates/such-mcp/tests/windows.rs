//! Under Windows there is no socket way yet (the named pipe follows with variant B): the tool answers every
//! `brain_suche` call with "Suche nicht erreichbar" and `isError: true`, and asks nobody.
//! The tests run only under `cfg(windows)`; the cross build compiles them.
#![cfg(windows)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use such_mcp::suche::{Befrager, Unerreichbar, UnixSocket};

const PROGRAMM: &str = env!("CARGO_BIN_EXE_such-mcp");

#[test]
fn every_question_is_unreachable_and_without_waiting() {
    for pfad in [
        "C:\\nicht\\da.sock",
        "\\\\.\\pipe\\such",
        "/run/brain/such.sock",
        "",
    ] {
        let socket = UnixSocket::mit_grenzen(pfad, Duration::from_secs(5), 1000);
        let start = Instant::now();
        assert_eq!(
            socket.frage("{\"op\":\"ping\"}"),
            Err(Unerreichbar),
            "{pfad:?}"
        );
        assert!(start.elapsed() < Duration::from_secs(2), "{pfad:?}");
    }
}

#[test]
fn the_program_answers_brain_suche_with_the_fixed_text() {
    let eingabe = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"brain_suche\",\"arguments\":{\"query\":\"x\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"brain_suche\",\"arguments\":{\"query\":\"x\",\"route\":\"lokal\"}}}\n",
    );
    let mut kind = Command::new(PROGRAMM)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the program");
    kind.stdin
        .take()
        .expect("stdin")
        .write_all(eingabe.as_bytes())
        .expect("write");
    let ausgang = kind.wait_with_output().expect("wait");
    assert_eq!(ausgang.status.code(), Some(0));
    let text = String::from_utf8(ausgang.stdout).expect("UTF-8");
    let antworten: Vec<Value> = text
        .lines()
        .map(|z| serde_json::from_str(z).expect("JSON"))
        .collect();
    assert_eq!(antworten.len(), 3);
    assert_eq!(antworten[0]["result"]["serverInfo"]["name"], "such-mcp");
    assert_eq!(
        antworten[1]["result"]["content"][0]["text"],
        "Suche nicht erreichbar"
    );
    assert_eq!(antworten[1]["result"]["isError"], true);
    assert_eq!(
        antworten[2]["result"]["content"][0]["text"],
        "Ungültige Anfrage"
    );
}

#[test]
fn a_wrong_call_exits_with_64_under_windows_too() {
    let ausgang = Command::new(PROGRAMM)
        .args(["--route", "lokal"])
        .stdin(Stdio::null())
        .output()
        .expect("run");
    assert_eq!(ausgang.status.code(), Some(64));
    assert!(ausgang.stdout.is_empty() && ausgang.stderr.is_empty());
}
