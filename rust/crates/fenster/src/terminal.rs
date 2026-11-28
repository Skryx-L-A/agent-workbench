//! The terminal picture (B35): a raster of `rasterauszug` painted with the monospace font, a mark with the mouse
//! (B38) and the accessibility nodes of SE3 point 1 (one text box, its lines as text).

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, FontId, Id, Pos2, Rect, Stroke, TextFormat, Ui, pos2, vec2};
use rasterauszug::{
    Bild, DIMM, DURCHGESTRICHEN, FETT, INVERS, KURSIV, Lauf, UNTERSTRICHEN, VERDECKT,
};

use crate::gestalt::{Terminalfarben, stilfarbe};
use crate::kasten;

/// A mark in the terminal: from and to as (row, column), both included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Markierung {
    /// Where the drag began.
    pub von: (usize, usize),
    /// Where it is now.
    pub bis: (usize, usize),
}

impl Markierung {
    /// Start and end in reading order.
    #[must_use]
    pub fn geordnet(&self) -> ((usize, usize), (usize, usize)) {
        if self.von <= self.bis {
            (self.von, self.bis)
        } else {
            (self.bis, self.von)
        }
    }

    /// The columns of row `z` inside the mark.
    #[must_use]
    pub fn spalten(&self, z: usize, breite: usize) -> Option<(usize, usize)> {
        let (a, b) = self.geordnet();
        if z < a.0 || z > b.0 {
            return None;
        }
        let von = if z == a.0 { a.1 } else { 0 };
        let bis = if z == b.0 {
            b.1
        } else {
            breite.saturating_sub(1)
        };
        Some((von, bis))
    }
}

/// The text of line `l` (the same text as the browser view, SE4: the texts of its runs).
#[must_use]
pub fn zeilentext(l: &[Lauf]) -> String {
    ansicht::zeile::text(l)
}

/// The columns a character takes in the raster: the width of `unicode-width` 0.2.3, the table `alacritty_terminal`
/// gives the grid of `traeger` (decision of 19.11.2026), so that a character stands in the window in the cell it has
/// in the raster. The five skin tone modifiers take no cell, as `traeger` keeps them behind their base character.
#[must_use]
pub fn breite(c: char) -> usize {
    if ('\u{1f3fb}'..='\u{1f3ff}').contains(&c) {
        return 0;
    }
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(0)
}

/// One character of a line with the characters of no width that follow it: the run it is in, its first column, its
/// columns and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zeichen {
    /// The index of the run.
    pub lauf: usize,
    /// The first column.
    pub spalte: usize,
    /// The columns it takes (0 only for characters of no width at the start of a run).
    pub breite: usize,
    /// The base character and the characters of no width behind it.
    pub text: String,
}

/// The characters of line `l` in their columns.
#[must_use]
pub fn zeichen(l: &[Lauf]) -> Vec<Zeichen> {
    let mut aus: Vec<Zeichen> = Vec::new();
    let mut spalte = 0;
    for (i, lauf) in l.iter().enumerate() {
        for c in lauf.text.chars() {
            let b = breite(c);
            match aus.last_mut() {
                Some(z) if b == 0 && z.lauf == i => z.text.push(c),
                _ => {
                    aus.push(Zeichen {
                        lauf: i,
                        spalte,
                        breite: b,
                        text: c.to_string(),
                    });
                    spalte += b;
                }
            }
        }
    }
    aus
}

/// The marked text: rows joined with a line break, the end of each row without trailing spaces. A wide character
/// belongs to the mark when one of its columns does.
#[must_use]
pub fn markierter_text(bild: &Bild, m: &Markierung) -> String {
    let mut aus = Vec::new();
    for (z, l) in bild.lines.iter().enumerate() {
        if let Some((von, bis)) = m.spalten(z, bild.spalten) {
            let t: String = zeichen(l)
                .into_iter()
                .filter(|c| c.spalte <= bis && c.spalte + c.breite.max(1) > von)
                .map(|c| c.text)
                .collect();
            aus.push(t.trim_end().to_owned());
        }
    }
    aus.join("\n")
}

/// The size of one character cell of the monospace font.
#[must_use]
pub fn zelle(ui: &Ui, schrift: &FontId) -> (f32, f32) {
    ui.fonts_mut(|f| (f.glyph_width(schrift, 'M'), f.row_height(schrift)))
}

