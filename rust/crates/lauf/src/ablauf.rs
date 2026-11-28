//! The steps 0 to 8 of contract 1 ("Ablauf vor dem exec"), in this order. The first fault ends the program; after step
//! 7 the children are ended with SIGTERM first.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use einstellungen::{Einstellungen, Route, Routenart, Schema};
use start::{Startdatei, Startform};

use crate::fehler::Fehler;
use crate::kind::{Ausfall, BEREIT_FRIST, Kind};
use crate::konfig::Konfig;
use crate::meldung::Meldung;
use crate::uebergabe::Platz;
use crate::{dateien, gespraech, harness, kind, marke, uebergabe};

/// The longest host path of a socket in bytes, without the closing zero byte (`sun_path`: 108 on Linux, 104 on macOS).
pub const SOCKET_MAX: usize = if cfg!(target_os = "linux") { 107 } else { 103 };

/// The largest settings file that is read; a longer one is cut there and stage 1 of contract 2 refuses it.
const EINST_LESEN_MAX: u64 = 65_537;

/// What the start found in the own environment, read once at the start.
#[derive(Debug, Clone)]
pub struct Umgebung {
    /// The names of the own environment (step 0).
    pub namen: Vec<OsString>,
    /// The own `TERM`, the one variable of the service the harness gets.
    pub term: Option<String>,
    /// The own `XDG_CACHE_HOME`: the base of the cache folder of the harness when it lies inside the home (step 6a).
    pub xdg_cache: Option<OsString>,
}

impl Umgebung {
    /// The environment of this process.
    #[must_use]
    pub fn eigene() -> Umgebung {
        Umgebung {
            namen: std::env::vars_os().map(|(n, _)| n).collect(),
            term: std::env::var("TERM").ok(),
            xdg_cache: std::env::var_os("XDG_CACHE_HOME"),
        }
    }
}

/// The checked start: the start file, the settings, the configuration and the paths derived from them.
struct Lage {
    start: Startdatei,
    einst: Einstellungen,
    konfig: Konfig,
    /// The folder of the start file (resolved): the session files lie next to it.
    startordner: PathBuf,
    /// `realpath(arbeitsordner)`.
    ordner: PathBuf,
    /// Step 5d: the conversation of the predecessor, when the start continues one.
    fortsetzung: Option<gespraech::Fortsetzung>,
}

impl Lage {
    fn route(&self) -> Result<&Route, Fehler> {
        self.einst.route(self.start.route()).ok_or(Fehler::Intern)
    }
}

/// The session files this run made next to the start file (key and certificate of the CA, bundle, PID files). They
/// are removed again when a step fails; a file that existed before is never recorded (it was refused, not made).
#[derive(Default)]
struct Spuren(Vec<PathBuf>);

impl Spuren {
    /// Writes a new file and records it as soon as it exists: a name that was refused (taken, or a fault before the
    /// file was made) is never recorded, so it is never removed.
    fn schreiben(&mut self, pfad: &Path, inhalt: &[u8]) -> std::io::Result<()> {
        let mut datei = dateien::datei_anlegen(pfad)?;
        self.0.push(pfad.to_path_buf());
        dateien::datei_fuellen(&mut datei, inhalt)
    }

