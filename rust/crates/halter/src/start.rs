//! The start under Linux (section 2.1, "Start und Exitcodes"): steps 1 to 4 after the shielding of step 0, in this
//! order; the first violation ends the holder with its exit code and one line `<code>: <grund>`.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, FileType, Mode, OFlags, fstat, mkdirat, openat, statat};
use rustix::io::Errno;

use crate::ablage::{self, Ablage};
use crate::code::Startfehler;
use crate::konfig::{self, Konfig};
use crate::pruefung::Rolle;

/// The fixed places of the delivered build, and the seams of the test build.
#[derive(Debug, Clone)]
pub struct Orte {
    /// `halter.json`.
    pub konfig: PathBuf,
    /// Required owner of `halter.json` and its folder.
    pub konfig_eigner: u32,
    /// The file of the accounts (`/etc/passwd`).
    pub passwd: PathBuf,
    /// The file of the groups (`/etc/group`), for the role groups (section 2.3a).
    pub gruppen: PathBuf,
    /// The switch of the name services (`/etc/nsswitch.conf`), for the private group (decision of the operator, 28.11.2026).
    pub nsswitch: PathBuf,
    /// The runtime folder, `None`: always `/run/user/<uid>`, never `$XDG_RUNTIME_DIR` (section 2.3).
    pub laufzeit: Option<PathBuf>,
}

impl Orte {
    /// The places of the delivered build.
    #[must_use]
    pub fn fest() -> Self {
        Orte {
            konfig: PathBuf::from("/etc/werkbank/halter.json"),
            konfig_eigner: 0,
            passwd: PathBuf::from("/etc/passwd"),
            gruppen: PathBuf::from("/etc/group"),
            nsswitch: PathBuf::from("/etc/nsswitch.conf"),
            laufzeit: None,
        }
    }

    /// The places of the test build (feature `pruefbau`): the seams `WB_HALTER_KONFIG`, `WB_HALTER_KONFIG_EIGNER`,
    /// `WB_HALTER_PASSWD`, `WB_HALTER_GROUP`, `WB_HALTER_NSSWITCH` and `WB_HALTER_LAUFZEIT` read through `lies`, valid only with
    /// `WB_TESTUMGEBUNG=1`; a missing seam (or an owner that is no number) keeps the fixed place. Only the test build
    /// calls it, with the environment; the delivered build uses [`Orte::fest`] only.
    #[must_use]
    pub fn aus_naht(lies: impl Fn(&str) -> Option<OsString>) -> Self {
        let mut o = Orte::fest();
        if lies("WB_TESTUMGEBUNG").as_deref() != Some(OsStr::new("1")) {
            return o;
        }
        if let Some(p) = lies("WB_HALTER_KONFIG") {
            o.konfig = PathBuf::from(p);
        }
        if let Some(u) =
            lies("WB_HALTER_KONFIG_EIGNER").and_then(|v| v.to_str()?.parse::<u32>().ok())
        {
            o.konfig_eigner = u;
        }
        if let Some(p) = lies("WB_HALTER_PASSWD") {
            o.passwd = PathBuf::from(p);
        }
        if let Some(p) = lies("WB_HALTER_GROUP") {
            o.gruppen = PathBuf::from(p);
        }
        if let Some(p) = lies("WB_HALTER_NSSWITCH") {
            o.nsswitch = PathBuf::from(p);
        }
        o.laufzeit = lies("WB_HALTER_LAUFZEIT").map(PathBuf::from);
        o
    }
}

/// What the start made.
pub struct Bereit {
    /// The configuration.
    pub konfig: Konfig,
    /// The role groups: gid to role (section 2.3a).
    pub rollen: BTreeMap<u32, Rolle>,
    /// The store.
    pub ablage: Ablage,
    /// The folder of the chain.
    pub kette: Option<PathBuf>,
    /// The listening socket.
    pub hoerer: UnixListener,
    /// Path of the socket.
    pub socket: PathBuf,
    /// Device and inode of the socket, so that the end removes only the own one.
    pub socket_id: (u64, u64),
    /// The folder `wb-halter`, for the removal relative to it.
    pub ordner: OwnedFd,
}

