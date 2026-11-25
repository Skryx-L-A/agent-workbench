//! The store (section 2.3): the rules of each level and of each file, in throw-away folders below
//! `CARGO_TARGET_TMPDIR`. Each rule is checked alone: everything else of the set-up is right.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use halter::ablage::{Ablage, EBENEN};

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

fn modus(p: &Path, m: u32) {
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).expect("mode");
}

struct Heim(PathBuf);

impl Drop for Heim {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn heim(mit_ebenen: bool) -> Heim {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let p =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ab-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("home");
    modus(&p, 0o700);
    if mit_ebenen {
        let mut g = p.clone();
        for e in EBENEN {
            g = g.join(e);
            std::fs::create_dir(&g).expect("level");
            modus(&g, 0o700);
        }
    }
    Heim(p)
}

fn geheim(h: &Heim) -> PathBuf {
    EBENEN.iter().fold(h.0.clone(), |p, e| p.join(e))
}

fn datei(h: &Heim, name: &str, inhalt: &[u8], m: u32) {
    let p = geheim(h).join(name);
    std::fs::write(&p, inhalt).expect("write");
    modus(&p, m);
}

fn wert(a: &Ablage, name: &str) -> Option<Vec<u8>> {
    a.lesen(name).map(|g| g.bytes().to_vec())
}

#[test]
fn gute_datei_wird_gelesen() {
    let h = heim(true);
    datei(&h, "anmeldung.copilot", b"hgx2-wert\n", 0o600);
    let a = Ablage::neu(&h.0, uid());
    assert_eq!(
        wert(&a, "anmeldung.copilot").as_deref(),
        Some(&b"hgx2-wert"[..])
    );
    assert_eq!(a.namen(), vec!["anmeldung.copilot".to_owned()]);
    assert_eq!(a.alle().len(), 1);
}

#[test]
fn dateiregeln_einzeln() {
    for m in [0o644, 0o640, 0o400, 0o700, 0o604, 0o4600] {
        let h = heim(true);
        datei(&h, "anmeldung.copilot", b"hgx2-wert\n", m);
        let a = Ablage::neu(&h.0, uid());
        assert_eq!(wert(&a, "anmeldung.copilot"), None, "mode {m:o}");
        assert!(a.alle().is_empty(), "mode {m:o}");
    }
    // another owner (the uid of the account does not match)
    let h = heim(true);
    datei(&h, "anmeldung.copilot", b"hgx2-wert\n", 0o600);
    let ab = Ablage::neu(&h.0, uid().wrapping_add(1));
    assert_eq!(wert(&ab, "anmeldung.copilot"), None);
    // a link to a good file, a folder, a FIFO
    let h = heim(true);
    let ziel = h.0.join("aussen");
    std::fs::write(&ziel, b"hgx2-wert\n").expect("write");
    modus(&ziel, 0o600);
    std::os::unix::fs::symlink(&ziel, geheim(&h).join("anmeldung.copilot")).expect("link");
    std::fs::create_dir(geheim(&h).join("mcp.ordner")).expect("dir");
    fifo(&geheim(&h).join("mcp.fifo"));
    let a = Ablage::neu(&h.0, uid());
    assert_eq!(wert(&a, "anmeldung.copilot"), None);
    assert_eq!(wert(&a, "mcp.ordner"), None);
    assert_eq!(wert(&a, "mcp.fifo"), None);
    assert!(a.alle().is_empty());
}

#[test]
fn inhaltsregeln_und_groesse() {
    let h = heim(true);
    let a = Ablage::neu(&h.0, uid());
    for (inhalt, soll) in [
        (b"w\n".to_vec(), Some(b"w".to_vec())),
        (b"w".to_vec(), None),
        (b"w\n\n".to_vec(), None),
        (b"\n".to_vec(), None),
        (b"a b\n".to_vec(), None),
    ] {
        datei(&h, "mcp.x", &inhalt, 0o600);
        assert_eq!(wert(&a, "mcp.x"), soll, "{inhalt:?}");
    }
    let mut gross = vec![b'a'; halter::ablage::DATEI_MAX - 1];
    gross.push(b'\n');
    datei(&h, "mcp.x", &gross, 0o600);
    assert_eq!(
        wert(&a, "mcp.x").map(|w| w.len()),
        Some(halter::ablage::DATEI_MAX - 1)
    );
    let mut zu_gross = vec![b'a'; halter::ablage::DATEI_MAX];
    zu_gross.push(b'\n');
    datei(&h, "mcp.x", &zu_gross, 0o600);
    assert_eq!(wert(&a, "mcp.x"), None);
}

#[test]
fn namen_ohne_punkt_vorn_und_nur_in_form() {
    let h = heim(true);
    for n in [
        "anmeldung.copilot",
        ".anmeldung.copilot.tmp",
        "fremd",
        "mcp.Gross",
        "modell.lokal",
    ] {
        datei(&h, n, b"w\n", 0o600);
    }
    let a = Ablage::neu(&h.0, uid());
    assert_eq!(
        a.namen(),
        vec!["anmeldung.copilot".to_owned(), "modell.lokal".to_owned()]
    );
    // a name with a slash or dots never leaves the folder
    datei(&h, "mcp.x", b"w\n", 0o600);
    assert_eq!(wert(&a, "../geheim/mcp.x"), None);
    assert_eq!(wert(&a, "mcp.x/.."), None);
    assert_eq!(wert(&a, ".."), None);
}

#[test]
fn ebenen_werden_mit_0700_angelegt_nur_wenn_verlangt() {
    let h = heim(false);
    let a = Ablage::neu(&h.0, uid());
    assert!(
        a.oeffnen(false).is_err(),
        "missing levels are not made without anlegen"
    );
    assert!(!h.0.join(".local").exists());
    assert!(a.oeffnen(true).is_ok());
    let mut p = h.0.clone();
    for e in EBENEN {
        p = p.join(e);
        let m = std::fs::symlink_metadata(&p).expect("level");
        assert!(m.is_dir());
        assert_eq!(m.permissions().mode() & 0o7777, 0o700, "{e}");
    }
    assert!(a.oeffnen(false).is_ok());
}

#[test]
fn ebenenregeln_einzeln() {
    for (i, e) in EBENEN.iter().enumerate() {
        for m in [0o770, 0o702, 0o720] {
            let h = heim(true);
            let p = EBENEN[..=i].iter().fold(h.0.clone(), |p, e| p.join(e));
            modus(&p, m);
            let a = Ablage::neu(&h.0, uid());
            assert!(a.oeffnen(true).is_err(), "{e} {m:o}");
            assert_eq!(a.namen(), Vec::<String>::new());
            modus(&p, 0o700);
        }
        // a level with 0755 is right except for `geheim`
        let h = heim(true);
        let p = EBENEN[..=i].iter().fold(h.0.clone(), |p, e| p.join(e));
        modus(&p, 0o755);
        let a = Ablage::neu(&h.0, uid());
        assert_eq!(a.oeffnen(false).is_ok(), *e != "geheim", "{e} 0755");
        modus(&p, 0o700);
        // a link as level
        let h = heim(true);
        let p = EBENEN[..=i].iter().fold(h.0.clone(), |p, e| p.join(e));
        let weg = h.0.join("weg");
        std::fs::rename(&p, &weg).expect("move");
        std::os::unix::fs::symlink(&weg, &p).expect("link");
        assert!(Ablage::neu(&h.0, uid()).oeffnen(true).is_err(), "{e} link");
    }
    // levels of another account
    let h = heim(true);
    assert!(
        Ablage::neu(&h.0, uid().wrapping_add(1))
            .oeffnen(false)
            .is_err()
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