    /// Best effort: a fault of the removal changes nothing about the code of the program.
    fn loeschen(&mut self) {
        for p in self.0.drain(..) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// The children that run so far; ended when a later step fails.
#[derive(Default)]
struct Kinder {
    suche: Option<Kind>,
    modellweg: Option<Kind>,
    spuren: Spuren,
    /// The socket folder of the session, once this run made it (a folder that was there before is never recorded).
    ordner: Option<PathBuf>,
    /// The session folder of the harness (`sitzungen/<sitzung>`), once this run made it: a folder that was there before
    /// (`lauf.sitzung_belegt`) is never recorded, so it is never removed.
    sitzung: Option<PathBuf>,
    /// The PID of the watcher, once it runs.
    wache: Option<u32>,
    /// The PID of the holding process of the report, once it runs ([`crate::anfordern`]); it ends with this process.
    halter: Option<u32>,
    /// The old key of a conversation this run renamed to the new session (step 8): a fault before the exec renames it
    /// back.
    umbenannt: Option<String>,
}

impl Kinder {
    fn beenden(&mut self) {
        if let Some(k) = self.modellweg.take() {
            k.beenden();
        }
        if let Some(k) = self.suche.take() {
            k.beenden();
        }
    }
}

/// The whole program. `args` are the arguments without the program name. Returns only with a fault: on success the
/// process stays the parent of the starter and ends with it (contract 1, "Lebensdauer").
#[must_use]
pub fn lauf(args: &[OsString], umgebung: &Umgebung) -> Fehler {
    // step 0, before everything else
    if crate::tls_gesetzt(&umgebung.namen) {
        return Fehler::TlsUmgebung;
    }
    // step 1: the arguments, before any file or connection
    let (meldung_pfad, startdatei) = match aufruf_lesen(args) {
        Ok(a) => a,
        Err(f) => return f,
    };
    // descriptor 3 (and with a report descriptor 4) is reserved before the first file is opened
    let mut platz = Platz::reservieren();
    let mut meldung = match meldung_pfad {
        None => Meldung::default(),
        Some(p) => {
            let vier = Meldung::reservieren();
            // the first step after the arguments; without a connection only the line on standard error remains
            match Meldung::verbinden(p, vier) {
                Ok(m) => m,
                Err(f) => return f,
            }
        }
    };
    let fehler = if meldung.aktiv() && !meldung.vier_da() {
        Fehler::MeldungFehlt
    } else {
        pruefen_und_starten(startdatei, umgebung, &mut platz, &mut meldung)
    };
    // a start that reached the starter has ended this process already; every fault here is one before the start
    meldung.melden(&fehler);
    fehler
}

/// Step 1: `<startdatei>` or `--meldung <pfad> <startdatei>` with an absolute `<pfad>` (the form the core service
/// uses). Returns the path of the socket of the service, if given, and the start file.
///
/// # Errors
/// `lauf.aufruf` for every other form: the option after the start file, a relative path, the option without a path
/// (also alone).
pub fn aufruf_lesen(args: &[OsString]) -> Result<(Option<&Path>, &OsString), Fehler> {
    match args {
        [startdatei] if startdatei != MELDUNG_OPTION => Ok((None, startdatei)),
        [option, pfad, startdatei] if option == MELDUNG_OPTION && Path::new(pfad).is_absolute() => {
            Ok((Some(Path::new(pfad)), startdatei))
        }
        _ => Err(Fehler::Aufruf),
    }
}

/// The option of the report (contract 1, "Meldung an den Dienst").
pub const MELDUNG_OPTION: &str = "--meldung";

/// Steps 2 to 8 and the clean-up after a fault.
fn pruefen_und_starten(
    startdatei: &OsString,
    umgebung: &Umgebung,
    platz: &mut Platz,
    meldung: &mut Meldung,
) -> Fehler {
    let lage = match pruefen(std::slice::from_ref(startdatei)) {
        Ok(lage) => lage,
        Err(f) => return f,
    };
    let mut kinder = Kinder::default();
    let fehler = starten(&lage, umgebung, platz, meldung, &mut kinder);
    if let Some(alt) = kinder.umbenannt.take() {
        // first of all: the conversation goes back to its old session, best effort and never over another one
        gespraech::zurueck(&lage.konfig.gespraeche, lage.start.sitzung(), &alt);
    }
    kinder.beenden();
    // after the children, which may still read the files
    kinder.spuren.loeschen();
    if let Some(o) = kinder.ordner.take() {
        // best effort, like the files: a fault of the removal changes nothing about the code
        let _ = std::fs::remove_dir_all(o);
    }
    if let Some(o) = kinder.sitzung.take() {
        // by name and through descriptors, never through a link; best effort as well
        let _ = dateien::ordner_entfernen(&o);
    }
    fehler
}

/// Steps 1 to 5b with the configuration of this build.
fn pruefen(args: &[OsString]) -> Result<Lage, Fehler> {
    pruefen_mit(args, crate::konfig::pfad, crate::ziele::vorgabe::lesen)
}

/// Steps 1 to 5d. `konfig_pfad` names the configuration; it is asked for at step 3, after the start file was read. The
/// unit tests pass a configuration of their own (the delivered build reads a fixed path below `/etc`). `vorgabe` reads
/// the default of the firm at step 4b, for every route; the unit tests pass a default of their own. Step 5d comes after
/// steps 4b and 5c.
fn pruefen_mit(
    args: &[OsString],
    konfig_pfad: impl FnOnce() -> Option<PathBuf>,
    vorgabe: impl FnOnce() -> Result<Einstellungen, Fehler>,
) -> Result<Lage, Fehler> {
    // step 1
    let [pfad] = args else {
        return Err(Fehler::Aufruf);
    };
    let pfad = Path::new(pfad);
    // step 2
    let bytes = dateien::startdatei_lesen(pfad)?;
    // step 3
    let konfig = crate::konfig::lesen_datei(&konfig_pfad().ok_or(Fehler::KonfigFehlt)?)?;
    let einst = einstellungen_lesen(&konfig.einstellungen)?;
    // step 4
    let start = start::lesen_datei(&bytes).map_err(Fehler::Start)?;
    let art = start::pruefen(&start, &einst).map_err(Fehler::Start)?;
    let route = einst.route(start.route()).ok_or(Fehler::Intern)?;
    // step 4a: the settings give a local route both or neither (stage 18 of contract 2), so one check of the pair
    if art == Routenart::Lokal && route.ziel().zip(route.modell()).is_none() {
        return Err(Fehler::ZielFehlt);
    }
    // step 4b: the route table only from the default of the firm; the user may strike, never add or change (A45)
    crate::ziele::vorgabe::pruefen(&vorgabe()?, &einst)?;
    // step 5
    let arbeit = Path::new(start.arbeitsordner());
    if !arbeit.is_dir() {
        return Err(Fehler::OrdnerFehlt);
    }
    let ordner = std::fs::canonicalize(arbeit).map_err(|_| Fehler::OrdnerFehlt)?;
    // step 5a
    if let Some(m) = start.marker() {
        dateien::zaehlordner_pruefen(Path::new(m.ordner()))?;
    }
    // step 5b
    let ordner_text = ordner
        .to_str()
        .filter(|t| crate::ordner_form(t))
        .ok_or(Fehler::OrdnerMarkeKaputt)?;
    marke::pruefen_und_markieren(
        &konfig.zustand,
        marke::Anfrage {
            ordner: ordner_text,
            kunde: start.kunde(),
            klasse: route.klasse(),
        },
        einst.klassen(),
        marke::SPERR_FRIST,
    )?;
    let startordner = startordner(pfad).ok_or(Fehler::Intern)?;
    // step 5c
    startordner_frei(&startordner)?;
    // step 5d: before the enclosure and before any child; a fault never becomes a new start without the conversation
    let fortsetzbar = harness::fortsetzen_optionen(start.harness(), "").is_some()
        && !konfig.ohne_fortsetzen.iter().any(|h| h == start.harness());
    let fortsetzung = gespraech::pruefen(&konfig.gespraeche, &start, fortsetzbar)?;
    Ok(Lage {
        start,
        einst,
        konfig,
        startordner,
        ordner,
        fortsetzung,
    })
}

pub(crate) fn einstellungen_lesen(pfad: &Path) -> Result<Einstellungen, Fehler> {
    use std::io::Read;
    let datei = std::fs::File::open(pfad).map_err(|_| Fehler::EinstUnlesbar)?;
    let mut text = Vec::new();
    datei
        .take(EINST_LESEN_MAX)
        .read_to_end(&mut text)
        .map_err(|_| Fehler::EinstUnlesbar)?;
    einstellungen::lesen(&text).map_err(Fehler::Einst)
}

/// The two files every start writes next to the start file must not exist yet: the program never writes over a file, and
/// a folder that several sessions share (the home folder, say) may still hold the files of an earlier run. The files of
/// a cloud route are not checked here: an existing one ends the start at step 7a with the cleanup of the contract.
fn startordner_frei(ordner: &Path) -> Result<(), Fehler> {
    for name in SESSIONSDATEIEN {
        match std::fs::symlink_metadata(ordner.join(name)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(Fehler::StartordnerBelegt),
        }
    }
    Ok(())
}

/// The names of the PID files next to the start file.
const SESSIONSDATEIEN: [&str; 2] = ["suche.pid", "modellweg.pid"];

fn startordner(pfad: &Path) -> Option<PathBuf> {
    let eltern = match pfad.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::canonicalize(eltern).ok()
}

/// The bundle of the enclosure: the host bundle with the certificate of the CA appended, on a line of its own.
#[must_use]
pub fn bundle_inhalt(mut wirt: Vec<u8>, ca: &[u8]) -> Vec<u8> {
    if !wirt.is_empty() && !wirt.ends_with(b"\n") {
        wirt.push(b'\n');
    }
    wirt.extend_from_slice(ca);
    wirt
}

/// `TERM` of the harness: the own `TERM` when it is one word of printable ASCII, else `dumb`.
#[must_use]
pub fn term_wert(eigener: Option<String>) -> String {
    eigener
        .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_graphic()))
        .unwrap_or_else(|| "dumb".to_owned())
}

