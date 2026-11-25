//! DI2, SECURITY POINT: the rights of the socket folder and of the socket. This file is small on purpose and holds the whole
//! check; nothing else in the service decides who may talk to it. The service has no check of the peer (`std` has no call for
//! it, `unsafe` is forbidden): who may enter the folder may speak.
//!
//! 1. The folder is a folder by `lstat` (no link, even to a good folder), and it exists: the service never makes it.
//! 2. The owner of the folder is the effective user of the service.
//! 3. The mode of the folder is exactly 0700.
//! 4. Nothing exists at the path of the socket: the service never replaces or deletes what is there.
//! 5. The socket gets the mode 0600 after `bind` and before `listen`: nobody can connect while it has another mode.
//! 6. No TCP, no UDP, no other network socket: this file and `dienst.rs` open one Unix socket and nothing else.

use std::fs;
use std::io::ErrorKind;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixListener;
use std::path::Path;

use crate::fehler::StartFehler;
use crate::startdatei::Startdatei;

/// The mode of the socket folder.
pub const ORDNER_MODUS: u32 = 0o700;
/// The mode of the socket.
pub const SOCKET_MODUS: u32 = 0o600;

fn verweigert(text: &str) -> StartFehler {
    StartFehler::neu(3, text)
}

/// The owner of the folder is the effective user (point 2).
pub(crate) fn besitzer_gleich(besitzer: u32, euid: u32) -> bool {
    besitzer == euid
}

/// The mode bits of a file as `st_mode & 07777` (point 3): exactly 0700 is good, every other value is not.
pub(crate) fn modus_gut(mode: u32) -> bool {
    mode & 0o7777 == ORDNER_MODUS
}

/// Checks the folder and the path of the socket (exit code 3 on every error).
///
/// # Errors
/// The folder is missing, a link, no folder, owned by another user or not exactly 0700, or the path of the socket exists.
pub fn rechte_pruefen(start: &Startdatei) -> Result<(), StartFehler> {
    ordner_pruefen(
        Path::new(&start.ordner),
        rustix::process::geteuid().as_raw(),
    )?;
    socket_pfad_pruefen(Path::new(&start.pfad))
}

/// Points 1 to 3 for `ordner`, with `euid` as the effective user.
pub(crate) fn ordner_pruefen(ordner: &Path, euid: u32) -> Result<(), StartFehler> {
    ordner_geprueft(ordner, euid).map(|_| ())
}

/// The same checks; returns the metadata of the folder that was checked, so the caller can hold the folder it opens to exactly that inode.
///
/// # Errors
/// See [`rechte_pruefen`].
pub(crate) fn ordner_geprueft(ordner: &Path, euid: u32) -> Result<fs::Metadata, StartFehler> {
    let meta = match fs::symlink_metadata(ordner) {
        Ok(m) => m,
        Err(e) => {
            return Err(verweigert(&format!(
                "Ordner des Sockets fehlt oder ist nicht lesbar: {}",
                e.kind()
            )));
        }
    };
    if meta.file_type().is_symlink() {
        return Err(verweigert("Ordner des Sockets ist ein symbolischer Link"));
    }
    if !meta.is_dir() {
        return Err(verweigert("Ordner des Sockets ist kein Ordner"));
    }
    if !besitzer_gleich(meta.uid(), euid) {
        return Err(verweigert(
            "Ordner des Sockets gehoert einem anderen Benutzer",
        ));
    }
    let modus = meta.mode() & 0o7777;
    if !modus_gut(modus) {
        return Err(verweigert(&format!(
            "Ordner des Sockets hat Modus {modus:04o} statt 0700"
        )));
    }
    Ok(meta)
}

/// `true` when both are the same inode (device and inode equal).
pub(crate) fn gleicher_inode(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    (a.dev(), a.ino()) == (b.dev(), b.ino())
}

