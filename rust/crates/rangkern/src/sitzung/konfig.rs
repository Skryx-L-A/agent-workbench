//! The arguments of `rangkern sitzung` (contract 4, section "Aufruf").

use std::collections::HashSet;
use std::path::PathBuf;

use crate::cli::{db_paths, embedder_of, parse_args};
use crate::embed::Embedder;
use crate::notes::is_key;
use crate::vdb::DbPaths;

use super::mac::{Plattform, aufruf_ok};

/// The options of the process; each stands at most once and only in full.
const OPTIONS: [&str; 13] = [
    "--socket",
    "--route",
    "--sitzung",
    "--einstellungen",
    "--protokoll",
    "--socket-rechte",
    "--vectors",
    "--index-db",
    "--extra-db",
    "--embed",
    "--embed-timeout-ms",
    "--marker",
    "--marker-zaehlung",
];

/// The five options that are required.
const REQUIRED: [&str; 5] = [
    "--socket",
    "--route",
    "--sitzung",
    "--einstellungen",
    "--protokoll",
];

/// The probe-run mode: the markers to count and the file of the counts.
#[derive(Debug)]
pub struct Probelauf {
    /// The markers, 1 to 16, each of the form `^[A-Z0-9-]{1,32}$`, none twice.
    pub marker: Vec<String>,
    /// The file the counts are written to.
    pub zaehlung: PathBuf,
}

/// Whether the text is a marker: 1 to 32 characters of `A-Z0-9-`.
fn ist_marker(text: &str) -> bool {
    (1..=32).contains(&text.len())
        && text
            .bytes()
            .all(|b| matches!(b, b'A'..=b'Z' | b'0'..=b'9' | b'-'))
}

/// The markers of `--marker`: 1 to 16, separated by commas, each of the marker form, none twice.
fn marker_lesen(text: &str) -> Option<Vec<String>> {
    let marker: Vec<String> = text.split(',').map(str::to_owned).collect();
    let unique = marker.iter().collect::<HashSet<_>>().len() == marker.len();
    (marker.len() <= 16 && unique && marker.iter().all(|m| ist_marker(m))).then_some(marker)
}

/// Where the note vectors come from.
#[derive(Debug)]
pub enum Vektoren {
    /// The vector file (`--vectors`), or none.
    Datei(Option<PathBuf>),
    /// The databases of the model (`--index-db`, `--extra-db`).
    Datenbanken(DbPaths),
}

/// The valid arguments of a start.
#[derive(Debug)]
pub struct Konfig {
    /// The vault.
    pub vault: PathBuf,
    /// The path of the socket.
    pub socket: PathBuf,
    /// The rights of the socket: `0o600` or `0o666`.
    pub rechte: u32,
    /// The key of the route of this session.
    pub route: String,
    /// The key of the session.
    pub sitzung: String,
    /// The settings file.
    pub einstellungen: PathBuf,
    /// The folder of the log files.
    pub protokoll: PathBuf,
    /// The note vectors.
    pub vektoren: Vektoren,
    /// The call to Ollama, none without `--embed`.
    pub embedder: Option<Embedder>,
    /// The probe-run mode with its markers, none without `--probelauf`.
    pub probelauf: Option<Probelauf>,
}

impl Konfig {
    /// Reads the arguments after the command name. `None` is a usage error (exit 64): an unknown option or one in short
    /// form, a missing or a repeated one, a vault that is no folder, a route or a session that is no key, a right other than
    /// `0600` and `0666`, or an option for the vectors or the embedding that `rangkern serve` would refuse.
    #[must_use]
    pub fn lesen(args: &[String]) -> Option<Self> {
        Self::lesen_fuer(Plattform::AKTUELL, args)
    }

    /// [`Konfig::lesen`] for a platform: under macOS the socket path must also be absolute and at most 103 bytes long, and the
    /// rights must be `0600` (addendum to contract 4, section 2).
    #[must_use]
    pub fn lesen_fuer(plattform: Plattform, args: &[String]) -> Option<Self> {
        // `--probelauf` is a switch, not an option with a value: it stands at most once, in full.
        let schalter = args.iter().filter(|arg| *arg == "--probelauf").count();
        let uebrige: Vec<String> = args
            .iter()
            .filter(|arg| *arg != "--probelauf")
            .cloned()
            .collect();
        let parsed = parse_args(&uebrige, &OPTIONS, 1)?;
        // An option stands at most once, the five are required, and no option has an empty value (before any look at a
        // file, a folder or the socket).
        if OPTIONS.iter().any(|name| parsed.count(name) > 1)
            || REQUIRED.iter().any(|name| parsed.count(name) == 0)
            || OPTIONS.iter().any(|name| parsed.option(name) == Some(""))
        {
            return None;
        }
        let vault = PathBuf::from(&parsed.positional[0]);
        if !vault.is_dir() {
            return None;
        }
        let route = parsed.option("--route")?;
        let sitzung = parsed.option("--sitzung")?;
        if !is_key(route) || !is_key(sitzung) {
            return None;
        }
        let rechte = match parsed.option("--socket-rechte") {
            None | Some("0600") => 0o600,
            Some("0666") => 0o666,
            Some(_) => return None,
        };
        let socket = PathBuf::from(parsed.option("--socket")?);
        if !aufruf_ok(plattform, &socket, rechte) {
            return None;
        }
        // The three options of the probe run stand together or not at all.
        let probelauf = match (
            schalter,
            parsed.option("--marker"),
            parsed.option("--marker-zaehlung"),
        ) {
            (0, None, None) => None,
            (1, Some(marker), Some(datei)) => Some(Probelauf {
                marker: marker_lesen(marker)?,
                zaehlung: PathBuf::from(datei),
            }),
            _ => return None,
        };
        let dbs = db_paths(&parsed).ok()?;
        let embedder = embedder_of(&parsed, dbs.as_ref()).ok()?;
        let vektoren = match dbs {
            Some(dbs) => Vektoren::Datenbanken(dbs),
            None => Vektoren::Datei(parsed.option("--vectors").map(PathBuf::from)),
        };
        Some(Self {
            vault,
            socket,
            rechte,
            route: route.to_owned(),
            sitzung: sitzung.to_owned(),
            einstellungen: PathBuf::from(parsed.option("--einstellungen")?),
            protokoll: PathBuf::from(parsed.option("--protokoll")?),
            vektoren,
            embedder,
            probelauf,
        })
    }
}
