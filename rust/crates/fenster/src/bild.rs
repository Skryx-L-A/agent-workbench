//! Pictures of the window without a window (OF13): egui runs without a platform, its triangles are painted by a small
//! software painter and written as PNG. Nothing opens on the screen, nothing needs a GPU or a display.
//!
//! Acceptance pictures come only from a real start: `werkbank-fenster --attrappe ziel --szene <name> --thema
//! hell|dunkel` on a display with a real GL driver (for example Xvfb with llvmpipe in a throwaway VM), taken of the
//! window. This painter is not the window: it once left a dotted line through every big rounded rectangle and strokes in
//! its lower right corner that no real start showed. Its pictures serve the tests and a quick look; a fault seen only
//! here is first checked against a real start.

// The painter names corners and weights with single letters, as the formulas do.
#![allow(clippy::many_single_char_names)]

use std::collections::HashMap;
use std::io::Write;

use egui::epaint::{ClippedPrimitive, Primitive};
use egui::{Color32, Pos2, RawInput, Rect, TextureId, Vec2, pos2, vec2};

use crate::fenster::Fenster;

/// A texture of egui: size and premultiplied sRGBA texels.
struct Textur {
    breite: usize,
    hoehe: usize,
    texel: Vec<Color32>,
}

/// The painter: premultiplied sRGBA pixels, blended like the glow painter of egui (in gamma space).
pub struct Leinwand {
    /// Width in pixels.
    pub breite: usize,
    /// Height in pixels.
    pub hoehe: usize,
    /// The pixels, row by row: r, g, b, a in 0 to 1, premultiplied.
    pub pixel: Vec<[f32; 4]>,
    texturen: HashMap<TextureId, Textur>,
}

impl Leinwand {
    /// A canvas of this size filled with `grund`.
    #[must_use]
    pub fn neu(breite: usize, hoehe: usize, grund: Color32) -> Leinwand {
        let g = farbe(grund);
        Leinwand {
            breite,
            hoehe,
            pixel: vec![g; breite * hoehe],
            texturen: HashMap::new(),
        }
    }

