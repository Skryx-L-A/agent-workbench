//! The run of `rangkern sitzung`: the start in its fixed order, the socket, the connections and the end on a signal.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use formate::kette::{
    Abgewiesen, Gesperrt, Probelauf, anhaengen_abgewiesen, anhaengen_gesperrt_in,
    anhaengen_probelauf, anhaengen_zeile,
};
use rustix::fs::{FileType, Mode, RawMode};
use serde_json::{Value, json};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use super::anfrage::{self, Op};
use super::einst;
use super::konfig::{Konfig, Probelauf as ProbeKonfig, Vektoren};
use super::loeschen::{Eigene, Ordner};
use super::mac::{self, Plattform};
use super::probelauf::{STDERR_ZEILE, Zaehlung, zaehldatei_pruefen};
use super::tor::{Gruppe, Tor};
use super::{BEREIT, EXIT_CONFIG, EXIT_SOCKET};
use crate::cli::hit_json;
use crate::embed::Grund;
use crate::serve::{Line, SearchRequest, Server, bad_request, next_line};
use crate::stamp::now_ns;
use crate::vault::contradiction_pairs;

/// The socket while the process owns it (its folder, name and identity), for the end on a signal.
type SocketPfad = Arc<Mutex<Option<Eigene>>>;

/// Everything a connection needs to answer.
struct Sitzung {
    server: Mutex<Server>,
    tor: Tor,
    route: String,
    kennung: String,
    protokoll: PathBuf,
    /// The counting of the markers, only in the probe-run mode.
    zaehlung: Option<Zaehlung>,
    /// The check of the peer of each connection (macOS only).
    peer: Peerpruefung,
}

/// The check of the peer before the first byte (addendum to contract 4, section 4): the platform, the own euid and the
/// measurement. Under Linux it lets every connection through and measures nothing.
struct Peerpruefung {
    plattform: Plattform,
    euid: u32,
    messen: fn(&UnixStream) -> std::io::Result<u32>,
}

impl Peerpruefung {
    /// The check of this program: its platform, its euid, `getpeereid`.
    fn eigene() -> Self {
        Self {
            plattform: Plattform::AKTUELL,
            euid: mac::eigene_euid(),
            messen: mac::gemessene_euid,
        }
    }

    /// The reason to turn the connection away, or `None` to accept it.
    fn abweisen(&self, strom: &UnixStream) -> Option<&'static str> {
        if self.plattform == Plattform::Linux {
            return None;
        }
        mac::peer_entscheiden(self.plattform, (self.messen)(strom).ok(), self.euid)
    }
}

/// The most connections the process serves at the same time. A fixed number, no option: a further connection is accepted
/// and closed at once without a byte, and the connections that are open stay served.
pub const MAX_VERBINDUNGEN: usize = 16;

/// Removes the socket of the process, if it has made one.
fn socket_loeschen(pfad: &SocketPfad) {
    let eigener = pfad.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(eigener) = eigener {
        // Only through the held folder and only if the name still shows the socket made at the start.
        let _ = eigener.entfernen();
    }
}

/// SIGTERM and SIGINT remove the socket and end the process with exit 0. A thread waits for the signal, so that the handler
/// itself does nothing but the library's flag.
fn signale_anmelden(pfad: SocketPfad) -> std::io::Result<()> {
    let mut signale = Signals::new([SIGTERM, SIGINT])?;
    thread::spawn(move || {
        if signale.forever().next().is_some() {
            socket_loeschen(&pfad);
            std::process::exit(0);
        }
    });
    Ok(())
}

/// Binds the socket `name` in the held folder under the mask 077: the socket exists from the first moment with no right for
/// group and others. The mask holds for the whole process; every file it makes later is 0600 or narrower anyway.
fn socket_binden(ordner: &Ordner, name: &OsStr, pfad: &Path) -> std::io::Result<UnixListener> {
    // Anything at the name, even a link that leads nowhere, makes it taken. `bind` alone is not enough: on macOS it follows a
    // dangling link and makes the socket at the link's target (measured 20.11.2026). The look goes through the held folder.
    if ordner.kennung(name).is_some() {
        return Err(std::io::Error::from(std::io::ErrorKind::AddrInUse));
    }
    rustix::process::umask(Mode::from_raw_mode(0o077));
    binden_im_ordner(ordner, name, pfad)
}

/// Under Linux the socket is bound through the held folder itself: `/proc/self/fd/<n>/<name>` names exactly that folder, so no
/// swap of a folder on the path can come between the check and the `bind` (review B6). Without `/proc` the start fails with 74,
/// as the change of the rights does (`loeschen::Ordner::rechte_setzen_wenn`).
#[cfg(target_os = "linux")]
fn binden_im_ordner(ordner: &Ordner, name: &OsStr, _pfad: &Path) -> std::io::Result<UnixListener> {
    use std::os::fd::{AsFd, AsRawFd};
    let mut weg =
        std::ffi::OsString::from(format!("/proc/self/fd/{}/", ordner.as_fd().as_raw_fd()));
    weg.push(name);
    UnixListener::bind(weg)
}

/// Elsewhere there is no `bindat`: `bind` takes the path. The folder the path names must be the held folder right before and
/// right after the `bind` (device and inode, review B6); otherwise the start fails with 74.
#[cfg(not(target_os = "linux"))]
fn binden_im_ordner(ordner: &Ordner, _name: &OsStr, pfad: &Path) -> std::io::Result<UnixListener> {
    derselbe_ordner(ordner, pfad)?;
    let listener = UnixListener::bind(pfad)?;
    derselbe_ordner(ordner, pfad)?;
    Ok(listener)
}

