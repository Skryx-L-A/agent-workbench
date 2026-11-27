//! The run of `werkbank-anmelden copilot [--neu]`: throw-away home, login, token into the key file, check of the
//! real home. No output of its own: the caller prints. The token is never in an error, a path or a message.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::fehler::Fehler;
use crate::form::{CONFIG_GRENZE, Fund, config_hat_anmeldung, token_aus_config, token_form_ok};

/// Name of the key file in the store of the holder: the secret `anmeldung.copilot` (contract of the holder, section 4).
pub const SCHLUESSELDATEI: &str = "anmeldung.copilot";

/// The levels of the store of the holder below the home: `<home>/.local/state/agent-workbench/halter/geheim`. Each is
/// opened relative to the one before, never as a joined path.
pub const ABLAGE: [&str; 5] = [".local", "state", "agent-workbench", "halter", "geheim"];

/// The root of the throw-away homes: the folder of the holder above the store (`ABLAGE` without `geheim`),
/// `<home>/.local/state/agent-workbench/halter`. The enclosure never mounts it (`ANMELDE_ORDNER`), for no harness, so no
/// agent sees the token of a running or broken login; step 3 has made and checked its levels before the home is made.
/// alt: `/var/tmp`, which an agent could mount as its working folder.
#[must_use]
pub fn wegwerf_wurzel(heim: &Path) -> PathBuf {
    ABLAGE[..ABLAGE.len() - 1]
        .iter()
        .fold(heim.to_path_buf(), |p, ebene| p.join(ebene))
}

/// Variables of the caller that the CLI may keep (network of the firm). Nothing else passes.
const DURCHREICHEN: [&str; 14] = [
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
    "TERM",
    "LANG",
    "LC_ALL",
];

/// How long the CLI gets after SIGTERM before it is killed.
const NACHSPIELZEIT: Duration = Duration::from_secs(2);
/// How long a keychain tool may take.
const BUND_FRIST: Duration = Duration::from_secs(10);
/// Poll step of the wait loops.
const TAKT: Duration = Duration::from_millis(20);

/// Everything the run needs from its surroundings. `main` builds it from the real environment; the tests build it
/// by hand (the seams of `main` exist only in the test build, feature `pruefbau`, and are valid only with `WB_TESTUMGEBUNG=1`).
pub struct Umgebung {
    /// Home of the user: field 6 of the own entry in `/etc/passwd`, an absolute path. The store of the holder lies
    /// below it (`ABLAGE`).
    pub heim: PathBuf,
    /// Value of PATH of the caller: where `copilot`, `secret-tool` and `security` are looked up.
    pub suchpfad: OsString,
    /// Folder `.copilot` of the real home (`COPILOT_HOME`, or `<HOME>/.copilot`).
    pub echtes_copilot_heim: PathBuf,
    /// Folder under which the throw-away home is made.
    pub wegwerf_wurzel: PathBuf,
    /// Own user id.
    pub uid: u32,
    /// Variables of the caller that pass to the CLI (see `DURCHREICHEN`).
    pub durchreichen: Vec<(OsString, OsString)>,
}

/// What a successful run reports besides the stored key.
#[derive(Debug, PartialEq, Eq)]
pub struct Erfolg {
    /// `false`: no keychain tool was usable, the keychain of the real home was not checked.
    pub bund_geprueft: bool,
}

/// The variables of `std::env` that pass to the CLI.
#[must_use]
pub fn durchreichen_aus_umgebung() -> Vec<(OsString, OsString)> {
    DURCHREICHEN
        .iter()
        .filter_map(|n| std::env::var_os(n).map(|v| (OsString::from(*n), v)))
        .collect()
}

/// The throw-away home. It is deleted when the value goes away, on every path, also on a panic; the delete goes
/// through the open folder above it (`baum_loeschen`), never through a joined path.
struct Wegwerf {
    wurzel: PathBuf,
    oben: OwnedFd,
    name: String,
}

