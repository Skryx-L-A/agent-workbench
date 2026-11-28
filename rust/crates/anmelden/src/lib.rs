//! `werkbank-anmelden` (contract 9, `rust/spec/anmelden/README.md`): the login of the Copilot CLI so that the
//! token lands only in the store of the holder (`anmeldung.copilot`) and never in the home of the user.
//!
//! The logic lives here, the process shell (arguments, signals, output) in `main.rs`. Unix only: the product runs
//! on Linux; the code uses Unix permissions and signals.
#![cfg(unix)]

pub mod fehler;
pub mod form;
pub mod gruppe;
pub mod lauf;
pub mod schutz;
pub mod uebernehmen;
