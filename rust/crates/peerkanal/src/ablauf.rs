//! The order of "Server anlegen", "Annehmen und Messen" and "Client verbinden" over a small set of system calls.
//!
//! The calls stand in the trait [`Plattform`]. The Windows build implements it in the module `sys`; the tests of this module
//! implement it with a stand-in, so the order of the steps, the instance that keeps the name and "no byte before the checks"
//! are testable without Windows.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::acl;
use crate::fehler::{Code, Ereignis, Fehler, Grund, Peer, System};
use crate::kennung::{Kanalname, Kennung};
use crate::pruefung::{self, Token};
use crate::vorgabe::{ClientVorgabe, Erlaubnis, Erwartet, ServerVorgabe};

/// The hello: `pk1` and LF.
pub(crate) const HALLO: [u8; 4] = *b"pk1\n";

/// `ERROR_FILE_NOT_FOUND`.
pub(crate) const OS_NICHT_GEFUNDEN: i32 = 2;
/// `ERROR_ACCESS_DENIED`.
pub(crate) const OS_ZUGRIFF: i32 = 5;
/// `ERROR_PIPE_BUSY`.
pub(crate) const OS_BELEGT: i32 = 231;
/// `ERROR_NO_DATA`: the client connected and left before `ConnectNamedPipe` returned.
pub(crate) const OS_KEINE_DATEN: i32 = 232;

/// The result of one read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Lesen {
    /// So many bytes (at least one).
    Daten(usize),
    /// The other side closed.
    Ende,
    /// The time limit passed without a byte.
    Frist,
    /// Another error of the system.
    Fehler(i32),
}

/// The system calls of the order. Error numbers are those of the system (Windows: `GetLastError`).
pub(crate) trait Plattform: Send + Sync + 'static {
    /// A pipe instance of the server or the handle of the client.
    type Griff: Send + Sync;
    /// A bound process (Windows: the handle).
    type Prozess: Send + Sync;
    /// The job of the run (Windows: the handle).
    type Job: Send + Sync;

    /// The own `TokenUser` SID as text.
    fn eigene_sid(&self) -> Result<String, i32>;
    /// `CreateNamedPipeW` with the flags of [`acl::oeffnungsmodus`] and [`acl::PIPE_MODUS`] and the given descriptor.
    fn instanz_anlegen(
        &self,
        name: &str,
        erste: bool,
        instanzen: u8,
        sddl: &str,
    ) -> Result<Self::Griff, i32>;
    /// `ConnectNamedPipe`, blocking. `ERROR_PIPE_CONNECTED` counts as success.
    fn auf_client_warten(&self, g: &Self::Griff) -> Result<(), i32>;
    /// The current time in the unit of the start time (Windows: `FILETIME`).
    fn jetzt(&self) -> u64;
    /// `GetNamedPipeClientProcessId`.
    fn client_pid(&self, g: &Self::Griff) -> Option<u32>;
    /// `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` and `GetProcessTimes`: the handle and the start time, or `None`.
    fn prozess_oeffnen(&self, pid: u32) -> Option<(Self::Prozess, u64)>;
    /// `QueryFullProcessImageNameW`.
    fn programm(&self, p: &Self::Prozess) -> Option<String>;
    /// `GetExitCodeProcess` is `STILL_ACTIVE`; `None` without an answer.
    fn lebt(&self, p: &Self::Prozess) -> Option<bool>;
    /// `IsProcessInJob`; `None` without an answer.
    fn im_job(&self, p: &Self::Prozess, job: &Self::Job) -> Option<bool>;
    /// Impersonation, `OpenThreadToken`, `RevertToSelf`, then `TokenUser`, `TokenAppContainerSid` and `IsTokenRestricted`.
    fn kennung_messen(&self, g: &Self::Griff) -> Result<Token, i32>;
    /// `DisconnectNamedPipe`: closes the connection of the client; the instance stays.
    fn trennen(&self, g: &Self::Griff);
    /// One read; with a time limit it returns [`Lesen::Frist`] when it passes.
    fn lesen(&self, g: &Self::Griff, puffer: &mut [u8], frist: Option<Duration>) -> Lesen;
    /// One write.
    fn schreiben(&self, g: &Self::Griff, daten: &[u8]) -> Result<usize, i32>;
    /// `CreateFileW` with [`acl::CLIENT_ZUGRIFF`] and [`acl::CLIENT_FLAGS`].
    fn oeffnen(&self, name: &str) -> Result<Self::Griff, i32>;
    /// `WaitNamedPipeW` for at most this long.
    fn warten(&self, name: &str, frist: Duration);
    /// `GetSecurityInfo` with `OWNER_SECURITY_INFORMATION` on the own handle.
    fn besitzer(&self, g: &Self::Griff) -> Result<String, i32>;
    /// `GetNamedPipeServerProcessId`.
    fn server_pid(&self, g: &Self::Griff) -> Option<u32>;
}

/// The binding of the process of the other side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prozessbindung {
    /// The process id.
    pub pid: u32,
    /// The start time (Windows: `FILETIME` of the creation), `None` when the process could not be opened.
    pub startzeit: Option<u64>,
}

fn sperren<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

struct PoolZustand<G> {
    /// New instances: a client can connect to them before anyone waits on them, so they are served first.
    frisch: Vec<G>,
    /// Disconnected instances: no client reaches them until `ConnectNamedPipe` runs on them again.
    getrennt: Vec<G>,
    /// All instances of this server that exist.
    vorhanden: usize,
    /// `false` after the end of the server: returned instances are closed.
    offen: bool,
}

/// The instances of one server. An instance is never closed while the server lives; after a connection it is disconnected and
/// used again. So the name stays taken from `anlegen` to the end of the server (B6).
pub(crate) struct Pool<P: Plattform> {
    plattform: Arc<P>,
    name: String,
    sddl: String,
    max: u8,
    zustand: Mutex<PoolZustand<P::Griff>>,
    frei: Condvar,
}

