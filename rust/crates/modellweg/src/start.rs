//! The start (contract 8, sections 3 and 4): the steps before readiness with their exit codes and stderr lines, the line
//! `{"bereit":true}`, the loop over connections and the end on SIGTERM or SIGINT.

use std::ffi::OsString;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustix::fs::Mode;
use signal_hook::iterator::Signals;

use crate::aufruf::{self, Aufruffehler};
#[cfg(target_os = "linux")]
use crate::betrieb::Betrieb;
use crate::betrieb::{Faden, Server, Uhr};
use crate::betriebsart::ArtOptionen;
use crate::betriebsart::openai_lokal::Optionen;
#[cfg(target_os = "linux")]
use crate::betriebsart::{kopf_einsetzen, zielliste};
use crate::dienst::{Dienst, Lage};
use crate::loeschen::{Eigene, Ordner};
use crate::os;
use crate::probelauf::Zaehlung;
use crate::protokoll::{self, Probelauf};
use crate::schluessel::{self, Schluesselfehler};
use crate::tls::{SystemTls, Tls};
use crate::ziel::{self, Aufloeser, Fristen, NetzVerbinder};

/// How long the end waits for the lines of running requests.
const ENDE_FRIST: Duration = Duration::from_secs(5);

/// What the end of this process needs: the socket of this process as soon as it exists and the service as soon as it
/// serves. A process has one socket and one service, so the place is a static: the thread of the signals reaches it in
/// every step of the start, without a hand-over through the steps. [`socket_binden_mit`] remembers the socket in the
/// same step as it makes it, so there is no moment in which a socket lies there and the end does not know it.
struct Ende {
    socket: Mutex<Option<Arc<Eigene>>>,
    dienst: Mutex<Option<Arc<dyn Server>>>,
}

/// The one place of this process.
static ENDE: Ende = Ende {
    socket: Mutex::new(None),
    dienst: Mutex::new(None),
};

impl Ende {
    /// Remembers the socket this process made; the end deletes it, before readiness too.
    fn socket_merken(&self, socket: &Arc<Eigene>) {
        *self.socket.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(socket));
    }

    /// Remembers the service; from here on the end tears running requests down first.
    fn dienst_merken(&self, dienst: &Arc<dyn Server>) {
        *self.dienst.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(dienst));
    }

    /// The end on SIGTERM or SIGINT (section 3): [`Ende::aufraeumen`], then exit 0. Before readiness there is no
    /// service and maybe no socket; the exit code stays 0.
    fn beenden(&self) -> ! {
        self.aufraeumen();
        std::process::exit(0)
    }

    /// The work of the end without the exit: tear every request down, wait for their lines, delete the socket of this
    /// process. Each part only where there is one, so an end before readiness works the same way.
    fn aufraeumen(&self) {
        let dienst = self
            .dienst
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(d) = dienst {
            d.betrieb().abreissen_alle();
            let _ = d.betrieb().warte_leer(ENDE_FRIST);
        }
        let socket = self
            .socket
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        loeschen_socket(socket.as_deref());
    }
}

/// A failure before readiness: code, fixed reason and exit code of section 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Startfehler {
    /// `mw.umgebung_unsicher`, exit 78 (stage 0: a variable of OpenSSL is set).
    UmgebungUnsicher,
    /// `mw.aufruf`, exit 64.
    Aufruf,
    /// `mw.betriebsart_unbekannt`, exit 64.
    BetriebsartUnbekannt,
    /// `mw.probelauf_aufruf`, exit 64.
    ProbelaufAufruf,
    /// `mw.schluessel_datei`, exit 78.
    SchluesselDatei,
    /// `mw.schluessel_form`, exit 78.
    SchluesselForm,
    /// `mw.protokoll_ordner`, exit 78.
    ProtokollOrdner,
    /// `mw.zaehlung_fehler`, exit 74 (trial run: the count file cannot be written).
    ZaehlungFehler,
    /// `mw.protokoll_fehlt`, exit 74 (trial run: the start line cannot be appended).
    ProtokollFehlt,
    /// `mw.umgebung_rest`, exit 78 (section 3a, duty 3: after the emptying a name of the environment is still there,
    /// or the emptying itself failed).
    UmgebungRest,
    /// `mw.ziele_ungueltig`, exit 78 (mode 2: the target list).
    ZieleUngueltig,
    /// `mw.ziel_halter_name`, exit 78 (mode 2, section 3a, duty 1: a `halter_ziel` is not the `host` of its target).
    ZielHalterName,
    /// `mw.ca_nicht_zugelassen`, exit 78 (mode 2: the company has not approved the local CA).
    CaNichtZugelassen,
    /// `mw.ca_ungueltig`, exit 78 (mode 2: the throw-away CA).
    CaUngueltig,
    /// `mw.halter_fehlt`, exit 78 (mode 2, step 3: the fixed endpoint of the holder does not answer).
    HalterFehlt,
    /// `mw.halter_unecht`, exit 78 (mode 2, step 3: the check of the counterparty fails).
    HalterUnecht,
    /// `mw.halter_kein_schluessel`, exit 78 (mode 2, step 3: the holder holds no secret for a bound target, or does not
    /// know the target).
    HalterKeinSchluessel,
    /// `mw.halter_fehler`, exit 78 (mode 2, step 3: any other code of the holder, an unreadable line or a torn
    /// connection in answer to `schluessel.vorhanden`).
    HalterFehler,
    /// `mw.halter_frist`, exit 78 (mode 2, step 3: no answer to `schluessel.vorhanden` within `FRIST_START`).
    HalterFrist,
    /// `mw.ziel_unaufloesbar`, exit 69.
    ZielUnaufloesbar,
    /// `mw.socket_belegt`, exit 74.
    SocketBelegt,
    /// `mw.intern`, exit 70.
    Intern,
    /// `mw.kind_verboten`, exit 70 (section 3a, duty 5: the process has a child).
    KindVerboten,
    /// `mw.kernabbild`, exit 5 (step 1: `kernabbild::abschirmen` refuses; default of the Werkbank, 21.11.2026).
    Kernabbild,
    /// `mw.eltern_weg`, exit 69 (step 1: the parent ended before the death signal was set, so no signal comes).
    ElternWeg,
}

