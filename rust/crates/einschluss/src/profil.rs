//! The fixed bubblewrap profile: the argument list is built from the description, then checked once more as text.
//!
//! The profile is a whitelist. The enclosure sees `/usr`, five files of `/etc`, a private `/proc`, `/dev`, `/tmp`,
//! `/run` and home, the program folders (read-only), the empty state folder, the working folder and the sockets of the
//! session. There is no `--ro-bind / /`: what the list does not name does not exist inside.
//!
//! | Switch | Why |
//! | --- | --- |
//! | `--unshare-user` | own user namespace; the user keeps the same kernel user (no privilege needed) |
//! | `--disable-userns` | no further user namespace inside, so the enclosure cannot build a looser one |
//! | `--unshare-pid` | own process space: the search process and every other process of the host do not exist inside (E3) |
//! | `--unshare-net` | own network space with `lo` only: no address of the host is reachable |
//! | `--unshare-ipc`, `--unshare-uts`, `--unshare-cgroup` | no shared memory, no host name, no cgroup tree of the host |
//! | `--cap-drop ALL` | a safeguard only: without it the capability sets are empty too (measured by the Ubuntu check, `CapEff` and `CapBnd` 0), so it is no evidence of a boundary; `--cap-add` is refused |
//! | `--clearenv` and five `--setenv` | the harness gets `HOME`, `PATH`, `TERM`, `LANG`, `TMPDIR` and nothing of the caller |
//! | `--tmpfs` for `/tmp`, `/run`, home | private and empty; the home is empty before any mount below it |
//! | `--dir /run/user/<uid>` | an empty folder where the bus and the key ring of the person would be |
//! | no `--new-session` | the harness keeps its terminal (size changes, Ctrl-C); the start checks that `TIOCSTI` is closed |
//! | `--ro-bind /dev/null /usr/bin/werkbank-ordnermarke` | only when that file exists on the host: the command of the human is hidden inside (`/usr` is visible); the one bind of `/dev/null` the text check accepts |
//! | `--ro-bind <package> <home>/.cache/copilot/pkg/<platform>/<version>` | only for the cloud harness with a package: the folder the CLI extracts itself into otherwise (see [`crate::paket`]); read-only, checked on the host before |
//! | `--die-with-parent` | when the starting process dies, the harness dies with it (measured; the case `elternbindung`) |
//!
//! There is no `seccomp` filter in this profile. A fault of the kernel breaks the enclosure; that is a follow-up, not
//! a claim.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::beschreibung::{
    Beschreibung, COPILOT_PLATZHALTER, Cloud, DATEI_ZIELE, GAST_MODELL_SOCKET, GAST_SUCH_SOCKET,
    GAST_WEITER, HARNESS_COPILOT, HARNESS_PI, LOKAL_PORT, ORDNERMARKE_PFAD, PI_UMGEBUNG,
    WEITER_PORT,
};
use crate::fehler::Fehler;
use crate::pfad::{gast_pfad, innerhalb, normal, ueberlappt};
use crate::verbote::{
    SYSTEM_UNTER_VERBOTEN, arbeitsordner_verboten, beruehrt_anmeldespeicher, hat_geheimnisteil,
};

/// The fixed path of `bwrap`. A tool found through `PATH` could be set by the user, and on Ubuntu the transition of the
/// `AppArmor` profile of the starter (`werkbank-einschluss`) names this path only. No profile exists for `bwrap` itself.
pub const BWRAP_PFAD: &str = "/usr/bin/bwrap";

/// Size of the private `/tmp`.
const TMP_BYTES: u64 = 256 * 1024 * 1024;
/// Size of the private `/run` and of the private home.
pub(crate) const KLEIN_BYTES: u64 = 16 * 1024 * 1024;

/// Switches every profile must carry.
const PFLICHT: [&str; 10] = [
    "--unshare-user",
    "--disable-userns",
    "--unshare-pid",
    "--unshare-net",
    "--unshare-ipc",
    "--unshare-uts",
    "--unshare-cgroup",
    "--die-with-parent",
    "--clearenv",
    "--cap-drop",
];

