//! The start (contract 8, sections 3 and 4): the steps before readiness with their exit codes and stderr lines, the line
//! `{"bereit":true}`, the loop over connections and the end on SIGTERM or SIGINT.

use std::ffi::OsString;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
    /// `mw.ziele_ungueltig`, exit 78 (mode 2: the target list).
    ZieleUngueltig,
    /// `mw.ca_nicht_zugelassen`, exit 78 (mode 2: the company has not approved the local CA).
    CaNichtZugelassen,
    /// `mw.ca_ungueltig`, exit 78 (mode 2: the throw-away CA).
    CaUngueltig,
    /// `mw.ziel_unaufloesbar`, exit 69.
    ZielUnaufloesbar,
    /// `mw.socket_belegt`, exit 74.
    SocketBelegt,
    /// `mw.intern`, exit 70.
    Intern,
    /// `mw.kernabbild`, exit 5 (step 1: `kernabbild::abschirmen` refuses; default of the Werkbank, 21.11.2026).
    Kernabbild,
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
            Self::ZieleUngueltig => "mw.ziele_ungueltig",
            Self::CaNichtZugelassen => "mw.ca_nicht_zugelassen",
            Self::CaUngueltig => "mw.ca_ungueltig",
            Self::ZielUnaufloesbar => "mw.ziel_unaufloesbar",
            Self::SocketBelegt => "mw.socket_belegt",
            Self::Intern => "mw.intern",
            Self::Kernabbild => "mw.kernabbild",
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
            Self::ZieleUngueltig => "Zielliste ungueltig",
            Self::CaNichtZugelassen => "Lokale CA nicht zugelassen",
            Self::CaUngueltig => "CA-Datei ungueltig",
            Self::ZielUnaufloesbar => "Ziel nicht aufloesbar",
            Self::SocketBelegt => "Socket belegt oder nicht anlegbar",
            Self::Intern => "interner Fehler",
            Self::Kernabbild => "Kernabbild nicht abschirmbar",
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
            | Self::ZieleUngueltig
            | Self::CaNichtZugelassen
            | Self::CaUngueltig => 78,
            Self::ZielUnaufloesbar => 69,
            Self::SocketBelegt | Self::ZaehlungFehler | Self::ProtokollFehlt => 74,
            Self::Intern => 70,
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
    let (listener, eigener_socket) = socket_anlegen(&a.socket).ok_or(Startfehler::SocketBelegt)?;
    let eigener_socket = Some(eigener_socket);
    let probelauf = zaehlung.is_some();
    let lage = Lage {
        art: optionen,
        sitzung: a.sitzung,
        route: a.route,
        protokoll: a.protokoll,
        schluessel,
        probelauf: zaehlung,
    };
    Ok(Bereit {
        probelauf,
        dienst: Arc::new(Dienst::neu(lage, Box::new(verbinder), uhr)),
        listener,
        socket: a.socket,
        eigener_socket,
    })
}

