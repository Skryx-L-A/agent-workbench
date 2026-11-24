//! A local channel with a check of the other side (contract `rust/spec/VERTRAEGE.md`, version 1).
//!
//! A server offers a channel. For each connection the library measures the identity of the other side and lets only
//! identities from the list of the caller through; the caller then decides in a hook ([`Vorlaeufig`]) before the first byte
//! goes back. A client checks the server against an expected identity before it sends a byte.
//!
//! **Windows** (this build): a named pipe `\\.\pipe\<name>`. The server creates the first instance with
//! `FILE_FLAG_FIRST_PIPE_INSTANCE` and a protected DACL of the own SID, the allowed SIDs and package SIDs (build plan,
//! blocks B1 to B9). The client checks the owner of the pipe instance and opens with identification only.
//!
//! **Linux and macOS**: the Unix side (contract sections "Unix") is not part of this version. The settings are checked as
//! for every system; then [`Server::anlegen`] returns `pk.anlegen_fehlgeschlagen` and [`verbinden`] `pk.nicht_erreichbar`,
//! both without `os_fehler`.
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
mod vorgabe;

use std::io::{self, Read, Write};
use std::sync::Arc;

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
    kern: ServerKern<Os>,
}

impl Server {
    /// "Server anlegen": checks the settings, builds the security descriptor and creates the first instance with
    /// `FILE_FLAG_FIRST_PIPE_INSTANCE`. A taken name gives `pk.name_belegt`; there is no fallback.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig`, `pk.name_belegt` or `pk.anlegen_fehlgeschlagen` (Unix: also `pk.ordner_rechte` and
    /// `pk.pfad_zu_lang` once the Unix side exists).
    pub fn anlegen(v: ServerVorgabe) -> Result<Self, Fehler> {
        Self::anlegen_mit(v, &[], None)
    }

    fn anlegen_mit(
        mut v: ServerVorgabe,
        zusatz: &[&str],
        halt: Option<ablauf::Haltepunkt>,
    ) -> Result<Self, Fehler> {
        v.pruefen(System::aktuell())?;
        if !cfg!(windows) {
            return Err(Fehler::neu(Code::AnlegenFehlgeschlagen));
        }
        let job = v.job.take();
        let mut kern = ServerKern::anlegen(Arc::new(Os), v, job, zusatz)?;
        kern.nach_verbinden = halt;
        Ok(Self { kern })
    }

    /// "Annehmen und Messen": blocks until a client connects, then measures and checks it. Callable from up to `instanzen`
    /// threads at the same time.
    ///
    /// # Errors
    /// `pk.io` with `os_fehler` when the system fails while waiting for a client. A refused client is no error but
    /// [`Annahme::Abgewiesen`].
    pub fn annehmen(&self) -> Result<Annahme, Fehler> {
        Ok(match self.kern.annehmen()? {
            AnnahmeKern::Vorlaeufig(v) => Annahme::Vorlaeufig(Vorlaeufig { kern: v }),
            AnnahmeKern::Abgewiesen(e) => Annahme::Abgewiesen(e),
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
    kern: VorlaeufigKern<Os>,
}

impl Vorlaeufig {
    /// The measured peer.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        &self.kern.peer
    }

    /// The bound process: pid and start time. `None` when the pid could not be read.
    #[must_use]
    pub fn prozess(&self) -> Option<&Prozessbindung> {
        self.kern.prozess()
    }

    /// Sends the hello back and hands the connection over.
    ///
    /// # Errors
    /// `pk.io` when the hello cannot be written (for example, the client has gone).
    pub fn bestaetigen(self) -> Result<Verbindung, Fehler> {
        self.kern.bestaetigen().map(|kern| Verbindung { kern })
    }

    /// Closes without an answer and returns the event `pk.aufrufer_abgewiesen` with the reason `aufrufer:<kennung>`.
    #[must_use]
    pub fn ablehnen(self, grund: Aufrufergrund) -> Ereignis {
        self.kern.ablehnen(grund.0)
    }
}

/// A connection after both checks: a byte stream in both directions. Errors of reading and writing carry a [`Fehler`] with
/// `pk.io` and `os_fehler`.
pub struct Verbindung {
    kern: VerbindungKern<Os>,
}

impl Verbindung {
    /// The peer: at the server the measured client, at the client the owner of the pipe instance.
    #[must_use]
    pub fn peer(&self) -> &Peer {
        &self.kern.peer
    }

    /// The bound process of the client (server side only). The library holds it as long as the connection lives.
    #[must_use]
    pub fn prozess(&self) -> Option<&Prozessbindung> {
        self.kern.prozess()
    }

    /// Windows, server side: the held handle of the process of the client, opened with `PROCESS_QUERY_LIMITED_INFORMATION`.
    #[cfg(windows)]
    #[must_use]
    pub fn prozess_griff(&self) -> Option<std::os::windows::io::BorrowedHandle<'_>> {
        use std::os::windows::io::AsHandle;
        self.kern.prozess_roh().map(AsHandle::as_handle)
    }
}

impl Read for Verbindung {
    fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        self.kern.lesen(puffer)
    }
}

impl Write for Verbindung {
    fn write(&mut self, daten: &[u8]) -> io::Result<usize> {
        self.kern.schreiben(daten)
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
    if !cfg!(windows) {
        return Err(Fehler::neu(Code::NichtErreichbar));
    }
    ablauf::verbinden(Arc::new(Os), v).map(|kern| Verbindung { kern })
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