/// Opens the socket folder once for the whole run of the service (`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`) and holds it: every removal below it goes
/// through this descriptor (`loeschen.rs`). Owner and mode (points 2 and 3) are checked at the inode that `lstat` of the path gave
/// ([`ordner_geprueft`]); the descriptor must be exactly this inode: `fstat` of the descriptor and that same checked metadata agree on
/// device and inode. There is no second `lstat`. Any other result ends the start like a failed check of the rights (exit code 3).
///
/// # Errors
/// The folder fails the checks, cannot be opened without following a link, or the descriptor is not the inode that was checked.
pub(crate) fn ordner_oeffnen(ordner: &Path, euid: u32) -> Result<OwnedFd, StartFehler> {
    use rustix::fs::{CWD, Mode, openat};
    let geprueft = ordner_geprueft(ordner, euid)?;
    let fd = openat(CWD, ordner, crate::loeschen::ORDNER_OFFEN, Mode::empty())
        .map_err(|_| verweigert("Ordner des Sockets laesst sich nicht oeffnen"))?;
    let datei = fs::File::from(fd);
    let st = datei
        .metadata()
        .map_err(|_| verweigert("Ordner des Sockets nicht pruefbar"))?;
    if !gleicher_inode(&st, &geprueft) {
        return Err(verweigert(
            "Ordner des Sockets ist nicht der geprueften Ordner",
        ));
    }
    Ok(OwnedFd::from(datei))
}

/// Point 4: any entry at the path (file, socket, folder, even a dangling link) is an error.
pub(crate) fn socket_pfad_pruefen(pfad: &Path) -> Result<(), StartFehler> {
    match fs::symlink_metadata(pfad) {
        Ok(_) => Err(verweigert("Socket-Pfad besteht schon")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(verweigert(&format!(
            "Socket-Pfad nicht pruefbar: {}",
            e.kind()
        ))),
    }
}

/// The backlog of the listening socket (the value `std` uses for `UnixListener::bind`).
const WARTESCHLANGE: i32 = 128;

/// A Unix socket whose descriptor is closed on `exec`, so no child inherits it. Linux does that in one step (`SOCK_CLOEXEC`). Apple
/// systems have no `SOCK_CLOEXEC` (`rustix` 1.1.6 leaves `SocketFlags::CLOEXEC` out there), so the flag is set right after `socket` with
/// `fcntl(F_SETFD, FD_CLOEXEC)`; the service makes the socket at its start, before it starts any child.
pub(crate) fn unix_socket(typ: rustix::net::SocketType) -> rustix::io::Result<OwnedFd> {
    use rustix::net::{AddressFamily, SocketFlags, socket_with};
    #[cfg(not(target_vendor = "apple"))]
    {
        socket_with(AddressFamily::UNIX, typ, SocketFlags::CLOEXEC, None)
    }
    #[cfg(target_vendor = "apple")]
    {
        let fd = socket_with(AddressFamily::UNIX, typ, SocketFlags::empty(), None)?;
        rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC)?;
        Ok(fd)
    }
}

/// Binds the socket and sets the mode 0600, but does not listen yet (point 5). A client that connects before `hoeren` gets `ECONNREFUSED`,
/// so no client ever sees the socket with the mode of the umask: the mode is 0600 before anyone can connect. The process umask stays
/// untouched (it holds for the whole process).
///
/// # Errors
/// Exit code 1: the socket cannot be made or bound or its mode cannot be set; a socket file that was made is removed again.
pub(crate) fn socket_gebunden(pfad: &Path, wurzel: BorrowedFd<'_>) -> Result<OwnedFd, StartFehler> {
    use rustix::net::{SocketAddrUnix, SocketType, bind};
    let fd = unix_socket(SocketType::STREAM)
        .map_err(|e| StartFehler::neu(1, &format!("socket: {e}")))?;
    let adresse =
        SocketAddrUnix::new(pfad).map_err(|e| StartFehler::neu(1, &format!("bind: {e}")))?;
    bind(&fd, &adresse).map_err(|e| StartFehler::neu(1, &format!("bind: {e}")))?;
    if socket_modus_setzen(pfad, wurzel).is_err() {
        socket_entfernen(pfad, wurzel);
        return Err(StartFehler::neu(1, "chmod: kein Socket ohne Link"));
    }
    Ok(fd)
}