/// Paints `bild` into `rect` and builds one text node per line under the accessibility parent `knoten`. The ground has
/// the corners `ecken` (the bottom corners of a tile); the text stays 6 points inside, clear of them.
#[allow(clippy::too_many_arguments)]
pub fn malen(
    ui: &mut Ui,
    rect: Rect,
    ecken: CornerRadius,
    bild: Option<&Bild>,
    farben: &Terminalfarben,
    schrift: &FontId,
    markierung: Option<&Markierung>,
    knoten: Id,
    zeilen_knoten: bool,
) {
    let (zw, zh) = zelle(ui, schrift);
    let maler = ui.painter_at(rect);
    maler.rect_filled(rect, ecken, farben.grund);
    let Some(bild) = bild else {
        return;
    };
    let innen = rect.shrink(6.0);
    let kind = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(innen)
            .id_salt(knoten.with("zeilen"))
            .accessibility_parent(knoten),
    );
    for (z, l) in bild.lines.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = innen.top() + z as f32 * zh;
        if y > innen.bottom() {
            break;
        }
        // each character stands in the column the raster gives it (`zeichen`): a run of characters with the width of a
        // cell goes as one piece, a character of another width (wide, a fallback font, with marks of no width) in its
        // own cells, a box drawing character as lines of its cell
        let formate: Vec<TextFormat> = l.iter().map(|x| format(x, farben, schrift)).collect();
        let mut stueck = String::new();
        let mut anfang = 0;
        let mut stil = 0;
        for c in zeichen(l) {
            let f = &formate[c.lauf];
            if c.lauf != stil {
                stueck_malen(
                    &maler,
                    &mut stueck,
                    anfang,
                    innen.left(),
                    y,
                    zw,
                    &formate[stil],
                );
                stil = c.lauf;
            }
            let eins = c.text.chars().count() == 1;
            let erstes = c.text.chars().next().unwrap_or(' ');
            #[allow(clippy::cast_precision_loss)]
            let zelle = Rect::from_min_size(
                pos2(innen.left() + c.spalte as f32 * zw, y),
                vec2(c.breite.max(1) as f32 * zw, zh),
            );
            if eins && kasten::ist_kasten(erstes) {
                stueck_malen(&maler, &mut stueck, anfang, innen.left(), y, zw, f);
                if f.background != Color32::TRANSPARENT {
                    maler.rect_filled(zelle, 0.0, f.background);
                }
                kasten::malen(&maler, erstes, zelle, f.color);
                continue;
            }
            let breit = ui.fonts_mut(|x| x.glyph_width(schrift, erstes));
            if eins && c.breite == 1 && (breit - zw).abs() < 0.01 {
                if stueck.is_empty() {
                    anfang = c.spalte;
                }
                stueck.push(erstes);
            } else {
                stueck_malen(&maler, &mut stueck, anfang, innen.left(), y, zw, f);
                einzeln_malen(&maler, &c.text, zelle, f);
            }
        }
        if let Some(f) = formate.get(stil) {
            stueck_malen(&maler, &mut stueck, anfang, innen.left(), y, zw, f);
        }
        if let Some((von, bis)) = markierung.and_then(|m| m.spalten(z, bild.spalten)) {
            #[allow(clippy::cast_precision_loss)]
            let r = Rect::from_min_size(
                pos2(innen.left() + von as f32 * zw, y),
                vec2((bis + 1 - von) as f32 * zw, zh),
            );
            maler.rect_filled(r, 0.0, farben.markierung);
        }
        if zeilen_knoten {
            let id = knoten.with(("zeile", z));
            let zr = Rect::from_min_size(pos2(innen.left(), y), vec2(innen.width(), zh));
            let _ = kind.interact(zr, id, egui::Sense::hover());
            let text = zeilentext(l);
            ui.ctx().accesskit_node_builder(id, |n| {
                n.set_role(egui::accesskit::Role::Label);
                n.set_value(text.as_str());
                n.set_author_id(format!("terminal-zeile-{z}"));
            });
        }
    }
    let c = bild.cursor;
    if c.sichtbar {
        #[allow(clippy::cast_precision_loss)]
        let r = Rect::from_min_size(
            pos2(
                innen.left() + c.spalte as f32 * zw,
                innen.top() + c.zeile as f32 * zh,
            ),
            vec2(zw, zh),
        );
        maler.rect_filled(r, 0.0, farben.schrift.gamma_multiply(0.7));
    }
}

/// Paints the characters of `stueck` from column `spalte` on and empties it.
fn stueck_malen(
    maler: &egui::Painter,
    stueck: &mut String,
    spalte: usize,
    links: f32,
    y: f32,
    zw: f32,
    f: &TextFormat,
) {
    if stueck.is_empty() {
        return;
    }
    let mut job = LayoutJob::default();
    job.append(stueck, 0.0, f.clone());
    let galley = maler.layout_job(job);
    #[allow(clippy::cast_precision_loss)]
    maler.galley(pos2(links + spalte as f32 * zw, y), galley, f.color);
    stueck.clear();
}