/// Steps 6 to 8.
fn starten(
    lage: &Lage,
    umgebung: &Umgebung,
    platz: &mut Platz,
    meldung: &mut Meldung,
    kinder: &mut Kinder,
) -> Fehler {
    let route = match lage.route() {
        Ok(r) => r,
        Err(f) => return f,
    };
    // step 6; descriptor 5 of the lifeline is the place's own too (without it the start ends at step 8, before the
    // trampoline: `Platz::leine_legen`)
    let eigene = [meldung.eigene(), platz.eigene()].concat();
    let verteilung = match einschluss_pruefen(
        lage,
        route,
        &einschluss_programm(&lage.konfig),
        platz,
        &eigene,
    ) {
        Ok(v) => v,
        Err(f) => return f,
    };
    // step 6a, before the first child
    let paket = match paket_bereitstellen(lage, umgebung, platz) {
        Ok(p) => p,
        Err(f) => return f,
    };
    // step 7
    let sockel = match socket_ordner(lage) {
        Ok(s) => s,
        Err(f) => return f,
    };
    kinder.ordner = Some(sockel.clone());
    // the fork condition, once before both children: between the two starts no thread begins
    if let Err(f) = einthreadig_oder_intern() {
        return f;
    }
    match such_starten(lage, &sockel) {
        Ok(k) => kinder.suche = Some(k),
        Err(f) => return f,
    }
    // step 7a
    let modell_socket = sockel.join("modell.sock");
    let cloud = match modellweg_starten(lage, route, &modell_socket, &mut kinder.spuren) {
        Ok((k, cloud)) => {
            kinder.modellweg = Some(k);
            cloud
        }
        Err(f) => return f,
    };
    // step 8
    if let Err(f) = pids_schreiben(lage, kinder) {
        return f;
    }
    // the socket of the worker request, only with a report: the holding process listens on it
    let anfordern = match anfordern_binden(&sockel, meldung.aktiv()) {
        Ok(a) => a,
        Err(f) => return f,
    };
    let mut beschreibung = match beschreibung_bauen(
        lage,
        umgebung,
        &sockel,
        cloud.as_ref(),
        verteilung,
        paket,
        kinder,
    ) {
        Ok(b) => b,
        Err(f) => return f,
    };
    beschreibung.anfordern_socket = anfordern.as_ref().map(|(p, _)| p.clone());
    // the watcher starts last: it removes the session folder, which exists and is this run's only from here on
    match wache_starten(lage, &sockel, kinder) {
        Ok(pid) => kinder.wache = Some(pid),
        Err(f) => return f,
    }
    // the last step before the hand-over: exactly one line with the excerpt of the argv, or no hand-over (fail closed)
    if let Err(f) = argv_protokollieren(lage, &beschreibung.argv, jetzt_ms()) {
        return f;
    }
    // the holding process takes the connection and the listener; from here on every line goes through its pipe
    if let Some((_, hoerer)) = anfordern {
        match meldung.halter_starten(&lage.konfig.programme.kind, hoerer) {
            Ok(pid) => kinder.halter = pid,
            Err(f) => return f,
        }
    }
    uebergeben(lage, platz, meldung, &beschreibung, kinder)
}

/// The source of the chain of the line `lauf.harness`: the chain of the model path of the session (contract 5, section
/// 6.9).
pub const PROTOKOLL_QUELLE: &str = "gateway";

/// Milliseconds since 1970 of the clock; `None` for a clock before 1970 or past `i64`.
fn jetzt_ms() -> Option<i64> {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    i64::try_from(d.as_millis()).ok()
}

