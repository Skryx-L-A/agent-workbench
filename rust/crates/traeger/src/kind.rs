//! TR12: one child in a PTY, read by a thread, fed into a [`Raster`].

use std::fmt;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::abschluss::{Abschluss, Aktion, Prozessende};
use crate::ereignis::Ereignis;
use crate::raster::{GroesseUngueltig, Raster, groesse_gueltig};
use crate::schlange::Schlange;

/// What went wrong while starting a child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fehler {
    /// The size is outside 2 to 1000 columns and 1 to 1000 rows (TR13). Nothing was opened or started.
    GroesseUngueltig,
    /// The PTY or the child could not be opened or started; the text says why.
    Start(String),
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GroesseUngueltig => GroesseUngueltig.fmt(f),
            Self::Start(text) => f.write_str(text),
        }
    }
}

impl From<GroesseUngueltig> for Fehler {
    fn from(_: GroesseUngueltig) -> Self {
        Self::GroesseUngueltig
    }
}

impl std::error::Error for Fehler {}

/// How a child starts.
#[derive(Debug, Clone)]
pub struct Start {
    /// The program and its arguments.
    pub argv: Vec<String>,
    /// The environment of the child: exactly these variables and nothing else.
    pub env: Vec<(String, String)>,
    /// The working directory of the child. It must be a folder; if it is not, [`Kind::starten`] is an error (TR15).
    pub cwd: PathBuf,
    /// Columns of the PTY and of the screen.
    pub spalten: usize,
    /// Rows of the PTY and of the screen.
    pub zeilen: usize,
    /// Lines of scrollback.
    pub ruecklauf: usize,
}

/// A piece of what the reading thread saw.
enum Stueck {
    Daten(Vec<u8>),
    Ende,
}

/// Only one `openpty` runs at a time. On macOS the call fails with an unknown error when several threads make it at once
/// (14 sessions started together lost about a third of the runs); sessions of one service start from several threads.
static OEFFNEN: Mutex<()> = Mutex::new(());

