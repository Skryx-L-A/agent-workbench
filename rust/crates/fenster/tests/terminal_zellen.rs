//! The terminal picture paints each character in the cell the raster gives it, and the box drawing characters fill their
//! cells: a frame around a line of mixed characters (umlauts, a wide character, a mark of no width, an emoji, a skin
//! tone) closes on all four sides. The picture is painted without a window by the software painter of the crate.

use egui::{CornerRadius, FontId, Id, Pos2, RawInput, Rect, vec2};
use fenster::bild::Leinwand;
use fenster::gestalt::TERMINAL_DUNKEL;
use fenster::terminal;
use rasterauszug::{Bild, Lauf, Stil};

/// The frame: the middle line holds characters of width 1, 2 and 0; inside it is 14 columns wide in the raster.
const RAHMEN: [&str; 3] = [
    "┌──────────────┐",
    "│äß€…中e\u{301}😀👍\u{1f3fd}„“a│",
    "└──────────────┘",
];

/// Paints the frame in a font of `punkte` points at `ppp` pixels per point and gives the canvas, the cell size and the
/// left upper corner of the raster, all in pixels.
fn malen(punkte: f32, ppp: f32) -> (Leinwand, f32, f32, Pos2) {
    let ctx = egui::Context::default();
    fenster::einrichten(&ctx);
    ctx.set_pixels_per_point(ppp);
    let mut bild = Bild::leer(20, 3);
    bild.lines = RAHMEN
        .iter()
        .map(|t| {
            vec![Lauf {
                stil: Stil::VORGABE,
                text: (*t).to_owned(),
            }]
        })
        .collect();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut lw = Leinwand::neu(
        (240.0 * ppp) as usize,
        (80.0 * ppp) as usize,
        TERMINAL_DUNKEL.grund,
    );
    let mut zelle = (0.0, 0.0);
    let mut letzte = None;
    for i in 0..2 {
        let eingabe = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(240.0, 80.0))),
            time: Some(f64::from(i) * 0.05),
            ..RawInput::default()
        };
        let mut aus = ctx.run_ui(eingabe, |ui| {
            let schrift = FontId::monospace(punkte);
            zelle = terminal::zelle(ui, &schrift);
            terminal::malen(
                ui,
                Rect::from_min_size(Pos2::ZERO, vec2(240.0, 80.0)),
                CornerRadius::ZERO,
                Some(&bild),
                &TERMINAL_DUNKEL,
                &schrift,
                None,
                Id::NULL.with("terminal"),
                false,
            );
        });
        lw.texturen(&aus.textures_delta);
        aus.textures_delta.clear();
        letzte = Some(aus);
    }
    let aus = letzte.expect("two frames ran");
    lw.pixel.fill([0.0, 0.0, 0.0, 1.0]);
    let prims = ctx.tessellate(aus.shapes, aus.pixels_per_point);
    lw.malen(&prims, aus.pixels_per_point);
    assert_eq!(aus.pixels_per_point, ppp);
    // the raster starts 6 points inside the area (`terminal::malen`)
    (
        lw,
        zelle.0 * ppp,
        zelle.1 * ppp,
        Pos2::new(6.0 * ppp, 6.0 * ppp),
    )
}

/// Whether the pixel is lit by the white text colour (the ground is 0x1e).
fn hell(lw: &Leinwand, x: usize, y: usize) -> bool {
    lw.pixel[y * lw.breite + x][0] > 0.6
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn px(v: f32) -> usize {
    v.floor() as usize
}

/// The x of the lit pixel nearest to the middle of column `spalte` in the middle of row `zeile`.
fn senkrechte(lw: &Leinwand, zw: f32, zh: f32, o: Pos2, spalte: f32, zeile: f32) -> Option<usize> {
    let y = px(o.y + (zeile + 0.5) * zh);
    let (a, b) = (px(o.x + spalte * zw), px(o.x + (spalte + 1.0) * zw));
    let mitte = px(o.x + (spalte + 0.5) * zw);
    (a..b)
        .filter(|&x| hell(lw, x, y))
        .min_by_key(|&x| x.abs_diff(mitte))
}

/// The y of the lit pixel nearest to the middle of row `zeile` in the middle of column `spalte`.
fn waagrechte(lw: &Leinwand, zw: f32, zh: f32, o: Pos2, spalte: f32, zeile: f32) -> Option<usize> {
    let x = px(o.x + (spalte + 0.5) * zw);
    let (a, b) = (px(o.y + zeile * zh), px(o.y + (zeile + 1.0) * zh));
    let mitte = px(o.y + (zeile + 0.5) * zh);
    (a..b)
        .filter(|&y| hell(lw, x, y))
        .min_by_key(|&y| y.abs_diff(mitte))
}

#[test]
fn a_frame_around_mixed_characters_closes_without_gaps() {
    for (punkte, ppp) in [
        (12.0, 1.0),
        (13.0, 1.0),
        (14.0, 1.0),
        (12.0, 2.0),
        (13.0, 2.0),
        (14.0, 2.0),
    ] {
        rahmen_pruefen(punkte, ppp);
    }
}

fn rahmen_pruefen(punkte: f32, ppp: f32) {
    let (lw, zw, zh, o) = malen(punkte, ppp);
    // 16 columns: the corners and the strokes at the right stand in column 15 in all three rows
    let rechts = 15.0;
    let strich = |spalte: f32| {
        senkrechte(&lw, zw, zh, o, spalte, 1.0).unwrap_or_else(|| {
            panic!(
                "{punkte} pt, {ppp} ppp: no vertical stroke in column {spalte} of the middle row"
            )
        })
    };
    let linie = |zeile: f32| {
        waagrechte(&lw, zw, zh, o, 7.0, zeile)
            .unwrap_or_else(|| panic!("{punkte} pt, {ppp} ppp: no horizontal line in row {zeile}"))
    };
    let (xl, xr) = (strich(0.0), strich(rechts));
    let (yo, yu) = (linie(0.0), linie(2.0));
    // the vertical strokes of the middle row run on from the upper line to the lower one, without a gap
    for x in [xl, xr] {
        let luecken: Vec<usize> = (yo..=yu).filter(|&y| !hell(&lw, x, y)).collect();
        assert!(
            luecken.is_empty(),
            "{punkte} pt, {ppp} ppp: the stroke at x={x} has gaps at y={luecken:?}"
        );
    }
    // the upper and the lower line run from the left stroke to the right one, without a gap
    for y in [yo, yu] {
        let luecken: Vec<usize> = (xl..=xr).filter(|&x| !hell(&lw, x, y)).collect();
        assert!(
            luecken.is_empty(),
            "{punkte} pt, {ppp} ppp: the line at y={y} has gaps at x={luecken:?}"
        );
    }
    // the right stroke stands in the column of the right corners: its x is the end of both lines
    // the right stroke stands in the column of the right corners: both lines end inside the band of the stroke
    let ym = px(o.y + 1.5 * zh);
    let band_von = (0..=xr).rev().take_while(|&x| hell(&lw, x, ym)).last();
    let band_bis = (xr..lw.breite).take_while(|&x| hell(&lw, x, ym)).last();
    let ende = |y: usize| (xl..lw.breite).take_while(|&x| hell(&lw, x, y)).last();
    for y in [yo, yu] {
        let e = ende(y);
        assert!(
            e >= band_von && e <= band_bis,
            "{punkte} pt, {ppp} ppp: the line at y={y} ends at {e:?}, the stroke is {band_von:?} to {band_bis:?}"
        );
    }
}
