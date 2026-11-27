//! Rule levels, version 2 (contract `rust/spec/regelebenen/README.md`, section 15): settings at run time.
//!
//! A pure library. It opens no file, uses no network, reads no clock, draws no random number and starts no process.
//! The caller hands in bytes and the results of `fstat`; the library hands back values and fixed codes.
//!
//! - [`vorgabe`]: the company default. [`vorgabe::laden`] reads its bytes (stage H4), [`vorgabe::h3`] checks file, folder
//!   and parents from the data of `fstat` (stage H3), [`vorgabe::Firmenvorgabe::stand`] builds the [`Stand`] with the
//!   mapping of accounts to pseudonyms that the caller owns (15.2, DI-L11).
//! - [`setzen`] and [`sicht`] (15.4 to 15.6 and 15.11) over the catalogue [`katalog`] (15.1).
//! - [`json::streng_lesen`]: the strict reader that refuses a duplicate key.

pub mod einstellung;
pub mod json;
pub mod katalog;
pub mod vorgabe;

pub use einstellung::{
    Code, Einstellungen, Grund, Pruefung, Sichteintrag, Stand, Werte, Zeile, setzen, sicht,
};