/// Sets the mode 0600 on the name of the socket inside the held folder `wurzel` without following a link, and checks afterwards with
/// `lstat` through the same descriptor that the name is a socket with mode 0600 (the same way as `modus_setzen` of the report socket,
/// N3 of the review of 23.11.2026): a process of the same user that puts a link in place of the socket between `bind` and this call
/// changes no other file, and the start fails.
///
/// Linux: the name is opened with `O_PATH | O_NOFOLLOW` below `wurzel` (a link fails with `ELOOP`), the descriptor must be a socket,
/// and the mode is set through `/proc/self/fd/<n>`, which names exactly that inode (`rustix` 1.1.6 refuses `AT_SYMLINK_NOFOLLOW` for
/// `fchmodat` under Linux). Elsewhere `fchmodat` with `AT_SYMLINK_NOFOLLOW` changes a link itself, never its target.
fn socket_modus_setzen(pfad: &Path, wurzel: BorrowedFd<'_>) -> Result<(), ()> {
    use rustix::fs::{AtFlags, FileType, Mode, statat};
    let name = pfad.file_name().ok_or(())?;
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::{OFlags, openat};
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::PermissionsExt;
        let fd = openat(
            wurzel,
            name,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ())?;
        let st = rustix::fs::fstat(&fd).map_err(|_| ())?;
        if FileType::from_raw_mode(st.st_mode) != FileType::Socket {
            return Err(());
        }
        fs::set_permissions(
            format!("/proc/self/fd/{}", fd.as_raw_fd()),
            fs::Permissions::from_mode(SOCKET_MODUS),
        )
        .map_err(|_| ())?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        rustix::fs::chmodat(
            wurzel,
            name,
            Mode::RUSR | Mode::WUSR,
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| ())?;
    }
    let st = statat(wurzel, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|_| ())?;
    if FileType::from_raw_mode(st.st_mode) == FileType::Socket
        && Mode::from_raw_mode(st.st_mode) == Mode::RUSR | Mode::WUSR
    {
        Ok(())
    } else {
        Err(())
    }
}

/// Starts to listen on a bound socket whose mode is 0600 already.
///
/// # Errors
/// Exit code 1: `listen` fails; the socket file is removed again.
pub(crate) fn hoeren(
    fd: OwnedFd,
    pfad: &Path,
    wurzel: BorrowedFd<'_>,
) -> Result<UnixListener, StartFehler> {
    if let Err(e) = rustix::net::listen(&fd, WARTESCHLANGE) {
        socket_entfernen(pfad, wurzel);
        return Err(StartFehler::neu(1, &format!("listen: {e}")));
    }
    Ok(UnixListener::from(fd))
}

/// Removes the socket file through the descriptor of the folder (rule 4).
fn socket_entfernen(pfad: &Path, wurzel: BorrowedFd<'_>) {
    if let Some(name) = pfad.file_name().and_then(|n| n.to_str()) {
        crate::loeschen::datei_entfernen(wurzel, name);
    }
}

