//! The settings of server and client and their check (step 1 of "Server anlegen" and of "Client verbinden").
//!
//! The same rules hold for the Rust values and for their JSON form (`peerkanal.schema.json`, `$defs/server_vorgabe` and
//! `client_vorgabe`). The check takes the system as a parameter, so the rules are testable for every system on any machine.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value};

use crate::fehler::{Code, Fehler, System};
use crate::kennung::{Kanalname, Kennung, PackageSid, Sid};

/// The largest uid or gid of the schema (`2^32 - 2`; `2^32 - 1` is the invalid id).
const UNIX_ID_MAX: u32 = u32::MAX - 1;

/// One allowed identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Erlaubnis {
    /// Windows: the pair holds exactly so. `appcontainer_sid: None` allows only processes outside a container.
    Windows {
        /// The SID of the person or the service.
        sid: Sid,
        /// The expected package SID, or `None` for a process outside a container.
        appcontainer_sid: Option<PackageSid>,
    },
    /// Unix: uid, and if given the primary gid.
    Unix {
        /// The uid.
        uid: u32,
        /// The primary gid, if it must match.
        gid: Option<u32>,
    },
}

/// The mode of the Unix socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketModus {
    /// 0600.
    M0600,
    /// 0660.
    M0660,
    /// 0666.
    M0666,
}

impl SocketModus {
    /// The permission bits.
    #[must_use]
    pub fn bits(self) -> u32 {
        match self {
            Self::M0600 => 0o600,
            Self::M0660 => 0o660,
            Self::M0666 => 0o666,
        }
    }

    fn aus_text(text: &str) -> Option<Self> {
        match text {
            "0600" => Some(Self::M0600),
            "0660" => Some(Self::M0660),
            "0666" => Some(Self::M0666),
            _ => None,
        }
    }
}

/// Unix: the folder of the socket and its mode. Mandatory under Unix, forbidden under Windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnixOrt {
    /// An absolute folder without `.` and `..`.
    pub ordner: PathBuf,
    /// The mode of the socket.
    pub modus: SocketModus,
}

/// Windows only: the handle of the job of the run. With it, the peer carries `im_job`; without it, `im_job` is `None`.
///
/// On other systems the type has no value, so a job cannot be given there.
pub struct JobBezug {
    #[cfg(windows)]
    pub(crate) griff: std::os::windows::io::OwnedHandle,
    #[cfg(not(windows))]
    nie: std::convert::Infallible,
}

#[cfg(windows)]
impl JobBezug {
    /// Takes the handle of the job. The library only asks `IsProcessInJob` with it.
    #[must_use]
    pub fn neu(griff: std::os::windows::io::OwnedHandle) -> Self {
        Self { griff }
    }

    /// Opens a named job object (`OpenJobObjectW` with `JOB_OBJECT_QUERY`), for a caller that knows the job only by name.
    ///
    /// # Errors
    /// The error of the system, for example when no job has this name.
    pub fn oeffnen(name: &str) -> std::io::Result<Self> {
        crate::sys::job_oeffnen(name).map(Self::neu)
    }
}

impl std::fmt::Debug for JobBezug {
    #[cfg_attr(not(windows), allow(unused_variables))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(not(windows))]
        match self.nie {}
        #[cfg(windows)]
        f.write_str("JobBezug")
    }
}

/// The settings of a server.
#[derive(Debug)]
pub struct ServerVorgabe {
    /// The channel name.
    pub name: Kanalname,
    /// 1 to 64 permissions without duplicates, all of the kind of the system.
    pub erlaubt: Vec<Erlaubnis>,
    /// 1 to 64 connections at the same time.
    pub instanzen: u8,
    /// 1 ms to 60 s; default 2 s.
    pub hallo_frist: Duration,
    /// Refuse when the process cannot be bound. Default `false`.
    pub prozess_pflicht: bool,
    /// Windows only, optional: the job of the run.
    pub job: Option<JobBezug>,
    /// Mandatory under Unix, forbidden under Windows.
    pub unix: Option<UnixOrt>,
}

impl ServerVorgabe {
    /// Settings with the defaults of the contract: `hallo_frist` 2 s, no process duty, no job, no Unix part.
    #[must_use]
    pub fn neu(name: Kanalname, erlaubt: Vec<Erlaubnis>, instanzen: u8) -> Self {
        Self {
            name,
            erlaubt,
            instanzen,
            hallo_frist: Duration::from_secs(2),
            prozess_pflicht: false,
            job: None,
            unix: None,
        }
    }