impl Startfehler {
    /// The code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::UmgebungUnsicher => "mw.umgebung_unsicher",
            Self::Aufruf => "mw.aufruf",
            Self::BetriebsartUnbekannt => "mw.betriebsart_unbekannt",
            Self::ProbelaufAufruf => "mw.probelauf_aufruf",
            Self::ZaehlungFehler => "mw.zaehlung_fehler",
            Self::ProtokollFehlt => "mw.protokoll_fehlt",
            Self::SchluesselDatei => "mw.schluessel_datei",
            Self::SchluesselForm => "mw.schluessel_form",
            Self::ProtokollOrdner => "mw.protokoll_ordner",
            Self::UmgebungRest => "mw.umgebung_rest",
            Self::ZieleUngueltig => "mw.ziele_ungueltig",
            Self::ZielHalterName => "mw.ziel_halter_name",
            Self::CaNichtZugelassen => "mw.ca_nicht_zugelassen",
            Self::CaUngueltig => "mw.ca_ungueltig",
            Self::HalterFehlt => "mw.halter_fehlt",
            Self::HalterUnecht => "mw.halter_unecht",
            Self::HalterKeinSchluessel => "mw.halter_kein_schluessel",
            Self::HalterFehler => "mw.halter_fehler",
            Self::HalterFrist => "mw.halter_frist",
            Self::ZielUnaufloesbar => "mw.ziel_unaufloesbar",
            Self::SocketBelegt => "mw.socket_belegt",
            Self::Intern => "mw.intern",
            Self::KindVerboten => "mw.kind_verboten",
            Self::Kernabbild => "mw.kernabbild",
            Self::ElternWeg => "mw.eltern_weg",
        }
    }

    /// The fixed reason (ASCII).
    #[must_use]
    pub fn grund(self) -> &'static str {
        match self {
            Self::UmgebungUnsicher => "Umgebung unsicher",
            Self::Aufruf => "Aufruf ungueltig",
            Self::BetriebsartUnbekannt => "Betriebsart unbekannt oder nicht gebaut",
            Self::ProbelaufAufruf => "Probelauf-Optionen ungueltig",
            Self::ZaehlungFehler => "Zaehldatei nicht schreibbar",
            Self::ProtokollFehlt => "Protokoll nicht schreibbar",
            Self::SchluesselDatei => "Schluesseldatei nicht eigen, nicht 0600 oder keine Datei",
            Self::SchluesselForm => "Schluessel hat falsche Form",
            Self::ProtokollOrdner => "Protokollordner fehlt oder ist kein Ordner",
            Self::UmgebungRest => "Umgebung nicht geleert",
            Self::ZieleUngueltig => "Zielliste ungueltig",
            Self::ZielHalterName => "Halterziel nicht der Host",
            Self::CaNichtZugelassen => "Lokale CA nicht zugelassen",
            Self::CaUngueltig => "CA-Datei ungueltig",
            Self::HalterFehlt => "Halter nicht erreichbar",
            Self::HalterUnecht => "Halter nicht echt",
            Self::HalterKeinSchluessel => "kein Schluessel beim Halter",
            Self::HalterFehler => "Halter lehnte ab",
            Self::HalterFrist => "Halter antwortete nicht",
            Self::ZielUnaufloesbar => "Ziel nicht aufloesbar",
            Self::SocketBelegt => "Socket belegt oder nicht anlegbar",
            Self::Intern => "interner Fehler",
            Self::KindVerboten => "Kindprozess verboten",
            Self::Kernabbild => "Kernabbild nicht abschirmbar",
            Self::ElternWeg => "Elternprozess beendet",
        }
    }

    /// The exit code.
    #[must_use]
    pub fn exit(self) -> u8 {
        match self {
            Self::Aufruf | Self::BetriebsartUnbekannt | Self::ProbelaufAufruf => 64,
            Self::UmgebungUnsicher
            | Self::SchluesselDatei
            | Self::SchluesselForm
            | Self::ProtokollOrdner
            | Self::UmgebungRest
            | Self::ZieleUngueltig
            | Self::ZielHalterName
            | Self::CaNichtZugelassen
            | Self::CaUngueltig
            | Self::HalterFehlt
            | Self::HalterUnecht
            | Self::HalterKeinSchluessel
            | Self::HalterFehler
            | Self::HalterFrist => 78,
            Self::ZielUnaufloesbar | Self::ElternWeg => 69,
            Self::SocketBelegt | Self::ZaehlungFehler | Self::ProtokollFehlt => 74,
            Self::Intern | Self::KindVerboten => 70,
            Self::Kernabbild => 5,
        }
    }

    /// The line on stderr: `<code>: <grund>` and a line feed.
    #[must_use]
    pub fn zeile(self) -> String {
        format!("{}: {}\n", self.code(), self.grund())
    }
}

/// The state after step 5: the service and its socket, ready for the line `{"bereit":true}`.
pub struct Bereit {
    /// The service.
    pub dienst: Arc<dyn Server>,
    /// The listening socket.
    pub listener: UnixListener,
    /// Its path (deleted at the end).
    pub socket: PathBuf,
    /// Whether this is a trial run (section 9).
    pub probelauf: bool,
    /// The socket as this process made it (folder, kind, device, inode): the end deletes exactly that. `None` if it
    /// could not be remembered; then the socket stays.
    pub(crate) eigener_socket: Option<Arc<Eigene>>,
}

/// The variables of the environment that OpenSSL follows (section 3, F1); stage 0 refuses the start if one is set.
pub const UMGEBUNG_UNSICHER: [&str; 3] = ["SSL_CERT_FILE", "SSL_CERT_DIR", "OPENSSL_CONF"];

/// Stage 0 (section 3, F1): whether none of [`UMGEBUNG_UNSICHER`] is set, also not empty. `wert` reads one variable of
/// the own environment; neither name nor value goes anywhere else.
#[must_use]
pub fn umgebung_sicher(wert: impl Fn(&'static str) -> Option<OsString>) -> bool {
    UMGEBUNG_UNSICHER.iter().all(|n| wert(n).is_none())
}

/// argv\[0\] of the image after the emptying of duty 3 (section 3a): this prefix, a colon and the first reading of the
/// parent (`-` for none). The new image takes that reading as its own first one, so the check after the death signal
/// still compares against the parent before the `execve`.
///
/// A caller may set the mark himself; it gives him nothing: with a non-empty environment the start ends with
/// `mw.umgebung_rest`, and a wrong number only ends his own start with `mw.eltern_weg` or does not.
pub const GELEERT: &str = "wb-modellweg-geleert";

/// The mark of [`GELEERT`] for the reading `eltern`.
#[must_use]
pub fn marke(eltern: Option<i32>) -> OsString {
    match eltern {
        Some(p) => format!("{GELEERT}:{p}").into(),
        None => format!("{GELEERT}:-").into(),
    }
}

/// The reading of the parent in a mark of [`GELEERT`]; `None` if `name` is no such mark (then this image is the first
/// one).
#[must_use]
pub fn marke_lesen(name: &std::ffi::OsStr) -> Option<Option<i32>> {
    let rest = name.to_str()?.strip_prefix(GELEERT)?.strip_prefix(':')?;
    if rest == "-" {
        return Some(None);
    }
    let p: i32 = rest.parse().ok()?;
    (p > 0 && rest == p.to_string()).then_some(Some(p))
}

/// What duty 3 (section 3a) does after stage 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leeren {
    /// The environment is empty: go on.
    Weiter,
    /// The first image with names in the environment: replace the program by itself with an empty environment.
    Neu,
    /// The image after the emptying still has a name: `mw.umgebung_rest`.
    Rest,
}

/// Duty 3: `geleert` says whether this image came from the emptying (argv\[0\] is a mark), `leer` whether the own
/// environment is empty. The positive list of version 1 is empty, so every name is a rest.
#[must_use]
pub fn leeren(geleert: bool, leer: bool) -> Leeren {
    match (leer, geleert) {
        (true, _) => Leeren::Weiter,
        (false, false) => Leeren::Neu,
        (false, true) => Leeren::Rest,
    }
}

/// Duty 5 (section 3a): the process has no child. Read in two places, before the ready line and before each accepted
/// connection is served; a child ends the process with `mw.kind_verboten`, and files of the threads that cannot be
/// read end it with `mw.intern` (fail closed). Only Linux has the files; elsewhere the duty is not built (section 3a,
/// scope).
#[cfg(target_os = "linux")]
fn ohne_kind() -> Result<(), Startfehler> {
    match os::hat_kinder() {
        Ok(false) => Ok(()),
        Ok(true) => Err(Startfehler::KindVerboten),
        Err(_) => Err(Startfehler::Intern),
    }
}

/// Only Linux carries the duties of section 3a.
#[cfg(not(target_os = "linux"))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "the same form as the Linux function"
)]
fn ohne_kind() -> Result<(), Startfehler> {
    Ok(())
}

/// The name of the program in the start line of a trial run.
pub const PROGRAMM: &str = "modellweg";
/// The line on stderr before readiness in a trial run (step 8).
pub const PROBELAUF_ZEILE: &str = "mw.probelauf: Marker werden gezählt\n";