/// The rule of `halter.json` and its folder after `stat`: of the kind `art`, owned by `eigner`, not writable for group
/// and others. A pure function on the raw fields, so that a test can give every kind and owner (no test can make a file
/// of another owner, and for another kind the open or the read fails too).
#[must_use]
pub fn datei_ok(modus: rustix::fs::RawMode, uid: u32, eigner: u32, art: FileType) -> bool {
    FileType::from_raw_mode(modus) == art && uid == eigner && modus & 0o022 == 0
}

/// Largest `/etc/group` or `/etc/passwd` read for the role groups, in bytes.
pub const KONTEN_MAX: u64 = 1 << 24;

/// The file of the groups or of the accounts as text, at most [`KONTEN_MAX`] bytes.
///
/// # Errors
/// [`Startfehler::Konfig`] if it cannot be read, is no UTF-8 or is bigger.
pub fn text_lesen(pfad: &Path) -> Result<String, Startfehler> {
    let mut t = String::new();
    std::io::Read::read_to_string(
        &mut std::io::Read::take(
            std::fs::File::open(pfad).map_err(|_| Startfehler::Konfig)?,
            KONTEN_MAX + 1,
        ),
        &mut t,
    )
    .map_err(|_| Startfehler::Konfig)?;
    if t.len() as u64 > KONTEN_MAX {
        return Err(Startfehler::Konfig);
    }
    Ok(t)
}

// In this file flags are joined with `union`: `|` and `^` give the same value for disjoint flags (a mutant no test
// could see).

/// Step 2: `halter.json`, owned by `eigner`, not writable for group and others, also its folder; strict and closed,
/// no local server with a token, the own uid in `konten`, and every role group resolved in the file of the groups
/// after section 2.3a (also against the primary groups of the accounts in the file of the accounts).
///
/// # Errors
/// [`Startfehler::Konfig`].
pub fn konfig_lesen(
    pfad: &Path,
    eigner: u32,
    eigene_uid: u32,
    gruppen: &Path,
    passwd: &Path,
) -> Result<(Konfig, BTreeMap<u32, Rolle>), Startfehler> {
    let ordner = pfad.parent().ok_or(Startfehler::Konfig)?;
    let st = statat(rustix::fs::CWD, ordner, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| Startfehler::Konfig)?;
    if !datei_ok(st.st_mode, st.st_uid, eigner, FileType::Directory) {
        return Err(Startfehler::Konfig);
    }
    let fd: OwnedFd = openat(
        rustix::fs::CWD,
        pfad,
        OFlags::RDONLY
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC)
            .union(OFlags::NONBLOCK),
        Mode::empty(),
    )
    .map_err(|_| Startfehler::Konfig)?;
    let st = fstat(&fd).map_err(|_| Startfehler::Konfig)?;
    if !datei_ok(st.st_mode, st.st_uid, eigner, FileType::RegularFile) {
        return Err(Startfehler::Konfig);
    }
    let mut inhalt = Vec::new();
    let mut f = std::fs::File::from(fd);
    std::io::Read::read_to_end(
        &mut std::io::Read::take(&mut f, konfig::GROESSE_MAX as u64 + 1),
        &mut inhalt,
    )
    .map_err(|_| Startfehler::Konfig)?;
    let group = text_lesen(gruppen)?;
    let konten = text_lesen(passwd)?;
    konfig::fuer_start(&inhalt, eigene_uid, &group, &konten).map_err(|_| Startfehler::Konfig)
}

/// The home and the store of `uid` with its private group: `None` without a readable `passwd` or a home in it.
fn ablage_fuer(
    passwd: &Path,
    gruppen: &Path,
    nsswitch: &Path,
    uid: u32,
) -> Option<(PathBuf, Ablage)> {
    let inhalt = std::fs::read_to_string(passwd).ok()?;
    let heim = ablage::heim_aus_passwd(&inhalt, uid)?;
    let gruppe = text_lesen(nsswitch)
        .ok()
        .filter(|n| ablage::nss_nur_dateien(n))
        .and_then(|_| text_lesen(gruppen).ok())
        .and_then(|g| ablage::private_gruppe(&inhalt, &g, uid));
    let a = Ablage::neu(&heim, uid).mit_gruppe(gruppe);
    Some((heim, a))
}