/// The size of the PTY. Sizes are 2 to 1000 columns and 1 to 1000 rows, so they fit in `u16`.
fn pty_groesse(spalten: usize, zeilen: usize) -> PtySize {
    PtySize {
        rows: u16::try_from(zeilen).unwrap_or(u16::MAX),
        cols: u16::try_from(spalten).unwrap_or(u16::MAX),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// One session: a child in a PTY and the screen made from its output.
pub struct Kind {
    raster: Raster,
    /// The master of the PTY; `None` once the pseudo console was closed (A64, see `abschluss.rs`).
    master: Option<Box<dyn MasterPty + Send>>,
    schlange: Arc<Schlange>,
    empfaenger: Receiver<Stueck>,
    prozess: Box<dyn Child + Send + Sync>,
    ende: bool,
    /// A64: the end of the process under `ConPTY`, where the output does not end by itself.
    abschluss: Abschluss,
    /// A64: whether `lies` watches the process (`ENDE_BEOBACHTEN`: Windows). A field, so that a test under Linux reaches the wiring.
    beobachte_ende: bool,
    /// The last call of [`Kind::lies`] took a piece of output (it may have given no event).
    daten_gelesen: bool,
    leser: Option<JoinHandle<()>>,
}

impl Kind {
    /// Opens the PTY, starts the child with exactly `start.env` and a reading and a writing thread.
    ///
    /// # Errors
    /// The size is outside 2 to 1000 columns and 1 to 1000 rows ([`Fehler::GroesseUngueltig`]; checked first, so no PTY is
    /// opened, no child started and no size reaches the grid), or `cwd` is not a folder (TR15: no fall back to another
    /// folder), or the PTY could not be opened or the child could not be started.
    pub fn starten(start: Start) -> Result<Self, Fehler> {
        Self::starten_mit(start, VORSPANN_ERWARTEN)
    }

    /// [`Kind::starten`]; `vorspann` says whether the child starts behind the prelude of `ConPTY` (W7). A parameter, so that a test under
    /// Linux reaches the wiring.
    fn starten_mit(start: Start, vorspann: bool) -> Result<Self, Fehler> {
        let fehler = |e: &dyn fmt::Display| Fehler::Start(e.to_string());
        let Start {
            argv,
            env,
            cwd,
            spalten,
            zeilen,
            ruecklauf,
        } = start;
        // SECURITY (TR13): the size is checked before the PTY is opened and before the grid is made.
        if !groesse_gueltig(spalten, zeilen) {
            return Err(Fehler::GroesseUngueltig);
        }
        let mut raster = Raster::neu(spalten, zeilen, ruecklauf)?;
        let Some((programm, argumente)) = argv.split_first() else {
            return Err(Fehler::Start("argv ist leer".to_owned()));
        };
        // TR15: `portable-pty` starts the child in the home folder if `cwd` is not a folder; the carrier refuses instead.
        if !cwd.is_dir() {
            return Err(Fehler::Start(format!(
                "cwd ist kein Ordner: {}",
                cwd.display()
            )));
        }
        if vorspann {
            raster.vorspann_erwarten(programm);
        }
        let befehl = befehl_bauen(programm, argumente, &env, &cwd).map_err(Fehler::Start)?;
        let paar = {
            let _einer = OEFFNEN.lock().unwrap_or_else(PoisonError::into_inner);
            native_pty_system().openpty(pty_groesse(spalten, zeilen))
        }
        .map_err(|e| fehler(&e))?;
        let prozess = paar.slave.spawn_command(befehl).map_err(|e| fehler(&e))?;
        drop(paar.slave);
        let mut leser = paar.master.try_clone_reader().map_err(|e| fehler(&e))?;
        let mut schreiber = paar.master.take_writer().map_err(|e| fehler(&e))?;
        // TR15: the writing thread takes the queue; `schreibe` never waits for the PTY. It ends when the queue is closed
        // (drop of the session) or a write fails; it is not joined, because a child that keeps the PTY open (a grandchild)
        // could hold it in `write`.
        let schlange = Arc::new(Schlange::default());
        {
            let schlange = Arc::clone(&schlange);
            std::thread::spawn(move || schlange.schreiben(&mut schreiber));
        }
        let (sender, empfaenger) = channel::<Stueck>();
        let faden = std::thread::spawn(move || {
            let mut puffer = vec![0u8; 4096];
            loop {
                match leser.read(&mut puffer) {
                    Ok(0) | Err(_) => {
                        let _ = sender.send(Stueck::Ende);
                        return;
                    }
                    Ok(n) => {
                        if sender.send(Stueck::Daten(puffer[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        Ok(Self {
            raster,
            master: Some(paar.master),
            schlange,
            empfaenger,
            prozess,
            ende: false,
            abschluss: Abschluss::default(),
            beobachte_ende: ENDE_BEOBACHTEN,
            daten_gelesen: false,
            leser: Some(faden),
        })
    }

    /// Waits up to `frist` for the next piece of output and returns its events. After the end of the output the child is
    /// waited for and `Kindende` follows the last output (TR2, TR12). The answers of the grid to device queries go back to
    /// the child (TR10). An empty result means: nothing came within `frist`, or the child has ended already.
    ///
    /// Under Windows (`ConPTY`) the output does not end when the child ends (A64): when nothing came within `frist`, the process is
    /// asked without waiting; if it has ended, the pseudo console is closed once, the rest of the output is read until the pipe
    /// ends, and only then `Kindende` follows. Under Linux and macOS nothing of this happens.
    pub fn lies(&mut self, frist: Duration) -> Vec<Ereignis> {
        self.daten_gelesen = false;
        if self.ende {
            return Vec::new();
        }
        match self.empfaenger.recv_timeout(frist) {
            Ok(Stueck::Daten(bytes)) => {
                self.daten_gelesen = true;
                let ereignisse = self.raster.bytes(&bytes);
                let antworten = self.raster.antworten();
                if !antworten.is_empty() {
                    let _ = self.schreibe(&antworten);
                }
                ereignisse
            }
            Ok(Stueck::Ende) | Err(RecvTimeoutError::Disconnected) => {
                let aktion = self.abschluss_schritt(true);
                debug_assert_eq!(aktion, Aktion::Melden);
                self.ende = true;
                let code = kindcode(self.prozess.as_mut());
                self.raster.kindende(code)
            }
            Err(RecvTimeoutError::Timeout) => {
                if self.beobachte_ende {
                    let _ = self.abschluss_schritt(false);
                }
                Vec::new()
            }
        }
    }

    /// One step of the end logic (`abschluss.rs`) with the real process and master of this session.
    fn abschluss_schritt(&mut self, ausgabe_zu_ende: bool) -> Aktion {
        let mut quelle = Wirklich {
            prozess: self.prozess.as_mut(),
            master: &mut self.master,
        };
        self.abschluss.schritt(ausgabe_zu_ende, &mut quelle)
    }

    /// `true` if the last call of [`Kind::lies`] took a piece of output, also when the piece gave no event (the bytes of an
    /// unfinished sequence give none until it ends). An empty result with `false` means: nothing was there.
    #[must_use]
    pub fn daten_gelesen(&self) -> bool {
        self.daten_gelesen
    }

    /// `true` once the output has ended and `Kindende` was reported.
    #[must_use]
    pub fn ist_zu_ende(&self) -> bool {
        self.ende
    }

    /// Queues the bytes for the PTY, unchanged and in order (the effect `pty` of the core), and returns at once; it never
    /// waits for the child (TR15). A thread of its own writes the queue to the PTY. The queue holds at most
    /// [`crate::SCHLANGE_MAX`] bytes, counting those the thread is writing.
    ///
    /// # Errors
    /// `WouldBlock` if the queue would hold more than [`crate::SCHLANGE_MAX`] bytes with these (nothing of them is queued; the
    /// child does not read its input); `BrokenPipe` if a write to the PTY failed earlier (the child is gone).
    pub fn schreibe(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.schlange.einreihen(bytes)
    }

    /// The effect `groessen` of the core: sets the size of the PTY (the child learns it from the system) and of the screen.
    ///
    /// # Errors
    /// The size is outside 2 to 1000 columns and 1 to 1000 rows. The PTY keeps its old size then, and so does the screen.
    pub fn groesse(
        &mut self,
        spalten: usize,
        zeilen: usize,
    ) -> Result<Vec<Ereignis>, GroesseUngueltig> {
        // SECURITY (TR13): the size is checked before the PTY size is set, so a refused size changes neither the PTY nor the grid.
        if !groesse_gueltig(spalten, zeilen) {
            return Err(GroesseUngueltig);
        }
        if let Some(master) = &self.master {
            let _ = master.resize(pty_groesse(spalten, zeilen));
        }
        self.raster.groesse(spalten, zeilen)
    }

    /// The screen made from the output so far.
    #[must_use]
    pub fn raster(&self) -> &Raster {
        &self.raster
    }

    /// The process id of the child.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.prozess.process_id()
    }
}

/// A64: where the output does not end when the child ends, the session watches the process itself. That is `ConPTY` (Windows); under Unix
/// the reading of the output ends with the child.
const ENDE_BEOBACHTEN: bool = cfg!(windows);

/// W7: whether the child starts behind the prelude of `ConPTY` (Windows), whose title the carrier does not report (`vorspann.rs`).
const VORSPANN_ERWARTEN: bool = cfg!(windows);

/// The real side of the session for the end logic (`abschluss.rs`): the process of the child and the master of the PTY.
struct Wirklich<'a> {
    prozess: &'a mut (dyn Child + Send + Sync),
    master: &'a mut Option<Box<dyn MasterPty + Send>>,
}

impl Prozessende for Wirklich<'_> {
    /// `Child::try_wait`: asks without waiting (under Windows `GetExitCodeProcess`).
    fn beendet(&mut self) -> bool {
        matches!(self.prozess.try_wait(), Ok(Some(_)))
    }

    /// Dropping the master closes the pseudo console under Windows (`portable-pty` 0.9.0, `win/psuedocon.rs:73-77`).
    fn schliessen(&mut self) {
        self.master.take();
    }
}

/// The command of the child: the program, its arguments and exactly `env` as the environment (TR12, TR15).
///
/// `portable-pty` clears the environment of the child and then always sets `SHELL` (`cmdbuilder.rs`, `as_command`: the
/// shell of the passwd database if `env` has none), and there is no call that leaves it out. So, if `env` has no `SHELL`,
/// the child starts as `env -u SHELL <program> <arguments>`: `env` removes the variable and replaces itself by the
/// program (the process id, the session and the PTY stay the same). The program is searched here the way `portable-pty`
/// searches it, so that a missing program is still an error of the start; `argv[0]` of the child is then its absolute path.
// Only the search on Unix can fail; elsewhere the result is always `Ok`, and one signature serves both systems.
#[cfg_attr(not(unix), allow(clippy::unnecessary_wraps))]
fn befehl_bauen(
    programm: &str,
    argumente: &[String],
    env: &[(String, String)],
    cwd: &std::path::Path,
) -> Result<CommandBuilder, String> {
    #[cfg(unix)]
    let mut befehl = if env.iter().any(|(name, _)| name == "SHELL") {
        CommandBuilder::new(programm)
    } else {
        let pfad = crate::programm::finden(programm, env, cwd)?;
        let pfad = pfad.to_string_lossy().into_owned();
        // `env` takes a word with `=` for a variable to set
        if pfad.contains('=') {
            return Err(format!("Programmpfad mit '=': {pfad}"));
        }
        let mut befehl = CommandBuilder::new("/usr/bin/env");
        befehl.args(["-u", "SHELL", pfad.as_str()]);
        befehl
    };
    // no `SHELL` is added to the environment on this system
    #[cfg(not(unix))]
    let mut befehl = CommandBuilder::new(programm);
    befehl.args(argumente);
    befehl.env_clear();
    for (name, wert) in env {
        befehl.env(name, wert);
    }
    befehl.cwd(cwd);
    Ok(befehl)
}

/// Waits for the child and gives the code `Kindende` carries (TR12): the exit code, or 128 plus the number of the signal that
/// ended the child (decision of 19.11.2026). The number comes from the wait status itself (`WIFSIGNALED`, `WTERMSIG`), not
/// from the text `portable-pty` makes of it, which differs from system to system. `portable-pty` keeps only that text, so
/// the child is taken back as the `std::process::Child` it is on Unix and waited for there. A wait that fails gives 255.
fn kindcode(prozess: &mut (dyn Child + Send + Sync)) -> u32 {
    #[cfg(unix)]
    if let Some(kind) = (&mut *prozess as &mut dyn Child).downcast_mut::<std::process::Child>() {
        use std::os::unix::process::ExitStatusExt;
        return kind.wait().map_or(255, |status| match status.signal() {
            Some(signal) => 128 + u32::try_from(signal).unwrap_or(127),
            None => status
                .code()
                .and_then(|code| u32::try_from(code).ok())
                .unwrap_or(255),
        });
    }
    prozess.wait().map_or(255, |status| status.exit_code())
}

impl Drop for Kind {
    /// Ends a child that still runs and collects it, so that no process stays behind.
    fn drop(&mut self) {
        self.schlange.schliessen();
        if self.ende {
            if let Some(faden) = self.leser.take() {
                let _ = faden.join();
            }
        } else {
            let _ = self.prozess.kill();
            let _ = self.prozess.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;
    #[cfg(unix)]
    use std::time::Instant;

    #[cfg(unix)]
    fn start(argv: &[&str]) -> Start {
        Start {
            argv: argv.iter().map(|a| (*a).to_owned()).collect(),
            env: Vec::new(),
            cwd: PathBuf::from("/"),
            spalten: 20,
            zeilen: 5,
            ruecklauf: 10,
        }
    }

    /// The program `true`: `/bin/true` on Linux, only `/usr/bin/true` on macOS. On Linux this is the same binary as before
    /// (`/bin` is `/usr/bin` with merged folders, and without them `/bin/true` is taken).
    #[cfg(unix)]
    fn wahr() -> &'static str {
        ["/bin/true", "/usr/bin/true"]
            .into_iter()
            .find(|pfad| std::path::Path::new(pfad).is_file())
            .expect("true is installed in /bin or /usr/bin")
    }

    #[cfg(unix)]
    #[test]
    fn ein_laufender_prozess_wird_nicht_geschlossen_und_ein_beendeter_genau_einmal() {
        let mut laeuft = Kind::starten(start(&["/bin/sleep", "60"])).expect("sleep starts");
        assert_eq!(laeuft.abschluss_schritt(false), Aktion::Warten);
        assert_eq!(laeuft.abschluss_schritt(false), Aktion::Warten);
        assert!(laeuft.master.is_some(), "a running child keeps its PTY");

        let mut kurz = Kind::starten(start(&[wahr()])).expect("true starts");
        let frist = Instant::now() + Duration::from_secs(10);
        let mut aktion = kurz.abschluss_schritt(false);
        while aktion == Aktion::Warten {
            assert!(Instant::now() < frist, "the process did not end");
            std::thread::sleep(Duration::from_millis(10));
            aktion = kurz.abschluss_schritt(false);
        }
        assert_eq!(aktion, Aktion::Schliessen);
        assert!(
            kurz.master.is_none(),
            "the master is dropped: the pseudo console closes"
        );
        assert_eq!(kurz.abschluss_schritt(false), Aktion::Warten, "closed once");
        // the end of the output is reported without asking the process or closing anything
        assert_eq!(kurz.abschluss_schritt(true), Aktion::Melden);
    }

    #[cfg(unix)]
    #[test]
    fn nach_dem_schliessen_gibt_groesse_keinen_fehler_und_aendert_das_raster() {
        let mut kind = Kind::starten(start(&[wahr()])).expect("true starts");
        let frist = Instant::now() + Duration::from_secs(10);
        while kind.abschluss_schritt(false) == Aktion::Warten {
            assert!(Instant::now() < frist, "the process did not end");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(kind.groesse(30, 7).is_ok());
        assert_eq!(kind.raster().groesse_jetzt(), (30, 7));
    }

    /// The wiring of `lies` with the end logic, as it runs under `ConPTY`, where the output stays open after the child has ended. Under
    /// Linux the output ends with the session leader (the terminal is hung up), so the open output is made by hand: the channel of the
    /// reading thread is replaced by one whose sender this test keeps. The shell ends with 3; `lies` (watching switched on) closes the
    /// pseudo console; `Kindende` does not come while the output is open; when the output ends, `Kindende` follows with code 3.
    #[cfg(unix)]
    #[test]
    fn lies_schliesst_und_meldet_kindende_erst_nach_dem_ende_der_ausgabe() {
        let mut kind = Kind::starten(start(&["/bin/sh", "-c", "exit 3"])).expect("sh starts");
        // the real reading thread ends on its own (it sees the hang-up); this channel stays open like the pipe of `ConPTY`
        let (sender, empfaenger) = channel::<Stueck>();
        kind.empfaenger = empfaenger;
        kind.beobachte_ende = true;
        let mut ereignisse = Vec::new();
        let frist = Instant::now() + Duration::from_secs(10);
        while kind.master.is_some() {
            assert!(Instant::now() < frist, "the pseudo console was not closed");
            ereignisse.extend(kind.lies(Duration::from_millis(50)));
        }
        // the process has ended and the console is closed, but the output is still open: no `Kindende`
        for _ in 0..4 {
            ereignisse.extend(kind.lies(Duration::from_millis(50)));
        }
        assert!(
            !kind.ist_zu_ende(),
            "Kindende must wait for the end of the output"
        );
        assert!(
            !ereignisse
                .iter()
                .any(|e| matches!(e, Ereignis::Kindende { .. })),
            "{ereignisse:?}"
        );
        // the rest of the output comes, then the pipe ends: now `Kindende` follows, once, with the code of the process
        sender
            .send(Stueck::Daten(b"rest\r\n".to_vec()))
            .expect("the channel is open");
        sender.send(Stueck::Ende).expect("the channel is open");
        let frist = Instant::now() + Duration::from_secs(10);
        while !kind.ist_zu_ende() {
            assert!(
                Instant::now() < frist,
                "Kindende did not follow the end of the output"
            );
            ereignisse.extend(kind.lies(Duration::from_millis(50)));
        }
        let kindende: Vec<&Ereignis> = ereignisse
            .iter()
            .filter(|e| matches!(e, Ereignis::Kindende { .. }))
            .collect();
        assert_eq!(kindende.len(), 1, "{ereignisse:?}");
        assert!(
            matches!(kindende[0], Ereignis::Kindende { code: 3 }),
            "{kindende:?}"
        );
        assert!(
            kind.raster().zeilen().join(" ").contains("rest"),
            "the rest of the output reached the screen before the end"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ohne_beobachten_bleibt_das_verhalten_unter_unix_wie_vorher() {
        // Linux and macOS: the output ends with the child; the console is not closed by the end logic, `Kindende` follows the output
        let mut kind =
            Kind::starten(start(&["/bin/sh", "-c", "echo fertig; exit 4"])).expect("sh starts");
        assert!(!kind.beobachte_ende || cfg!(windows));
        let mut ereignisse = Vec::new();
        let frist = Instant::now() + Duration::from_secs(10);
        while !kind.ist_zu_ende() {
            assert!(Instant::now() < frist, "Kindende did not come");
            ereignisse.extend(kind.lies(Duration::from_millis(50)));
        }
        assert!(kind.master.is_some(), "nothing closed the console");
        assert!(
            ereignisse
                .iter()
                .any(|e| matches!(e, Ereignis::Kindende { code: 4 })),
            "{ereignisse:?}"
        );
    }

    /// The wiring of the prelude rule (W7): with `vorspann` on, the first title that names the started program is dropped; with it off
    /// (Linux, macOS) it stays an event. The child writes the title of its own path first, then one title of its own.
    #[cfg(unix)]
    #[test]
    fn starten_mit_vorspann_verwirft_den_ersten_titel_mit_dem_pfad_und_ohne_bleibt_er_ein_ereignis()
    {
        let alle = |vorspann: bool| {
            let mut kind = Kind::starten_mit(
                start(&[
                    "/bin/sh",
                    "-c",
                    "printf '\\033]0;/bin/sh\\007\\033]0;eigen\\007'",
                ]),
                vorspann,
            )
            .expect("sh starts");
            let mut texte = Vec::new();
            let frist = Instant::now() + Duration::from_secs(10);
            while !kind.ist_zu_ende() {
                assert!(Instant::now() < frist, "Kindende did not come");
                for e in kind.lies(Duration::from_millis(50)) {
                    if let Ereignis::Titel { text } = e {
                        texte.push(text);
                    }
                }
            }
            (texte, kind.raster().titel().map(str::to_owned))
        };
        assert_eq!(
            alle(true),
            (vec!["eigen".to_owned()], Some("eigen".to_owned()))
        );
        assert_eq!(
            alle(false),
            (
                vec!["/bin/sh".to_owned(), "eigen".to_owned()],
                Some("eigen".to_owned())
            )
        );
        assert_eq!(VORSPANN_ERWARTEN, cfg!(windows));
    }
}
