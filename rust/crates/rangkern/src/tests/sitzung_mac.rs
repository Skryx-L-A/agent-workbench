//! The rules of `rangkern sitzung` for macOS (addendum to contract 4): the arguments, the parent folder before `bind` and the
//! decision about the peer. The rules take the platform as a parameter, so these tests run under Linux with `MacOs`; they
//! use real folders. The measurement of the peer itself (`getpeereid`) is tested only on a Mac, at the end of this file.

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use super::TempDir;
use crate::sitzung::konfig::Konfig;
use crate::sitzung::mac::{
    GRUND_FREMD, GRUND_UNMESSBAR, Plattform, SOCKET_MAX_MACOS, aufruf_ok, eigene_euid,
    ordner_pruefen, peer_entscheiden,
};

const MAC: Plattform = Plattform::MacOs;

/// A socket path of exactly `laenge` bytes: `/a/aaaa...`.
fn pfad_von(laenge: usize) -> String {
    let kopf = "/tmp/";
    format!("{kopf}{}", "a".repeat(laenge - kopf.len()))
}

/// The vault of `dir` and the five required options with the given socket and rights, then nothing else.
fn args(dir: &TempDir, socket: &str, rechte: Option<&str>) -> Vec<String> {
    let mut args = vec![dir.path().to_string_lossy().to_string()];
    for teil in [
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
    ] {
        args.push(teil.to_string());
    }
    if let Some(rechte) = rechte {
        args.push("--socket-rechte".to_string());
        args.push(rechte.to_string());
    }
    args
}

#[test]
fn the_current_platform_matches_the_target() {
    assert_eq!(
        Plattform::AKTUELL == MAC,
        cfg!(target_os = "macos"),
        "AKTUELL follows the target"
    );
}

#[test]
fn macos_refuses_the_rights_0666_and_linux_still_takes_them() {
    let dir = TempDir::new();
    assert!(Konfig::lesen_fuer(MAC, &args(&dir, "/tmp/s.sock", Some("0666"))).is_none());
    assert!(Konfig::lesen_fuer(MAC, &args(&dir, "/tmp/s.sock", Some("0600"))).is_some());
    assert!(Konfig::lesen_fuer(MAC, &args(&dir, "/tmp/s.sock", None)).is_some());
    assert!(
        Konfig::lesen_fuer(Plattform::Linux, &args(&dir, "/tmp/s.sock", Some("0666"))).is_some()
    );
    assert!(!aufruf_ok(MAC, Path::new("/tmp/s.sock"), 0o666));
    assert!(aufruf_ok(Plattform::Linux, Path::new("/tmp/s.sock"), 0o666));
}

#[test]
fn macos_wants_an_absolute_socket_path_of_at_most_103_bytes() {
    let dir = TempDir::new();
    assert_eq!(SOCKET_MAX_MACOS, 103);
    for relativ in ["such.sock", "./such.sock", "s/such.sock"] {
        assert!(
            Konfig::lesen_fuer(MAC, &args(&dir, relativ, None)).is_none(),
            "{relativ}"
        );
        assert!(Konfig::lesen_fuer(Plattform::Linux, &args(&dir, relativ, None)).is_some());
    }
    assert!(Konfig::lesen_fuer(MAC, &args(&dir, &pfad_von(103), None)).is_some());
    assert!(Konfig::lesen_fuer(MAC, &args(&dir, &pfad_von(104), None)).is_none());
    assert!(Konfig::lesen_fuer(Plattform::Linux, &args(&dir, &pfad_von(104), None)).is_some());
    // The length counts bytes, not characters.
    let umlaute = format!("/tmp/{}", "ä".repeat(49)); // 5 + 98 bytes = 103
    assert!(Konfig::lesen_fuer(MAC, &args(&dir, &umlaute, None)).is_some());
    let zu_lang = format!("/tmp/{}", "ä".repeat(50)); // 5 + 100 bytes
    assert!(Konfig::lesen_fuer(MAC, &args(&dir, &zu_lang, None)).is_none());
}

/// A folder with the mode.
fn ordner_mit(dir: &TempDir, name: &str, modus: u32) -> PathBuf {
    let ordner = dir.path().join(name);
    fs::create_dir_all(&ordner).expect("folder");
    fs::set_permissions(&ordner, fs::Permissions::from_mode(modus)).expect("mode");
    ordner
}

#[test]
fn the_parent_folder_0700_of_the_own_user_is_taken() {
    let dir = TempDir::new();
    let ordner = ordner_mit(&dir, "s", 0o700);
    assert_eq!(
        ordner_pruefen(MAC, &ordner.join("such.sock"), eigene_euid()),
        Ok(())
    );
}

