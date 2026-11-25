//! The start under Linux (section 2.1, "Start und Exitcodes"): steps 1 to 4 after the shielding of step 0, in this
//! order; the first violation ends the holder with its exit code and one line `<code>: <grund>`.

use std::collections::BTreeMap;
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
    /// The runtime folder, `None`: `$XDG_RUNTIME_DIR` or `/run/user/<uid>`.
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
            laufzeit: None,
        }
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

fn datei_ok(st: &rustix::fs::Stat, eigner: u32, art: FileType) -> bool {
    FileType::from_raw_mode(st.st_mode) == art && st.st_uid == eigner && st.st_mode & 0o022 == 0
}

/// Largest `/etc/group` or `/etc/passwd` read for the role groups, in bytes.
pub const KONTEN_MAX: u64 = 1 << 24;

fn text_lesen(pfad: &Path) -> Result<String, Startfehler> {
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
    if !datei_ok(&st, eigner, FileType::Directory) {
        return Err(Startfehler::Konfig);
    }
    let fd: OwnedFd = openat(
        rustix::fs::CWD,
        pfad,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| Startfehler::Konfig)?;
    let st = fstat(&fd).map_err(|_| Startfehler::Konfig)?;
    if !datei_ok(&st, eigner, FileType::RegularFile) {
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

/// Step 3: the home from the file of the accounts and the levels of the store; also the folder of the chain
/// (`halter/protokoll`, 0700) next to `geheim`.
///
/// # Errors
/// [`Startfehler::AblageRechte`].
pub fn ablage_pruefen(passwd: &Path, uid: u32) -> Result<(Ablage, Option<PathBuf>), Startfehler> {
    let inhalt = std::fs::read_to_string(passwd).map_err(|_| Startfehler::AblageRechte)?;
    let heim = ablage::heim_aus_passwd(&inhalt, uid).ok_or(Startfehler::AblageRechte)?;
    let a = Ablage::neu(&heim, uid);
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
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
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
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
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
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
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
        match statat(&ordner_fd, "halter.sock", AtFlags::SYMLINK_NOFOLLOW) {
            Ok(jetzt) if crate::suche::geraet_inode(&jetzt) == crate::suche::geraet_inode(&alt) => {
                rustix::fs::unlinkat(&ordner_fd, "halter.sock", AtFlags::empty())
                    .map_err(|_| Startfehler::PipeBelegt)?;
            }
            _ => return Err(Startfehler::PipeBelegt),
        }
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

/// The runtime folder: the seam, else `$XDG_RUNTIME_DIR` if absolute, else `/run/user/<uid>`.
#[must_use]
pub fn laufzeit_ordner(orte: &Orte, uid: u32) -> PathBuf {
    if let Some(p) = &orte.laufzeit {
        return p.clone();
    }
    match std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) {
        Some(p) if p.is_absolute() => p,
        _ => PathBuf::from(format!("/run/user/{uid}")),
    }
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
    let (ablage, kette) = ablage_pruefen(&orte.passwd, uid)?;
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
