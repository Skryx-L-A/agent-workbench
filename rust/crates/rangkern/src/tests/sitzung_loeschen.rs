//! The folder-relative calls of `rangkern sitzung`: a single name, the identity of what the code made, and no link followed.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;

use rustix::fs::{FileType, Mode};

use super::TempDir;
use crate::sitzung::loeschen::{Eigene, Kennung, Ordner, einfacher_name};

fn modus(pfad: &std::path::Path) -> u32 {
    fs::metadata(pfad).expect("meta").permissions().mode() & 0o7777
}

#[test]
fn a_single_name_is_not_empty_has_no_slash_and_is_no_dot() {
    for gut in ["a", "a.b", "..a", "a..", ".a.1.neu"] {
        assert!(einfacher_name(OsStr::new(gut)), "{gut:?}");
    }
    for schlecht in ["", "a/b", "/a", "a/", ".", "..", "../a"] {
        assert!(!einfacher_name(OsStr::new(schlecht)), "{schlecht:?}");
    }
}

#[test]
fn an_own_file_is_deleted_by_name_in_the_folder() {
    let dir = TempDir::new();
    let pfad = dir.path().join("f");
    let (o, name) = Ordner::von(&pfad).expect("folder");
    let datei = o
        .neu_anlegen(&name, Mode::from_raw_mode(0o600))
        .expect("make");
    let soll = Kennung::von_datei(&datei).expect("fstat");
    assert!(soll.ist(FileType::RegularFile));
    assert!(pfad.exists());
    assert!(o.entfernen_wenn(&name, &soll));
    assert!(!pfad.exists());
    assert!(!o.entfernen_wenn(&name, &soll), "nothing is there any more");
}

#[test]
fn another_file_a_link_to_the_own_file_or_a_folder_under_the_name_stays() {
    let dir = TempDir::new();
    let aussen = dir.write("aussen", b"x");
    let pfad = dir.path().join("f");
    let (o, name) = Ordner::von(&pfad).expect("folder");
    let datei = o
        .neu_anlegen(&name, Mode::from_raw_mode(0o600))
        .expect("make");
    let soll = Kennung::von_datei(&datei).expect("fstat");
    // The own file moves away; a link to it takes its name. Followed, the link would show the own identity.
    let weg = dir.path().join("weg");
    fs::rename(&pfad, &weg).expect("move");
    symlink(&weg, &pfad).expect("link");
    assert!(!o.entfernen_wenn(&name, &soll));
    assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
    assert!(weg.exists());
    // Another file with another inode (the own one stays open, so its inode is not used again).
    fs::remove_file(&pfad).expect("remove");
    fs::write(&pfad, "fremd").expect("write");
    assert!(!o.entfernen_wenn(&name, &soll));
    assert_eq!(fs::read_to_string(&pfad).expect("read"), "fremd");
    // A link to a file outside: the link stays and so does its target.
    fs::remove_file(&pfad).expect("remove");
    symlink(&aussen, &pfad).expect("link");
    assert!(!o.entfernen_wenn(&name, &soll));
    assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
    assert_eq!(fs::read_to_string(&aussen).expect("read"), "x");
    // A folder.
    fs::remove_file(&pfad).expect("remove");
    fs::create_dir(&pfad).expect("folder");
    assert!(!o.entfernen_wenn(&name, &soll));
    assert!(pfad.is_dir());
}

#[test]
fn a_name_with_a_slash_a_dot_or_nothing_deletes_nothing() {
    let dir = TempDir::new();
    let tief = dir.write("unter/t", b"x");
    let (o, _) = Ordner::von(&dir.path().join("f")).expect("folder");
    let soll = Kennung::von_datei(&fs::File::open(&tief).expect("open")).expect("fstat");
    for schlecht in ["unter/t", "./unter/t", "", ".", ".."] {
        assert!(
            !o.entfernen_wenn(OsStr::new(schlecht), &soll),
            "{schlecht:?}"
        );
        assert!(o.kennung(OsStr::new(schlecht)).is_none(), "{schlecht:?}");
        assert!(
            o.rechte_setzen_wenn(OsStr::new(schlecht), &soll, Mode::from_raw_mode(0o666))
                .is_err(),
            "{schlecht:?}"
        );
    }
    assert!(tief.exists());
    assert!(
        o.neu_anlegen(OsStr::new("unter/n"), Mode::from_raw_mode(0o600))
            .is_err(),
        "a name with a slash makes nothing"
    );
    assert!(!dir.path().join("unter/n").exists());
    for (von, nach) in [("unter/t", "z"), ("a", "unter/z"), ("a", ".."), ("..", "a")] {
        assert!(
            o.umbenennen_wenn(OsStr::new(von), OsStr::new(nach), &soll)
                .is_err(),
            "{von:?} {nach:?}"
        );
    }
    assert!(tief.exists());
    // The path itself must end in a single name.
    for pfad in [dir.path().join(".."), PathBuf::from("/"), PathBuf::new()] {
        assert!(Ordner::von(&pfad).is_err(), "{pfad:?}");
    }
}