impl<P: Plattform> Pool<P> {
    /// An instance to wait on: a waiting one, else a new one while the limit allows, else wait for a returned one.
    fn holen(&self) -> Result<P::Griff, Fehler> {
        let mut z = sperren(&self.zustand);
        loop {
            if let Some(g) = z.frisch.pop().or_else(|| z.getrennt.pop()) {
                return Ok(g);
            }
            if z.vorhanden < usize::from(self.max) {
                z.vorhanden += 1;
                drop(z);
                return self
                    .plattform
                    .instanz_anlegen(&self.name, false, self.max, &self.sddl)
                    .map_err(|e| {
                        sperren(&self.zustand).vorhanden -= 1;
                        Fehler::os(Code::Io, e)
                    });
            }
            z = self.frei.wait(z).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// B6: before a connected instance goes to the caller, the next instance waits, as far as the limit allows.
    fn bereitstellen(&self) {
        let mut z = sperren(&self.zustand);
        if !z.frisch.is_empty() || z.vorhanden >= usize::from(self.max) {
            return;
        }
        z.vorhanden += 1;
        drop(z);
        match self
            .plattform
            .instanz_anlegen(&self.name, false, self.max, &self.sddl)
        {
            Ok(g) => self.ablegen(g, true),
            Err(_) => sperren(&self.zustand).vorhanden -= 1,
        }
    }

    /// Closes the connection of the client and keeps the instance for the next one.
    fn zurueck(&self, g: P::Griff) {
        self.plattform.trennen(&g);
        self.ablegen(g, false);
    }

    fn ablegen(&self, g: P::Griff, frisch: bool) {
        let mut z = sperren(&self.zustand);
        if z.offen {
            if frisch {
                z.frisch.push(g);
            } else {
                z.getrennt.push(g);
            }
        } else {
            z.vorhanden -= 1;
            drop(z);
            drop(g);
        }
        self.frei.notify_one();
    }
}

/// A test seam of the outside runner (feature `testnaht`): called right after `ConnectNamedPipe` returned.
pub(crate) type Haltepunkt = Box<dyn Fn() + Send + Sync>;

/// The hook of the caller for events (decision 4 of 25.11.2026): the library calls it with each event of a refusal before it
/// disconnects the instance.
pub(crate) type Ereignishaken = Arc<dyn Fn(&Ereignis) + Send + Sync>;

/// Decision 4: the event first, then the end. The hook gets the event while the client still holds its connection; only
/// then the instance is disconnected. The caller waits for nothing: no drop of a value of the caller ends the connection.
fn melden_und_trennen<P: Plattform>(
    pool: &Pool<P>,
    haken: Option<&Ereignishaken>,
    g: P::Griff,
    ereignis: Ereignis,
) -> Ereignis {
    if let Some(haken) = haken {
        haken(&ereignis);
    }
    pool.zurueck(g);
    ereignis
}

/// The own `TokenUser` SID in the form of [`Sid`]; a SID of another form (such as `LOCAL SERVICE`) is `pk.io` without
/// `os_fehler`, a failed call `pk.io` with it.
pub(crate) fn eigene_sid<P: Plattform>(p: &P) -> Result<crate::kennung::Sid, Fehler> {
    let text = p.eigene_sid().map_err(|e| Fehler::os(Code::Io, e))?;
    crate::kennung::Sid::neu(&text).map_err(|_| Fehler::neu(Code::Io))
}

pub(crate) struct ServerKern<P: Plattform> {
    plattform: Arc<P>,
    pool: Arc<Pool<P>>,
    kanal: Kanalname,
    erlaubt: Vec<Erlaubnis>,
    hallo_frist: Duration,
    prozess_pflicht: bool,
    job: Option<P::Job>,
    pub(crate) nach_verbinden: Option<Haltepunkt>,
    pub(crate) haken: Option<Ereignishaken>,
}

impl<P: Plattform> Drop for ServerKern<P> {
    fn drop(&mut self) {
        let wartend = {
            let mut z = sperren(&self.pool.zustand);
            z.offen = false;
            let mut w = std::mem::take(&mut z.frisch);
            w.append(&mut z.getrennt);
            z.vorhanden -= w.len();
            w
        };
        drop(wartend);
    }
}

/// The outcome of `annehmen`.
pub(crate) enum AnnahmeKern<P: Plattform> {
    Vorlaeufig(VorlaeufigKern<P>),
    Abgewiesen(Ereignis),
}

impl<P: Plattform> ServerKern<P> {
    /// "Server anlegen" from step 2 on; step 1 (the settings) is checked by the caller. `zusatz`: see [`acl::sddl`].
    pub(crate) fn anlegen(
        plattform: Arc<P>,
        v: ServerVorgabe,
        job: Option<P::Job>,
        zusatz: &[&str],
    ) -> Result<Self, Fehler> {
        let eigene = plattform
            .eigene_sid()
            .map_err(|e| Fehler::os(Code::AnlegenFehlgeschlagen, e))?;
        let sddl = acl::sddl(&eigene, &v.erlaubt, zusatz);
        let erste = plattform
            .instanz_anlegen(v.name.als_str(), true, v.instanzen, &sddl)
            .map_err(|e| {
                if e == OS_ZUGRIFF || e == OS_BELEGT {
                    Fehler::os(Code::NameBelegt, e)
                } else {
                    Fehler::os(Code::AnlegenFehlgeschlagen, e)
                }
            })?;
        let pool = Arc::new(Pool {
            plattform: Arc::clone(&plattform),
            name: v.name.als_str().to_owned(),
            sddl,
            max: v.instanzen,
            zustand: Mutex::new(PoolZustand {
                frisch: vec![erste],
                getrennt: Vec::new(),
                vorhanden: 1,
                offen: true,
            }),
            frei: Condvar::new(),
        });
        Ok(Self {
            plattform,
            pool,
            kanal: v.name,
            erlaubt: v.erlaubt,
            hallo_frist: v.hallo_frist,
            prozess_pflicht: v.prozess_pflicht,
            job,
            nach_verbinden: None,
            haken: None,
        })
    }

    fn abweisen(&self, g: P::Griff, grund: Grund, peer: Option<Peer>) -> AnnahmeKern<P> {
        let ereignis = Ereignis {
            kanal: self.kanal.clone(),
            grund,
            peer,
        };
        AnnahmeKern::Abgewiesen(melden_und_trennen(
            &self.pool,
            self.haken.as_ref(),
            g,
            ereignis,
        ))
    }

    /// "Annehmen und Messen", steps 1 to 7.
    pub(crate) fn annehmen(&self) -> Result<AnnahmeKern<P>, Fehler> {
        let p = &*self.plattform;
        let g = loop {
            let g = self.pool.holen()?;
            match p.auf_client_warten(&g) {
                Ok(()) => break g,
                Err(OS_KEINE_DATEN) => self.pool.zurueck(g),
                Err(e) => {
                    self.pool.zurueck(g);
                    return Err(Fehler::os(Code::Io, e));
                }
            }
        };
        // Step 1: the moment of the connection.
        let verbindungszeit = p.jetzt();
        if let Some(halt) = &self.nach_verbinden {
            halt();
        }
        // Step 2: bind the process at once.
        let pid = p.client_pid(&g);
        let prozess = pid.and_then(|pid| p.prozess_oeffnen(pid));
        let programm = prozess.as_ref().and_then(|(h, _)| p.programm(h));
        let bindung = pid.map(|pid| Prozessbindung {
            pid,
            startzeit: prozess.as_ref().map(|(_, s)| *s),
        });
        // B6: the next instance waits before this one can go to the caller.
        self.pool.bereitstellen();
        // Step 3: the hello, read before the impersonation (error 1368 otherwise). Under Windows the event has no peer.
        if let Err(grund) = hallo_lesen(p, &g, self.hallo_frist) {
            return Ok(self.abweisen(g, grund, None));
        }
        // Step 4: the identity.
        let Ok(token) = p.kennung_messen(&g) else {
            return Ok(self.abweisen(g, Grund::MessungGescheitert, None));
        };
        let im_job = match (&self.job, &prozess) {
            (Some(job), Some((h, _))) => p.im_job(h, job),
            _ => None,
        };
        let peer = Peer {
            sid: Some(token.sid.clone()),
            appcontainer_sid: token.appcontainer_sid.clone(),
            pid,
            startzeit: bindung.and_then(|b| b.startzeit),
            im_job,
            programm,
            eingeschraenkt: Some(token.eingeschraenkt),
            ..Peer::leer(System::Windows)
        };
        if let Some(grund) = pruefung::stufe_messung(
            self.prozess_pflicht,
            prozess.is_some(),
            self.job.is_some(),
            im_job,
        ) {
            return Ok(self.abweisen(g, grund, Some(peer)));
        }
        // Step 5: the bound process is the one that connected.
        let gebunden = prozess
            .as_ref()
            .map(|(h, start)| (p.lebt(h) == Some(true), *start));
        if let Some(grund) = pruefung::stufe_prozess(gebunden, verbindungszeit) {
            return Ok(self.abweisen(g, grund, Some(peer)));
        }
        // Step 6: the list.
        if let Some(grund) = pruefung::stufe_kennung_windows(&self.erlaubt, &token) {
            return Ok(self.abweisen(g, grund, Some(peer)));
        }
        // Step 7: the hook of the caller; no byte has gone back yet.
        Ok(AnnahmeKern::Vorlaeufig(VorlaeufigKern {
            plattform: Arc::clone(&self.plattform),
            pool: Arc::clone(&self.pool),
            kanal: self.kanal.clone(),
            haken: self.haken.clone(),
            griff: Some(g),
            prozess: prozess.map(|(h, _)| h),
            bindung,
            peer,
        }))
    }
}

fn hallo_lesen<P: Plattform>(p: &P, g: &P::Griff, frist: Duration) -> Result<(), Grund> {
    let ende = Instant::now() + frist;
    let mut puffer = [0u8; 4];
    let mut n = 0;
    while n < puffer.len() {
        let rest = ende.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            return Err(Grund::HalloFehlt);
        }
        // Decision 5: bytes that are no start of the hello are `pk.hallo_falsch`, checked after each read; an end of the
        // connection or of the time limit before 4 fitting bytes is `pk.hallo_fehlt`.
        match p.lesen(g, &mut puffer[n..], Some(rest)) {
            Lesen::Daten(k) if k > 0 => n += k,
            Lesen::Daten(_) | Lesen::Frist | Lesen::Ende | Lesen::Fehler(_) => {
                return Err(Grund::HalloFehlt);
            }
        }
        if puffer[..n] != HALLO[..n] {
            return Err(Grund::HalloFalsch);
        }
    }
    Ok(())
}

fn alles_schreiben<P: Plattform>(p: &P, g: &P::Griff, mut daten: &[u8]) -> Result<(), i32> {
    while !daten.is_empty() {
        let n = p.schreiben(g, daten)?;
        if n == 0 {
            return Err(0);
        }
        daten = &daten[n..];
    }
    Ok(())
}

/// A connection before the decision of the hook.
pub(crate) struct VorlaeufigKern<P: Plattform> {
    plattform: Arc<P>,
    pool: Arc<Pool<P>>,
    kanal: Kanalname,
    haken: Option<Ereignishaken>,
    griff: Option<P::Griff>,
    prozess: Option<P::Prozess>,
    bindung: Option<Prozessbindung>,
    pub(crate) peer: Peer,
}

impl<P: Plattform> VorlaeufigKern<P> {
    pub(crate) fn prozess(&self) -> Option<&Prozessbindung> {
        self.bindung.as_ref()
    }