/// The steps 2 to 5 after the shielding of step 1: call, key file, protocol folder, resolution, socket.
/// `eigene_uid` is the owner a key file must have; `aufloeser` resolves the target once; `fristen` are the deadlines
/// of the way to the target.
///
/// # Errors
/// The first failing step as [`Startfehler`].
pub fn vorbereiten(
    args: &[OsString],
    eigene_uid: u32,
    aufloeser: &Aufloeser,
    fristen: Fristen,
    uhr: Uhr,
) -> Result<Bereit, Startfehler> {
    let a = aufruf::lies(args).map_err(|f| match f {
        Aufruffehler::Aufruf => Startfehler::Aufruf,
        Aufruffehler::BetriebsartUnbekannt => Startfehler::BetriebsartUnbekannt,
        Aufruffehler::ProbelaufAufruf => Startfehler::ProbelaufAufruf,
    })?;
    match &a.art {
        ArtOptionen::OpenaiLokal(o) => {
            let o = o.clone();
            vorbereiten_eins(a, o, eigene_uid, aufloeser, fristen, uhr)
        }
        #[cfg(target_os = "linux")]
        ArtOptionen::KopfEinsetzen(o) => {
            let o = o.clone();
            vorbereiten_zwei(a, &o, eigene_uid, aufloeser, fristen, uhr)
        }
    }
}

/// Step 5 of a trial run: the count file with its first state, then the start line.
fn probelauf_starten(a: &aufruf::Aufruf, uhr: &Uhr) -> Result<Option<Zaehlung>, Startfehler> {
    let Some(p) = &a.probelauf else {
        return Ok(None);
    };
    let z = Zaehlung::neu(p.marker.clone(), p.datei.clone(), a.sitzung.clone());
    z.anfang().map_err(|_| Startfehler::ZaehlungFehler)?;
    let zeile = Probelauf {
        zeit_ms: uhr(),
        programm: PROGRAMM.to_owned(),
        sitzung: a.sitzung.clone(),
        anzahl: u32::try_from(z.anzahl()).map_err(|_| Startfehler::Intern)?,
    };
    protokoll::anhaengen_probelauf(&a.protokoll, &zeile)
        .map_err(|_| Startfehler::ProtokollFehlt)?;
    Ok(Some(z))
}

fn schluesselfehler(f: Schluesselfehler) -> Startfehler {
    match f {
        Schluesselfehler::Datei => Startfehler::SchluesselDatei,
        Schluesselfehler::Form => Startfehler::SchluesselForm,
    }
}

/// Steps 3 to 7 of mode 1.
fn vorbereiten_eins(
    a: aufruf::Aufruf,
    optionen: Optionen,
    eigene_uid: u32,
    aufloeser: &Aufloeser,
    fristen: Fristen,
    uhr: Uhr,
) -> Result<Bereit, Startfehler> {
    let schluessel =
        schluessel::lies(&optionen.schluessel_datei, eigene_uid).map_err(schluesselfehler)?;
    if !a.protokoll.is_dir() {
        return Err(Startfehler::ProtokollOrdner);
    }
    let zaehlung = probelauf_starten(&a, &uhr)?;
    let adressen =
        ziel::aufloesen(&optionen.ziel, aufloeser).ok_or(Startfehler::ZielUnaufloesbar)?;
    let tls: Option<Box<dyn Tls>> = if optionen.ziel.tls {
        Some(Box::new(SystemTls::neu().map_err(|_| Startfehler::Intern)?))
    } else {
        None
    };
    let verbinder = NetzVerbinder::neu(adressen, optionen.ziel.host.clone(), tls, fristen);
    let (listener, eigener_socket) =
        socket_anlegen(&a.socket, eigene_uid).ok_or(Startfehler::SocketBelegt)?;
    let eigener_socket = Some(eigener_socket);
    let probelauf = zaehlung.is_some();
    let lage = Lage {
        art: optionen,
        sitzung: a.sitzung,
        route: a.route,
        protokoll: a.protokoll,
        schluessel,
        probelauf: zaehlung,
        pfad_voll: a.pfad_voll,
    };
    Ok(Bereit {
        probelauf,
        dienst: Arc::new(Dienst::neu(lage, Box::new(verbinder), uhr)),
        listener,
        socket: a.socket,
        eigener_socket,
    })
}

/// Step 3 of mode 2 (section 13.5): for every bound target in `ziele` the model path reaches the holder at his fixed
/// endpoint, checks the counterparty before the first line and asks `schluessel.vorhanden` with `ziel`, one connection
/// per question (section 5.1). All answers together have to come before `bis`. Fail closed: the start goes on only if
/// the holder answers `{"vorhanden": true}` for every target. The path is never an option and never comes from the
/// start request.
///
/// # Errors
/// `HalterFehlt` and `HalterUnecht` from the connection; `HalterKeinSchluessel` for `false` and
/// `halter.ziel_unbekannt`; `HalterFrist` without an answer in time; `HalterFehler` for everything else.
#[cfg(target_os = "linux")]
pub fn halter_bereit(
    pfad: &Path,
    eigene_uid: u32,
    ziele: &[&str],
    bis: std::time::Instant,
) -> Result<(), Startfehler> {
    for ziel in ziele {
        let mut k = match crate::halter::Klient::verbinde(pfad, eigene_uid) {
            Ok(k) => k,
            Err("mw.halter_unecht") => return Err(Startfehler::HalterUnecht),
            Err(_) => return Err(Startfehler::HalterFehlt),
        };
        match crate::halter::schluessel_da(&mut k, ziel, bis) {
            Ok(()) => {}
            Err("mw.halter_kein_schluessel") => return Err(Startfehler::HalterKeinSchluessel),
            Err("mw.halter_frist") => return Err(Startfehler::HalterFrist),
            Err(_) => return Err(Startfehler::HalterFehler),
        }
    }
    Ok(())
}

