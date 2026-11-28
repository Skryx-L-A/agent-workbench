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

    /// The children of `n` in the order of the tree.
    fn kinder(&self, n: &Node) -> Vec<&Node> {
        n.children()
            .iter()
            .filter_map(|id| self.baum.get(id))
            .collect()
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
            // a control: a switch, a drop-down, or a segment of a choice (a button that is chosen or not)
            let bedienbar = match n.role() {
                Role::Switch | Role::ComboBox => true,
                Role::Button => n.toggled().is_some(),
                _ => false,
            };
            if !bedienbar {
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

/// The situation of the settings named `name`, painted light.
fn seite(name: &str) -> Bild {
    let (_, szene, admin) = einstellungen()
        .into_iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("keine Lage {name}"));
    Bild::neu(Some(&szene), admin, false)
}

/// Decision of the operator, 28.11.2026: the deadlines per MCP server are one table like in the browser view, with the
/// columns server, deadline of the decision and deadline to redeem, each value with its origin, every word from
/// `ansicht::woerter`. The mock-up sends the catalogue 120 s and 5 min; `tickets` sets 30 s, `wiki` 1 min to redeem
/// (`regelebenen` 15.1.2): the stricter server shows, the catalogue does not loosen it. For a person and for an admin.
#[test]
fn the_deadlines_per_server_are_one_table_with_the_origin_of_each_value() {
    use ansicht::woerter as w;
    let zelle = |sekunden: u32, herkunft: &str| {
        format!("{sekunden} {}{}{herkunft}", w::EINHEIT_SEKUNDEN, w::TRENNER)
    };
    let soll = vec![
        vec![
            w::SERVER.to_owned(),
            w::werte::FREIGABEFRIST.1.to_owned(),
            w::werte::EINLOESEFRIST.1.to_owned(),
        ],
        vec![
            "tickets".to_owned(),
            zelle(30, w::HERKUNFT_FIRMENDATEI),
            zelle(300, w::HERKUNFT_KATALOG),
        ],
        vec![
            "wiki".to_owned(),
            zelle(120, w::HERKUNFT_KATALOG),
            zelle(60, w::HERKUNFT_FIRMENDATEI),
        ],
    ];
    for name in [
        "einstellungen-aufsicht-nutzer",
        "einstellungen-aufsicht-admin",
    ] {
        let bild = seite(name);
        let tabelle = bild
            .unter(&["fristen-je-server"])
            .into_iter()
            .find(|n| n.role() == Role::Table)
            .unwrap_or_else(|| panic!("{name}: keine Tabelle der Fristen je Server"));
        assert_eq!(tabelle.label(), Some(w::FRISTEN_JE_SERVER), "{name}");
        let zeilen: Vec<&Node> = bild
            .kinder(tabelle)
            .into_iter()
            .filter(|z| z.role() == Role::Row)
            .collect();
        let texte: Vec<Vec<String>> = zeilen
            .iter()
            .map(|z| {
                bild.kinder(z)
                    .iter()
                    .filter_map(|c| c.label())
                    .map(str::to_owned)
                    .collect()
            })
            .collect();
        assert_eq!(texte, soll, "{name}");
        let kopf: Vec<Role> = zeilen
            .first()
            .map(|z| bild.kinder(z).iter().map(|c| c.role()).collect())
            .unwrap_or_default();
        assert_eq!(kopf, [Role::ColumnHeader; 3], "{name}: Kopf der Tabelle");
        // a table to read: no control in it
        let bedienbar: Vec<&Node> = bild
            .unter(&["fristen-je-server"])
            .into_iter()
            .filter(|n| matches!(n.role(), Role::Button | Role::Switch | Role::ComboBox))
            .collect();
        assert!(bedienbar.is_empty(), "{name}: {bedienbar:?}");
    }
}

/// (segments, drop-downs) with the element id `kennung`.
fn form(bild: &Bild, kennung: &str) -> (usize, usize) {
    let n: Vec<&Node> = bild
        .baum
        .values()
        .filter(|n| n.author_id() == Some(kennung))
        .collect();
    (
        n.iter()
            .filter(|n| n.role() == Role::Button && n.toggled().is_some())
            .count(),
        n.iter().filter(|n| n.role() == Role::ComboBox).count(),
    )
}

/// Decision of the operator, 28.11.2026: in the settings up to four values are a segmented choice, from five values a
/// drop-down. Measured at the boundary on the page of supervision, on the look (theme three, font size six) and on the
/// page of a session (the programs and models the limit of the admins allows).
///
/// Changed with the decision of the operator, 28.11.2026, on the scope of the rule: the deadline of a decision was a
/// drop-down of five values, the four fixed ones and the effective 120 s. An effective value outside the fixed ones is
/// no choice any more but shows beside it (see the test of the deadlines below), so both deadlines are four segments.
#[test]
fn up_to_four_values_are_segments_and_five_or_more_a_drop_down() {
    let aufsicht = seite("einstellungen-aufsicht-admin");
    assert_eq!(
        form(&aufsicht, "aufsicht.einloesefrist_ms"),
        (4, 0),
        "vier Werte"
    );
    assert_eq!(
        form(&aufsicht, "aufsicht.freigabefrist_ms"),
        (4, 0),
        "vier feste Werte, der wirksame Wert 120 s steht daneben"
    );
    let aussehen = seite("einstellungen-aussehen-nutzer");
    assert_eq!(form(&aussehen, "aussehen.thema"), (3, 0), "drei Werte");
    assert_eq!(
        form(&aussehen, "terminal.schriftgroesse"),
        (0, 1),
        "sechs Werte, 11 bis 16"
    );
    let sitzung = seite("einstellungen-sitzung-nutzer");
    assert_eq!(
        form(&sitzung, "sitzung.harness"),
        (3, 0),
        "drei Programme in der Grenze"
    );
    assert_eq!(
        form(&sitzung, "sitzung.modell"),
        (2, 0),
        "zwei Modelle in der Grenze"
    );
}

/// Decision of the operator, 28.11.2026: the rule of segments holds only in the settings. The start form of the window
/// "Sessions" keeps a drop-down for each of its five choices, like the contract `seite` (SE7: the choices are fields of
/// the role `combobox`), even though each has at most three values.
#[test]
fn the_start_form_of_the_window_sessions_has_five_drop_downs() {
    use ansicht::woerter as w;
    let bild = Bild::neu(Some(&Szene::SitzungenLeer), false, false);
    for (kennung, name) in [
        ("harness-wahl", w::PROGRAMM),
        ("modell-wahl", "Modell"),
        ("effort-wahl", w::DENKSTUFE),
        ("route-wahl", "Route"),
        ("kunde-wahl", "Kunde"),
    ] {
        assert_eq!(form(&bild, kennung), (0, 1), "{kennung}: kein Ausklapper");
        let feld = bild
            .baum
            .values()
            .find(|n| n.author_id() == Some(kennung) && n.role() == Role::ComboBox)
            .expect("Ausklapper");
        assert_eq!(feld.label(), Some(name), "{kennung}");
    }
}

/// Decision of the operator, 28.11.2026: both deadlines of a release are a segmented choice of their four fixed values
/// (`ansicht::woerter::werte`). The mock-up sets the deadline of a decision to 120 s, which is none of them: no segment
/// is marked, and the effective value „2 min“ stands visibly beside the segments with its origin „Firma“ after it, as text
/// and not as a control. The deadline to redeem is 5 min, a fixed value, marked and with no extra text. For a person and
/// for an admin.
#[test]
fn the_deadlines_of_a_release_are_four_segments_and_an_effective_value_outside_stands_beside() {
    for name in [
        "einstellungen-aufsicht-nutzer",
        "einstellungen-aufsicht-admin",
    ] {
        let bild = seite(name);
        for (schluessel, soll, gewaehlt, daneben) in [
            (
                "aufsicht.freigabefrist_ms",
                ["30 s", "1 min", "5 min", "15 min"],
                None,
                Some("2 min"),
            ),
            (
                "aufsicht.einloesefrist_ms",
                ["1 min", "5 min", "1 h", "24 h"],
                Some("5 min"),
                None,
            ),
        ] {
            // left edge, text, marked, top and bottom of each segment
            let mut segmente: Vec<(f64, &str, bool, f64, f64)> = bild
                .baum
                .values()
                .filter(|n| n.author_id() == Some(schluessel))
                .inspect(|n| assert_ne!(n.role(), Role::ComboBox, "{name}: {schluessel}"))
                .filter(|n| n.role() == Role::Button && n.toggled().is_some())
                .map(|n| {
                    let b = n.bounds().expect("Lage");
                    (
                        b.x0,
                        n.label().unwrap_or_default(),
                        n.toggled() == Some(Toggled::True),
                        b.y0,
                        b.y1,
                    )
                })
                .collect();
            segmente.sort_by(|a, b| a.0.total_cmp(&b.0));
            let texte: Vec<&str> = segmente.iter().map(|s| s.1).collect();
            assert_eq!(texte, soll, "{name}: {schluessel}");
            let markiert: Vec<&str> = segmente.iter().filter(|s| s.2).map(|s| s.1).collect();
            assert_eq!(
                markiert,
                gewaehlt.into_iter().collect::<Vec<_>>(),
                "{name}: {schluessel}"
            );
            // the texts in the row left of the first segment, from left to right
            let (_, _, _, oben, unten) = segmente[0];
            let erstes = segmente[0].0;
            let mut links: Vec<(f64, &str)> = bild
                .baum
                .values()
                .filter(|n| n.role() == Role::Label)
                .filter_map(|n| {
                    let b = n.bounds()?;
                    let mitte = b.y0.midpoint(b.y1);
                    (b.x1 <= erstes + 0.5 && mitte > oben && mitte < unten)
                        // egui gives the text of a label as its value
                        .then(|| (b.x0, n.value().unwrap_or_default()))
                })
                .collect();
            links.sort_by(|a, b| a.0.total_cmp(&b.0));
            let links: Vec<&str> = links.into_iter().map(|(_, t)| t).collect();
            let rechts = links.len().saturating_sub(2);
            match daneben {
                Some(wert) => assert_eq!(
                    links[rechts..],
                    [wert, "Firma"],
                    "{name}: {schluessel}: {links:?}"
                ),
                None => assert_eq!(
                    links.last().copied(),
                    Some("Firma"),
                    "{name}: {schluessel}: {links:?}"
                ),
            }
        }
    }
}

/// Decision of the operator, 28.11.2026: the thinking level of a session offers „Vorgabe des Programms“ first, then the
/// efforts the limit of the admins allows for the chosen program, from the source of the browser view (the efforts of
/// the routes in `Startwahl`). The word for off comes from `ansicht::woerter`, never `none` or `off`. The mock-up allows
/// copilot `none`, `low` and `medium`; the person chose `medium`.
#[test]
fn the_thinking_level_offers_the_default_of_the_program_and_the_allowed_efforts() {
    use ansicht::woerter as w;
    let bild = seite("einstellungen-sitzung-nutzer");
    let mut wahl: Vec<(f64, String, bool)> = bild
        .baum
        .values()
        .filter(|n| n.author_id() == Some("sitzung.effort") && n.toggled().is_some())
        .map(|n| {
            (
                n.bounds().map_or(0.0, |b| b.x0),
                n.label().unwrap_or_default().to_owned(),
                n.toggled() == Some(Toggled::True),
            )
        })
        .collect();
    wahl.sort_by(|a, b| a.0.total_cmp(&b.0));
    let wahl: Vec<(String, bool)> = wahl.into_iter().map(|(_, t, an)| (t, an)).collect();
    assert_eq!(
        wahl,
        [
            (w::VORGABE_DES_PROGRAMMS.to_owned(), false),
            (w::AUS.to_owned(), false),
            ("low".to_owned(), false),
            ("medium".to_owned(), true),
        ]
    );
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
