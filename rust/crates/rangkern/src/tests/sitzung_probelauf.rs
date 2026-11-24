//! The counting of the markers: the count in a buffer, and the file of the counts.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::TempDir;
use crate::sitzung::probelauf::{Zaehlung, zaehle};

#[test]
fn a_marker_is_counted_byte_for_byte_from_the_left_without_overlap() {
    assert_eq!(zaehle(b"", b"MI-1"), 0);
    assert_eq!(zaehle(b"MI-1", b"MI-1"), 1);
    assert_eq!(zaehle(b"xMI-1yMI-1", b"MI-1"), 2);
    assert_eq!(zaehle(b"mi-1 Mi-1 MI-2", b"MI-1"), 0);
    assert_eq!(zaehle(b"AAA", b"AA"), 1);
    assert_eq!(zaehle(b"AAAA", b"AA"), 2);
    assert_eq!(zaehle(b"A-A-A-A", b"A-A"), 2);
    assert_eq!(zaehle(b"MI-", b"MI-1"), 0);
    assert_eq!(zaehle(b"abc", b""), 0);
    // A marker at the very end and at the very start.
    assert_eq!(zaehle(b"MI-1....MI-1", b"MI-1"), 2);
}

fn marker(namen: &[&str]) -> Vec<String> {
    namen.iter().map(ToString::to_string).collect()
}

#[test]
fn the_file_starts_at_zero_and_counts_every_answer() {
    let dir = TempDir::new();
    let datei = dir.path().join("zaehlung.json");
    let zaehlung = Zaehlung::anlegen(datei.clone(), "s-1".into(), marker(&["MK-7Q2", "MI-4F9"]))
        .expect("the file is written");
    assert_eq!(zaehlung.anzahl_marker(), 2);
    assert_eq!(
        fs::read_to_string(&datei).expect("file"),
        r#"{"antworten":0,"marker":{"MI-4F9":0,"MK-7Q2":0},"sitzung":"s-1"}"#
    );
    assert_eq!(
        fs::metadata(&datei).expect("meta").permissions().mode() & 0o7777,
        0o600
    );
    zaehlung
        .zaehlen(b"MK-7Q2 MK-7Q2 MI-4F9\n")
        .expect("counted");
    zaehlung.zaehlen(b"nichts\n").expect("counted");
    zaehlung.zaehlen(b"MI-4F9\n").expect("counted");
    assert_eq!(
        fs::read_to_string(&datei).expect("file"),
        r#"{"antworten":3,"marker":{"MI-4F9":2,"MK-7Q2":2},"sitzung":"s-1"}"#
    );
    assert_eq!(
        fs::metadata(&datei).expect("meta").permissions().mode() & 0o7777,
        0o600
    );
    // No file beside it stays.
    let namen: Vec<String> = fs::read_dir(dir.path())
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(namen, ["zaehlung.json"]);
}

#[test]
fn an_existing_file_is_replaced_and_keeps_the_rights_of_the_new_one() {
    let dir = TempDir::new();
    let datei = dir.write("zaehlung.json", b"alt");
    fs::set_permissions(&datei, fs::Permissions::from_mode(0o644)).expect("chmod");
    Zaehlung::anlegen(datei.clone(), "s-1".into(), marker(&["A"])).expect("written");
    assert_eq!(
        fs::read_to_string(&datei).expect("file"),
        r#"{"antworten":0,"marker":{"A":0},"sitzung":"s-1"}"#
    );
    assert_eq!(
        fs::metadata(&datei).expect("meta").permissions().mode() & 0o7777,
        0o600
    );
}

#[test]
fn a_file_that_cannot_be_written_is_an_error_and_the_counts_stay() {
    let dir = TempDir::new();
    assert!(
        Zaehlung::anlegen(dir.path().join("fehlt/z.json"), "s".into(), marker(&["A"])).is_err()
    );
    let ordner = dir.path().join("z");
    fs::create_dir(&ordner).expect("dir");
    let datei = ordner.join("zaehlung.json");
    let zaehlung = Zaehlung::anlegen(datei.clone(), "s".into(), marker(&["A"])).expect("written");
    zaehlung.zaehlen(b"A").expect("counted");
    fs::set_permissions(&ordner, fs::Permissions::from_mode(0o500)).expect("chmod");
    if fs::File::create(ordner.join("probe")).is_ok() {
        // Running with the rights of root: no mode keeps the write out.
        fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("chmod");
        return;
    }
    assert!(zaehlung.zaehlen(b"A A").is_err());
    fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("chmod");
    assert_eq!(
        fs::read_to_string(&datei).expect("file"),
        r#"{"antworten":1,"marker":{"A":1},"sitzung":"s"}"#
    );
    // The failed answer is not in the counts: the next one adds to the last written state.
    zaehlung.zaehlen(b"A").expect("counted");
    assert_eq!(
        fs::read_to_string(&datei).expect("file"),
        r#"{"antworten":2,"marker":{"A":2},"sitzung":"s"}"#
    );
}

#[test]
fn a_foreign_file_or_link_under_the_name_beside_stays_and_the_write_fails() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new();
    let datei = dir.path().join("zaehlung.json");
    let beside = format!("zaehlung.json.{}.neu", std::process::id());
    // A foreign file under the name of the file beside it: `create_new` fails, and the file is not removed.
    let fremd = dir.write(&beside, b"fremd");
    assert!(Zaehlung::anlegen(datei.clone(), "s".into(), marker(&["A"])).is_err());
    assert_eq!(fs::read_to_string(&fremd).expect("file"), "fremd");
    assert!(!datei.exists());
    // A link there: the link stays and so does its target.
    fs::remove_file(&fremd).expect("remove");
    let aussen = dir.write("aussen", b"x");
    symlink(&aussen, &fremd).expect("link");
    assert!(Zaehlung::anlegen(datei.clone(), "s".into(), marker(&["A"])).is_err());
    assert!(fs::symlink_metadata(&fremd).expect("link").is_symlink());
    assert_eq!(fs::read_to_string(&aussen).expect("file"), "x");
    assert!(!datei.exists());
}

#[test]
fn a_failed_rename_removes_the_own_file_beside_and_nothing_else() {
    let dir = TempDir::new();
    // The name of the count file is a folder with content: the rename fails; the file beside it must not stay.
    let datei = dir.path().join("zaehlung.json");
    fs::create_dir(&datei).expect("dir");
    fs::write(datei.join("inhalt"), "x").expect("write");
    let fremd = dir.write("daneben.txt", b"fremd");
    assert!(Zaehlung::anlegen(datei.clone(), "s".into(), marker(&["A"])).is_err());
    let mut namen: Vec<String> = fs::read_dir(dir.path())
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    namen.sort();
    assert_eq!(namen, ["daneben.txt", "zaehlung.json"]);
    assert_eq!(fs::read_to_string(&fremd).expect("file"), "fremd");
    assert!(datei.join("inhalt").exists());
}