/// Whether the parent folder of `pfad`, not followed if it is a link, is the held folder (device and inode).
#[cfg(not(target_os = "linux"))]
fn derselbe_ordner(ordner: &Ordner, pfad: &Path) -> std::io::Result<()> {
    let eltern = match pfad.parent() {
        Some(eltern) if !eltern.as_os_str().is_empty() => eltern,
        _ => Path::new("."),
    };
    let gehalten = rustix::fs::fstat(ordner)?;
    let jetzt = rustix::fs::lstat(eltern)?;
    if (gehalten.st_dev, gehalten.st_ino) == (jetzt.st_dev, jetzt.st_ino) {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "the folder of the socket path is no longer the folder that was checked",
        ))
    }
}

/// The check of the held parent folder (section 3 of the macOS addendum; under Linux too since review B7): `fstat` on the
/// descriptor that the rights, the `bind` (Linux) and the removal use later, never a second look through the path (review B6).
#[allow(clippy::useless_conversion)] // `mode_t` is u16 under macOS and u32 under Linux
fn ordner_halten(
    pfad: &Path,
    plattform: Plattform,
    euid: u32,
) -> std::io::Result<(Ordner, std::ffi::OsString)> {
    if plattform == Plattform::MacOs
        && pfad
            .parent()
            .is_none_or(|eltern| eltern.as_os_str().is_empty())
    {
        return Err(std::io::Error::other("the socket has no parent folder"));
    }
    let (ordner, name) = Ordner::von(pfad)?;
    let st = rustix::fs::fstat(&ordner)?;
    mac::ordner_regel(
        FileType::from_raw_mode(st.st_mode) == FileType::Directory,
        st.st_uid,
        u32::from(st.st_mode),
        euid,
    )
    .map_err(std::io::Error::other)?;
    Ok((ordner, name))
}

/// Makes the socket with the given rights. The parent folder is opened once (no link) and checked through that descriptor
/// first (exit 74 on a failure, the path unchanged). Nothing at the path is touched if the path is taken: the start fails
/// then. A socket whose rights cannot be set is removed again. The start goes through [`socket_anlegen_gemerkt`].
#[cfg(test)]
fn socket_anlegen(
    pfad: &Path,
    rechte: u32,
    plattform: Plattform,
    euid: u32,
) -> std::io::Result<(UnixListener, Eigene)> {
    socket_anlegen_mit(pfad, rechte, plattform, euid, &|| {})
}

/// [`socket_anlegen`] with a seam right after the `bind`, before the socket is remembered (for the test of review B5).
fn socket_anlegen_mit(
    pfad: &Path,
    rechte: u32,
    plattform: Plattform,
    euid: u32,
    nach_bind: &dyn Fn(),
) -> std::io::Result<(UnixListener, Eigene)> {
    // The rights as `Mode` before anything is made (fail closed): `mode_t` is u32 under Linux and u16 under macOS. A value
    // that does not fit, or has a bit beyond the nine rights bits, is refused, and the path stays as it is. The bits are
    // checked here because `Mode::from_bits` takes every value (rustix declares all bits as known, `const _ = !0`).
    let modus = RawMode::try_from(rechte)
        .ok()
        .filter(|roh| roh & !0o777 == 0)
        .map(Mode::from_raw_mode)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // The folder is opened once and held to the end: the check, the rights and the removal go through it, never through the
    // path.
    let (ordner, name) = ordner_halten(pfad, plattform, euid)?;
    let listener = socket_binden(&ordner, &name, pfad)?;
    nach_bind();
    let eigene = Eigene::merken(ordner, name, FileType::Socket)
        .ok_or_else(|| std::io::Error::other("the socket path does not show the new socket"))?;
    eigene.rechte_setzen(modus).inspect_err(|_| {
        let _ = eigene.entfernen();
    })?;
    Ok((listener, eigene))
}

/// Makes the socket and remembers it for the end on a signal under one lock (review B5): the signal thread takes the same lock
/// in `socket_loeschen`, so a SIGTERM between the `bind` and the remembering waits and then removes the socket.
fn socket_anlegen_gemerkt(
    gemerkt: &SocketPfad,
    pfad: &Path,
    rechte: u32,
    plattform: Plattform,
    euid: u32,
    nach_bind: &dyn Fn(),
) -> std::io::Result<UnixListener> {
    let mut eintrag = gemerkt.lock().unwrap_or_else(PoisonError::into_inner);
    let (listener, eigene) = socket_anlegen_mit(pfad, rechte, plattform, euid, nach_bind)?;
    *eintrag = Some(eigene);
    Ok(listener)
}

/// A place among the open connections: it counts as long as it lives.
struct Platz(Arc<AtomicUsize>);

impl Platz {
    /// Takes a place, or `None` if `MAX_VERBINDUNGEN` connections are open.
    fn nehmen(offen: &Arc<AtomicUsize>) -> Option<Self> {
        if offen.fetch_add(1, Ordering::SeqCst) >= MAX_VERBINDUNGEN {
            offen.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self(Arc::clone(offen)))
    }
}