/// The remedy for `halter.ablage_rechte` (decision of the operator, 28.11.2026): `chmod g-w` with the levels above
/// `geheim` that are writable for a group that is not the private group ([`Ablage::gruppen_schreibbar`]), read anew
/// from the places of the start; `None` when there is no such level. Only paths of the store, never a content.
#[must_use]
pub fn ablage_abhilfe(orte: &Orte, uid: u32) -> Option<String> {
    let (_, a) = ablage_fuer(&orte.passwd, &orte.gruppen, &orte.nsswitch, uid)?;
    let pfade = a.gruppen_schreibbar();
    (!pfade.is_empty()).then(|| ablage::chmod_g_w(&pfade))
}

/// Step 3: the home from the file of the accounts and the levels of the store; also the folder of the chain
/// (`halter/protokoll`, 0700) next to `geheim`. A level above `geheim` may be writable for the private group of the
/// account after [`ablage::private_gruppe`], read from `passwd` and `gruppen`, and only when `nsswitch` takes accounts
/// and groups from the files and systemd alone ([`ablage::nss_nur_dateien`]); if `gruppen` or `nsswitch` cannot be
/// read, no group write is allowed.
///
/// # Errors
/// [`Startfehler::AblageRechte`].
pub fn ablage_pruefen(
    passwd: &Path,
    gruppen: &Path,
    nsswitch: &Path,
    uid: u32,
) -> Result<(Ablage, Option<PathBuf>), Startfehler> {
    let (heim, a) = ablage_fuer(passwd, gruppen, nsswitch, uid).ok_or(Startfehler::AblageRechte)?;
    let geheim = a.oeffnen(true).map_err(|_| Startfehler::AblageRechte)?;
    drop(geheim);
    let halter: PathBuf = ablage::EBENEN[..4]
        .iter()
        .fold(heim.clone(), |p, e| p.join(e));
    let kette = halter.join("protokoll");
    let kette = match std::fs::symlink_metadata(&kette) {
        Ok(m) if m.is_dir() && m.uid() == uid && m.mode() & 0o7777 == 0o700 => Some(kette),
        Ok(_) => None,
        Err(_) => openat(
            rustix::fs::CWD,
            &halter,
            OFlags::RDONLY
                .union(OFlags::DIRECTORY)
                .union(OFlags::NOFOLLOW)
                .union(OFlags::CLOEXEC),
            Mode::empty(),
        )
        .ok()
        .and_then(|fd| mkdirat(&fd, "protokoll", Mode::from_raw_mode(0o700)).ok())
        .map(|()| kette),
    };
    Ok((a, kette))
}

/// Step 4: the folder `wb-halter` (0700, own, no link; made if missing) and the socket `halter.sock` (0600), made
/// exclusively. If something lies at the path already, nothing is changed, with one exception (section 2.3, finding F2):
/// a socket of the own uid that refuses a connection (`ECONNREFUSED`, the rest of a holder ended by `SIGKILL` or the OOM
/// killer) is removed relative to the folder, only while device and inode are still the ones looked at; then the start
/// goes on. A living holder (the connection succeeds) and every other entry stay and give 74.
///
/// # Errors
/// [`Startfehler::PipeBelegt`].
pub fn endpunkt(
    laufzeit: &Path,
    uid: u32,
) -> Result<(UnixListener, PathBuf, (u64, u64), OwnedFd), Startfehler> {
    let rt = openat(
        rustix::fs::CWD,
        laufzeit,
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )
    .map_err(|_| Startfehler::PipeBelegt)?;
    match mkdirat(&rt, "wb-halter", Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(_) => return Err(Startfehler::PipeBelegt),
    }
    let ordner_fd = openat(
        &rt,
        "wb-halter",
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )
    .map_err(|_| Startfehler::PipeBelegt)?;
    let st = fstat(&ordner_fd).map_err(|_| Startfehler::PipeBelegt)?;
    if st.st_uid != uid || st.st_mode & 0o7777 != 0o700 {
        return Err(Startfehler::PipeBelegt);
    }
    let ordner = laufzeit.join("wb-halter");
    let socket = ordner.join("halter.sock");
    if let Ok(alt) = statat(&ordner_fd, "halter.sock", AtFlags::SYMLINK_NOFOLLOW) {
        if !toter_eigener_socket(&alt, uid, &socket) {
            return Err(Startfehler::PipeBelegt);
        }
        // still the same dead socket: remove it relative to the folder, never by following a path
        let jetzt = statat(&ordner_fd, "halter.sock", AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Startfehler::PipeBelegt)?;
        if crate::suche::geraet_inode(&jetzt) != crate::suche::geraet_inode(&alt) {
            return Err(Startfehler::PipeBelegt);
        }
        rustix::fs::unlinkat(&ordner_fd, "halter.sock", AtFlags::empty())
            .map_err(|_| Startfehler::PipeBelegt)?;
    }
    // the folder is 0700: between bind and chmod nobody else reaches the socket
    let hoerer = UnixListener::bind(&socket).map_err(|_| Startfehler::PipeBelegt)?;
    rustix::fs::chmodat(
        &ordner_fd,
        "halter.sock",
        Mode::from_raw_mode(0o600),
        AtFlags::empty(),
    )
    .map_err(|_| Startfehler::Intern)?;
    let m = statat(&ordner_fd, "halter.sock", AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| Startfehler::Intern)?;
    let id = crate::suche::geraet_inode(&m);
    Ok((hoerer, socket, id, ordner_fd))
}

