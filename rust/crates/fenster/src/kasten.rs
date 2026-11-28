//! The box drawing characters U+2500 to U+257F in the terminal, painted as lines of the cell and not as glyphs of a font:
//! each line reaches the edge of its cell, so neighbouring cells join without a gap and a frame closes (B35). The
//! straight lines are rectangles on whole pixels without anti-aliasing; only arcs and diagonals are strokes.

// The geometry names edges, middles and bands with single letters, as the formulas do.
#![allow(clippy::many_single_char_names)]

use egui::epaint::Mesh;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

/// Weight of an arm: none, light, heavy, double.
const KEINE: u8 = 0;
const LEICHT: u8 = 1;
const SCHWER: u8 = 2;
const DOPPELT: u8 = 3;

/// The arms of U+2500 to U+257F from their Unicode names: two bits per direction (up, right, down, left from the low
/// bits on), each [`KEINE`], [`LEICHT`], [`SCHWER`] or [`DOPPELT`]. 0 marks the dashes, arcs and diagonals, which
/// [`malen`] draws on their own.
const ARME: [u8; 128] = [
    // ─━│┃┄┅┆┇┈┉┊┋┌┍┎┏
    0x44, 0x88, 0x11, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x18, 0x24, 0x28,
    // ┐┑┒┓└┕┖┗┘┙┚┛├┝┞┟
    0x50, 0x90, 0x60, 0xa0, 0x05, 0x09, 0x06, 0x0a, 0x41, 0x81, 0x42, 0x82, 0x15, 0x19, 0x16, 0x25,
    // ┠┡┢┣┤┥┦┧┨┩┪┫┬┭┮┯
    0x26, 0x1a, 0x29, 0x2a, 0x51, 0x91, 0x52, 0x61, 0x62, 0x92, 0xa1, 0xa2, 0x54, 0x94, 0x58, 0x98,
    // ┰┱┲┳┴┵┶┷┸┹┺┻┼┽┾┿
    0x64, 0xa4, 0x68, 0xa8, 0x45, 0x85, 0x49, 0x89, 0x46, 0x86, 0x4a, 0x8a, 0x55, 0x95, 0x59, 0x99,
    // ╀╁╂╃╄╅╆╇╈╉╊╋╌╍╎╏
    0x56, 0x65, 0x66, 0x96, 0x5a, 0xa5, 0x69, 0x9a, 0xa9, 0xa6, 0x6a, 0xaa, 0x00, 0x00, 0x00, 0x00,
    // ═║╒╓╔╕╖╗╘╙╚╛╜╝╞╟
    0xcc, 0x33, 0x1c, 0x34, 0x3c, 0xd0, 0x70, 0xf0, 0x0d, 0x07, 0x0f, 0xc1, 0x43, 0xc3, 0x1d, 0x37,
    // ╠╡╢╣╤╥╦╧╨╩╪╫╬╭╮╯
    0x3f, 0xd1, 0x73, 0xf3, 0xdc, 0x74, 0xfc, 0xcd, 0x47, 0xcf, 0xdd, 0x77, 0xff, 0x00, 0x00, 0x00,
    // ╰╱╲╳╴╵╶╷╸╹╺╻╼╽╾╿
    0x00, 0x00, 0x00, 0x00, 0x40, 0x01, 0x04, 0x10, 0x80, 0x02, 0x08, 0x20, 0x48, 0x21, 0x84, 0x12,
];

/// Whether `c` is a box drawing character that [`malen`] paints.
#[must_use]
pub fn ist_kasten(c: char) -> bool {
    ('\u{2500}'..='\u{257f}').contains(&c)
}

/// The arms of `c` as (up, right, down, left), or `None` outside the straight lines.
fn arme(c: char) -> Option<[u8; 4]> {
    let i = usize::try_from(u32::from(c).checked_sub(0x2500)?).ok()?;
    let a = *ARME.get(i)?;
    (a != 0).then_some([a & 3, (a >> 2) & 3, (a >> 4) & 3, (a >> 6) & 3])
}