impl Drop for Platz {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The start after the arguments are valid: settings, route, log folder, count file (probe run), socket, notes, readiness line, then the answers.
/// Returns an exit code only when the start fails; after the readiness line only a signal ends the process (review B9: a failed
/// `accept` is reported and the loop goes on).
pub fn starten<O: Write, E: Write>(konfig: Konfig, out: &mut O, err: &mut E) -> u8 {
    let pfad: SocketPfad = Arc::default();
    if let Err(fehler) = signale_anmelden(Arc::clone(&pfad)) {
        let _ = writeln!(err, "rangkern sitzung: signals: {fehler}");
        return EXIT_SOCKET;
    }
    let einst = match einst::lesen_datei(&konfig.einstellungen) {
        Ok(einst) => einst,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: settings: {fehler}");
            return EXIT_CONFIG;
        }
    };
    if !einst.routen.contains(&konfig.route) {
        let _ = writeln!(err, "rangkern sitzung: the route is not in the settings");
        return EXIT_CONFIG;
    }
    if !konfig.protokoll.is_dir() {
        let _ = writeln!(err, "rangkern sitzung: the log folder is not a folder");
        return EXIT_CONFIG;
    }
    // The count file belongs to 78, and 78 comes before 74: it is checked before the socket (W3).
    if let Some(probe) = &konfig.probelauf
        && let Err(fehler) = zaehldatei_pruefen(&probe.zaehlung)
    {
        let _ = writeln!(err, "rangkern sitzung: probe run: {fehler}");
        return EXIT_CONFIG;
    }
    let listener = match socket_anlegen_gemerkt(
        &pfad,
        &konfig.socket,
        konfig.rechte,
        Plattform::AKTUELL,
        mac::eigene_euid(),
        &|| {},
    ) {
        Ok(listener) => listener,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: socket: {fehler}");
            return EXIT_SOCKET;
        }
    };
    let zaehlung = match probelauf_starten(&konfig, err) {
        Ok(zaehlung) => zaehlung,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: probe run: {fehler}");
            socket_loeschen(&pfad);
            return EXIT_CONFIG;
        }
    };
    let server = match konfig.vektoren {
        Vektoren::Datei(datei) => Server::start(konfig.vault, datei),
        Vektoren::Datenbanken(dbs) => Server::start_dbs(konfig.vault, dbs),
    }
    .with_embedder(konfig.embedder);
    let sitzung = Arc::new(Sitzung {
        server: Mutex::new(server),
        tor: Tor::new(einst.vorgabe, konfig.route.clone()),
        route: konfig.route,
        kennung: konfig.sitzung,
        protokoll: konfig.protokoll,
        zaehlung,
        peer: Peerpruefung::eigene(),
    });
    // The starter may start the harness only after this line. A closed pipe leaves nobody to tell.
    let _ = out.write_all(BEREIT).and_then(|()| out.flush());
    let offen = Arc::new(AtomicUsize::new(0));
    let mut folge = Annahmefehler::default();
    // The loop never ends: the process ends only through a signal (`signale_anmelden`).
    loop {
        let strom = match listener.accept() {
            Ok((strom, _)) => strom,
            Err(fehler) => {
                folge.fehler(&fehler, err);
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        folge.erfolg();
        // Over the limit the stream is dropped here: closed without a byte.
        let Some(platz) = Platz::nehmen(&offen) else {
            continue;
        };
        let sitzung = Arc::clone(&sitzung);
        thread::spawn(move || {
            let _platz = platz;
            verbindung(strom, &sitzung);
        });
    }
}

/// A run of failed `accept` calls (review B9): the first failure of a run goes to the error output, one line without the content
/// of anything; the next ones are silent until an `accept` works again. So a lasting failure (`EMFILE`) is reported once, not
/// 20 times a second.
#[derive(Debug, Default)]
struct Annahmefehler {
    in_folge: bool,
}

impl Annahmefehler {
    fn fehler<E: Write>(&mut self, fehler: &std::io::Error, err: &mut E) {
        if !self.in_folge {
            let _ = writeln!(err, "rangkern sitzung: accept: {fehler}");
        }
        self.in_folge = true;
    }

    fn erfolg(&mut self) {
        self.in_folge = false;
    }
}

/// The start of the probe-run mode: the file of the counts with the count 0 first, then the log line `probelauf.zaehlung`, then
/// the fixed line on the error output. `None` without the mode. A failure of the file or of the log line ends the start (the
/// caller removes the socket); a failed log line removes the file again, so nothing of the failed start stays (review B8). The
/// line on the error output comes after the log line and its failure ends nothing: the line is in the chain then, and a closed
/// error output leaves nobody to tell (review B8: no failed start with a line in the chain).
fn probelauf_starten<E: Write>(konfig: &Konfig, err: &mut E) -> std::io::Result<Option<Zaehlung>> {
    let Some(ProbeKonfig { marker, zaehlung }) = &konfig.probelauf else {
        return Ok(None);
    };
    let zaehlung = Zaehlung::anlegen(zaehlung.clone(), konfig.sitzung.clone(), marker.clone())?;
    if let Err(fehler) =
        probelauf_zeile(&konfig.protokoll, &konfig.sitzung, zaehlung.anzahl_marker())
    {
        let _ = zaehlung.entfernen();
        return Err(fehler);
    }
    let _ = writeln!(err, "{STDERR_ZEILE}");
    Ok(Some(zaehlung))
}

/// The log line `probelauf.zaehlung` of the start: `anhaengen_probelauf` of `formate` writes it into the log of the month.
fn probelauf_zeile(ordner: &Path, sitzung: &str, anzahl: usize) -> std::io::Result<()> {
    let zeile = Probelauf {
        zeit_ms: jetzt_ms(),
        programm: "suche".to_owned(),
        sitzung: sitzung.to_owned(),
        anzahl: u32::try_from(anzahl).unwrap_or(u32::MAX),
    };
    anhaengen_probelauf(ordner, "brain", &zeile)
        .map(|_| ())
        .map_err(|fehler| std::io::Error::other(fehler.code()))
}

/// One connection: the answers come in the order of the requests; the end of the connection ends nothing else.
fn verbindung(strom: UnixStream, sitzung: &Sitzung) {
    // Before the first read and the first write: a peer that is not ours gets no byte, and the connection ends.
    if let Some(grund) = sitzung.peer.abweisen(&strom) {
        sitzung.abgewiesen(grund);
        return;
    }
    let Ok(lesestrom) = strom.try_clone() else {
        return;
    };
    let mut leser = BufReader::new(lesestrom);
    let mut schreiber = strom;
    while let Ok(Some(zeile)) = next_line(&mut leser) {
        let bytes = sitzung.antworten(&zeile);
        if schreiber.write_all(&bytes).is_err() {
            break;
        }
    }
}

/// The current time in milliseconds since 1970.
fn jetzt_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|dauer| i64::try_from(dauer.as_millis()).ok())
        .unwrap_or_default()
}