impl Wegwerf {
    fn neu(unter: &Path) -> Result<Wegwerf, Fehler> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let name = format!("wb-anmelden-{}-{nanos}", std::process::id());
        let oben = ordner_oeffnen(unter).map_err(|_| Fehler::HeimNichtAnlegbar)?;
        ebene_anlegen(&oben, &name).map_err(|_| Fehler::HeimNichtAnlegbar)?;
        let w = Wegwerf {
            wurzel: unter.join(&name),
            oben,
            name,
        };
        fs::set_permissions(&w.wurzel, fs::Permissions::from_mode(0o700))
            .map_err(|_| Fehler::HeimNichtAnlegbar)?;
        ordner_0700(&w.heim())?;
        ordner_0700(&w.copilot_heim())?;
        let einstellungen = w.copilot_heim().join("settings.json");
        let mut datei = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(einstellungen)
            .map_err(|_| Fehler::HeimNichtAnlegbar)?;
        datei
            .set_permissions(fs::Permissions::from_mode(0o600))
            .and_then(|()| datei.write_all(b"{\"storeTokenPlaintext\":true}\n"))
            .map_err(|_| Fehler::HeimNichtAnlegbar)?;
        Ok(w)
    }

    fn heim(&self) -> PathBuf {
        self.wurzel.join("heim")
    }

    fn copilot_heim(&self) -> PathBuf {
        self.wurzel.join("heim").join(".copilot")
    }
}

/// Makes one folder with exactly 0700, whatever the umask is.
fn ordner_0700(pfad: &Path) -> Result<(), Fehler> {
    let mut bauer = fs::DirBuilder::new();
    bauer.mode(0o700);
    bauer.create(pfad).map_err(|_| Fehler::HeimNichtAnlegbar)?;
    fs::set_permissions(pfad, fs::Permissions::from_mode(0o700))
        .map_err(|_| Fehler::HeimNichtAnlegbar)
}

impl Drop for Wegwerf {
    fn drop(&mut self) {
        let _ = baum_loeschen(&self.oben, self.name.as_str());
    }
}

/// Deletes the entry `name` of the open folder `ordner` with everything below it. Every level is opened relative to
/// the one above with `O_NOFOLLOW`, every entry is looked at without following a link and removed relative to its
/// open folder: a link is removed as a link, its target stays. Nothing is reached through a joined path.
pub(crate) fn baum_loeschen<P: rustix::path::Arg + Copy>(
    ordner: &OwnedFd,
    name: P,
) -> rustix::io::Result<()> {
    use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags};
    let st = rustix::fs::statat(ordner, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if FileType::from_raw_mode(st.st_mode) != FileType::Directory {
        return rustix::fs::unlinkat(ordner, name, AtFlags::empty());
    }
    let fd = rustix::fs::openat(
        ordner,
        name,
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )?;
    let mut namen = Vec::new();
    let mut liste = Dir::read_from(&fd)?;
    while let Some(eintrag) = liste.read() {
        let eintrag = eintrag?;
        let n = eintrag.file_name();
        if n.to_bytes() != b"." && n.to_bytes() != b".." {
            namen.push(n.to_owned());
        }
    }
    drop(liste);
    for n in &namen {
        baum_loeschen(&fd, n.as_c_str())?;
    }
    rustix::fs::unlinkat(ordner, name, AtFlags::REMOVEDIR)
}

/// Prefix of the name of a throw-away home: `wb-anmelden-<pid>-<nanos>`.
const WEGWERF_PRAEFIX: &str = "wb-anmelden-";

/// The pid in the name of a throw-away home, or `None` when the name has another form
/// (`wb-anmelden-<digits>-<digits>`, nothing else) or the pid is 0 or does not fit.
#[must_use]
pub fn pid_aus_name(name: &str) -> Option<rustix::process::Pid> {
    let rest = name.strip_prefix(WEGWERF_PRAEFIX)?;
    let (pid, nanos) = rest.split_once('-')?;
    let ziffern = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    if !ziffern(pid) || !ziffern(nanos) {
        return None;
    }
    rustix::process::Pid::from_raw(pid.parse::<i32>().ok()?)
}

/// Does a run of this user with `pid` still live? Our own pid does not count (a folder with our pid at the start is
/// an old one). A process we may not signal is not ours. An unclear answer counts as living: then nothing is deleted.
pub(crate) fn lauf_lebt(pid: rustix::process::Pid, eigene_pid: u32) -> bool {
    if u32::try_from(pid.as_raw_pid()).is_ok_and(|p| p == eigene_pid) {
        return false;
    }
    lebt_nach_signaltest(rustix::process::test_kill_process(pid))
}

