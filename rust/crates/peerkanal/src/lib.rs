//! A local channel with a check of the other side (contract `rust/spec/peerkanal/README.md`, version 1).
//!
//! A server offers a channel. For each connection the library measures the identity of the other side and lets only
//! identities from the list of the caller through; the caller then decides in a hook ([`Vorlaeufig`]) before the first byte
//! goes back. A client checks the server against an expected identity before it sends a byte.
//!
//! **Windows** (this build): a named pipe `\\.\pipe\<name>`. The server creates the first instance with
//! `FILE_FLAG_FIRST_PIPE_INSTANCE` and a protected DACL of the own SID, the allowed SIDs and package SIDs (build plan,
//! blocks B1 to B9). The client checks the owner of the pipe instance and opens with identification only.
//!
//! **Linux and macOS**: a stream socket `<ordner>/<name>.sock` (module `unix`). The server measures uid and gid of each client
//! from the kernel (Linux `SO_PEERCRED`, macOS `LOCAL_PEERCRED`); the client checks the uid of the server the same way before
//! it sends a byte. When the kernel gives no uid, there is no connection (fail closed). Other targets keep the stand-in that
//! refuses every call.
//!
//! The interface needs no `unsafe`; a caller may set `#![forbid(unsafe_code)]`. The crate itself has `unsafe` only in the
//! Windows part of its module `sys` (decision E1). The library writes no log and no file, reads no environment variable and
//! never ends the process, with one exception: when `RevertToSelf` fails, it aborts, because the thread would otherwise go on
//! with the identity of the client.

#![deny(unsafe_code)]

// Without Windows the system calls are a stand-in: the flags, the descriptor and parts of the order are used only by the
// Windows module and by the tests.
#[cfg_attr(not(windows), allow(dead_code))]
mod ablauf;
#[cfg_attr(not(windows), allow(dead_code))]
mod acl;
mod fehler;
mod kennung;
mod pruefung;
mod sys;
#[cfg(unix)]
mod unix;
mod vorgabe;

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::time::Duration;

pub use ablauf::Prozessbindung;
pub use fehler::{Code, Ereignis, Fehler, Grund, Peer, Signatur, System};
pub use kennung::{Kanalname, Kennung, PackageSid, Sid};
pub use vorgabe::{
    ClientVorgabe, Erlaubnis, Erwartet, JobBezug, ServerVorgabe, SocketModus, UnixOrt,
    client_json_lesen, server_json_pruefen,
};

use ablauf::{AnnahmeKern, ServerKern, VerbindungKern, VorlaeufigKern};
use sys::Os;

/// The hello: `pk1` and LF. The client sends it after its check of the server; the server answers with the same 4 bytes when
/// it accepts. It is no payload byte.
pub const HALLO: [u8; 4] = ablauf::HALLO;

/// A server of a channel.
pub struct Server {
    innen: ServerInnen,
}

/// The side of the system: the pipe side (Windows, and the stand-in that refuses every call on other targets) or the Unix
/// side (Linux and macOS).
enum ServerInnen {
    #[cfg_attr(unix, allow(dead_code))]
    Pipe(ServerKern<Os>),
    #[cfg(unix)]
    Unix(unix::Server),
}

impl Server {
    /// "Server anlegen": checks the settings, builds the security descriptor and creates the first instance with
    /// `FILE_FLAG_FIRST_PIPE_INSTANCE`. A taken name gives `pk.name_belegt`; there is no fallback.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig`, `pk.name_belegt` or `pk.anlegen_fehlgeschlagen` (Unix: also `pk.ordner_rechte` and
    /// `pk.pfad_zu_lang`).
    pub fn anlegen(v: ServerVorgabe) -> Result<Self, Fehler> {
        Self::anlegen_mit(v, &[], None)
    }