/// The bands of a line of weight `w` across an axis, in pixels: one for light and heavy, two for double. `mitte` is
/// the middle of the cell on that axis, `t` the light thickness.
fn baender(w: u8, mitte: f32, t: f32) -> Vec<(f32, f32)> {
    match w {
        LEICHT => {
            let a = (mitte - t / 2.0).round();
            vec![(a, a + t)]
        }
        SCHWER => {
            let a = (mitte - t).round();
            vec![(a, a + 2.0 * t)]
        }
        DOPPELT => {
            let a = (mitte - 1.5 * t).round();
            vec![(a, a + t), (a + 2.0 * t, a + 3.0 * t)]
        }
        _ => Vec::new(),
    }
}

/// Paints `c` into the cell `zelle` (points) in `farbe`; a character outside U+2500 to U+257F paints nothing.
pub fn malen(maler: &Painter, c: char, zelle: Rect, farbe: Color32) {
    if !ist_kasten(c) {
        return;
    }
    let ppp = maler.pixels_per_point();
    let px = |v: f32| (v * ppp).round();
    let (x0, x1, y0, y1) = (
        px(zelle.left()),
        px(zelle.right()),
        px(zelle.top()),
        px(zelle.bottom()),
    );
    // the light thickness: one pixel for a cell of up to about 12 pixels, more on a screen with more pixels per point
    let t = ((x1 - x0) / 8.0).round().max(1.0);
    let (xm, ym) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
    let mut mesh = Mesh::default();
    let mut rechteck = |xa: f32, xb: f32, ya: f32, yb: f32| {
        if xb > xa && yb > ya {
            mesh.add_colored_rect(
                Rect::from_min_max(pos2(xa / ppp, ya / ppp), pos2(xb / ppp, yb / ppp)),
                farbe,
            );
        }
    };
    if let Some([o, r, u, l]) = arme(c) {
        gerade(&mut rechteck, [o, r, u, l], (x0, x1, y0, y1), (xm, ym), t);
    } else {
        let code = u32::from(c);
        let striche = match code {
            0x2504..=0x2507 => 3.0,
            0x2508..=0x250b => 4.0,
            0x254c..=0x254f => 2.0,
            _ => 0.0,
        };
        if striche > 0.0 {
            let w = if code % 2 == 0 { LEICHT } else { SCHWER };
            let senkrecht = matches!(code, 0x2506 | 0x2507 | 0x250a | 0x250b | 0x254e | 0x254f);
            gestrichelt(
                &mut rechteck,
                w,
                senkrecht,
                striche,
                (x0, x1, y0, y1),
                (xm, ym),
                t,
            );
        } else {
            let b = baender(LEICHT, xm, t)[0];
            let h = baender(LEICHT, ym, t)[0];
            let (xc, yc) = (f32::midpoint(b.0, b.1), f32::midpoint(h.0, h.1));
            let s = Stroke::new(t / ppp, farbe);
            let p = |x: f32, y: f32| Pos2::new(x / ppp, y / ppp);
            if (0x256d..=0x2570).contains(&code) {
                {
                    // the arcs: from the middle of one edge round the middle of the cell to the middle of the other
                    let rad = (x1 - x0).min(y1 - y0) / 2.0;
                    let (sx, kante_x) = if matches!(code, 0x256d | 0x2570) {
                        (1.0, x1)
                    } else {
                        (-1.0, x0)
                    };
                    let (sy, kante_y) = if matches!(code, 0x256d | 0x256e) {
                        (1.0, y1)
                    } else {
                        (-1.0, y0)
                    };
                    let (mx, my) = (xc + sx * rad, yc + sy * rad);
                    let mut punkte = vec![p(kante_x, yc)];
                    for i in 0..=8_u8 {
                        let w = f32::from(i) / 8.0 * std::f32::consts::FRAC_PI_2;
                        punkte.push(p(mx - sx * rad * w.sin(), my - sy * rad * w.cos()));
                    }
                    punkte.push(p(xc, kante_y));
                    maler.add(Shape::line(punkte, s));
                }
            } else {
                // the diagonals
                if matches!(code, 0x2571 | 0x2573) {
                    maler.line_segment([p(x1, y0), p(x0, y1)], s);
                }
                if matches!(code, 0x2572 | 0x2573) {
                    maler.line_segment([p(x0, y0), p(x1, y1)], s);
                }
            }
        }
    }
    maler.add(Shape::mesh(mesh));
}

