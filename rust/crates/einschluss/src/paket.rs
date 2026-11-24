//! The package of the cloud harness (contract 1, "Paket der Copilot CLI"): the check of the extracted folder on the host
//! and the small enclosure that extracts it.
//!
//! The CLI of the cloud harness extracts its bundled package when it starts, before any screen, into
//! `<home>/.cache/copilot/pkg/<platform>/<version>`. Inside the enclosure the home is a tmpfs of 16 MiB, and the package
//! has 166 MB: the start fails with `ENOSPC` (measured with 1.0.88). The program "Lauf starten" therefore extracts it
//! once in a small enclosure of its own ([`Vorentpacken`]) into a cache folder of the user outside, checks the result on
//! the host ([`paket_pruefen`]) and binds the folder read-only at the place where the CLI looks
//! ([`crate::Paket`], [`crate::paket_ziel`]).
//!
//! The small enclosure is the profile of the session without working folder, state folder, sockets and cloud exit:
//! the same required switches (own user, process, network, IPC, UTS and cgroup space, no further user namespace,
//! `--cap-drop ALL`, cleared environment), `/usr` and the system part read-only, the program folders read-only, a
//! private `/tmp`, `/run` and home of 16 MiB each, and one writable bind: the cache folder at
//! `<home>/.cache/copilot`. The environment is `HOME`, `PATH` and `COPILOT_AUTO_UPDATE=false`; the program runs with
//! the one argument `--version` (measured: `--version` extracts, `--binary-version` does not). No network, no token.

use std::path::{Path, PathBuf};

#[cfg(any(unix, test))]
use crate::beschreibung::paket_pfade;
use crate::beschreibung::{HARNESS_COPILOT, PAKET_GAST_CACHE, System, fassung_gut};
use crate::eingabe::{EINGABE_MAX, Felder};
use crate::fehler::Fehler;
use crate::json::{lesen, text_schreiben};
use crate::pfad::{gast_pfad, innerhalb, normal, ueberlappt};
use crate::profil::{KLEIN_BYTES, kopf_pruefen, text, umgebung_pruefen, zeichen};
use crate::verbote::{beruehrt_anmeldespeicher, hat_geheimnisteil};

/// The cache folder of the harness always ends with these segments: no wider folder of the home is ever bound writable.
pub const CACHE_ENDE: &str = "werkbank/harness/copilot";

/// The one argument of the program in the small enclosure.
pub const VORENTPACKEN_ARGUMENT: &str = "--version";

/// Why a folder of the package cannot be bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaketMangel {
    /// The folder does not exist.
    Fehlt,
    /// The folder, a folder above it or one of the four paths is a link (the real path on the host differs).
    Verweis,
    /// The folder or one of the four paths belongs to another user.
    Fremd,
    /// One of the four paths is missing or is not a regular file, or the place holds no folder.
    Unvollstaendig,
}

/// Checks the folder of the package on the host, before it is bound: it exists, its real path is the path itself (no
/// link anywhere, also not above it: `bwrap` resolves links, so inside the check could not see them), it belongs to
/// `uid`, and each of the four paths of [`paket_pfade`] is a regular file of `uid` with a real path equal to its path.
///
/// # Errors
/// The first [`PaketMangel`] found, in the order: missing, link, foreign, incomplete.
#[cfg(unix)]
pub fn paket_pruefen(ordner: &Path, plattform: &str, uid: u32) -> Result<(), PaketMangel> {
    use std::os::unix::fs::MetadataExt;
    let meta = match std::fs::symlink_metadata(ordner) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(PaketMangel::Fehlt),
        Err(_) => return Err(PaketMangel::Unvollstaendig),
    };
    let echt_gleich = |p: &Path| std::fs::canonicalize(p).is_ok_and(|e| e == p);
    // a link has a real path other than its own, also a link above the folder
    if !echt_gleich(ordner) {
        return Err(PaketMangel::Verweis);
    }
    if meta.uid() != uid {
        return Err(PaketMangel::Fremd);
    }
    if !meta.is_dir() {
        return Err(PaketMangel::Unvollstaendig);
    }
    for rel in paket_pfade(plattform) {
        let pfad = ordner.join(&rel);
        let Ok(m) = std::fs::symlink_metadata(&pfad) else {
            return Err(PaketMangel::Unvollstaendig);
        };
        if !echt_gleich(&pfad) {
            return Err(PaketMangel::Verweis);
        }
        if m.uid() != uid {
            return Err(PaketMangel::Fremd);
        }
        if !m.is_file() {
            return Err(PaketMangel::Unvollstaendig);
        }
    }
    Ok(())
}