#[test]
fn a_parent_folder_with_another_mode_is_refused_and_stays_as_it_is() {
    let dir = TempDir::new();
    for modus in [
        0o755, 0o750, 0o770, 0o701, 0o777, 0o500, 0o600, 0o1700, 0o2700,
    ] {
        let ordner = ordner_mit(&dir, &format!("m{modus:o}"), modus);
        assert!(
            ordner_pruefen(MAC, &ordner.join("such.sock"), eigene_euid()).is_err(),
            "mode {modus:o}"
        );
        let danach = fs::metadata(&ordner).expect("folder").permissions().mode() & 0o7777;
        assert_eq!(danach, modus, "the folder stays as it is");
        assert!(!ordner.join("such.sock").exists());
    }
}

#[test]
fn a_parent_folder_that_is_a_link_is_refused_even_if_the_target_is_0700() {
    let dir = TempDir::new();
    let ziel = ordner_mit(&dir, "ziel", 0o700);
    let verweis = dir.path().join("verweis");
    symlink(&ziel, &verweis).expect("link");
    assert!(ordner_pruefen(MAC, &verweis.join("such.sock"), eigene_euid()).is_err());
    assert!(!ziel.join("such.sock").exists());
}

#[test]
fn a_parent_folder_of_another_owner_is_refused() {
    let dir = TempDir::new();
    let ordner = ordner_mit(&dir, "s", 0o700);
    assert!(
        ordner_pruefen(
            MAC,
            &ordner.join("such.sock"),
            eigene_euid().wrapping_add(1)
        )
        .is_err()
    );
}

#[test]
fn a_missing_parent_folder_or_a_file_is_refused() {
    let dir = TempDir::new();
    assert!(ordner_pruefen(MAC, &dir.path().join("fehlt/such.sock"), eigene_euid()).is_err());
    let datei = dir.write("datei", b"x");
    assert!(ordner_pruefen(MAC, &datei.join("such.sock"), eigene_euid()).is_err());
    assert!(ordner_pruefen(MAC, Path::new("such.sock"), eigene_euid()).is_err());
}

#[test]
fn linux_checks_no_parent_folder() {
    let dir = TempDir::new();
    let offen = ordner_mit(&dir, "offen", 0o755);
    assert_eq!(
        ordner_pruefen(
            Plattform::Linux,
            &offen.join("such.sock"),
            eigene_euid().wrapping_add(1)
        ),
        Ok(())
    );
    assert_eq!(
        ordner_pruefen(Plattform::Linux, Path::new("such.sock"), 0),
        Ok(())
    );
}

#[test]
fn the_peer_with_the_own_euid_is_taken() {
    assert_eq!(peer_entscheiden(MAC, Some(501), 501), None);
    assert_eq!(peer_entscheiden(MAC, Some(0), 0), None);
}

#[test]
fn a_peer_with_another_euid_is_foreign_even_root() {
    assert_eq!(peer_entscheiden(MAC, Some(502), 501), Some(GRUND_FREMD));
    assert_eq!(peer_entscheiden(MAC, Some(0), 501), Some(GRUND_FREMD));
    assert_eq!(peer_entscheiden(MAC, Some(501), 0), Some(GRUND_FREMD));
    assert_eq!(GRUND_FREMD, "such.kennung_fremd");
}

#[test]
fn a_failed_measurement_is_unmeasurable() {
    assert_eq!(peer_entscheiden(MAC, None, 501), Some(GRUND_UNMESSBAR));
    assert_eq!(GRUND_UNMESSBAR, "such.kennung_unmessbar");
}

#[test]
fn linux_checks_no_peer() {
    assert_eq!(peer_entscheiden(Plattform::Linux, Some(502), 501), None);
    assert_eq!(peer_entscheiden(Plattform::Linux, None, 501), None);
}

/// The real way, only on a Mac: the same user is taken. The measurement of `getpeereid` on a socket pair names the own euid.
#[cfg(target_os = "macos")]
#[test]
fn getpeereid_names_the_own_euid_on_a_pair_of_sockets() {
    let (a, _b) = std::os::unix::net::UnixStream::pair().expect("pair");
    let gemessen = crate::sitzung::mac::gemessene_euid(&a).expect("measured");
    assert_eq!(gemessen, eigene_euid());
    assert_eq!(peer_entscheiden(MAC, Some(gemessen), eigene_euid()), None);
}