/// `true` for a socket of the own uid at `pfad` (described by `st`, looked at without following a link) that refuses a
/// connection: nobody listens there any more. A connection that succeeds, any other error, another kind or another
/// owner is `false`.
fn toter_eigener_socket(st: &rustix::fs::Stat, uid: u32, pfad: &Path) -> bool {
    FileType::from_raw_mode(st.st_mode) == FileType::Socket
        && st.st_uid == uid
        && matches!(
            std::os::unix::net::UnixStream::connect(pfad),
            Err(e) if e.raw_os_error() == Some(Errno::CONNREFUSED.raw_os_error())
        )
}

/// The runtime folder: the seam of the test build, else the fixed `/run/user/<uid>`. The environment is never read: a
/// foreign `XDG_RUNTIME_DIR` must not move the socket (section 2.3, finding F17).
#[must_use]
pub fn laufzeit_ordner(orte: &Orte, uid: u32) -> PathBuf {
    orte.laufzeit
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{uid}")))
}

/// Steps 2 to 4.
///
/// # Errors
/// The first failed step.
pub fn starten(orte: &Orte, uid: u32) -> Result<Bereit, Startfehler> {
    let (konfig, rollen) = konfig_lesen(
        &orte.konfig,
        orte.konfig_eigner,
        uid,
        &orte.gruppen,
        &orte.passwd,
    )?;
    let (ablage, kette) = ablage_pruefen(&orte.passwd, &orte.gruppen, &orte.nsswitch, uid)?;
    let (hoerer, socket, socket_id, ordner) = endpunkt(&laufzeit_ordner(orte, uid), uid)?;
    Ok(Bereit {
        konfig,
        rollen,
        ablage,
        kette,
        hoerer,
        socket,
        socket_id,
        ordner,
    })
}