    fn anlegen_mit(
        v: ServerVorgabe,
        zusatz: &[&str],
        halt: Option<ablauf::Haltepunkt>,
    ) -> Result<Self, Fehler> {
        v.pruefen(System::aktuell())?;
        #[cfg(unix)]
        {
            let _ = (zusatz, halt);
            Ok(Self {
                innen: ServerInnen::Unix(unix::Server::anlegen(v)?),
            })
        }
        #[cfg(not(unix))]
        {
            if !cfg!(windows) {
                return Err(Fehler::neu(Code::AnlegenFehlgeschlagen));
            }
            let mut v = v;
            let job = v.job.take();
            let mut kern = ServerKern::anlegen(Arc::new(Os), v, job, zusatz)?;
            kern.nach_verbinden = halt;
            Ok(Self {
                innen: ServerInnen::Pipe(kern),
            })
        }
    }

    /// Sets the hook for events (decision 4): for each refusal the library builds the [`Ereignis`], calls the hook with it and
    /// only then disconnects the instance; afterwards it returns the same event ([`Annahme::Abgewiesen`] or
    /// [`Vorlaeufig::ablehnen`]). A caller that writes its log line in the hook has the line before the client sees the end.
    /// The hook runs on the thread of `annehmen` or `ablehnen` and should return soon. A later call replaces the hook.
    pub fn bei_abweisung(&mut self, haken: impl Fn(&Ereignis) + Send + Sync + 'static) {
        let haken = Arc::new(haken);
        match &mut self.innen {
            ServerInnen::Pipe(kern) => kern.haken = Some(haken),
            #[cfg(unix)]
            ServerInnen::Unix(u) => u.haken = Some(haken),
        }
    }

    /// "Annehmen und Messen": blocks until a client connects, then measures and checks it. Callable from up to `instanzen`
    /// threads at the same time.
    ///
    /// # Errors
    /// `pk.io` with `os_fehler` when the system fails while waiting for a client. A refused client is no error but
    /// [`Annahme::Abgewiesen`].
    pub fn annehmen(&self) -> Result<Annahme, Fehler> {
        Ok(match &self.innen {
            ServerInnen::Pipe(kern) => match kern.annehmen()? {
                AnnahmeKern::Vorlaeufig(v) => Annahme::Vorlaeufig(Vorlaeufig {
                    innen: VorlaeufigInnen::Pipe(v),
                }),
                AnnahmeKern::Abgewiesen(e) => Annahme::Abgewiesen(e),
            },
            #[cfg(unix)]
            ServerInnen::Unix(u) => match u.annehmen()? {
                unix::Annahme::Vorlaeufig(v) => Annahme::Vorlaeufig(Vorlaeufig {
                    innen: VorlaeufigInnen::Unix(v),
                }),
                unix::Annahme::Abgewiesen(e) => Annahme::Abgewiesen(e),
            },
        })
    }
}

/// The outcome of [`Server::annehmen`].
pub enum Annahme {
    /// The list allows the client; the caller decides in the hook.
    Vorlaeufig(Vorlaeufig),
    /// Refused after the system let the connection through. The caller writes the event into its log.
    Abgewiesen(Ereignis),
}

/// The name of a refusal by the caller; it becomes the reason `aufrufer:<kennung>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aufrufergrund(pub Kennung);

/// A connection in the hook of the caller (step 7): no byte has gone back to the client yet, and the library holds the
/// binding of the process. Dropped without a decision, it closes without an answer and without an event.
pub struct Vorlaeufig {
    innen: VorlaeufigInnen,
}

enum VorlaeufigInnen {
    #[cfg_attr(unix, allow(dead_code))]
    Pipe(VorlaeufigKern<Os>),
    #[cfg(unix)]
    Unix(unix::Vorlaeufig),
}

