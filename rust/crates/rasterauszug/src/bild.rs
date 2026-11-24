//! The picture and the extract: what a viewer holds and what one frame says (contract sections 3 and 4).

use crate::zeile::Zeile;

/// The most columns or rows of a picture (RA5, RA14).
pub const GROESSE_MAX: usize = 1000;

/// The fewest columns of a picture (RA5, RA10): the whole session takes no size with one column, because the carrier
/// (`alacritty_terminal` 0.26.0, alacritty/alacritty issue 9112) cannot hold one.
pub const SPALTEN_MIN: usize = 2;

/// The fewest rows of a picture (RA5, RA10).
pub const ZEILEN_MIN: usize = 1;

/// The cursor of a picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    /// Row, smaller than the number of rows.
    pub zeile: usize,
    /// Column, smaller than the number of columns.
    pub spalte: usize,
    /// Whether the cursor is shown.
    pub sichtbar: bool,
}

/// A scroll up: the rows `oben` to `unten - 1` move up by `n` rows, the last `n` of them become empty (RA7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schub {
    /// First row of the region.
    pub oben: usize,
    /// End of the region (excluded).
    pub unten: usize,
    /// How many rows the text moves up.
    pub n: usize,
}

/// A picture: the model picture of the sender and the picture of the receiver (`Abbild`) have this form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bild {
    /// Columns.
    pub spalten: usize,
    /// Rows; `lines.len()` equals it.
    pub zeilen: usize,
    /// The lines in canonical form, exactly `zeilen` of them.
    pub lines: Vec<Zeile>,
    /// Whether the picture is inverse.
    pub invers: bool,
    /// The cursor.
    pub cursor: Cursor,
    /// The stand of the last message or frame taken in.
    pub stand: u64,
}

/// The picture of the receiver.
pub type Abbild = Bild;

impl Bild {
    /// An empty picture: every line empty, not inverse, the cursor at (0, 0) hidden, stand 0.
    #[must_use]
    pub fn leer(spalten: usize, zeilen: usize) -> Bild {
        Bild {
            spalten,
            zeilen,
            lines: vec![Vec::new(); zeilen],
            invers: false,
            cursor: Cursor::default(),
            stand: 0,
        }
    }

    /// Whether two pictures show the same: size, lines, `invers` and cursor; the stand is not compared.
    #[must_use]
    pub fn gleicht(&self, anderes: &Bild) -> bool {
        self.spalten == anderes.spalten
            && self.zeilen == anderes.zeilen
            && self.invers == anderes.invers
            && self.cursor == anderes.cursor
            && self.lines == anderes.lines
    }
}

/// What one frame says, in readable form (the `Auszug` of the contract).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Auszug {
    /// Session number.
    pub nr: u8,
    /// The stand the frame leads to.
    pub stand: u64,
    /// Columns and rows; set for a full picture.
    pub groesse: Option<(usize, usize)>,
    /// Whether the picture is inverse.
    pub invers: bool,
    /// The scroll, if any.
    pub schub: Option<Schub>,
    /// The cursor, if the frame carries one.
    pub cursor: Option<Cursor>,
    /// The changed lines with their index, rising.
    pub zeilen: Vec<(usize, Zeile)>,
}