    /// Sends the hello back and hands the connection over.
    pub(crate) fn bestaetigen(mut self) -> Result<VerbindungKern<P>, Fehler> {
        let Some(g) = self.griff.take() else {
            return Err(Fehler::neu(Code::Io));
        };
        if let Err(e) = alles_schreiben(&*self.plattform, &g, &HALLO) {
            self.pool.zurueck(g);
            return Err(Fehler::os(Code::Io, e));
        }
        Ok(VerbindungKern {
            plattform: Arc::clone(&self.plattform),
            seite: Seite::Server {
                pool: Arc::clone(&self.pool),
                griff: Some(g),
            },
            gehalten: self.prozess.take(),
            bindung: self.bindung,
            peer: self.peer.clone(),
            lesefrist: None,
        })
    }

    /// Gives the event `pk.aufrufer_abgewiesen` to the hook, then closes without an answer (decision 4).
    pub(crate) fn ablehnen(mut self, grund: Kennung) -> Ereignis {
        let ereignis = Ereignis {
            kanal: self.kanal.clone(),
            grund: Grund::Aufrufer(grund),
            peer: Some(self.peer.clone()),
        };
        let ereignis = match self.griff.take() {
            Some(g) => melden_und_trennen(&self.pool, self.haken.as_ref(), g, ereignis),
            None => ereignis,
        };
        self.prozess = None;
        ereignis
    }
}

impl<P: Plattform> Drop for VorlaeufigKern<P> {
    fn drop(&mut self) {
        if let Some(g) = self.griff.take() {
            self.pool.zurueck(g);
        }
    }
}

enum Seite<P: Plattform> {
    Server {
        pool: Arc<Pool<P>>,
        griff: Option<P::Griff>,
    },
    Client {
        griff: P::Griff,
    },
}

/// A connection after both checks: a byte stream.
pub(crate) struct VerbindungKern<P: Plattform> {
    plattform: Arc<P>,
    seite: Seite<P>,
    /// Holds the bound process as long as the connection lives.
    gehalten: Option<P::Prozess>,
    bindung: Option<Prozessbindung>,
    pub(crate) peer: Peer,
    /// Decision 2: the time limit of each read; `None` reads without one.
    lesefrist: Option<Duration>,
}

impl<P: Plattform> VerbindungKern<P> {
    pub(crate) fn lesefrist(&mut self, frist: Option<Duration>) {
        self.lesefrist = frist;
    }

    fn griff(&self) -> Option<&P::Griff> {
        match &self.seite {
            Seite::Server { griff, .. } => griff.as_ref(),
            Seite::Client { griff } => Some(griff),
        }
    }

    pub(crate) fn prozess(&self) -> Option<&Prozessbindung> {
        self.bindung.as_ref()
    }

    #[cfg(windows)]
    pub(crate) fn prozess_roh(&self) -> Option<&P::Prozess> {
        self.gehalten.as_ref()
    }

