//! The symbols of the window, painted with lines: the fonts that come with egui have no gear, shield or folder, and the
//! window loads nothing from the network (OF4 point 3). Each symbol button is a button in the accessibility tree with
//! its name (SE3 point 1).

// The painter names centre, half size and corners with single letters, as the geometry does.
#![allow(clippy::many_single_char_names)]

use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, WidgetInfo, pos2, vec2};

/// The symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// Show or hide the sidebar.
    Leiste,
    /// Open or close the inspector.
    Inspektor,
    /// Settings.
    Zahnrad,
    /// Requests.
    Schild,
    /// Folder.
    Ordner,
    /// New session.
    Plus,
    /// Enlarge a pane.
    Vergroessern,
    /// The application menu (B75).
    Menue,
    /// Close a window.
    Schliessen,
    /// The settings page "Aussehen": a circle, half filled.
    Kontrast,
    /// The settings page "Sitzung": a terminal with a prompt.
    Terminal,
    /// The settings page "Harnesses und Modelle": a chip.
    Chip,
    /// The settings page "Grenzen des Efforts": a gauge.
    Regler,
}

/// Paints symbol `s` into `r` with colour `c`.
pub fn malen(ui: &Ui, s: Symbol, r: Rect, c: Color32) {
    let p = ui.painter();
    let st = Stroke::new(1.4, c);
    let m = r.center();
    let h = r.width().min(r.height()) / 2.0;
    match s {
        Symbol::Leiste | Symbol::Inspektor => {
            let rahmen = Rect::from_center_size(m, vec2(h * 1.8, h * 1.4));
            p.rect_stroke(rahmen, 2.0, st, egui::StrokeKind::Inside);
            let x = if s == Symbol::Leiste {
                rahmen.left() + rahmen.width() * 0.33
            } else {
                rahmen.right() - rahmen.width() * 0.33
            };
            p.line_segment([pos2(x, rahmen.top()), pos2(x, rahmen.bottom())], st);
        }
        Symbol::Zahnrad => {
            p.circle_stroke(m, h * 0.42, st);
            for i in 0..8 {
                let w = f32::from(u8::try_from(i).unwrap_or(0)) * std::f32::consts::FRAC_PI_4;
                let d = Vec2::angled(w);
                p.line_segment([m + d * h * 0.62, m + d * h * 0.92], Stroke::new(2.2, c));
            }
            p.circle_stroke(m, h * 0.68, st);
        }
        Symbol::Schild => {
            let punkte = vec![
                pos2(m.x, m.y - h * 0.9),
                pos2(m.x + h * 0.75, m.y - h * 0.55),
                pos2(m.x + h * 0.6, m.y + h * 0.35),
                pos2(m.x, m.y + h * 0.9),
                pos2(m.x - h * 0.6, m.y + h * 0.35),
                pos2(m.x - h * 0.75, m.y - h * 0.55),
            ];
            p.add(Shape::closed_line(punkte, st));
        }
        Symbol::Ordner => {
            let k = Rect::from_center_size(m + vec2(0.0, h * 0.1), vec2(h * 1.8, h * 1.3));
            p.add(Shape::closed_line(
                vec![
                    k.left_top(),
                    pos2(k.left() + h * 0.6, k.top()),
                    pos2(k.left() + h * 0.8, k.top() + h * 0.25),
                    pos2(k.right(), k.top() + h * 0.25),
                    k.right_bottom(),
                    k.left_bottom(),
                ],
                st,
            ));
        }
        Symbol::Plus => {
            p.line_segment([pos2(m.x - h * 0.7, m.y), pos2(m.x + h * 0.7, m.y)], st);
            p.line_segment([pos2(m.x, m.y - h * 0.7), pos2(m.x, m.y + h * 0.7)], st);
        }
        Symbol::Vergroessern => {
            let a = h * 0.75;
            for (ecke, dx, dy) in [
                (pos2(m.x - a, m.y - a), 1.0, 1.0),
                (pos2(m.x + a, m.y - a), -1.0, 1.0),
                (pos2(m.x - a, m.y + a), 1.0, -1.0),
                (pos2(m.x + a, m.y + a), -1.0, -1.0),
            ] {
                p.line_segment([ecke, ecke + vec2(dx * a * 0.6, 0.0)], st);
                p.line_segment([ecke, ecke + vec2(0.0, dy * a * 0.6)], st);
            }
        }
        Symbol::Menue
        | Symbol::Schliessen
        | Symbol::Kontrast
        | Symbol::Terminal
        | Symbol::Chip
        | Symbol::Regler => weitere(p, s, m, h, st, c),
    }
}