/// The straight arms (up, right, down, left) of a cell, all in pixels. A double line keeps its two strokes apart at a
/// corner, a junction and a crossing, as the glyphs of the Unicode chart show.
fn gerade(
    rechteck: &mut impl FnMut(f32, f32, f32, f32),
    [o, r, u, l]: [u8; 4],
    (x0, x1, y0, y1): (f32, f32, f32, f32),
    (xm, ym): (f32, f32),
    t: f32,
) {
    // the bands of each arm across its direction: x for up and down, y for left and right
    let (bo, bu) = (baender(o, xm, t), baender(u, xm, t));
    let (bl, br) = (baender(l, ym, t), baender(r, ym, t));
    let senkrecht: Vec<(f32, f32)> = bo.iter().chain(&bu).copied().collect();
    let waagrecht: Vec<(f32, f32)> = bl.iter().chain(&br).copied().collect();
    let da = |w: u8| w != KEINE;
    let doppelt = |w: u8| w == DOPPELT;
    // right arm
    if da(r) {
        if doppelt(r) {
            let (oben, unten) = (br[0], br[1]);
            let a = doppel_anfang(o, u, &bo, &bu, l, xm, false);
            let b = doppel_anfang(u, o, &bu, &bo, l, xm, false);
            rechteck(a, x1, oben.0, oben.1);
            rechteck(b, x1, unten.0, unten.1);
        } else {
            let a = einfach(o, u, &senkrecht, l, xm, false);
            rechteck(a, x1, br[0].0, br[0].1);
        }
    }
    // left arm
    if da(l) {
        if doppelt(l) {
            let (oben, unten) = (bl[0], bl[1]);
            let a = doppel_anfang(o, u, &bo, &bu, r, xm, true);
            let b = doppel_anfang(u, o, &bu, &bo, r, xm, true);
            rechteck(x0, a, oben.0, oben.1);
            rechteck(x0, b, unten.0, unten.1);
        } else {
            let e = einfach(o, u, &senkrecht, r, xm, true);
            rechteck(x0, e, bl[0].0, bl[0].1);
        }
    }
    // down arm
    if da(u) {
        if doppelt(u) {
            let (links, rechts) = (bu[0], bu[1]);
            let a = doppel_anfang(l, r, &bl, &br, o, ym, false);
            let b = doppel_anfang(r, l, &br, &bl, o, ym, false);
            rechteck(links.0, links.1, a, y1);
            rechteck(rechts.0, rechts.1, b, y1);
        } else {
            let a = einfach(l, r, &waagrecht, o, ym, false);
            rechteck(bu[0].0, bu[0].1, a, y1);
        }
    }
    // up arm
    if da(o) {
        if doppelt(o) {
            let (links, rechts) = (bo[0], bo[1]);
            let a = doppel_anfang(l, r, &bl, &br, u, ym, true);
            let b = doppel_anfang(r, l, &br, &bl, u, ym, true);
            rechteck(links.0, links.1, y0, a);
            rechteck(rechts.0, rechts.1, y0, b);
        } else {
            let e = einfach(l, r, &waagrecht, u, ym, true);
            rechteck(bo[0].0, bo[0].1, y0, e);
        }
    }
}

/// Where a light or heavy arm ends towards the middle. `vorn` and `hinten` are the two arms across it, `quer` their
/// bands, `gegen` the arm opposite; `zum_kleinen` is true for the arms that end at a larger coordinate (left and up),
/// whose end is the far edge.
fn einfach(
    vorn: u8,
    hinten: u8,
    quer: &[(f32, f32)],
    gegen: u8,
    mitte: f32,
    zum_kleinen: bool,
) -> f32 {
    let doppelt = vorn == DOPPELT || hinten == DOPPELT;
    if doppelt {
        // straight through crosses the double line; otherwise it reaches the near line of a junction (both arms across)
        // or the far line of a corner (one arm across)
        if gegen != KEINE {
            return mitte;
        }
        let beide = vorn != KEINE && hinten != KEINE;
        let (nah, fern) = (quer[0], quer[1]);
        return match (zum_kleinen, beide) {
            (true, true) => nah.1,
            (true, false) => fern.1,
            (false, true) => fern.0,
            (false, false) => nah.0,
        };
    }
    if quer.is_empty() {
        mitte
    } else if zum_kleinen {
        quer.iter().map(|x| x.1).fold(f32::MIN, f32::max)
    } else {
        quer.iter().map(|x| x.0).fold(f32::MAX, f32::min)
    }
}

