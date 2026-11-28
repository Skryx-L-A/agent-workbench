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
    // a good value with its LF within the limit and one byte more after it: the file is too big, the reader may not
    // stop at the limit and keep the front
    let mut mit_rest = gross.clone();
    mit_rest.push(b'b');
    datei(&h, "mcp.x", &mit_rest, 0o600);
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

/// The gid of a level as the file system made it (the gid of this process or of a folder with set-gid above).
fn gid_von(p: &Path) -> u32 {
    std::os::unix::fs::MetadataExt::gid(&std::fs::symlink_metadata(p).expect("stat"))
}

/// The decision of the operator of 28.11.2026: with the private group, the levels above `geheim` may be 0775 (umask
/// 002); `geheim` stays 0700, a file stays 0600, and the write of others stays refused. Counter-check: the same levels
/// without the group, or with another gid, are refused.
#[test]
fn private_gruppe_erlaubt_gruppenschreibrecht_nur_ueber_geheim() {
    let h = heim(true);
    datei(&h, "anmeldung.copilot", b"hgx2-wert\n", 0o600);
    let mut p = h.0.clone();
    for e in &EBENEN[..4] {
        p = p.join(e);
        modus(&p, 0o775);
    }
    let gid = gid_von(&h.0.join(".local"));
    let mit = Ablage::neu(&h.0, uid()).mit_gruppe(Some(gid));
    assert!(mit.oeffnen(false).is_ok());
    assert_eq!(
        wert(&mit, "anmeldung.copilot").as_deref(),
        Some(&b"hgx2-wert"[..])
    );
    // counter-check: no private group, or another gid
    assert!(Ablage::neu(&h.0, uid()).oeffnen(false).is_err());
    let anders = Ablage::neu(&h.0, uid()).mit_gruppe(Some(gid.wrapping_add(1)));
    assert!(anders.oeffnen(false).is_err());
    assert_eq!(wert(&anders, "anmeldung.copilot"), None);
    // the file 0660 is refused even with the group
    datei(&h, "anmeldung.copilot", b"hgx2-wert\n", 0o660);
    assert_eq!(wert(&mit, "anmeldung.copilot"), None);
    datei(&h, "anmeldung.copilot", b"hgx2-wert\n", 0o600);
    // geheim 0770 or 0750 is refused even with the group
    for m in [0o770, 0o750] {
        modus(&geheim(&h), m);
        assert!(mit.oeffnen(false).is_err(), "geheim {m:o}");
        assert_eq!(wert(&mit, "anmeldung.copilot"), None, "geheim {m:o}");
    }
    modus(&geheim(&h), 0o700);
    assert!(mit.oeffnen(false).is_ok());
    // the write of others at any level above is refused even with the group
    let mut p = h.0.clone();
    for e in &EBENEN[..4] {
        p = p.join(e);
        for m in [0o777, 0o757, 0o702] {
            modus(&p, m);
            assert!(mit.oeffnen(false).is_err(), "{e} {m:o}");
        }
        modus(&p, 0o775);
    }
    assert!(mit.oeffnen(false).is_ok());
}

/// Step 3 with the files of the accounts and the groups of a set-up: the private group lets the start take 0775 above
/// `geheim`; a second member, a second account with that primary group, or a file of the groups that cannot be read
/// refuses (exit 78 of the start). The start exists only on Linux (`src/lib.rs`), so this test runs where it is built.
#[cfg(target_os = "linux")]
#[test]
fn schritt_3_mit_privater_gruppe_aus_passwd_und_group() {
    let h = heim(true);
    let mut p = h.0.clone();
    for e in &EBENEN[..4] {
        p = p.join(e);
        modus(&p, 0o775);
    }
    let gid = gid_von(&h.0.join(".local"));
    let konten = format!(
        "root:x:0:0:root:/root:/bin/sh\nperson:x:{}:{gid}::{}:/bin/sh\n",
        uid(),
        h.0.display()
    );
    let gut = format!("root:x:0:\nperson:x:{gid}:\n");
    let wurzel = PathBuf::from(format!("{}-etc", h.0.display()));
    let _ = std::fs::remove_dir_all(&wurzel);
    std::fs::create_dir(&wurzel).expect("etc");
    let (pw, gr) = (wurzel.join("passwd"), wurzel.join("group"));
    let start = |konten: &str, gruppen: Option<&str>| {
        std::fs::write(&pw, konten).expect("passwd");
        let _ = std::fs::remove_file(&gr);
        if let Some(g) = gruppen {
            std::fs::write(&gr, g).expect("group");
        }
        halter::start::ablage_pruefen(&pw, &gr, uid()).map(|_| ())
    };
    assert_eq!(start(&konten, Some(&gut)), Ok(()));
    let rechte = Err(halter::code::Startfehler::AblageRechte);
    let zweites = format!("root:x:0:\nperson:x:{gid}:andere\n");
    assert_eq!(start(&konten, Some(&zweites)), rechte, "second member");
    let zwei_konten = format!(
        "{konten}andere:x:{}:{gid}::/heim/beispiel/andere:/bin/sh\n",
        uid().wrapping_add(1)
    );
    assert_eq!(start(&zwei_konten, Some(&gut)), rechte, "second account");
    assert_eq!(start(&konten, None), rechte, "no file of the groups");
    // a file of the groups that is a folder cannot be read either
    std::fs::create_dir(&gr).expect("folder");
    assert_eq!(
        halter::start::ablage_pruefen(&pw, &gr, uid()).map(|_| ()),
        rechte,
        "group is a folder"
    );
    std::fs::remove_dir(&gr).expect("rmdir");
    // the levels 0700 need no group: the start takes them without a file of the groups
    let mut p = h.0.clone();
    for e in &EBENEN[..4] {
        p = p.join(e);
        modus(&p, 0o700);
    }
    assert_eq!(start(&konten, None), Ok(()));
    let _ = std::fs::remove_dir_all(&wurzel);
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