impl Vorlaeufig {
    /// The measured peer.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        match &self.innen {
            VorlaeufigInnen::Pipe(k) => &k.peer,
            #[cfg(unix)]
            VorlaeufigInnen::Unix(u) => &u.peer,
        }
    }

    /// The bound process: pid and start time. `None` when the pid could not be read.
    #[must_use]
    pub fn prozess(&self) -> Option<&Prozessbindung> {
        match &self.innen {
            VorlaeufigInnen::Pipe(k) => k.prozess(),
            #[cfg(unix)]
            VorlaeufigInnen::Unix(u) => u.prozess(),
        }
    }

    /// Sends the hello back and hands the connection over.
    ///
    /// # Errors
    /// `pk.io` when the hello cannot be written (for example, the client has gone).
    pub fn bestaetigen(self) -> Result<Verbindung, Fehler> {
        match self.innen {
            VorlaeufigInnen::Pipe(k) => k.bestaetigen().map(|kern| Verbindung {
                innen: VerbindungInnen::Pipe(kern),
            }),
            #[cfg(unix)]
            VorlaeufigInnen::Unix(u) => u.bestaetigen().map(|v| Verbindung {
                innen: VerbindungInnen::Unix(v),
            }),
        }
    }

    /// Closes without an answer and returns the event `pk.aufrufer_abgewiesen` with the reason `aufrufer:<kennung>`. The hook
    /// of [`Server::bei_abweisung`] gets the event before the end of the connection.
    #[must_use]
    pub fn ablehnen(self, grund: Aufrufergrund) -> Ereignis {
        match self.innen {
            VorlaeufigInnen::Pipe(k) => k.ablehnen(grund.0),
            #[cfg(unix)]
            VorlaeufigInnen::Unix(u) => u.ablehnen(grund.0),
        }
    }
}

/// A connection after both checks: a byte stream in both directions. Errors of reading and writing carry a [`Fehler`] with
/// `pk.io` and `os_fehler`.
pub struct Verbindung {
    innen: VerbindungInnen,
}

enum VerbindungInnen {
    #[cfg_attr(unix, allow(dead_code))]
    Pipe(VerbindungKern<Os>),
    #[cfg(unix)]
    Unix(unix::Verbindung),
}

impl Verbindung {
    /// The peer: at the server the measured client, at the client the owner of the pipe instance.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        match &self.innen {
            VerbindungInnen::Pipe(k) => &k.peer,
            #[cfg(unix)]
            VerbindungInnen::Unix(u) => &u.peer,
        }
    }

    /// The bound process of the client (server side only). The library holds it as long as the connection lives.
    #[must_use]
    pub fn prozess(&self) -> Option<&Prozessbindung> {
        match &self.innen {
            VerbindungInnen::Pipe(k) => k.prozess(),
            #[cfg(unix)]
            VerbindungInnen::Unix(u) => u.prozess(),
        }
    }

    /// Decision 2: the time limit of every following read. When it passes without a byte, the read returns an error of kind
    /// [`io::ErrorKind::TimedOut`] (with `pk.io` inside); the connection stays and a later read may go on. `None` reads without a
    /// limit (the start value). The same on every system.
    pub fn lesefrist(&mut self, frist: Option<Duration>) {
        match &mut self.innen {
            VerbindungInnen::Pipe(k) => k.lesefrist(frist),
            #[cfg(unix)]
            VerbindungInnen::Unix(u) => u.lesefrist(frist),
        }
    }

    /// Windows, server side: the held handle of the process of the client, opened with `PROCESS_QUERY_LIMITED_INFORMATION`.
    #[cfg(windows)]
    #[must_use]
    pub fn prozess_griff(&self) -> Option<std::os::windows::io::BorrowedHandle<'_>> {
        use std::os::windows::io::AsHandle;
        match &self.innen {
            VerbindungInnen::Pipe(k) => k.prozess_roh().map(AsHandle::as_handle),
        }
    }
}

impl Read for Verbindung {
    fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        match &mut self.innen {
            VerbindungInnen::Pipe(k) => k.lesen(puffer),
            #[cfg(unix)]
            VerbindungInnen::Unix(u) => u.lesen(puffer),
        }
    }
}

