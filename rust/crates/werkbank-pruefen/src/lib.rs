//! The check command of the Ubuntu package: `werkbank-pruefen` (contract `rust/spec/paket`, "Prüfbefehl").
//!
//! Seven checks in a fixed order, fixed codes, fixed sentences, fixed exit codes. The command changes nothing on the
//! machine and prints no secret, no file content and no path outside the manifest. The logic has three parts:
//! [`sammeln`] reads the machine into a [`lage::Lage`], [`bewerte`] turns a `Lage` into lines and an exit code, and
//! [`manifest`] checks the manifest of the package.

pub mod bewerte;
pub mod lage;
pub mod manifest;
pub mod ohne_doppel;
pub mod programmordner;
// Reads the machine with Unix owners and modes; the rest of the crate is pure and builds everywhere.
#[cfg(unix)]
pub mod sammeln;