/// Steps 2 to 7 of mode 2 (section 13.5): target list, approval of the CA, CA, the holder for the bound targets,
/// protocol folder, trial run, one resolution per unbound host, socket.
#[cfg(target_os = "linux")]
fn vorbereiten_zwei(
    a: aufruf::Aufruf,
    optionen: &kopf_einsetzen::Optionen,
    eigene_uid: u32,
    aufloeser: &Aufloeser,
    fristen: Fristen,
    uhr: Uhr,
) -> Result<Bereit, Startfehler> {
    let text = crate::ca::lies_datei(&optionen.zielliste, eigene_uid)
        .ok_or(Startfehler::ZieleUngueltig)?;
    // Duty 1 (section 3a): a `halter_ziel` that is not the `host` of its target has its own code, in every start and
    // not only under setgid: one code for one thing.
    let liste = zielliste::lies(&text).map_err(|c| {
        if c == zielliste::HALTER_NAME {
            Startfehler::ZielHalterName
        } else {
            Startfehler::ZieleUngueltig
        }
    })?;
    if liste.zulassung != zielliste::Zulassung::Zugelassen {
        return Err(Startfehler::CaNichtZugelassen);
    }
    let hosts: Vec<&str> = liste.ziele.iter().map(|z| z.host.as_str()).collect();
    let ca = crate::ca::Ca::pruefe(
        &optionen.ca_schluessel,
        &optionen.ca_zertifikat,
        eigene_uid,
        &hosts,
    )
    .ok_or(Startfehler::CaUngueltig)?;
    // Step 3 (section 13.5): a bound target carries no key file any more. The holder holds the token; the model path
    // reaches him at his fixed endpoint, checks the counterparty, asks him per bound target whether he holds its
    // secret and never falls back on a file. A list without a bound target skips the step: the holder is not needed
    // then.
    let halter_socket = crate::halter::fester_pfad(eigene_uid);
    let gebunden: Vec<&str> = liste
        .ziele
        .iter()
        .filter_map(|z| z.halter_ziel.as_deref())
        .collect();
    if !gebunden.is_empty() {
        let bis = std::time::Instant::now() + crate::halter::FRIST_START;
        halter_bereit(&halter_socket, eigene_uid, &gebunden, bis)?;
    }
    if !a.protokoll.is_dir() {
        return Err(Startfehler::ProtokollOrdner);
    }
    let zaehlung = probelauf_starten(&a, &uhr)?;
    let mut verbinder: Vec<Option<Box<dyn ziel::Verbinder>>> = Vec::new();
    for z in &liste.ziele {
        // Step 6: a bound host is never resolved here. Its address stands only in `halter.json`, and a failure there
        // is `mw.halter_fehler` in the session, not `mw.ziel_unaufloesbar` at the start.
        if z.gebunden() {
            verbinder.push(None);
            continue;
        }
        let adressen = aufloeser(&z.host, z.port).ok().filter(|v| !v.is_empty());
        let Some(adressen) = adressen else {
            // An unbound target without address blocks nothing (section 13.5).
            verbinder.push(None);
            continue;
        };
        let tls: Box<dyn Tls> = Box::new(SystemTls::neu().map_err(|_| Startfehler::Intern)?);
        verbinder.push(Some(Box::new(NetzVerbinder::neu(
            adressen,
            z.host.clone(),
            Some(tls),
            fristen,
        ))));
    }
    let (listener, eigener_socket) =
        socket_anlegen(&a.socket, eigene_uid).ok_or(Startfehler::SocketBelegt)?;
    let eigener_socket = Some(eigener_socket);
    let probelauf = zaehlung.is_some();
    let tunnel = crate::tunnel::Tunnel::neu(
        liste,
        ca,
        verbinder,
        a.sitzung,
        a.route,
        a.protokoll,
        zaehlung,
        a.pfad_voll,
        eigene_uid,
        halter_socket,
        Betrieb::neu(uhr),
    );
    Ok(Bereit {
        probelauf,
        dienst: Arc::new(tunnel),
        listener,
        socket: a.socket,
        eigener_socket,
    })
}

/// Step 7, in this order: open the folder of the socket so that **no** part of the path is followed through a link,
/// check that it is a folder of this user that no other user may write into, bind relative to that open folder with the
/// umask 0177, then check the socket. Each of the three steps before the bind ends the start before the bind if it
/// fails; there is no fallback that opens or binds. After the bind the code remembers the socket (kind, device, inode)
/// relative to the open folder, in the same step as it checks the kind (a socket) and the rights (exactly 0600). If
/// anything differs, the socket is deleted again, but only over the descriptor of the folder and only if the name still
/// shows that inode, and the start ends. Whatever already lies at the path stays untouched.
fn socket_anlegen(pfad: &Path, eigene_uid: u32) -> Option<(UnixListener, Arc<Eigene>)> {
    os::mit_umask(|| socket_binden(pfad, eigene_uid))
}

/// [`socket_anlegen`] without the umask (it is a setting of the whole process, so the tests of this file call this).
fn socket_binden(pfad: &Path, eigene_uid: u32) -> Option<(UnixListener, Arc<Eigene>)> {
    socket_binden_mit(pfad, eigene_uid, || {})
}

/// [`socket_binden`]; `zwischen` runs between the bind and the check (a seam for the tests, empty in the program).
fn socket_binden_mit(
    pfad: &Path,
    eigene_uid: u32,
    zwischen: impl FnOnce(),
) -> Option<(UnixListener, Arc<Eigene>)> {
    let (ordner, name) = Ordner::ohne_verweis(pfad).ok()?;
    // Before the bind: only while no other user may write into this folder is the window between the bind and the
    // check closed, because nobody else can put an entry under the name in between (section 7).
    if !ordner.nur_eigen_schreibbar(eigene_uid) {
        return None;
    }
    let listener = match bind_im_ordner(&ordner, &name) {
        Bindung::Da(l) => l,
        Bindung::Nichts => return None,
        #[cfg(not(target_os = "linux"))]
        Bindung::Aufraeumen => {
            // The bind made the entry, but the step cannot be finished. The entry goes again, over the descriptor of
            // the folder and only if the name still shows a socket with the rights of this bind.
            if let Some(e) = Eigene::merken(ordner, name, rustix::fs::FileType::Socket)
                && e.modus() == SOCKET_MODUS
            {
                e.entfernen();
            }
            return None;
        }
    };
    zwischen();
    let eigene = Arc::new(Eigene::merken(ordner, name, rustix::fs::FileType::Socket)?);
    // in the same step as the remembering: from here on the end deletes this socket, also before readiness
    ENDE.socket_merken(&eigene);
    if eigene.modus() != SOCKET_MODUS {
        eigene.entfernen();
        return None;
    }
    Some((listener, eigene))
}

/// What a bind relative to the open folder left behind.
enum Bindung {
    /// Bound; the entry lies in the open folder.
    Da(UnixListener),
    /// Nothing was made.
    Nichts,
    /// The bind made the entry, but the step cannot be finished: the entry has to go again. Only the way over the
    /// working folder can end here, so under Linux, which binds relative to the path of the descriptor, there is no
    /// such outcome and no such variant.
    #[cfg(not(target_os = "linux"))]
    Aufraeumen,
}

/// Binds the socket `name` relative to the open folder, so that the kernel does not resolve the names above the socket
/// a second time. Under Linux the path of the descriptor does it: `/proc/self/fd/<n>/<name>` reaches exactly the folder
/// of that descriptor, whatever happens to the names above it in the meantime. Needs `/proc`; without it the bind fails
/// and the start ends (fail closed). A name that does not fit into `sun_path` together with the prefix ends the start
/// as well.
#[cfg(target_os = "linux")]
fn bind_im_ordner(ordner: &Ordner, name: &std::ffi::OsStr) -> Bindung {
    use std::os::fd::AsRawFd;

    let pfad = PathBuf::from(format!("/proc/self/fd/{}", ordner.fd().as_raw_fd())).join(name);
    match UnixListener::bind(pfad) {
        Ok(l) => Bindung::Da(l),
        Err(_) => Bindung::Nichts,
    }
}

/// The working folder belongs to the whole process, so only one bind of [`bind_im_ordner`] may hold it at a time. The
/// lock makes the three calls of that bind one step: whoever waits here finds the working folder of the process as it
/// was before (measured 23.11.2026: without the lock four tests that bind side by side fail, because one of them goes
/// back into the folder of another one and that folder is removed in the meantime).
#[cfg(not(target_os = "linux"))]
static BIND_PLATZ: Mutex<()> = Mutex::new(());

/// Binds the socket `name` relative to the open folder. macOS has neither `openat2` nor a path of a descriptor that
/// leads into a folder (measured 23.11.2026: `/dev/fd/<n>/<name>` ends with `ENOTDIR`), and `bind` has no `…at` form.
/// The working folder is therefore the only way to a bind without a path of names: `fchdir` to the descriptor, bind the
/// bare name, `fchdir` back, the three of them under [`BIND_PLATZ`]. The restriction is named in the contract: the
/// working folder of the whole process is this folder for the length of the bind, so no other thread may use a relative
/// path in that moment (the umask of step 7 is a setting of the whole process in the same way). Cannot the code go
/// back, it binds nothing and the start ends.
#[cfg(not(target_os = "linux"))]
fn bind_im_ordner(ordner: &Ordner, name: &std::ffi::OsStr) -> Bindung {
    let _platz = BIND_PLATZ.lock().unwrap_or_else(PoisonError::into_inner);
    let Ok(zurueck) = Ordner::hier() else {
        return Bindung::Nichts;
    };
    if rustix::process::fchdir(ordner.fd()).is_err() {
        return Bindung::Nichts;
    }
    let gebunden = UnixListener::bind(Path::new(name));
    let zurueck_ok = rustix::process::fchdir(zurueck.fd()).is_ok();
    match (gebunden, zurueck_ok) {
        (Ok(l), true) => Bindung::Da(l),
        // the entry is there, but the working folder of the process is not the old one any more: fail closed
        (Ok(_), false) => Bindung::Aufraeumen,
        (Err(_), _) => Bindung::Nichts,
    }
}

