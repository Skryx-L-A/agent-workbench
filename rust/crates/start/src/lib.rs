//! Start structure of a session (contract 1): reads the request of the view and the start file of the
//! service, and checks a start against the settings file.
//!
//! [`lesen_anfrage`] and [`lesen_datei`] check the form and the stages 1 to 13. [`pruefen`] adds the
//! stages 14 to 17 against the settings file. [`copilot_argv`] builds the start command of `copilot`, [`ist_marken_kennung`] tests the key of a folder mark. [`schreiben`] writes the start file that the program
//! `Lauf starten` reads. The crate reads bytes and never touches the file system.

mod fehler;
mod form;
mod lesen;
mod pruefen;
mod schreiben;
mod startbefehl;

pub use einstellungen::Routenart;
pub use einstellungen::Werkzeug;
pub use fehler::Fehler;
pub use form::{Marker, Startanfrage, Startdatei, Startform};
pub use lesen::{lesen_anfrage, lesen_datei};
pub use pruefen::pruefen;
pub use schreiben::schreiben;
pub use startbefehl::{MODELL_AUTO, copilot_argv, ist_marken_kennung};