#[test]
fn a_folder_that_is_a_link_is_not_opened() {
    let dir = TempDir::new();
    fs::create_dir(dir.path().join("echt")).expect("folder");
    symlink(dir.path().join("echt"), dir.path().join("lnk")).expect("link");
    assert!(Ordner::von(&dir.path().join("lnk").join("f")).is_err());
    assert!(Ordner::von(&dir.path().join("echt").join("f")).is_ok());
}

#[test]
fn a_name_is_remembered_only_for_the_kind_that_was_made() {
    let dir = TempDir::new();
    dir.write("f", b"x");
    let (o, name) = Ordner::von(&dir.path().join("f")).expect("folder");
    assert!(Eigene::merken(o, name, FileType::Socket).is_none());
    let (o, name) = Ordner::von(&dir.path().join("g")).expect("folder");
    assert!(
        Eigene::merken(o, name, FileType::RegularFile).is_none(),
        "nothing is there"
    );
    let (o, name) = Ordner::von(&dir.path().join("f")).expect("folder");
    let eigene = Eigene::merken(o, name, FileType::RegularFile).expect("remember");
    assert!(eigene.entfernen());
    assert!(!dir.path().join("f").exists());
    assert!(!eigene.entfernen());
}

#[test]
fn a_rename_goes_relative_to_the_folder_and_only_for_the_own_file() {
    let dir = TempDir::new();
    let (o, _) = Ordner::von(&dir.path().join("x")).expect("folder");
    let datei = o
        .neu_anlegen(OsStr::new("a"), Mode::from_raw_mode(0o600))
        .expect("make");
    let soll = Kennung::von_datei(&datei).expect("fstat");
    assert!(
        o.neu_anlegen(OsStr::new("a"), Mode::from_raw_mode(0o600))
            .is_err(),
        "O_EXCL: an existing name is not made again"
    );
    // The name shows a link instead of the own file: no rename.
    let weg = dir.path().join("weg");
    fs::rename(dir.path().join("a"), &weg).expect("move");
    symlink(&weg, dir.path().join("a")).expect("link");
    assert!(
        o.umbenennen_wenn(OsStr::new("a"), OsStr::new("b"), &soll)
            .is_err()
    );
    assert!(!dir.path().join("b").exists());
    fs::remove_file(dir.path().join("a")).expect("remove");
    fs::rename(&weg, dir.path().join("a")).expect("back");
    o.umbenennen_wenn(OsStr::new("a"), OsStr::new("b"), &soll)
        .expect("rename");
    assert!(!dir.path().join("a").exists());
    assert!(dir.path().join("b").exists());
}

#[test]
fn the_rights_are_set_on_the_own_entry_and_on_nothing_a_link_leads_to() {
    let dir = TempDir::new();
    let fremd = dir.write("fremd", b"x");
    fs::set_permissions(&fremd, fs::Permissions::from_mode(0o600)).expect("chmod");
    let pfad = dir.path().join("f");
    let (o, name) = Ordner::von(&pfad).expect("folder");
    let datei = o
        .neu_anlegen(&name, Mode::from_raw_mode(0o600))
        .expect("make");
    let soll = Kennung::von_datei(&datei).expect("fstat");
    o.rechte_setzen_wenn(&name, &soll, Mode::from_raw_mode(0o640))
        .expect("chmod");
    assert_eq!(modus(&pfad), 0o640);
    // A link to a foreign file under the name: the target keeps its mode.
    fs::remove_file(&pfad).expect("remove");
    symlink(&fremd, &pfad).expect("link");
    assert!(
        o.rechte_setzen_wenn(&name, &soll, Mode::from_raw_mode(0o666))
            .is_err()
    );
    assert_eq!(modus(&fremd), 0o600);
    // A link to the own file, moved away: still no change.
    fs::remove_file(&pfad).expect("remove");
    let weg = dir.path().join("weg");
    fs::write(&weg, "y").expect("write");
    fs::set_permissions(&weg, fs::Permissions::from_mode(0o600)).expect("chmod");
    symlink(&weg, &pfad).expect("link");
    assert!(
        o.rechte_setzen_wenn(&name, &soll, Mode::from_raw_mode(0o666))
            .is_err()
    );
    assert_eq!(modus(&weg), 0o600);
    // The same with the own file itself moved away: followed, the link would show the own identity.
    fs::remove_file(&pfad).expect("remove");
    fs::remove_file(&weg).expect("remove");
    let datei2 = o
        .neu_anlegen(&name, Mode::from_raw_mode(0o600))
        .expect("make");
    let soll2 = Kennung::von_datei(&datei2).expect("fstat");
    fs::rename(&pfad, &weg).expect("move");
    symlink(&weg, &pfad).expect("link");
    assert!(
        o.rechte_setzen_wenn(&name, &soll2, Mode::from_raw_mode(0o666))
            .is_err()
    );
    assert_eq!(modus(&weg), 0o600, "the moved own file keeps its mode");
    // Another file under the name.
    fs::remove_file(&pfad).expect("remove");
    fs::write(&pfad, "z").expect("write");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("chmod");
    assert!(
        o.rechte_setzen_wenn(&name, &soll, Mode::from_raw_mode(0o666))
            .is_err()
    );
    assert_eq!(modus(&pfad), 0o600);
}
