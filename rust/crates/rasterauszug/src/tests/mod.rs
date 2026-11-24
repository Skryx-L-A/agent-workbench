//! Unit tests of the library.

mod bild;
mod deflate;
mod fehler;
mod lauf;
mod rahmen;
mod schub;
mod verbindung;
mod zeile;

use crate::{Lauf, Stil, Zeile};

/// A line of runs `(fg, bg, fl, text)`.
pub(crate) fn zeile(laeufe: &[(u32, u32, u8, &str)]) -> Zeile {
    laeufe
        .iter()
        .map(|(fg, bg, fl, text)| Lauf {
            stil: Stil {
                fg: *fg,
                bg: *bg,
                fl: *fl,
            },
            text: (*text).to_owned(),
        })
        .collect()
}