    pub(crate) fn lesen(&self, puffer: &mut [u8]) -> std::io::Result<usize> {
        let Some(g) = self.griff() else {
            return Ok(0);
        };
        match self.plattform.lesen(g, puffer, self.lesefrist) {
            Lesen::Daten(n) => Ok(n),
            Lesen::Ende => Ok(0),
            Lesen::Frist => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                Fehler::neu(Code::Io),
            )),
            Lesen::Fehler(e) => Err(std::io::Error::other(Fehler::os(Code::Io, e))),
        }
    }

    pub(crate) fn schreiben(&self, daten: &[u8]) -> std::io::Result<usize> {
        let Some(g) = self.griff() else {
            return Err(std::io::Error::other(Fehler::neu(Code::Io)));
        };
        self.plattform
            .schreiben(g, daten)
            .map_err(|e| std::io::Error::other(Fehler::os(Code::Io, e)))
    }
}

impl<P: Plattform> Drop for VerbindungKern<P> {
    fn drop(&mut self) {
        if let Seite::Server { pool, griff } = &mut self.seite
            && let Some(g) = griff.take()
        {
            pool.zurueck(g);
        }
    }
}

/// "Client verbinden", steps 2 to 5: an instance whose owner is the expected SID, with no byte sent. The same for
/// [`verbinden`] and [`verbinden_ohne_hallo`] (decision 3): same check, same codes.
fn erreichen<P: Plattform>(
    p: &P,
    v: &ClientVorgabe,
    ende: Instant,
) -> Result<(P::Griff, String), Fehler> {
    let Erwartet::Windows { sid: erwartet } = &v.erwartet else {
        return Err(Fehler::neu(Code::VorgabeUngueltig));
    };
    let name = v.name.als_str();
    // Steps 2 and 3: reach an instance.
    let g = loop {
        match p.oeffnen(name) {
            Ok(g) => break g,
            Err(OS_BELEGT) => {
                let rest = ende.saturating_duration_since(Instant::now());
                if rest.is_zero() {
                    return Err(Fehler::os(Code::NichtErreichbar, OS_BELEGT));
                }
                p.warten(name, rest);
            }
            Err(OS_NICHT_GEFUNDEN) => {
                return Err(Fehler::os(Code::NichtErreichbar, OS_NICHT_GEFUNDEN));
            }
            Err(OS_ZUGRIFF) => return Err(Fehler::os(Code::ZugriffVerweigert, OS_ZUGRIFF)),
            Err(e) => return Err(Fehler::os(Code::Io, e)),
        }
    };
    // Steps 4 and 5: the owner of the instance, before any byte.
    let besitzer = p
        .besitzer(&g)
        .map_err(|e| Fehler::os(Code::ServerUnmessbar, e))?;
    if besitzer != erwartet.als_str() {
        return Err(Fehler::neu(Code::FalscherServer));
    }
    Ok((g, besitzer))
}

fn client_verbindung<P: Plattform>(
    plattform: Arc<P>,
    g: P::Griff,
    besitzer: String,
) -> VerbindungKern<P> {
    let peer = Peer {
        sid: Some(besitzer),
        pid: plattform.server_pid(&g),
        eingeschraenkt: Some(false),
        ..Peer::leer(System::Windows)
    };
    VerbindungKern {
        plattform,
        seite: Seite::Client { griff: g },
        gehalten: None,
        bindung: None,
        peer,
        lesefrist: None,
    }
}

/// "Client verbinden" without the hello (decision 3): the check of the owner as in [`verbinden`], then the connection as it
/// is. For a server that speaks no `pk1` (the control pipe of the platform service); there is no way without an expected owner.
pub(crate) fn verbinden_ohne_hallo<P: Plattform>(
    plattform: Arc<P>,
    v: &ClientVorgabe,
) -> Result<VerbindungKern<P>, Fehler> {
    let ende = Instant::now() + v.frist;
    let (g, besitzer) = erreichen(&*plattform, v, ende)?;
    Ok(client_verbindung(plattform, g, besitzer))
}

/// "Client verbinden", steps 2 to 6; step 1 (the settings) is checked by the caller.
#[allow(clippy::many_single_char_names)]
pub(crate) fn verbinden<P: Plattform>(
    plattform: Arc<P>,
    v: &ClientVorgabe,
) -> Result<VerbindungKern<P>, Fehler> {
    let p = &*plattform;
    let ende = Instant::now() + v.frist;
    let (g, besitzer) = erreichen(p, v, ende)?;
    // Step 6: the hello and its answer within the rest of the time limit.
    if alles_schreiben(p, &g, &HALLO).is_err() {
        return Err(Fehler::neu(Code::Abgewiesen));
    }
    let mut antwort = [0u8; 4];
    let mut n = 0;
    while n < antwort.len() {
        let rest = ende.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            return Err(Fehler::neu(Code::Abgewiesen));
        }
        match p.lesen(&g, &mut antwort[n..], Some(rest)) {
            Lesen::Daten(k) if k > 0 => n += k,
            _ => return Err(Fehler::neu(Code::Abgewiesen)),
        }
    }
    if antwort != HALLO {
        return Err(Fehler::neu(Code::Abgewiesen));
    }
    Ok(client_verbindung(plattform, g, besitzer))
}

#[cfg(test)]
#[allow(clippy::many_single_char_names, clippy::assert_is_empty)]
mod tests {
    use super::*;
    use crate::kennung::{PackageSid, Sid};
    use std::collections::VecDeque;

    const PERSON: &str = "S-1-5-21-1000000001-1000000002-1000000003-1001";
    const DIENST: &str = "S-1-5-80-1000000011-1000000012-1000000013-1000000014-1000000015";
    const A: &str =
        "S-1-15-2-1000000021-1000000022-1000000023-1000000024-1000000025-1000000026-1000000027";

    /// A stand-in of the system with one client at a time.
    struct Attrappe {
        z: Mutex<Zustand>,
    }

    struct Zustand {
        eigene: String,
        belegt: bool,
        angelegt: Vec<(bool, String)>,
        getrennt: usize,
        von_client: VecDeque<u8>,
        client_sendet: Vec<u8>,
        gewartet: Vec<u32>,
        client_schliesst: bool,
        an_client: Vec<u8>,
        an_server: Vec<u8>,
        token: Result<Token, i32>,
        pid: Option<u32>,
        prozess: Option<u64>,
        lebt: bool,
        im_job: Option<bool>,
        jetzt: u64,
        oeffnen: VecDeque<i32>,
        besitzer: Result<String, i32>,
        antwort: Vec<u8>,
        protokoll: Vec<&'static str>,
        fristen: Vec<Option<Duration>>,
        eigene_fehler: Option<i32>,
    }

    impl Attrappe {
        fn neu() -> Arc<Self> {
            Arc::new(Self {
                z: Mutex::new(Zustand {
                    eigene: DIENST.to_owned(),
                    belegt: false,
                    angelegt: Vec::new(),
                    getrennt: 0,
                    von_client: VecDeque::new(),
                    client_sendet: HALLO.to_vec(),
                    gewartet: Vec::new(),
                    client_schliesst: false,
                    an_client: Vec::new(),
                    an_server: Vec::new(),
                    token: Ok(Token {
                        sid: PERSON.to_owned(),
                        appcontainer_sid: None,
                        eingeschraenkt: false,
                    }),
                    pid: Some(4242),
                    prozess: Some(50),
                    lebt: true,
                    im_job: None,
                    jetzt: 100,
                    oeffnen: VecDeque::new(),
                    besitzer: Ok(DIENST.to_owned()),
                    antwort: HALLO.to_vec(),
                    protokoll: Vec::new(),
                    fristen: Vec::new(),
                    eigene_fehler: None,
                }),
            })
        }

