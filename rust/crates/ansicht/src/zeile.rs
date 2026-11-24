//! A run of the line form (contract `rasterauszug`, section 4) as CSS classes and inline colours.
//!
//! Foreground `f-N`, background `b-N` with N from 0 to 255 (palette colour c from 1 to 256 is N = c - 1; 0 to 15 are
//! the named colours), attributes `fett`, `dimm`, `kursiv`, `unterstrichen`, `verdeckt`, `durchgestrichen`. Inverse
//! swaps foreground and background; the default colours are then `f-grund` and `b-vorn`. Truecolor has no finite set
//! of classes and becomes an inline colour `#rrggbb`. The text is never HTML: the view puts it into a text node.

use rasterauszug::{
    DIMM, DURCHGESTRICHEN, FETT, INVERS, KURSIV, Lauf, Stil, UNTERSTRICHEN, VERDECKT,
};

const ATTRIBUTE: [(u8, &str); 6] = [
    (FETT, "fett"),
    (DIMM, "dimm"),
    (KURSIV, "kursiv"),
    (UNTERSTRICHEN, "unterstrichen"),
    (VERDECKT, "verdeckt"),
    (DURCHGESTRICHEN, "durchgestrichen"),
];
const TRUECOLOR: u32 = 0x0100_0000;

/// Classes and inline colours of one style.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Aussehen {
    /// The classes, separated by spaces.
    pub klassen: String,
    /// `color` as `#rrggbb`, for Truecolor.
    pub farbe: Option<String>,
    /// `background-color` as `#rrggbb`, for Truecolor.
    pub grund: Option<String>,
}

impl Aussehen {
    /// The inline style attribute, empty without Truecolor.
    #[must_use]
    pub fn stil(&self) -> String {
        let mut s = String::new();
        if let Some(f) = &self.farbe {
            s.push_str("color:");
            s.push_str(f);
            s.push(';');
        }
        if let Some(g) = &self.grund {
            s.push_str("background-color:");
            s.push_str(g);
            s.push(';');
        }
        s
    }
}

fn hex(c: u32) -> String {
    format!("#{:06x}", c & 0x00ff_ffff)
}

/// The look of a style.
#[must_use]
pub fn aussehen(stil: Stil) -> Aussehen {
    let mut klassen: Vec<String> = ATTRIBUTE
        .iter()
        .filter(|(bit, _)| stil.fl & bit != 0)
        .map(|(_, n)| (*n).to_owned())
        .collect();
    let (mut vorn, mut grund) = (stil.fg, stil.bg);
    if stil.fl & INVERS != 0 {
        std::mem::swap(&mut vorn, &mut grund);
        if vorn == 0 {
            klassen.push("f-grund".to_owned());
        }
        if grund == 0 {
            klassen.push("b-vorn".to_owned());
        }
    }
    let mut a = Aussehen::default();
    if vorn >= TRUECOLOR {
        a.farbe = Some(hex(vorn));
    } else if vorn > 0 {
        klassen.push(format!("f-{}", vorn - 1));
    }
    if grund >= TRUECOLOR {
        a.grund = Some(hex(grund));
    } else if grund > 0 {
        klassen.push(format!("b-{}", grund - 1));
    }
    a.klassen = klassen.join(" ");
    a
}

/// Whether a run is in the default style (a bare text node, no `span`).
#[must_use]
pub fn vorgabe(l: &Lauf) -> bool {
    l.stil == Stil::VORGABE
}

/// The text of a line: the texts of its runs (contract `seite` SE4 point 1).
#[must_use]
pub fn text(z: &[Lauf]) -> String {
    z.iter().map(|l| l.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(fg: u32, bg: u32, fl: u8) -> Stil {
        Stil { fg, bg, fl }
    }

    #[test]
    fn palette_attributes_inverse_and_truecolor() {
        assert_eq!(aussehen(s(2, 0, 0)).klassen, "f-1");
        assert_eq!(
            aussehen(s(0, 256, FETT | KURSIV)).klassen,
            "fett kursiv b-255"
        );
        assert_eq!(aussehen(s(0, 0, INVERS)).klassen, "f-grund b-vorn");
        assert_eq!(aussehen(s(5, 0, INVERS)).klassen, "f-grund b-4");
        let t = aussehen(s(0x0100_0000 | 0x00ab_cdef, 0x0100_0000, 0));
        assert_eq!(t.farbe.as_deref(), Some("#abcdef"));
        assert_eq!(t.grund.as_deref(), Some("#000000"));
        assert_eq!(t.stil(), "color:#abcdef;background-color:#000000;");
        assert_eq!(aussehen(Stil::VORGABE), Aussehen::default());
    }
}