/// Not available: the enclosure exists on Linux only.
///
/// # Errors
/// Always [`PaketMangel::Unvollstaendig`].
#[cfg(not(unix))]
pub fn paket_pruefen(_ordner: &Path, _plattform: &str, _uid: u32) -> Result<(), PaketMangel> {
    Err(PaketMangel::Unvollstaendig)
}

/// The small enclosure that extracts the package of the cloud harness into the cache folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vorentpacken {
    /// The home of the user on the host; a private, empty tmpfs inside.
    pub heim: PathBuf,
    /// The cache folder of the harness on the host, inside the home and ending with [`CACHE_ENDE`]: an own folder
    /// without links. It is the one writable bind, at `<heim>/.cache/copilot`.
    pub cache: PathBuf,
    /// The program folders of the harness: read-only, at the same path inside.
    pub programmordner: Vec<PathBuf>,
    /// The program of the harness (`harness.copilot`), started as a file with [`VORENTPACKEN_ARGUMENT`].
    pub programm: PathBuf,
    /// `PATH` inside.
    pub path: String,
    /// The host part of the system layout.
    pub system: System,
}

impl Vorentpacken {
    /// The place of the cache folder inside: `<heim>/.cache/copilot`.
    #[must_use]
    pub fn gast_cache(&self) -> PathBuf {
        gast_pfad(&self.heim, PAKET_GAST_CACHE)
    }

    /// The checks of form and the deny lists: every path normal, the cache folder strictly inside the home, no secret
    /// segment, no login store, the cache folder apart from the program folders.
    fn pruefen(&self) -> Result<(), Fehler> {
        let ungueltig = Fehler::BeschreibungUngueltig;
        if !normal(&self.heim) || !normal(&self.cache) || !normal(&self.programm) {
            return Err(ungueltig("vorentpacken"));
        }
        if self.programmordner.iter().any(|p| !normal(p)) {
            return Err(ungueltig("vorentpacken.programmordner"));
        }
        if self.path.is_empty() || self.path.chars().any(char::is_control) {
            return Err(ungueltig("vorentpacken.path"));
        }
        if !innerhalb(&self.cache, &self.heim) || !self.cache.ends_with(CACHE_ENDE) {
            return Err(Fehler::Verboten(
                "cache folder is not <base>/werkbank/harness/copilot inside the home",
            ));
        }
        let gast = self.gast_cache();
        for pfad in std::iter::once(self.cache.as_path())
            .chain(std::iter::once(gast.as_path()))
            .chain(self.programmordner.iter().map(PathBuf::as_path))
        {
            if hat_geheimnisteil(pfad) {
                return Err(Fehler::Verboten("path with a secret segment"));
            }
            if beruehrt_anmeldespeicher(pfad, &self.heim, HARNESS_COPILOT) {
                return Err(Fehler::Verboten(
                    "path in or over a login store of the account",
                ));
            }
        }
        if self
            .programmordner
            .iter()
            .any(|p| ueberlappt(p, &self.cache) || ueberlappt(p, &gast))
        {
            return Err(Fehler::Ueberlappt("cache folder overlaps a program folder"));
        }
        Ok(())
    }

