//! The carrier of the new session view: one PTY with one child and one screen grid per session. It turns the bytes of the
//! child into the events of the state core and the effects of the core into bytes and sizes for the child.
//!
//! Two parts, both checked by the program `tr` (see `main.rs`):
//! - the stream part ([`Raster`]): bytes in, events and screen out; a pure function of the bytes and the size changes,
//!   with no child and no process;
//! - the child part ([`Kind`]): a real child in a PTY (`portable-pty`).
//!
//! The carrier knows no time, no viewer, no setting except the scrollback, no session name and no socket. The grid is
//! `alacritty_terminal` 0.26.0; the contract names what it adds (the scanner of control sequences, the zero-width skin
//! tones, the report of changed lines).

mod abschluss;
mod ereignis;
mod fall;
mod hex;
mod kind;
mod kinder;
#[cfg(unix)]
mod programm;
mod raster;
mod scan;
mod schlange;
mod vorspann;
mod zeile;

pub use ereignis::Ereignis;
pub use fall::{EINGABE_MAX, Ungueltig, laufen, laufen_mit_frist};
pub use kind::{Fehler, Kind, Start};
pub use kinder::lauf as kinderlauf;
pub use raster::{
    GroesseUngueltig, RUECKLAUF_VORGABE, Raster, SPALTEN_MAX, SPALTEN_MIN, ZEILEN_MAX, ZEILEN_MIN,
    groesse_gueltig,
};
pub use schlange::SCHLANGE_MAX;
