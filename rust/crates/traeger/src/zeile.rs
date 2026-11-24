//! TR1: one row of the grid as a canonical SGR line (the form of contract `RA` R3).

use std::fmt::Write as _;

use alacritty_terminal::grid::Row;
use alacritty_terminal::index::Column;
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::Color;

/// Zero-width stand-ins for the skin tone modifiers U+1F3FB to U+1F3FF, see `Raster::umschreiben`.
pub(crate) const HAUTTON_ERSATZ: u32 = 0xE0100;
/// The first skin tone modifier.
pub(crate) const HAUTTON_ERSTER: u32 = 0x1F3FB;

/// One run of cells with the same style: foreground, background, attribute bits and the text.
struct Lauf {
    fg: u32,
    bg: u32,
    fl: u16,
    text: String,
}

impl Lauf {
    fn stil(&self) -> (u32, u32, u16) {
        (self.fg, self.bg, self.fl)
    }
}

/// The colour as a number: named colours 0 to 15 are 1 to 16, index `i` is `i + 1`, true colour is
/// `0x0100_0000 | rgb`, the default colours (and every other named colour) are 0.
fn farbe(farbe: Color) -> u32 {
    match farbe {
        Color::Named(name) => match u32::try_from(name as usize) {
            Ok(nummer @ 0..=15) => 1 + nummer,
            _ => 0,
        },
        Color::Indexed(index) => 1 + u32::from(index),
        Color::Spec(rgb) => {
            0x0100_0000 + (u32::from(rgb.r) << 16) + (u32::from(rgb.g) << 8) + u32::from(rgb.b)
        }
    }
}

/// The attribute bits: bold 1, dim 2, italic 4, every underline 8, inverse 16, hidden 32, strikeout 64.
fn attribute(flags: Flags) -> u16 {
    let mut bits = 0;
    for (gesetzt, bit) in [
        (flags.contains(Flags::BOLD), 1),
        (flags.contains(Flags::DIM), 2),
        (flags.contains(Flags::ITALIC), 4),
        (flags.intersects(Flags::ALL_UNDERLINES), 8),
        (flags.contains(Flags::INVERSE), 16),
        (flags.contains(Flags::HIDDEN), 32),
        (flags.contains(Flags::STRIKEOUT), 64),
    ] {
        if gesetzt {
            bits |= bit;
        }
    }
    bits
}

/// A character as it may stand in the text: the stand-in of a skin tone modifier turns back into the modifier, and the
/// C1 controls (U+0080 to U+009F, invalid after `RA` R4) become U+FFFD.
fn sauber(zeichen: char) -> char {
    let nummer = zeichen as u32;
    if (HAUTTON_ERSATZ..HAUTTON_ERSATZ + 5).contains(&nummer) {
        return char::from_u32(HAUTTON_ERSTER + (nummer - HAUTTON_ERSATZ)).unwrap_or('\u{fffd}');
    }
    if (0x80..=0x9f).contains(&nummer) {
        '\u{fffd}'
    } else {
        zeichen
    }
}

/// The runs of a row: the second half of a wide character is left out, zero-width characters stay behind their base
/// character, and blanks in the default style at the end of the row are cut off.
fn laeufe(reihe: &Row<Cell>, spalten: usize) -> Vec<Lauf> {
    let mut laeufe: Vec<Lauf> = Vec::new();
    for spalte in 0..spalten {
        let zelle = &reihe[Column(spalte)];
        if zelle
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER.union(Flags::LEADING_WIDE_CHAR_SPACER))
        {
            continue;
        }
        let stil = (farbe(zelle.fg), farbe(zelle.bg), attribute(zelle.flags));
        if laeufe.last().is_none_or(|lauf| lauf.stil() != stil) {
            laeufe.push(Lauf {
                fg: stil.0,
                bg: stil.1,
                fl: stil.2,
                text: String::new(),
            });
        }
        if let Some(lauf) = laeufe.last_mut() {
            lauf.text.push(sauber(zelle.c));
            for zusatz in zelle.zerowidth().unwrap_or_default() {
                lauf.text.push(sauber(*zusatz));
            }
        }
    }
    while let Some(lauf) = laeufe.last_mut() {
        if lauf.stil() != (0, 0, 0) {
            break;
        }
        lauf.text.truncate(lauf.text.trim_end_matches(' ').len());
        if !lauf.text.is_empty() {
            break;
        }
        laeufe.pop();
    }
    laeufe
}

/// The SGR parameter of a colour number (the colour is not the default).
fn farbparameter(nummer: u32, hintergrund: bool) -> String {
    if nummer >= 0x0100_0000 {
        let (r, g, b) = ((nummer >> 16) & 255, (nummer >> 8) & 255, nummer & 255);
        return format!("{};2;{r};{g};{b}", if hintergrund { 48 } else { 38 });
    }
    let index = nummer - 1;
    if index < 8 {
        (if hintergrund { 40 } else { 30 } + index).to_string()
    } else if index < 16 {
        (if hintergrund { 100 } else { 90 } + index - 8).to_string()
    } else {
        format!("{};5;{index}", if hintergrund { 48 } else { 38 })
    }
}

/// The canonical SGR form of the runs: a style change is `ESC[0;…m` (or `ESC[0m` back to the default).
fn schreiben(laeufe: &[Lauf]) -> String {
    let mut aus = String::new();
    let mut vorher = (0, 0, 0);
    for lauf in laeufe {
        if lauf.stil() != vorher {
            if lauf.stil() == (0, 0, 0) {
                aus.push_str("\x1b[0m");
            } else {
                let mut teile = vec!["0".to_owned()];
                for (bit, code) in [(1, 1), (2, 2), (4, 3), (8, 4), (16, 7), (32, 8), (64, 9)] {
                    if lauf.fl & bit != 0 {
                        teile.push(code.to_string());
                    }
                }
                if lauf.fg != 0 {
                    teile.push(farbparameter(lauf.fg, false));
                }
                if lauf.bg != 0 {
                    teile.push(farbparameter(lauf.bg, true));
                }
                let _ = write!(aus, "\x1b[{}m", teile.join(";"));
            }
        }
        aus.push_str(&lauf.text);
        vorher = lauf.stil();
    }
    aus
}

/// One row in the canonical SGR form (TR1).
pub(crate) fn zeilenform(zeile: &Row<Cell>, spalten: usize) -> String {
    schreiben(&laeufe(zeile, spalten))
}

/// One row as plain text without style (what the step `Warte` looks for).
pub(crate) fn klartext(zeile: &Row<Cell>, spalten: usize) -> String {
    laeufe(zeile, spalten)
        .into_iter()
        .map(|lauf| lauf.text)
        .collect()
}