/// Where one stroke of a double arm ends towards the middle. `naechst` is the arm across on the side of this stroke (up
/// for the upper stroke of a horizontal arm), `fern` the arm across on the other side, with their bands; `gegen` is the
/// opposite arm; `zum_kleinen` as in [`einfach`].
fn doppel_anfang(
    naechst: u8,
    fern: u8,
    b_naechst: &[(f32, f32)],
    b_fern: &[(f32, f32)],
    gegen: u8,
    mitte: f32,
    zum_kleinen: bool,
) -> f32 {
    if naechst != KEINE {
        // the stroke stops at the arm on its side: at its inner line when double, at its edge when single
        return match (naechst == DOPPELT, zum_kleinen) {
            (true, false) => b_naechst[1].0,
            (true, true) | (false, false) => b_naechst[0].1,
            (false, true) => b_naechst[0].0,
        };
    }
    if gegen != KEINE {
        return mitte;
    }
    if fern != KEINE {
        // the outer stroke of a corner runs to the outer edge of the arm on the other side
        return if zum_kleinen {
            b_fern[b_fern.len() - 1].1
        } else {
            b_fern[0].0
        };
    }
    mitte
}

/// A dashed line of `striche` dashes in the cell, light or heavy, across or down.
fn gestrichelt(
    rechteck: &mut impl FnMut(f32, f32, f32, f32),
    w: u8,
    senkrecht: bool,
    striche: f32,
    (x0, x1, y0, y1): (f32, f32, f32, f32),
    (xm, ym): (f32, f32),
    t: f32,
) {
    let (a, b) = if senkrecht { (y0, y1) } else { (x0, x1) };
    let band = baender(w, if senkrecht { xm } else { ym }, t)[0];
    let feld = (b - a) / striche;
    let mut i = 0.0;
    while i < striche {
        let von = (a + feld * (i + 0.2)).round();
        let bis = (a + feld * (i + 0.8)).round().max(von + 1.0);
        if senkrecht {
            rechteck(band.0, band.1, von, bis);
        } else {
            rechteck(von, bis, band.0, band.1);
        }
        i += 1.0;
    }
}

#[cfg(test)]
// the test cells are a few pixels; the counts fit f32 exactly
#[allow(clippy::cast_precision_loss)]
mod tests {
    use super::*;
    use crate::bild::Leinwand;
    use egui::{LayerId, RawInput, vec2};

    /// The size of a test cell in pixels (one point is one pixel): light 2, heavy 4, double 2 + 2 gap + 2.
    const B: usize = 16;
    const H: usize = 32;

