//! The start check of the subreaper (contract `dienst`, section 17, point 1; Linux only): a service that cannot read the `children`
//! file of its thread ends at the start with exit code 1 and the fixed text, before the child is started.
//!
//! The program runs as pid 1 of its own user, mount and pid namespace (`unshare -Urpfm --mount-proc`); a `tmpfs` over `/proc/1/task`
//! hides `/proc/thread-self/children` of that pid only, the rest of `/proc` stays. The child of the start file is a program that does not
//! exist: a service that passes the check fails later with exit code 4 (the counter-check of the same namespace without the `tmpfs`).
//! Needs `unshare` and user namespaces; without them the test is red, not skipped.

#![cfg(target_os = "linux")]

mod gemeinsam;

use std::process::Command;

use gemeinsam::{DIENST, Wegwerf, datei_schreiben, startdatei};
use serde_json::json;

/// Runs the program in its own namespaces; `verdecken` puts the `tmpfs` over `/proc/1/task` first. Returns exit code and stderr.
fn im_namensraum(w: &Wegwerf, verdecken: bool) -> (Option<i32>, String) {
    let datei = datei_schreiben(w, &startdatei(w, &["/nirgends/programm"], &json!({})));
    let skript = if verdecken {
        "mount -t tmpfs none /proc/1/task || exit 97; exec \"$0\" \"$1\""
    } else {
        "exec \"$0\" \"$1\""
    };
    let aus = Command::new("unshare")
        .args([
            "-Urpfm",
            "--mount-proc",
            "--",
            "/bin/sh",
            "-c",
            skript,
            DIENST,
        ])
        .arg(&datei)
        .output()
        .expect("unshare runs");
    (
        aus.status.code(),
        String::from_utf8_lossy(&aus.stderr).into_owned(),
    )
}

#[test]
fn ohne_lesbare_children_datei_endet_der_start_mit_1_vor_dem_kind() {
    let w = Wegwerf::neu("kinder-lesbar");
    // counter-check: the same namespace with a readable file passes the check and fails at the missing child
    let (code, fehler) = im_namensraum(&w, false);
    assert_eq!(code, Some(4), "without the tmpfs: {fehler}");
    let (code, fehler) = im_namensraum(&w, true);
    assert_eq!(code, Some(1), "with the tmpfs: {fehler}");
    assert!(
        fehler.contains("Subreaper: children fehlt"),
        "the fixed text: {fehler}"
    );
    assert!(!w.socket().exists(), "no socket file is left");
}
