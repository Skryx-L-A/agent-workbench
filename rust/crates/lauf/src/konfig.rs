//! The own configuration of "Lauf starten" (contract 1: the paths of the settings file, the vault, the log, the
//! sockets and the state never come from the start file, the environment or the agent).
//!
//! The delivered build reads [`KONFIG_PFAD`]. The test build (feature `pruefbau`) reads `lauf.json` in the folder of
//! the program instead, so a test needs neither root nor an environment variable. The file is a closed JSON object:
//!
//! ```text
//! { "fassung": 1,
//!   "einstellungen": str, "vault": str, "protokoll": str,
//!   "zustand": str,            the folder of ordnermarken.json (0700)
//!   "laufzeit": str,           the folder of the socket folders, short (sun_path)
//!   "sitzungen": str,          the folder of the state folders and configuration files of the sessions
//!   "heim": str,               the home of the user on the host (an empty tmpfs inside)
//!   "path": str,               PATH of the harness
//!   "programme": {"rangkern": str, "modellweg": str, "kind": str, "such_mcp": str, "weiter": str},
//!   "harness": {"<harness>": str, ...},   program of each harness of the table, a subset of claude, copilot, pi
//!   "programmordner": [str],   folders bound read-only (the programs of the harness)
//!   "schluessel": {"<route>": str, ...},  key file of the route for the model path
//!   "ca_zulassung": "zugelassen" | "offen" | "abgelehnt",
//!   "bwrap": str }             only in the test build: the bwrap of the direct way
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use einstellungen::json::{Wert, lesen};

use crate::fehler::Fehler;

/// The configuration file of the delivered build.
pub const KONFIG_PFAD: &str = "/etc/werkbank/lauf.json";

/// The largest configuration file in bytes.
const KONFIG_MAX: u64 = 65_536;

/// The programs of the session that run outside the enclosure, and the two that run inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Programme {
    /// `rangkern` (the search process, contract 4).
    pub rangkern: PathBuf,
    /// The model path (contract 8).
    pub modellweg: PathBuf,
    /// The trampoline `werkbank-lauf-kind`.
    pub kind: PathBuf,
    /// `such-mcp`, the MCP tool inside the enclosure (copilot).
    pub such_mcp: PathBuf,
    /// `einschluss-weiter`, the forwarder inside the enclosure.
    pub weiter: PathBuf,
}

/// The own configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Konfig {
    /// The settings file (contract 2).
    pub einstellungen: PathBuf,
    /// The vault of the search process.
    pub vault: PathBuf,
    /// The log folder.
    pub protokoll: PathBuf,
    /// The folder of `ordnermarken.json` and `ordnermarken.lock`.
    pub zustand: PathBuf,
    /// The folder of the socket folders of the sessions.
    pub laufzeit: PathBuf,
    /// The folder of the session folders (state folder and configuration files of the harness).
    pub sitzungen: PathBuf,
    /// The home of the user on the host.
    pub heim: PathBuf,
    /// `PATH` of the harness.
    pub path: String,
    /// The programs.
    pub programme: Programme,
    /// The program of each harness.
    pub harness: BTreeMap<String, PathBuf>,
    /// Read-only program folders.
    pub programmordner: Vec<PathBuf>,
    /// The key file of each route.
    pub schluessel: BTreeMap<String, PathBuf>,
    /// The answer of the firm on the local CA (contract 8, section 12).
    pub ca_zulassung: String,
    /// Test build only: the bwrap of the direct way.
    pub bwrap: Option<PathBuf>,
}

struct Felder(Vec<(String, Wert)>);

impl Felder {
    fn aus(wert: Wert) -> Option<Felder> {
        match wert {
            Wert::Objekt(f) => Some(Felder(f)),
            _ => None,
        }
    }

    fn nehmen(&mut self, name: &str) -> Option<Wert> {
        let i = self.0.iter().position(|(n, _)| n == name)?;
        Some(self.0.remove(i).1)
    }

    fn text(&mut self, name: &str) -> Option<String> {
        match self.nehmen(name)? {
            Wert::Text(t) => Some(t),
            _ => None,
        }
    }

    fn pfad(&mut self, name: &str) -> Option<PathBuf> {
        let t = self.text(name)?;
        crate::ordner_form(&t).then(|| PathBuf::from(t))
    }

    fn karte(&mut self, name: &str) -> Option<BTreeMap<String, PathBuf>> {
        let Wert::Objekt(f) = self.nehmen(name)? else {
            return None;
        };
        f.into_iter()
            .map(|(k, w)| match w {
                Wert::Text(t) if crate::ordner_form(&t) => Some((k, PathBuf::from(t))),
                _ => None,
            })
            .collect()
    }

