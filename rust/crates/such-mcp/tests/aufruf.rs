//! The command line: no argument or `--socket <path>`; everything else is a usage error.

use std::ffi::OsString;
use std::path::PathBuf;

use such_mcp::aufruf::{SOCKET_IM_EINSCHLUSS, socket_pfad};

fn args(teile: &[&str]) -> Vec<OsString> {
    teile.iter().map(OsString::from).collect()
}

#[test]
fn without_an_argument_the_fixed_path_of_the_enclosure_is_used() {
    assert_eq!(SOCKET_IM_EINSCHLUSS, "/run/brain/such.sock");
    assert_eq!(
        socket_pfad(&[]),
        Some(PathBuf::from("/run/brain/such.sock"))
    );
}

#[test]
fn socket_with_a_path_replaces_the_fixed_path() {
    assert_eq!(
        socket_pfad(&args(&["--socket", "/tmp/x.sock"])),
        Some(PathBuf::from("/tmp/x.sock"))
    );
    assert_eq!(
        socket_pfad(&args(&["--socket", "rel/s"])),
        Some(PathBuf::from("rel/s"))
    );
    assert_eq!(
        socket_pfad(&args(&["--socket", "--socket"])),
        Some(PathBuf::from("--socket"))
    );
}

#[test]
fn every_other_call_is_refused() {
    for teile in [
        &["--socket"][..],
        &["--socket", ""],
        &["--socket", "a", "b"],
        &["--socket", "a", "--socket", "b"],
        &["--socket=a"],
        &["--sock", "a"],
        &["-s", "a"],
        &["--Socket", "a"],
        &["a"],
        &["a", "--socket", "b"],
        &["--route", "lokal"],
        &["--socket", "a", "--route", "lokal"],
        &["--help"],
        &["--version"],
        &[""],
        &["", ""],
    ] {
        assert_eq!(socket_pfad(&args(teile)), None, "{teile:?}");
    }
}
