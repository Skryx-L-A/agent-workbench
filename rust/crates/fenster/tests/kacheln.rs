//! The tiles of the worker view measured in the painted picture (OF13 painter, 1 pixel = 1 point): a joint of 4 points
//! in the ground of the stage between two tiles, corners of 8 points, a frame of 2 points on the active tile only when
//! more than one tile stands on the stage.

// The measuring names picture, row, roles and terminals with single letters, as the painter of the pictures does.
#![allow(clippy::many_single_char_names)]

use egui::Color32;
use fenster::attrappe::Form;
use fenster::bild::{self, Leinwand};
use fenster::fenster::Szene;
use fenster::gestalt::{self, GRAU_DUNKEL, HELL, MASSE, Rollen, TERMINAL_DUNKEL};

/// A painted picture of a situation: the pixels as colours, and the frame colour of the window.
struct Bild {
    lw: Leinwand,
    rahmen: Color32,
}

impl Bild {
    fn neu(szene: &Szene, dunkel: bool) -> Bild {
        let ctx = egui::Context::default();
        fenster::einrichten(&ctx);
        let (mut f, _griff) = fenster::mit_attrappe(Form::Ziel);
        let mut lw = Leinwand::neu(1280, 800, Color32::BLACK);
        bild::aufnehmen(&ctx, &mut f, &mut lw, 3, dunkel);
        f.szene(szene);
        bild::aufnehmen(&ctx, &mut f, &mut lw, 8, dunkel);
        let rahmen = f.rahmenfarbe(rollen(dunkel));
        Bild { lw, rahmen }
    }

    fn px(&self, x: usize, y: usize) -> Color32 {
        let p = self.lw.pixel[y * self.lw.breite + x];
        // 0 to 1, clamped before the cast
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let k = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgb(k(p[0]), k(p[1]), k(p[2]))
    }

    /// Runs of equal colour along row `y`: (start, length, colour).
    fn zeile(&self, y: usize) -> Vec<(usize, usize, Color32)> {
        laeufe((0..self.lw.breite).map(|x| self.px(x, y)))
    }

    /// Runs of equal colour along column `x`.
    fn spalte(&self, x: usize) -> Vec<(usize, usize, Color32)> {
        laeufe((0..self.lw.hoehe).map(|y| self.px(x, y)))
    }
}

fn rollen(dunkel: bool) -> &'static Rollen {
    if dunkel { &GRAU_DUNKEL } else { &HELL }
}

fn laeufe(farben: impl Iterator<Item = Color32>) -> Vec<(usize, usize, Color32)> {
    let mut aus: Vec<(usize, usize, Color32)> = Vec::new();
    for (i, c) in farben.enumerate() {
        match aus.last_mut() {
            Some((_, n, f)) if *f == c => *n += 1,
            _ => aus.push((i, 1, c)),
        }
    }
    aus
}

fn nah(a: Color32, b: Color32) -> bool {
    a.r().abs_diff(b.r()) <= 2 && a.g().abs_diff(b.g()) <= 2 && a.b().abs_diff(b.b()) <= 2
}

/// The long runs of terminal ground along row `y`: the terminals of the tiles side by side.
fn terminals(b: &Bild, y: usize) -> Vec<(usize, usize)> {
    b.zeile(y)
        .into_iter()
        .filter(|(_, n, c)| *n > 100 && *c == TERMINAL_DUNKEL.grund)
        .map(|(s, n, _)| (s, n))
        .collect()
}

/// The head of a tile in column `x`: the first run of 24 points in the colour of a head, below the title row.
fn kopf_oben(b: &Bild, x: usize, r: &Rollen) -> usize {
    b.spalte(x)
        .into_iter()
        .find(|(_, n, c)| *n >= 20 && (*c == r.kopf || *c == r.kopf_aktiv))
        .map(|(s, _, _)| s)
        .expect("no head of a tile in this column")
}