    /// Step 1 of "Server anlegen" for the given system.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig`.
    pub fn pruefen(&self, system: System) -> Result<(), Fehler> {
        pruefe_server(&ServerTeile {
            erlaubt: &self.erlaubt,
            instanzen: u64::from(self.instanzen),
            hallo_frist: self.hallo_frist,
            job: self.job.is_some(),
            unix: self.unix.as_ref(),
            system,
        })
    }
}

/// The identity the client expects of the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Erwartet {
    /// Windows: the owner of the pipe instance.
    Windows {
        /// The expected SID.
        sid: Sid,
    },
    /// Unix: the uid of the server.
    Unix {
        /// The expected uid.
        uid: u32,
    },
}

/// The settings of a client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientVorgabe {
    /// The channel name.
    pub name: Kanalname,
    /// The expected identity of the server.
    pub erwartet: Erwartet,
    /// 1 ms to 60 s for the whole connection: waiting for a free instance and the answer to the hello.
    pub frist: Duration,
    /// Unix: the folder of the socket. Mandatory under Unix, forbidden under Windows.
    pub unix_ordner: Option<PathBuf>,
}

impl ClientVorgabe {
    /// Step 1 of "Client verbinden" for the given system.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig`.
    pub fn pruefen(&self, system: System) -> Result<(), Fehler> {
        let ungueltig = Err(Fehler::neu(Code::VorgabeUngueltig));
        if !frist_gueltig(self.frist) {
            return ungueltig;
        }
        match (&self.erwartet, system.ist_windows()) {
            (Erwartet::Windows { .. }, true) => {
                if self.unix_ordner.is_some() {
                    return ungueltig;
                }
            }
            (Erwartet::Unix { uid }, false) => {
                if *uid > UNIX_ID_MAX {
                    return ungueltig;
                }
                match &self.unix_ordner {
                    Some(o) if pfad_gueltig(o) => {}
                    _ => return ungueltig,
                }
            }
            _ => return ungueltig,
        }
        Ok(())
    }
}

struct ServerTeile<'a> {
    erlaubt: &'a [Erlaubnis],
    instanzen: u64,
    hallo_frist: Duration,
    job: bool,
    unix: Option<&'a UnixOrt>,
    system: System,
}

fn frist_gueltig(d: Duration) -> bool {
    d >= Duration::from_millis(1) && d <= Duration::from_secs(60)
}

fn pruefe_server(t: &ServerTeile<'_>) -> Result<(), Fehler> {
    let ungueltig = Err(Fehler::neu(Code::VorgabeUngueltig));
    if t.erlaubt.is_empty() || t.erlaubt.len() > 64 {
        return ungueltig;
    }
    for (i, a) in t.erlaubt.iter().enumerate() {
        if t.erlaubt[..i].contains(a) {
            return ungueltig;
        }
        let passt = match a {
            Erlaubnis::Windows { .. } => t.system.ist_windows(),
            Erlaubnis::Unix { uid, gid } => {
                !t.system.ist_windows()
                    && *uid <= UNIX_ID_MAX
                    && gid.is_none_or(|g| g <= UNIX_ID_MAX)
            }
        };
        if !passt {
            return ungueltig;
        }
    }
    if !(1..=64).contains(&t.instanzen) || !frist_gueltig(t.hallo_frist) {
        return ungueltig;
    }
    if t.system.ist_windows() {
        if t.unix.is_some() {
            return ungueltig;
        }
    } else {
        if t.job {
            return ungueltig;
        }
        match t.unix {
            Some(o) if pfad_gueltig(&o.ordner) => {}
            _ => return ungueltig,
        }
    }
    Ok(())
}

/// The path rule of the schema (`$defs/pfad`): absolute, no `.` or `..` component, no `//`, no NUL, at most 4096 characters.
pub(crate) fn pfad_gueltig(p: &Path) -> bool {
    let Some(s) = p.to_str() else {
        return false;
    };
    s.starts_with('/')
        && !s.contains("//")
        && !s.contains('\0')
        && s.chars().count() <= 4096
        && !s.split('/').any(|t| t == "." || t == "..")
}

// ---------------------------------------------------------------------------------------------------------------------
// JSON form

fn objekt<'a>(v: &'a Value, erlaubt: &[&str], pflicht: &[&str]) -> Option<&'a Map<String, Value>> {
    let o = v.as_object()?;
    if o.keys().any(|k| !erlaubt.contains(&k.as_str()))
        || pflicht.iter().any(|k| !o.contains_key(*k))
    {
        return None;
    }
    Some(o)
}

