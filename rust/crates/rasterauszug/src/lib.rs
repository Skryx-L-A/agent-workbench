//! The screen extract of the new interface: line form, difference frames per viewer, byte frame and Deflate.
//!
//! The library holds the logic only. It reads no file, starts no process, opens no socket and asks for neither
//! the time nor a random number. The program `ra` (`src/main.rs`) reads one case and writes the results; the
//! contract is `rust/spec/VERTRAEGE.md` of the contract package, rules RA1 to RA14.
//!
//! - [`zeile_lesen`] and [`zeile_schreiben`]: the line form, text with SGR sequences (RA1 to RA4).
//! - [`Verbindung`]: the sender of one viewer; takes the messages `Abzug` and `Rasteraenderung` of the state core
//!   and gives frames (RA5 to RA9, RA11 to RA13).
//! - [`Empfang`]: the decoder of the viewer; reads frames strictly (RA10, RA11).

mod bild;
mod deflate;
mod empfang;
mod fehler;
mod lauf;
mod rahmen;
mod schub;
mod verbindung;
mod zeile;

#[cfg(test)]
mod tests;

pub use bild::{Abbild, Auszug, Bild, Cursor, GROESSE_MAX, SPALTEN_MIN, Schub, ZEILEN_MIN};
pub use empfang::Empfang;
pub use fehler::Fehler;
pub use lauf::{EINGABE_MAX, Ungueltig, laufen};
pub use rahmen::{RAHMEN_MAX, rahmen_dekodieren, rahmen_kodieren};
pub use schub::unterschied;
pub use verbindung::Verbindung;
pub use zeile::{
    DIMM, DURCHGESTRICHEN, FETT, INVERS, KURSIV, Lauf, Stil, UNTERSTRICHEN, VERDECKT, Zeile,
    kanonisch, zeichen_ok, zeile_lesen, zeile_pruefen, zeile_schreiben,
};
