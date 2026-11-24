//! The stand-in harness is not in the delivery build. The case builds the crate twice in throw-away target folders:
//! without the feature `attrappe` (the delivery build) and with it (the control). The delivery build has no program
//! `einschluss-attrappe`, and no file of its target folder holds the marker of the stand-in; the control has both. A
//! build that held the stand-in would turn the first half red, and a search that cannot find it would turn the second
//! half red.
//!
//! The case runs on every platform and with and without the feature in the test build itself.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The text that stands only in the module `attrappe`. The test does not import the module (it does not exist without
/// the feature), so the text is repeated here; a case below checks that it is the marker.
const MARKE: &str = "einschluss-attrappe-marker-7f3a";

fn bauort() -> PathBuf {
    let basis = std::env::var_os("EINSCHLUSS_TESTORT")
        .map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
    basis.join(format!("ei-lieferung-{}", std::process::id()))
}

fn bauen(ziel: &Path, merkmale: &[&str]) {
    let cargo = std::env::var_os("CARGO").expect("cargo sets CARGO for its tests");
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let mut c = Command::new(cargo);
    c.args([
        "build",
        "--release",
        "--locked",
        "--offline",
        "--manifest-path",
    ])
    .arg(manifest)
    .arg("--target-dir")
    .arg(ziel);
    if !merkmale.is_empty() {
        c.args(["--features", &merkmale.join(",")]);
    }
    let aus = c.output().expect("cargo starts");
    assert!(
        aus.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&aus.stderr)
    );
}

/// Every file below `wurzel` that holds the marker, and every file whose name has `attrappe` in it.
fn funde(wurzel: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let (mut mit_marke, mut mit_namen) = (Vec::new(), Vec::new());
    let mut stapel = vec![wurzel.to_path_buf()];
    while let Some(pfad) = stapel.pop() {
        let Ok(meta) = fs::symlink_metadata(&pfad) else {
            continue;
        };
        if meta.is_dir() {
            stapel.extend(
                fs::read_dir(&pfad)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path()),
            );
        } else if meta.is_file() {
            let name = pfad
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.contains("attrappe") {
                mit_namen.push(pfad.clone());
            }
            let inhalt = fs::read(&pfad).unwrap_or_default();
            if inhalt.windows(MARKE.len()).any(|f| f == MARKE.as_bytes()) {
                mit_marke.push(pfad);
            }
        }
    }
    (mit_marke, mit_namen)
}

#[test]
fn the_delivery_build_has_no_stand_in_and_the_control_build_has() {
    let wurzel = bauort();
    let _ = fs::remove_dir_all(&wurzel);
    let lieferung = wurzel.join("lieferung");
    let kontrolle = wurzel.join("kontrolle");
    bauen(&lieferung, &[]);
    bauen(&kontrolle, &["attrappe"]);
    let (marke_l, namen_l) = funde(&lieferung);
    let (marke_k, namen_k) = funde(&kontrolle);
    let _ = fs::remove_dir_all(&wurzel);
    assert!(
        marke_l.is_empty(),
        "the delivery build holds the marker: {marke_l:?}"
    );
    assert!(
        namen_l.is_empty(),
        "the delivery build has a file named attrappe: {namen_l:?}"
    );
    assert!(
        !marke_k.is_empty(),
        "the control build does not hold the marker, so the search is blind"
    );
    // the program has the suffix of the host (`.exe` on Windows, none on Unix)
    let programm = format!("einschluss-attrappe{}", std::env::consts::EXE_SUFFIX);
    assert!(
        namen_k
            .iter()
            .any(|p| p.file_name().is_some_and(|n| *n == *programm)),
        "the control build has no program {programm}: {namen_k:?}"
    );
}

#[cfg(feature = "attrappe")]
#[test]
fn the_marker_of_this_case_is_the_marker_of_the_module() {
    assert_eq!(MARKE, einschluss::attrappe::MARKER);
}
