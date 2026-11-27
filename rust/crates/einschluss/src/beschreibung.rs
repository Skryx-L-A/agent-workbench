//! The typed description of an enclosure: what the session may see, and nothing else.

use std::path::{Path, PathBuf};

use crate::fehler::Fehler;

/// Where the socket of the search process of the session stands inside the enclosure (contract 4).
pub const GAST_SUCH_SOCKET: &str = "/run/brain/such.sock";

/// Where the socket of the model path stands inside the enclosure, when the session has one. The bridge that listens
/// on it is not part of this crate (see [`crate::erweiterung`]).
pub const GAST_MODELL_SOCKET: &str = "/run/brain/modell.sock";

/// Where the program of the forwarder stands inside the enclosure (one file; its folder is not mounted).
pub const GAST_WEITER: &str = "/run/brain/weiter";

/// The harness of the cloud route that this crate knows. Its environment gets the three variables of [`Cloud`].
pub const HARNESS_COPILOT: &str = "copilot";

/// The port of the forwarder on the own loopback of the enclosure. The harness reaches the outside only through it.
pub const WEITER_PORT: u16 = 8402;

/// The port of the forwarder of the local route on the own loopback of the enclosure (contract 1, E5): the harness
/// reaches the model path of the session there, and nothing else.
pub const LOKAL_PORT: u16 = 11434;

/// The harness of the local route that this crate knows. Its environment gets the two variables of [`PI_UMGEBUNG`].
pub const HARNESS_PI: &str = "pi";

/// The fixed variables of the harness [`HARNESS_PI`] (contract 1, start form of `pi`). The values live in this crate;
/// the input never carries them.
pub const PI_UMGEBUNG: [(&str, &str); 2] = [("PI_OFFLINE", "1"), ("PI_TELEMETRY", "0")];

/// The configuration files of a harness that "Lauf starten" writes outside and that the enclosure binds read-only, as
/// (harness, place below the home). The agent cannot change its own provider and MCP configuration. No other place
/// and no other harness is allowed.
pub const DATEI_ZIELE: [(&str, &str); 2] = [
    ("pi", ".pi/agent/models.json"),
    ("copilot", ".copilot/wb-mcp.json"),
];

/// The fixed stand-in for the token of the cloud harness: `gho_` and 36 zeros (contract 1 of the cloud route). The
/// real key never enters the enclosure; the relay outside swaps this text for it.
///
/// The text is put together from parts so that no tool that looks for token shapes in source files takes it for a key.
pub const COPILOT_PLATZHALTER: &str =
    concat!("gho_", "000000000000", "000000000000", "000000000000");

/// The state folder of the session: always empty at the start, mounted below the home of the session; it is never the
/// state of the operator. Only the place of the conversation inside it ([`GESPRAECH_ORTE`]) may carry what an earlier
/// session wrote: the settings of the harness never pass from one session to the next (contract 1, Fassung 2, OPEN 34).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zustand {
    /// The folder on the host: exists, belongs to the own user, and is empty.
    pub host: PathBuf,
    /// Its name below the home inside the enclosure, one plain segment (for example `.claude`).
    pub name: String,
    /// The session continues the conversation of an earlier one (contract 1, Fassung 2): the folder of the conversation
    /// holds its state and need not be empty. Only together with `gespraech`.
    pub fortsetzen: bool,
    /// The folder of the conversation on the host, if the session keeps one: it exists, belongs to the own user, is
    /// empty unless `fortsetzen` is set, and is bound writable at the place of the harness in [`GESPRAECH_ORTE`] below
    /// the state folder.
    pub gespraech: Option<PathBuf>,
}

/// The place of the conversation of each harness below its state folder (measured 27.11.2026 with claude 2.1.332,
/// copilot 1.0.116 and pi 1.6.1, contract 1, "Gespräch und Einstellungen im Zustandsordner"). No path a harness reads
/// as a setting lies below these places.
pub const GESPRAECH_ORTE: [(&str, &str); 3] = [
    ("claude", "projects"),
    ("copilot", "session-state"),
    ("pi", "agent/sessions"),
];

/// The place of the conversation of `harness` below its state folder, if it has one.
#[must_use]
pub fn gespraech_ort(harness: &str) -> Option<&'static str> {
    GESPRAECH_ORTE
        .iter()
        .find(|(h, _)| *h == harness)
        .map(|(_, ort)| *ort)
}

/// The environment of the harness. Nothing else reaches it: the profile clears the environment first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Umgebung {
    /// `HOME`; must be the home of the description.
    pub home: String,
    /// `PATH`.
    pub path: String,
    /// `TERM`.
    pub term: String,
    /// `LANG`.
    pub lang: String,
    /// `TMPDIR`, an absolute path (the profile mounts a private `/tmp`).
    pub tmpdir: String,
}

