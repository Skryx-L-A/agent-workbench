//! Model path (contract 8, `rust/spec/modellweg/README.md`): a forwarder outside the enclosure. It takes requests of the
//! harness on a Unix socket, lets through only what a rule allows and hands the answer back byte for byte. Every
//! request leaves one line in the chain of contract 5 (source `gateway`).
//!
//! Where the key lies depends on the way. In mode 1 (`openai-lokal`) the forwarder reads it from a file of the person
//! and runs its own lock against a mirrored key ([`schluessel`], [`dienst`]). For a **bound** target of mode 2 it
//! holds no token at all: the request goes to the holder ([`halter`]), who inserts the header with the token, keeps
//! the way out and runs the leak lock himself (contract 8, section 5.1). An unbound target never gets a key.
//!
//! Part A of the contract (every mode) lives in [`aufruf`], [`schluessel`], [`ziel`], [`tls`], [`dienst`] (with the leak lock
//! and the answer), [`protokoll`] and [`start`]. The rules of a mode (part B, `openai-lokal`) live in their own module
//! below `betriebsart`; [`Betriebsart`] chooses one. Mode 2 (`kopf-einsetzen`) is `tunnel` with `ca` and
//! [`halter`], under Linux only (both modules exist there).

// Unix only: the product runs on Linux; the code uses Unix sockets, permissions and signals. On another system the crate
// is empty and the program says that it is not available.
#![cfg(unix)]

pub mod aufruf;
pub mod b64;
pub mod betrieb;
pub mod betriebsart;
#[cfg(target_os = "linux")]
pub mod ca;
pub mod codes;
#[cfg(target_os = "linux")]
mod der;
pub mod dienst;
pub mod halter;
mod http;
mod json;
mod loeschen;
mod os;
pub mod pfad;
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
