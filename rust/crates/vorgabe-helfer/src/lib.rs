//! The write helper of the company default, `werkbank-vorgabe` (contract `rust/spec/regelebenen/README.md`, section
//! 15.12; package `rust/spec/paket/README.md`, part A3).
//!
//! The helper runs under its own system user `werkbank-vorgabe`, never as root. systemd hands it one listening Unix
//! socket. For each connection it measures the peer ([`partner`], Linux), runs the stages H1 to H12 ([`helfer`]) and
//! answers with one JSON object. It can do one thing only: replace `/etc/werkbank/vorgabe/firmenvorgabe.json`
//! atomically as a whole ([`ersetzen`]), and only in `leseklassen` ([`form`]). Every file call goes through the narrow
//! layer [`sys`], so that the injected errors of the cases can be checked.

#[cfg(target_os = "linux")]
pub mod bedienen;
pub mod code;
#[cfg(unix)]
pub mod ersetzen;
pub mod form;
#[cfg(unix)]
pub mod helfer;
pub mod json;
#[cfg(unix)]
pub mod konten;
#[cfg(unix)]
pub mod lesen;
#[cfg(target_os = "linux")]
pub mod partner;
#[cfg(target_os = "linux")]
pub mod start;
#[cfg(unix)]
pub mod sys;