/// Steps 2 to 7 of mode 2 (section 13.5): target list, approval of the CA, CA, key files of the bound targets,
/// protocol folder, trial run, one resolution per host, socket.
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
    let liste = zielliste::lies(&text).map_err(|_| Startfehler::ZieleUngueltig)?;
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
    for b in liste.ziele.iter().filter_map(|z| z.bindung.as_ref()) {
        schluessel::lies(Path::new(&b.schluessel_datei), eigene_uid).map_err(schluesselfehler)?;
    }
    if !a.protokoll.is_dir() {
        return Err(Startfehler::ProtokollOrdner);
    }
    let zaehlung = probelauf_starten(&a, &uhr)?;
    let mut verbinder: Vec<Option<Box<dyn ziel::Verbinder>>> = Vec::new();
    for z in &liste.ziele {
        let adressen = aufloeser(&z.host, z.port).ok().filter(|v| !v.is_empty());
        let Some(adressen) = adressen else {
            // A bound target without address ends the start; an unbound one blocks nothing (section 13.5).
            if z.bindung.is_some() {
                return Err(Startfehler::ZielUnaufloesbar);
            }
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
    let (listener, eigener_socket) = socket_anlegen(&a.socket).ok_or(Startfehler::SocketBelegt)?;
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
        eigene_uid,
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

/// Step 7: opens the folder of the socket, binds with the umask 0177, then checks the socket. A folder that cannot be
/// opened without following a link (also a link) ends the start before the bind. After the bind the code remembers the
/// socket (kind, device, inode) relative to the open folder, in the same step as it checks the kind (a socket) and the
/// rights (exactly 0600). If anything differs, the socket is deleted again, but only if the name still shows that
/// inode, and the start ends. Whatever already lies at the path stays untouched.
fn socket_anlegen(pfad: &Path) -> Option<(UnixListener, Arc<Eigene>)> {
    os::mit_umask(|| socket_binden(pfad))
}

/// [`socket_anlegen`] without the umask (it is a setting of the whole process, so the tests of this file call this).
fn socket_binden(pfad: &Path) -> Option<(UnixListener, Arc<Eigene>)> {
    socket_binden_mit(pfad, || {})
}

/// [`socket_binden`]; `zwischen` runs between the bind and the check (a seam for the tests, empty in the program).
fn socket_binden_mit(pfad: &Path, zwischen: impl FnOnce()) -> Option<(UnixListener, Arc<Eigene>)> {
    let (ordner, name) = Ordner::von(pfad).ok()?;
    let listener = UnixListener::bind(pfad).ok()?;
    zwischen();
    let eigene = Eigene::merken(ordner, name, rustix::fs::FileType::Socket)?;
    if eigene.modus() != SOCKET_MODUS {
        eigene.entfernen();
        return None;
    }
    Some((listener, Arc::new(eigene)))
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

/// Steps 2 to 9 after the shielding of step 1: [`vorbereiten`], the handlers of SIGTERM and SIGINT, the line of a
/// trial run on stderr and the ready line on stdout. On a failure the stderr line is written and the exit code returned.
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
) -> Result<(Bereit, Signals), u8> {
    let bereit =
        vorbereiten(args, eigene_uid, aufloeser, fristen, uhr).map_err(|f| fehler(stderr, f))?;
    let Ok(signale) = Signals::new([signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT])
    else {
        loeschen_socket(bereit.eigener_socket.as_deref());
        return Err(fehler(stderr, Startfehler::Intern));
    };
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
    Ok((bereit, signale))
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

/// The program: checks its environment (stage 0, before anything else and before any file), shields itself, runs the
/// steps, and serves until SIGTERM or SIGINT. Returns the exit code of a failure before readiness; after readiness the
/// process ends in [`beenden`].
pub fn haupt(args: &[OsString], stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
    if !umgebung_sicher(std::env::var_os) {
        return fehler(stderr, Startfehler::UmgebungUnsicher);
    }
    if let Err(exit) = abschirmen(kernabbild::abschirmen(), stderr) {
        return exit;
    }
    #[cfg(target_os = "linux")]
    if os::todessignal().is_err() {
        return fehler(stderr, Startfehler::Intern);
    }
    if let Some((erstes, rest)) = args.split_first()
        && erstes == "suche"
    {
        return crate::suche::lauf(rest, os::eigene_uid(), crate::suche::PUFFER, stdout, stderr);
    }
    let gestartet = starten(
        args,
        os::eigene_uid(),
        &ziel::system_aufloeser,
        ziel::FRISTEN,
        Box::new(jetzt_ms),
        stdout,
        stderr,
    );
    let (bereit, mut signale) = match gestartet {
        Ok(b) => b,
        Err(exit) => return exit,
    };
    let dienst = Arc::clone(&bereit.dienst);
    let eigener_socket = bereit.eigener_socket.clone();
    let waechter = std::thread::Builder::new().spawn(move || {
        if signale.forever().next().is_some() {
            beenden(dienst.as_ref(), eigener_socket.as_deref());
        }
    });
    if waechter.is_err() {
        loeschen_socket(bereit.eigener_socket.as_deref());
        return fehler(stderr, Startfehler::Intern);
    }
    dienen(&bereit)
}

/// Deletes the socket this process made; nothing if it was not remembered or the name shows something else now. A
/// failure changes no exit code.
fn loeschen_socket(socket: Option<&Eigene>) {
    if let Some(e) = socket {
        e.entfernen();
    }
}

/// The loop over connections; each one gets its own thread.
fn dienen(bereit: &Bereit) -> ! {
    loop {
        let Ok((strom, _)) = bereit.listener.accept() else {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        let faden = Faden::nimm(Arc::clone(&bereit.dienst));
        let _ = std::thread::Builder::new().spawn(move || {
            faden.bediene(strom);
        });
    }
}

/// The end: tear every request down, wait for their lines, delete the socket, exit 0.
fn beenden(dienst: &dyn Server, socket: Option<&Eigene>) -> ! {
    dienst.betrieb().abreissen_alle();
    let _ = dienst.betrieb().warte_leer(ENDE_FRIST);
    loeschen_socket(socket);
    std::process::exit(0)
}

#[cfg(test)]
mod tests {
    use super::{
        Startfehler, UMGEBUNG_UNSICHER, loeschen_socket, socket_binden_mit, umgebung_sicher,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

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
    fn the_line_of_stage_0() {
        let f = Startfehler::UmgebungUnsicher;
        assert_eq!(f.zeile(), "mw.umgebung_unsicher: Umgebung unsicher\n");
        assert_eq!(f.exit(), 78);
    }

    /// A short folder of its own (a socket path is limited to about 100 bytes).
    fn wegwerf(name: &str) -> PathBuf {
        let pfad = std::env::temp_dir().join(format!("mw-st-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("folder");
        pfad
    }

    /// Binds like the program, but gives the socket 0600 in the seam: the umask is a setting of the whole process and
    /// the tests run side by side, so they do not set it (the start of the program does, see `tests/aussen.rs`).
    fn binden(pfad: &std::path::Path) -> Option<(UnixListener, std::sync::Arc<super::Eigene>)> {
        let p = pfad.to_path_buf();
        socket_binden_mit(pfad, move || {
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
    fn rights_other_than_0600_delete_the_own_socket_and_end_the_start() {
        for modus in [0o666, 0o660, 0o700, 0o400, 0o4600] {
            let ordner = wegwerf("rechte");
            let pfad = ordner.join("s");
            let p = pfad.clone();
            let r = socket_binden_mit(&pfad, move || {
                fs::set_permissions(&p, fs::Permissions::from_mode(modus)).expect("chmod");
            });
            assert!(r.is_none(), "{modus:o}");
            assert!(fs::symlink_metadata(&pfad).is_err(), "{modus:o}: deleted");
            let _ = fs::remove_dir_all(&ordner);
        }
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
            let r = socket_binden_mit(&pfad, move || {
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
        let r = socket_binden_mit(&pfad, move || {
            fs::remove_file(&p).expect("remove");
            fs::write(&p, "fremd").expect("write");
            fs::set_permissions(&p, fs::Permissions::from_mode(0o666)).expect("chmod");
        });
        assert!(r.is_none());
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "fremd");
        let _ = fs::remove_dir_all(&ordner);
    }
}