/// What the host contributes to the system part of the profile: the owner and the layout of `/usr`.
/// It is a value so that the profile builder stays a pure function; [`System::vom_host`] reads the real host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System {
    /// The numeric user of the process. The profile makes `/run/user/<uid>` an empty folder.
    pub uid: u32,
    /// Links of the host root that the enclosure repeats, as (link text, link path), for example `usr/bin` and `/bin`.
    pub symlinks: Vec<(String, String)>,
    /// Read-only bindings of the system part, as (host path, path in the enclosure).
    pub ro_binds: Vec<(String, String)>,
}

impl System {
    /// The layout of Arch with merged `/usr`: `/bin` and `/sbin` are links to `usr/bin`, `/lib` and `/lib64` links to
    /// `usr/lib`, and five files of `/etc` are bound. A pattern for the pure tests only, never for a real enclosure:
    /// other systems link differently (Ubuntu 26.04 and Debian: `/sbin -> usr/sbin`, `/lib64 -> usr/lib64`, measured;
    /// with this pattern `bwrap` there finds no loader), so a real enclosure reads the links of the host
    /// ([`System::vom_host`]).
    #[must_use]
    pub fn usr_merge(uid: u32) -> System {
        let verweise = [
            ("usr/bin", "/bin"),
            ("usr/bin", "/sbin"),
            ("usr/lib", "/lib"),
            ("usr/lib", "/lib64"),
        ];
        System {
            uid,
            symlinks: verweise
                .iter()
                .map(|(text, pfad)| ((*text).to_owned(), (*pfad).to_owned()))
                .collect(),
            ro_binds: ETC_DATEIEN
                .iter()
                .map(|p| ((*p).to_owned(), (*p).to_owned()))
                .collect(),
        }
    }

    /// Reads the real host: the user of this process, which of `/bin`, `/sbin`, `/lib`, `/lib64`, `/lib32` are links
    /// (repeated) or folders (bound), and which of the five `/etc` files exist (bound from the resolved path).
    ///
    /// # Errors
    /// [`Fehler::NichtVerfuegbar`] on every system but Linux; [`Fehler::QuelleUnbrauchbar`] when the user of the
    /// process cannot be read.
    pub fn vom_host() -> Result<System, Fehler> {
        System::vom_host_mit(Path::new(ORDNERMARKE_PFAD))
    }

    /// As [`System::vom_host`], with the host path of the command of the human a parameter (for tests). When that
    /// file exists, the enclosure hides it: `/dev/null` is bound over [`ORDNERMARKE_PFAD`] right after the bind of
    /// `/usr` (contract 1, "Ordnermarke": the command is not there inside). Without the file there is no bind,
    /// because `bwrap` fails at a missing place in the read-only `/usr`.
    ///
    /// # Errors
    /// As [`System::vom_host`].
    pub fn vom_host_mit(ordnermarke: &Path) -> Result<System, Fehler> {
        // one body with a `cfg` block per system: cargo-mutants does not evaluate `cfg` (see `starter::selbsttest_lauf`)
        #[cfg(target_os = "linux")]
        let host = host_linux::lesen();
        #[cfg(not(target_os = "linux"))]
        let host: Result<System, Fehler> = Err(Fehler::NichtVerfuegbar);
        let mut system = host?;
        if ordnermarke.exists() {
            system
                .ro_binds
                .push(("/dev/null".to_owned(), ORDNERMARKE_PFAD.to_owned()));
        }
        Ok(system)
    }
}

/// The place of the command `werkbank-ordnermarke` in the package (contract 1, open point of the contract). The enclosure sees `/usr`, so
/// it hides this file by a bind of `/dev/null` over it; this pair is the only bind of `/dev/null` the profile accepts.
pub const ORDNERMARKE_PFAD: &str = "/usr/bin/werkbank-ordnermarke";

/// The files of `/etc` the enclosure sees. No name service, no certificates: the enclosure has no network.
pub const ETC_DATEIEN: [&str; 5] = [
    "/etc/passwd",
    "/etc/group",
    "/etc/nsswitch.conf",
    "/etc/ld.so.cache",
    "/etc/localtime",
];

#[cfg(target_os = "linux")]
mod host_linux {
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    use super::{ETC_DATEIEN, System};
    use crate::fehler::Fehler;