/// The answer of the signal test (signal 0) as living or dead: no such process (`ESRCH`) and a process we may not
/// signal (`EPERM`) are dead for us; success and every other error count as living. Separate from the signal itself,
/// so a test can give it `EPERM`: an own pid namespace (the enclosure of the mutation gate) has no process the user
/// may not signal.
fn lebt_nach_signaltest(antwort: rustix::io::Result<()>) -> bool {
    match antwort {
        Ok(()) => true,
        Err(e) => e != rustix::io::Errno::SRCH && e != rustix::io::Errno::PERM,
    }
}

/// Deletes the left-overs of earlier runs of this user under `wurzel`: a folder named `wb-anmelden-<pid>-<nanos>`,
/// a real folder (never a link), owned by `uid`, whose run is dead. Others stay. `wurzel` is opened once without
/// following a link at its last part; every look and every delete is relative to it (`baum_loeschen`). Nothing is
/// followed and nothing is shown. Returns how many were deleted.
///
/// # Errors
/// `RestNichtLoeschbar` when a matching left-over cannot be deleted.
pub fn reste_raeumen(wurzel: &Path, uid: u32, eigene_pid: u32) -> Result<usize, Fehler> {
    use rustix::fs::{AtFlags, Dir, FileType};
    let Ok(fd) = ordner_oeffnen(wurzel) else {
        return Ok(0);
    };
    let Ok(mut liste) = Dir::read_from(&fd) else {
        return Ok(0);
    };
    let mut kandidaten = Vec::new();
    while let Some(Ok(eintrag)) = liste.read() {
        if let Ok(name) = eintrag.file_name().to_str()
            && let Some(pid) = pid_aus_name(name)
        {
            kandidaten.push((name.to_owned(), pid));
        }
    }
    drop(liste);
    let mut geloescht = 0;
    for (name, pid) in kandidaten {
        let Ok(st) = rustix::fs::statat(&fd, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) else {
            continue;
        };
        if FileType::from_raw_mode(st.st_mode) != FileType::Directory
            || st.st_uid != uid
            || lauf_lebt(pid, eigene_pid)
        {
            continue;
        }
        baum_loeschen(&fd, name.as_str()).map_err(|_| Fehler::RestNichtLoeschbar)?;
        geloescht += 1;
    }
    Ok(geloescht)
}

/// The temp files of the command (section 3a): the temp file of the key in the target folder (both ways) and the
/// temp file of the new `config.json` in the folder of the CLI (`--uebernehmen`). Each carries the pid of its run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempArt {
    /// `.anmeldung.copilot.<pid>.tmp` in the store of the holder; it may hold a token.
    Schluessel,
    /// `.config.json.wb-anmelden-<pid>.tmp` in the folder of the CLI; it holds `config.json` without the token.
    Config,
}

impl TempArt {
    fn teile(self) -> (&'static str, &'static str) {
        match self {
            TempArt::Schluessel => (".anmeldung.copilot.", ".tmp"),
            TempArt::Config => (".config.json.wb-anmelden-", ".tmp"),
        }
    }

    /// The name of the temp file of the run with `pid`.
    #[must_use]
    pub fn name(self, pid: u32) -> String {
        let (vorn, hinten) = self.teile();
        format!("{vorn}{pid}{hinten}")
    }

    /// The pid in `name`, or `None` when the name has another form (nothing but digits between the fixed parts) or
    /// the pid is 0 or does not fit.
    #[must_use]
    pub fn pid(self, name: &str) -> Option<rustix::process::Pid> {
        let (vorn, hinten) = self.teile();
        let ziffern = name.strip_prefix(vorn)?.strip_suffix(hinten)?;
        if ziffern.is_empty() || !ziffern.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        rustix::process::Pid::from_raw(ziffern.parse::<i32>().ok()?)
    }
}

/// Is `name` a single entry of a folder: not empty, no `/`, not `.` and not `..`? Every name that the command hands
/// to a call relative to an open folder passes this check first.
#[must_use]
pub fn einfacher_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && name != "." && name != ".."
}

