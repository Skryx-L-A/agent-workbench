//! Places for what a later step adds. Nothing of it is in this crate.
//!
//! - **The program "Lauf starten".** It reads the start file, checks it, starts the search process, calls
//!   [`crate::Profil::bauen`] and [`crate::Profil::vorpruefen`] and replaces itself by [`crate::Profil::befehl`].
//!   The crate gives it the pieces and keeps no state.
//! - **The model bridge.** The field `modell_socket` of the description binds a socket of the host at
//!   [`crate::GAST_MODELL_SOCKET`]. The program that listens on `127.0.0.1` inside and passes the bytes to that socket
//!   is a later subcommand; the profile needs no change for it, because the network space has `lo`.
//! - **The forwarder for the local model.** Built: with the field `lokal_weiter` the harness runs below
//!   `einschluss-weiter` on `127.0.0.1:11434`, which passes the bytes to the model socket.
//! - **The key of the cloud model.** It stays in a file of mode 0600 outside the enclosure and is read by the
//!   process outside that sets the headers. It never enters the description, the profile or the environment of the
//!   harness (the five variables are the whole environment).
//! - **`seccomp`.** A filter handed to `bwrap` with `--seccomp <fd>` would be one more switch of the profile and one
//!   more entry of the list of required switches in `profil.rs`.