    pub(super) fn lesen() -> Result<System, Fehler> {
        let uid = fs::metadata("/proc/self")
            .map_err(|_| Fehler::QuelleUnbrauchbar("user of the process"))?
            .uid();
        let mut symlinks = Vec::new();
        let mut ro_binds = Vec::new();
        for ziel in ["/bin", "/sbin", "/lib", "/lib64", "/lib32"] {
            let pfad = Path::new(ziel);
            if pfad.is_symlink() {
                if let Some(text) = fs::read_link(pfad)
                    .ok()
                    .and_then(|t| t.to_str().map(str::to_owned))
                {
                    symlinks.push((text, ziel.to_owned()));
                }
            } else if pfad.exists() {
                ro_binds.push((ziel.to_owned(), ziel.to_owned()));
            }
        }
        for datei in ETC_DATEIEN {
            if let Ok(echt) = fs::canonicalize(datei)
                && let Some(quelle) = echt.to_str()
            {
                ro_binds.push((quelle.to_owned(), datei.to_owned()));
            }
        }
        Ok(System {
            uid,
            symlinks,
            ro_binds,
        })
    }
}

/// The distribution of the host. It decides where the system bundle of certificates stands, which the session bundle
/// replaces inside the enclosure. The host bundle itself is never changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verteilung {
    /// Ubuntu and Debian: `/etc/ssl/certs/ca-certificates.crt`.
    Ubuntu,
    /// Arch and its derivatives: `/etc/ssl/certs/ca-certificates.crt` and `/etc/ssl/cert.pem` (both are links to the
    /// extracted bundle on the host; inside they are the session file).
    Arch,
}

impl Verteilung {
    /// The places inside the enclosure where the session bundle is bound. Fixed per distribution.
    #[must_use]
    pub fn bundle_orte(self) -> &'static [&'static str] {
        match self {
            Verteilung::Ubuntu => &["/etc/ssl/certs/ca-certificates.crt"],
            Verteilung::Arch => &["/etc/ssl/certs/ca-certificates.crt", "/etc/ssl/cert.pem"],
        }
    }

    /// Reads the distribution from the text of `/etc/os-release`: `ID` or `ID_LIKE` names `ubuntu` or `debian`, or
    /// `arch`. `None` for any other system: there the cloud route is not available until its place is measured.
    #[must_use]
    pub fn aus_os_release(text: &str) -> Option<Verteilung> {
        let mut namen: Vec<String> = Vec::new();
        for zeile in text.lines() {
            if let Some((schluessel, wert)) = zeile.split_once('=')
                && matches!(schluessel, "ID" | "ID_LIKE")
            {
                let wert = wert.trim().trim_matches('"').trim_matches('\'');
                namen.extend(wert.split_whitespace().map(str::to_ascii_lowercase));
            }
        }
        if namen.iter().any(|n| n == "ubuntu" || n == "debian") {
            Some(Verteilung::Ubuntu)
        } else if namen.iter().any(|n| n == "arch") {
            Some(Verteilung::Arch)
        } else {
            None
        }
    }

    /// The distribution of this host, from `/etc/os-release`.
    ///
    /// # Errors
    /// [`Fehler::NichtVerfuegbar`] when the file cannot be read or the distribution is not known.
    pub fn vom_host() -> Result<Verteilung, Fehler> {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|t| Verteilung::aus_os_release(&t))
            .ok_or(Fehler::NichtVerfuegbar)
    }
}

/// The exit of the cloud harness (the route `copilot`): a loopback forwarder inside the enclosure, the socket of the
/// relay outside, and the bundle of certificates of the session. The crate only binds what it is given; the relay and
/// the throw-away certificate authority are made by the program "Lauf starten" outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cloud {
    /// The socket of the relay on the host. It stands at [`GAST_MODELL_SOCKET`] inside; the description must not
    /// carry a `modell_socket` as well.
    pub weiterreicher: PathBuf,
    /// The program of the forwarder on the host (the binary `einschluss-weiter`): a regular file, bound read-only at
    /// [`GAST_WEITER`]. It listens on `127.0.0.1:`[`WEITER_PORT`] inside and passes the bytes to the relay socket.
    pub weiter_programm: PathBuf,
    /// The bundle file of the session on the host (the bundle of the host plus the certificate of the session). It is
    /// bound read-only at the places of the system bundle; it lies outside every writable mount.
    pub bundle: PathBuf,
    /// The distribution, which fixes the places of the bundle.
    pub verteilung: Verteilung,
}

/// One configuration file of the harness: a file of the own user, mode 0600, outside every writable mount, bound
/// read-only at a fixed place of [`DATEI_ZIELE`] below the home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datei {
    /// The file on the host: a regular file (no link), owned by the user, mode exactly 0600.
    pub quelle: PathBuf,
    /// The place inside the enclosure: `<heim>/<place of DATEI_ZIELE for the harness>`.
    pub ziel: PathBuf,
}

/// The platforms of the package of the cloud harness this crate knows (contract 1, "Paket der Copilot CLI").
pub const PAKET_PLATTFORMEN: [&str; 2] = ["linux-x64", "linux-arm64"];

