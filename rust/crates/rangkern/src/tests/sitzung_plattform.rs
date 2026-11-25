//! The rules of `rangkern sitzung` for Windows that need no system call (addendum for Windows, sections 2 and 4): the pipe
//! name, the package SID, the options and the decision about the answer of the platform service. They take the platform as a
//! parameter, so these tests run on every system.

use super::TempDir;
use crate::sitzung::dienst::{
    self, GRUND_LAUF_UNBEKANNT, GRUND_SITZUNG_FREMD, GRUND_STUMM, Messung, entscheiden,
};
use crate::sitzung::konfig::Konfig;
use crate::sitzung::plattform::{Plattform, aufruf_ok, kanalname, paket_sid_ok, pipe_name_ok};

const WIN: Plattform = Plattform::Windows;
const PIPE: &str = r"\\.\pipe\wb-such-0123456789abcdef0123456789abcdef";
const PAKET: &str =
    "S-1-15-2-1111111111-2222222222-3333333333-444444444-555555555-666666666-777777777";

/// The vault of `dir` and the six required options under Windows, then `rest`.
fn args(dir: &TempDir, socket: &str, paket: Option<&str>, rest: &[&str]) -> Vec<String> {
    let mut args = vec![dir.path().to_string_lossy().to_string()];
    let mut teile = vec![
        "--socket",
        socket,
        "--route",
        "cloud",
        "--sitzung",
        "s-1",
        "--einstellungen",
        "/e.json",
        "--protokoll",
        "/p",
    ];
    if let Some(paket) = paket {
        teile.extend(["--paket-sid", paket]);
    }
    teile.extend(rest);
    args.extend(teile.iter().map(|teil| (*teil).to_string()));
    args
}

#[test]
fn the_current_platform_follows_the_target() {
    assert_eq!(Plattform::AKTUELL == WIN, cfg!(windows));
    assert_eq!(
        Plattform::AKTUELL == Plattform::MacOs,
        cfg!(target_os = "macos")
    );
}

#[test]
fn the_pipe_name_has_exactly_the_prefix_and_32_small_hex_characters() {
    assert!(pipe_name_ok(PIPE));
    assert_eq!(
        kanalname(PIPE),
        Some("wb-such-0123456789abcdef0123456789abcdef")
    );
    for falsch in [
        r"\\.\pipe\wb-such-0123456789abcdef0123456789abcde",
        r"\\.\pipe\wb-such-0123456789abcdef0123456789abcdef0",
        r"\\.\pipe\wb-such-0123456789ABCDEF0123456789ABCDEF",
        r"\\.\pipe\wb-such-0123456789abcdef0123456789abcdeg",
        r"\\host\pipe\wb-such-0123456789abcdef0123456789abcdef",
        r"\\.\pipe\wb-plattform",
        r"C:\tmp\s.sock",
        "/tmp/s.sock",
        "",
    ] {
        assert!(!pipe_name_ok(falsch), "{falsch}");
        assert_eq!(kanalname(falsch), None, "{falsch}");
    }
}

#[test]
fn the_package_sid_has_seven_numbers_without_a_leading_zero_and_at_most_u32() {
    assert!(paket_sid_ok(PAKET));
    assert!(paket_sid_ok("S-1-15-2-0-1-2-3-4-5-4294967295"));
    for falsch in [
        "S-1-15-2-01-2-3-4-5-6-7",
        "S-1-15-2-1-2-3-4-5-6",
        "S-1-15-2-1-2-3-4-5-6-7-8",
        "S-1-15-2-4294967296-2-3-4-5-6-7",
        "S-1-15-2-1-2-3-4-5-6-",
        "S-1-15-2-1-2-3-4-5--7",
        "S-1-15-2-+1-2-3-4-5-6-7",
        "S-1-5-21-1000000001-1000000002-1000000003-1001",
        "s-1-15-2-1-2-3-4-5-6-7",
        "",
    ] {
        assert!(!paket_sid_ok(falsch), "{falsch}");
    }
}

#[test]
fn windows_takes_the_pipe_name_and_the_package_sid() {
    let dir = TempDir::new();
    let konfig = Konfig::lesen_fuer(WIN, &args(&dir, PIPE, Some(PAKET), &[])).expect("valid");
    assert_eq!(konfig.paket_sid.as_deref(), Some(PAKET));
    assert_eq!(konfig.socket.to_str(), Some(PIPE));
    // The rule of the arguments for the pipe alone.
    assert!(aufruf_ok(WIN, std::path::Path::new(PIPE), 0o600));
    assert!(!aufruf_ok(WIN, std::path::Path::new("/tmp/s.sock"), 0o600));
}

#[test]
fn windows_refuses_a_missing_a_doubled_or_a_wrong_package_sid() {
    let dir = TempDir::new();
    assert!(Konfig::lesen_fuer(WIN, &args(&dir, PIPE, None, &[])).is_none());
    assert!(
        Konfig::lesen_fuer(WIN, &args(&dir, PIPE, Some(PAKET), &["--paket-sid", PAKET])).is_none()
    );
    let gleich = format!("--paket-sid={PAKET}");
    assert!(Konfig::lesen_fuer(WIN, &args(&dir, PIPE, Some(PAKET), &[&gleich])).is_none());
    assert!(
        Konfig::lesen_fuer(WIN, &args(&dir, PIPE, Some("S-1-15-2-1-2-3-4-5-6"), &[])).is_none()
    );
    assert!(Konfig::lesen_fuer(WIN, &args(&dir, PIPE, Some(""), &[])).is_none());
}

