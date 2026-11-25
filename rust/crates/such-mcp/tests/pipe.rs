//! The call of `such-mcp` under Windows (addendum for Windows, section 6): `--socket` is required and names a pipe of the form
//! `\\.\pipe\wb-such-<32 hex>`. The rules take the platform as a parameter, so these tests run on every system.

use std::ffi::OsString;
use std::path::PathBuf;

use such_mcp::aufruf::socket_pfad_fuer;
use such_mcp::mac::{Plattform, pfad_ok, server_ok};
use such_mcp::pipe::name_ok;

const PIPE: &str = r"\\.\pipe\wb-such-0123456789abcdef0123456789abcdef";

fn args(teile: &[&str]) -> Vec<OsString> {
    teile.iter().map(OsString::from).collect()
}

#[test]
fn the_current_platform_follows_the_target() {
    assert_eq!(Plattform::AKTUELL == Plattform::Windows, cfg!(windows));
}

#[test]
fn the_name_has_exactly_the_prefix_and_32_small_hex_characters() {
    assert!(name_ok(PIPE));
    for falsch in [
        r"\\.\pipe\wb-such-0123456789abcdef0123456789abcde",
        r"\\.\pipe\wb-such-0123456789abcdef0123456789abcdef0",
        r"\\.\pipe\wb-such-0123456789ABCDEF0123456789ABCDEF",
        r"\\host\pipe\wb-such-0123456789abcdef0123456789abcdef",
        r"\\.\pipe\wb-plattform",
        r"\\.\pipe\such",
        "/run/brain/such.sock",
        "",
    ] {
        assert!(!name_ok(falsch), "{falsch}");
    }
}

#[test]
fn windows_takes_only_the_option_with_a_pipe_name() {
    assert_eq!(
        socket_pfad_fuer(Plattform::Windows, &args(&["--socket", PIPE])),
        Some(PathBuf::from(PIPE))
    );
    for teile in [
        &[][..],
        &["--socket"],
        &["--socket", ""],
        &["--socket", "/run/brain/such.sock"],
        &["--socket", r"C:\nicht\da.sock"],
        &["--socket", r"\\.\pipe\wb-plattform"],
        &["--socket", PIPE, "--socket", PIPE],
        &["--socket", PIPE, "--route", "lokal"],
        &["--route", "lokal"],
    ] {
        assert_eq!(
            socket_pfad_fuer(Plattform::Windows, &args(teile)),
            None,
            "{teile:?}"
        );
    }
    assert!(pfad_ok(Plattform::Windows, std::path::Path::new(PIPE)));
}

#[test]
fn the_rule_of_the_euid_says_no_under_windows() {
    assert!(!server_ok::<()>(Plattform::Windows, Ok(501), 501));
}
