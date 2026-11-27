//! `dienst`: the service of the new session view, and the program `wb-dienst` made from it.
//!
//! The service holds one session: a child in a PTY (through `traeger`), the state core (`zustandskern`, one `Sitzungskern`), one
//! extract per viewer (`rasterauszug`), a clock and one Unix socket. It only translates: events of the viewers into events of the
//! core, effects of the core into bytes for the PTY and frames on the socket. Every rule of the core stays in the core, every
//! rule of the extract in the extract, every rule of the screen in the carrier. The contract is `rust/spec/VERTRAEGE.md` of the contract
//! package (rules DI1 to DI11).
//!
//! `wb-dienst` is the core service of the trial run (the package ships it as `/usr/lib/werkbank/werkbank-kern`; it runs as a service of the user)
//! and, with `--kind`, the check program of the contract cases. The person stands in the start file, and the socket has folder and file rights
//! only (no check of the peer): the boundary is the folder with mode 0700.
//!
//! - [`Startdatei`] reads and checks the start file (DI1); exit code 2.
//! - [`rechte_pruefen`] checks the folder and the path of the socket (DI2, the security point); exit code 3.
//! - [`Dienst::neu`] starts the child (exit code 4), builds the core and binds the socket (exit code 1); [`signale_anmelden`] catches SIGTERM and SIGINT
//!   first and [`Dienst::laufen`] runs the loop until one of them comes.
//! - [`programm`] is the whole program: `wb-dienst <start.json>`, `wb-dienst --kind <name> ...` (a fixed child program) and
//!   `wb-dienst --exec ...` (the cover the service starts its child through, see `huelle.rs`).

// Unix and Windows (Nachtrag 25.11.2026, contract section 21): the product runs on Linux; under Windows the socket is `AF_UNIX`,
// the rights are access lists and the signals are the console events (`sys/windows.rs`, the only place with `unsafe`). On
// another system the crate is empty and the program says that it is not available.
#![cfg(any(unix, windows))]

#[cfg(unix)]
mod abschirmung;
mod arbeitsordner;
#[cfg(unix)]
mod arbeitsordner_fs;
mod aufraeumen;
mod bereit;
mod betrachter;
mod dienst;
mod fehler;
#[cfg(unix)]
mod huelle;
mod json;
#[cfg(feature = "pruefbau")]
mod kindprogramm;
#[cfg_attr(windows, path = "loeschen_windows.rs")]
mod loeschen;
mod markerdatei;
mod meldung;
mod rahmen;
#[cfg_attr(windows, path = "rechte_windows.rs")]
mod rechte;
mod sitzung;
mod startdatei;
mod starten;
mod sys;
mod waisen;

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

pub use dienst::{Anhalter, Dienst, Optionen, signale_anmelden};
pub use fehler::StartFehler;
#[cfg(feature = "pruefbau")]
pub use kindprogramm::lauf as kindprogramm;
#[cfg(unix)]
pub use rechte::socket_binden;
pub use rechte::{ORDNER_MODUS, SOCKET_MODUS, rechte_pruefen};
pub use startdatei::{
    KENNUNG_MAX, KLEINSTE_SPALTEN, Lauf, MAX_START, NUMMER_MAX, SOCKET_PFAD_MAX, Startdatei,
    WARTESCHLANGE_MAX, lesen, pruefen,
};
#[cfg(windows)]
pub use sys::Signale;

/// The whole program with the arguments after the program name; returns the exit code.
///
/// `--kind <name> [arguments]` runs a fixed child program (DI8; test build only, feature `pruefbau`); `--exec` is the cover of the child. One argument is the start file: the checks run in the order
/// 2 (file), 3 (rights), 4 (child), 1 (socket); each error is one line `wb-dienst: ...` on stderr, nothing on stdout, and
/// nothing stays behind. Otherwise the exit code is 2 with one line of usage. Before the signals and everything else the program shields
/// its memory from core images (`kernabbild`); when it cannot, the exit code is 5 with one line `wb-dienst: Kernabbild nicht abschirmbar: ...`.
/// Right after that it becomes the child subreaper (Linux; exit code 1 with one line `wb-dienst: Subreaper: ...` when the call is refused).
#[must_use]
pub fn programm(args: &[OsString]) -> i32 {
    #[cfg(unix)]
    if args.first().is_some_and(|a| a == "--exec") {
        let rest: Vec<String> = args[1..]
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        return huelle::ausfuehren(&rest);
    }
    // the fixed child programs of the tests exist in the test build only (feature `pruefbau`); the delivered build treats `--kind`
    // like any other argument
    // Windows, test build (section 21): a copy of the program named `laufkind.exe` is the fixed child `lauf` with all its
    // arguments; under Unix a small script does the same with `exec`, which Windows does not have
    #[cfg(all(windows, feature = "pruefbau"))]
    if let Some(code) = sys::laufkind() {
        return code;
    }
    #[cfg(feature = "pruefbau")]
    if args.first().is_some_and(|a| a == "--kind") {
        let rest: Vec<String> = args[1..]
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        return kindprogramm::lauf(&rest) & 0xff;
    }
    let [datei] = args else {
        return melden(&StartFehler::neu(2, "Aufruf: wb-dienst <startdatei.json>"));
    };
    // the first step: the memory of the process is shielded from core images before anything else happens (exit code 5 when it cannot be)
    // (Unix; under Windows `kernabbild` has no shield, a deviation of contract section 21)
    #[cfg(unix)]
    if let Some(code) = abschirmung::abschirmen(kernabbild::abschirmen(), &mut std::io::stderr()) {
        return code;
    }
    // Linux: the service becomes the child subreaper before its first child, so that every orphan below a session comes to it
    // (contract, section 17; `waisen.rs`). Elsewhere nothing happens.
    if let Err(e) = waisen::subreaper_werden() {
        let _ = writeln!(std::io::stderr(), "wb-dienst: Subreaper: {}", e.kind());
        return 1;
    }
    // the signals are caught before anything becomes visible to the outside (the socket, the child, the marker file): a SIGTERM
    // during the start waits and ends the loop cleanly with 0, it never ends the process by the default action
    let signale = match signale_anmelden() {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "wb-dienst: Signale: {}", e.kind());
            return 1;
        }
    };
    match starten(Path::new(datei)) {
        Ok(mut dienst) => dienst.laufen(signale),
        Err(f) => melden(&f),
    }
}

/// The three steps of the start (DI1, DI2, DI8): the start file, the rights, the child and the socket.
fn starten(pfad: &Path) -> Result<Dienst, StartFehler> {
    let start = lesen(pfad)?;
    rechte_pruefen(&start)?;
    Dienst::neu(start)
}

/// One line on stderr; returns the exit code of the error.
fn melden(f: &StartFehler) -> i32 {
    let _ = writeln!(std::io::stderr(), "{f}");
    f.code
}