    /// Takes the texture changes of one frame of egui.
    pub fn texturen(&mut self, delta: &egui::TexturesDelta) {
        for (id, d) in delta
            .set
            .iter()
            .flat_map(|(id, ds)| ds.iter().map(move |d| (id, d)))
        {
            let egui::ImageData::Color(bild) = &d.image;
            let [w, h] = bild.size;
            match d.pos {
                None => {
                    self.texturen.insert(
                        *id,
                        Textur {
                            breite: w,
                            hoehe: h,
                            texel: bild.pixels.clone(),
                        },
                    );
                }
                Some([x0, y0]) => {
                    if let Some(t) = self.texturen.get_mut(id) {
                        for y in 0..h {
                            for x in 0..w {
                                if x0 + x < t.breite && y0 + y < t.hoehe {
                                    t.texel[(y0 + y) * t.breite + x0 + x] = bild.pixels[y * w + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.texturen.remove(id);
        }
    }

    fn textur(&self, id: TextureId, uv: Pos2) -> [f32; 4] {
        let Some(t) = self.texturen.get(&id) else {
            return [1.0; 4];
        };
        // bilinear, clamped at the edge
        #[allow(clippy::cast_precision_loss)]
        let (fx, fy) = (uv.x * t.breite as f32 - 0.5, uv.y * t.hoehe as f32 - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (ax, ay) = (fx - x0, fy - y0);
        let hol = |x: f32, y: f32| {
            // clamped into the texture before the cast
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let (xi, yi) = (
                x.clamp(0.0, (t.breite - 1) as f32) as usize,
                y.clamp(0.0, (t.hoehe - 1) as f32) as usize,
            );
            farbe(t.texel[yi * t.breite + xi])
        };
        let (a, b, c, d) = (
            hol(x0, y0),
            hol(x0 + 1.0, y0),
            hol(x0, y0 + 1.0),
            hol(x0 + 1.0, y0 + 1.0),
        );
        let mut aus = [0.0; 4];
        for i in 0..4 {
            let oben = a[i] * (1.0 - ax) + b[i] * ax;
            let unten = c[i] * (1.0 - ax) + d[i] * ax;
            aus[i] = oben * (1.0 - ay) + unten * ay;
        }
        aus
    }

    /// Paints the tessellated primitives of one frame at `ppp` pixels per point.
    pub fn malen(&mut self, prims: &[ClippedPrimitive], ppp: f32) {
        for p in prims {
            let Primitive::Mesh(mesh) = &p.primitive else {
                continue;
            };
            let clip = Rect::from_min_max(p.clip_rect.min * ppp, p.clip_rect.max * ppp);
            for tri in mesh.indices.as_chunks::<3>().0 {
                let v = [
                    &mesh.vertices[tri[0] as usize],
                    &mesh.vertices[tri[1] as usize],
                    &mesh.vertices[tri[2] as usize],
                ];
                self.dreieck(&v, mesh.texture_id, clip, ppp);
            }
        }
    }

    fn dreieck(&mut self, v: &[&egui::epaint::Vertex; 3], tex: TextureId, clip: Rect, ppp: f32) {
        let p: Vec<Pos2> = v
            .iter()
            .map(|x| pos2(x.pos.x * ppp, x.pos.y * ppp))
            .collect();
        let flaeche = (p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[2].x - p[0].x) * (p[1].y - p[0].y);
        if flaeche.abs() < 1e-6 {
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        let rahmen = Rect::from_min_max(
            pos2(
                p.iter().map(|q| q.x).fold(f32::MAX, f32::min),
                p.iter().map(|q| q.y).fold(f32::MAX, f32::min),
            ),
            pos2(
                p.iter().map(|q| q.x).fold(f32::MIN, f32::max),
                p.iter().map(|q| q.y).fold(f32::MIN, f32::max),
            ),
        )
        .intersect(clip)
        .intersect(Rect::from_min_size(
            Pos2::ZERO,
            vec2(self.breite as f32, self.hoehe as f32),
        ));
        if !rahmen.is_positive() {
            return;
        }
        let farben: Vec<[f32; 4]> = v.iter().map(|x| farbe(x.color)).collect();
        // the edges with the inside on their left; a pixel centre on an edge belongs to one triangle only (top-left
        // rule), else a shared edge of two half-transparent triangles is painted twice and shows as a seam
        let s = flaeche.signum();
        let kanten = [(p[2] - p[1]) * s, (p[0] - p[2]) * s, (p[1] - p[0]) * s];
        let oben_links = |e: Vec2| e.y < -1e-6 || (e.y.abs() <= 1e-6 && e.x > 0.0);
        // which side of an edge a pixel centre lies on is decided on the unscaled edge function (twice the area of the
        // edge and the centre), the same number with the opposite sign in both triangles of a shared edge; a tolerance
        // on the scaled weights instead depends on the area, and a centre near the long edge of a big and a thin
        // triangle then belongs to neither (a dotted line through every rounded rectangle)
        let s = f64::from(s);
        let draussen = |e: f64, k: Vec2| {
            let e = e * s;
            e < 0.0 || (e <= 0.0 && !oben_links(k))
        };
        // the bounds are inside the canvas
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x0, x1, y0, y1) = (
            rahmen.min.x.floor() as usize,
            (rahmen.max.x.ceil() as usize).min(self.breite),
            rahmen.min.y.floor() as usize,
            (rahmen.max.y.ceil() as usize).min(self.hoehe),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                #[allow(clippy::cast_precision_loss)]
                let q = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let e = [
                    kantenfunktion(p[1], p[2], q),
                    kantenfunktion(p[2], p[0], q),
                    kantenfunktion(p[0], p[1], q),
                ];
                if e.iter().zip(kanten).any(|(e, k)| draussen(*e, k)) {
                    continue;
                }
                let summe = e[0] + e[1] + e[2];
                // weights in 0 to 1 for the colours and the texture
                #[allow(clippy::cast_possible_truncation)]
                let (w0, w1) = ((e[0] / summe) as f32, (e[1] / summe) as f32);
                let w2 = 1.0 - w0 - w1;
                let uv = pos2(
                    v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2,
                    v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2,
                );
                let t = self.textur(tex, uv);
                let mut quelle = [0.0; 4];
                for i in 0..4 {
                    quelle[i] = (farben[0][i] * w0 + farben[1][i] * w1 + farben[2][i] * w2) * t[i];
                }
                let ziel = &mut self.pixel[y * self.breite + x];
                for i in 0..4 {
                    ziel[i] = quelle[i] + ziel[i] * (1.0 - quelle[3]);
                }
            }
        }
    }

    /// The canvas as PNG (RGBA, 8 bit, opaque).
    #[must_use]
    pub fn png(&self) -> Vec<u8> {
        let mut roh = Vec::with_capacity((self.breite * 4 + 1) * self.hoehe);
        for y in 0..self.hoehe {
            roh.push(0);
            for x in 0..self.breite {
                let p = self.pixel[y * self.breite + x];
                for c in &p[..3] {
                    // clamped to 0..=255 before the cast
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    roh.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
                roh.push(255);
            }
        }
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let _ = z.write_all(&roh);
        let idat = z.finish().unwrap_or_default();
        let mut aus = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&u32::try_from(self.breite).unwrap_or(0).to_be_bytes());
        ihdr.extend_from_slice(&u32::try_from(self.hoehe).unwrap_or(0).to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        stueck(&mut aus, *b"IHDR", &ihdr);
        stueck(&mut aus, *b"IDAT", &idat);
        stueck(&mut aus, *b"IEND", &[]);
        aus
    }
}

fn farbe(c: Color32) -> [f32; 4] {
    [
        f32::from(c.r()) / 255.0,
        f32::from(c.g()) / 255.0,
        f32::from(c.b()) / 255.0,
        f32::from(c.a()) / 255.0,
    ]
}

/// Twice the signed area of the edge `a` to `b` and the point `q`, in f64. The edge is always taken from its smaller end,
/// so the edge `b` to `a` gives exactly the negated number: two triangles agree on every point of their shared edge.
fn kantenfunktion(a: Pos2, b: Pos2, q: Pos2) -> f64 {
    let (u, w, vorzeichen) = if (a.x, a.y) <= (b.x, b.y) {
        (a, b, 1.0)
    } else {
        (b, a, -1.0)
    };
    let (ux, uy) = (f64::from(u.x), f64::from(u.y));
    let (qx, qy) = (f64::from(q.x) - ux, f64::from(q.y) - uy);
    vorzeichen * ((f64::from(w.x) - ux) * qy - (f64::from(w.y) - uy) * qx)
}

fn crc32(teile: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for t in teile {
        for b in *t {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn stueck(aus: &mut Vec<u8>, art: [u8; 4], daten: &[u8]) {
    aus.extend_from_slice(&u32::try_from(daten.len()).unwrap_or(0).to_be_bytes());
    aus.extend_from_slice(&art);
    aus.extend_from_slice(daten);
    aus.extend_from_slice(&crc32(&[&art, daten]).to_be_bytes());
}

/// Runs the window `fenster` for `bilder` frames without a window on the canvas `lw` (its size in points) and paints
/// the last one. `dunkel` is the theme of the system. The canvas keeps the textures of egui between calls.
pub fn aufnehmen(
    ctx: &egui::Context,
    fenster: &mut Fenster,
    lw: &mut Leinwand,
    bilder: usize,
    dunkel: bool,
) {
    fenster.system_dunkel = Some(dunkel);
    #[allow(clippy::cast_precision_loss)]
    let groesse = vec2(lw.breite as f32, lw.hoehe as f32);
    let mut letzte = None;
    for _ in 0..bilder.max(1) {
        let zeit = ctx.input(|i| i.time) + 0.05;
        let eingabe = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, groesse)),
            time: Some(zeit),
            system_theme: Some(if dunkel {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            }),
            ..RawInput::default()
        };
        let mut aus = ctx.run_ui(eingabe, |ui| fenster.zeichnen(ui));
        lw.texturen(&aus.textures_delta);
        aus.textures_delta.clear();
        letzte = Some(aus);
    }
    if let Some(aus) = letzte {
        lw.pixel.fill([0.0, 0.0, 0.0, 1.0]);
        let prims = ctx.tessellate(aus.shapes, aus.pixels_per_point);
        lw.malen(&prims, aus.pixels_per_point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_png_has_signature_and_a_known_crc() {
        // CRC-32 of "IEND" with no data is 0xae426082 (PNG specification)
        assert_eq!(crc32(&[b"IEND", &[]]), 0xae42_6082);
        let lw = Leinwand::neu(3, 2, Color32::from_rgb(10, 20, 30));
        let png = lw.png();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
    }

    #[test]
    fn a_triangle_covers_its_pixels() {
        let mut lw = Leinwand::neu(4, 4, Color32::BLACK);
        let mut mesh = egui::epaint::Mesh::default();
        mesh.add_colored_rect(
            Rect::from_min_max(pos2(1.0, 1.0), pos2(3.0, 3.0)),
            Color32::WHITE,
        );
        lw.malen(
            &[ClippedPrimitive {
                clip_rect: Rect::from_min_size(Pos2::ZERO, vec2(4.0, 4.0)),
                primitive: Primitive::Mesh(mesh),
            }],
            1.0,
        );
        assert_eq!(lw.pixel[4 + 1], [1.0; 4]);
        assert_eq!(lw.pixel[0], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_big_rounded_rectangle_has_no_gap_inside() {
        // the size of the settings: egui fills it as a fan of long thin triangles from one corner
        let rechteck = Rect::from_min_max(pos2(80.0, 20.0), pos2(1200.0, 780.0));
        let mut mesh = egui::epaint::Mesh::default();
        egui::epaint::Tessellator::new(
            1.0,
            egui::epaint::TessellationOptions::default(),
            [1, 1],
            Vec::new(),
        )
        .tessellate_rect(
            &egui::epaint::RectShape::filled(
                rechteck,
                egui::CornerRadius::same(12),
                Color32::WHITE,
            ),
            &mut mesh,
        );
        let mut lw = Leinwand::neu(1280, 800, Color32::BLACK);
        lw.malen(
            &[ClippedPrimitive {
                clip_rect: Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0)),
                primitive: Primitive::Mesh(mesh),
            }],
            1.0,
        );
        let innen = rechteck.shrink(1.0);
        let ecke = 14.0;
        let mut luecken = Vec::new();
        for y in 21..779 {
            for x in 81..1199 {
                #[allow(clippy::cast_precision_loss)]
                let q = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let in_ecke = (q.x < innen.left() + ecke || q.x > innen.right() - ecke)
                    && (q.y < innen.top() + ecke || q.y > innen.bottom() - ecke);
                if !in_ecke
                    && lw.pixel[y * 1280 + x]
                        .iter()
                        .any(|c| (c - 1.0).abs() > 1e-4)
                {
                    luecken.push((x, y));
                }
            }
        }
        assert!(
            luecken.is_empty(),
            "{} pixels not painted, first {:?}",
            luecken.len(),
            &luecken[..luecken.len().min(8)]
        );
    }
}
