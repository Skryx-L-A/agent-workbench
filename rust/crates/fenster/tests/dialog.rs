//! The settings (B72, B78) measured on the painted picture and on the accessibility tree of egui: the dialog stays with a
//! margin inside the middle and dims the ground (OF2, OF5), exactly the open page is marked (OF9 point 1), and the track
//! of a locked switch holds 3:1 to its ground (OF3 point 5). The page of supervision shows no control without a key of
//! the catalog (OF8, decision of the operator, 27.11.2026).
//!
//! Every situation of the settings in [`fenster::szenen`] is measured, as a user and as an admin, light and dark.

use std::collections::{BTreeMap, BTreeSet};

use egui::accesskit::{Node, NodeId, Role, Toggled};
use egui::{Color32, Id, Pos2, RawInput, Rect, vec2};
use fenster::attrappe::Form;
use fenster::bild::Leinwand;
use fenster::fenster::Szene;
use fenster::gestalt::{MASSE, kontrast, leuchtdichte};

const BREITE: usize = 1280;
const HOEHE: usize = 800;

/// One painted frame: the tree, the picture and the place of the settings.
struct Bild {
    baum: BTreeMap<NodeId, Node>,
    lw: Leinwand,
    dialog: Option<Rect>,
}

impl Bild {
    /// Runs the window on the target state of the mock-up, sets `szene` after the first frames, paints the last frame.
    fn neu(szene: Option<&Szene>, admin: bool, dunkel: bool) -> Bild {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        fenster::einrichten(&ctx);
        let (mut f, g) = fenster::mit_attrappe(Form::Ziel);
        g.borrow_mut().admin_setzen(admin);
        f.system_dunkel = Some(dunkel);
        let mut lw = Leinwand::neu(BREITE, HOEHE, Color32::BLACK);
        let mut baum = BTreeMap::new();
        let mut letzte = None;
        for i in 0..20_u8 {
            if i == 3
                && let Some(s) = szene
            {
                f.szene(s);
            }
            #[allow(clippy::cast_precision_loss)]
            let eingabe = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    vec2(BREITE as f32, HOEHE as f32),
                )),
                time: Some(f64::from(i) * 0.05),
                system_theme: Some(if dunkel {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                }),
                ..RawInput::default()
            };
            let mut aus = ctx.run_ui(eingabe, |ui| f.zeichnen(ui));
            lw.texturen(&aus.textures_delta);
            aus.textures_delta.clear();
            if let Some(u) = aus.platform_output.accesskit_update.take() {
                baum = u.nodes.into_iter().collect();
            }
            letzte = Some(aus);
        }
        let aus = letzte.expect("ein Bild");
        lw.pixel.fill([0.0, 0.0, 0.0, 1.0]);
        let prims = ctx.tessellate(aus.shapes, aus.pixels_per_point);
        lw.malen(&prims, aus.pixels_per_point);
        let dialog = ctx.memory(|m| m.area_rect(Id::unique("einstellungen-fenster")));
        Bild { baum, lw, dialog }
    }

    /// The colour of the pixel at `x`, `y` (opaque).
    fn pixel(&self, x: usize, y: usize) -> Color32 {
        let p = self.lw.pixel[y * self.lw.breite + x];
        // clamped to 0..=1 before the cast
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let k = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgb(k(p[0]), k(p[1]), k(p[2]))
    }

    /// The nodes below the nodes with element id `kennung`, those included.
    fn unter(&self, kennungen: &[&str]) -> Vec<&Node> {
        let mut offen: Vec<NodeId> = self
            .baum
            .iter()
            .filter(|(_, n)| n.author_id().is_some_and(|a| kennungen.contains(&a)))
            .map(|(id, _)| *id)
            .collect();
        let mut gesehen = BTreeSet::new();
        let mut aus = Vec::new();
        while let Some(id) = offen.pop() {
            if !gesehen.insert(id) {
                continue;
            }
            if let Some(n) = self.baum.get(&id) {
                aus.push(n);
                offen.extend(n.children().iter().copied());
            }
        }
        aus
    }
}

/// The situations of the settings: name, situation, whether the viewer is an admin.
fn einstellungen() -> Vec<(&'static str, Szene, bool)> {
    let v: Vec<_> = fenster::szenen()
        .into_iter()
        .filter(|(n, ..)| n.starts_with("einstellungen-"))
        .collect();
    assert!(!v.is_empty(), "keine Lage der Einstellungen");
    v
}

#[test]
fn the_settings_stay_inside_the_middle_with_a_margin_and_dim_the_ground() {
    #[allow(clippy::cast_precision_loss)]
    let (b, h) = (BREITE as f32, HOEHE as f32);
    for dunkel in [false, true] {
        let ohne = Bild::neu(None, false, dunkel);
        assert!(ohne.dialog.is_none(), "ohne Lage kein Dialog");
        for (name, szene, admin) in einstellungen() {
            let mit = Bild::neu(Some(&szene), admin, dunkel);
            let r = mit.dialog.unwrap_or_else(|| panic!("{name}: kein Dialog"));
            // below the title row and above the foot of the sidebar, each with a margin; inside the window
            assert!(
                r.top() > MASSE.titelleiste,
                "{name}: oben {} reicht in die Titelleiste (bis {})",
                r.top(),
                MASSE.titelleiste
            );
            assert!(
                r.bottom() < h - MASSE.leistenfuss,
                "{name}: unten {} reicht in den Fuß der Leiste (ab {})",
                r.bottom(),
                h - MASSE.leistenfuss
            );
            assert!(
                r.left() > 0.0 && r.right() < b,
                "{name}: {r:?} nicht im Fenster"
            );
            // outside the dialog the ground is darker than without it: in the title row and in the foot of the sidebar
            for (x, y) in [(4, 4), (4, HOEHE - 4)] {
                let (vorher, nachher) = (ohne.pixel(x, y), mit.pixel(x, y));
                assert!(
                    leuchtdichte(nachher) < leuchtdichte(vorher) * 0.9,
                    "{name}: ({x}, {y}) nicht abgedunkelt: {vorher:?} -> {nachher:?}"
                );
            }
        }
    }
}

