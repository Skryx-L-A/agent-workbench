//! Tests of the start file of step 2, the counting folder of step 5a and the new session files.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

fn ordner() -> PathBuf {
    let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
    let basis = std::fs::canonicalize(std::env::temp_dir()).expect("tmp");
    // a fresh root of this test alone with a marker; the folder handed out is one level below, so even a `..` that
    // slipped through the guards would stay inside the root. The test checks the marker before it goes on.
    let wurzel = basis.join(format!("lauf-dateien-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&wurzel);
    std::fs::create_dir(&wurzel).expect("root");
    std::fs::write(wurzel.join(".testwurzel"), b"x").expect("marker");
    assert!(wurzel.join(".testwurzel").is_file());
    let p = wurzel.join("arbeit");
    std::fs::create_dir(&p).expect("folder");
    p
}

#[test]
fn the_rule_of_step_2_takes_kind_owner_and_mode() {
    assert!(start_modus_gut(FileType::RegularFile, 5, 0o100_600, 5));
    assert!(!start_modus_gut(FileType::RegularFile, 6, 0o100_600, 5));
    assert!(!start_modus_gut(FileType::Fifo, 5, 0o010_600, 5));
    assert!(!start_modus_gut(FileType::RegularFile, 5, 0o100_644, 5));
    assert!(!start_modus_gut(FileType::RegularFile, 5, 0o104_600, 5));
    assert!(!start_modus_gut(FileType::RegularFile, 5, 0o100_400, 5));
}

#[test]
fn the_rule_of_step_5a_takes_kind_owner_and_mode() {
    assert!(zaehlordner_gut(FileType::Directory, 5, 0o040_700, 5));
    assert!(!zaehlordner_gut(FileType::Directory, 6, 0o040_700, 5));
    assert!(!zaehlordner_gut(FileType::RegularFile, 5, 0o100_700, 5));
    assert!(!zaehlordner_gut(FileType::Directory, 5, 0o040_755, 5));
    assert!(!zaehlordner_gut(FileType::Directory, 5, 0o041_700, 5));
}

#[test]
fn the_start_file_is_read_only_when_it_is_an_own_file_of_mode_0600() {
    let o = ordner();
    let datei = o.join("start.json");
    std::fs::write(&datei, b"{}").expect("write");
    std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o600)).expect("mode");
    assert_eq!(startdatei_lesen(&datei), Ok(b"{}".to_vec()));
    std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o644)).expect("mode");
    assert_eq!(startdatei_lesen(&datei), Err(Fehler::DateiModus));
    std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o600)).expect("mode");
    let verweis = o.join("verweis.json");
    std::os::unix::fs::symlink(&datei, &verweis).expect("link");
    assert_eq!(startdatei_lesen(&verweis), Err(Fehler::DateiModus));
    assert_eq!(
        startdatei_lesen(&o.join("fehlt.json")),
        Err(Fehler::DateiUnlesbar)
    );
    assert_eq!(startdatei_lesen(&o), Err(Fehler::DateiModus), "a folder");
    // a FIFO without a writer: opened without waiting (O_NONBLOCK) and refused by its kind; a hang fails within the limit
    let fifo = o.join("fifo.json");
    let st = std::process::Command::new("mkfifo")
        .args(["-m", "600"])
        .arg(&fifo)
        .status()
        .expect("mkfifo");
    assert!(st.success());
    let r = crate::testhilfe::mit_frist(std::time::Duration::from_secs(10), move || {
        startdatei_lesen(&fifo)
    });
    assert_eq!(r, Err(Fehler::DateiModus), "a FIFO");
    // a long file is cut after 4097 bytes, so stage 1 sees "too large"
    std::fs::write(&datei, vec![b' '; 10_000]).expect("write");
    assert_eq!(startdatei_lesen(&datei).map(|t| t.len()), Ok(4097));
    let _ = std::fs::remove_dir_all(&o);
}