/// Step 8, before the hand-over: appends the line `lauf.harness` (contract 5, section 6.9) to the chain `gateway` in the
/// log folder of the configuration. Standard error stays empty.
///
/// # Errors
/// `lauf.protokoll` for every fault of the clock or of `formate::kette::anhaengen_zeile` (fail closed).
fn argv_protokollieren(lage: &Lage, argv: &[String], zeit_ms: Option<i64>) -> Result<(), Fehler> {
    let zeit_ms = zeit_ms.ok_or(Fehler::Protokoll)?;
    let zeile = harness::zeile(
        lage.start.harness(),
        lage.start.modell(),
        start::effort(&lage.start, &lage.einst),
        lage.start.route(),
        lage.start.sitzung(),
        argv,
    );
    formate::kette::anhaengen_zeile(&lage.konfig.protokoll, PROTOKOLL_QUELLE, zeit_ms, &zeile)
        .map(|_| ())
        .map_err(|_| Fehler::Protokoll)
}

/// The children this process keeps past the hand-over (search process, model path, watcher): the parent of the starter
/// leaves them to the watcher. Their pipes are closed here, as the `exec` closed them before.
fn bekannte(kinder: &mut Kinder) -> Vec<u32> {
    let mut b = Vec::new();
    for k in [kinder.suche.as_mut(), kinder.modellweg.as_mut()]
        .into_iter()
        .flatten()
    {
        k.ausgabe_schliessen();
        b.push(k.pid());
    }
    b.extend(kinder.wache);
    b.extend(kinder.halter);
    b
}

/// The name of the socket of the worker request in the socket folder of the session (no longer than `modell.sock`, so
/// the limit of [`socket_ordner`] holds for it too).
pub const ANFORDERN_SOCKET: &str = "anfr.sock";

/// Binds the listening socket of the worker request in the socket folder (mode 0600 in the folder 0700), when the
/// program reports to the service; without a report there is none.
///
/// # Errors
/// `lauf.intern` when the socket cannot be bound or its mode cannot be set.
fn anfordern_binden(
    sockel: &Path,
    melden: bool,
) -> Result<Option<(PathBuf, std::os::fd::OwnedFd)>, Fehler> {
    use std::os::unix::fs::PermissionsExt;
    if !melden {
        return Ok(None);
    }
    let pfad = sockel.join(ANFORDERN_SOCKET);
    let hoerer = std::os::unix::net::UnixListener::bind(&pfad).map_err(|_| Fehler::Intern)?;
    std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| Fehler::Intern)?;
    Ok(Some((pfad, std::os::fd::OwnedFd::from(hoerer))))
}

/// The hand-over of step 8: the delivered build starts the starter, the test build `bwrap` of the configuration. One
/// function with a `cfg` on its body: cargo-mutants reads the text and would keep the mutants of a second function that
/// the run without (or with) the feature never builds.
fn uebergeben(
    lage: &Lage,
    platz: &mut Platz,
    meldung: &mut Meldung,
    b: &einschluss::Beschreibung,
    kinder: &mut Kinder,
) -> Fehler {
    let bekannt = bekannte(kinder);
    #[cfg(not(feature = "pruefbau"))]
    let fehler = uebergabe::starter_halten(
        &lage.konfig.programme.kind,
        Path::new(einschluss::starter::STARTER_PFAD),
        platz,
        meldung,
        b,
        &bekannt,
    );
    #[cfg(feature = "pruefbau")]
    let fehler = uebergabe::bwrap_halten(
        &lage.konfig.programme.kind,
        lage.konfig.bwrap.as_deref(),
        platz,
        meldung,
        b,
        &bekannt,
    );
    fehler
}

/// Step 6: the platform has the enclosure, the starter (or in the test build `bwrap`) is an executable file, the
/// process holds descriptor 3 and no inherited descriptor (`eigene`: the descriptors of the report, its own), and a
/// cloud route knows the place of the system bundle.
fn einschluss_pruefen(
    lage: &Lage,
    route: &Route,
    programm: &Path,
    platz: &Platz,
    eigene: &[i32],
) -> Result<Option<einschluss::Verteilung>, Fehler> {
    if !cfg!(target_os = "linux") {
        return Err(Fehler::EinschlussFehlt);
    }
    if !ausfuehrbar(programm) {
        return Err(Fehler::EinschlussFehlt);
    }
    if !uebergabe::platz_gut(
        cfg!(feature = "pruefbau"),
        platz.da(),
        uebergabe::drei_belegt(),
    ) {
        return Err(Fehler::EinschlussFehlt);
    }
    if uebergabe::fremde_deskriptoren().is_none_or(|f| f.iter().any(|n| !eigene.contains(n))) {
        return Err(Fehler::EinschlussFehlt);
    }
    if route.art() == Routenart::Cloud && lage.start.harness() == einschluss::HARNESS_COPILOT {
        return einschluss::Verteilung::vom_host()
            .map(Some)
            .map_err(|_| Fehler::EinschlussFehlt);
    }
    Ok(None)
}

/// The program of step 6: the starter, in the test build `bwrap` (of the configuration, if it names one).
fn einschluss_programm(k: &Konfig) -> PathBuf {
    if cfg!(feature = "pruefbau") {
        k.bwrap
            .clone()
            .unwrap_or_else(|| PathBuf::from(einschluss::BWRAP_PFAD))
    } else {
        PathBuf::from(einschluss::starter::STARTER_PFAD)
    }
}