/// Switches no profile may carry.
const VERBOTEN: [&str; 8] = [
    "--share-net",
    "--new-session",
    "--dev-bind",
    "--dev-bind-try",
    "--userns",
    "--userns2",
    "--pidns",
    "--cap-add",
];

/// Options that put a host path or a link at a place of the enclosure (the place is the second argument).
const EINBINDUNGEN: [&str; 7] = [
    "--bind",
    "--ro-bind",
    "--bind-try",
    "--ro-bind-try",
    "--dev-bind",
    "--dev-bind-try",
    "--symlink",
];

/// The variables the profile may set, and no others (the allow list of the environment). The first five are the
/// fixed base; the next four belong to the cloud harness and are set only with a [`Cloud`]; the last two belong to
/// the harness [`HARNESS_PI`] only, with the fixed values of [`PI_UMGEBUNG`].
const ERLAUBTE_VARIABLEN: [&str; 11] = [
    "HOME",
    "PATH",
    "TERM",
    "LANG",
    "TMPDIR",
    "COPILOT_GITHUB_TOKEN",
    "HTTPS_PROXY",
    "COPILOT_AUTO_UPDATE",
    "COPILOT_MASCOT_BLINK",
    "PI_OFFLINE",
    "PI_TELEMETRY",
];

/// The value of `HTTPS_PROXY` of the cloud harness: the forwarder on the own loopback.
fn proxy_wert() -> String {
    format!("http://127.0.0.1:{WEITER_PORT}")
}

/// A finished, checked profile. Build it with [`Profil::bauen`], check the host with [`Profil::vorpruefen`], start it
/// with [`Profil::befehl`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profil {
    pub(crate) bwrap: PathBuf,
    /// Everything before the `--`.
    pub(crate) kopf: Vec<String>,
    pub(crate) beschreibung: Beschreibung,
}

fn kennung(text: &str) -> bool {
    let b = text.as_bytes();
    let erlaubt = |c: u8, erstes: bool| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || (!erstes && matches!(c, b'.' | b'_' | b'-'))
    };
    !b.is_empty() && b.len() <= 64 && b.iter().enumerate().all(|(i, c)| erlaubt(*c, i == 0))
}

/// The configuration files: each source a normal path, each place one of [`DATEI_ZIELE`] for the harness below the
/// home, no place twice.
fn dateien_pruefen(b: &Beschreibung) -> Result<(), Fehler> {
    let ungueltig = Fehler::BeschreibungUngueltig;
    let mut gesehen: Vec<&Path> = Vec::new();
    for d in &b.dateien {
        if !normal(&d.quelle) {
            return Err(ungueltig("dateien.quelle"));
        }
        let erlaubt = DATEI_ZIELE
            .iter()
            .any(|(harness, rel)| *harness == b.harness && gast_pfad(&b.heim, rel) == d.ziel);
        if !erlaubt {
            return Err(ungueltig("dateien.ziel (not a fixed place of the harness)"));
        }
        if gesehen.contains(&d.ziel.as_path()) {
            return Err(ungueltig("dateien.ziel twice"));
        }
        gesehen.push(&d.ziel);
    }
    Ok(())
}

fn text_ok(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(char::is_control)
}

