//! The start steps 2 to 4 as functions of the library, in every build (also without the feature `pruefbau`, which the
//! tests of the program in `start.rs` need): the rules of `halter.json` and of the files of groups and accounts, the
//! folder of the chain next to the store, the endpoint with its folder and the removal of a dead own socket, and the
//! removal of the own socket at the end. Throw-away folders lie below `CARGO_TARGET_TMPDIR`, sockets below a short
//! folder (`TMPDIR` if short, else `$HOME/.cache`).
#![cfg(target_os = "linux")]

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use halter::code::Startfehler;
use halter::start::{self, KONTEN_MAX};
use serde_json::{Value, json};

static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
/// The gid of the role group `wb-bruecke` in the file of the groups: no gid of this test process.
const FREMDE_GID: u32 = 3_999_999_999;
/// The primary group of the person in the file of the accounts: never a role group.
const HAUPTGRUPPE: u32 = 3_999_999_998;

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

fn modus(p: &Path, m: u32) {
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).expect("mode");
}

/// A throw-away folder, removed at the end of the test.
struct Ordner(PathBuf);

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ordner() -> Ordner {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let p =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("al-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("folder");
    modus(&p, 0o700);
    Ordner(p)
}

/// A short folder for a socket (a Unix socket path has at most 107 bytes).
fn kurzer_ordner() -> Ordner {
    let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
    let basis = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.as_os_str().len() <= 48)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".cache"));
    let p = basis.join(format!("al-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("short folder");
    modus(&p, 0o700);
    Ordner(p)
}

fn standard() -> Value {
    json!({"fassung": 1, "konten": {uid().to_string(): {"person": null}}, "rollen": {"bruecke": "wb-bruecke"},
        "ziele": {"copilot": {"host": "127.0.0.1", "port": 9, "tls": false, "praefix": "", "kopf": "authorization",
            "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot", "pfade": ["GET /*"]}},
        "server": {}, "kids": {}})
}

/// `etc/halter.json` (folder 0755, file 0644), `group` and `passwd` of a set-up below `w`.
struct Konfig {
    datei: PathBuf,
    gruppen: PathBuf,
    passwd: PathBuf,
}

fn konfig_in(w: &Path, inhalt: &[u8]) -> Konfig {
    let etc = w.join("etc");
    std::fs::create_dir(&etc).expect("etc");
    modus(&etc, 0o755);
    let datei = etc.join("halter.json");
    std::fs::write(&datei, inhalt).expect("konfig");
    modus(&datei, 0o644);
    let gruppen = w.join("group");
    std::fs::write(&gruppen, format!("root:x:0:\nwb-bruecke:x:{FREMDE_GID}:\n")).expect("group");
    let passwd = w.join("passwd");
    std::fs::write(
        &passwd,
        format!(
            "root:x:0:0:root:/root:/bin/sh\nperson:x:{}:{HAUPTGRUPPE}::{}:/bin/sh\n",
            uid(),
            w.join("heim").display()
        ),
    )
    .expect("passwd");
    Konfig {
        datei,
        gruppen,
        passwd,
    }
}

fn lesen(k: &Konfig, eigner: u32) -> Result<(), Startfehler> {
    start::konfig_lesen(&k.datei, eigner, uid(), &k.gruppen, &k.passwd).map(|(konfig, rollen)| {
        assert!(konfig.konten.contains_key(&uid().to_string()));
        assert_eq!(rollen.len(), 1);
        assert!(rollen.contains_key(&FREMDE_GID));
    })
}

#[test]
fn schritt_2_regeln_von_datei_und_ordner() {
    let w = ordner();
    let k = konfig_in(&w.0, standard().to_string().as_bytes());
    assert_eq!(lesen(&k, uid()), Ok(()));
    // another owner than the expected one
    assert_eq!(lesen(&k, uid().wrapping_add(1)), Err(Startfehler::Konfig));
    // the file writable for the group or others
    for m in [0o664, 0o646] {
        modus(&k.datei, m);
        assert_eq!(lesen(&k, uid()), Err(Startfehler::Konfig), "{m:o}");
    }
    modus(&k.datei, 0o644);
    // the folder writable for the group or others
    for m in [0o775, 0o757] {
        modus(k.datei.parent().expect("etc"), m);
        assert_eq!(lesen(&k, uid()), Err(Startfehler::Konfig), "{m:o}");
    }
    modus(k.datei.parent().expect("etc"), 0o755);
    assert_eq!(lesen(&k, uid()), Ok(()));
    // a link instead of the file
    let echt = w.0.join("echt.json");
    std::fs::rename(&k.datei, &echt).expect("rename");
    std::os::unix::fs::symlink(&echt, &k.datei).expect("link");
    assert_eq!(lesen(&k, uid()), Err(Startfehler::Konfig));
}

/// `halter.json` is read up to its limit: a file of exactly `GROESSE_MAX` bytes counts whole (the closing brace is its
/// last byte), one byte more is too big.
#[test]
fn schritt_2_groesse_der_konfig() {
    let gut = standard().to_string();
    let max = halter::konfig::GROESSE_MAX;
    let w = ordner();
    let genau = format!("{}{gut}", " ".repeat(max - gut.len()));
    assert_eq!(genau.len(), max);
    let k = konfig_in(&w.0, genau.as_bytes());
    assert_eq!(lesen(&k, uid()), Ok(()));
    let w = ordner();
    let zu_gross = format!("{gut}{}", " ".repeat(max + 1 - gut.len()));
    let k = konfig_in(&w.0, zu_gross.as_bytes());
    assert_eq!(lesen(&k, uid()), Err(Startfehler::Konfig));
}

/// The files of groups and accounts: read whole up to `KONTEN_MAX` bytes, one byte more is refused.
#[test]
fn schritt_2_text_bis_konten_max() {
    let w = ordner();
    let p = w.0.join("group");
    let max = usize::try_from(KONTEN_MAX).expect("size");
    assert_eq!(KONTEN_MAX, 16_777_216);
    let mut t = vec![b'#'; max - 1];
    t.push(b'\n');
    std::fs::write(&p, &t).expect("write");
    let gelesen = start::text_lesen(&p).expect("exactly the limit");
    assert_eq!(gelesen.len(), max);
    assert!(gelesen.ends_with("#\n"));
    t.push(b'x');
    std::fs::write(&p, &t).expect("write");
    assert_eq!(start::text_lesen(&p), Err(Startfehler::Konfig));
    std::fs::write(&p, b"root:x:0:\n").expect("write");
    assert_eq!(start::text_lesen(&p).as_deref(), Ok("root:x:0:\n"));
    assert_eq!(
        start::text_lesen(&w.0.join("fehlt")),
        Err(Startfehler::Konfig)
    );
}

/// Step 3 with the folder of the chain next to `geheim`: made with 0700 if missing, kept if it is an own folder with
/// exactly 0700, else no chain (`None`), and nothing is changed.
#[test]
fn schritt_3_ordner_der_kette() {
    let w = ordner();
    let heim = w.0.join("heim");
    std::fs::create_dir(&heim).expect("heim");
    modus(&heim, 0o700);
    let k = konfig_in(&w.0, b"{}");
    let halter_ordner = halter::ablage::EBENEN[..4]
        .iter()
        .fold(heim.clone(), |p, e| p.join(e));
    let kette = halter_ordner.join("protokoll");
    // missing: made
    let (_, k1) = start::ablage_pruefen(&k.passwd, uid()).expect("store");
    assert_eq!(k1.as_deref(), Some(kette.as_path()));
    let m = std::fs::symlink_metadata(&kette).expect("chain folder");
    assert!(m.is_dir());
    assert_eq!(m.mode() & 0o7777, 0o700);
    // there with 0700: kept
    let (_, k2) = start::ablage_pruefen(&k.passwd, uid()).expect("store");
    assert_eq!(k2.as_deref(), Some(kette.as_path()));
    // there with 0750: no chain, the folder stays as it is
    modus(&kette, 0o750);
    let (_, k3) = start::ablage_pruefen(&k.passwd, uid()).expect("store");
    assert_eq!(k3, None);
    assert_eq!(
        std::fs::symlink_metadata(&kette).expect("stat").mode() & 0o7777,
        0o750
    );
    // a file with 0700: no chain
    std::fs::remove_dir(&kette).expect("rmdir");
    std::fs::write(&kette, b"").expect("file");
    modus(&kette, 0o700);
    let (_, k4) = start::ablage_pruefen(&k.passwd, uid()).expect("store");
    assert_eq!(k4, None);
    assert!(std::fs::symlink_metadata(&kette).expect("stat").is_file());
    // no entry of the uid in the file of the accounts
    assert_eq!(
        start::ablage_pruefen(&k.passwd, uid().wrapping_add(1)).map(|_| ()),
        Err(Startfehler::AblageRechte)
    );
}

fn socket_pfad(lz: &Path) -> PathBuf {
    lz.join("wb-halter/halter.sock")
}

/// Step 4: the folder is made with 0700 and the socket with 0600; it listens. A folder with another mode, a file, a
/// living socket at the path give 74 and stay; a dead own socket is removed and replaced.
#[test]
fn schritt_4_endpunkt() {
    let lz = kurzer_ordner();
    let (hoerer, pfad, id, _fd) = start::endpunkt(&lz.0, uid()).expect("endpoint");
    assert_eq!(pfad, socket_pfad(&lz.0));
    let m = std::fs::symlink_metadata(&pfad).expect("socket");
    assert_eq!(m.mode() & 0o7777, 0o600);
    assert_eq!((m.dev(), m.ino()), id);
    assert_eq!(
        std::fs::symlink_metadata(lz.0.join("wb-halter"))
            .expect("folder")
            .mode()
            & 0o7777,
        0o700
    );
    let _c = UnixStream::connect(&pfad).expect("listens");
    assert!(hoerer.accept().is_ok());
    // a living socket at the path: 74, the same inode
    assert_eq!(
        start::endpunkt(&lz.0, uid()).map(|_| ()),
        Err(Startfehler::PipeBelegt)
    );
    assert_eq!(std::fs::symlink_metadata(&pfad).expect("stat").ino(), id.1);
    drop(hoerer);
    // now dead (nobody listens): removed and made anew
    let (h2, _, _, _fd2) = start::endpunkt(&lz.0, uid()).expect("dead socket removed");
    let _c2 = UnixStream::connect(&pfad).expect("the new socket listens");
    assert!(h2.accept().is_ok());
    drop(h2);
    // a file at the path: 74, the file stays
    let lz = kurzer_ordner();
    std::fs::create_dir(lz.0.join("wb-halter")).expect("dir");
    modus(&lz.0.join("wb-halter"), 0o700);
    std::fs::write(socket_pfad(&lz.0), b"vorher").expect("file");
    modus(&socket_pfad(&lz.0), 0o600);
    assert_eq!(
        start::endpunkt(&lz.0, uid()).map(|_| ()),
        Err(Startfehler::PipeBelegt)
    );
    assert_eq!(std::fs::read(socket_pfad(&lz.0)).expect("read"), b"vorher");
    // the folder with another mode: 74, unchanged
    let lz = kurzer_ordner();
    std::fs::create_dir(lz.0.join("wb-halter")).expect("dir");
    modus(&lz.0.join("wb-halter"), 0o750);
    assert_eq!(
        start::endpunkt(&lz.0, uid()).map(|_| ()),
        Err(Startfehler::PipeBelegt)
    );
    assert!(!socket_pfad(&lz.0).exists());
    // a runtime folder that does not exist
    assert_eq!(
        start::endpunkt(&lz.0.join("fehlt"), uid()).map(|_| ()),
        Err(Startfehler::PipeBelegt)
    );
}

/// The end removes only the own socket: another device and inode leave it.
#[test]
fn socket_entfernen_nur_den_eigenen() {
    let lz = kurzer_ordner();
    let (h, pfad, id, fd) = start::endpunkt(&lz.0, uid()).expect("endpoint");
    start::socket_entfernen(&fd, (id.0, id.1.wrapping_add(1)));
    assert!(pfad.exists(), "another inode: kept");
    start::socket_entfernen(&fd, id);
    assert!(!pfad.exists(), "the own socket is removed");
    drop(h);
    // a socket bound again at the path after the end is not the own one any more
    let _neu = UnixListener::bind(&pfad).expect("bind");
    start::socket_entfernen(&fd, id);
    assert!(pfad.exists());
}