        fn fuer_client() -> Arc<Self> {
            let a = Self::neu();
            a.z().client_sendet.clear();
            a
        }

        fn z(&self) -> MutexGuard<'_, Zustand> {
            sperren(&self.z)
        }
    }

    impl Plattform for Attrappe {
        type Griff = u32;
        type Prozess = u64;
        type Job = ();

        fn eigene_sid(&self) -> Result<String, i32> {
            let z = self.z();
            z.eigene_fehler.map_or_else(|| Ok(z.eigene.clone()), Err)
        }
        fn instanz_anlegen(
            &self,
            _name: &str,
            erste: bool,
            _n: u8,
            sddl: &str,
        ) -> Result<u32, i32> {
            let mut z = self.z();
            if erste && z.belegt {
                return Err(OS_ZUGRIFF);
            }
            z.angelegt.push((erste, sddl.to_owned()));
            z.protokoll.push("anlegen");
            Ok(u32::try_from(z.angelegt.len()).expect("klein"))
        }
        fn auf_client_warten(&self, g: &u32) -> Result<(), i32> {
            let mut z = self.z();
            z.gewartet.push(*g);
            z.von_client = z.client_sendet.iter().copied().collect();
            z.protokoll.push("verbunden");
            Ok(())
        }
        fn jetzt(&self) -> u64 {
            self.z().jetzt
        }
        fn client_pid(&self, _g: &u32) -> Option<u32> {
            self.z().pid
        }
        fn prozess_oeffnen(&self, _pid: u32) -> Option<(u64, u64)> {
            let mut z = self.z();
            z.protokoll.push("prozess");
            z.prozess.map(|s| (s, s))
        }
        fn programm(&self, _p: &u64) -> Option<String> {
            Some("C:\\p.exe".to_owned())
        }
        fn lebt(&self, _p: &u64) -> Option<bool> {
            Some(self.z().lebt)
        }
        fn im_job(&self, _p: &u64, _job: &()) -> Option<bool> {
            self.z().im_job
        }
        fn kennung_messen(&self, _g: &u32) -> Result<Token, i32> {
            let mut z = self.z();
            z.protokoll.push("token");
            z.token.clone()
        }
        fn trennen(&self, _g: &u32) {
            let mut z = self.z();
            z.getrennt += 1;
            z.protokoll.push("trennen");
        }
        fn lesen(&self, _g: &u32, puffer: &mut [u8], frist: Option<Duration>) -> Lesen {
            let mut z = self.z();
            z.protokoll.push("lesen");
            z.fristen.push(frist);
            if let Some(b) = z.von_client.pop_front() {
                puffer[0] = b;
                return Lesen::Daten(1);
            }
            if !z.antwort.is_empty() && !z.an_server.is_empty() {
                let n = z.antwort.len().min(puffer.len());
                let teil: Vec<u8> = z.antwort.drain(..n).collect();
                puffer[..n].copy_from_slice(&teil);
                return Lesen::Daten(n);
            }
            if z.client_schliesst {
                Lesen::Ende
            } else {
                Lesen::Frist
            }
        }
        fn schreiben(&self, _g: &u32, daten: &[u8]) -> Result<usize, i32> {
            let mut z = self.z();
            z.protokoll.push("schreiben");
            z.an_client.extend_from_slice(daten);
            z.an_server.extend_from_slice(daten);
            Ok(daten.len())
        }
        fn oeffnen(&self, _name: &str) -> Result<u32, i32> {
            let mut z = self.z();
            z.protokoll.push("oeffnen");
            z.oeffnen.pop_front().map_or(Ok(1), Err)
        }
        fn warten(&self, _name: &str, _frist: Duration) {
            self.z().protokoll.push("warten");
        }
        fn besitzer(&self, _g: &u32) -> Result<String, i32> {
            let mut z = self.z();
            z.protokoll.push("besitzer");
            z.besitzer.clone()
        }
        fn server_pid(&self, _g: &u32) -> Option<u32> {
            Some(77)
        }
    }

    fn vorgabe(erlaubt: Vec<Erlaubnis>, instanzen: u8) -> ServerVorgabe {
        let mut v = ServerVorgabe::neu(
            Kennung::neu("k-beispiel").expect("form"),
            erlaubt,
            instanzen,
        );
        v.hallo_frist = Duration::from_millis(50);
        v
    }

    fn person(ac: Option<&str>) -> Erlaubnis {
        Erlaubnis::Windows {
            sid: Sid::neu(PERSON).expect("form"),
            appcontainer_sid: ac.map(|p| PackageSid::neu(p).expect("form")),
        }
    }

    fn server(a: &Arc<Attrappe>, erlaubt: Vec<Erlaubnis>, instanzen: u8) -> ServerKern<Attrappe> {
        ServerKern::anlegen(Arc::clone(a), vorgabe(erlaubt, instanzen), None, &[]).expect("anlegen")
    }

    fn abgewiesen(r: Result<AnnahmeKern<Attrappe>, Fehler>) -> Ereignis {
        match r.expect("annehmen") {
            AnnahmeKern::Abgewiesen(e) => e,
            AnnahmeKern::Vorlaeufig(_) => panic!("vorläufig angenommen"),
        }
    }

    fn vorlaeufig(r: Result<AnnahmeKern<Attrappe>, Fehler>) -> VorlaeufigKern<Attrappe> {
        match r.expect("annehmen") {
            AnnahmeKern::Vorlaeufig(v) => v,
            AnnahmeKern::Abgewiesen(e) => panic!("abgewiesen: {}", e.als_json()),
        }
    }

    #[test]
    fn anlegen_erste_instanz_mit_flag_und_sddl() {
        let a = Attrappe::neu();
        let _s = server(&a, vec![person(Some(A))], 4);
        let z = a.z();
        assert_eq!(z.angelegt.len(), 1);
        assert!(
            z.angelegt[0].0,
            "die erste Instanz trägt FILE_FLAG_FIRST_PIPE_INSTANCE"
        );
        assert_eq!(
            z.angelegt[0].1,
            format!(
                "O:{DIENST}D:P(A;;0x12019F;;;{DIENST})(A;;0x12019B;;;{PERSON})(A;;0x12019B;;;{A})S:(ML;;NW;;;LW)"
            )
        );
    }

    #[test]
    fn name_belegt_ohne_rueckfall() {
        let a = Attrappe::neu();
        a.z().belegt = true;
        let r = ServerKern::anlegen(Arc::clone(&a), vorgabe(vec![person(None)], 4), None, &[]);
        assert_eq!(r.err(), Some(Fehler::os(Code::NameBelegt, OS_ZUGRIFF)));
        assert!(a.z().angelegt.is_empty(), "keine Instanz ohne das Flag");
    }

    #[test]
    fn erlaubt_kein_byte_vor_dem_haken() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 4);
        let v = vorlaeufig(s.annehmen());
        assert!(a.z().an_client.is_empty(), "kein Byte vor dem Haken");
        assert_eq!(v.peer.sid.as_deref(), Some(PERSON));
        assert_eq!(v.peer.pid, Some(4242));
        assert_eq!(v.peer.startzeit, Some(50));
        assert_eq!(
            v.prozess(),
            Some(&Prozessbindung {
                pid: 4242,
                startzeit: Some(50)
            })
        );
        let verbindung = v.bestaetigen().expect("bestätigen");
        assert_eq!(a.z().an_client, HALLO.to_vec(), "erst jetzt das Hallo");
        assert_eq!(verbindung.peer.eingeschraenkt, Some(false));
    }

    #[test]
    fn haken_lehnt_ab_ohne_byte() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 4);
        let v = vorlaeufig(s.annehmen());
        let e = v.ablehnen(Kennung::neu("sitzung-fremd").expect("form"));
        assert!(a.z().an_client.is_empty(), "auch kein Hallo");
        assert_eq!(e.code(), Code::AufruferAbgewiesen);
        assert_eq!(e.als_json()["grund"], "aufrufer:sitzung-fremd");
        assert!(a.z().getrennt >= 1);
    }

    #[test]
    fn fallen_gelassen_schliesst() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 4);
        drop(vorlaeufig(s.annehmen()));
        assert!(a.z().an_client.is_empty());
        assert_eq!(a.z().getrennt, 1);
    }

    #[test]
    fn hallo_vor_der_messung() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 4);
        let _v = vorlaeufig(s.annehmen());
        let z = a.z();
        let erstes_lesen = z
            .protokoll
            .iter()
            .position(|p| *p == "lesen")
            .expect("gelesen");
        let token = z
            .protokoll
            .iter()
            .position(|p| *p == "token")
            .expect("gemessen");
        let prozess = z
            .protokoll
            .iter()
            .position(|p| *p == "prozess")
            .expect("gebunden");
        assert!(
            prozess < erstes_lesen && erstes_lesen < token,
            "{:?}",
            z.protokoll
        );
    }

    #[test]
    fn naechste_instanz_vor_der_uebergabe() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 2);
        let _v = vorlaeufig(s.annehmen());
        assert_eq!(a.z().angelegt.len(), 2, "die nächste Instanz wartet schon");
        // The limit: with one instance there is no second one, the connected one keeps the name.
        let b = Attrappe::neu();
        let s1 = server(&b, vec![person(None)], 1);
        let _w = vorlaeufig(s1.annehmen());
        assert_eq!(b.z().angelegt.len(), 1);
    }

    /// A new instance takes clients before anyone waits on it; the next `annehmen` must wait on it, not on a disconnected one
    /// (measured in the VM: otherwise a client sits on the new instance until its time limit ends).
    #[test]
    fn frische_instanz_zuerst() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 4);
        let v = vorlaeufig(s.annehmen()).bestaetigen().expect("bestätigen");
        assert_eq!(a.z().angelegt.len(), 2, "Instanz 2 wartet");
        drop(v);
        let _w = vorlaeufig(s.annehmen());
        assert_eq!(a.z().gewartet, vec![1, 2], "die frische Instanz 2 zuerst");
    }

    #[test]
    fn abgewiesene_instanz_bleibt_dem_server() {
        let a = Attrappe::neu();
        a.z().client_sendet = b"GET /\n".to_vec();
        let s = server(&a, vec![person(None)], 1);
        let e = abgewiesen(s.annehmen());
        assert_eq!(e.grund, Grund::HalloFalsch);
        assert_eq!(e.peer, None);
        assert!(a.z().an_client.is_empty());
        // The instance was disconnected, not closed, and waits again.
        assert_eq!(sperren(&s.pool.zustand).getrennt.len(), 1);
        assert_eq!(sperren(&s.pool.zustand).vorhanden, 1);
    }

    #[test]
    fn hallo_fehlt_und_falsch() {
        let a = Attrappe::neu();
        a.z().client_sendet.clear();
        let s = server(&a, vec![person(None)], 4);
        assert_eq!(abgewiesen(s.annehmen()).grund, Grund::HalloFehlt);
        // Decision 5: the end of the connection after a fitting start is `pk.hallo_fehlt`.
        a.z().client_sendet = b"pk".to_vec();
        a.z().client_schliesst = true;
        assert_eq!(abgewiesen(s.annehmen()).grund, Grund::HalloFehlt);
        a.z().client_sendet = b"pk2\n".to_vec();
        let e = abgewiesen(s.annehmen());
        assert_eq!(e.grund, Grund::HalloFalsch);
        assert!(
            !e.als_json().to_string().contains("pk1"),
            "die Bytes stehen nie im Ereignis"
        );
    }

    /// Decision 5: wrong bytes give `pk.hallo_falsch` as soon as they come, also when the connection ends after them; an end
    /// or the time limit before 4 fitting bytes gives `pk.hallo_fehlt`, also without any byte (the runner of
    /// `w-pipe-sicherheit` opens and closes the pipe).
    #[test]
    fn ende_vor_vier_byte() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 4);
        a.z().client_schliesst = true;
        for (bytes, grund) in [
            (&b""[..], Grund::HalloFehlt),
            (b"p", Grund::HalloFehlt),
            (b"pk", Grund::HalloFehlt),
            (b"pk1", Grund::HalloFehlt),
            (b"x", Grund::HalloFalsch),
            (b"pq", Grund::HalloFalsch),
            (b"pk1x", Grund::HalloFalsch),
            (b"pk1\r", Grund::HalloFalsch),
        ] {
            a.z().client_sendet = bytes.to_vec();
            let e = abgewiesen(s.annehmen());
            assert_eq!(e.grund, grund, "{bytes:?}");
            assert_eq!(e.code(), Code::HalloUngueltig);
        }
        // Without an end the time limit ends the wait: also `pk.hallo_fehlt`.
        a.z().client_schliesst = false;
        a.z().client_sendet = b"pk".to_vec();
        assert_eq!(abgewiesen(s.annehmen()).grund, Grund::HalloFehlt);
        assert!(a.z().an_client.is_empty());
    }

    /// Decision 4: the hook gets the event while the instance is still connected; the end comes after it, without a drop of
    /// a value of the caller. For a refusal of the library and for one of the caller.
    #[test]
    fn ereignis_vor_dem_trennen() {
        let a = Attrappe::neu();
        let mut s = server(&a, vec![person(None)], 4);
        let gesehen: Arc<Mutex<Vec<(String, usize)>>> = Arc::default();
        let (a2, g2) = (Arc::clone(&a), Arc::clone(&gesehen));
        s.haken = Some(Arc::new(move |e: &Ereignis| {
            let getrennt = a2.z().getrennt;
            sperren(&g2).push((e.grund.als_text(), getrennt));
        }));
        a.z().client_sendet = b"GET /\n".to_vec();
        let e = abgewiesen(s.annehmen());
        assert_eq!(a.z().getrennt, 1, "danach getrennt");
        assert_eq!(*sperren(&gesehen), vec![("pk.hallo_falsch".to_owned(), 0)]);
        assert_eq!(e.grund, Grund::HalloFalsch, "dasselbe Ereignis zurück");
        a.z().client_sendet = HALLO.to_vec();
        let v = vorlaeufig(s.annehmen());
        let e = v.ablehnen(Kennung::neu("sitzung-fremd").expect("form"));
        assert_eq!(e.als_json()["grund"], "aufrufer:sitzung-fremd");
        assert_eq!(a.z().getrennt, 2);
        assert_eq!(
            sperren(&gesehen)[1],
            ("aufrufer:sitzung-fremd".to_owned(), 1),
            "der Haken sah die Verbindung noch offen"
        );
        // Without a hook nothing changes: the event comes back and the instance is disconnected.
        let b = Attrappe::neu();
        b.z().client_sendet.clear();
        let s2 = server(&b, vec![person(None)], 4);
        assert_eq!(abgewiesen(s2.annehmen()).grund, Grund::HalloFehlt);
        assert_eq!(b.z().getrennt, 1);
    }

    #[test]
    fn eigene_sid_als_sid() {
        let a = Attrappe::neu();
        assert_eq!(
            eigene_sid(&*a).map(|s| s.als_str().to_owned()),
            Ok(DIENST.to_owned())
        );
        a.z().eigene = "S-1-5-19".to_owned();
        assert_eq!(eigene_sid(&*a), Err(Fehler::neu(Code::Io)), "LOCAL SERVICE");
        a.z().eigene_fehler = Some(5);
        assert_eq!(eigene_sid(&*a), Err(Fehler::os(Code::Io, 5)));
    }

    #[test]
    fn messung_gescheitert() {
        let a = Attrappe::neu();
        a.z().token = Err(1368);
        let s = server(&a, vec![person(None)], 4);
        let e = abgewiesen(s.annehmen());
        assert_eq!((e.grund, e.peer), (Grund::MessungGescheitert, None));
    }

    #[test]
    fn prozess_pflicht() {
        let a = Attrappe::neu();
        a.z().prozess = None;
        let mut v = vorgabe(vec![person(None)], 4);
        v.prozess_pflicht = true;
        let s = ServerKern::anlegen(Arc::clone(&a), v, None, &[]).expect("anlegen");
        let e = abgewiesen(s.annehmen());
        assert_eq!(e.grund, Grund::ProzessNichtOffen);
        let peer = e.peer.expect("peer");
        assert_eq!(
            (peer.pid, peer.startzeit, peer.programm),
            (Some(4242), None, None)
        );
        // Without the duty the step is skipped, and the start time is null.
        let s2 = server(&a, vec![person(None)], 4);
        let w = vorlaeufig(s2.annehmen());
        assert_eq!(
            w.prozess(),
            Some(&Prozessbindung {
                pid: 4242,
                startzeit: None
            })
        );
    }

    #[test]
    fn job_gemeldet_und_unbekannt() {
        let a = Attrappe::neu();
        a.z().im_job = Some(false);
        let s = ServerKern::anlegen(
            Arc::clone(&a),
            vorgabe(vec![person(Some(A))], 4),
            Some(()),
            &[],
        )
        .expect("anlegen");
        a.z().token = Ok(Token {
            sid: PERSON.to_owned(),
            appcontainer_sid: Some(A.to_owned()),
            eingeschraenkt: false,
        });
        let v = vorlaeufig(s.annehmen());
        assert_eq!(v.peer.im_job, Some(false), "gemeldet, nicht abgewiesen");
        drop(v);
        a.z().im_job = None;
        assert_eq!(abgewiesen(s.annehmen()).grund, Grund::JobUnbekannt);
    }

    #[test]
    fn startzeit_und_ende() {
        let a = Attrappe::neu();
        a.z().prozess = Some(101);
        let s = server(&a, vec![person(None)], 4);
        let e = abgewiesen(s.annehmen());
        assert_eq!(e.grund, Grund::StartzeitWeichtAb);
        assert_eq!(e.peer.and_then(|p| p.startzeit), Some(101));
        a.z().prozess = Some(50);
        a.z().lebt = false;
        assert_eq!(abgewiesen(s.annehmen()).grund, Grund::ProzessBeendet);
    }

    #[test]
    fn liste_und_einschraenkung() {
        let a = Attrappe::neu();
        let s = server(&a, vec![person(None)], 4);
        a.z().token = Ok(Token {
            sid: PERSON.to_owned(),
            appcontainer_sid: Some(A.to_owned()),
            eingeschraenkt: false,
        });
        let e = abgewiesen(s.annehmen());
        assert_eq!(e.grund, Grund::AppcontainerFalsch);
        assert_eq!(e.peer.and_then(|p| p.appcontainer_sid).as_deref(), Some(A));
        a.z().token = Ok(Token {
            sid: PERSON.to_owned(),
            appcontainer_sid: None,
            eingeschraenkt: true,
        });
        let e = abgewiesen(s.annehmen());
        assert_eq!(e.grund, Grund::Eingeschraenkt);
        assert_eq!(e.peer.and_then(|p| p.eingeschraenkt), Some(true));
        assert!(a.z().an_client.is_empty());
    }

    fn client(erwartet: &str) -> ClientVorgabe {
        ClientVorgabe {
            name: Kennung::neu("k-beispiel").expect("form"),
            erwartet: Erwartet::Windows {
                sid: Sid::neu(erwartet).expect("form"),
            },
            frist: Duration::from_millis(50),
            unix_ordner: None,
        }
    }

    #[test]
    fn client_falscher_server_kein_byte() {
        let a = Attrappe::fuer_client();
        a.z().besitzer = Ok(PERSON.to_owned());
        let r = verbinden(Arc::clone(&a), &client(DIENST));
        assert_eq!(r.err(), Some(Fehler::neu(Code::FalscherServer)));
        assert!(
            a.z().an_server.is_empty(),
            "0 Byte gesendet, auch kein Hallo"
        );
    }

    #[test]
    fn client_besitzer_vor_dem_hallo() {
        let a = Attrappe::fuer_client();
        let v = verbinden(Arc::clone(&a), &client(DIENST)).expect("verbinden");
        assert_eq!(v.peer.sid.as_deref(), Some(DIENST));
        assert_eq!(
            (v.peer.pid, v.peer.startzeit, v.peer.programm.clone()),
            (Some(77), None, None)
        );
        let z = a.z();
        let b = z
            .protokoll
            .iter()
            .position(|p| *p == "besitzer")
            .expect("geprüft");
        let s = z
            .protokoll
            .iter()
            .position(|p| *p == "schreiben")
            .expect("gesendet");
        assert!(b < s);
        assert_eq!(z.an_server, HALLO.to_vec());
    }

    #[test]
    fn client_fehlerwege() {
        let a = Attrappe::fuer_client();
        a.z().besitzer = Err(5);
        assert_eq!(
            verbinden(Arc::clone(&a), &client(DIENST))
                .err()
                .map(|f| f.code),
            Some(Code::ServerUnmessbar)
        );
        let a = Attrappe::fuer_client();
        a.z().oeffnen = [OS_NICHT_GEFUNDEN].into();
        assert_eq!(
            verbinden(Arc::clone(&a), &client(DIENST))
                .err()
                .map(|f| f.code),
            Some(Code::NichtErreichbar)
        );
        let a = Attrappe::fuer_client();
        a.z().oeffnen = [OS_ZUGRIFF].into();
        assert_eq!(
            verbinden(Arc::clone(&a), &client(DIENST))
                .err()
                .map(|f| f.code),
            Some(Code::ZugriffVerweigert)
        );
        let a = Attrappe::fuer_client();
        a.z().oeffnen = [OS_BELEGT, OS_BELEGT].into();
        assert!(
            verbinden(Arc::clone(&a), &client(DIENST)).is_ok(),
            "wartet und versucht wieder"
        );
        assert_eq!(
            a.z().protokoll.iter().filter(|p| **p == "warten").count(),
            2
        );
        let a = Attrappe::fuer_client();
        a.z().antwort.clear();
        assert_eq!(
            verbinden(Arc::clone(&a), &client(DIENST))
                .err()
                .map(|f| f.code),
            Some(Code::Abgewiesen)
        );
        let a = Attrappe::fuer_client();
        a.z().antwort = b"pk2\n".to_vec();
        assert_eq!(
            verbinden(Arc::clone(&a), &client(DIENST))
                .err()
                .map(|f| f.code),
            Some(Code::Abgewiesen)
        );
    }

    /// Decision 2: the time limit goes to every following read; when it passes the error has the kind `TimedOut`. `None`
    /// reads without a limit again.
    #[test]
    fn lesefrist_jedes_folgende_lesen() {
        let a = Attrappe::fuer_client();
        let mut v = verbinden(Arc::clone(&a), &client(DIENST)).expect("verbinden");
        a.z().fristen.clear();
        let mut puffer = [0u8; 8];
        assert!(v.lesen(&mut puffer).is_err());
        assert_eq!(a.z().fristen, vec![None], "ohne Frist zu Beginn");
        v.lesefrist(Some(Duration::from_millis(30)));
        for _ in 0..2 {
            let fehler = v.lesen(&mut puffer).expect_err("Frist");
            assert_eq!(fehler.kind(), std::io::ErrorKind::TimedOut);
        }
        v.lesefrist(None);
        let _ = v.lesen(&mut puffer);
        assert_eq!(
            a.z().fristen,
            vec![
                None,
                Some(Duration::from_millis(30)),
                Some(Duration::from_millis(30)),
                None
            ]
        );
        // Server side as well.
        let b = Attrappe::neu();
        let s = server(&b, vec![person(None)], 4);
        let mut w = vorlaeufig(s.annehmen()).bestaetigen().expect("bestätigen");
        b.z().antwort.clear();
        w.lesefrist(Some(Duration::from_secs(10)));
        let fehler = w.lesen(&mut puffer).expect_err("Frist");
        assert_eq!(fehler.kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(b.z().fristen.last(), Some(&Some(Duration::from_secs(10))));
    }

    /// Decision 3: without the hello the client checks the owner the same way and sends no byte at all.
    #[test]
    fn ohne_hallo_prueft_den_besitzer_und_sendet_nichts() {
        let a = Attrappe::fuer_client();
        let v = verbinden_ohne_hallo(Arc::clone(&a), &client(DIENST)).expect("verbinden");
        assert_eq!(v.peer.sid.as_deref(), Some(DIENST));
        assert_eq!(v.peer.pid, Some(77));
        let z = a.z();
        assert!(z.an_server.is_empty(), "kein pk1");
        assert!(z.protokoll.contains(&"besitzer"));
        assert!(!z.protokoll.contains(&"schreiben"));
        assert!(!z.protokoll.contains(&"lesen"));
    }

    /// Counter-check of decision 3: a wrong owner is `pk.falscher_server` with 0 bytes.
    #[test]
    fn ohne_hallo_falscher_server_kein_byte() {
        let a = Attrappe::fuer_client();
        a.z().besitzer = Ok(PERSON.to_owned());
        let r = verbinden_ohne_hallo(Arc::clone(&a), &client(DIENST));
        assert_eq!(r.err(), Some(Fehler::neu(Code::FalscherServer)));
        assert!(a.z().an_server.is_empty(), "0 Byte gesendet");
        assert!(!a.z().protokoll.contains(&"schreiben"));
    }

    /// Decision 3: the same codes as `verbinden` on every way of the check.
    #[test]
    fn ohne_hallo_gleiche_codes() {
        type Einrichten = fn(&Attrappe);
        let wege: [Einrichten; 5] = [
            |a| a.z().besitzer = Err(5),
            |a| a.z().besitzer = Ok(PERSON.to_owned()),
            |a| a.z().oeffnen = [OS_NICHT_GEFUNDEN].into(),
            |a| a.z().oeffnen = [OS_ZUGRIFF].into(),
            |a| a.z().oeffnen = [1450].into(),
        ];
        for weg in wege {
            let (a, b) = (Attrappe::fuer_client(), Attrappe::fuer_client());
            weg(&a);
            weg(&b);
            let mit = verbinden(Arc::clone(&a), &client(DIENST)).err();
            let ohne = verbinden_ohne_hallo(Arc::clone(&b), &client(DIENST)).err();
            assert!(mit.is_some());
            assert_eq!(mit, ohne);
            assert!(b.z().an_server.is_empty());
        }
        let a = Attrappe::fuer_client();
        a.z().oeffnen = std::iter::repeat_n(OS_BELEGT, 1_000_000).collect();
        let mut c = client(DIENST);
        c.frist = Duration::from_millis(20);
        assert_eq!(
            verbinden_ohne_hallo(Arc::clone(&a), &c).err(),
            Some(Fehler::os(Code::NichtErreichbar, OS_BELEGT))
        );
    }

    #[test]
    fn client_wartet_hoechstens_die_frist() {
        let a = Attrappe::fuer_client();
        a.z().oeffnen = std::iter::repeat_n(OS_BELEGT, 1_000_000).collect();
        let mut c = client(DIENST);
        c.frist = Duration::from_millis(30);
        let start = Instant::now();
        assert_eq!(
            verbinden(Arc::clone(&a), &c).err().map(|f| f.code),
            Some(Code::NichtErreichbar)
        );
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