/// The rights of the socket: exactly 0600 (the umask 0177 gives them).
const SOCKET_MODUS: Mode = Mode::from_bits_retain(0o600);

/// The system time in milliseconds since 1970; `i64::MIN` (no valid time, so the line fails closed) if the clock
/// cannot be read.
#[must_use]
pub fn jetzt_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(i64::MIN)
}

/// Whether the parent changed between the two readings. `gemerkt` is the parent from the first line of [`haupt`],
/// `jetzt` the one after the death signal was set. A parent that ended in between is gone for good: the kernel sent
/// the death signal before the setting, so none ever comes, and the process must end at once (contract 8, section 3;
/// the same rule as in the trampoline of contract 1).
fn eltern_weg(gemerkt: Option<i32>, jetzt: Option<i32>) -> bool {
    gemerkt != jetzt
}

/// Sets the death signal of step 1; `true` if the system took it.
#[cfg(target_os = "linux")]
fn todessignal_setzen() -> bool {
    os::todessignal().is_ok()
}

/// Only Linux has the death signal; elsewhere there is nothing to set and the check of the parent alone stays.
#[cfg(not(target_os = "linux"))]
fn todessignal_setzen() -> bool {
    true
}

/// Step 1, the binding to the parent: first the death signal, then the check of the parent against `gemerkt`.
/// `setzen` sets the signal and `jetzt` reads the parent again (the seam of the tests: the race cannot be provoked
/// from outside the process).
///
/// # Errors
/// [`Startfehler::Intern`] when the system refuses the death signal, [`Startfehler::ElternWeg`] when the parent
/// changed in between.
fn binden_mit(
    gemerkt: Option<i32>,
    setzen: impl FnOnce() -> bool,
    jetzt: impl FnOnce() -> Option<i32>,
) -> Result<(), Startfehler> {
    if !setzen() {
        return Err(Startfehler::Intern);
    }
    if eltern_weg(gemerkt, jetzt()) {
        return Err(Startfehler::ElternWeg);
    }
    Ok(())
}

/// [`binden_mit`] with the calls of the system.
///
/// # Errors
/// As [`binden_mit`].
fn binden(gemerkt: Option<i32>) -> Result<(), Startfehler> {
    binden_mit(gemerkt, todessignal_setzen, os::eltern)
}

/// Steps 2 to 9 after step 1: [`vorbereiten`], the line of a trial run on stderr and the ready line on stdout. On a
/// failure the stderr line is written and the exit code returned. The handlers of SIGTERM and SIGINT stand before
/// this (step 1, [`haupt`]).
///
/// This is the entry of the library for a start inside another process, such as a test: it does not read duty 5 of
/// section 3a, because the children it would see are those of that other process. The program ([`haupt`]) reads it.
///
/// # Errors
/// The exit code of the first failing step.
pub fn starten(
    args: &[OsString],
    eigene_uid: u32,
    aufloeser: &Aufloeser,
    fristen: Fristen,
    uhr: Uhr,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<Bereit, u8> {
    starten_mit(
        args,
        eigene_uid,
        aufloeser,
        fristen,
        uhr,
        stdout,
        stderr,
        || Ok(()),
    )
}

/// [`starten`] with the check of duty 5 (section 3a) before the ready line: `ohne_kind` fails if the process has a
/// child, and the start then ends with its code (and deletes its socket). The program passes the reading of its own
/// threads; a test passes its own answer.
///
/// # Errors
/// The exit code of the first failing step.
#[allow(
    clippy::too_many_arguments,
    reason = "the arguments of starten and the one check"
)]
pub fn starten_mit(
    args: &[OsString],
    eigene_uid: u32,
    aufloeser: &Aufloeser,
    fristen: Fristen,
    uhr: Uhr,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    ohne_kind: impl FnOnce() -> Result<(), Startfehler>,
) -> Result<Bereit, u8> {
    let bereit =
        vorbereiten(args, eigene_uid, aufloeser, fristen, uhr).map_err(|f| fehler(stderr, f))?;
    // duty 5 (section 3a): no child before the ready line
    if let Err(f) = ohne_kind() {
        loeschen_socket(bereit.eigener_socket.as_deref());
        return Err(fehler(stderr, f));
    }
    // before the two lines: from here on the end tears running requests down, whatever a signal interrupts
    ENDE.dienst_merken(&bereit.dienst);
    if bereit.probelauf {
        let _ = stderr.write_all(PROBELAUF_ZEILE.as_bytes());
        let _ = stderr.flush();
    }
    if stdout
        .write_all(b"{\"bereit\":true}\n")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        loeschen_socket(bereit.eigener_socket.as_deref());
        return Err(fehler(stderr, Startfehler::Intern));
    }
    Ok(bereit)
}

/// Writes the stderr line of a failure and returns its exit code.
fn fehler(stderr: &mut dyn Write, f: Startfehler) -> u8 {
    let _ = stderr.write_all(f.zeile().as_bytes());
    let _ = stderr.flush();
    f.exit()
}

/// Step 1 (section 3): the result of `kernabbild::abschirmen` as the behaviour of the start. A note (the hard limit for core was 0 already and
/// the core pattern is a file pattern or missing) goes to stderr as `mw.kernabbild_hinweis: <text>` and the start goes on; an abort writes the
/// line of [`Startfehler::Kernabbild`] and a second line `mw.kernabbild_grund: <reason>` and returns exit 5 (fail closed).
///
/// # Errors
/// The exit code 5.
pub fn abschirmen(
    ergebnis: Result<kernabbild::Abgeschirmt, kernabbild::Abbruch>,
    stderr: &mut dyn Write,
) -> Result<(), u8> {
    match ergebnis {
        Ok(abgeschirmt) => {
            if let Some(hinweis) = abgeschirmt.hinweis {
                let _ = writeln!(stderr, "mw.kernabbild_hinweis: {}", hinweis.text());
                let _ = stderr.flush();
            }
            Ok(())
        }
        Err(grund) => {
            let exit = fehler(stderr, Startfehler::Kernabbild);
            let _ = writeln!(stderr, "mw.kernabbild_grund: {}", grund.grund());
            let _ = stderr.flush();
            Err(exit)
        }
    }
}