    /// Paints all 128 characters, each in its own cell with empty cells around it.
    fn alle() -> Leinwand {
        let ctx = egui::Context::default();
        let mut lw = Leinwand::neu(32 * B, 16 * H, Color32::BLACK);
        let groesse = vec2(lw.breite as f32, lw.hoehe as f32);
        let mut aus = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, groesse)),
                ..RawInput::default()
            },
            |ui| {
                let maler = Painter::new(
                    ui.ctx().clone(),
                    LayerId::background(),
                    Rect::from_min_size(Pos2::ZERO, groesse),
                );
                for i in 0..128_u32 {
                    let c = char::from_u32(0x2500 + i).expect("a character");
                    let (x, y) = zelle(i);
                    assert!(ist_kasten(c));
                    malen(
                        &maler,
                        c,
                        Rect::from_min_size(pos2(x as f32, y as f32), vec2(B as f32, H as f32)),
                        Color32::WHITE,
                    );
                }
                assert!(!ist_kasten('a') && !ist_kasten('\u{2580}'));
            },
        );
        lw.texturen(&aus.textures_delta);
        aus.textures_delta.clear();
        let prims = ctx.tessellate(aus.shapes, aus.pixels_per_point);
        lw.pixel.fill([0.0, 0.0, 0.0, 1.0]);
        lw.malen(&prims, aus.pixels_per_point);
        lw
    }

    /// The left upper pixel of the cell of character `i`.
    fn zelle(i: u32) -> (usize, usize) {
        let i = i as usize;
        ((i % 16) * 2 * B, (i / 16) * 2 * H)
    }

    /// The runs of lit pixels along a line of the canvas, as their lengths.
    fn laeufe(punkte: impl Iterator<Item = bool>) -> Vec<usize> {
        let mut aus = Vec::new();
        let mut n = 0;
        for an in punkte.chain(std::iter::once(false)) {
            if an {
                n += 1;
            } else if n > 0 {
                aus.push(n);
                n = 0;
            }
        }
        aus
    }

    #[test]
    fn every_straight_line_reaches_the_edges_of_its_arms_with_its_weight() {
        let lw = alle();
        let an = |x: usize, y: usize| lw.pixel[y * lw.breite + x][0] > 0.5;
        let erwartet = |w: u8| match w {
            LEICHT => vec![2],
            SCHWER => vec![4],
            DOPPELT => vec![2, 2],
            _ => Vec::new(),
        };
        let mut geprueft = 0;
        for i in 0..128_u32 {
            let c = char::from_u32(0x2500 + i).expect("a character");
            let Some([o, r, u, l]) = arme(c) else {
                continue;
            };
            let (x, y) = zelle(i);
            let oben = laeufe((x..x + B).map(|px| an(px, y)));
            let unten = laeufe((x..x + B).map(|px| an(px, y + H - 1)));
            let links = laeufe((y..y + H).map(|py| an(x, py)));
            let rechts = laeufe((y..y + H).map(|py| an(x + B - 1, py)));
            assert_eq!(oben, erwartet(o), "{c} up");
            assert_eq!(unten, erwartet(u), "{c} down");
            assert_eq!(links, erwartet(l), "{c} left");
            assert_eq!(rechts, erwartet(r), "{c} right");
            // nothing outside the cell
            assert!(
                !an(x + B, y + H / 2) && !an(x + B / 2, y + H),
                "{c} outside"
            );
            geprueft += 1;
        }
        // 128 characters less 12 dashes, 4 arcs and 3 diagonals
        assert_eq!(geprueft, 109);
    }

    #[test]
    fn a_double_corner_keeps_its_strokes_apart_and_a_light_corner_is_solid() {
        let lw = alle();
        let an = |x: usize, y: usize| lw.pixel[y * lw.breite + x][0] > 0.5;
        // ╔ (U+2554): along the diagonal from the middle to the lower right corner two strokes cross it, not one block
        let (x, y) = zelle(0x54);
        let diagonale = laeufe((0..B / 2).map(|d| an(x + B / 2 - 3 + d, y + H / 2 - 3 + d)));
        assert_eq!(diagonale.len(), 2, "{diagonale:?}");
        // ┌ (U+250C): the corner pixel where both arms meet is lit
        let (x, y) = zelle(0x0c);
        assert!(an(x + B / 2, y + H / 2));
    }

    #[test]
    fn the_arcs_and_dashes_touch_their_edges() {
        let lw = alle();
        let an = |x: usize, y: usize| lw.pixel[y * lw.breite + x][0] > 0.3;
        // ╭ (U+256D): the middle of the right edge and of the lower edge
        let (x, y) = zelle(0x6d);
        assert!((y..y + H).any(|py| an(x + B - 1, py)), "right");
        assert!((x..x + B).any(|px| an(px, y + H - 1)), "down");
        assert!(!(y..y + H).any(|py| an(x, py)), "not left");
        // ┄ (U+2504): three dashes
        let (x, y) = zelle(0x04);
        let mitte = (y..y + H)
            .find(|&py| an(x + B / 2, py))
            .expect("a dash in the middle");
        assert_eq!(laeufe((x..x + B).map(|px| an(px, mitte))).len(), 3);
    }
}
