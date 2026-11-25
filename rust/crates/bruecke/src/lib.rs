//! The bridge `bruecke` and its program `wb-bruecke` (contract `bruecke`, walking skeleton).
//!
//! `wb-bruecke` is a program of the walking skeleton. It binds only `127.0.0.1` (fixed in the code, there is no switch), carries
//! no TLS, speaks HTTP/1.1 itself with `std` and hands bytes to the Unix socket of the service. It checks every request in
//! one fixed order (form, `Host`, `Origin`, token, method and route) before it passes anything on, and sends no CORS head.
//! No `unsafe`, and no input panics.

// Unix and Windows (Nachtrag 25.11.2026, contract section of the platforms): the product runs on Linux; under Windows the
// socket of the service is `AF_UNIX`, the rights are access lists and the signals are the console events (`sys/windows.rs`, the
// only place with `unsafe`). On another system the crate is empty and the program says that it is not available.
#![cfg(any(unix, windows))]
// Test code only: long case tables, `format!` into a text and `assert!(x.is_empty())` read better than their replacements.
#![cfg_attr(
    test,
    allow(
        clippy::too_many_lines,
        clippy::type_complexity,
        clippy::format_push_string,
        clippy::assert_is_empty,
        clippy::useless_format
    )
)]

mod abschirmung;
mod dienst;
mod fehler;
mod form;
mod programm;
mod pruefung;
#[cfg_attr(windows, path = "rechte_windows.rs")]
mod rechte;
mod seite;
mod server;
mod startdatei;
mod sys;
mod token;
mod verbindung;

#[cfg(all(test, unix))]
mod pruefstand;
#[cfg(all(test, unix))]
mod tests_bruecke;

pub use abschirmung::{KERNABBILD_CODE, abschirmen};
pub use fehler::{Abgelehnt, Antwort, StartFehler};
pub use programm::{lauf, signale_anmelden};
pub use server::{Bruecke, Halt, protokoll_stderr, starten};
pub use startdatei::{Config, aus_bytes, lesen};
pub use verbindung::Protokoll;