/// The mark the cloud harness writes last when it has extracted its package.
pub const PAKET_MARKE: &str = ".extraction-complete";

/// Where the cloud harness extracts its package below the home: `<home>/.cache/copilot/pkg/<platform>/<version>`
/// (the harness takes `$XDG_CACHE_HOME` first; the enclosure never sets it).
pub const PAKET_GAST_CACHE: &str = ".cache/copilot";

/// The extracted package of the cloud harness: a complete folder of the own user outside the enclosure, bound
/// read-only at [`paket_ziel`]. Without it the harness extracts the package into the home of the enclosure, a tmpfs of
/// 16 MiB, and fails (`ENOSPC`, measured with the CLI 1.0.88).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paket {
    /// The folder on the host: `<cache>/pkg/<platform>/<version>`, a real path without links, owned by the user, with
    /// the four paths of [`paket_pfade`].
    pub quelle: PathBuf,
    /// One of [`PAKET_PLATTFORMEN`].
    pub plattform: String,
    /// The version of the package, `<number>.<number>.<number>`.
    pub fassung: String,
}

/// `true` for a version of the form `<number>.<number>.<number>` (each 1 to 9 ASCII digits).
#[must_use]
pub fn fassung_gut(text: &str) -> bool {
    let teile: Vec<&str> = text.split('.').collect();
    teile.len() == 3
        && teile
            .iter()
            .all(|t| !t.is_empty() && t.len() <= 9 && t.bytes().all(|b| b.is_ascii_digit()))
}

/// The four paths below the folder of the package that make it complete (measured for `linux-x64`: without one of them
/// the harness extracts again). The mark comes last.
#[must_use]
pub fn paket_pfade(plattform: &str) -> [String; 4] {
    [
        "index.js".to_owned(),
        "app.js".to_owned(),
        format!("prebuilds/{plattform}/runtime.node"),
        PAKET_MARKE.to_owned(),
    ]
}

/// The place of the package inside the enclosure: `<heim>/.cache/copilot/pkg/<platform>/<version>`.
#[must_use]
pub fn paket_ziel(heim: &Path, plattform: &str, fassung: &str) -> PathBuf {
    let cache = crate::pfad::gast_pfad(heim, PAKET_GAST_CACHE);
    let pkg = crate::pfad::gast_pfad(&cache, "pkg");
    crate::pfad::gast_pfad(&crate::pfad::gast_pfad(&pkg, plattform), fassung)
}

/// Everything the enclosure needs, as typed fields. All paths are absolute and normalised (see
/// [`crate::pfad::normal`]) and are real paths without links: the profile mounts what it is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beschreibung {
    /// The harness, as its identifier stands in the harness table of contract 1. It decides which login files of the
    /// account are foreign and therefore closed.
    pub harness: String,
    /// The working folder: writable, the start directory of the harness.
    pub arbeitsordner: PathBuf,
    /// The program folders of the harness: read-only, at the same path inside the enclosure.
    pub programmordner: Vec<PathBuf>,
    /// The empty state folder of the session.
    pub zustand: Zustand,
    /// The home of the user on the host. Inside the enclosure it is a private, empty tmpfs; the other mounts below it
    /// come after it.
    pub heim: PathBuf,
    /// The socket of the search process of the session on the host (contract 4). It stands at
    /// [`GAST_SUCH_SOCKET`] inside; its folder is never mounted.
    pub such_socket: PathBuf,
    /// The socket of the model path on the host, if the session has one. It stands at [`GAST_MODELL_SOCKET`].
    pub modell_socket: Option<PathBuf>,
    /// The program of the forwarder of the local route on the host (the binary `einschluss-weiter`), only together with
    /// `modell_socket`: bound read-only at [`GAST_WEITER`], it listens on `127.0.0.1:`[`LOKAL_PORT`] inside and passes
    /// the bytes to the model socket. Without it the model socket is bound, but nothing listens on the loopback.
    pub lokal_weiter: Option<PathBuf>,
    /// The exit of the cloud harness, if the session has one. Only for the harness [`HARNESS_COPILOT`].
    pub cloud: Option<Cloud>,
    /// The extracted package of the cloud harness, bound read-only at [`paket_ziel`]. Only for the harness
    /// [`HARNESS_COPILOT`].
    pub paket: Option<Paket>,
    /// The configuration files of the harness, bound read-only (at most one per place of [`DATEI_ZIELE`]).
    pub dateien: Vec<Datei>,
    /// The environment of the harness.
    pub umgebung: Umgebung,
    /// Folders that must not overlap any mount: the vault, the log folder, the folder of the start file.
    pub geschuetzt: Vec<PathBuf>,
    /// The program and its arguments, as the harness table of contract 1 builds them.
    pub argv: Vec<String>,
    /// The host part of the system layout.
    pub system: System,
}