#[test]
fn the_counting_folder_must_be_an_own_real_folder_of_mode_0700() {
    let o = ordner();
    let z = o.join("zaehlung");
    std::fs::create_dir(&z).expect("folder");
    std::fs::set_permissions(&z, std::fs::Permissions::from_mode(0o700)).expect("mode");
    assert_eq!(zaehlordner_pruefen(&z), Ok(()));
    std::fs::set_permissions(&z, std::fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(zaehlordner_pruefen(&z), Err(Fehler::MarkerOrdner));
    std::fs::set_permissions(&z, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let v = o.join("verweis");
    std::os::unix::fs::symlink(&z, &v).expect("link");
    assert_eq!(zaehlordner_pruefen(&v), Err(Fehler::MarkerOrdner));
    assert_eq!(
        zaehlordner_pruefen(&o.join("fehlt")),
        Err(Fehler::MarkerOrdner)
    );
    let d = o.join("datei");
    std::fs::write(&d, b"").expect("write");
    assert_eq!(zaehlordner_pruefen(&d), Err(Fehler::MarkerOrdner));
    let _ = std::fs::remove_dir_all(&o);
}

#[test]
fn a_session_file_is_new_and_0600_and_a_folder_is_new_and_0700() {
    let o = ordner();
    let d = o.join("a.pid");
    datei_neu(&d, b"7\n").expect("new");
    assert_eq!(std::fs::read(&d).expect("read"), b"7\n");
    assert_eq!(
        std::fs::metadata(&d).expect("meta").permissions().mode() & 0o7777,
        0o600
    );
    assert!(
        datei_neu(&d, b"8\n").is_err(),
        "never over an existing file"
    );
    let v = o.join("v.pid");
    std::os::unix::fs::symlink(o.join("ziel"), &v).expect("link");
    assert!(datei_neu(&v, b"8\n").is_err(), "never through a link");
    assert!(!o.join("ziel").exists());
    let n = o.join("neu");
    ordner_neu(&n).expect("new folder");
    assert_eq!(
        std::fs::metadata(&n).expect("meta").permissions().mode() & 0o7777,
        0o700
    );
    assert!(ordner_neu(&n).is_err(), "an existing folder is a fault");
    let _ = std::fs::remove_dir_all(&o);
}

#[test]
fn a_folder_is_removed_with_everything_in_it() {
    let w = ordner();
    let o = w.join("sitzung");
    std::fs::create_dir_all(o.join("zustand/tief")).expect("tree");
    std::fs::write(o.join("zustand/tief/a.txt"), b"a").expect("file");
    std::fs::write(o.join("b.txt"), b"b").expect("file");
    ordner_entfernen(&o).expect("removed");
    assert!(!o.exists());
    assert!(w.is_dir(), "the parent stays");
}

#[test]
fn a_link_in_the_place_of_the_folder_is_left_alone() {
    let w = ordner();
    let fremd = w.join("fremd");
    std::fs::create_dir(&fremd).expect("folder");
    std::fs::write(fremd.join("x.txt"), b"x").expect("file");
    let l = w.join("sitzung");
    std::os::unix::fs::symlink(&fremd, &l).expect("link");
    assert!(ordner_entfernen(&l).is_err());
    assert!(fremd.join("x.txt").is_file());
    assert!(
        std::fs::symlink_metadata(&l)
            .expect("link")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn a_link_inside_the_folder_is_removed_as_a_link_and_not_followed() {
    let w = ordner();
    let fremd = w.join("fremd");
    std::fs::create_dir(&fremd).expect("folder");
    std::fs::write(fremd.join("x.txt"), b"x").expect("file");
    let o = w.join("sitzung");
    std::fs::create_dir(&o).expect("folder");
    std::os::unix::fs::symlink(&fremd, o.join("verweis")).expect("link");
    std::os::unix::fs::symlink(w.join("nirgends"), o.join("lose")).expect("dangling");
    ordner_entfernen(&o).expect("removed");
    assert!(!o.exists());
    assert!(
        fremd.join("x.txt").is_file(),
        "the target of the link is untouched"
    );
}

#[test]
fn a_file_or_a_missing_name_is_an_error_and_a_file_stays() {
    let w = ordner();
    let f = w.join("datei");
    std::fs::write(&f, b"x").expect("file");
    assert!(ordner_entfernen(&f).is_err());
    assert!(f.is_file());
    assert!(ordner_entfernen(&w.join("fehlt")).is_err());
    assert!(ordner_entfernen(Path::new("/")).is_err());
}

#[test]
fn a_tree_deeper_than_the_limit_is_an_error_and_what_is_above_it_is_removed() {
    let w = ordner();
    let mut p = w.join("sitzung");
    for _ in 0..40 {
        p = p.join("e");
    }
    std::fs::create_dir_all(&p).expect("tree");
    assert!(ordner_entfernen(&w.join("sitzung")).is_err());
    assert!(
        w.join("sitzung").is_dir(),
        "the folder stays when something below it stays"
    );
    let _ = std::fs::remove_dir_all(w.join("sitzung"));
}

/// `.` and `..` are never entered, whatever the listing says: the call is refused and nothing above is removed. The
/// folder `aussen` stands for the parent; the test never runs the call with a folder whose parent is shared.
#[test]
fn the_names_dot_and_dot_dot_and_a_slash_are_refused() {
    let w = ordner();
    let aussen = w.join("aussen");
    let innen = aussen.join("innen");
    std::fs::create_dir_all(&innen).expect("tree");
    std::fs::write(aussen.join("bleibt.txt"), b"x").expect("file");
    let fd = rustix::fs::open(
        &innen,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .expect("open");
    for n in [c"..", c".", c"a/b"] {
        assert!(entfernen_in(&fd, n, 0).is_err(), "{n:?}");
    }
    assert!(aussen.join("bleibt.txt").is_file());
    assert!(innen.is_dir());
}

/// The open that the removal uses refuses a link and a file even when the stat before it was fooled (a name swapped in
/// between): the flags `O_NOFOLLOW` and `O_DIRECTORY` are what the test sees.
#[test]
fn the_open_below_a_descriptor_refuses_a_link_and_a_file() {
    let w = ordner();
    std::fs::create_dir(w.join("echt")).expect("folder");
    std::fs::write(w.join("datei"), b"x").expect("file");
    std::os::unix::fs::symlink(w.join("echt"), w.join("verweis")).expect("link");
    let fd = rustix::fs::open(
        &w,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .expect("open");
    assert!(unterordner_oeffnen(&fd, c"echt").is_ok());
    assert!(unterordner_oeffnen(&fd, c"verweis").is_err(), "a link");
    assert!(unterordner_oeffnen(&fd, c"datei").is_err(), "a file");
}