#[test]
fn windows_refuses_the_option_of_the_rights_also_with_0600() {
    let dir = TempDir::new();
    for rechte in ["0600", "0666", "0644"] {
        assert!(
            Konfig::lesen_fuer(
                WIN,
                &args(&dir, PIPE, Some(PAKET), &["--socket-rechte", rechte])
            )
            .is_none(),
            "{rechte}"
        );
    }
}

#[test]
fn windows_refuses_a_file_path_and_every_other_pipe_name() {
    let dir = TempDir::new();
    for socket in [
        "/tmp/s.sock",
        r"\\host\pipe\wb-such-0123456789abcdef0123456789abcdef",
        r"\\.\pipe\wb-plattform",
    ] {
        assert!(
            Konfig::lesen_fuer(WIN, &args(&dir, socket, Some(PAKET), &[])).is_none(),
            "{socket}"
        );
    }
}

#[test]
fn linux_and_macos_do_not_know_the_package_sid() {
    let dir = TempDir::new();
    // An absolute path on every system that runs the test (`/tmp/…` is not absolute under Windows).
    let sock = dir.path().join("s.sock").to_string_lossy().to_string();
    for plattform in [Plattform::Linux, Plattform::MacOs] {
        assert!(Konfig::lesen_fuer(plattform, &args(&dir, &sock, Some(PAKET), &[])).is_none());
        // The positive control: the same start without it.
        assert!(Konfig::lesen_fuer(plattform, &args(&dir, &sock, None, &[])).is_some());
    }
}

fn messung() -> Messung {
    Messung {
        pid: 4242,
        startzeit: Some(133_000_000_000_000_000),
        paket_sid: Some(PAKET.to_owned()),
    }
}

fn antwort(sitzung: &str, pid: u32, start: u64, sid: &str) -> Vec<u8> {
    format!(r#"{{"ok":true,"pid":{pid},"start":{start},"sid":"{sid}","sitzung":"{sitzung}"}}"#)
        .into_bytes()
}

#[test]
fn the_request_names_only_the_operation_and_the_pid() {
    assert_eq!(dienst::anfrage(4242), b"{\"op\":\"peer\",\"pid\":4242}\n");
}

#[test]
fn the_answer_of_the_own_run_is_accepted() {
    let gut = antwort("s-1", 4242, 133_000_000_000_000_000, PAKET);
    assert_eq!(entscheiden(Some(&gut), &messung(), "s-1"), None);
}

#[test]
fn another_session_is_sitzung_fremd() {
    let fremd = antwort("s-2", 4242, 133_000_000_000_000_000, PAKET);
    assert_eq!(
        entscheiden(Some(&fremd), &messung(), "s-1"),
        Some(GRUND_SITZUNG_FREMD)
    );
}

#[test]
fn an_unknown_peer_or_a_differing_answer_is_lauf_unbekannt() {
    let unbekannt = br#"{"ok":false,"code":"dienst.peer_unbekannt"}"#;
    assert_eq!(
        entscheiden(Some(unbekannt), &messung(), "s-1"),
        Some(GRUND_LAUF_UNBEKANNT)
    );
    for anders in [
        antwort("s-1", 4243, 133_000_000_000_000_000, PAKET),
        antwort("s-1", 4242, 133_000_000_000_000_001, PAKET),
        antwort(
            "s-1",
            4242,
            133_000_000_000_000_000,
            "S-1-15-2-1-2-3-4-5-6-7",
        ),
    ] {
        assert_eq!(
            entscheiden(Some(&anders), &messung(), "s-1"),
            Some(GRUND_LAUF_UNBEKANNT)
        );
    }
    // A measurement without a start time or outside a container never matches.
    let gut = antwort("s-1", 4242, 133_000_000_000_000_000, PAKET);
    let ohne_start = Messung {
        startzeit: None,
        ..messung()
    };
    assert_eq!(
        entscheiden(Some(&gut), &ohne_start, "s-1"),
        Some(GRUND_LAUF_UNBEKANNT)
    );
    let ohne_paket = Messung {
        paket_sid: None,
        ..messung()
    };
    assert_eq!(
        entscheiden(Some(&gut), &ohne_paket, "s-1"),
        Some(GRUND_LAUF_UNBEKANNT)
    );
}

#[test]
fn no_answer_another_error_or_a_broken_answer_is_dienst_stumm() {
    assert_eq!(entscheiden(None, &messung(), "s-1"), Some(GRUND_STUMM));
    for kaputt in [
        &br#"{"ok":false,"code":"dienst.intern"}"#[..],
        br#"{"ok":false}"#,
        br#"{"ok":"ja"}"#,
        br#"{"pid":4242}"#,
        br#"{"ok":true,"pid":4242,"start":1,"sid":"x"}"#,
        br#"{"ok":true,"pid":"4242","start":133000000000000000,"sid":"x","sitzung":"s-1"}"#,
        b"[]",
        b"kein json",
        b"",
    ] {
        assert_eq!(
            entscheiden(Some(kaputt), &messung(), "s-1"),
            Some(GRUND_STUMM),
            "{}",
            String::from_utf8_lossy(kaputt)
        );
    }
}

/// Without a check of the owner of the control pipe the question is not asked: every client that reaches this step is turned
/// away (fail closed).
#[test]
fn without_a_check_of_the_owner_the_service_is_not_asked() {
    assert_eq!(dienst::fragen(4242), None);
    assert_eq!(
        entscheiden(dienst::fragen(4242).as_deref(), &messung(), "s-1"),
        Some(GRUND_STUMM)
    );
}