/// Opens a folder without following a link at its last part.
pub(crate) fn ordner_oeffnen(pfad: &Path) -> rustix::io::Result<OwnedFd> {
    use rustix::fs::{Mode, OFlags};
    rustix::fs::open(
        pfad,
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )
}

/// Deletes the temp files of `art` that dead runs of this user left in `ordner`: a regular file (never a link),
/// owned by `uid`, named after `TempArt`, whose run is dead (as in `reste_raeumen`). The folder is opened once
/// without following a link; every look and every delete is relative to it. A missing or unreadable folder has none.
///
/// # Errors
/// `RestNichtLoeschbar` when a matching temp file cannot be deleted.
pub fn temp_reste_raeumen(
    ordner: &Path,
    art: TempArt,
    uid: u32,
    eigene_pid: u32,
) -> Result<usize, Fehler> {
    let Ok(fd) = ordner_oeffnen(ordner) else {
        return Ok(0);
    };
    temp_reste_in(&fd, art, uid, eigene_pid)
}

/// As `temp_reste_raeumen`, in a folder that is already open (the store of the holder, found by `ablage_suchen`).
///
/// # Errors
/// `RestNichtLoeschbar` when a matching temp file cannot be deleted.
pub fn temp_reste_in(
    fd: &OwnedFd,
    art: TempArt,
    uid: u32,
    eigene_pid: u32,
) -> Result<usize, Fehler> {
    use rustix::fs::{AtFlags, Dir, FileType};
    let Ok(mut liste) = Dir::read_from(fd) else {
        return Ok(0);
    };
    let mut kandidaten = Vec::new();
    while let Some(Ok(eintrag)) = liste.read() {
        if let Ok(name) = eintrag.file_name().to_str()
            && let Some(pid) = art.pid(name)
            && einfacher_name(name)
        {
            kandidaten.extend([(name.to_owned(), pid)]);
        }
    }
    let mut geloescht = 0;
    for (name, pid) in kandidaten {
        let Ok(st) = rustix::fs::statat(fd, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) else {
            continue;
        };
        if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile
            || st.st_uid != uid
            || lauf_lebt(pid, eigene_pid)
        {
            continue;
        }
        rustix::fs::unlinkat(fd, name.as_str(), AtFlags::empty())
            .map_err(|_| Fehler::RestNichtLoeschbar)?;
        geloescht += 1;
    }
    Ok(geloescht)
}

/// Looks `name` up in the directories of `suchpfad`; the first regular file with an execute bit wins.
#[must_use]
pub fn finde(suchpfad: &OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(suchpfad)
        .map(|ordner| ordner.join(name))
        .find(|p| ist_ausfuehrbar(p))
}