/// Removes the own socket at the end, relative to the folder `wb-halter` and only if device and inode are still the
/// own ones.
pub fn socket_entfernen(ordner: &OwnedFd, id: (u64, u64)) {
    if let Ok(m) = statat(ordner, "halter.sock", AtFlags::SYMLINK_NOFOLLOW)
        && crate::suche::geraet_inode(&m) == id
    {
        let _ = rustix::fs::unlinkat(ordner, "halter.sock", AtFlags::empty());
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::Orte;

    fn naht(test: Option<&'static str>) -> impl Fn(&str) -> Option<OsString> {
        move |n: &str| {
            let w = match n {
                "WB_TESTUMGEBUNG" => test?,
                "WB_HALTER_KONFIG" => "/srv/beispiel/halter.json",
                "WB_HALTER_KONFIG_EIGNER" => "4321",
                "WB_HALTER_PASSWD" => "/srv/beispiel/passwd",
                "WB_HALTER_GROUP" => "/srv/beispiel/group",
                "WB_HALTER_NSSWITCH" => "/srv/beispiel/nsswitch.conf",
                "WB_HALTER_LAUFZEIT" => "/srv/beispiel/laufzeit",
                _ => return None,
            };
            Some(OsString::from(w))
        }
    }

    fn fest_gleich(o: &Orte) {
        let f = Orte::fest();
        assert_eq!(o.konfig, f.konfig);
        assert_eq!(o.konfig_eigner, 0);
        assert_eq!(o.passwd, f.passwd);
        assert_eq!(o.gruppen, f.gruppen);
        assert_eq!(o.nsswitch, f.nsswitch);
        assert_eq!(o.laufzeit, None);
    }

    #[test]
    fn die_naehte_gelten_nur_mit_der_testumgebung() {
        let o = Orte::aus_naht(naht(Some("1")));
        assert_eq!(o.konfig, PathBuf::from("/srv/beispiel/halter.json"));
        assert_eq!(o.konfig_eigner, 4321);
        assert_eq!(o.passwd, PathBuf::from("/srv/beispiel/passwd"));
        assert_eq!(o.gruppen, PathBuf::from("/srv/beispiel/group"));
        assert_eq!(o.nsswitch, PathBuf::from("/srv/beispiel/nsswitch.conf"));
        assert_eq!(o.laufzeit, Some(PathBuf::from("/srv/beispiel/laufzeit")));
        fest_gleich(&Orte::aus_naht(naht(None)));
        fest_gleich(&Orte::aus_naht(naht(Some("0"))));
        fest_gleich(&Orte::aus_naht(naht(Some("11"))));
        // only the marker, no seam: the fixed places
        fest_gleich(&Orte::aus_naht(|n: &str| {
            (n == "WB_TESTUMGEBUNG").then(|| OsString::from("1"))
        }));
    }

    #[test]
    fn regel_von_konfig_und_ordner() {
        use rustix::fs::FileType;

        use super::datei_ok;
        let d = FileType::Directory;
        let f = FileType::RegularFile;
        assert!(datei_ok(0o040_755, 7, 7, d));
        assert!(datei_ok(0o100_644, 7, 7, f));
        assert!(datei_ok(0o100_600, 0, 0, f));
        // the kind
        assert!(!datei_ok(0o100_755, 7, 7, d));
        assert!(!datei_ok(0o040_644, 7, 7, f));
        assert!(!datei_ok(0o120_644, 7, 7, f));
        // the owner
        assert!(!datei_ok(0o100_644, 8, 7, f));
        assert!(!datei_ok(0o040_755, 0, 7, d));
        // writable for group or others
        for m in [0o664, 0o646, 0o666, 0o775] {
            assert!(!datei_ok(0o100_000 | m, 7, 7, f), "{m:o}");
        }
        assert!(datei_ok(0o100_444, 7, 7, f));
    }

    #[test]
    fn laufzeit_aus_naht_oder_run_user() {
        let mit = Orte {
            laufzeit: Some(PathBuf::from("/srv/beispiel/lz")),
            ..Orte::fest()
        };
        assert_eq!(
            super::laufzeit_ordner(&mit, 5),
            PathBuf::from("/srv/beispiel/lz")
        );
        assert_eq!(
            super::laufzeit_ordner(&Orte::fest(), 5),
            PathBuf::from("/run/user/5")
        );
        assert_eq!(
            super::laufzeit_ordner(&Orte::fest(), 1000),
            PathBuf::from("/run/user/1000")
        );
    }

    /// F17: a foreign `XDG_RUNTIME_DIR` does not move the socket. The test runs itself once more with the variable set
    /// to another folder (a test cannot set the environment of its own process without `unsafe`); in that child the
    /// fixed folder must still come out, and the child checks that the variable really is set.
    #[test]
    fn xdg_runtime_dir_verlegt_den_socket_nie() {
        const FREMD: &str = "/srv/beispiel/fremd-xdg";
        let kind =
            std::env::var_os("XDG_RUNTIME_DIR").as_deref() == Some(std::ffi::OsStr::new(FREMD));
        assert_eq!(
            super::laufzeit_ordner(&Orte::fest(), 5),
            PathBuf::from("/run/user/5")
        );
        if kind {
            return;
        }
        let aus = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "start::tests::xdg_runtime_dir_verlegt_den_socket_nie",
                "--test-threads=1",
            ])
            .env("XDG_RUNTIME_DIR", FREMD)
            .output()
            .expect("child");
        let text = String::from_utf8_lossy(&aus.stdout);
        assert!(aus.status.success(), "{text}");
        assert!(text.contains("1 passed"), "{text}");
    }

    #[test]
    fn ein_eigner_ohne_zahl_laesst_root() {
        let o = Orte::aus_naht(|n: &str| match n {
            "WB_TESTUMGEBUNG" => Some(OsString::from("1")),
            "WB_HALTER_KONFIG_EIGNER" => Some(OsString::from("x1")),
            _ => None,
        });
        assert_eq!(o.konfig_eigner, 0);
    }
}