fn ausfuehrbar(pfad: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(pfad).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Step 6a (only for the harness `copilot`): the complete package folder of the harness in the cache folder of the user,
/// extracted in the small enclosure when it is missing or incomplete (see [`crate::paket`]). `None` for every other
/// harness.
fn paket_bereitstellen(
    lage: &Lage,
    umgebung: &Umgebung,
    platz: &mut Platz,
) -> Result<Option<einschluss::Paket>, Fehler> {
    if lage.start.harness() != einschluss::HARNESS_COPILOT {
        return Ok(None);
    }
    let k = &lage.konfig;
    // a missing program stays the fault of step 8 (`lauf.harness_fehlt`, after the children): nothing to extract
    let Some(programm) = k
        .harness
        .get(einschluss::HARNESS_COPILOT)
        .filter(|p| ausfuehrbar(p))
    else {
        return Ok(None);
    };
    let form = crate::paket::form_lesen(programm, crate::paket::eigene_architektur())?;
    let cache = crate::paket::cache_ordner(&k.heim, umgebung.xdg_cache.as_ref());
    let v = einschluss::Vorentpacken {
        heim: k.heim.clone(),
        cache: cache.clone(),
        programmordner: k.programmordner.clone(),
        programm: programm.clone(),
        path: k.path.clone(),
        system: einschluss::System::vom_host()
            .unwrap_or_else(|_| einschluss::System::usr_merge(rustix::process::geteuid().as_raw())),
    };
    let (quelle, _tat) =
        crate::paket::bereitstellen(&cache, &k.heim, &form, || vorentpacken(lage, platz, &v))?;
    Ok(Some(einschluss::Paket {
        quelle,
        plattform: form.plattform,
        fassung: form.fassung,
    }))
}

/// The extraction of step 6a. The delivered build: the starter with the call `vorentpacken` and the text form on
/// descriptor 3, as a child. The test build: `bwrap` of the configuration directly, with the same checked arguments.
/// One function with a `cfg` on its body, for the same reason as [`uebergeben`].
fn vorentpacken(
    lage: &Lage,
    platz: &mut Platz,
    v: &einschluss::Vorentpacken,
) -> Result<crate::paket::Ende, Fehler> {
    #[cfg(not(feature = "pruefbau"))]
    let befehl = {
        let _ = lage;
        uebergabe::auf_drei_legen(platz, einschluss::vorentpacken_schreiben(v).as_bytes())
            .map_err(|_| Fehler::PaketEntpacken {
                rc: None,
                wort: "start",
            })?;
        einschluss::starter::vorentpacken_befehl(Path::new(einschluss::starter::STARTER_PFAD))
    };
    #[cfg(feature = "pruefbau")]
    let befehl = {
        let _ = platz;
        let args = v.argumente().map_err(|_| Fehler::PaketEntpacken {
            rc: None,
            wort: "start",
        })?;
        let bwrap = lage
            .konfig
            .bwrap
            .clone()
            .unwrap_or_else(|| PathBuf::from(einschluss::BWRAP_PFAD));
        let mut befehl = std::process::Command::new(bwrap);
        befehl.args(args).env_clear();
        befehl
    };
    crate::paket::lauf_messen(befehl, crate::paket::ENTPACK_FRIST)
}

/// Step 7, first part: the socket folder 0700 of the session, outside the enclosure, with a short path.
fn socket_ordner(lage: &Lage) -> Result<PathBuf, Fehler> {
    let ordner = lage.konfig.laufzeit.join(lage.start.sitzung());
    let laengster = ordner.join("modell.sock");
    if laengster.as_os_str().len() > SOCKET_MAX {
        return Err(Fehler::SucheFehlt);
    }
    dateien::ordner_neu(&ordner).map_err(|_| Fehler::SucheFehlt)?;
    Ok(ordner)
}

/// The three options of a trial run, or none.
fn marker_optionen(start: &Startdatei, endung: &str) -> Vec<String> {
    match start.marker() {
        Some(m) => vec![
            "--probelauf".to_owned(),
            "--marker".to_owned(),
            m.liste().join(","),
            "--marker-zaehlung".to_owned(),
            format!("{}/{}.{endung}.json", m.ordner(), start.sitzung()),
        ],
        None => Vec::new(),
    }
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The arguments of the search process (contract 4, "Aufruf").
#[must_use]
fn such_argumente(lage: &Lage, socket: &Path) -> Vec<String> {
    let k = &lage.konfig;
    let mut a = vec![
        "sitzung".to_owned(),
        text(&k.vault),
        "--socket".to_owned(),
        text(socket),
        "--route".to_owned(),
        lage.start.route().to_owned(),
        "--sitzung".to_owned(),
        lage.start.sitzung().to_owned(),
        "--einstellungen".to_owned(),
        text(&k.einstellungen),
        "--protokoll".to_owned(),
        text(&k.protokoll),
        "--socket-rechte".to_owned(),
        "0600".to_owned(),
    ];
    a.extend(marker_optionen(&lage.start, "suche"));
    a
}

fn einthreadig_oder_intern() -> Result<(), Fehler> {
    if kind::einthreadig() {
        Ok(())
    } else {
        Err(Fehler::Intern)
    }
}

fn such_starten(lage: &Lage, sockel: &Path) -> Result<Kind, Fehler> {
    let args = such_argumente(lage, &sockel.join("such.sock"));
    kind::starten(
        &lage.konfig.programme.kind,
        &lage.konfig.programme.rangkern,
        &args,
        BEREIT_FRIST,
    )
    .map_err(|a| match a {
        Ausfall::Fehlt => Fehler::SucheFehlt,
        // the code of a missing holder belongs to the model path; from the search process it is just no readiness
        Ausfall::NichtBereit | Ausfall::HalterFehlt => Fehler::SucheNichtBereit,
    })
}

/// The base address of the local route, as `--ziel` of the model path takes it.
#[must_use]
pub fn ziel_text(ziel: &einstellungen::Ziel) -> String {
    let schema = match ziel.schema() {
        Schema::Http => "http",
        Schema::Https => "https",
    };
    format!("{schema}://{}:{}{}", ziel.host(), ziel.port(), ziel.pfad())
}

/// The parts of the cloud exit that step 8 binds: the bundle of the session.
struct CloudTeile {
    bundle: PathBuf,
}

/// The arguments every mode shares (contract 8, section 4).
fn modellweg_basis(lage: &Lage, betriebsart: &str, socket: &Path) -> Vec<String> {
    vec![
        "--betriebsart".to_owned(),
        betriebsart.to_owned(),
        "--socket".to_owned(),
        text(socket),
        "--socket-rechte".to_owned(),
        "0600".to_owned(),
        "--sitzung".to_owned(),
        lage.start.sitzung().to_owned(),
        "--route".to_owned(),
        lage.start.route().to_owned(),
        "--protokoll".to_owned(),
        text(&lage.konfig.protokoll),
    ]
}

/// Step 7a: mode `openai-lokal` for the local route, mode `kopf-einsetzen` with the target list and the throw-away CA
/// for the cloud route.
fn modellweg_starten(
    lage: &Lage,
    route: &Route,
    socket: &Path,
    spuren: &mut Spuren,
) -> Result<(Kind, Option<CloudTeile>), Fehler> {
    let (mut args, cloud) = match route.art() {
        Routenart::Lokal => {
            let (Some(ziel), Some(modell)) = (route.ziel(), route.modell()) else {
                return Err(Fehler::ZielFehlt);
            };
            let schluessel = lage
                .konfig
                .schluessel
                .get(route.schluessel())
                .ok_or(Fehler::ModellwegFehlt)?;
            let mut a = modellweg_basis(lage, "openai-lokal", socket);
            a.extend([
                "--ziel".to_owned(),
                ziel_text(ziel),
                "--modell".to_owned(),
                modell.to_owned(),
                "--schluessel-datei".to_owned(),
                text(schluessel),
            ]);
            (a, None)
        }
        Routenart::Cloud => cloud_vorbereiten(lage, route, socket, spuren)?,
        // a kind of route this program does not know has no model path (fail closed)
        _ => return Err(Fehler::ModellwegFehlt),
    };
    args.extend(marker_optionen(&lage.start, "modellweg"));
    let k = kind::starten(
        &lage.konfig.programme.kind,
        &lage.konfig.programme.modellweg,
        &args,
        BEREIT_FRIST,
    )
    .map_err(|a| match a {
        Ausfall::Fehlt => Fehler::ModellwegFehlt,
        Ausfall::NichtBereit => Fehler::ModellwegNichtBereit,
        Ausfall::HalterFehlt => Fehler::HalterFehlt,
    })?;
    Ok((k, cloud))
}

/// The session files of the cloud route next to the start file: the target list, the key and the certificate of the
/// CA and the bundle of the enclosure (the host bundle with the certificate of the CA appended).
///
/// One function with a `cfg` on its body (Linux only, like the CA), for the same reason as [`uebergeben`].
fn cloud_vorbereiten(
    lage: &Lage,
    route: &Route,
    socket: &Path,
    spuren: &mut Spuren,
) -> Result<(Vec<String>, Option<CloudTeile>), Fehler> {
    #[cfg(not(target_os = "linux"))]
    let ergebnis = {
        let _ = (lage, route, socket, spuren);
        Err(Fehler::EinschlussFehlt)
    };
    #[cfg(target_os = "linux")]
    let ergebnis = cloud_vorbereiten_linux(lage, route, socket, spuren);
    ergebnis
}

#[cfg(target_os = "linux")]
fn cloud_vorbereiten_linux(
    lage: &Lage,
    route: &Route,
    socket: &Path,
    spuren: &mut Spuren,
) -> Result<(Vec<String>, Option<CloudTeile>), Fehler> {
    let fehlt = |_| Fehler::ModellwegFehlt;
    // a target in the subdomain form cannot go into the list: no silent start without it
    if crate::ziele::hat_unterdomaene(route) {
        return Err(Fehler::Zielliste);
    }
    let ziele = crate::ziele::ziele(route);
    // no target or more than the model path takes: no list and no CA (a CA without a DNS name in its constraints would
    // allow every name)
    if ziele.is_empty() || ziele.len() > crate::ziele::ZIELE_MAX {
        return Err(Fehler::Zielliste);
    }
    let gebunden = ziele.iter().any(|z| z.gebunden);
    // The list names no key file any more: the key of a bound target lies with the holder. The route of a bound
    // target still has to carry one in the configuration, so a route without it ends the start as before.
    if gebunden && !lage.konfig.schluessel.contains_key(route.schluessel()) {
        return Err(Fehler::ModellwegFehlt);
    }
    // step 7a0: the target list in the state folder, before the model path starts; it stays after the session
    let sitzungen = lage.konfig.zustand.join("sitzungen");
    if !dateien::eigener_ordner_0700(&sitzungen) {
        return Err(Fehler::Zielliste);
    }
    let liste = sitzungen.join(format!("{}.zielliste.json", lage.start.sitzung()));
    dateien::datei_neu(
        &liste,
        crate::ziele::text(&ziele, &lage.konfig.ca_zulassung).as_bytes(),
    )
    .map_err(|_| Fehler::Zielliste)?;
    let hosts: Vec<&str> = ziele.iter().map(|z| z.host.as_str()).collect();
    let jetzt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Fehler::ModellwegFehlt)?
        .as_secs();
    let jetzt = i64::try_from(jetzt).map_err(|_| Fehler::ModellwegFehlt)?;
    let ca = crate::ca::erzeugen(&hosts, lage.start.sitzung(), jetzt)
        .map_err(|_| Fehler::ModellwegFehlt)?;
    let ca_schluessel = lage.startordner.join("ca.key");
    let ca_zertifikat = lage.startordner.join("ca.crt");
    spuren
        .schreiben(&ca_schluessel, &ca.schluessel)
        .map_err(fehlt)?;
    spuren
        .schreiben(&ca_zertifikat, &ca.zertifikat)
        .map_err(fehlt)?;
    let cloud = if lage.start.harness() == einschluss::HARNESS_COPILOT {
        let verteilung = einschluss::Verteilung::vom_host().map_err(|_| Fehler::EinschlussFehlt)?;
        let ort = verteilung
            .bundle_orte()
            .first()
            .ok_or(Fehler::EinschlussFehlt)?;
        let inhalt = bundle_inhalt(std::fs::read(ort).map_err(fehlt)?, &ca.zertifikat);
        let bundle = lage.startordner.join("ca-bundle.pem");
        spuren.schreiben(&bundle, &inhalt).map_err(fehlt)?;
        Some(CloudTeile { bundle })
    } else {
        None
    };
    let mut a = modellweg_basis(lage, "kopf-einsetzen", socket);
    a.extend([
        "--zielliste".to_owned(),
        text(&liste),
        "--ca-schluessel".to_owned(),
        text(&ca_schluessel),
        "--ca-zertifikat".to_owned(),
        text(&ca_zertifikat),
    ]);
    Ok((a, cloud))
}

/// Step 8, last part: the watcher (contract 1, "Lebensdauer", point 3). It outlives this process, which becomes the
/// starter and then the harness: when that ends, also when the starter refuses, the watcher ends both children and
/// removes the session folder and the socket folder. The files next to the start file stay for the core service.
fn wache_starten(lage: &Lage, sockel: &Path, kinder: &Kinder) -> Result<u32, Fehler> {
    let (Some(suche), Some(modellweg)) = (kinder.suche.as_ref(), kinder.modellweg.as_ref()) else {
        return Err(Fehler::Intern);
    };
    let sitzung = kinder.sitzung.as_deref().ok_or(Fehler::Intern)?;
    kind::wache_starten(
        &lage.konfig.programme.kind,
        [suche, modellweg],
        sockel,
        sitzung,
    )
    .map_err(|_| Fehler::Intern)
}

/// Step 8, first part: `suche.pid` and `modellweg.pid` next to the start file (decimal and LF, mode 0600).
fn pids_schreiben(lage: &Lage, kinder: &mut Kinder) -> Result<(), Fehler> {
    for (name, pid) in [
        ("suche.pid", kinder.suche.as_ref().map(Kind::pid)),
        ("modellweg.pid", kinder.modellweg.as_ref().map(Kind::pid)),
    ] {
        let pid = pid.ok_or(Fehler::Intern)?;
        kinder
            .spuren
            .schreiben(&lage.startordner.join(name), format!("{pid}\n").as_bytes())
            .map_err(|_| Fehler::Intern)?;
    }
    Ok(())
}

/// The folders the enclosure must not meet: vault, log, the folder of the start file, state folder, the folder of the
/// conversations (the state folder of the harness may lie below it, never be it), run folder, the
/// counting folder of a trial run and, whatever the installation does, the folder of every key file and the folder of
/// the settings file. Each folder once; the root is never one of them.
#[must_use]
pub fn geschuetzte_ordner(
    k: &Konfig,
    startordner: &Path,
    zaehlordner: Option<&Path>,
) -> Vec<PathBuf> {
    let mut liste = vec![
        k.vault.clone(),
        k.protokoll.clone(),
        startordner.to_path_buf(),
        k.zustand.clone(),
        k.gespraeche.clone(),
        k.laufzeit.clone(),
    ];
    liste.extend(zaehlordner.map(Path::to_path_buf));
    let mut dateiordner: Vec<PathBuf> = k
        .schluessel
        .values()
        .map(PathBuf::as_path)
        .chain(std::iter::once(k.einstellungen.as_path()))
        .filter_map(Path::parent)
        .filter(|o| o.parent().is_some())
        .map(Path::to_path_buf)
        .collect();
    dateiordner.sort();
    for o in dateiordner {
        if !liste.contains(&o) {
            liste.push(o);
        }
    }
    liste
}

/// Step 8, second part: the session folders (folder of the configuration file 0700 with the file 0600), the
/// conversation ([`gespraech_bereitstellen`]) and the typed description of the enclosure.
fn beschreibung_bauen(
    lage: &Lage,
    umgebung: &Umgebung,
    sockel: &Path,
    cloud: Option<&CloudTeile>,
    verteilung: Option<einschluss::Verteilung>,
    paket: Option<einschluss::Paket>,
    kinder: &mut Kinder,
) -> Result<einschluss::Beschreibung, Fehler> {
    let k = &lage.konfig;
    let harness_name = lage.start.harness();
    let programm = k.harness.get(harness_name).ok_or(Fehler::HarnessFehlt)?;
    if !ausfuehrbar(programm) {
        return Err(Fehler::HarnessFehlt);
    }
    let sitzung = k.sitzungen.join(lage.start.sitzung());
    let konfig_ordner = sitzung.join("konfig");
    let intern = |_| Fehler::Intern;
    // the folder of a session is made once: a session that ran before (or was refused after this step) has it already
    dateien::ordner_neu(&sitzung).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => Fehler::SitzungBelegt,
        _ => Fehler::Intern,
    })?;
    // recorded as soon as it exists, so a later fault removes it again (and a refused name never)
    kinder.sitzung = Some(sitzung.clone());
    let mut dateien_liste = Vec::new();
    if let (Some(ziel), Some(inhalt)) = (
        harness::datei_ziel(harness_name),
        harness::datei_text(
            harness_name,
            lage.start.modell(),
            &k.programme.such_mcp,
            start::effort(&lage.start, &lage.einst),
        ),
    ) {
        dateien::ordner_neu(&konfig_ordner).map_err(intern)?;
        let name = Path::new(ziel)
            .file_name()
            .ok_or(Fehler::Intern)?
            .to_owned();
        let quelle = konfig_ordner.join(name);
        dateien::datei_neu(&quelle, inhalt.as_bytes()).map_err(intern)?;
        dateien_liste.push(einschluss::Datei {
            quelle,
            ziel: k.heim.join(ziel),
        });
    }
    let modell_socket = sockel.join("modell.sock");
    let lokal = lage.route()?.art() == Routenart::Lokal;
    let cloud_teil = match (cloud, verteilung) {
        (Some(c), Some(v)) => Some(einschluss::Cloud {
            weiterreicher: modell_socket.clone(),
            weiter_programm: k.programme.weiter.clone(),
            bundle: c.bundle.clone(),
            verteilung: v,
        }),
        _ => None,
    };
    let term = term_wert(umgebung.term.clone());
    let geschuetzt = geschuetzte_ordner(
        k,
        &lage.startordner,
        lage.start.marker().map(|m| Path::new(m.ordner())),
    );
    let system = einschluss::System::vom_host()
        .unwrap_or_else(|_| einschluss::System::usr_merge(rustix::process::geteuid().as_raw()));
    // the last thing before the description: a conversation is never made for a start that fails before this point
    let (zustand, gespraech_ordner, sitzungsoption) =
        gespraech_bereitstellen(lage, &sitzung, &mut kinder.umbenannt)?;
    Ok(einschluss::Beschreibung {
        harness: harness_name.to_owned(),
        arbeitsordner: lage.ordner.clone(),
        programmordner: k.programmordner.clone(),
        zustand: einschluss::Zustand {
            host: zustand,
            name: harness::zustand_name(harness_name).to_owned(),
            fortsetzen: lage.fortsetzung.is_some(),
            gespraech: gespraech_ordner,
        },
        heim: k.heim.clone(),
        such_socket: sockel.join("such.sock"),
        modell_socket: cloud_teil.is_none().then(|| modell_socket.clone()),
        anfordern_socket: None,
        lokal_weiter: lokal.then(|| k.programme.weiter.clone()),
        cloud: cloud_teil,
        paket,
        dateien: dateien_liste,
        umgebung: einschluss::Umgebung {
            home: text(&k.heim),
            path: k.path.clone(),
            term,
            lang: "C.UTF-8".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt,
        argv: harness::argv(
            harness_name,
            programm,
            lage.start.modell(),
            &k.heim,
            lage.einst.copilot_ohne_rueckfrage(),
            start::effort(&lage.start, &lage.einst),
            &sitzungsoption,
        ),
        system,
    })
}