fn ist_ausfuehrbar(pfad: &Path) -> bool {
    fs::metadata(pfad).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// PATH of the CLI: the directories of the caller without those that hold an executable `gh`.
#[must_use]
pub fn pfad_ohne_gh(suchpfad: &OsStr) -> OsString {
    let behalten: Vec<PathBuf> = std::env::split_paths(suchpfad)
        .filter(|ordner| !ist_ausfuehrbar(&ordner.join("gh")))
        .collect();
    std::env::join_paths(behalten).unwrap_or_default()
}

/// Opens the level `name` relative to `ordner`: a folder, never following a link.
fn ebene_oeffnen(ordner: &OwnedFd, name: &str) -> rustix::io::Result<OwnedFd> {
    use rustix::fs::{Mode, OFlags};
    rustix::fs::openat(
        ordner,
        name,
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    )
}

/// The store of the holder as it is, without making or checking a level (left-overs and the look for the key file
/// before step 3): `None` when the home or a level is missing, a link or no folder.
#[must_use]
pub fn ablage_suchen(heim: &Path) -> Option<OwnedFd> {
    let mut fd = ordner_oeffnen(heim).ok()?;
    for name in ABLAGE {
        fd = ebene_oeffnen(&fd, name).ok()?;
    }
    Some(fd)
}

/// A level of the store (or the home): owned by `uid` and not writable by group or others; `geheim` has exactly 0700.
fn ebene_pruefen(fd: &OwnedFd, uid: u32, geheim: bool) -> Result<(), Fehler> {
    let st = rustix::fs::fstat(fd).map_err(|_| Fehler::ZielNichtSchreibbar)?;
    if st.st_uid == uid && st.st_mode & 0o022 == 0 && (!geheim || st.st_mode & 0o7777 == 0o700) {
        Ok(())
    } else {
        Err(Fehler::ZielNichtSchreibbar)
    }
}

/// Makes the missing level `name` with 0700 under the umask 077, whatever the umask of the caller is.
fn ebene_anlegen(ordner: &OwnedFd, name: &str) -> rustix::io::Result<()> {
    use rustix::fs::Mode;
    let alt = rustix::process::umask(Mode::from_raw_mode(0o077));
    let ergebnis = rustix::fs::mkdirat(ordner, name, Mode::from_raw_mode(0o700));
    rustix::process::umask(alt);
    ergebnis
}

/// Step 3 (and step 6 of the takeover): opens the store of the holder below `heim` level by level without following
/// a link, checks the home and every level (`ebene_pruefen`) before it goes one deeper, and makes a missing level with
/// 0700. Made at the same time by another process (`EEXIST`): the check of the present level holds. A level that
/// breaks a rule stops the walk before anything below it is made.
///
/// # Errors
/// `ZielNichtSchreibbar`.
pub fn ziel_oeffnen(heim: &Path, uid: u32) -> Result<OwnedFd, Fehler> {
    let mut fd = ordner_oeffnen(heim).map_err(|_| Fehler::ZielNichtSchreibbar)?;
    ebene_pruefen(&fd, uid, false)?;
    for (i, name) in ABLAGE.iter().enumerate() {
        let naechste = match ebene_oeffnen(&fd, name) {
            Err(rustix::io::Errno::NOENT) => {
                match ebene_anlegen(&fd, name) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(_) => return Err(Fehler::ZielNichtSchreibbar),
                }
                ebene_oeffnen(&fd, name)
            }
            anders => anders,
        }
        .map_err(|_| Fehler::ZielNichtSchreibbar)?;
        ebene_pruefen(&naechste, uid, i + 1 == ABLAGE.len())?;
        fd = naechste;
    }
    Ok(fd)
}

/// Runs the whole command.
///
/// # Errors
/// Every `Fehler` of contract 9. On `EchteAnmeldung` the key file is already stored.
pub fn anmelden(umg: &Umgebung, neu: bool, signal: &AtomicUsize) -> Result<Erfolg, Fehler> {
    if !neu
        && ablage_suchen(&umg.heim).is_some_and(|fd| {
            rustix::fs::statat(&fd, SCHLUESSELDATEI, rustix::fs::AtFlags::SYMLINK_NOFOLLOW).is_ok()
        })
    {
        return Err(Fehler::DateiDa);
    }
    let ablage = ziel_oeffnen(&umg.heim, umg.uid)?;
    let cli = finde(&umg.suchpfad, "copilot").ok_or(Fehler::CliFehlt)?;
    let wegwerf = Wegwerf::neu(&umg.wegwerf_wurzel)?;
    login_laufen_lassen(umg, &cli, &wegwerf, signal)?;
    let token = token_lesen(&wegwerf)?;
    signal_pruefen(signal)?;
    ablegen(&ablage, &token, neu)?;
    drop(wegwerf);
    echtes_heim_pruefen(umg)
}

fn signal_pruefen(signal: &AtomicUsize) -> Result<(), Fehler> {
    match signal.load(Ordering::SeqCst) {
        0 => Ok(()),
        n => Err(Fehler::Signal(i32::try_from(n).unwrap_or(0))),
    }
}