    fn fertig(self) -> Option<()> {
        self.0.is_empty().then_some(())
    }
}

/// Reads the text of the configuration. `None` for every fault: no JSON, an unknown or missing key, a wrong type, a
/// path that is not absolute and normal, a harness outside the table, a route that is no key.
#[must_use]
pub fn lesen_text(bytes: &[u8]) -> Option<Konfig> {
    let mut f = Felder::aus(lesen(bytes).ok()?)?;
    if f.nehmen("fassung")? != Wert::Ganzzahl("1".to_owned()) {
        return None;
    }
    let mut p = Felder::aus(f.nehmen("programme")?)?;
    let programme = Programme {
        rangkern: p.pfad("rangkern")?,
        modellweg: p.pfad("modellweg")?,
        kind: p.pfad("kind")?,
        such_mcp: p.pfad("such_mcp")?,
        weiter: p.pfad("weiter")?,
    };
    p.fertig()?;
    let harness = f.karte("harness")?;
    if harness
        .keys()
        .any(|h| !matches!(h.as_str(), "claude" | "copilot" | "pi"))
    {
        return None;
    }
    let schluessel = f.karte("schluessel")?;
    if !schluessel.keys().all(|r| einstellungen::ist_kennung(r)) {
        return None;
    }
    let programmordner = match f.nehmen("programmordner")? {
        Wert::Liste(l) => l
            .into_iter()
            .map(|w| match w {
                Wert::Text(t) if crate::ordner_form(&t) => Some(PathBuf::from(t)),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?,
        _ => return None,
    };
    let ca_zulassung = f.text("ca_zulassung")?;
    if !matches!(ca_zulassung.as_str(), "zugelassen" | "offen" | "abgelehnt") {
        return None;
    }
    let path = f.text("path")?;
    if path.is_empty() || path.chars().any(char::is_control) {
        return None;
    }
    let bwrap = if cfg!(feature = "pruefbau") && f.0.iter().any(|(n, _)| n == "bwrap") {
        Some(f.pfad("bwrap")?)
    } else {
        None
    };
    let k = Konfig {
        einstellungen: f.pfad("einstellungen")?,
        vault: f.pfad("vault")?,
        protokoll: f.pfad("protokoll")?,
        zustand: f.pfad("zustand")?,
        laufzeit: f.pfad("laufzeit")?,
        sitzungen: f.pfad("sitzungen")?,
        heim: f.pfad("heim")?,
        path,
        programme,
        harness,
        programmordner,
        schluessel,
        ca_zulassung,
        bwrap,
    };
    f.fertig()?;
    Some(k)
}

/// Where the configuration of this build stands.
#[must_use]
pub fn pfad() -> Option<PathBuf> {
    if cfg!(feature = "pruefbau") {
        let programm = std::env::current_exe().ok()?.canonicalize().ok()?;
        Some(programm.parent()?.join("lauf.json"))
    } else {
        Some(PathBuf::from(KONFIG_PFAD))
    }
}

/// The rule on the result of `fstat`: a regular file that no other user may write, owned by root or by the own user.
#[must_use]
pub fn modus_gut(regulaer: bool, modus: u32, eigner: u32, eigen: u32) -> bool {
    regulaer && modus & 0o022 == 0 && (eigner == 0 || eigner == eigen)
}

/// Reads the configuration of this build. A missing file is `KonfigFehlt`; every other fault is `KonfigKaputt`: the file
/// cannot be opened or read, is not regular, belongs to another user, may be written by others, is over the limit or
/// has an invalid form.
///
/// # Errors
/// `Fehler::KonfigFehlt` or `Fehler::KonfigKaputt` as above.
pub fn lesen_datei(pfad: &Path) -> Result<Konfig, Fehler> {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;
    let datei = std::fs::File::open(pfad).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Fehler::KonfigFehlt
        } else {
            Fehler::KonfigKaputt
        }
    })?;
    let meta = datei.metadata().map_err(|_| Fehler::KonfigKaputt)?;
    let eigen = rustix::process::geteuid().as_raw();
    if !modus_gut(meta.is_file(), meta.mode(), meta.uid(), eigen) {
        return Err(Fehler::KonfigKaputt);
    }
    let mut text = Vec::new();
    datei
        .take(KONFIG_MAX + 1)
        .read_to_end(&mut text)
        .map_err(|_| Fehler::KonfigKaputt)?;
    if u64::try_from(text.len()).map_err(|_| Fehler::KonfigKaputt)? > KONFIG_MAX {
        return Err(Fehler::KonfigKaputt);
    }
    lesen_text(&text).ok_or(Fehler::KonfigKaputt)
}

#[cfg(test)]
mod tests;
