//! The checks before the start (contract 1, step 5 and 6): the folders exist, `bwrap` runs, the terminal cannot be
//! typed into, and the finished profile runs `true`.

use crate::fehler::Fehler;
use crate::profil::Profil;

#[cfg(unix)]
use std::{
    fs,
    io::Read,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(unix)]
/// The kernel switch that lets a process push keystrokes into a terminal (`TIOCSTI`).
const TIOCSTI_DATEI: &str = "/proc/sys/dev/tty/legacy_tiocsti";
#[cfg(unix)]
/// The release of the kernel.
const OSRELEASE_DATEI: &str = "/proc/sys/kernel/osrelease";
#[cfg(unix)]
/// How long the test run may take.
pub(crate) const PROBE_FRIST: Duration = Duration::from_secs(10);
#[cfg(unix)]
/// How many bytes of standard error of `bwrap` the error carries.
const MELDUNG_MAX: usize = 300;

#[cfg(unix)]
/// `true` when the release text names a kernel of at least `major.minor`.
fn kernel_ab(osrelease: &str, major: u32, minor: u32) -> bool {
    let mut teile = osrelease
        .trim()
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<u32>().unwrap_or(0));
    let (Some(ist_major), Some(ist_minor)) = (teile.next(), teile.next()) else {
        return false;
    };
    (ist_major, ist_minor) >= (major, minor)
}

#[cfg(unix)]
/// `true` when nothing can push keystrokes into the terminal of the session. The profile has no `--new-session`
/// (the harness keeps its terminal), so this is the only protection. `wert` is the content of `legacy_tiocsti` or
/// `None` when the file is missing. `0` closes it. A missing file closes it too when the kernel is at least 6.2: from
/// then on the kernel can be built without the switch, and without the switch the call is always refused. A kernel
/// older than 6.2 has no such switch, and the call works.
fn tiocsti_geschlossen(wert: Option<&str>, osrelease: &str) -> bool {
    match wert {
        Some(w) => w.trim() == "0",
        None => kernel_ab(osrelease, 6, 2),
    }
}

#[cfg(unix)]
/// A short, ASCII-only form of the standard error of `bwrap`.
fn kurz(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(MELDUNG_MAX)
        .map(|b| {
            if b.is_ascii_graphic() || *b == b' ' {
                char::from(*b)
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(unix)]
/// The messages of `bwrap` when the system refuses the user namespace (Ubuntu with the `AppArmor` restriction and the
/// profile of the starter not loaded, or the kernel switch off).
const NAMENSRAUM_MELDUNGEN: [&str; 4] = [
    "RTM_NEWADDR",
    "creating new namespace",
    "setting up uid map",
    "unprivileged_userns",
];

#[cfg(unix)]
fn namensraum_gesperrt(meldung: &str) -> bool {
    NAMENSRAUM_MELDUNGEN.iter().any(|m| meldung.contains(m))
}

#[cfg(unix)]
fn quelle_pruefen(pfad: &Path) -> Result<fs::Metadata, Fehler> {
    let echt =
        fs::canonicalize(pfad).map_err(|_| Fehler::QuelleUnbrauchbar("mount source missing"))?;
    if echt != pfad {
        return Err(Fehler::QuelleUnbrauchbar(
            "mount source is not a real path (link)",
        ));
    }
    fs::symlink_metadata(pfad).map_err(|_| Fehler::QuelleUnbrauchbar("mount source unreadable"))
}

/// A configuration file of the harness: a regular file of the user `uid` with mode exactly 0600.
#[cfg(unix)]
fn datei_quelle_gut(ist_datei: bool, eigner: u32, modus: u32, uid: u32) -> bool {
    ist_datei && eigner == uid && modus & 0o7777 == 0o600
}

/// `pfad` is a real path (no link) of a folder that belongs to `uid`.
#[cfg(unix)]
fn eigener_ordner(pfad: &Path, uid: u32, was: &'static str) -> Result<(), Fehler> {
    let meta = quelle_pruefen(pfad)?;
    if meta.is_dir() && meta.uid() == uid {
        Ok(())
    } else {
        Err(Fehler::QuelleUnbrauchbar(was))
    }
}

/// The folder has no entry (an unreadable folder counts as not empty).
#[cfg(unix)]
fn ordner_leer(pfad: &Path) -> bool {
    fs::read_dir(pfad).is_ok_and(|mut eintraege| eintraege.next().is_none())
}

/// `bwrap` is an executable regular file: missing is [`Fehler::BwrapFehlt`], anything else [`Fehler::BwrapNichtAusfuehrbar`].
#[cfg(unix)]
fn bwrap_datei_pruefen(bwrap: &Path) -> Result<(), Fehler> {
    let meta = fs::metadata(bwrap).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Fehler::BwrapFehlt,
        _ => Fehler::BwrapNichtAusfuehrbar,
    })?;
    if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
        return Err(Fehler::BwrapNichtAusfuehrbar);
    }
    Ok(())
}