fn login_laufen_lassen(
    umg: &Umgebung,
    cli: &Path,
    wegwerf: &Wegwerf,
    signal: &AtomicUsize,
) -> Result<(), Fehler> {
    let mut befehl = Command::new(cli);
    befehl
        .args(["login", "--device-code"])
        .env_clear()
        .envs(umg.durchreichen.iter().map(|(k, v)| (k, v)))
        .env("HOME", wegwerf.heim())
        .env("COPILOT_HOME", wegwerf.copilot_heim())
        .env("PATH", pfad_ohne_gh(&umg.suchpfad))
        // Fixed, never from the caller (Schritt 6 of the contract): the CLI 1.0.88 uses rustls with the system truststore, the variable
        // does no harm and holds if a later CLI moves to TLS of Node.
        .env("NODE_USE_SYSTEM_CA", "1")
        .current_dir(wegwerf.heim());
    // The child holds the token: it inherits the shielded state (section 3c). Checked again right before it starts, fail closed.
    crate::schutz::vor_dem_kind()?;
    let mut kind = befehl.spawn().map_err(|_| Fehler::LoginAbgebrochen)?;
    let status = loop {
        if signal.load(Ordering::SeqCst) != 0 {
            beenden(&mut kind);
            return signal_pruefen(signal);
        }
        match kind.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(TAKT),
            Err(_) => {
                beenden(&mut kind);
                return Err(Fehler::LoginAbgebrochen);
            }
        }
    };
    signal_pruefen(signal)?;
    if status.success() {
        Ok(())
    } else {
        Err(Fehler::LoginAbgebrochen)
    }
}

/// SIGTERM, a grace time, then SIGKILL; always reaped.
fn beenden(kind: &mut Child) {
    if let Some(pid) = rustix::process::Pid::from_child(kind).into() {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    }
    let ende = Instant::now() + NACHSPIELZEIT;
    while ende.checked_duration_since(Instant::now()).is_some() {
        if matches!(kind.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(TAKT);
    }
    let _ = kind.kill();
    let _ = kind.wait();
}

fn token_lesen(wegwerf: &Wegwerf) -> Result<crate::form::Token, Fehler> {
    let mut text = Vec::new();
    File::open(wegwerf.copilot_heim().join("config.json"))
        .and_then(|f| f.take(CONFIG_GRENZE).read_to_end(&mut text))
        .map_err(|_| Fehler::KeinToken)?;
    match token_aus_config(&text) {
        Fund::Eins(token) => {
            if token_form_ok(token.as_str()) {
                Ok(token)
            } else {
                Err(Fehler::FormFalsch)
            }
        }
        _ => Err(Fehler::KeinToken),
    }
}

/// Step 9: temp file `.anmeldung.copilot.<pid>.tmp` (0600, `O_EXCL`, synced) in the open store, then rename (`--neu`)
/// or hard link (no replace), all relative to the store; the temp name goes in every case, then fsync of the store.
fn ablegen(ablage: &OwnedFd, token: &crate::form::Token, neu: bool) -> Result<(), Fehler> {
    use rustix::fs::AtFlags;
    let (temp, _) = crate::uebernehmen::schluessel_temp_schreiben(ablage, token)?;
    let ergebnis = if neu {
        rustix::fs::renameat(ablage, temp.name.as_str(), ablage, SCHLUESSELDATEI)
            .map_err(|_| Fehler::ZielNichtSchreibbar)
    } else {
        match rustix::fs::linkat(
            ablage,
            temp.name.as_str(),
            ablage,
            SCHLUESSELDATEI,
            AtFlags::empty(),
        ) {
            Ok(()) => Ok(()),
            Err(e) => Err(fehler_des_verweises(e)),
        }
    };
    drop(temp);
    let _ = rustix::fs::fsync(ablage);
    ergebnis
}

/// The error of the hard link to the key file: a name that is taken (`EEXIST`) is `DateiDa`, any other error
/// `ZielNichtSchreibbar`. Separate, so a test can give it an error other than `EEXIST`: no test without root can make
/// the link fail otherwise once the temp file is written in the same store.
fn fehler_des_verweises(e: rustix::io::Errno) -> Fehler {
    if e == rustix::io::Errno::EXIST {
        Fehler::DateiDa
    } else {
        Fehler::ZielNichtSchreibbar
    }
}

/// After the key is stored: no login of the CLI may lie in the real home. Reads `config.json` of the real home and
/// asks the keychain tool that exists; nothing of the content is kept or shown.
pub(crate) fn echtes_heim_pruefen(umg: &Umgebung) -> Result<Erfolg, Fehler> {
    if let Ok(text) = datei_lesen(&umg.echtes_copilot_heim.join("config.json"))
        && config_hat_anmeldung(&text)
    {
        return Err(Fehler::EchteAnmeldung);
    }
    match bund_pruefen(&umg.suchpfad) {
        Bund::Gefunden => Err(Fehler::EchteAnmeldung),
        Bund::Leer => Ok(Erfolg {
            bund_geprueft: true,
        }),
        Bund::NichtPruefbar => Ok(Erfolg {
            bund_geprueft: false,
        }),
    }
}

fn datei_lesen(pfad: &Path) -> std::io::Result<Vec<u8>> {
    let mut text = Vec::new();
    File::open(pfad)?
        .take(CONFIG_GRENZE)
        .read_to_end(&mut text)?;
    Ok(text)
}

/// Result of the keychain question.
#[derive(Debug, PartialEq, Eq)]
pub enum Bund {
    /// A tool answered: an entry `copilot-cli` exists.
    Gefunden,
    /// A tool answered: there is none.
    Leer,
    /// No tool is installed, or none answered in time.
    NichtPruefbar,
}

/// Asks `secret-tool lookup service copilot-cli` (Linux) and `security find-generic-password -s copilot-cli`
/// (macOS), whichever is in the PATH. Output is discarded: the exit code is the answer. Measurement point M3.
#[must_use]
pub fn bund_pruefen(suchpfad: &OsStr) -> Bund {
    const WERKZEUGE: [(&str, &[&str]); 2] = [
        ("secret-tool", &["lookup", "service", "copilot-cli"]),
        ("security", &["find-generic-password", "-s", "copilot-cli"]),
    ];
    let mut antwort = Bund::NichtPruefbar;
    for (name, args) in WERKZEUGE {
        let Some(pfad) = finde(suchpfad, name) else {
            continue;
        };
        match tool_laufen_lassen(&pfad, args) {
            Some(true) => return Bund::Gefunden,
            Some(false) => antwort = Bund::Leer,
            None => {}
        }
    }
    antwort
}

fn tool_laufen_lassen(pfad: &Path, args: &[&str]) -> Option<bool> {
    let mut kind = Command::new(pfad)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let ende = Instant::now() + BUND_FRIST;
    while ende.checked_duration_since(Instant::now()).is_some() {
        match kind.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) => std::thread::sleep(TAKT),
            Err(_) => return None,
        }
    }
    let _ = kind.kill();
    let _ = kind.wait();
    None
}

