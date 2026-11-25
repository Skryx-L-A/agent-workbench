//! `rangkern sitzung` under Windows (addendum for Windows, section 2): a usage error ends with exit 64 and the usage text on
//! the error output, and writes nothing to the standard output. These tests run only for a Windows target; the start with a
//! real pipe is a case of `rust/spec/such-socket/windows/` in the Windows VM.

use crate::cli::run;

fn lauf(args: &[&str]) -> (u8, Vec<u8>, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let code = run(&args, &mut out, &mut err);
    (code, out, String::from_utf8(err).expect("text"))
}

#[test]
fn an_incomplete_call_ends_with_exit_64_and_the_usage() {
    let (code, out, err) = lauf(&["sitzung", "vault", "--socket", "s", "--route", "r"]);
    assert_eq!(code, 64);
    assert!(out.is_empty());
    assert!(err.starts_with("usage: rangkern"), "{err}");
}

#[test]
fn the_command_without_arguments_ends_the_same_way() {
    let (code, out, err) = lauf(&["sitzung"]);
    assert_eq!(code, 64);
    assert!(out.is_empty());
    assert!(err.starts_with("usage: rangkern"), "{err}");
}

#[test]
fn the_rights_of_the_socket_are_a_usage_error_before_any_file_is_read() {
    let ordner = std::env::temp_dir();
    let vault = ordner.to_string_lossy().to_string();
    let (code, out, _) = lauf(&[
        "sitzung",
        &vault,
        "--socket",
        r"\\.\pipe\wb-such-0123456789abcdef0123456789abcdef",
        "--paket-sid",
        "S-1-15-2-1-2-3-4-5-6-7",
        "--route",
        "cloud",
        "--sitzung",
        "s-1",
        "--einstellungen",
        "fehlt.json",
        "--protokoll",
        "fehlt",
        "--socket-rechte",
        "0600",
    ]);
    assert_eq!(code, 64);
    assert!(out.is_empty());
}