/// The host checks of the small enclosure of [`crate::Vorentpacken`]: platform, `bwrap`, the cache folder an own
/// folder without links, the program folders real folders.
///
/// # Errors
/// [`Fehler::NichtVerfuegbar`], [`Fehler::BwrapFehlt`], [`Fehler::BwrapNichtAusfuehrbar`] or
/// [`Fehler::QuelleUnbrauchbar`].
#[cfg(unix)]
pub fn vorentpacken_pruefen(v: &crate::Vorentpacken, bwrap: &Path) -> Result<(), Fehler> {
    if !cfg!(target_os = "linux") {
        return Err(Fehler::NichtVerfuegbar);
    }
    bwrap_datei_pruefen(bwrap)?;
    eigener_ordner(&v.cache, v.system.uid, "cache folder is not an own folder")?;
    for ordner in &v.programmordner {
        if !quelle_pruefen(ordner)?.is_dir() {
            return Err(Fehler::QuelleUnbrauchbar("program folder is not a folder"));
        }
    }
    Ok(())
}

impl Profil {
    /// Checks the host before the start. Order: the working folder (contract 1, step 5), then the platform, `bwrap`,
    /// the terminal, the other mount sources and last the test run of the finished profile with `true`. The first
    /// failure is returned; nothing of the session is changed.
    ///
    /// # Errors
    /// [`Fehler::ArbeitsordnerFehlt`], [`Fehler::NichtVerfuegbar`], [`Fehler::BwrapFehlt`],
    /// [`Fehler::BwrapNichtAusfuehrbar`], [`Fehler::NamensraumGesperrt`], [`Fehler::TiocstiOffen`],
    /// [`Fehler::QuelleUnbrauchbar`] or [`Fehler::Probelauf`].
    pub fn vorpruefen(&self) -> Result<(), Fehler> {
        if !self.beschreibung.arbeitsordner.is_dir() {
            return Err(Fehler::ArbeitsordnerFehlt);
        }
        if !cfg!(target_os = "linux") {
            return Err(Fehler::NichtVerfuegbar);
        }
        #[cfg(unix)]
        {
            self.unix_pruefen()
        }
        // A system without Unix has no enclosure; this is never reached (the platform check above answers first).
        #[cfg(not(unix))]
        {
            Err(Fehler::NichtVerfuegbar)
        }
    }
}

#[cfg(unix)]
impl Profil {
    fn unix_pruefen(&self) -> Result<(), Fehler> {
        self.bwrap_pruefen()?;
        let wert = fs::read_to_string(TIOCSTI_DATEI).ok();
        let release = fs::read_to_string(OSRELEASE_DATEI).unwrap_or_default();
        if !tiocsti_geschlossen(wert.as_deref(), &release) {
            return Err(Fehler::TiocstiOffen);
        }
        self.quellen_pruefen()?;
        self.probelauf()
    }

    fn bwrap_pruefen(&self) -> Result<(), Fehler> {
        bwrap_datei_pruefen(&self.bwrap)
    }