/// Paints one character (with its marks of no width) in its cells `zelle`: in the middle when it is narrower, from the
/// left when it is wider.
fn einzeln_malen(maler: &egui::Painter, text: &str, zelle: Rect, f: &TextFormat) {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, f.clone());
    let galley = maler.layout_job(job);
    let rest = (zelle.width() - galley.size().x).max(0.0);
    maler.galley(
        pos2(zelle.left() + rest / 2.0, zelle.top()),
        galley,
        f.color,
    );
}

fn format(l: &Lauf, t: &Terminalfarben, schrift: &FontId) -> TextFormat {
    let fl = l.stil.fl;
    let mut vorn = stilfarbe(t, l.stil.fg, t.schrift);
    let mut hinten = stilfarbe(t, l.stil.bg, Color32::TRANSPARENT);
    if fl & FETT != 0 && (1..=8).contains(&l.stil.fg) {
        vorn = stilfarbe(t, l.stil.fg + 8, t.schrift);
    }
    if fl & INVERS != 0 {
        let h = if hinten == Color32::TRANSPARENT {
            t.grund
        } else {
            hinten
        };
        (vorn, hinten) = (h, vorn);
    }
    if fl & DIMM != 0 {
        vorn = vorn.gamma_multiply(0.6);
    }
    if fl & VERDECKT != 0 {
        vorn = Color32::TRANSPARENT;
    }
    TextFormat {
        font_id: schrift.clone(),
        color: vorn,
        background: hinten,
        italics: fl & KURSIV != 0,
        underline: if fl & UNTERSTRICHEN != 0 {
            Stroke::new(1.0, vorn)
        } else {
            Stroke::NONE
        },
        strikethrough: if fl & DURCHGESTRICHEN != 0 {
            Stroke::new(1.0, vorn)
        } else {
            Stroke::NONE
        },
        ..Default::default()
    }
}

/// The cell under a point of the raster area `rect`.
#[must_use]
pub fn zelle_bei(p: Pos2, rect: Rect, zw: f32, zh: f32) -> (usize, usize) {
    let innen = rect.shrink(6.0);
    // the values are clamped at 0 before the cast
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let z = ((p.y - innen.top()) / zh).max(0.0) as usize;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let s = ((p.x - innen.left()) / zw).max(0.0) as usize;
    (z, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marked_text_spans_rows_and_drops_trailing_spaces() {
        let zeile = |t: &str| {
            vec![Lauf {
                stil: rasterauszug::Stil::VORGABE,
                text: t.to_owned(),
            }]
        };
        let mut bild = Bild::leer(10, 3);
        bild.lines = vec![zeile("abc def"), zeile("ghi   "), zeile("jkl")];
        let m = Markierung {
            von: (2, 1),
            bis: (0, 4),
        };
        assert_eq!(markierter_text(&bild, &m), "def\nghi\njk");
    }

    fn eine(t: &str) -> Vec<Lauf> {
        vec![Lauf {
            stil: rasterauszug::Stil::VORGABE,
            text: t.to_owned(),
        }]
    }

    #[test]
    fn the_columns_follow_the_width_of_the_raster() {
        let z = zeichen(&eine("a中e\u{301}😀👍\u{1f3fd}€…„│"));
        let spalten: Vec<(usize, usize, &str)> = z
            .iter()
            .map(|c| (c.spalte, c.breite, c.text.as_str()))
            .collect();
        assert_eq!(
            spalten,
            [
                (0, 1, "a"),
                (1, 2, "中"),
                (3, 1, "e\u{301}"),
                (4, 2, "😀"),
                (6, 2, "👍\u{1f3fd}"),
                (8, 1, "€"),
                (9, 1, "…"),
                (10, 1, "„"),
                (11, 1, "│"),
            ]
        );
        // the frame of the mock-up: the middle line ends in the column of the corners
        let rahmen: Vec<usize> = crate::attrappe::orchestrator_text()
            .iter()
            .filter(|t| t.starts_with(['┌', '│', '└']))
            .map(|t| zeichen(&eine(t)).iter().map(|c| c.breite).sum())
            .collect();
        assert_eq!(rahmen, [48, 48, 48]);
    }

    #[test]
    fn a_mark_takes_a_wide_character_with_any_of_its_columns() {
        let mut bild = Bild::leer(10, 1);
        bild.lines = vec![eine("a中b")];
        let m = Markierung {
            von: (0, 2),
            bis: (0, 3),
        };
        assert_eq!(markierter_text(&bild, &m), "中b");
    }
}