    /// The arguments of `bwrap` (without the program path), checked as text like the profile of a session.
    ///
    /// # Errors
    /// [`Fehler::BeschreibungUngueltig`], [`Fehler::Verboten`] or [`Fehler::Ueberlappt`].
    pub fn argumente(&self) -> Result<Vec<String>, Fehler> {
        self.pruefen()?;
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
        for (link_text, link) in &self.system.symlinks {
            zeichen(&mut k, &["--symlink", link_text, link]);
        }
        for (quelle, ziel) in &self.system.ro_binds {
            zeichen(&mut k, &["--ro-bind", quelle, ziel]);
        }
        zeichen(&mut k, &["--proc", "/proc", "--dev", "/dev"]);
        let klein = KLEIN_BYTES.to_string();
        for ort in ["/tmp", "/run"] {
            zeichen(
                &mut k,
                &["--size", &klein, "--perms", "0700", "--tmpfs", ort],
            );
        }
        zeichen(&mut k, &["--perms", "0700", "--dir", "/run/user"]);
        let nutzer = format!("/run/user/{}", self.system.uid);
        zeichen(&mut k, &["--perms", "0700", "--dir", &nutzer]);
        let heim = text(&self.heim);
        zeichen(
            &mut k,
            &["--size", &klein, "--perms", "0700", "--tmpfs", &heim],
        );
        for ordner in &self.programmordner {
            let o = text(ordner);
            zeichen(&mut k, &["--ro-bind", &o, &o]);
        }
        zeichen(
            &mut k,
            &["--bind", &text(&self.cache), &text(&self.gast_cache())],
        );
        for (name, wert) in [
            ("HOME", heim.as_str()),
            ("PATH", self.path.as_str()),
            ("COPILOT_AUTO_UPDATE", "false"),
        ] {
            zeichen(&mut k, &["--setenv", name, wert]);
        }
        zeichen(&mut k, &["--chdir", &heim]);
        kopf_pruefen(&k, &self.heim, HARNESS_COPILOT, self.system.uid)?;
        umgebung_pruefen(&k, HARNESS_COPILOT)?;
        // the cache folder is the only writable bind
        let schreibbar = k.iter().filter(|a| a.as_str() == "--bind").count();
        if schreibbar != 1 {
            return Err(Fehler::Verboten(
                "small enclosure with another writable bind",
            ));
        }
        k.push("--".to_owned());
        k.push(text(&self.programm));
        k.push(VORENTPACKEN_ARGUMENT.to_owned());
        Ok(k)
    }
}

/// The text form of the small enclosure, the input of the call `vorentpacken` of the starter: a closed JSON object
/// `{"heim": str, "cache": str, "programmordner": [str], "programm": str, "path": str}`. There is no field `system`
/// and no field for the arguments: the starter reads the host and fixes the one argument itself.
#[must_use]
pub fn vorentpacken_schreiben(v: &Vorentpacken) -> String {
    let mut s = String::from("{\"heim\":");
    text_schreiben(&mut s, &text(&v.heim));
    s.push_str(",\"cache\":");
    text_schreiben(&mut s, &text(&v.cache));
    s.push_str(",\"programmordner\":[");
    for (i, o) in v.programmordner.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        text_schreiben(&mut s, &text(o));
    }
    s.push_str("],\"programm\":");
    text_schreiben(&mut s, &text(&v.programm));
    s.push_str(",\"path\":");
    text_schreiben(&mut s, &v.path);
    s.push('}');
    s
}

/// Reads the text form of [`vorentpacken_schreiben`]. `None` for every refusal: no JSON, an unknown key, a missing
/// field, a wrong type, more than [`EINGABE_MAX`] bytes.
#[must_use]
pub fn vorentpacken_lesen(bytes: &[u8], system: System) -> Option<Vorentpacken> {
    if bytes.len() > EINGABE_MAX {
        return None;
    }
    let mut f = Felder::aus(lesen(bytes)?)?;
    let v = Vorentpacken {
        heim: f.pfad("heim")?,
        cache: f.pfad("cache")?,
        programmordner: f.pfade("programmordner")?,
        programm: f.pfad("programm")?,
        path: f.text("path")?,
        system,
    };
    f.fertig()?;
    Some(v)
}

/// The form of a version and of a platform of the package (for the checks of the description).
pub(crate) fn paket_form_gut(plattform: &str, fassung: &str) -> bool {
    crate::beschreibung::PAKET_PLATTFORMEN.contains(&plattform) && fassung_gut(fassung)
}

#[cfg(test)]
mod tests;