/// Step 8, the conversation (contract 1, "Ablage des Gesprächs"). Returns the state folder of the harness, the folder of
/// the conversation and the session option of the start form.
///
/// The state folder of the harness is always the empty `zustand` in the session folder, which the watcher removes at
/// the end: what a session writes there as a setting never reaches a later one (contract 1, "Gespräch und
/// Einstellungen im Zustandsordner", OPEN 34). Only the folder of the conversation passes on; the enclosure binds it at
/// the place of the conversation of the harness inside the state folder.
/// - Continuing: renames `gespraeche/<fortsetzung_von>` to `gespraeche/<sitzung>` without replacing (and records the
///   old key in `umbenannt`, so a later fault renames it back); the option to continue with the key of step 5d.
/// - New start of version 2: makes `gespraeche/<sitzung>` exclusively with `gespraech.json` (a new UUID v4) and the
///   empty folder of the conversation; `--session-id <uuid>`.
/// - New start of version 2 of a harness without an option to continue (`opencode`): the same folder with
///   `gespraech.json` without a key (`kennung` `null`) and the empty folder of the conversation; no session option.
/// - Start of version 1: makes `gespraeche/<sitzung>` with `gespraech.json` (`kennung` `null`) only; no folder of the
///   conversation and no session option.
fn gespraech_bereitstellen(
    lage: &Lage,
    sitzung_ordner: &Path,
    umbenannt: &mut Option<String>,
) -> Result<(PathBuf, Option<PathBuf>, Vec<String>), Fehler> {
    let k = &lage.konfig;
    let start = &lage.start;
    let neu = |kennung: Option<String>| gespraech::Gespraech {
        harness: start.harness().to_owned(),
        arbeitsordner: start.arbeitsordner().to_owned(),
        route: start.route().to_owned(),
        kunde: start.kunde().map(str::to_owned),
        kennung,
    };
    let zustand = sitzung_ordner.join("zustand");
    dateien::ordner_neu(&zustand).map_err(|_| Fehler::Intern)?;
    if let Some(f) = &lage.fortsetzung {
        let optionen =
            harness::fortsetzen_optionen(start.harness(), &f.kennung).ok_or(Fehler::Intern)?;
        let ordner = gespraech::uebernehmen(&k.gespraeche, &f.alt, start.sitzung())?;
        *umbenannt = Some(f.alt.clone());
        return Ok((zustand, Some(ordner.join(gespraech::ZUSTAND)), optionen));
    }
    if start.fassung() == 2 && harness::fortsetzen_optionen(start.harness(), "").is_none() {
        // a harness that takes no key from outside (opencode): its conversation is kept at its place, without a key and
        // without a session option, so it cannot be continued (5d.3a) and never restarts silently
        let ordner = gespraech::anlegen(&k.gespraeche, start.sitzung(), &neu(None), true)?;
        return Ok((zustand, Some(ordner.join(gespraech::ZUSTAND)), Vec::new()));
    }
    if start.fassung() == 2 {
        let kennung = gespraech::neue_kennung().ok_or(Fehler::Intern)?;
        let ordner = gespraech::anlegen(
            &k.gespraeche,
            start.sitzung(),
            &neu(Some(kennung.clone())),
            true,
        )?;
        return Ok((
            zustand,
            Some(ordner.join(gespraech::ZUSTAND)),
            harness::neu_optionen(&kennung),
        ));
    }
    gespraech::anlegen(&k.gespraeche, start.sitzung(), &neu(None), false)?;
    Ok((zustand, None, Vec::new()))
}

#[cfg(test)]
mod tests;