/// The buffer of an answer: one JSON line with its line feed.
fn zeile_von(antwort: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(antwort).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The answer with the `id` of the request, if it had one.
fn mit_id(mut antwort: Value, id: Option<Value>) -> Value {
    if let (Some(id), Some(objekt)) = (id, antwort.as_object_mut()) {
        objekt.insert("id".to_owned(), id);
    }
    antwort
}

impl Sitzung {
    fn server(&self) -> MutexGuard<'_, Server> {
        self.server.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The answer to one line, as the buffer that goes to the socket: one JSON line with its line feed.
    fn antworten(&self, zeile: &Line) -> Vec<u8> {
        if zeile.too_long {
            return zeile_von(&bad_request(None));
        }
        let anfrage = match anfrage::lesen(&zeile.bytes) {
            Ok(anfrage) => anfrage,
            Err(id) => return zeile_von(&bad_request(id)),
        };
        match anfrage.op {
            Op::Ping => zeile_von(&mit_id(json!({"ok": true}), anfrage.id)),
            Op::Search { query, k, space } => self.suchen(query, k, space, anfrage.id),
        }
    }

    /// A search: the whole list, the gate, then the counting of the markers (probe run only), then the log lines for the
    /// blocked hits, then the line `suche.rueckfall` when the embedding of `--embed` failed, then the buffer of the answer.
    /// If the counting or a line of a blocked hit cannot be written, the answer is `{"error":"protokoll"}` with no hit; a
    /// failed counting writes no log line. A failed line `suche.rueckfall` changes nothing of the answer.
    fn suchen(&self, query: String, k: usize, space: Option<String>, id: Option<Value>) -> Vec<u8> {
        let mut server = self.server();
        // The whole list: the prefix property makes `L[..k]` the answer of `rangkern serve` for `k`.
        let frage = SearchRequest {
            query,
            k: usize::MAX,
            space,
            today: None,
            query_vector: None,
        };
        let (antwort, gruppen, ausfall) = match server.search_hits_ausfall(&frage, now_ns()) {
            Err(_) => (json!({"error": "unreadable"}), Vec::new(), None),
            Ok((liste, fallback, ausfall)) => {
                let notes: HashMap<_, _> = server.notes_by_rel();
                let paare = contradiction_pairs(server.root());
                let durch = self.tor.durchlassen(liste, k, &notes, &paare);
                let antwort = json!({
                    "fallback": fallback,
                    "hits": durch.hits.iter().map(hit_json).collect::<Vec<_>>(),
                    "gesperrt": durch.gesperrt(),
                });
                (antwort, durch.gruppen, ausfall)
            }
        };
        let puffer = zeile_von(&mit_id(antwort, id.clone()));
        let gezaehlt = self
            .zaehlung
            .as_ref()
            .is_none_or(|zaehlung| zaehlung.zaehlen(&puffer).is_ok());
        if !gezaehlt || self.protokollieren(&gruppen).is_err() {
            return zeile_von(&mit_id(json!({"error": "protokoll"}), id));
        }
        if let Some(grund) = ausfall {
            self.rueckfall(grund);
        }
        puffer
    }

    /// The log line `suche.rueckfall` (contract 5, section 6.5): the search answers lexically because the embedding of
    /// `--embed` failed. A call of its own after the lines of the blocked hits. A failure is dropped: no second try, no
    /// buffer, no text on the error output, no field in the answer (contract 4, "Rückfall der Einbettung").
    fn rueckfall(&self, grund: Grund) {
        let _ = anhaengen_zeile(
            &self.protokoll,
            "brain",
            jetzt_ms(),
            &json!({
                "aktionsname": "suche.rueckfall",
                "ergebnis": "lexikalisch",
                "grund": grund.text(),
                "route": self.route,
                "sitzung": self.kennung,
            }),
        );
    }

    /// The log line `kanal.abgewiesen` for a connection turned away: channel `such`, route and session from the start, no uid.
    /// If it cannot be written, one line without content goes to the error output; the process goes on.
    fn abgewiesen(&self, grund: &str) {
        let zeile = Abgewiesen {
            zeit_ms: jetzt_ms(),
            grund: grund.to_owned(),
            kanal: "such".to_owned(),
            route: Some(self.route.clone()),
            sitzung: Some(self.kennung.clone()),
        };
        if anhaengen_abgewiesen(&self.protokoll, "brain", &zeile).is_err() {
            let _ = writeln!(
                std::io::stderr(),
                "rangkern sitzung: kanal.abgewiesen: the line could not be written"
            );
        }
    }

    /// Writes one line per group to the chain `brain`, with the time of now. Every line of the process goes through the
    /// functions with folder and source, which lock the source (contract 5, section 5.3: "beide Wege nie mischen"); until
    /// 23.11.2026 these lines locked only the file of the month (`anhaengen_gesperrt`), while the other lines locked the source.
    fn protokollieren(&self, gruppen: &[Gruppe]) -> Result<(), formate::kette::Fehler> {
        let zeit_ms = jetzt_ms();
        for gruppe in gruppen {
            anhaengen_gesperrt_in(
                &self.protokoll,
                "brain",
                &Gesperrt {
                    zeit_ms,
                    route: self.route.clone(),
                    sitzung: self.kennung.clone(),
                    grund: gruppe.grund.to_owned(),
                    klasse: gruppe.klasse.clone(),
                    kunde: gruppe.kunde.clone(),
                    anzahl: gruppe.anzahl,
                },
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    fn ordner(name: &str) -> PathBuf {
        let ordner = PathBuf::from(format!("/tmp/rkl-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&ordner);
        fs::create_dir_all(&ordner).expect("folder is made");
        ordner
    }

    /// Runs in a process of its own: the mask holds for the whole process, and a test in a parallel thread of this binary could
    /// change it between the `umask` of the test and the `bind`. The test starts its own test binary with only this test and the
    /// marker `RK_UMASK_KIND`; the child does the check. "1 passed" shows that the child ran it and did not filter it away.
    #[test]
    fn the_socket_has_no_right_for_group_and_others_right_after_the_bind() {
        const NAME: &str = "sitzung::lauf::tests::the_socket_has_no_right_for_group_and_others_right_after_the_bind";
        if std::env::var_os("RK_UMASK_KIND").is_none() {
            let aus = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args(["--exact", NAME, "--test-threads=1"])
                .env("RK_UMASK_KIND", "1")
                .output()
                .expect("the child test runs");
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&aus.stdout),
                String::from_utf8_lossy(&aus.stderr)
            );
            assert!(aus.status.success(), "{text}");
            assert!(text.contains("1 passed"), "the child ran the test: {text}");
            return;
        }
        let ordner = ordner("umask");
        // A loose mask, as a caller may have it: only the mask of the process makes the socket narrow.
        rustix::process::umask(Mode::from_raw_mode(0o000));
        let pfad = ordner.join("s.sock");
        let (gehalten, name) = Ordner::von(&pfad).expect("the folder is opened");
        let _listener = socket_binden(&gehalten, &name, &pfad).expect("the socket is bound");
        let modus = fs::symlink_metadata(ordner.join("s.sock"))
            .expect("the socket")
            .mode();
        assert_eq!(modus & 0o077, 0, "mode {modus:o} before the chmod");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn places_are_taken_up_to_the_limit_and_freed_on_drop() {
        let offen = Arc::new(AtomicUsize::new(0));
        let mut plaetze: Vec<Platz> = (0..MAX_VERBINDUNGEN)
            .map(|_| Platz::nehmen(&offen).expect("a place"))
            .collect();
        assert_eq!(MAX_VERBINDUNGEN, 16);
        assert!(Platz::nehmen(&offen).is_none(), "the 17th has no place");
        assert_eq!(offen.load(Ordering::SeqCst), MAX_VERBINDUNGEN);
        plaetze.pop();
        assert_eq!(offen.load(Ordering::SeqCst), MAX_VERBINDUNGEN - 1);
        assert!(Platz::nehmen(&offen).is_some(), "a freed place is free");
    }

    /// A session for the tests of the peer check: an empty vault, a route `cloud`, the log folder `protokoll` below `ordner`.
    fn sitzung_fuer(ordner: &Path, peer: Peerpruefung) -> Sitzung {
        fs::create_dir_all(ordner.join("vault")).expect("vault");
        fs::create_dir_all(ordner.join("protokoll")).expect("log folder");
        let einst_datei = ordner.join("einst.json");
        fs::write(
            &einst_datei,
            r#"{"fassung":1,"klassen":["oeffentlich","intern"],"vorgabestufe":"intern","routen":[{"schluessel":"cloud","art":"cloud","klasse":"intern"}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#,
        )
        .expect("settings");
        let einst = einst::lesen_datei(&einst_datei).expect("settings are read");
        Sitzung {
            server: Mutex::new(Server::start(ordner.join("vault"), None)),
            tor: Tor::new(einst.vorgabe, "cloud".to_owned()),
            route: "cloud".to_owned(),
            kennung: "s-1".to_owned(),
            protokoll: ordner.join("protokoll"),
            zaehlung: None,
            peer,
        }
    }

    const FREMD: u32 = 4242;
    const PING: &str = "{\"op\":\"ping\"}\n";

    #[allow(clippy::unnecessary_wraps)] // the type of the measurement is fixed by the check
    fn messen_fremd(_: &UnixStream) -> std::io::Result<u32> {
        Ok(FREMD)
    }

    #[allow(clippy::unnecessary_wraps)] // the type of the measurement is fixed by the check
    fn messen_eigen(_: &UnixStream) -> std::io::Result<u32> {
        Ok(mac::eigene_euid())
    }

    fn messen_scheitert(_: &UnixStream) -> std::io::Result<u32> {
        Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
    }

    fn mac_peer(messen: fn(&UnixStream) -> std::io::Result<u32>) -> Peerpruefung {
        Peerpruefung {
            plattform: Plattform::MacOs,
            euid: mac::eigene_euid(),
            messen,
        }
    }

    /// One connection of a client that sends `anfrage` and its end: what the process sends back.
    fn verbinden(sitzung: &Sitzung, anfrage: &str) -> Vec<u8> {
        use std::io::Read;
        use std::net::Shutdown;
        let (server, mut client) = UnixStream::pair().expect("pair");
        client.write_all(anfrage.as_bytes()).expect("request");
        client
            .shutdown(Shutdown::Write)
            .expect("end of the request");
        verbindung(server, sitzung);
        let mut antwort = Vec::new();
        // A connection closed with the request unread may end in a reset instead of a plain end: still no byte.
        match client.read_to_end(&mut antwort) {
            Ok(_) => {}
            Err(fehler) if fehler.kind() == std::io::ErrorKind::ConnectionReset => {}
            Err(fehler) => panic!("answer: {fehler}"),
        }
        antwort
    }

    /// The lines of the log of the month in the log folder.
    fn zeilen(ordner: &Path) -> Vec<Value> {
        let mut alle = Vec::new();
        for eintrag in fs::read_dir(ordner.join("protokoll")).expect("log folder") {
            let text = fs::read_to_string(eintrag.expect("entry").path()).expect("log");
            alle.extend(text.lines().map(|z| serde_json::from_str(z).expect("JSON")));
        }
        alle
    }

    #[test]
    fn a_foreign_peer_gets_no_byte_and_one_line_without_uid_and_the_process_goes_on() {
        let ordner = ordner("peer-fremd");
        let fremd = sitzung_fuer(&ordner, mac_peer(messen_fremd));
        assert_eq!(verbinden(&fremd, PING), b"");
        assert_eq!(verbinden(&fremd, PING), b"");
        let alle = zeilen(&ordner);
        assert_eq!(alle.len(), 2, "one line per turned-away connection");
        for zeile in &alle {
            let objekt = zeile.as_object().expect("object");
            let mut felder: Vec<&str> = objekt.keys().map(String::as_str).collect();
            felder.sort_unstable();
            assert_eq!(
                felder,
                [
                    "aktionsname",
                    "ergebnis",
                    "fassung",
                    "grund",
                    "kanal",
                    "route",
                    "sitzung",
                    "vorher",
                    "zeit"
                ]
            );
            assert_eq!(zeile["aktionsname"], "kanal.abgewiesen");
            assert_eq!(zeile["grund"], "such.kennung_fremd");
            assert_eq!(zeile["kanal"], "such");
            assert_eq!(zeile["route"], "cloud");
            assert_eq!(zeile["sitzung"], "s-1");
            assert!(!zeile.to_string().contains(&FREMD.to_string()), "no uid");
        }
        // The next connection of the own user is answered: the process went on.
        let eigen = sitzung_fuer(&ordner, mac_peer(messen_eigen));
        assert_eq!(
            String::from_utf8(verbinden(&eigen, PING)).expect("text"),
            "{\"ok\":true}\n"
        );
        assert_eq!(zeilen(&ordner).len(), 2, "an accepted peer writes no line");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_peer_that_cannot_be_measured_gets_no_byte_and_a_line_with_its_reason() {
        let ordner = ordner("peer-unmessbar");
        let sitzung = sitzung_fuer(&ordner, mac_peer(messen_scheitert));
        assert_eq!(verbinden(&sitzung, PING), b"");
        let alle = zeilen(&ordner);
        assert_eq!(alle.len(), 1);
        assert_eq!(alle[0]["grund"], "such.kennung_unmessbar");
        assert_eq!(alle[0]["kanal"], "such");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_line_that_cannot_be_written_still_turns_the_peer_away_and_the_process_goes_on() {
        let ordner = ordner("peer-zeile-scheitert");
        let sitzung = sitzung_fuer(&ordner, mac_peer(messen_fremd));
        fs::remove_dir_all(ordner.join("protokoll")).expect("remove the log folder");
        assert_eq!(verbinden(&sitzung, PING), b"");
        let eigen = sitzung_fuer(&ordner, mac_peer(messen_eigen));
        assert_eq!(
            String::from_utf8(verbinden(&eigen, PING)).expect("text"),
            "{\"ok\":true}\n"
        );
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn linux_measures_nothing_and_turns_nobody_away() {
        let ordner = ordner("peer-linux");
        let linux = Peerpruefung {
            plattform: Plattform::Linux,
            euid: mac::eigene_euid(),
            messen: messen_scheitert,
        };
        let sitzung = sitzung_fuer(&ordner, linux);
        assert_eq!(
            String::from_utf8(verbinden(&sitzung, PING)).expect("text"),
            "{\"ok\":true}\n"
        );
        assert_eq!(zeilen(&ordner).len(), 0);
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_parent_folder_that_is_not_0700_ends_the_socket_before_bind_under_macos() {
        let ordner = ordner("ordner-0755");
        let eltern = ordner.join("s");
        fs::create_dir_all(&eltern).expect("parent");
        fs::set_permissions(&eltern, fs::Permissions::from_mode(0o755)).expect("mode");
        let euid = mac::eigene_euid();
        assert!(socket_anlegen(&eltern.join("such.sock"), 0o600, Plattform::MacOs, euid).is_err());
        assert!(
            !eltern.join("such.sock").exists(),
            "the path stays as it is"
        );
        fs::set_permissions(&eltern, fs::Permissions::from_mode(0o700)).expect("mode");
        assert!(socket_anlegen(&eltern.join("such.sock"), 0o600, Plattform::MacOs, euid).is_ok());
        let _ = fs::remove_dir_all(&ordner);
    }

    /// Review B5: a SIGTERM between the `bind` and the remembering of the socket. The seam after the `bind` starts what the signal
    /// thread does (`socket_loeschen`) and gives it time; it must wait for the lock and then remove the socket. Without the lock
    /// around the making it would find nothing, and the socket would stay.
    #[test]
    fn a_signal_right_after_the_bind_waits_for_the_socket_and_then_removes_it() {
        use std::cell::RefCell;
        use std::sync::mpsc;
        let ordner = ordner("b5-signal");
        fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("mode");
        let pfad = ordner.join("s.sock");
        let gemerkt: SocketPfad = Arc::default();
        let faden: RefCell<Option<thread::JoinHandle<()>>> = RefCell::new(None);
        let nach_bind = || {
            let gemerkt = Arc::clone(&gemerkt);
            let (los, gestartet) = mpsc::channel();
            *faden.borrow_mut() = Some(thread::spawn(move || {
                let _ = los.send(());
                socket_loeschen(&gemerkt);
            }));
            gestartet.recv().expect("the signal thread runs");
            thread::sleep(Duration::from_millis(300));
            assert!(
                fs::symlink_metadata(&pfad).is_ok(),
                "the socket stands while it is not yet remembered"
            );
        };
        let listener = socket_anlegen_gemerkt(
            &gemerkt,
            &pfad,
            0o600,
            Plattform::AKTUELL,
            mac::eigene_euid(),
            &nach_bind,
        )
        .expect("socket");
        faden
            .borrow_mut()
            .take()
            .expect("the seam ran")
            .join()
            .expect("the signal thread ends");
        assert!(
            fs::symlink_metadata(&pfad).is_err(),
            "the signal removed the socket once it was remembered"
        );
        drop(listener);
        let _ = fs::remove_dir_all(&ordner);
    }

    /// Review B6: the folder is swapped after it was opened and checked. Under Linux the `bind` goes through the held folder,
    /// so the socket lands there and not in the new folder at the path. Elsewhere the comparison of device and inode refuses
    /// the `bind`, and neither folder gets a socket. The positive control: without a swap the socket is made at the path.
    #[test]
    fn a_folder_swapped_after_the_check_gets_no_socket_at_the_path() {
        let ordner = ordner("b6-tausch");
        let a = ordner.join("a");
        fs::create_dir_all(&a).expect("folder");
        let pfad = a.join("s.sock");
        let (gehalten, name) = Ordner::von(&pfad).expect("the folder is opened");
        assert!(
            socket_binden(&gehalten, &name, &pfad).is_ok(),
            "no swap: bound"
        );
        assert!(fs::symlink_metadata(&pfad).is_ok());
        fs::remove_file(&pfad).expect("remove");
        fs::rename(&a, ordner.join("weg")).expect("move the held folder away");
        fs::create_dir_all(&a).expect("a new folder at the path");
        let ergebnis = socket_binden(&gehalten, &name, &pfad);
        assert!(
            fs::symlink_metadata(&pfad).is_err(),
            "no socket in the new folder at the path"
        );
        if cfg!(target_os = "linux") {
            assert!(ergebnis.is_ok(), "bound in the held folder");
            assert!(fs::symlink_metadata(ordner.join("weg/s.sock")).is_ok());
        } else {
            assert!(ergebnis.is_err(), "the swap is refused");
            assert!(fs::symlink_metadata(ordner.join("weg/s.sock")).is_err());
        }
        let _ = fs::remove_dir_all(&ordner);
    }

    /// Review B7: the rule for the parent folder holds under Linux as under macOS, also with the rights 0666. The positive
    /// control: the same call with a folder 0700 makes the socket.
    #[test]
    fn under_linux_a_parent_folder_that_is_not_0700_ends_the_socket_also_with_0666() {
        let ordner = ordner("b7-linux");
        let eltern = ordner.join("s");
        fs::create_dir_all(&eltern).expect("parent");
        let euid = mac::eigene_euid();
        for modus in [0o755, 0o750, 0o777] {
            fs::set_permissions(&eltern, fs::Permissions::from_mode(modus)).expect("mode");
            for rechte in [0o600, 0o666] {
                assert!(
                    socket_anlegen(&eltern.join("such.sock"), rechte, Plattform::Linux, euid)
                        .is_err(),
                    "{modus:o} {rechte:o}"
                );
                assert!(
                    !eltern.join("such.sock").exists(),
                    "the path stays as it is"
                );
            }
        }
        // Another owner: the euid of the process is not the owner of the folder.
        fs::set_permissions(&eltern, fs::Permissions::from_mode(0o700)).expect("mode");
        assert!(
            socket_anlegen(
                &eltern.join("such.sock"),
                0o666,
                Plattform::Linux,
                euid.wrapping_add(1)
            )
            .is_err()
        );
        let (_listener, eigene) =
            socket_anlegen(&eltern.join("such.sock"), 0o666, Plattform::Linux, euid)
                .expect("a folder 0700 of the own user");
        assert!(eigene.entfernen());
        let _ = fs::remove_dir_all(&ordner);
    }

    /// A start of the probe run for the tests of review B8: the count file in `ordner/zaehl`, the log folder `protokoll`.
    fn probe_konfig(ordner: &Path, protokoll: PathBuf) -> Konfig {
        fs::create_dir_all(ordner.join("zaehl")).expect("folder");
        Konfig {
            vault: ordner.join("vault"),
            socket: ordner.join("s.sock"),
            rechte: 0o600,
            route: "cloud".to_owned(),
            sitzung: "s-1".to_owned(),
            einstellungen: ordner.join("einst.json"),
            protokoll,
            vektoren: Vektoren::Datei(None),
            embedder: None,
            probelauf: Some(ProbeKonfig {
                marker: vec!["MI-1A".to_owned()],
                zaehlung: ordner.join("zaehl/zaehlung.json"),
            }),
        }
    }

    /// An error output that fails every write.
    struct Kaputt;

    impl Write for Kaputt {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }

    /// Review B8: the log line `probelauf.zaehlung` fails (the log folder is a file); the start fails, and the count file with
    /// the count 0 does not stay. The positive control: with a log folder the file stays and the line stands in the log.
    #[test]
    fn a_failed_log_line_of_the_probe_run_removes_the_count_file_again() {
        let ordner = ordner("b8-zeile");
        fs::write(ordner.join("kein-ordner"), "x").expect("a file");
        let konfig = probe_konfig(&ordner, ordner.join("kein-ordner"));
        let mut err = Vec::new();
        assert!(probelauf_starten(&konfig, &mut err).is_err());
        assert!(
            fs::symlink_metadata(ordner.join("zaehl/zaehlung.json")).is_err(),
            "the count file is removed"
        );
        assert_eq!(
            fs::read_dir(ordner.join("zaehl")).expect("folder").count(),
            0
        );
        assert!(
            err.is_empty(),
            "no line on the error output for a failed start"
        );
        fs::create_dir_all(ordner.join("protokoll")).expect("log folder");
        let konfig = probe_konfig(&ordner, ordner.join("protokoll"));
        assert!(
            probelauf_starten(&konfig, &mut err)
                .expect("start")
                .is_some()
        );
        assert!(ordner.join("zaehl/zaehlung.json").exists());
        assert_eq!(zeilen(&ordner).len(), 1);
        assert_eq!(
            String::from_utf8(err).expect("text"),
            format!("{STDERR_ZEILE}\n")
        );
        let _ = fs::remove_dir_all(&ordner);
    }

    /// Review B8: a failed line on the error output does not fail the start: the line in the chain belongs to a start that goes
    /// on.
    #[test]
    fn a_failed_line_on_the_error_output_does_not_fail_the_probe_run() {
        let ordner = ordner("b8-stderr");
        fs::create_dir_all(ordner.join("protokoll")).expect("log folder");
        let konfig = probe_konfig(&ordner, ordner.join("protokoll"));
        assert!(
            probelauf_starten(&konfig, &mut Kaputt)
                .expect("start")
                .is_some()
        );
        let alle = zeilen(&ordner);
        assert_eq!(alle.len(), 1);
        assert_eq!(alle[0]["aktionsname"], "probelauf.zaehlung");
        assert!(ordner.join("zaehl/zaehlung.json").exists());
        let _ = fs::remove_dir_all(&ordner);
    }

    /// Review B9: the first failed `accept` of a run is reported, the next ones are silent, and after a working `accept` the
    /// next failure is reported again.
    #[test]
    fn the_first_failed_accept_of_a_run_is_reported_once() {
        let mut folge = Annahmefehler::default();
        let mut err = Vec::new();
        let emfile = std::io::Error::other("too many open files");
        folge.fehler(&emfile, &mut err);
        folge.fehler(&emfile, &mut err);
        folge.fehler(&emfile, &mut err);
        assert_eq!(
            String::from_utf8(err.clone()).expect("text"),
            "rangkern sitzung: accept: too many open files\n"
        );
        folge.erfolg();
        folge.fehler(&emfile, &mut err);
        assert_eq!(String::from_utf8(err).expect("text").lines().count(), 2);
    }

    /// The pointer of the end on a signal for a socket just made at `pfad`.
    fn angelegt(pfad: &Path) -> (UnixListener, SocketPfad) {
        let euid = mac::eigene_euid();
        // Under macOS the folder must be 0700.
        let eltern = pfad.parent().expect("parent");
        fs::set_permissions(eltern, fs::Permissions::from_mode(0o700)).expect("mode");
        let (listener, eigene) =
            socket_anlegen(pfad, 0o600, Plattform::AKTUELL, euid).expect("socket");
        (listener, Arc::new(Mutex::new(Some(eigene))))
    }

    #[test]
    fn the_end_removes_the_own_socket_through_the_held_folder() {
        let ordner = ordner("loeschen-eigen");
        let pfad = ordner.join("s.sock");
        let (_listener, eigene) = angelegt(&pfad);
        assert!(fs::symlink_metadata(&pfad).is_ok());
        socket_loeschen(&eigene);
        assert!(fs::symlink_metadata(&pfad).is_err(), "the socket is gone");
        socket_loeschen(&eigene);
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn the_end_leaves_another_file_a_link_or_a_folder_under_the_name() {
        let ordner = ordner("loeschen-fremd");
        let pfad = ordner.join("s.sock");
        let (_listener, eigene) = angelegt(&pfad);
        // The own socket moves away; a regular file takes its name.
        let weg = ordner.join("weg.sock");
        fs::rename(&pfad, &weg).expect("move");
        fs::write(&pfad, "fremd").expect("file");
        socket_loeschen(&eigene);
        assert_eq!(fs::read_to_string(&pfad).expect("file"), "fremd");
        assert!(weg.exists(), "the moved socket is not touched");
        // A link to the own socket (same identity once followed) takes its name.
        let (_listener2, eigene2) = {
            fs::remove_file(&pfad).expect("remove");
            fs::remove_file(&weg).expect("remove");
            angelegt(&pfad)
        };
        fs::rename(&pfad, &weg).expect("move");
        std::os::unix::fs::symlink(&weg, &pfad).expect("link");
        socket_loeschen(&eigene2);
        assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
        assert!(fs::symlink_metadata(&weg).is_ok());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn the_rights_of_the_socket_are_set_only_on_the_own_socket() {
        let ordner = ordner("rechte-fremd");
        let pfad = ordner.join("s.sock");
        let (_listener, eigene) = angelegt(&pfad);
        let fremd = ordner.join("fremd");
        fs::write(&fremd, "x").expect("file");
        fs::set_permissions(&fremd, fs::Permissions::from_mode(0o600)).expect("chmod");
        // The own socket moves away; a link to a foreign file takes its name.
        fs::rename(&pfad, ordner.join("weg.sock")).expect("move");
        std::os::unix::fs::symlink(&fremd, &pfad).expect("link");
        let guard = eigene.lock().unwrap_or_else(PoisonError::into_inner);
        let eigene_socket = guard.as_ref().expect("own");
        assert!(
            eigene_socket
                .rechte_setzen(Mode::from_raw_mode(0o666))
                .is_err()
        );
        assert_eq!(
            fs::metadata(&fremd).expect("meta").mode() & 0o7777,
            0o600,
            "the target of the link keeps its mode"
        );
        // A regular file under the name: no change either.
        fs::remove_file(&pfad).expect("remove");
        fs::write(&pfad, "y").expect("file");
        fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("chmod");
        assert!(
            eigene_socket
                .rechte_setzen(Mode::from_raw_mode(0o666))
                .is_err()
        );
        assert_eq!(fs::metadata(&pfad).expect("meta").mode() & 0o7777, 0o600);
        drop(guard);
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_made_socket_has_the_asked_rights_and_a_taken_path_stays() {
        let ordner = ordner("rechte-eigen");
        let pfad = ordner.join("s.sock");
        let euid = mac::eigene_euid();
        fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("mode");
        let (_listener, eigene) =
            socket_anlegen(&pfad, 0o600, Plattform::AKTUELL, euid).expect("socket");
        assert_eq!(
            fs::symlink_metadata(&pfad).expect("socket").mode() & 0o7777,
            0o600
        );
        eigene
            .rechte_setzen(Mode::from_raw_mode(0o660))
            .expect("chmod");
        assert_eq!(
            fs::symlink_metadata(&pfad).expect("socket").mode() & 0o7777,
            0o660
        );
        // Rights beyond the nine rights bits, or too large for `mode_t`: refused before anything is made.
        let anderer = ordner.join("t.sock");
        for schlecht in [0o4600, 0o1000_0600, u32::MAX] {
            assert!(
                socket_anlegen(&anderer, schlecht, Plattform::AKTUELL, euid).is_err(),
                "{schlecht:o}"
            );
            assert!(
                fs::symlink_metadata(&anderer).is_err(),
                "nothing made for {schlecht:o}"
            );
        }
        // The path is taken: nothing changes, and the first socket is not the own one of the second call.
        assert!(socket_anlegen(&pfad, 0o666, Plattform::AKTUELL, euid).is_err());
        assert_eq!(
            fs::symlink_metadata(&pfad).expect("socket").mode() & 0o7777,
            0o660
        );
        assert!(eigene.entfernen());
        let _ = fs::remove_dir_all(&ordner);
    }
}

#[cfg(test)]
#[path = "lauf_stderr_test.rs"]
mod stderr_test;