impl Write for Verbindung {
    fn write(&mut self, daten: &[u8]) -> io::Result<usize> {
        match &mut self.innen {
            VerbindungInnen::Pipe(k) => k.schreiben(daten),
            #[cfg(unix)]
            VerbindungInnen::Unix(u) => u.schreiben(daten),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// "Client verbinden": reaches an instance, checks its owner against the expected SID before any byte, sends the hello and
/// waits for its answer, all within `frist`.
///
/// # Errors
/// In this order: `pk.vorgabe_ungueltig`, `pk.nicht_erreichbar`, `pk.zugriff_verweigert`, `pk.server_unmessbar`,
/// `pk.falscher_server` (no byte sent), `pk.abgewiesen`.
pub fn verbinden(v: &ClientVorgabe) -> Result<Verbindung, Fehler> {
    v.pruefen(System::aktuell())?;
    #[cfg(unix)]
    {
        unix::verbinden(v, true).map(|u| Verbindung {
            innen: VerbindungInnen::Unix(u),
        })
    }
    #[cfg(not(unix))]
    {
        if !cfg!(windows) {
            return Err(Fehler::neu(Code::NichtErreichbar));
        }
        ablauf::verbinden(Arc::new(Os), v).map(|kern| Verbindung {
            innen: VerbindungInnen::Pipe(kern),
        })
    }
}

/// "Client verbinden" without the hello (decision 3), for a server that speaks no `pk1`: reaches an instance and checks its
/// owner against the expected SID exactly as [`verbinden`] does (same check, same codes, no byte before the check), then hands
/// the connection over without sending `pk1`. The expected owner is mandatory, as in every [`ClientVorgabe`]; for a service
/// it is [`Sid::dienst`]. `frist` covers the wait for a free instance.
///
/// # Errors
/// In this order: `pk.vorgabe_ungueltig`, `pk.nicht_erreichbar`, `pk.zugriff_verweigert`, `pk.server_unmessbar`,
/// `pk.falscher_server` (no byte sent).
pub fn verbinden_ohne_hallo(v: &ClientVorgabe) -> Result<Verbindung, Fehler> {
    v.pruefen(System::aktuell())?;
    #[cfg(unix)]
    {
        unix::verbinden(v, false).map(|u| Verbindung {
            innen: VerbindungInnen::Unix(u),
        })
    }
    #[cfg(not(unix))]
    {
        if !cfg!(windows) {
            return Err(Fehler::neu(Code::NichtErreichbar));
        }
        ablauf::verbinden_ohne_hallo(Arc::new(Os), v).map(|kern| Verbindung {
            innen: VerbindungInnen::Pipe(kern),
        })
    }
}

/// The `TokenUser` SID of the own process token (decision 1). Pure information: it permits nothing; a caller may name it as
/// the expected owner of a server of the same account.
///
/// # Errors
/// Windows: `pk.io` with `os_fehler` when the token cannot be read, without it when the SID is not of the form [`Sid`] (such as
/// `LOCAL SERVICE`). Linux and macOS: `pk.nicht_unterstuetzt`.
pub fn eigene_sid() -> Result<Sid, Fehler> {
    if !cfg!(windows) {
        return Err(Fehler::neu(Code::NichtUnterstuetzt));
    }
    ablauf::eigene_sid(&Os)
}

/// Test seams of the outside runner. Never in a shipped build.
#[cfg(feature = "testnaht")]
pub mod testnaht {
    use super::{Fehler, PackageSid, Server, ServerVorgabe};

    /// Like [`Server::anlegen`], with two seams: `zusatz` gives package SIDs an ACE in the DACL without an entry in the list
    /// (case `n-container-ohne-eintrag-windows`); `nach_verbinden` runs right after `ConnectNamedPipe` returned, before the
    /// process is bound (case `n-pid-wiederverwendung-windows`).
    ///
    /// # Errors
    /// As [`Server::anlegen`].
    pub fn anlegen(
        v: ServerVorgabe,
        zusatz: &[PackageSid],
        nach_verbinden: Option<Box<dyn Fn() + Send + Sync>>,
    ) -> Result<Server, Fehler> {
        let zusatz: Vec<&str> = zusatz.iter().map(PackageSid::als_str).collect();
        Server::anlegen_mit(v, &zusatz, nach_verbinden)
    }
}

// The interface on a system without the pipe: the answers of `Server::anlegen` and the wrappers over the order. The values
// come from constructors of the module `ablauf` without any system call.
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn kanal() -> Kennung {
        Kennung::neu("k-beispiel").expect("form")
    }

    fn bindung() -> Prozessbindung {
        Prozessbindung {
            pid: 4242,
            startzeit: Some(50),
        }
    }

    /// Valid settings of this system and a folder that does not exist: `pk.ordner_rechte` without `os_fehler`, before the
    /// socket (the Unix side, step 2 of "Server anlegen").
    #[test]
    fn anlegen_ohne_ordner_ohne_os_fehler() {
        let mut v = ServerVorgabe::neu(kanal(), vec![Erlaubnis::Unix { uid: 1, gid: None }], 1);
        v.unix = Some(UnixOrt {
            ordner: "/srv/beispiel".into(),
            modus: SocketModus::M0600,
        });
        assert_eq!(
            Server::anlegen(v).err(),
            Some(Fehler::neu(Code::OrdnerRechte))
        );
    }

    #[test]
    fn haken_wird_gesetzt() {
        let mut s = Server {
            innen: ServerInnen::Pipe(ServerKern::ohne_instanz(Arc::new(Os), kanal())),
        };
        let haken = |s: &Server| match &s.innen {
            ServerInnen::Pipe(k) => k.haken.clone(),
            ServerInnen::Unix(u) => u.haken.clone(),
        };
        assert!(haken(&s).is_none());
        let gerufen = Arc::new(AtomicUsize::new(0));
        let g = Arc::clone(&gerufen);
        s.bei_abweisung(move |_| {
            g.fetch_add(1, Ordering::SeqCst);
        });
        let e = Ereignis {
            kanal: kanal(),
            grund: Grund::HalloFehlt,
            peer: None,
        };
        (haken(&s).expect("gesetzt"))(&e);
        assert_eq!(gerufen.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn vorlaeufig_nennt_peer_und_prozess() {
        let s = ServerKern::ohne_instanz(Arc::new(Os), kanal());
        let peer = Peer::leer(System::Windows);
        let v = Vorlaeufig {
            innen: VorlaeufigInnen::Pipe(s.vorlaeufig_ohne_griff(Some(bindung()), peer.clone())),
        };
        assert_eq!(v.prozess(), Some(&bindung()));
        assert_eq!(v.peer(), &peer);
    }

    /// The wrappers of `Verbindung` pass the binding, the time limit and the errors of the system through.
    #[test]
    fn verbindung_reicht_durch() {
        let mut v = Verbindung {
            innen: VerbindungInnen::Pipe(VerbindungKern::mit_griff(
                Arc::new(Os),
                (),
                Some(bindung()),
                Peer::leer(System::Windows),
            )),
        };
        let frist = |v: &Verbindung| match &v.innen {
            VerbindungInnen::Pipe(k) => k.frist_der_lesen(),
            VerbindungInnen::Unix(_) => None,
        };
        assert_eq!(v.prozess(), Some(&bindung()));
        assert_eq!(frist(&v), None);
        v.lesefrist(Some(Duration::from_millis(5)));
        assert_eq!(frist(&v), Some(Duration::from_millis(5)));
        let os_fehler = |e: &io::Error| {
            e.get_ref()
                .and_then(|f| f.downcast_ref::<Fehler>())
                .copied()
        };
        let mut puffer = [0u8; 4];
        let e = v.read(&mut puffer).expect_err("ohne System");
        assert_eq!(os_fehler(&e), Some(Fehler::os(Code::Io, 0)));
        let e = v.write(b"x").expect_err("ohne System");
        assert_eq!(os_fehler(&e), Some(Fehler::os(Code::Io, 0)));
        assert!(v.flush().is_ok());
    }
}