    /// The sources exist as real paths of the right kind: the working folder and the state folder belong to the own
    /// user, the state folder is empty unless the session continues a conversation, the sockets are sockets.
    fn quellen_pruefen(&self) -> Result<(), Fehler> {
        let b = &self.beschreibung;
        let uid = b.system.uid;
        eigener_ordner(&b.arbeitsordner, uid, "working folder is not an own folder")?;
        for ordner in &b.programmordner {
            if !quelle_pruefen(ordner)?.is_dir() {
                return Err(Fehler::QuelleUnbrauchbar("program folder is not a folder"));
            }
        }
        let leer = "state folder is not an empty own folder";
        eigener_ordner(&b.zustand.host, uid, leer)?;
        if !ordner_leer(&b.zustand.host) {
            return Err(Fehler::QuelleUnbrauchbar(leer));
        }
        if let Some(g) = &b.zustand.gespraech {
            let leer = "conversation folder is not an empty own folder";
            eigener_ordner(g, uid, leer)?;
            if !b.zustand.fortsetzen && !ordner_leer(g) {
                return Err(Fehler::QuelleUnbrauchbar(leer));
            }
        }
        let relay = b.cloud.iter().map(|c| &c.weiterreicher);
        for socket in std::iter::once(&b.such_socket)
            .chain(b.modell_socket.iter())
            .chain(relay)
        {
            if !quelle_pruefen(socket)?.file_type().is_socket() {
                return Err(Fehler::QuelleUnbrauchbar("socket is not a socket"));
            }
        }
        if let Some(weiter) = &b.lokal_weiter {
            let programm = quelle_pruefen(weiter)?;
            if !programm.is_file() || programm.permissions().mode() & 0o111 == 0 {
                return Err(Fehler::QuelleUnbrauchbar(
                    "forwarder program is not an executable file",
                ));
            }
        }
        for d in &b.dateien {
            let meta = quelle_pruefen(&d.quelle)?;
            if !datei_quelle_gut(meta.is_file(), meta.uid(), meta.mode(), uid) {
                return Err(Fehler::QuelleUnbrauchbar(
                    "configuration file is not an own regular file of mode 0600",
                ));
            }
        }
        if let Some(p) = &b.paket
            && crate::paket::paket_pruefen(&p.quelle, &p.plattform, uid).is_err()
        {
            return Err(Fehler::QuelleUnbrauchbar(
                "harness package is not a complete own folder without links",
            ));
        }
        if let Some(c) = &b.cloud {
            let programm = quelle_pruefen(&c.weiter_programm)?;
            if !programm.is_file() || programm.permissions().mode() & 0o111 == 0 {
                return Err(Fehler::QuelleUnbrauchbar(
                    "forwarder program is not an executable file",
                ));
            }
            let bundle = quelle_pruefen(&c.bundle)?;
            if !bundle.is_file() || bundle.permissions().mode() & 0o444 == 0 {
                return Err(Fehler::QuelleUnbrauchbar("bundle is not a readable file"));
            }
        }
        Ok(())
    }

    /// Runs the finished profile with `true`. It catches what the other checks cannot: a user namespace the system
    /// refuses (on Ubuntu without the `AppArmor` profile of `bwrap`: `Failed RTM_NEWADDR`), a mount that does not work.
    fn probelauf(&self) -> Result<(), Fehler> {
        self.probelauf_frist(PROBE_FRIST)
    }

    fn probelauf_frist(&self, frist: Duration) -> Result<(), Fehler> {
        lauf_pruefen(
            &self.bwrap,
            &self.argumente_mit(&["/usr/bin/true".to_owned()]),
            frist,
        )
    }
}

/// Runs `bwrap` with `args` and an empty environment, waits at most `frist` and judges the end: success is `Ok`, a
/// refusal of the user namespace is [`Fehler::NamensraumGesperrt`], any other end is [`Fehler::Probelauf`]. Shared by
/// the test run of a finished profile and by the self test of the starter.
#[cfg(unix)]
pub(crate) fn lauf_pruefen(bwrap: &Path, args: &[String], frist: Duration) -> Result<(), Fehler> {
    let mut kind = Command::new(bwrap)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| Fehler::BwrapNichtAusfuehrbar)?;
    let ende = Instant::now() + frist;
    let status = loop {
        match kind.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if !ende.saturating_duration_since(Instant::now()).is_zero() => {
                std::thread::sleep(Duration::from_millis(5));
            }
            _ => {
                let _ = kind.kill();
                let _ = kind.wait();
                return Err(Fehler::Probelauf {
                    status: None,
                    meldung: "no answer within the time limit".to_owned(),
                });
            }
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut meldung = Vec::new();
    if let Some(fehlerstrom) = kind.stderr.take() {
        let _ = fehlerstrom
            .take(MELDUNG_MAX as u64)
            .read_to_end(&mut meldung);
    }
    let meldung = kurz(&meldung);
    if namensraum_gesperrt(&meldung) {
        return Err(Fehler::NamensraumGesperrt);
    }
    Err(Fehler::Probelauf {
        status: status.code(),
        meldung,
    })
}

#[cfg(test)]
mod tests;