/// The program: remembers its parent, checks its environment (stage 0, before anything else and before any file),
/// shields itself, binds itself to its parent, runs the steps and serves until SIGTERM or SIGINT. Returns the exit code
/// of a failure before readiness; after readiness the process ends in [`Ende::beenden`].
pub fn haupt(args: &[OsString], stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
    // the first line of the program (only the argv of `main` runs before it): the check after the death signal compares
    // against this number, so it must be read before the parent can end. After the emptying of duty 3 the reading of
    // the first image comes in argv[0].
    let erste = os::eltern();
    // only Linux empties by the re-exec, so only there argv[0] can carry the mark
    let geleert = if cfg!(target_os = "linux") {
        std::env::args_os().next().as_deref().and_then(marke_lesen)
    } else {
        None
    };
    let eltern = geleert.unwrap_or(erste);
    if !umgebung_sicher(std::env::var_os) {
        return fehler(stderr, Startfehler::UmgebungUnsicher);
    }
    // Section 3a (safe mode) under Linux, in every start and not only under setgid (`AT_SECURE`): duty 3 empties the
    // environment before any file is read and any connection opened, duty 4 sets the umask before the first entry is
    // made. Stage 0 stays in front, so an attempt with a variable of OpenSSL is seen and not silently emptied.
    #[cfg(target_os = "linux")]
    {
        match leeren(geleert.is_some(), std::env::vars_os().next().is_none()) {
            Leeren::Weiter => {}
            Leeren::Neu => {
                // returns only on a failure: the environment was not emptied
                let _ = os::neu_ohne_umgebung(&marke(erste), args);
                return fehler(stderr, Startfehler::UmgebungRest);
            }
            Leeren::Rest => return fehler(stderr, Startfehler::UmgebungRest),
        }
        os::umask_sicher();
    }
    if let Err(exit) = abschirmen(kernabbild::abschirmen(), stderr) {
        return exit;
    }
    if let Some((erstes, rest)) = args.split_first()
        && erstes == "suche"
    {
        if let Err(f) = binden(eltern) {
            return fehler(stderr, f);
        }
        return crate::suche::lauf(rest, os::eigene_uid(), crate::suche::PUFFER, stdout, stderr);
    }
    // The handlers stand before the death signal: a SIGTERM inherited as ignored (`SIG_IGN` survives `execve`; measured
    // 21.11.2026 as `SigIgn` 0x5000) would swallow the signal of the parent's death, and `PR_SET_PDEATHSIG = SIGTERM`
    // would do nothing until a handler stands. `Signals::new` of signal-hook sets one without `unsafe`.
    let Ok(mut signale) = Signals::new([signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT])
    else {
        return fehler(stderr, Startfehler::Intern);
    };
    if let Err(f) = binden(eltern) {
        return fehler(stderr, f);
    }
    // the thread of the end runs before the first step: a signal before readiness ends the process too, and it deletes
    // the socket of this process if there is one already
    let waechter = std::thread::Builder::new().spawn(move || {
        if signale.forever().next().is_some() {
            ENDE.beenden();
        }
    });
    if waechter.is_err() {
        return fehler(stderr, Startfehler::Intern);
    }
    let gestartet = starten_mit(
        args,
        os::eigene_uid(),
        &ziel::system_aufloeser,
        ziel::FRISTEN,
        Box::new(jetzt_ms),
        stdout,
        stderr,
        ohne_kind,
    );
    let bereit = match gestartet {
        Ok(b) => b,
        Err(exit) => return exit,
    };
    dienen(&bereit)
}

/// Deletes the socket this process made; nothing if it was not remembered or the name shows something else now. A
/// failure changes no exit code.
fn loeschen_socket(socket: Option<&Eigene>) {
    if let Some(e) = socket {
        e.entfernen();
    }
}