fn sechs_kacheln(dunkel: bool) {
    let r = rollen(dunkel);
    let b = Bild::neu(&Szene::WorkerKacheln, dunkel);
    let y = 300;
    let t = terminals(&b, y);
    assert_eq!(t.len(), 3, "three tiles side by side in row {y}: {t:?}");
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (fuge, rahmen, ecke) = (
        MASSE.kachelabstand as usize,
        MASSE.fokusrahmen as usize,
        MASSE.radius_kachel as usize,
    );
    assert_eq!((fuge, rahmen, ecke), (4, 2, 8));
    // no joint at the edge of the stage: the last tile ends at the edge of the window (inspector closed), and the three
    // tiles are equally wide (the terminal of the active one is narrower by its frame)
    assert_eq!(
        t[2].0 + t[2].1,
        b.lw.breite,
        "the last tile ends at the edge"
    );
    assert!(t[1].1.abs_diff(t[2].1) <= 1, "equal tiles: {t:?}");
    assert!(
        (t[0].1 + 2 * rahmen).abs_diff(t[1].1) <= 1,
        "equal tiles: {t:?}"
    );
    // the active tile (the first) carries the frame on both sides, the others none
    for x in t[0].0 - rahmen..t[0].0 {
        assert_eq!(
            b.px(x, y),
            b.rahmen,
            "frame left of the active tile at x={x}"
        );
    }
    let ende0 = t[0].0 + t[0].1;
    for x in ende0..ende0 + rahmen {
        assert_eq!(
            b.px(x, y),
            b.rahmen,
            "frame right of the active tile at x={x}"
        );
    }
    // the joints: 4 points of the stage, nothing else
    let fuge1: Vec<Color32> = (ende0 + rahmen..t[1].0).map(|x| b.px(x, y)).collect();
    assert_eq!(fuge1, vec![r.buehne; fuge], "joint between tile 1 and 2");
    let ende1 = t[1].0 + t[1].1;
    let fuge2: Vec<Color32> = (ende1..t[2].0).map(|x| b.px(x, y)).collect();
    assert_eq!(fuge2, vec![r.buehne; fuge], "joint between tile 2 and 3");
    // the second tile: the joint to the next row, and the rounded corners
    let x0 = t[1].0;
    let mitte = x0 + t[1].1 / 2;
    let oben = kopf_oben(&b, mitte, r);
    assert!(
        nah(b.px(x0, oben), r.buehne),
        "upper corner is rounded: {:?}",
        b.px(x0, oben)
    );
    assert_eq!(b.px(x0 + ecke, oben), r.kopf, "the head beyond the corner");
    let spalte = b.spalte(mitte);
    let i = spalte
        .iter()
        .position(|(s, n, c)| *s > oben + 100 && *n == fuge && *c == r.buehne)
        .expect("a joint below the first row");
    let (unten, _, _) = spalte[i];
    assert_eq!(
        spalte[i - 1].2,
        TERMINAL_DUNKEL.grund,
        "terminal above the joint"
    );
    assert!(
        spalte[i + 1].2 == r.kopf || spalte[i + 1].2 == r.kopf_aktiv,
        "head below the joint"
    );
    assert!(
        nah(b.px(x0, unten - 1), r.buehne),
        "lower corner is rounded: {:?}",
        b.px(x0, unten - 1)
    );
    // no frame on a tile without focus: the frame colour nowhere in the second tile's row
    assert!(
        (t[1].0 - fuge..ende1 + fuge).all(|x| b.px(x, y) != b.rahmen),
        "a tile without focus has no frame"
    );
}

#[test]
fn six_tiles_dark_have_joints_corners_and_one_frame() {
    sechs_kacheln(true);
}

#[test]
fn six_tiles_light_have_joints_corners_and_one_frame() {
    sechs_kacheln(false);
}

#[test]
fn a_single_tile_has_no_frame() {
    for dunkel in [true, false] {
        let r = rollen(dunkel);
        let b = Bild::neu(&Szene::WorkerEinzeln, dunkel);
        let y = 300;
        let t = terminals(&b, y);
        assert_eq!(t.len(), 1, "one tile: {t:?}");
        let (s, n) = t[0];
        assert_ne!(b.px(s - 1, y), b.rahmen, "no frame left");
        assert_ne!(b.px(s + n, y), b.rahmen, "no frame right");
        let oben = kopf_oben(&b, s + n / 2, r);
        assert!(nah(b.px(s, oben), r.buehne), "corner still rounded");
        assert!(
            (0..b.lw.breite).all(|x| b.px(x, oben + 1) != b.rahmen),
            "no frame over the head"
        );
    }
}

#[test]
fn the_stage_of_dark_stands_out_from_the_terminal() {
    // the joint of today's app (#1f2020) is 1.02:1 on the terminal; the stage takes the ground of the sidebar
    assert_eq!(GRAU_DUNKEL.buehne, Color32::from_rgb(0x29, 0x2a, 0x2a));
    assert_eq!(
        gestalt::BLAUSCHWARZ.buehne,
        Color32::from_rgb(0x1b, 0x1e, 0x22)
    );
    assert_eq!(HELL.buehne, Color32::WHITE);
    assert!(gestalt::kontrast(GRAU_DUNKEL.buehne, TERMINAL_DUNKEL.grund) > 1.1);
}

#[test]
fn without_the_keyboard_no_row_carries_a_focus_frame() {
    // the window "Sitzungen" puts the focus on its first row; opened without the keyboard it shows no frame: no run of
    // 20 pixels in the accent colour along any row (a frame of 2 points around a row of the list is far longer)
    let b = Bild::neu(&Szene::SitzungenGewaehlt("s-3".to_owned()), true);
    let laeufe: Vec<(usize, usize)> = (0..b.lw.hoehe)
        .flat_map(|y| {
            b.zeile(y)
                .into_iter()
                .filter(|(_, n, c)| *n >= 20 && *c == GRAU_DUNKEL.akzent)
                .map(move |(x, _, _)| (x, y))
        })
        .collect();
    assert!(
        laeufe.is_empty(),
        "runs in the accent colour of the focus frame: {laeufe:?}"
    );
}