/// Binds the socket, sets the mode 0600 and only then listens (point 5). The caller has checked the rights before.
///
/// # Errors
/// Exit code 1: the socket cannot be bound, its mode cannot be set or it cannot listen; a socket file that was made is removed again.
pub fn socket_binden(pfad: &Path, wurzel: BorrowedFd<'_>) -> Result<UnixListener, StartFehler> {
    let fd = socket_gebunden(pfad, wurzel)?;
    hoeren(fd, pfad, wurzel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsFd;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    fn euid() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    /// A folder of this test only: below a base folder with the process id and the test name in its name.
    struct Wegwerf(PathBuf);

    impl Wegwerf {
        fn neu(name: &str) -> Self {
            let pfad =
                std::env::temp_dir().join(format!("dienst-rechte-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&pfad);
            fs::create_dir(&pfad).expect("make test folder");
            Self(pfad)
        }

        fn unter(&self, n: &str) -> PathBuf {
            self.0.join(n)
        }

        fn ordner(&self, n: &str, modus: u32) -> PathBuf {
            let p = self.unter(n);
            fs::create_dir(&p).expect("make folder");
            fs::set_permissions(&p, fs::Permissions::from_mode(modus)).expect("chmod");
            p
        }
    }

    impl Drop for Wegwerf {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn code(r: &Result<(), StartFehler>) -> Option<i32> {
        r.as_ref().err().map(|e| e.code)
    }

    #[test]
    fn modus_genau_0700() {
        assert!(modus_gut(0o700));
        assert!(modus_gut(0o40700), "file type bits do not count");
        for schlecht in [
            0o750, 0o755, 0o777, 0o701, 0o770, 0o600, 0o500, 0o2700, 0o1700, 0o4700, 0o0,
        ] {
            assert!(!modus_gut(schlecht), "{schlecht:o}");
        }
    }

    #[test]
    fn besitzer() {
        assert!(besitzer_gleich(501, 501));
        assert!(!besitzer_gleich(0, 501));
        assert!(!besitzer_gleich(501, 0));
    }

    #[test]
    fn ordner_gut_und_schlecht() {
        let w = Wegwerf::neu("ordner");
        let gut = w.ordner("gut", 0o700);
        assert_eq!(code(&ordner_pruefen(&gut, euid())), None);
        for modus in [0o750, 0o755, 0o777, 0o701, 0o770, 0o600, 0o500] {
            let o = w.ordner(&format!("m{modus:o}"), modus);
            assert_eq!(code(&ordner_pruefen(&o, euid())), Some(3), "{modus:o}");
            fs::set_permissions(&o, fs::Permissions::from_mode(0o700)).expect("chmod back");
        }
        // setgid and sticky bits: the mode is not exactly 0700
        for modus in [0o2700, 0o1700] {
            let o = w.ordner(&format!("s{modus:o}"), modus);
            let meta = fs::metadata(&o).expect("metadata");
            if meta.mode() & 0o7777 == modus {
                assert_eq!(code(&ordner_pruefen(&o, euid())), Some(3), "{modus:o}");
            }
        }
    }

    #[test]
    fn ordner_fehlt_datei_link() {
        let w = Wegwerf::neu("art");
        assert_eq!(code(&ordner_pruefen(&w.unter("fehlt"), euid())), Some(3));
        let datei = w.unter("datei");
        fs::write(&datei, b"x").expect("write");
        assert_eq!(code(&ordner_pruefen(&datei, euid())), Some(3));
        let gut = w.ordner("gut", 0o700);
        let link = w.unter("link");
        symlink(&gut, &link).expect("link");
        assert_eq!(
            code(&ordner_pruefen(&link, euid())),
            Some(3),
            "a link to a good folder"
        );
        assert_eq!(code(&ordner_pruefen(&gut, euid())), None);
        // a dangling link
        let haengt = w.unter("haengt");
        symlink(w.unter("nirgends"), &haengt).expect("link");
        assert_eq!(code(&ordner_pruefen(&haengt, euid())), Some(3));
    }

    #[test]
    fn socket_pfad_mit_anderem_fehler_als_nicht_gefunden() {
        // a file in the place of a folder: the path cannot be looked at (not found is not the error), and that is a refusal too
        let f = socket_pfad_pruefen(Path::new("/etc/hosts/unter")).expect_err("no");
        assert_eq!(f.code, 3);
        assert!(f.text.contains("nicht pruefbar"), "{}", f.text);
    }

    #[test]
    fn fremder_besitzer() {
        let w = Wegwerf::neu("besitzer");
        let o = w.ordner("o", 0o700);
        assert_eq!(code(&ordner_pruefen(&o, euid())), None);
        assert_eq!(code(&ordner_pruefen(&o, euid().wrapping_add(1))), Some(3));
        let f = ordner_pruefen(&o, euid().wrapping_add(1)).expect_err("error");
        assert!(f.text.contains("anderen Benutzer"), "{}", f.text);
    }

    #[test]
    fn socket_pfad_darf_nicht_bestehen() {
        let w = Wegwerf::neu("pfad");
        assert_eq!(code(&socket_pfad_pruefen(&w.unter("s"))), None);
        fs::write(w.unter("datei"), b"").expect("write");
        assert_eq!(code(&socket_pfad_pruefen(&w.unter("datei"))), Some(3));
        fs::create_dir(w.unter("ordner")).expect("dir");
        assert_eq!(code(&socket_pfad_pruefen(&w.unter("ordner"))), Some(3));
        symlink(w.unter("nirgends"), w.unter("haengt")).expect("link");
        assert_eq!(code(&socket_pfad_pruefen(&w.unter("haengt"))), Some(3));
        let _ = UnixListener::bind(w.unter("sock")).expect("bind");
        assert_eq!(code(&socket_pfad_pruefen(&w.unter("sock"))), Some(3));
        // the service deleted nothing
        assert!(
            w.unter("datei").exists() && w.unter("ordner").exists() && w.unter("sock").exists()
        );
    }

    #[test]
    fn bindet_mit_0600() {
        let w = Wegwerf::neu("bind");
        let o = w.ordner("o", 0o700);
        let pfad = o.join("s.sock");
        let wurzel = ordner_oeffnen(&o, euid()).expect("open");
        let hoerer = socket_binden(&pfad, wurzel.as_fd()).expect("bind");
        let meta = fs::symlink_metadata(&pfad).expect("meta");
        assert_eq!(meta.mode() & 0o7777, 0o600);
        // a second bind to the same path fails with exit code 1 and leaves the first socket
        let f = socket_binden(&pfad, wurzel.as_fd()).expect_err("second bind fails");
        assert_eq!(f.code, 1);
        assert!(pfad.exists());
        drop(hoerer);
    }

    #[test]
    fn zwischen_bind_und_listen_ist_der_modus_schon_0600_und_connect_wird_abgewiesen() {
        let w = Wegwerf::neu("fenster");
        let o = w.ordner("o", 0o700);
        let pfad = o.join("s.sock");
        let wurzel = ordner_oeffnen(&o, euid()).expect("open");
        let fd = socket_gebunden(&pfad, wurzel.as_fd()).expect("bound");
        // bound, not yet listening: the mode is 0600 and a client is refused
        let meta = fs::symlink_metadata(&pfad).expect("meta");
        assert_eq!(meta.mode() & 0o7777, 0o600);
        let fehler = std::os::unix::net::UnixStream::connect(&pfad).expect_err("refused");
        assert_eq!(fehler.kind(), ErrorKind::ConnectionRefused);
        let hoerer = hoeren(fd, &pfad, wurzel.as_fd()).expect("listen");
        std::os::unix::net::UnixStream::connect(&pfad).expect("connect after listen");
        drop(hoerer);
    }

    /// The mode of the service socket is set on the name itself: a link in place of the socket is refused, and the mode of its target stays.
    #[test]
    fn socket_modus_setzen_folgt_keinem_link() {
        let w = Wegwerf::neu("link");
        let o = w.ordner("o", 0o700);
        let wurzel = ordner_oeffnen(&o, euid()).expect("open");
        let ziel = w.unter("ziel");
        fs::write(&ziel, b"x").expect("target");
        fs::set_permissions(&ziel, fs::Permissions::from_mode(0o644)).expect("chmod");
        let link = o.join("s.sock");
        symlink(&ziel, &link).expect("link");
        assert_eq!(socket_modus_setzen(&link, wurzel.as_fd()), Err(()));
        let modus = fs::metadata(&ziel).expect("target").mode() & 0o7777;
        assert_eq!(modus, 0o644, "the target keeps its mode");
        // a plain file is no socket either, and a missing name fails
        let datei = o.join("datei");
        fs::write(&datei, b"").expect("file");
        fs::set_permissions(&datei, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(socket_modus_setzen(&datei, wurzel.as_fd()), Err(()));
        assert_eq!(
            socket_modus_setzen(&o.join("fehlt"), wurzel.as_fd()),
            Err(())
        );
        // a real socket gets 0600
        let _hoerer = UnixListener::bind(o.join("echt.sock")).expect("bind");
        fs::set_permissions(o.join("echt.sock"), fs::Permissions::from_mode(0o755)).expect("chmod");
        assert_eq!(
            socket_modus_setzen(&o.join("echt.sock"), wurzel.as_fd()),
            Ok(())
        );
        let meta = fs::symlink_metadata(o.join("echt.sock")).expect("meta");
        assert_eq!(meta.mode() & 0o7777, 0o600);
    }

    /// A link in place of the service socket that points to another socket: the call fails and the other socket keeps its mode.
    /// Under Linux this needs `O_NOFOLLOW` on the open; without it the open follows the link and the mode of the target changes,
    /// and only the check afterwards fails (mutant `PATH | NOFOLLOW & CLOEXEC`, 25.11.2026).
    #[test]
    fn ein_link_auf_einen_anderen_socket_aendert_dessen_modus_nicht() {
        let w = Wegwerf::neu("link-socket");
        let o = w.ordner("o", 0o700);
        let wurzel = ordner_oeffnen(&o, euid()).expect("open");
        let ziel = w.unter("anderer.sock");
        let _anderer = UnixListener::bind(&ziel).expect("bind");
        fs::set_permissions(&ziel, fs::Permissions::from_mode(0o644)).expect("chmod");
        let link = o.join("s.sock");
        symlink(&ziel, &link).expect("link");
        assert_eq!(socket_modus_setzen(&link, wurzel.as_fd()), Err(()));
        let modus = fs::symlink_metadata(&ziel).expect("target").mode() & 0o7777;
        assert_eq!(modus, 0o644, "the other socket keeps its mode");
    }

    #[test]
    fn der_socket_wird_bei_exec_geschlossen() {
        use rustix::io::{FdFlags, fcntl_getfd};
        use rustix::net::SocketType;
        for typ in [SocketType::STREAM, SocketType::DGRAM] {
            let fd = unix_socket(typ).expect("socket");
            let flags = fcntl_getfd(&fd).expect("getfd");
            assert!(flags.contains(FdFlags::CLOEXEC), "{typ:?}: {flags:?}");
        }
    }

    #[test]
    fn scheitert_listen_wird_die_socketdatei_entfernt() {
        use rustix::net::{SocketAddrUnix, SocketType, bind};
        let w = Wegwerf::neu("hoeren");
        let o = w.ordner("o", 0o700);
        let pfad = o.join("d.sock");
        let wurzel = ordner_oeffnen(&o, euid()).expect("open");
        // a datagram socket cannot listen
        let fd = unix_socket(SocketType::DGRAM).expect("socket");
        bind(&fd, &SocketAddrUnix::new(&pfad).expect("addr")).expect("bind");
        assert!(pfad.exists());
        let f = hoeren(fd, &pfad, wurzel.as_fd()).expect_err("listen fails");
        assert_eq!(f.code, 1);
        assert!(!pfad.exists(), "the socket file is removed again");
    }

    #[test]
    fn rechte_pruefen_ganz() {
        let w = Wegwerf::neu("ganz");
        let o = w.ordner("o", 0o700);
        let mut start = crate::startdatei::pruefen(&serde_json::json!({
            "socket": {"ordner": o.to_string_lossy()},
            "betrachter": {"person": "a", "raum": "r", "ansicht": "fenster"},
            "kind": {"argv": ["/bin/echo"], "env": {}},
            "groesse": [10, 5],
            "anfang": {"personen": {"a": {}}, "sitzungen": [{"id": "s"}]}
        }))
        .expect("start");
        assert_eq!(code(&rechte_pruefen(&start)), None);
        fs::write(&start.pfad, b"").expect("write");
        assert_eq!(code(&rechte_pruefen(&start)), Some(3));
        fs::remove_file(&start.pfad).expect("rm");
        start.ordner = w.unter("fehlt").to_string_lossy().into_owned();
        assert_eq!(code(&rechte_pruefen(&start)), Some(3));
    }

    #[test]
    fn der_vergleich_lehnt_die_metadaten_eines_anderen_ordners_ab() {
        let w = Wegwerf::neu("inode");
        let a = w.ordner("a", 0o700);
        let b = w.ordner("b", 0o700);
        let ma = fs::symlink_metadata(&a).expect("meta");
        let mb = fs::symlink_metadata(&b).expect("meta");
        assert!(gleicher_inode(&ma, &ma));
        assert!(!gleicher_inode(&ma, &mb));
        assert!(!gleicher_inode(&mb, &ma));
    }

    #[test]
    fn der_geoeffnete_ordner_ist_der_geprueften_inode_und_besitzer_und_modus_zaehlen() {
        use std::os::fd::AsFd;
        let w = Wegwerf::neu("oeffnen");
        let o = w.ordner("o", 0o700);
        let fd = ordner_oeffnen(&o, euid()).expect("open");
        let geprueft = ordner_geprueft(&o, euid()).expect("checked");
        let st = fs::File::from(fd.as_fd().try_clone_to_owned().expect("dup"))
            .metadata()
            .expect("meta");
        assert!(gleicher_inode(&st, &geprueft));
        assert_eq!(geprueft.mode() & 0o7777, 0o700);
        // owner and mode are checked at that inode: another user or another mode is refused before anything is opened
        let f = ordner_oeffnen(&o, euid().wrapping_add(1)).expect_err("other owner");
        assert_eq!(f.code, 3);
        let offen = w.ordner("offen", 0o755);
        assert_eq!(ordner_oeffnen(&offen, euid()).expect_err("mode").code, 3);
        assert_eq!(
            ordner_oeffnen(&w.unter("fehlt"), euid())
                .expect_err("missing")
                .code,
            3
        );
    }
}
