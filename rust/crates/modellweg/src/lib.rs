//! Model path (contract 8, `rust/spec/VERTRAEGE.md`): a forwarder outside the enclosure. It takes requests of the
//! harness on a Unix socket, lets exactly one path and one model through, inserts the key from a file and hands the answer
//! of the target back byte for byte, with a lock against a mirrored key. Every request leaves one line in the chain of
//! contract 5 (source `gateway`).
//!
//! Part A of the contract (every mode) lives in [`aufruf`], [`schluessel`], [`ziel`], [`tls`], [`dienst`] (with the leak lock
//! and the answer), [`protokoll`] and [`start`]. The rules of a mode (part B, `openai-lokal`) live in their own module
//! below `betriebsart`; [`Betriebsart`] chooses one.

// Unix only: the product runs on Linux; the code uses Unix sockets, permissions and signals. On another system the crate
// is empty and the program says that it is not available.
#![cfg(unix)]

pub mod aufruf;
pub mod betrieb;
pub mod betriebsart;
#[cfg(target_os = "linux")]
pub mod ca;
pub mod codes;
#[cfg(target_os = "linux")]
mod der;
pub mod dienst;
mod http;
mod json;
mod loeschen;
mod os;
pub mod probelauf;
pub mod protokoll;
pub mod schluessel;
mod sperre;
pub mod start;
pub mod suche;
pub mod tls;
mod tokens;
#[cfg(target_os = "linux")]
pub mod tunnel;
#[cfg(target_os = "linux")]
mod websocket;
pub mod ziel;

pub use betriebsart::Betriebsart;