fn unix_id(v: &Value) -> Option<u32> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n <= UNIX_ID_MAX)
}

fn frist_ms(v: &Value) -> Option<Duration> {
    v.as_u64()
        .filter(|n| (1..=60_000).contains(n))
        .map(Duration::from_millis)
}

fn pfad_json(v: &Value) -> Option<PathBuf> {
    let p = PathBuf::from(v.as_str()?);
    pfad_gueltig(&p).then_some(p)
}

fn erlaubnis_json(v: &Value) -> Option<Erlaubnis> {
    let o = v.as_object()?;
    if o.contains_key("sid") {
        let o = objekt(
            v,
            &["sid", "appcontainer_sid"],
            &["sid", "appcontainer_sid"],
        )?;
        let sid = Sid::neu(o.get("sid")?.as_str()?).ok()?;
        let appcontainer_sid = match o.get("appcontainer_sid")? {
            Value::Null => None,
            Value::String(s) => Some(PackageSid::neu(s).ok()?),
            _ => return None,
        };
        Some(Erlaubnis::Windows {
            sid,
            appcontainer_sid,
        })
    } else {
        let o = objekt(v, &["uid", "gid"], &["uid"])?;
        let uid = unix_id(o.get("uid")?)?;
        let gid = match o.get("gid") {
            None => None,
            Some(g) => Some(unix_id(g)?),
        };
        Some(Erlaubnis::Unix { uid, gid })
    }
}

/// Checks settings of a server in their JSON form (`$defs/server_vorgabe`) for the given system. `im_job_pruefen: true`
/// stands for a job handle, which JSON cannot carry.
///
/// # Errors
/// `pk.vorgabe_ungueltig`.
pub fn server_json_pruefen(v: &Value, system: System) -> Result<(), Fehler> {
    server_json(v, system).ok_or(Fehler::neu(Code::VorgabeUngueltig))?
}

fn server_json(v: &Value, system: System) -> Option<Result<(), Fehler>> {
    let o = objekt(
        v,
        &[
            "name",
            "erlaubt",
            "instanzen",
            "hallo_frist_ms",
            "prozess_pflicht",
            "im_job_pruefen",
            "unix",
        ],
        &["name", "erlaubt", "instanzen"],
    )?;
    Kennung::neu(o.get("name")?.as_str()?).ok()?;
    let erlaubt = o
        .get("erlaubt")?
        .as_array()?
        .iter()
        .map(erlaubnis_json)
        .collect::<Option<Vec<_>>>()?;
    let instanzen = o.get("instanzen")?.as_u64()?;
    let hallo_frist = match o.get("hallo_frist_ms") {
        None => Duration::from_secs(2),
        Some(f) => frist_ms(f)?,
    };
    if let Some(p) = o.get("prozess_pflicht") {
        p.as_bool()?;
    }
    let job = match o.get("im_job_pruefen") {
        None => false,
        Some(Value::Bool(true)) => true,
        Some(_) => return None,
    };
    let unix = match o.get("unix") {
        None => None,
        Some(u) => {
            let u = objekt(u, &["ordner", "modus"], &["ordner", "modus"])?;
            Some(UnixOrt {
                ordner: pfad_json(u.get("ordner")?)?,
                modus: SocketModus::aus_text(u.get("modus")?.as_str()?)?,
            })
        }
    };
    Some(pruefe_server(&ServerTeile {
        erlaubt: &erlaubt,
        instanzen,
        hallo_frist,
        job,
        unix: unix.as_ref(),
        system,
    }))
}

/// Reads settings of a client in their JSON form (`$defs/client_vorgabe`) and checks them for the given system.
///
/// # Errors
/// `pk.vorgabe_ungueltig`.
pub fn client_json_lesen(v: &Value, system: System) -> Result<ClientVorgabe, Fehler> {
    let vorgabe = client_json(v).ok_or(Fehler::neu(Code::VorgabeUngueltig))?;
    vorgabe.pruefen(system)?;
    Ok(vorgabe)
}