#[test]
fn exactly_the_open_page_is_marked() {
    for (name, szene, admin) in einstellungen() {
        let bild = Bild::neu(Some(&szene), admin, true);
        let markiert: Vec<String> = bild
            .unter(&["B72", "B78"])
            .into_iter()
            .filter(|n| n.role() == Role::Button && n.toggled() == Some(Toggled::True))
            .map(|n| format!("{:?} {:?}", n.label(), n.author_id()))
            .collect();
        assert_eq!(
            markiert.len(),
            1,
            "{name}: markierte Seiten in der Liste: {markiert:?}"
        );
    }
}

/// The keys of the catalog of the pilot (`regelebenen` 15.1): `sicht` gives exactly one entry per key, so the first view
/// of a case of the contract names them all.
fn katalog() -> BTreeSet<String> {
    let pfad = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/regelebenen/erwartet-setzen/setzen-nutzer-angenommen.json"
    );
    let fall: serde_json::Value =
        serde_json::from_slice(&std::fs::read(pfad).expect("Fall lesen")).expect("JSON");
    let sicht = fall["schritte"]
        .as_array()
        .expect("Schritte")
        .iter()
        .find_map(|s| {
            s.get("einstellungen")
                .and_then(serde_json::Value::as_object)
        })
        .expect("eine Sicht");
    sicht.keys().cloned().collect()
}

#[test]
fn the_page_of_supervision_shows_no_control_without_a_key_of_the_catalog() {
    let katalog = katalog();
    assert!(katalog.contains("aufsicht.einloesefrist_ms"), "{katalog:?}");
    let mut gemessen = 0;
    for (name, szene, admin) in einstellungen()
        .into_iter()
        .filter(|(n, ..)| n.contains("aufsicht"))
    {
        let bild = Bild::neu(Some(&szene), admin, false);
        for n in bild.unter(&["einstellungsseite"]) {
            // a control: a switch, or a choice (a button that is chosen or not)
            if n.role() != Role::Switch && !(n.role() == Role::Button && n.toggled().is_some()) {
                continue;
            }
            let k = n.author_id().unwrap_or("");
            assert!(
                katalog.contains(k),
                "{name}: Bedienelement {:?} ohne Schlüssel des Katalogs: {k:?}",
                n.label()
            );
            gemessen += 1;
        }
    }
    assert!(
        gemessen > 0,
        "kein Bedienelement auf der Seite Aufsicht gemessen"
    );
}

/// The effective deadline for each MCP server (`regelebenen` 15.1.2): the mock-up sends the catalogue 120 s and the
/// server `tickets` with 30 s; the page shows 30 s with the origin of the server and `wiki` with the catalogue, for a
/// person and for an admin, without a control.
#[test]
fn the_page_of_supervision_shows_the_effective_deadline_for_each_server_with_its_origin() {
    let mut seiten = 0;
    for (name, szene, admin) in einstellungen()
        .into_iter()
        .filter(|(n, ..)| n.contains("aufsicht"))
    {
        let bild = Bild::neu(Some(&szene), admin, false);
        let texte: BTreeSet<String> = bild
            .unter(&["einstellungsseite"])
            .iter()
            .flat_map(|n| [n.label(), n.value()])
            .flatten()
            .map(str::to_owned)
            .collect();
        for soll in [
            "Frist der Entscheidung je MCP-Server",
            "Frist zum Einlösen je MCP-Server",
            "tickets: 30 s",
            "wiki: 2 min",
            "tickets: 5 min",
            "wiki: 1 min",
            "Firmendatei, Server tickets",
            "Firmendatei, Server wiki",
            "Katalog",
        ] {
            assert!(texte.contains(soll), "{name}: {soll:?} fehlt in {texte:?}");
        }
        assert!(
            !texte.contains("tickets: 2 min"),
            "{name}: der Katalog lockert den strengeren Server"
        );
        seiten += 1;
    }
    assert_eq!(seiten, 2, "Aufsicht als Nutzer und als Admin");
}

#[test]
fn a_locked_switch_holds_three_to_one_to_its_ground() {
    let mut gemessen = 0;
    for dunkel in [false, true] {
        for (name, szene, admin) in einstellungen() {
            let bild = Bild::neu(Some(&szene), admin, dunkel);
            for n in bild.baum.values() {
                if n.role() != Role::Switch || !n.is_disabled() {
                    continue;
                }
                let r = n.bounds().expect("Lage des Schalters");
                // the track where the knob is not: right of it when off, left of it when on; the ground just above
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let (x0, y0, cy) = (
                    r.x0.round() as usize,
                    r.y0.round() as usize,
                    f64::midpoint(r.y0, r.y1).round() as usize,
                );
                let x = if n.toggled() == Some(Toggled::True) {
                    x0 + 9
                } else {
                    x0 + 29
                };
                let (spur, grund) = (bild.pixel(x, cy), bild.pixel(x0 + 19, y0 - 5));
                let k = kontrast(spur, grund);
                assert!(
                    k >= 3.0,
                    "{name} ({}): gesperrter Schalter {:?}: Spur {spur:?} auf Grund {grund:?} hat {k:.2}:1",
                    if dunkel { "dunkel" } else { "hell" },
                    n.label()
                );
                gemessen += 1;
            }
        }
    }
    assert!(gemessen > 0, "kein gesperrter Schalter gemessen");
}