/// The checks of form: every path normal, plain names, a consistent environment, a program to run.
fn felder_pruefen(b: &Beschreibung) -> Result<(), Fehler> {
    let ungueltig = Fehler::BeschreibungUngueltig;
    if !kennung(&b.harness) {
        return Err(ungueltig("harness"));
    }
    if b.argv.is_empty() || b.argv[0].is_empty() || b.argv.iter().any(|a| a.contains('\0')) {
        return Err(ungueltig("argv"));
    }
    let pfade: [(&Path, &'static str); 4] = [
        (&b.arbeitsordner, "arbeitsordner"),
        (&b.zustand.host, "zustand.host"),
        (&b.heim, "heim"),
        (&b.such_socket, "such_socket"),
    ];
    for (pfad, name) in pfade {
        if !normal(pfad) {
            return Err(ungueltig(name));
        }
    }
    if b.modell_socket.as_deref().is_some_and(|p| !normal(p)) {
        return Err(ungueltig("modell_socket"));
    }
    if let Some(w) = &b.lokal_weiter {
        if b.modell_socket.is_none() {
            return Err(ungueltig("lokal_weiter without modell_socket"));
        }
        if !normal(w) {
            return Err(ungueltig("lokal_weiter"));
        }
    }
    dateien_pruefen(b)?;
    if let Some(c) = &b.cloud {
        if b.harness != HARNESS_COPILOT {
            return Err(ungueltig("cloud (only for the cloud harness)"));
        }
        if b.modell_socket.is_some() {
            return Err(ungueltig("cloud and modell_socket together"));
        }
        if ![&c.weiterreicher, &c.weiter_programm, &c.bundle]
            .into_iter()
            .all(|p| normal(p))
        {
            return Err(ungueltig("cloud"));
        }
    }
    if let Some(p) = &b.paket {
        if b.harness != HARNESS_COPILOT {
            return Err(ungueltig("paket (only for the cloud harness)"));
        }
        if !normal(&p.quelle) || !crate::paket::paket_form_gut(&p.plattform, &p.fassung) {
            return Err(ungueltig("paket"));
        }
    }
    if b.programmordner.iter().any(|p| !normal(p)) {
        return Err(ungueltig("programmordner"));
    }
    if b.geschuetzt.iter().any(|p| !normal(p)) {
        return Err(ungueltig("geschuetzt"));
    }
    let name = &b.zustand.name;
    if !text_ok(name) || name == "." || name == ".." || name.contains('/') || name.len() > 255 {
        return Err(ungueltig("zustand.name"));
    }
    let u = &b.umgebung;
    if Path::new(&u.home) != b.heim.as_path() {
        return Err(ungueltig("umgebung.home"));
    }
    if ![&u.path, &u.term, &u.lang].into_iter().all(|w| text_ok(w)) {
        return Err(ungueltig("umgebung"));
    }
    if !normal(Path::new(&u.tmpdir)) {
        return Err(ungueltig("umgebung.tmpdir"));
    }
    let system_ok = |a: &str, z: &str| !a.contains('\0') && !z.contains('\0') && z.starts_with('/');
    if b.system.symlinks.iter().any(|(t, l)| !system_ok(t, l))
        || b.system.ro_binds.iter().any(|(q, z)| !system_ok(q, z))
    {
        return Err(ungueltig("system"));
    }
    Ok(())
}

/// The checks against the deny lists and the overlaps. The paths are normal at this point.
fn verbote_pruefen(b: &Beschreibung) -> Result<(), Fehler> {
    if arbeitsordner_verboten(&b.arbeitsordner, &b.heim) {
        return Err(Fehler::Verboten(
            "working folder is a system path, the home or above it",
        ));
    }
    if b.heim == Path::new("/tmp")
        || SYSTEM_UNTER_VERBOTEN
            .iter()
            .any(|p| innerhalb(&b.heim, Path::new(p)))
    {
        return Err(Fehler::Verboten("home is a system path"));
    }
    let zustand_ziel = gast_pfad(&b.heim, &b.zustand.name);
    let mut quellen: Vec<&Path> = vec![&b.arbeitsordner, &b.zustand.host, &b.such_socket];
    quellen.extend(b.programmordner.iter().map(PathBuf::as_path));
    quellen.extend(b.modell_socket.as_deref());
    quellen.extend(b.lokal_weiter.as_deref());
    quellen.extend(b.dateien.iter().map(|d| d.quelle.as_path()));
    if let Some(c) = &b.cloud {
        quellen.extend([&c.weiterreicher, &c.weiter_programm, &c.bundle].map(PathBuf::as_path));
    }
    quellen.extend(b.paket.iter().map(|p| p.quelle.as_path()));
    let paket_ziel = b
        .paket
        .as_ref()
        .map(|p| crate::beschreibung::paket_ziel(&b.heim, &p.plattform, &p.fassung));
    for pfad in quellen
        .iter()
        .copied()
        .chain([zustand_ziel.as_path()])
        .chain(paket_ziel.as_deref())
    {
        if hat_geheimnisteil(pfad) {
            return Err(Fehler::Verboten("path with a secret segment"));
        }
        if beruehrt_anmeldespeicher(pfad, &b.heim, &b.harness) {
            return Err(Fehler::Verboten(
                "path in or over a login store of the account",
            ));
        }
    }
    // folders: working folder, program folders, state folder. They must not meet a protected folder or each other
    // on the host, and the state folder must not meet them inside.
    let ordner: Vec<&Path> = std::iter::once(b.arbeitsordner.as_path())
        .chain(b.programmordner.iter().map(PathBuf::as_path))
        .chain(std::iter::once(b.zustand.host.as_path()))
        .collect();
    for schutz in &b.geschuetzt {
        if ordner.iter().any(|o| ueberlappt(o, schutz)) {
            return Err(Fehler::Ueberlappt("mount overlaps a protected folder"));
        }
    }
    if ueberlappt(&b.arbeitsordner, &b.zustand.host)
        || b.programmordner
            .iter()
            .any(|p| ueberlappt(p, &b.zustand.host))
    {
        return Err(Fehler::Ueberlappt("state folder overlaps another mount"));
    }
    if ueberlappt(&zustand_ziel, &b.arbeitsordner)
        || b.programmordner
            .iter()
            .any(|p| ueberlappt(p, &zustand_ziel))
    {
        return Err(Fehler::Ueberlappt(
            "state folder place overlaps another mount",
        ));
    }
    let sockel: Vec<&Path> = std::iter::once(b.such_socket.as_path())
        .chain(b.modell_socket.as_deref())
        .chain(b.cloud.iter().map(|c| c.weiterreicher.as_path()))
        .collect();
    if sockel
        .iter()
        .any(|s| ordner.iter().any(|o| innerhalb(s, o)))
    {
        return Err(Fehler::Ueberlappt("socket lies inside a mounted folder"));
    }
    if b.modell_socket.as_deref() == Some(b.such_socket.as_path())
        || b.cloud
            .as_ref()
            .is_some_and(|c| c.weiterreicher == b.such_socket)
    {
        return Err(Fehler::Ueberlappt("both sockets are the same file"));
    }
    if let Some(c) = &b.cloud {
        cloud_pruefen(b, c)?;
    }
    if let (Some(p), Some(ziel)) = (&b.paket, &paket_ziel) {
        paket_lage_pruefen(b, &p.quelle, ziel)?;
    }
    schreibbar_pruefen(b)?;
    Ok(())
}

/// The package of the cloud harness is bound read-only; its folder on the host must lie apart from every folder the
/// harness can write (working folder, state folder), from the program folders and from the protected folders, and its
/// place inside must meet neither the working folder nor the place of the state folder.
fn paket_lage_pruefen(b: &Beschreibung, quelle: &Path, ziel: &Path) -> Result<(), Fehler> {
    let neben = [b.arbeitsordner.as_path(), b.zustand.host.as_path()]
        .into_iter()
        .chain(b.programmordner.iter().map(PathBuf::as_path))
        .chain(b.geschuetzt.iter().map(PathBuf::as_path));
    for o in neben {
        if ueberlappt(quelle, o) {
            return Err(Fehler::Ueberlappt(
                "the package folder overlaps another folder",
            ));
        }
    }
    let zustand_ziel = gast_pfad(&b.heim, &b.zustand.name);
    // the configuration files have fixed places below `.pi` and `.copilot` (checked above), never below `.cache`
    if ueberlappt(ziel, &b.arbeitsordner) || ueberlappt(ziel, &zustand_ziel) {
        return Err(Fehler::Ueberlappt(
            "the package place overlaps another mount",
        ));
    }
    Ok(())
}

/// The read-only files (the forwarder of the local route, the configuration files) must not lie in a folder the
/// harness can write: it could change them there on the host.
fn schreibbar_pruefen(b: &Beschreibung) -> Result<(), Fehler> {
    let schreibbar = [b.arbeitsordner.as_path(), b.zustand.host.as_path()];
    let dateien = b
        .lokal_weiter
        .iter()
        .chain(b.dateien.iter().map(|d| &d.quelle));
    for datei in dateien {
        if schreibbar.iter().any(|o| ueberlappt(datei, o)) {
            return Err(Fehler::Ueberlappt(
                "a read-only file lies in a folder the harness can write",
            ));
        }
    }
    Ok(())
}

/// The files of the cloud exit are bound read-only, so the agent cannot change them inside; but a file that lies in a
/// folder the agent can write (working folder, state folder) could be changed through that folder on the host. The
/// files must lie outside both and must not be the folders themselves.
fn cloud_pruefen(b: &Beschreibung, c: &Cloud) -> Result<(), Fehler> {
    let schreibbar = [b.arbeitsordner.as_path(), b.zustand.host.as_path()];
    for datei in [&c.weiter_programm, &c.bundle] {
        if schreibbar.iter().any(|o| ueberlappt(datei, o)) {
            return Err(Fehler::Ueberlappt(
                "a file of the cloud exit lies in a folder the harness can write",
            ));
        }
    }
    if c.weiter_programm == c.bundle {
        return Err(Fehler::Ueberlappt(
            "the program and the bundle of the cloud exit are the same file",
        ));
    }
    Ok(())
}

pub(crate) fn zeichen(zeile: &mut Vec<String>, teile: &[&str]) {
    zeile.extend(teile.iter().map(|t| (*t).to_owned()));
}

pub(crate) fn text(pfad: &Path) -> String {
    pfad.to_string_lossy().into_owned()
}

/// The argument list up to the `--`.
fn kopf_bauen(b: &Beschreibung) -> Vec<String> {
    let mut k: Vec<String> = Vec::new();
    zeichen(
        &mut k,
        &[
            "--unshare-user",
            "--disable-userns",
            "--unshare-pid",
            "--unshare-net",
            "--unshare-ipc",
            "--unshare-uts",
            "--unshare-cgroup",
            "--die-with-parent",
            "--cap-drop",
            "ALL",
            "--clearenv",
            "--hostname",
            "wb-einschluss",
            "--ro-bind",
            "/usr",
            "/usr",
        ],
    );
    for (link_text, link) in &b.system.symlinks {
        zeichen(&mut k, &["--symlink", link_text, link]);
    }
    for (quelle, ziel) in &b.system.ro_binds {
        zeichen(&mut k, &["--ro-bind", quelle, ziel]);
    }
    zeichen(&mut k, &["--proc", "/proc", "--dev", "/dev"]);
    let tmp = TMP_BYTES.to_string();
    let klein = KLEIN_BYTES.to_string();
    zeichen(
        &mut k,
        &["--size", &tmp, "--perms", "0700", "--tmpfs", "/tmp"],
    );
    zeichen(
        &mut k,
        &["--size", &klein, "--perms", "0700", "--tmpfs", "/run"],
    );
    zeichen(&mut k, &["--perms", "0700", "--dir", "/run/user"]);
    let nutzer = format!("/run/user/{}", b.system.uid);
    zeichen(&mut k, &["--perms", "0700", "--dir", &nutzer]);
    let heim = text(&b.heim);
    zeichen(
        &mut k,
        &["--size", &klein, "--perms", "0700", "--tmpfs", &heim],
    );
    for ordner in &b.programmordner {
        let o = text(ordner);
        zeichen(&mut k, &["--ro-bind", &o, &o]);
    }
    let zustand_ziel = text(&gast_pfad(&b.heim, &b.zustand.name));
    zeichen(&mut k, &["--bind", &text(&b.zustand.host), &zustand_ziel]);
    let arbeit = text(&b.arbeitsordner);
    zeichen(&mut k, &["--bind", &arbeit, &arbeit]);
    zeichen(
        &mut k,
        &["--ro-bind", &text(&b.such_socket), GAST_SUCH_SOCKET],
    );
    if let Some(modell) = &b.modell_socket {
        zeichen(&mut k, &["--ro-bind", &text(modell), GAST_MODELL_SOCKET]);
    }
    if let Some(weiter) = &b.lokal_weiter {
        zeichen(&mut k, &["--ro-bind", &text(weiter), GAST_WEITER]);
    }
    for d in &b.dateien {
        zeichen(&mut k, &["--ro-bind", &text(&d.quelle), &text(&d.ziel)]);
    }
    if let Some(p) = &b.paket {
        let ziel = crate::beschreibung::paket_ziel(&b.heim, &p.plattform, &p.fassung);
        zeichen(&mut k, &["--ro-bind", &text(&p.quelle), &text(&ziel)]);
    }
    if let Some(c) = &b.cloud {
        zeichen(
            &mut k,
            &["--ro-bind", &text(&c.weiterreicher), GAST_MODELL_SOCKET],
        );
        zeichen(
            &mut k,
            &["--ro-bind", &text(&c.weiter_programm), GAST_WEITER],
        );
        for ort in c.verteilung.bundle_orte() {
            zeichen(&mut k, &["--ro-bind", &text(&c.bundle), ort]);
        }
    }
    umgebung_setzen(&mut k, b);
    zeichen(&mut k, &["--chdir", &arbeit]);
    k
}

/// The `--setenv` part: the five base variables, the two of `pi` and the four of the cloud harness.
fn umgebung_setzen(k: &mut Vec<String>, b: &Beschreibung) {
    let u = &b.umgebung;
    for (name, wert) in [
        ("HOME", &u.home),
        ("PATH", &u.path),
        ("TERM", &u.term),
        ("LANG", &u.lang),
        ("TMPDIR", &u.tmpdir),
    ] {
        zeichen(k, &["--setenv", name, wert]);
    }
    if b.harness == HARNESS_PI {
        for (name, wert) in PI_UMGEBUNG {
            zeichen(k, &["--setenv", name, wert]);
        }
    }
    if b.cloud.is_some() {
        let proxy = proxy_wert();
        for (name, wert) in [
            ("COPILOT_GITHUB_TOKEN", COPILOT_PLATZHALTER),
            ("HTTPS_PROXY", proxy.as_str()),
            ("COPILOT_AUTO_UPDATE", "false"),
            ("COPILOT_MASCOT_BLINK", "0"),
        ] {
            zeichen(k, &["--setenv", name, wert]);
        }
    }
}

/// The environment as text: every `--setenv` names a variable of the allow list, and the cloud variables carry their
/// fixed values (the placeholder, never a token). The bus, the runtime folder and the certificate paths of the host
/// are not on the list, so none of them can be set.
pub(crate) fn umgebung_pruefen(kopf: &[String], harness: &str) -> Result<(), Fehler> {
    let proxy = proxy_wert();
    for (i, option) in kopf.iter().enumerate() {
        if option != "--setenv" {
            continue;
        }
        let (Some(name), Some(wert)) = (kopf.get(i + 1), kopf.get(i + 2)) else {
            return Err(Fehler::Verboten("variable without a value"));
        };
        if !ERLAUBTE_VARIABLEN.contains(&name.as_str()) {
            return Err(Fehler::Verboten("variable outside the allow list"));
        }
        let pi = PI_UMGEBUNG.iter().find(|(n, _)| n == name);
        if pi.is_some() && harness != HARNESS_PI {
            return Err(Fehler::Verboten("variable of another harness"));
        }
        let soll = match name.as_str() {
            _ if pi.is_some() => pi.map(|(_, w)| *w),
            "COPILOT_GITHUB_TOKEN" => Some(COPILOT_PLATZHALTER),
            "HTTPS_PROXY" => Some(proxy.as_str()),
            "COPILOT_AUTO_UPDATE" => Some("false"),
            "COPILOT_MASCOT_BLINK" => Some("0"),
            _ => None,
        };
        if soll.is_some_and(|s| s != wert) {
            return Err(Fehler::Verboten("cloud variable with a value of its own"));
        }
    }
    Ok(())
}

/// The check of the finished text, the successor of `tuer_grenze_pruefen` of the old door: required switches, none of
/// the forbidden ones, private `/run` with an empty `/run/user/<uid>`, an empty home before every mount below it, no
/// mount of `/`, of the home, of anything above it or of `/run/user`, and no secret or login store anywhere.
pub(crate) fn kopf_pruefen(
    kopf: &[String],
    heim: &Path,
    harness: &str,
    uid: u32,
) -> Result<(), Fehler> {
    let fehlt = |was: &'static str| Fehler::Verboten(was);
    let hat = |s: &str| kopf.iter().any(|k| k == s);
    let stelle =
        |option: &str, ziel: &str| kopf.windows(2).position(|w| w[0] == option && w[1] == ziel);
    if PFLICHT.iter().any(|s| !hat(s)) || stelle("--cap-drop", "ALL").is_none() {
        return Err(fehlt("profile lacks a required switch"));
    }
    if VERBOTEN.iter().any(|s| hat(s)) {
        return Err(fehlt("profile carries a forbidden switch"));
    }
    let nutzer = format!("/run/user/{uid}");
    if stelle("--tmpfs", &text(heim)).is_none() {
        return Err(fehlt("home is not a private tmpfs"));
    }
    if stelle("--tmpfs", "/run").is_none() || stelle("--dir", &nutzer).is_none() {
        return Err(fehlt("private /run with an empty /run/user/<uid> missing"));
    }
    // one pass in the order of the list: what comes before the clearing or before the home tmpfs is the fault
    let (mut geleert, mut heim_leer) = (false, false);
    for (i, option) in kopf.iter().enumerate() {
        match option.as_str() {
            "--clearenv" => geleert = true,
            "--setenv" if !geleert => return Err(fehlt("environment set before it is cleared")),
            "--tmpfs" if kopf.get(i + 1).is_some_and(|z| Path::new(z) == heim) => heim_leer = true,
            _ => {}
        }
        if option == "--symlink" || !EINBINDUNGEN.contains(&option.as_str()) {
            continue;
        }
        let (Some(quelle), Some(ziel)) = (kopf.get(i + 1), kopf.get(i + 2)) else {
            return Err(fehlt("mount without a place"));
        };
        let (quelle, ziel) = (Path::new(quelle), Path::new(ziel));
        if quelle == Path::new("/") || ziel == Path::new("/") {
            return Err(fehlt("mount of the root"));
        }
        // `/dev/null` as a source hides a file. One pair is allowed: the command of the human, read-only, at its place
        // in the package. Every other bind of `/dev/null` is refused.
        if quelle == Path::new("/dev/null")
            && !(option == "--ro-bind" && ziel == Path::new(ORDNERMARKE_PFAD))
        {
            return Err(fehlt(
                "bind of /dev/null other than over the command of the human",
            ));
        }
        if ziel == Path::new("/run/user")
            || innerhalb(ziel, Path::new("/run/user"))
            || innerhalb(heim, ziel)
        {
            return Err(fehlt("mount breaks the private run or home"));
        }
        if innerhalb(ziel, heim) && !heim_leer {
            return Err(fehlt("mount in the home before its tmpfs"));
        }
        for pfad in [quelle, ziel] {
            if hat_geheimnisteil(pfad) || beruehrt_anmeldespeicher(pfad, heim, harness) {
                return Err(Fehler::Verboten("secret or login store in a mount"));
            }
        }
    }
    Ok(())
}

impl Profil {
    /// Builds the profile from the description: checks the form, the deny lists and the overlaps, builds the argument
    /// list and checks it once more as text. It reads no file and starts nothing; [`Profil::vorpruefen`] does that.
    ///
    /// # Errors
    /// [`Fehler::BeschreibungUngueltig`], [`Fehler::Verboten`] or [`Fehler::Ueberlappt`].
    pub fn bauen(beschreibung: &Beschreibung) -> Result<Profil, Fehler> {
        felder_pruefen(beschreibung)?;
        verbote_pruefen(beschreibung)?;
        let kopf = kopf_bauen(beschreibung);
        kopf_pruefen(
            &kopf,
            &beschreibung.heim,
            &beschreibung.harness,
            beschreibung.system.uid,
        )?;
        umgebung_pruefen(&kopf, &beschreibung.harness)?;
        Ok(Profil {
            bwrap: PathBuf::from(BWRAP_PFAD),
            kopf,
            beschreibung: beschreibung.clone(),
        })
    }

    /// Takes `bwrap` from another absolute path. Only for a path the operator set in the configuration of the system
    /// (the start never takes it from a file of the session, from the environment or from the agent).
    #[must_use]
    pub fn mit_bwrap(mut self, pfad: PathBuf) -> Profil {
        self.bwrap = pfad;
        self
    }

    /// The path of `bwrap`.
    #[must_use]
    pub fn bwrap(&self) -> &Path {
        &self.bwrap
    }

    /// The description the profile was built from.
    #[must_use]
    pub fn beschreibung(&self) -> &Beschreibung {
        &self.beschreibung
    }

    /// The arguments of `bwrap`, without the program path: the switches, then `--`, then the program and its
    /// arguments of the description.
    ///
    /// `--die-with-parent` is in the list. It ends the session when the process that started `bwrap` dies, also by
    /// `SIGKILL`; without it the harness stays behind as an orphan (measured on the case `elternbindung`, with the
    /// counter probe). One doubt is measured away: the kernel ties the signal of a plain `PR_SET_PDEATHSIG` to the
    /// *thread* that started the child, and a service that starts the session from a pool thread would lose it when
    /// that thread ends. `bwrap` 0.12.0 does not behave like that: it survives the end of the starting thread (measured
    /// with a Rust and a Python thread on the kernel 7.2.5). The same case holds that measurement, so a `bwrap` that
    /// changes it turns the case red.
    ///
    #[must_use]
    pub fn argumente(&self) -> Vec<String> {
        self.argumente_mit(&self.beschreibung.argv)
    }

    /// The list for another program in the same enclosure. With a cloud exit the program runs below the forwarder:
    /// `<forwarder> <port> <relay socket> -- <program> ...`.
    pub(crate) fn argumente_mit(&self, argv: &[String]) -> Vec<String> {
        let mut alle = self.kopf.clone();
        alle.push("--".to_owned());
        let port = if self.beschreibung.cloud.is_some() {
            Some(WEITER_PORT)
        } else if self.beschreibung.lokal_weiter.is_some() {
            Some(LOKAL_PORT)
        } else {
            None
        };
        if let Some(port) = port {
            alle.extend([
                GAST_WEITER.to_owned(),
                port.to_string(),
                GAST_MODELL_SOCKET.to_owned(),
                "--".to_owned(),
            ]);
        }
        alle.extend(argv.iter().cloned());
        alle
    }

    /// The command to start. It has an empty environment: `bwrap` needs nothing, and the harness gets its five
    /// variables from the profile. The caller sets the streams; the service later replaces its process by this command
    /// (`exec`), so the terminal stays the terminal of the harness.
    #[must_use]
    pub fn befehl(&self) -> Command {
        let mut c = Command::new(&self.bwrap);
        c.args(self.argumente()).env_clear();
        c
    }
}

#[cfg(test)]
mod tests;
