//! The holder `werkbank-halter` (contract `rust/spec/halter/README.md`, version 1, Linux step I).
//!
//! The holder keeps keys and tokens outside every agent session. It never gives a value out; it only uses one for a
//! model call (`modell.aufruf`, `modell.strom`), a search (`modell.suche`) or an MCP call. The library holds the pure
//! order of checks ([`pruefung`]), the answer lines ([`antwort`]), the leak lock ([`leck`]), the frame reader of the
//! stream ([`rahmen`]), the way to a target ([`ziel`], [`mcp`]) and, under Unix, the store ([`ablage`]) and the
//! search ([`suche`]); under Linux the peer check ([`peer`]), the start ([`start`]) and the server ([`dienst`]).

#[cfg(unix)]
pub mod ablage;
pub mod antwort;
pub mod b64;
pub mod code;
#[cfg(target_os = "linux")]
pub mod dienst;
pub mod ed25519;
pub mod form;
pub mod geheim;
pub mod json;
pub mod kette;
pub mod konfig;
pub mod leck;
pub mod mcp;
#[cfg(target_os = "linux")]
pub mod peer;
pub mod pruefung;
pub mod rahmen;
pub mod sha512;
#[cfg(target_os = "linux")]
pub mod start;
#[cfg(unix)]
pub mod suche;
pub mod ziel;