fn client_json(v: &Value) -> Option<ClientVorgabe> {
    let o = objekt(
        v,
        &["name", "erwartet", "frist_ms", "unix_ordner"],
        &["name", "erwartet", "frist_ms"],
    )?;
    let name = Kennung::neu(o.get("name")?.as_str()?).ok()?;
    let e = o.get("erwartet")?;
    let erwartet = if let Some(e) = objekt(e, &["sid"], &["sid"]) {
        Erwartet::Windows {
            sid: Sid::neu(e.get("sid")?.as_str()?).ok()?,
        }
    } else {
        let e = objekt(e, &["uid"], &["uid"])?;
        Erwartet::Unix {
            uid: unix_id(e.get("uid")?)?,
        }
    };
    let frist = frist_ms(o.get("frist_ms")?)?;
    let unix_ordner = match o.get("unix_ordner") {
        None => None,
        Some(p) => Some(pfad_json(p)?),
    };
    Some(ClientVorgabe {
        name,
        erwartet,
        frist,
        unix_ordner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person() -> Sid {
        Sid::neu("S-1-5-21-1000000001-1000000002-1000000003-1001").expect("form")
    }

    fn windows_vorgabe() -> ServerVorgabe {
        ServerVorgabe::neu(
            Kennung::neu("k-beispiel").expect("form"),
            vec![Erlaubnis::Windows {
                sid: person(),
                appcontainer_sid: None,
            }],
            4,
        )
    }

    #[test]
    fn rust_werte_gleiche_regeln() {
        let mut v = windows_vorgabe();
        assert!(v.pruefen(System::Windows).is_ok());
        assert!(v.pruefen(System::Linux).is_err(), "SID unter Unix");
        v.instanzen = 0;
        assert!(v.pruefen(System::Windows).is_err());
        v.instanzen = 65;
        assert!(v.pruefen(System::Windows).is_err());
        v.instanzen = 64;
        v.hallo_frist = Duration::ZERO;
        assert!(v.pruefen(System::Windows).is_err());
        v.hallo_frist = Duration::from_millis(60_001);
        assert!(v.pruefen(System::Windows).is_err());
        v.hallo_frist = Duration::from_secs(60);
        assert!(v.pruefen(System::Windows).is_ok());
        v.erlaubt.push(v.erlaubt[0].clone());
        assert!(v.pruefen(System::Windows).is_err(), "Doppel");
        v.erlaubt.pop();
        v.unix = Some(UnixOrt {
            ordner: PathBuf::from("/a"),
            modus: SocketModus::M0600,
        });
        assert!(
            v.pruefen(System::Windows).is_err(),
            "Unix-Teil unter Windows"
        );
    }

    #[test]
    fn unix_regeln() {
        let mut v = ServerVorgabe::neu(
            Kennung::neu("k").expect("form"),
            vec![Erlaubnis::Unix { uid: 1, gid: None }],
            1,
        );
        assert!(v.pruefen(System::Linux).is_err(), "Unix ohne Ort");
        v.unix = Some(UnixOrt {
            ordner: PathBuf::from("/a/b"),
            modus: SocketModus::M0600,
        });
        assert!(v.pruefen(System::Linux).is_ok());
        assert!(v.pruefen(System::Macos).is_ok());
        assert!(v.pruefen(System::Windows).is_err());
        v.erlaubt = vec![Erlaubnis::Unix {
            uid: u32::MAX,
            gid: None,
        }];
        assert!(v.pruefen(System::Linux).is_err());
        for schlecht in ["a/b", "/a/../b", "/a/./b", "/a//b", "/a/..", "/."] {
            assert!(!pfad_gueltig(Path::new(schlecht)), "{schlecht}");
        }
        for gut in ["/", "/a", "/a/b/", "/a/.b", "/a/..b"] {
            assert!(pfad_gueltig(Path::new(gut)), "{gut}");
        }
    }

    #[test]
    fn client_regeln() {
        let mut c = ClientVorgabe {
            name: Kennung::neu("k").expect("form"),
            erwartet: Erwartet::Windows { sid: person() },
            frist: Duration::from_secs(2),
            unix_ordner: None,
        };
        assert!(c.pruefen(System::Windows).is_ok());
        assert!(c.pruefen(System::Linux).is_err());
        c.frist = Duration::ZERO;
        assert!(c.pruefen(System::Windows).is_err());
        c.frist = Duration::from_secs(2);
        c.unix_ordner = Some(PathBuf::from("/a"));
        assert!(c.pruefen(System::Windows).is_err());
        c.erwartet = Erwartet::Unix { uid: 5 };
        assert!(c.pruefen(System::Linux).is_ok());
        c.unix_ordner = None;
        assert!(c.pruefen(System::Linux).is_err());
    }
}