/// The loop over connections; each one gets its own thread. Before a connection is served, duty 5 (section 3a) is read
/// again: a child that an earlier request left behind ends the process (the end of section 3, then the line of
/// [`Startfehler::KindVerboten`] and exit 70).
fn dienen(bereit: &Bereit) -> ! {
    loop {
        let Ok((strom, _)) = bereit.listener.accept() else {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        if let Err(f) = ohne_kind() {
            drop(strom);
            ENDE.aufraeumen();
            let _ = fehler(&mut std::io::stderr(), f);
            std::process::exit(i32::from(f.exit()));
        }
        let faden = Faden::nimm(Arc::clone(&bereit.dienst));
        let _ = std::thread::Builder::new().spawn(move || {
            faden.bediene(strom);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GELEERT, Leeren, Startfehler, UMGEBUNG_UNSICHER, binden_mit, eltern_weg, leeren,
        loeschen_socket, marke, marke_lesen, socket_binden_mit, umgebung_sicher,
    };
    use std::cell::Cell;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    /// Duty 5 (section 3a): a child of this process ends the start with `mw.kind_verboten`. (That none lets it pass is
    /// checked from outside, on the program: other tests of this binary start children of their own at any moment.)
    #[cfg(target_os = "linux")]
    #[test]
    fn a_child_is_forbidden() {
        let mut kind = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep");
        let gesehen = super::ohne_kind();
        let _ = kind.kill();
        let _ = kind.wait();
        assert_eq!(gesehen, Err(Startfehler::KindVerboten));
    }

    #[test]
    fn stage_0_refuses_each_variable_also_empty() {
        assert!(umgebung_sicher(|_| None));
        for n in UMGEBUNG_UNSICHER {
            assert!(!umgebung_sicher(|v| (v == n).then(OsString::new)), "{n}");
            assert!(
                !umgebung_sicher(|v| (v == n).then(|| OsString::from("/x"))),
                "{n}"
            );
        }
        assert!(umgebung_sicher(|v| {
            ["SSL_CERT_FILES", "OPENSSL_CONFX", "PATH"]
                .contains(&v)
                .then(OsString::new)
        }));
        assert_eq!(
            UMGEBUNG_UNSICHER,
            ["SSL_CERT_FILE", "SSL_CERT_DIR", "OPENSSL_CONF"]
        );
    }

    #[test]
    fn the_check_of_the_parent_sees_every_change_and_nothing_else() {
        assert!(!eltern_weg(Some(4711), Some(4711)), "the same parent");
        assert!(
            eltern_weg(Some(4711), Some(1)),
            "adopted by another process"
        );
        assert!(eltern_weg(Some(4711), None), "no parent any more");
        assert!(eltern_weg(None, Some(1)), "a parent where there was none");
        assert!(!eltern_weg(None, None), "no parent in both readings");
    }

    #[test]
    fn the_line_of_the_parent_that_ended() {
        let f = Startfehler::ElternWeg;
        assert_eq!(f.zeile(), "mw.eltern_weg: Elternprozess beendet\n");
        assert_eq!(f.exit(), 69);
    }

    #[test]
    fn the_binding_sets_the_death_signal_first_and_then_checks_the_parent() {
        // The order matters: between the two readings of the parent lies the setting, so a parent that ends before the
        // setting is seen. The seam counts the calls in their order.
        let folge: Cell<String> = Cell::new(String::new());
        let merke = |zeichen: &str| {
            let mut s = folge.take();
            s.push_str(zeichen);
            folge.set(s);
        };
        let r = binden_mit(
            Some(4711),
            || {
                merke("s");
                true
            },
            || {
                merke("p");
                Some(4711)
            },
        );
        assert_eq!(r, Ok(()));
        assert_eq!(folge.take(), "sp", "the signal is set before the check");
        // The parent ended in between: the start ends at once, no signal will ever come.
        assert_eq!(
            binden_mit(Some(4711), || true, || Some(1)),
            Err(Startfehler::ElternWeg)
        );
        assert_eq!(
            binden_mit(Some(4711), || true, || None),
            Err(Startfehler::ElternWeg)
        );
        // The system refuses the death signal: that stays an internal error (exit 70), as before.
        assert_eq!(
            binden_mit(Some(4711), || false, || Some(4711)),
            Err(Startfehler::Intern)
        );
        // The check does not run when the setting failed (its failure is the first one).
        let gelaufen = Cell::new(false);
        let r = binden_mit(
            Some(4711),
            || false,
            || {
                gelaufen.set(true);
                Some(1)
            },
        );
        assert_eq!(r, Err(Startfehler::Intern));
        assert!(!gelaufen.get(), "the check stays unrun");
    }

    #[test]
    fn the_lines_of_the_safe_mode() {
        for (f, zeile, exit) in [
            (
                Startfehler::UmgebungRest,
                "mw.umgebung_rest: Umgebung nicht geleert\n",
                78,
            ),
            (
                Startfehler::ZielHalterName,
                "mw.ziel_halter_name: Halterziel nicht der Host\n",
                78,
            ),
            (
                Startfehler::KindVerboten,
                "mw.kind_verboten: Kindprozess verboten\n",
                70,
            ),
        ] {
            assert_eq!(f.zeile(), zeile);
            assert_eq!(f.exit(), exit);
            assert!(f.zeile().is_ascii() && f.zeile().len() <= 200);
        }
    }

    #[test]
    fn duty_3_empties_once_and_refuses_a_rest() {
        assert_eq!(leeren(false, true), Leeren::Weiter);
        assert_eq!(leeren(true, true), Leeren::Weiter);
        assert_eq!(leeren(false, false), Leeren::Neu);
        assert_eq!(leeren(true, false), Leeren::Rest);
    }

    #[test]
    fn the_mark_carries_the_first_reading_of_the_parent() {
        for e in [Some(1), Some(4242), Some(i32::MAX), None] {
            assert_eq!(marke_lesen(&marke(e)), Some(e), "{e:?}");
        }
        assert_eq!(marke(Some(7)), OsString::from("wb-modellweg-geleert:7"));
        assert_eq!(marke(None), OsString::from("wb-modellweg-geleert:-"));
        assert_eq!(GELEERT, "wb-modellweg-geleert");
        // anything else is no mark: this image is the first one
        for kein in [
            "modellweg",
            "/usr/lib/werkbank/werkbank-modellweg",
            "wb-modellweg-geleert",
            "wb-modellweg-geleert:",
            "wb-modellweg-geleert:0",
            "wb-modellweg-geleert:-1",
            "wb-modellweg-geleert:+7",
            "wb-modellweg-geleert:07",
            "wb-modellweg-geleert:7 ",
            "wb-modellweg-geleert:x",
            "wb-modellweg-geleert:--",
            "wb-modellweg-geleert:99999999999",
            "xwb-modellweg-geleert:7",
            "wb-modellweg-geleert;7",
        ] {
            assert_eq!(marke_lesen(std::ffi::OsStr::new(kein)), None, "{kein}");
        }
    }

    /// The product part of a source file: the text before its module of tests, without the lines of comments.
    fn produkt_von(p: &std::path::Path) -> String {
        let text = fs::read_to_string(p).expect("source");
        let vor = text.split("\nmod tests {").next().expect("text");
        vor.lines()
            .filter(|z| !z.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Duty 5 (section 3a): no product file starts a process. The test reads every source file of the crate up to its
    /// module of tests and finds the calls that start or replace a process; the one replacement of duty 3
    /// (`os::neu_ohne_umgebung`) is the only hit, and it starts no child. `spawn(` stands only for threads.
    #[test]
    fn no_product_code_starts_a_process() {
        let wurzel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut dateien = vec![wurzel.clone()];
        let mut funde = Vec::new();
        while let Some(p) = dateien.pop() {
            if p.is_dir() {
                for e in fs::read_dir(&p).expect("folder") {
                    dateien.push(e.expect("entry").path());
                }
                continue;
            }
            if p.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let produkt = produkt_von(&p);
            let name = p
                .strip_prefix(&wurzel)
                .expect("below src")
                .to_string_lossy()
                .into_owned();
            for muster in [
                "Command",
                "fork",
                "posix_spawn",
                "execv",
                "exec(",
                "system(",
                "libc::",
                "nix::unistd",
                "nix::spawn",
                "rustix::runtime",
            ] {
                let n = produkt.matches(muster).count();
                if n > 0 {
                    funde.push((name.clone(), muster, n));
                }
            }
            // `spawn(` only for threads: within its statement, before the hit, stands `thread::`
            for (i, _) in produkt.match_indices("spawn(") {
                let anfang = produkt[..i].rfind(';').map_or(0, |n| n + 1);
                assert!(
                    produkt[anfang..i].contains("thread::"),
                    "{name}: {}",
                    &produkt[anfang..i]
                );
            }
        }
        funde.sort_unstable();
        assert_eq!(
            funde,
            vec![
                ("os.rs".to_owned(), "Command", 2),
                ("os.rs".to_owned(), "exec(", 1)
            ],
            "{funde:?}"
        );
    }

    #[test]
    fn the_line_of_stage_0() {
        let f = Startfehler::UmgebungUnsicher;
        assert_eq!(f.zeile(), "mw.umgebung_unsicher: Umgebung unsicher\n");
        assert_eq!(f.exit(), 78);
    }

    /// A short folder of its own (a socket path is limited to about 100 bytes). The base is resolved
    /// (`canonicalize`): step 7 refuses a socket path with a link in any of its parts, and on macOS both `/tmp` and
    /// `/var` are links.
    fn wegwerf(name: &str) -> PathBuf {
        let basis = fs::canonicalize(std::env::temp_dir()).expect("the base of the tests");
        let pfad = basis.join(format!("mw-st-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("folder");
        fs::set_permissions(&pfad, fs::Permissions::from_mode(0o700)).expect("chmod");
        pfad
    }

    /// The user of this process, as step 7 gets it.
    fn uid() -> u32 {
        crate::os::eigene_uid()
    }

    /// Binds like the program, but gives the socket 0600 in the seam: the umask is a setting of the whole process and
    /// the tests run side by side, so they do not set it (the start of the program does, see `tests/aussen.rs`).
    fn binden(pfad: &std::path::Path) -> Option<(UnixListener, std::sync::Arc<super::Eigene>)> {
        let p = pfad.to_path_buf();
        socket_binden_mit(pfad, uid(), move || {
            let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o600));
        })
    }

    #[test]
    fn the_own_socket_is_made_with_0600_and_deleted_again() {
        let ordner = wegwerf("eigen");
        let pfad = ordner.join("s");
        let (_listener, eigene) = binden(&pfad).expect("socket");
        assert_eq!(fs::metadata(&pfad).expect("stat").mode() & 0o777, 0o600);
        loeschen_socket(Some(&eigene));
        assert!(!pfad.exists(), "the own socket is deleted");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn an_existing_path_is_not_taken_and_stays() {
        let ordner = wegwerf("belegt");
        let pfad = ordner.join("s");
        fs::write(&pfad, "x").expect("write");
        assert!(binden(&pfad).is_none());
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "x");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn another_socket_a_file_or_a_link_under_the_name_stays() {
        let ordner = wegwerf("fremd");
        let pfad = ordner.join("s");
        let (_listener, eigene) = binden(&pfad).expect("socket");
        // Another socket under the same name (another inode; the first one is unlinked, not closed).
        fs::remove_file(&pfad).expect("remove");
        let fremd = UnixListener::bind(&pfad).expect("bind");
        loeschen_socket(Some(&eigene));
        assert!(
            fs::symlink_metadata(&pfad).is_ok(),
            "the other socket stays"
        );
        // A link to a socket outside: the link stays and so does its target.
        fs::remove_file(&pfad).expect("remove");
        let aussen = ordner.join("a");
        let _aussen_listener = UnixListener::bind(&aussen).expect("bind");
        symlink(&aussen, &pfad).expect("link");
        loeschen_socket(Some(&eigene));
        assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
        assert!(fs::symlink_metadata(&aussen).is_ok(), "the target stays");
        // A plain file.
        fs::remove_file(&pfad).expect("remove");
        fs::write(&pfad, "x").expect("write");
        loeschen_socket(Some(&eigene));
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "x");
        drop(fremd);
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn without_a_remembered_socket_nothing_is_deleted() {
        let ordner = wegwerf("nichts");
        let pfad = ordner.join("s");
        let (_listener, _eigene) = binden(&pfad).expect("socket");
        loeschen_socket(None);
        assert!(fs::symlink_metadata(&pfad).is_ok());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_folder_that_is_a_link_ends_the_start_before_the_bind() {
        let ordner = wegwerf("verweis");
        fs::create_dir(ordner.join("echt")).expect("folder");
        symlink(ordner.join("echt"), ordner.join("lnk")).expect("link");
        let pfad = ordner.join("lnk").join("s");
        assert!(binden(&pfad).is_none());
        assert!(
            !ordner.join("echt").join("s").exists(),
            "nothing is bound in the target folder"
        );
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_link_above_the_folder_of_the_socket_ends_the_start_before_the_bind() {
        let ordner = wegwerf("ueber");
        // `echt/unten` is the real folder of the socket and `lnk` a link to `echt`: in the socket path
        // `…/lnk/unten/s` the link stands **above** the folder of the socket, not at its last part.
        let echt = ordner.join("echt");
        let unten = echt.join("unten");
        fs::create_dir_all(&unten).expect("folders");
        symlink(&echt, ordner.join("lnk")).expect("link");
        let pfad = ordner.join("lnk").join("unten").join("s");
        assert!(binden(&pfad).is_none(), "a link above the folder ends it");
        assert_eq!(
            fs::read_dir(&unten).expect("dir").count(),
            0,
            "nothing is bound in the folder the link leads to"
        );
        // The same folder without the link binds, so the test measures the link and nothing else.
        let (_listener, eigene) = binden(&unten.join("s")).expect("socket");
        assert!(eigene.entfernen());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_folder_other_users_may_write_into_ends_the_start_before_the_bind() {
        // The window between the bind and the check (contract 8, step 7) stays closed only while no other user may
        // put an entry under the name. A folder with a `w` for group or others therefore ends the start **before** the
        // bind, so no socket of this process is ever made in it.
        for modus in [0o777, 0o770, 0o707, 0o722] {
            let ordner = wegwerf("offen");
            let pfad = ordner.join("s");
            fs::set_permissions(&ordner, fs::Permissions::from_mode(modus)).expect("chmod");
            assert!(binden(&pfad).is_none(), "{modus:o}");
            assert!(
                fs::symlink_metadata(&pfad).is_err(),
                "{modus:o}: nothing was bound"
            );
            // The tight folder binds under the same path, so the test measures the rights and nothing else.
            fs::set_permissions(&ordner, fs::Permissions::from_mode(0o750)).expect("chmod");
            let (_listener, eigene) = binden(&pfad).expect("socket");
            assert!(eigene.entfernen());
            let _ = fs::remove_dir_all(&ordner);
        }
    }

    #[test]
    fn a_foreign_user_as_the_owner_of_the_folder_ends_the_start_before_the_bind() {
        // The check is the pair of owner and rights; a folder of another user ends the start as well. Only the check
        // can be measured without privileges: a uid that is not the one of this process. The seam gives the socket
        // 0600 as everywhere else, so the owner is the only reason left for the end (without it the test would pass
        // on the rights of the socket, measured 24.11.2026).
        let ordner = wegwerf("fremder");
        let pfad = ordner.join("s");
        let p = pfad.clone();
        let fremd = uid().wrapping_add(1);
        let r = socket_binden_mit(&pfad, fremd, move || {
            let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o600));
        });
        assert!(r.is_none());
        assert!(fs::symlink_metadata(&pfad).is_err(), "nothing was bound");
        // The same folder with the own user binds, so the test measures the owner and nothing else.
        let (_listener, eigene) = binden(&pfad).expect("socket");
        assert!(eigene.entfernen());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn rights_other_than_0600_delete_the_own_socket_and_end_the_start() {
        for modus in [0o666, 0o660, 0o700, 0o400, 0o4600] {
            let ordner = wegwerf("rechte");
            let pfad = ordner.join("s");
            let p = pfad.clone();
            let r = socket_binden_mit(&pfad, uid(), move || {
                fs::set_permissions(&p, fs::Permissions::from_mode(modus)).expect("chmod");
            });
            assert!(r.is_none(), "{modus:o}");
            assert!(fs::symlink_metadata(&pfad).is_err(), "{modus:o}: deleted");
            let _ = fs::remove_dir_all(&ordner);
        }
    }

    #[test]
    fn the_end_deletes_the_socket_it_knows_and_nothing_else() {
        let ordner = wegwerf("ende");
        let pfad = ordner.join("s");
        let (_listener, eigene) = binden(&pfad).expect("socket");
        // a place of its own, so the tests beside this one do not matter; the program has the static `ENDE`
        let ende = super::Ende {
            socket: std::sync::Mutex::new(None),
            dienst: std::sync::Mutex::new(None),
        };
        // without a socket and without a service the end does nothing, and the entry stays
        ende.aufraeumen();
        assert!(pfad.exists(), "nothing is deleted without a socket");
        ende.socket_merken(&eigene);
        ende.aufraeumen();
        assert!(!pfad.exists(), "the socket of the process is deleted");
        // a second end finds the name gone and leaves whatever lies there now
        fs::write(&pfad, "x").expect("write");
        ende.aufraeumen();
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "x");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn the_bind_remembers_the_socket_in_the_place_of_the_end() {
        let ordner = wegwerf("platz");
        let pfad = ordner.join("s");
        let (_listener, _eigene) = binden(&pfad).expect("socket");
        // The place is a static of the process, and the tests run side by side: another test may have put its own
        // socket there in between. That it is never empty after a bind is enough here; that the end deletes exactly
        // this socket is measured in `tests/aussen.rs`.
        assert!(
            super::ENDE
                .socket
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some(),
            "the bind left the place of the end empty"
        );
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_socket_with_0600_is_remembered_with_its_rights() {
        let ordner = wegwerf("naht");
        let pfad = ordner.join("s");
        let (_l, eigene) = binden(&pfad).expect("socket");
        assert_eq!(eigene.modus(), super::SOCKET_MODUS);
        assert!(eigene.entfernen());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn another_entry_between_the_bind_and_the_check_stays_and_ends_the_start() {
        // A plain file, a link and a folder under the name: kind is not a socket.
        for art in ["datei", "verweis", "ordner"] {
            let ordner = wegwerf("tausch");
            let pfad = ordner.join("s");
            let aussen = ordner.join("a");
            fs::write(&aussen, "a").expect("write");
            let (p, a) = (pfad.clone(), aussen.clone());
            let r = socket_binden_mit(&pfad, uid(), move || {
                fs::remove_file(&p).expect("remove");
                match art {
                    "datei" => fs::write(&p, "fremd").expect("write"),
                    "verweis" => symlink(&a, &p).expect("link"),
                    _ => fs::create_dir(&p).expect("folder"),
                }
            });
            assert!(r.is_none(), "{art}");
            assert!(fs::symlink_metadata(&pfad).is_ok(), "{art}: stays");
            assert_eq!(fs::read_to_string(&aussen).expect("read"), "a", "{art}");
            let _ = fs::remove_dir_all(&ordner);
        }
    }

    #[test]
    fn an_entry_swapped_in_with_other_rights_stays_if_it_is_not_a_socket() {
        let ordner = wegwerf("tausch2");
        let pfad = ordner.join("s");
        let p = pfad.clone();
        let r = socket_binden_mit(&pfad, uid(), move || {
            fs::remove_file(&p).expect("remove");
            fs::write(&p, "fremd").expect("write");
            fs::set_permissions(&p, fs::Permissions::from_mode(0o666)).expect("chmod");
        });
        assert!(r.is_none());
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "fremd");
        let _ = fs::remove_dir_all(&ordner);
    }
}