/// The symbols of the menu, of closing and of the pages of the settings.
fn weitere(p: &egui::Painter, s: Symbol, m: Pos2, h: f32, st: Stroke, c: Color32) {
    match s {
        Symbol::Menue => {
            for dy in [-0.55, 0.0, 0.55] {
                p.line_segment(
                    [
                        pos2(m.x - h * 0.75, m.y + dy * h),
                        pos2(m.x + h * 0.75, m.y + dy * h),
                    ],
                    st,
                );
            }
        }
        Symbol::Schliessen => {
            let a = h * 0.55;
            p.line_segment([pos2(m.x - a, m.y - a), pos2(m.x + a, m.y + a)], st);
            p.line_segment([pos2(m.x + a, m.y - a), pos2(m.x - a, m.y + a)], st);
        }
        Symbol::Kontrast => {
            let rr = h * 0.8;
            p.circle_stroke(m, rr, st);
            let mut halb = vec![m];
            for i in 0..=16 {
                let w = std::f32::consts::FRAC_PI_2
                    + f32::from(u8::try_from(i).unwrap_or(0)) * std::f32::consts::PI / 16.0;
                halb.push(m + Vec2::angled(w) * rr);
            }
            p.add(Shape::convex_polygon(halb, c, Stroke::NONE));
        }
        Symbol::Terminal => {
            let k = Rect::from_center_size(m, vec2(h * 1.8, h * 1.4));
            p.rect_stroke(k, 2.0, st, egui::StrokeKind::Inside);
            let x = k.left() + h * 0.35;
            let y = k.top() + h * 0.45;
            p.line_segment([pos2(x, y), pos2(x + h * 0.3, y + h * 0.25)], st);
            p.line_segment([pos2(x + h * 0.3, y + h * 0.25), pos2(x, y + h * 0.5)], st);
            p.line_segment(
                [
                    pos2(x + h * 0.45, y + h * 0.55),
                    pos2(x + h * 0.9, y + h * 0.55),
                ],
                st,
            );
        }
        Symbol::Chip => {
            let k = Rect::from_center_size(m, Vec2::splat(h * 1.1));
            p.rect_stroke(k, 2.0, st, egui::StrokeKind::Inside);
            p.rect_stroke(k.shrink(h * 0.3), 1.0, st, egui::StrokeKind::Inside);
            for t in [-0.3, 0.0, 0.3] {
                let d = t * h;
                p.line_segment(
                    [pos2(m.x + d, k.top()), pos2(m.x + d, k.top() - h * 0.3)],
                    st,
                );
                p.line_segment(
                    [
                        pos2(m.x + d, k.bottom()),
                        pos2(m.x + d, k.bottom() + h * 0.3),
                    ],
                    st,
                );
                p.line_segment(
                    [pos2(k.left(), m.y + d), pos2(k.left() - h * 0.3, m.y + d)],
                    st,
                );
                p.line_segment(
                    [pos2(k.right(), m.y + d), pos2(k.right() + h * 0.3, m.y + d)],
                    st,
                );
            }
        }
        Symbol::Regler => {
            let rr = h * 0.85;
            let mitte = m + vec2(0.0, h * 0.35);
            let bogen: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let w = std::f32::consts::PI
                        + f32::from(u8::try_from(i).unwrap_or(0)) * std::f32::consts::PI / 16.0;
                    mitte + Vec2::angled(w) * rr
                })
                .collect();
            p.add(Shape::line(bogen, st));
            p.line_segment([mitte, mitte + Vec2::angled(-0.8) * rr * 0.8], st);
            p.circle_filled(mitte, 1.6, c);
        }
        _ => {}
    }
}

/// A round button with symbol `s`, name `name` and side `groesse`; `gewaehlt` paints the chosen ground.
pub fn knopf(ui: &mut Ui, s: Symbol, name: &str, groesse: f32, gewaehlt: bool) -> Response {
    knopf_form(ui, s, name, Vec2::splat(groesse), groesse / 2.0, gewaehlt)
}

/// A button with symbol `s` of size `groesse` and corner radius `radius`.
pub fn knopf_form(
    ui: &mut Ui,
    s: Symbol,
    name: &str,
    groesse: Vec2,
    radius: f32,
    gewaehlt: bool,
) -> Response {
    let (r, resp) = ui.allocate_exact_size(groesse, Sense::click());
    let v = ui.visuals();
    if gewaehlt {
        ui.painter().rect_filled(r, radius, v.selection.bg_fill);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(r, radius, v.widgets.hovered.weak_bg_fill);
    }
    let seite = groesse.x.min(groesse.y);
    let flaeche = Rect::from_center_size(r.center(), Vec2::splat(seite * 0.62));
    malen(ui, s, flaeche, v.widgets.inactive.fg_stroke.color);
    let name = name.to_owned();
    resp.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, gewaehlt, &name));
    resp
}

/// A small dot of the state colour at `p`.
pub fn punkt(ui: &Ui, p: Pos2, farbe: Color32, voll: bool) {
    let r = crate::gestalt::MASSE.punkt / 2.0;
    if voll {
        ui.painter().circle_filled(p, r, farbe);
    } else {
        ui.painter()
            .circle_stroke(p, r - 0.75, Stroke::new(1.5, farbe));
    }
}