/// The home of `uid` in the text of `/etc/passwd`: field 6 of the first entry with this user id, only if it is an
/// absolute path. `HOME` and `XDG_STATE_HOME` are never read (contract 9, section 2).
#[must_use]
pub fn heim_aus_passwd(text: &str, uid: u32) -> Option<PathBuf> {
    text.lines()
        .find_map(|zeile| {
            let mut teile = zeile.split(':');
            let nummer: u32 = teile.nth(2)?.parse().ok()?;
            let heim = teile.nth(2)?;
            (nummer == uid).then_some(heim)
        })
        .map(PathBuf::from)
        .filter(|heim| heim.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::{fehler_des_verweises, lebt_nach_signaltest};
    use crate::fehler::Fehler;
    use rustix::io::Errno;

    #[test]
    fn a_taken_name_is_dateida_and_any_other_link_error_is_not_writable() {
        assert_eq!(fehler_des_verweises(Errno::EXIST), Fehler::DateiDa);
        for e in [Errno::ACCESS, Errno::NOENT, Errno::NOSPC, Errno::MLINK] {
            assert_eq!(
                fehler_des_verweises(e),
                Fehler::ZielNichtSchreibbar,
                "{e:?}"
            );
        }
    }

    #[test]
    fn the_answer_of_the_signal_test_is_living_or_dead() {
        assert!(
            lebt_nach_signaltest(Ok(())),
            "a process we may signal lives"
        );
        assert!(!lebt_nach_signaltest(Err(Errno::SRCH)), "no such process");
        assert!(
            !lebt_nach_signaltest(Err(Errno::PERM)),
            "a process we may not signal is not ours"
        );
        assert!(
            lebt_nach_signaltest(Err(Errno::INVAL)),
            "an unclear answer counts as living"
        );
    }
}
