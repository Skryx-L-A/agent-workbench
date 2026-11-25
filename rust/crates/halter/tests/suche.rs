//! `modell.suche` without the server (contract 8, section 9a, S3 and S4): what is read, skipped or not readable, and how
//! hits are counted, in throw-away folders below `CARGO_TARGET_TMPDIR`.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use halter::suche::{Ergebnis, TIEFE_MAX, durchsuchen};

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
const W: &[u8] = b"hgx3-wert";

struct Ordner(PathBuf);

impl Drop for Ordner {
    fn drop(&mut self) {
        // folders made unreadable by a test are opened again first
        let _ = std::process::Command::new("chmod")
            .arg("-R")
            .arg("u+rwx")
            .arg(&self.0)
            .status();
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ordner() -> Ordner {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let p =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("su-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("folder");
    Ordner(p)
}

fn e(dateien: u64, nicht_lesbar: u64, treffer: u64, uebersprungen: u64) -> Ergebnis {
    Ergebnis {
        dateien,
        nicht_lesbar,
        treffer,
        uebersprungen,
    }
}

fn suche(wurzeln: &[&Path], puffer: usize) -> Ergebnis {
    durchsuchen(W, wurzeln, puffer)
}

#[test]
fn zaehlt_ueber_lesegrenzen_und_ohne_ueberlappung() {
    let o = ordner();
    std::fs::write(o.0.join("a"), [W, b" x ", W, W].concat()).expect("write");
    std::fs::write(o.0.join("b"), b"hgx3-wer hgx3-werthgx3-wert").expect("write");
    std::fs::write(o.0.join("c"), b"").expect("write");
    for puffer in [1, 2, 3, 7, 65_536] {
        assert_eq!(suche(&[&o.0], puffer), e(3, 0, 5, 0), "buffer {puffer}");
    }
    let o2 = ordner();
    std::fs::write(o2.0.join("a"), b"aaaaa").expect("write");
    assert_eq!(durchsuchen(b"aa", &[&o2.0], 1).treffer, 2);
    std::fs::write(o2.0.join("a"), b"abababab").expect("write");
    assert_eq!(durchsuchen(b"abab", &[&o2.0], 3).treffer, 2);
    std::fs::write(o2.0.join("a"), b"aabaabaaab").expect("write");
    assert_eq!(durchsuchen(b"aaab", &[&o2.0], 2).treffer, 1);
    std::fs::write(o2.0.join("a"), b"abcabcabd").expect("write");
    assert_eq!(durchsuchen(b"abcabd", &[&o2.0], 1).treffer, 1);
    std::fs::write(o2.0.join("a"), b"aabaacaabaab").expect("write");
    assert_eq!(durchsuchen(b"aabaab", &[&o2.0], 1).treffer, 1);
    assert_eq!(durchsuchen(b"", &[&o2.0], 1).treffer, 0);
}

#[test]
fn verweise_und_andere_arten_werden_uebersprungen() {
    let o = ordner();
    std::fs::write(o.0.join("echt"), W).expect("write");
    std::os::unix::fs::symlink(o.0.join("echt"), o.0.join("verweis")).expect("link");
    let aussen = ordner();
    std::fs::write(aussen.0.join("x"), W).expect("write");
    std::os::unix::fs::symlink(&aussen.0, o.0.join("ordnerverweis")).expect("link");
    fifo(&o.0.join("fifo"));
    let _sock = std::os::unix::net::UnixListener::bind(o.0.join("s"));
    let u = suche(&[&o.0], 4096);
    assert_eq!((u.dateien, u.treffer, u.nicht_lesbar), (1, 1, 0));
    assert!(u.uebersprungen >= 3, "{u:?}");
}

#[test]
fn wurzeln_datei_verweis_fehlt_doppelt() {
    let o = ordner();
    std::fs::write(o.0.join("a"), W).expect("write");
    std::fs::create_dir(o.0.join("u")).expect("dir");
    std::fs::write(o.0.join("u/b"), W).expect("write");
    assert_eq!(suche(&[&o.0.join("a")], 4096), e(1, 0, 1, 0));
    std::os::unix::fs::symlink(o.0.join("a"), o.0.join("l")).expect("link");
    assert_eq!(suche(&[&o.0.join("l")], 4096), e(0, 1, 0, 0));
    assert_eq!(suche(&[&o.0.join("fehlt")], 4096), e(0, 1, 0, 0));
    // nested and twice: every file once; the link `l` is skipped at each visit of its folder
    assert_eq!(
        suche(&[&o.0, &o.0.join("u"), &o.0, &o.0.join("a")], 4096),
        e(2, 0, 2, 2)
    );
    fifo(&o.0.join("fifo"));
    assert_eq!(suche(&[&o.0.join("fifo")], 4096), e(0, 1, 0, 0));
}

#[test]
fn nicht_lesbares_zaehlt_und_wird_nicht_betreten() {
    if rustix::process::geteuid().is_root() {
        return; // root reads everything; the rule is about the rights of the person
    }
    let o = ordner();
    std::fs::write(o.0.join("zu"), W).expect("write");
    std::fs::set_permissions(o.0.join("zu"), std::fs::Permissions::from_mode(0o000)).expect("mode");
    std::fs::create_dir(o.0.join("dicht")).expect("dir");
    std::fs::write(o.0.join("dicht/x"), W).expect("write");
    std::fs::set_permissions(o.0.join("dicht"), std::fs::Permissions::from_mode(0o000))
        .expect("mode");
    std::fs::write(o.0.join("offen"), W).expect("write");
    assert_eq!(suche(&[&o.0], 4096), e(1, 2, 1, 0));
    assert_eq!(suche(&[&o.0.join("dicht")], 4096), e(0, 1, 0, 0));
}

#[test]
fn tiefe_hoechstens_256() {
    let o = ordner();
    let mut p = o.0.clone();
    for _ in 0..TIEFE_MAX {
        p = p.join("d");
    }
    std::fs::create_dir_all(&p).expect("deep");
    std::fs::write(p.join("am-rand"), W).expect("write");
    assert_eq!(
        suche(&[&o.0], 4096),
        e(1, 0, 1, 0),
        "depth 256 is still read"
    );
    std::fs::create_dir(p.join("d")).expect("dir");
    std::fs::write(p.join("d/zu-tief"), W).expect("write");
    assert_eq!(
        suche(&[&o.0], 4096),
        e(1, 1, 1, 0),
        "depth 257 is not entered"
    );
}

fn fifo(p: &Path) {
    let st = std::process::Command::new("mkfifo")
        .arg("-m")
        .arg("600")
        .arg(p)
        .status()
        .expect("mkfifo");
    assert!(st.success(), "mkfifo");
}
