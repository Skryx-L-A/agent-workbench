//! The key file (contract 8, section 5): opened with `O_NOFOLLOW`, a regular file of the own user with mode exactly 0600,
//! one line of printable ASCII. The key stays in the own memory; no `Debug`, no `Display`, zeroed on drop.

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rustix::fs::{Mode, OFlags};

/// Largest content of the file in bytes (4096 bytes of key and a line feed).
pub const INHALT_MAX: usize = 4097;

/// Why a key file is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schluesselfehler {
    /// Not openable, a link, no regular file, not own or not 0600 (`mw.schluessel_datei`).
    Datei,
    /// The content is not one line `^[\x21-\x7e]{1,4096}$` (`mw.schluessel_form`).
    Form,
}

/// A key read from its file.
pub struct Schluessel(Vec<u8>);

impl Schluessel {
    /// The bytes of the key.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
impl Schluessel {
    /// A key for unit tests of other modules.
    pub(crate) fn fuer_test(b: &[u8]) -> Self {
        Self(b.to_vec())
    }
}

impl fmt::Debug for Schluessel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Schluessel(..)")
    }
}

impl Drop for Schluessel {
    fn drop(&mut self) {
        loeschen(&mut self.0);
    }
}

#[cfg(test)]
thread_local! {
    static GELOESCHT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Overwrites the bytes of a key with zeros before the memory is freed.
fn loeschen(bytes: &mut [u8]) {
    bytes.fill(0);
    std::hint::black_box(&*bytes);
    #[cfg(test)]
    GELOESCHT.with(|n| n.set(n.get() + 1));
}

/// Reads the key file at `pfad` for the user `eigene_uid`.
///
/// # Errors
/// [`Schluesselfehler`].
pub fn lies(pfad: &Path, eigene_uid: u32) -> Result<Schluessel, Schluesselfehler> {
    // Read only; a link is refused; a FIFO does not block the start.
    let flags = OFlags::NOFOLLOW
        .union(OFlags::CLOEXEC)
        .union(OFlags::NONBLOCK);
    let fd = rustix::fs::open(pfad, flags, Mode::empty()).map_err(|_| Schluesselfehler::Datei)?;
    let mut datei = File::from(fd);
    let meta = datei.metadata().map_err(|_| Schluesselfehler::Datei)?;
    if !datei_ok(meta.is_file(), meta.uid(), meta.mode(), eigene_uid) {
        return Err(Schluesselfehler::Datei);
    }
    let mut inhalt = Vec::new();
    let grenze = u64::try_from(INHALT_MAX + 1).unwrap_or(u64::MAX);
    let gelesen = (&mut datei).take(grenze).read_to_end(&mut inhalt);
    let schluessel = match gelesen {
        Ok(_) => form(&inhalt).map(|k| Schluessel(k.to_vec())),
        Err(_) => Err(Schluesselfehler::Datei),
    };
    loeschen(&mut inhalt);
    schluessel
}

/// A regular file of the own user with the permission bits exactly 0600 (no setuid, setgid or sticky bit either).
#[must_use]
pub fn datei_ok(ist_datei: bool, uid: u32, modus: u32, eigene_uid: u32) -> bool {
    ist_datei && uid == eigene_uid && modus & 0o7777 == 0o600
}

/// The key in the content: at most 4097 bytes, one line `^[\x21-\x7e]{1,4096}$` with or without a final line feed.
///
/// # Errors
/// [`Schluesselfehler::Form`].
pub fn form(inhalt: &[u8]) -> Result<&[u8], Schluesselfehler> {
    if inhalt.len() > INHALT_MAX {
        return Err(Schluesselfehler::Form);
    }
    let zeile = inhalt.strip_suffix(b"\n").unwrap_or(inhalt);
    let gut = (1..=INHALT_MAX - 1).contains(&zeile.len())
        && zeile.iter().all(|c| (0x21..=0x7e).contains(c));
    if gut {
        Ok(zeile)
    } else {
        Err(Schluesselfehler::Form)
    }
}

#[cfg(test)]
mod tests {
    use super::{GELOESCHT, Schluessel, Schluesselfehler, datei_ok, form, lies, loeschen};
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    #[test]
    fn a_dropped_key_is_overwritten() {
        let mut b = b"geheim".to_vec();
        loeschen(&mut b);
        assert_eq!(b, vec![0; 6]);
        let vorher = GELOESCHT.with(std::cell::Cell::get);
        drop(Schluessel(b"k".to_vec()));
        assert_eq!(GELOESCHT.with(std::cell::Cell::get), vorher + 1);
    }

    fn datei(name: &str, inhalt: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("mw-unit-sl-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        std::fs::write(&p, inhalt).expect("write");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        p
    }

    #[test]
    fn a_line_and_one_byte_more_is_too_long() {
        let mut inhalt = vec![b'a'; 4096];
        inhalt.extend_from_slice(b"\nb");
        let p = datei("lang", &inhalt);
        let uid = crate::os::eigene_uid();
        assert_eq!(lies(&p, uid).err(), Some(Schluesselfehler::Form));
        inhalt.pop();
        std::fs::write(&p, &inhalt).expect("write");
        assert_eq!(lies(&p, uid).map(|k| k.bytes().len()), Ok(4096));
        std::fs::remove_file(&p).expect("remove");
    }

    #[test]
    fn a_fifo_does_not_block() {
        let p = std::env::temp_dir().join(format!("mw-unit-fifo-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&p)
                .status()
                .expect("mkfifo")
                .success()
        );
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        let (tx, rx) = std::sync::mpsc::channel();
        let p2 = p.clone();
        std::thread::spawn(move || {
            let _ = tx.send(lies(&p2, crate::os::eigene_uid()).err());
        });
        let ergebnis = rx.recv_timeout(Duration::from_secs(2));
        let _ = std::fs::remove_file(&p);
        assert_eq!(ergebnis, Ok(Some(Schluesselfehler::Datei)));
    }

    #[test]
    fn only_an_own_regular_file_with_0600() {
        assert!(datei_ok(true, 7, 0o100_600, 7));
        assert!(!datei_ok(false, 7, 0o600, 7));
        assert!(!datei_ok(true, 8, 0o600, 7));
        assert!(!datei_ok(true, 7, 0o640, 7));
        assert!(!datei_ok(true, 7, 0o400, 7));
        assert!(!datei_ok(true, 7, 0o4600, 7));
        assert!(!datei_ok(true, 7, 0o700, 7));
    }

    #[test]
    fn the_form_of_the_key() {
        assert_eq!(form(b"a\n"), Ok(&b"a"[..]));
        assert_eq!(form(b"a"), Ok(&b"a"[..]));
        assert_eq!(form(b"!~"), Ok(&b"!~"[..]));
        assert_eq!(form(b""), Err(Schluesselfehler::Form));
        assert_eq!(form(b"\n"), Err(Schluesselfehler::Form));
        assert_eq!(form(b"a\n\n"), Err(Schluesselfehler::Form));
        assert_eq!(form(b"a b"), Err(Schluesselfehler::Form));
        assert_eq!(form(b"a\x7f"), Err(Schluesselfehler::Form));
        assert_eq!(form(b"a\r\n"), Err(Schluesselfehler::Form));
        let lang = vec![b'a'; 4096];
        assert_eq!(form(&lang), Ok(&lang[..]));
        let mut mit_lf = lang.clone();
        mit_lf.push(b'\n');
        assert_eq!(form(&mit_lf), Ok(&lang[..]));
        assert_eq!(form(&vec![b'a'; 4097]), Err(Schluesselfehler::Form));
        mit_lf.push(b'\n');
        assert_eq!(form(&mit_lf), Err(Schluesselfehler::Form));
    }

    #[test]
    fn the_key_does_not_show_itself() {
        let k = Schluessel(b"geheim".to_vec());
        assert_eq!(format!("{k:?}"), "Schluessel(..)");
        assert_eq!(k.bytes(), b"geheim");
    }
}
