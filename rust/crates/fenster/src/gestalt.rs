//! The style module of the window (contract `seite`, part surface, OF2 point 2 and OF3): every measure and every role
//! colour of the window stands here, and nowhere else.
//!
//! The measures and the colours of grey dark and light are measured at the reference pictures
//! (`the previous implementation/*.png`; Retina, two pixels are one point). The table with picture, pixel coordinates and value
//! of every number stands in the result of the measuring (`masse.md`). Where a measured colour breaks OF3 point 5, its
//! lightness is moved until every pair holds; hue and saturation stay. Blue-black is OF3 point 3, the light terminal
//! OF3 point 4.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// A colour of three bytes.
const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// The measures of the window in points (OF2 point 2, OF2 point 3, OF4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Masse {
    /// Height of the title row without its line (OF1 point 2).
    pub titelleiste: f32,
    /// Width of a line.
    pub linie: f32,
    /// Width of the sidebar at the start, flush up to the middle.
    pub leiste: f32,
    /// Smallest and largest width of the sidebar (OF2 point 3).
    pub leiste_grenzen: (f32, f32),
    /// Width of the inspector at the start.
    pub inspektor: f32,
    /// Smallest and largest width of the inspector (OF2 point 3).
    pub inspektor_grenzen: (f32, f32),
    /// The smallest width of the middle (OF2 point 3).
    pub mitte_min: f32,
    /// Height of a session row (B10).
    pub sitzungszeile: f32,
    /// Height of a project row (B09); the name stands in its middle.
    pub projektzeile: f32,
    /// Space above a project row.
    pub gruppenabstand: f32,
    /// Distance of the state dot from the edge of the chosen area of a session row.
    pub zeile_punkt: f32,
    /// Distance of the text from the edge of the chosen area of a session row.
    pub zeile_text: f32,
    /// Indent of the name of a project row from the edge of the sidebar.
    pub kopf_einzug: f32,
    /// Height of the head of a pane, focus frame included (B41).
    pub pane_kopf: f32,
    /// Space between the tiles of the worker view.
    pub kachelabstand: f32,
    /// Height of the head of the sidebar (B08).
    pub leistenkopf: f32,
    /// Height of the foot of the sidebar (B22, B23).
    pub leistenfuss: f32,
    /// Height of a capsule in the title row (B26, B27, B04, B28).
    pub kapsel: f32,
    /// Height of the chosen segment inside a capsule.
    pub kapsel_segment: f32,
    /// Space between two capsules.
    pub abstand_kapsel: f32,
    /// Space before the capsule of inspector and gear.
    pub abstand_kapsel_rechts: f32,
    /// Indent of the project name in the title row from the start of the middle.
    pub titel_einzug: f32,
    /// Height of the row of tabs of the inspector.
    pub inspektor_kopf: f32,
    /// Width and height of a tab of the inspector.
    pub reiter: (f32, f32),
    /// Height of a button.
    pub knopf: f32,
    /// Height of a search field.
    pub feld: f32,
    /// Height of a field in a form.
    pub formfeld: f32,
    /// Height of a row of the pages of the settings.
    pub seitenzeile: f32,
    /// Space between the heading of a group and the group.
    pub gruppe_kopf: f32,
    /// Largest width of a page of the settings.
    pub einstellungsseite: f32,
    /// Width of the list of pages of the settings.
    pub einstellungsleiste: f32,
    /// Radius of a capsule: half its height.
    pub radius_kapsel: f32,
    /// Radius of buttons.
    pub radius_knopf: f32,
    /// Radius of fields and of the choice inside a segmented field.
    pub radius_feld: f32,
    /// Radius of the chosen row.
    pub radius_wahl: f32,
    /// Radius of a group.
    pub radius_gruppe: f32,
    /// Radius of a tab of the inspector.
    pub radius_reiter: f32,
    /// Radius of windows and bubbles.
    pub radius_fenster: f32,
    /// Inner margin of the areas.
    pub innen: f32,
    /// Space between elements.
    pub abstand: f32,
    /// Font size of the project name in the title row.
    pub schrift_titel: f32,
    /// Font size of the base text.
    pub schrift_grund: f32,
    /// Font size of the small extra line.
    pub schrift_klein: f32,
    /// Font size of the text in a capsule.
    pub schrift_kapsel: f32,
    /// Font size of the title of a page of the settings.
    pub schrift_seite: f32,
    /// Font size of the terminal (OF4 point 2; it can be chosen in the settings).
    pub schrift_terminal: f32,
    /// Width of the focus frame (SE2 point 6: at least 2).
    pub fokusrahmen: f32,
    /// Diameter of a state dot.
    pub punkt: f32,
    /// Panes per tab of the worker view (B30, G7).
    pub kacheln_je_tab: usize,
    /// Seconds a short note stays (B67).
    pub kurzmeldung_s: f64,
    /// Width and height of the window "Sitzungen" (B68).
    pub sitzungsfenster: (f32, f32),
    /// Width and height of the settings (B72, B78).
    pub einstellungsfenster: (f32, f32),
}

/// The measures (OF2 point 2), measured at the reference pictures.
pub const MASSE: Masse = Masse {
    titelleiste: 51.0,
    linie: 1.0,
    leiste: 240.0,
    leiste_grenzen: (180.0, 400.0),
    inspektor: 300.0,
    inspektor_grenzen: (260.0, 560.0),
    mitte_min: 480.0,
    sitzungszeile: 43.0,
    projektzeile: 34.0,
    gruppenabstand: 12.0,
    zeile_punkt: 15.0,
    zeile_text: 36.0,
    kopf_einzug: 14.0,
    pane_kopf: 24.0,
    kachelabstand: 4.0,
    leistenkopf: 34.0,
    leistenfuss: 34.0,
    kapsel: 36.0,
    kapsel_segment: 28.0,
    abstand_kapsel: 8.0,
    abstand_kapsel_rechts: 18.0,
    titel_einzug: 20.0,
    inspektor_kopf: 36.0,
    reiter: (26.0, 24.0),
    knopf: 24.0,
    feld: 24.0,
    formfeld: 26.0,
    seitenzeile: 32.0,
    gruppe_kopf: 14.0,
    einstellungsseite: 704.0,
    einstellungsleiste: 223.0,
    radius_kapsel: 18.0,
    radius_knopf: 6.0,
    radius_feld: 7.0,
    radius_wahl: 8.0,
    radius_gruppe: 12.0,
    radius_reiter: 5.0,
    radius_fenster: 12.0,
    innen: 10.0,
    abstand: 6.0,
    schrift_titel: 13.5,
    schrift_grund: 13.0,
    schrift_klein: 11.0,
    schrift_kapsel: 14.0,
    schrift_seite: 18.0,
    schrift_terminal: 13.0,
    fokusrahmen: 2.0,
    punkt: 8.0,
    kacheln_je_tab: 6,
    kurzmeldung_s: 4.0,
    sitzungsfenster: (900.0, 652.0),
    einstellungsfenster: (1120.0, 760.0),
};

/// The role colours of one colour set (OF3 point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rollen {
    /// The window and the title row.
    pub fenster: Color32,
    /// The ground of the middle and of windows.
    pub grund: Color32,
    /// The ground of the sidebar and of the capsules of the title row.
    pub leiste: Color32,
    /// Raised surfaces: groups, fields, bubbles.
    pub erhoben: Color32,
    /// Lines, and the head of a pane without focus.
    pub linie: Color32,
    /// Text.
    pub schrift: Color32,
    /// Muted text.
    pub gedaempft: Color32,
    /// The chosen row and the chosen segment.
    pub wahl: Color32,
    /// The ground of the inspector.
    pub inspektor: Color32,
    /// The row of tabs of the inspector.
    pub inspektor_kopf: Color32,
    /// The chosen tab of the inspector.
    pub reiter_wahl: Color32,
    /// The head of a pane without focus.
    pub kopf: Color32,
    /// The head of the pane with the focus.
    pub kopf_aktiv: Color32,
    /// Accent: focus frame and lines.
    pub akzent: Color32,
    /// Accent as a surface with white ink.
    pub akzent_flaeche: Color32,
    /// Ink on the accent surface.
    pub akzent_tinte: Color32,
    /// Accent as text.
    pub akzent_text: Color32,
    /// State running.
    pub laeuft: Color32,
    /// State waiting.
    pub wartet: Color32,
    /// The waiting colour as a surface (the badge of an open request, B03).
    pub wartet_flaeche: Color32,
    /// Ink on the waiting surface.
    pub wartet_tinte: Color32,
    /// State done.
    pub fertig: Color32,
    /// State dead or rejected.
    pub tot: Color32,
    /// Whether the set is dark.
    pub dunkel: bool,
}

/// Light (OF3 point 1), measured.
pub const HELL: Rollen = Rollen {
    fenster: rgb(0xff, 0xff, 0xff),
    grund: rgb(0xff, 0xff, 0xff),
    leiste: rgb(0xf7, 0xf7, 0xf7),
    erhoben: rgb(0xf7, 0xf7, 0xf7),
    linie: rgb(0xe6, 0xe6, 0xe6),
    schrift: rgb(0x26, 0x26, 0x26),
    gedaempft: rgb(0x61, 0x61, 0x61),
    wahl: rgb(0xdc, 0xdc, 0xdc),
    inspektor: rgb(0xf9, 0xf9, 0xf9),
    inspektor_kopf: rgb(0xf4, 0xf4, 0xf4),
    reiter_wahl: rgb(0xcc, 0xcc, 0xcc),
    kopf: rgb(0xe6, 0xe6, 0xe6),
    kopf_aktiv: rgb(0xda, 0xe7, 0xfd),
    akzent: rgb(0x34, 0x78, 0xf6),
    akzent_flaeche: rgb(0x25, 0x6e, 0xf5),
    akzent_tinte: rgb(0xff, 0xff, 0xff),
    akzent_text: rgb(0x0a, 0x5a, 0xed),
    laeuft: rgb(0x24, 0x78, 0xf5),
    wartet: rgb(0xa2, 0x73, 0x25),
    wartet_flaeche: rgb(0xd7, 0xa5, 0x51),
    wartet_tinte: rgb(0x1a, 0x13, 0x00),
    fertig: rgb(0x4d, 0x89, 0x5c),
    tot: rgb(0xb5, 0x61, 0x5e),
    dunkel: false,
};

/// Grey dark, the default of the dark theme (OF3 point 1: neutral grey without a blue cast), measured.
pub const GRAU_DUNKEL: Rollen = Rollen {
    fenster: rgb(0x1f, 0x20, 0x20),
    grund: rgb(0x1f, 0x20, 0x20),
    leiste: rgb(0x29, 0x2a, 0x2a),
    erhoben: rgb(0x26, 0x26, 0x27),
    linie: rgb(0x35, 0x36, 0x36),
    schrift: rgb(0xde, 0xde, 0xde),
    gedaempft: rgb(0xb7, 0xb7, 0xb7),
    wahl: rgb(0x46, 0x46, 0x46),
    inspektor: rgb(0x48, 0x49, 0x49),
    inspektor_kopf: rgb(0x2f, 0x30, 0x30),
    reiter_wahl: rgb(0x59, 0x59, 0x59),
    kopf: rgb(0x35, 0x36, 0x36),
    kopf_aktiv: rgb(0x23, 0x30, 0x47),
    akzent: rgb(0x5e, 0x94, 0xf8),
    akzent_flaeche: rgb(0x25, 0x6e, 0xf5),
    akzent_tinte: rgb(0xff, 0xff, 0xff),
    akzent_text: rgb(0x60, 0x95, 0xf8),
    laeuft: rgb(0x63, 0x9f, 0xf8),
    wartet: rgb(0xd7, 0xa5, 0x51),
    wartet_flaeche: rgb(0xd7, 0xa5, 0x51),
    wartet_tinte: rgb(0x1a, 0x13, 0x00),
    fertig: rgb(0x5d, 0xa4, 0x6f),
    tot: rgb(0xc6, 0x85, 0x83),
    dunkel: true,
};

/// Blue-black (OF3 point 3): the role colours of the version of 23.11.2026; accent and state colours as there. The roles
/// the version of 23.11.2026 does not name follow the steps of grey dark: inspector and pane head on `erhoben`, the row
/// of tabs on `leiste`, the head with focus 15 % accent over `grund`.
pub const BLAUSCHWARZ: Rollen = Rollen {
    fenster: rgb(0x0b, 0x0d, 0x10),
    grund: rgb(0x10, 0x12, 0x16),
    leiste: rgb(0x17, 0x1a, 0x20),
    erhoben: rgb(0x1e, 0x23, 0x2b),
    linie: rgb(0x26, 0x2b, 0x34),
    schrift: rgb(0xd8, 0xde, 0xe9),
    gedaempft: rgb(0x8b, 0x93, 0xa1),
    wahl: rgb(0x22, 0x28, 0x33),
    inspektor: rgb(0x1e, 0x23, 0x2b),
    inspektor_kopf: rgb(0x17, 0x1a, 0x20),
    reiter_wahl: rgb(0x26, 0x2b, 0x34),
    kopf: rgb(0x1e, 0x23, 0x2b),
    kopf_aktiv: rgb(0x0e, 0x20, 0x34),
    akzent: rgb(0x00, 0x70, 0xe0),
    akzent_flaeche: rgb(0x00, 0x70, 0xe0),
    akzent_tinte: rgb(0xff, 0xff, 0xff),
    akzent_text: rgb(0x33, 0x98, 0xff),
    laeuft: rgb(0x4e, 0xa1, 0xff),
    wartet: rgb(0xe0, 0xa3, 0x3e),
    wartet_flaeche: rgb(0xe0, 0xa3, 0x3e),
    wartet_tinte: rgb(0x1a, 0x13, 0x00),
    fertig: rgb(0x3f, 0xa6, 0x6a),
    tot: rgb(0xcd, 0x79, 0x79),
    dunkel: true,
};

/// The colours of a terminal: ground, text, cursor and the 16 named colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terminalfarben {
    /// Ground.
    pub grund: Color32,
    /// Default text.
    pub schrift: Color32,
    /// The marked area (B38).
    pub markierung: Color32,
    /// The 16 named colours (0 to 7, bright 8 to 15).
    pub palette: [Color32; 16],
}

/// The dark terminal, default in both themes (OF3 point 1). Ground and text are measured; the palette keeps hue and
/// saturation of the version of 23.11.2026 and has at least 4.5:1 on the ground (colours 1, 4, 5, 8, 9 are lighter).
pub const TERMINAL_DUNKEL: Terminalfarben = Terminalfarben {
    grund: rgb(0x1e, 0x1e, 0x1e),
    schrift: rgb(0xff, 0xff, 0xff),
    markierung: Color32::from_rgba_premultiplied(0x1c, 0x3c, 0x66, 0xa0),
    palette: [
        rgb(0x2e, 0x34, 0x36),
        rgb(0xff, 0x2e, 0x2e),
        rgb(0x4e, 0x9a, 0x06),
        rgb(0xc4, 0xa0, 0x00),
        rgb(0x54, 0x87, 0xc9),
        rgb(0xa1, 0x78, 0xa7),
        rgb(0x06, 0x98, 0x9a),
        rgb(0xd3, 0xd7, 0xcf),
        rgb(0x83, 0x86, 0x80),
        rgb(0xf1, 0x45, 0x45),
        rgb(0x8a, 0xe2, 0x34),
        rgb(0xfc, 0xe9, 0x4f),
        rgb(0x72, 0x9f, 0xcf),
        rgb(0xad, 0x7f, 0xa8),
        rgb(0x34, 0xe2, 0xe2),
        rgb(0xee, 0xee, 0xec),
    ],
};

/// The light terminal (OF3 point 4): the palette of the version of 23.11.2026, ground `erhoben` of light.
pub const TERMINAL_HELL: Terminalfarben = Terminalfarben {
    grund: HELL.erhoben,
    schrift: HELL.schrift,
    markierung: Color32::from_rgba_premultiplied(0x55, 0x8c, 0xd8, 0x70),
    palette: [
        rgb(0x2e, 0x34, 0x36),
        rgb(0xcc, 0x00, 0x00),
        rgb(0x3a, 0x73, 0x04),
        rgb(0x72, 0x5d, 0x00),
        rgb(0x34, 0x65, 0xa4),
        rgb(0x75, 0x50, 0x7b),
        rgb(0x04, 0x71, 0x73),
        rgb(0x62, 0x68, 0x61),
        rgb(0x55, 0x57, 0x53),
        rgb(0xcc, 0x0f, 0x0f),
        rgb(0x43, 0x76, 0x11),
        rgb(0x69, 0x5d, 0x02),
        rgb(0x38, 0x6c, 0xa3),
        rgb(0x85, 0x55, 0x80),
        rgb(0x11, 0x76, 0x76),
        rgb(0x1b, 0x1f, 0x27),
    ],
};

/// Theme (OF3 point 2): like the system (default), light or dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Thema {
    /// Like the system.
    #[default]
    System,
    /// Light.
    Hell,
    /// Dark.
    Dunkel,
}

/// Colour set of the dark theme (OF3 point 2): grey (default) or blue-black.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Farbsatz {
    /// Grey.
    #[default]
    Grau,
    /// Blue-black.
    Blauschwarz,
}

/// Terminal (OF3 point 2): dark (default), light or like the theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Terminalwahl {
    /// Dark.
    #[default]
    Dunkel,
    /// Light.
    Hell,
    /// Like the theme.
    WieThema,
}

/// The three choices of the page "Aussehen" (B77), freely mixed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wahl {
    /// Theme.
    pub thema: Thema,
    /// Colour set of the dark theme.
    pub farbsatz: Farbsatz,
    /// Terminal.
    pub terminal: Terminalwahl,
}

impl Wahl {
    /// The role colours for this choice; `system_dunkel` is the theme of the system.
    #[must_use]
    pub fn rollen(&self, system_dunkel: bool) -> &'static Rollen {
        let dunkel = match self.thema {
            Thema::System => system_dunkel,
            Thema::Hell => false,
            Thema::Dunkel => true,
        };
        match (dunkel, self.farbsatz) {
            (false, _) => &HELL,
            (true, Farbsatz::Grau) => &GRAU_DUNKEL,
            (true, Farbsatz::Blauschwarz) => &BLAUSCHWARZ,
        }
    }

    /// The terminal colours for this choice.
    #[must_use]
    pub fn terminal(&self, system_dunkel: bool) -> &'static Terminalfarben {
        match self.terminal {
            Terminalwahl::Dunkel => &TERMINAL_DUNKEL,
            Terminalwahl::Hell => &TERMINAL_HELL,
            Terminalwahl::WieThema => {
                if self.rollen(system_dunkel).dunkel {
                    &TERMINAL_DUNKEL
                } else {
                    &TERMINAL_HELL
                }
            }
        }
    }
}

/// The egui look of a colour set: flat areas without joints (OF2 point 1), radii and focus frame from [`MASSE`].
#[must_use]
pub fn visuals(r: &Rollen) -> Visuals {
    let mut v = if r.dunkel {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.override_text_color = Some(r.schrift);
    v.panel_fill = r.grund;
    v.window_fill = r.grund;
    v.extreme_bg_color = r.grund;
    v.text_edit_bg_color = Some(r.grund);
    v.faint_bg_color = r.leiste;
    v.code_bg_color = r.erhoben;
    v.window_stroke = Stroke::new(MASSE.linie, r.linie);
    v.window_corner_radius = radius(MASSE.radius_fenster);
    v.menu_corner_radius = radius(MASSE.radius_fenster);
    v.hyperlink_color = r.akzent_text;
    v.warn_fg_color = r.wartet;
    v.error_fg_color = r.tot;
    // a chosen entry is grey like the reference, its text stays the text colour
    v.selection.bg_fill = r.wahl;
    v.selection.stroke = Stroke::new(MASSE.fokusrahmen, r.schrift);
    v.text_cursor.stroke = Stroke::new(MASSE.fokusrahmen, r.akzent);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius(MASSE.radius_knopf);
        w.fg_stroke.color = r.schrift;
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(MASSE.linie, r.linie);
    v.widgets.noninteractive.bg_fill = r.grund;
    v.widgets.noninteractive.fg_stroke.color = r.gedaempft;
    v.widgets.inactive.bg_fill = r.leiste;
    v.widgets.inactive.weak_bg_fill = r.erhoben;
    v.widgets.inactive.bg_stroke = Stroke::new(MASSE.linie, r.linie);
    v.widgets.hovered.bg_fill = r.wahl;
    v.widgets.hovered.weak_bg_fill = r.wahl;
    v.widgets.hovered.bg_stroke = Stroke::new(MASSE.linie, r.linie);
    v.widgets.active.bg_fill = r.wahl;
    v.widgets.active.weak_bg_fill = r.wahl;
    v.widgets.active.bg_stroke = Stroke::new(MASSE.linie, r.linie);
    v.widgets.open.weak_bg_fill = r.wahl;
    v.widgets.open.bg_stroke = Stroke::new(MASSE.linie, r.linie);
    v
}

/// The text sizes and spacing of egui after [`MASSE`] (OF4 point 1).
pub fn stil(s: &mut egui::Style) {
    use egui::{FontFamily, FontId, TextStyle};
    let p = |g| FontId::new(g, FontFamily::Proportional);
    s.text_styles
        .insert(TextStyle::Body, p(MASSE.schrift_grund));
    s.text_styles
        .insert(TextStyle::Button, p(MASSE.schrift_grund));
    s.text_styles
        .insert(TextStyle::Small, p(MASSE.schrift_klein));
    s.text_styles
        .insert(TextStyle::Heading, p(MASSE.schrift_seite));
    s.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(MASSE.schrift_klein, FontFamily::Monospace),
    );
    s.spacing.interact_size.y = MASSE.knopf;
    s.spacing.button_padding = egui::vec2(10.0, 3.0);
    s.spacing.item_spacing = egui::vec2(MASSE.abstand + 2.0, MASSE.abstand - 2.0);
}

/// A radius of [`MASSE`] as egui wants it.
#[must_use]
pub fn radius(x: f32) -> CornerRadius {
    // the radii are small whole numbers of points
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    CornerRadius::same(x.round() as u8)
}

/// The relative luminance of a colour (WCAG 2.x).
#[must_use]
pub fn leuchtdichte(c: Color32) -> f64 {
    let lin = |u: u8| {
        let s = f64::from(u) / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// The contrast ratio of two colours (WCAG 2.x), at least 1.
#[must_use]
pub fn kontrast(a: Color32, b: Color32) -> f64 {
    let (x, y) = (leuchtdichte(a), leuchtdichte(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The colour of a terminal style colour (`rasterauszug` `Stil`: 0 default, 1 to 256 palette, `0x0100_0000 | rgb`).
#[must_use]
pub fn stilfarbe(t: &Terminalfarben, wert: u32, vorgabe: Color32) -> Color32 {
    match wert {
        0 => vorgabe,
        1..=16 => t.palette[(wert - 1) as usize],
        17..=232 => {
            // the 6x6x6 cube of xterm
            let i = wert - 17;
            let stufe = |n: u32| {
                // each step is at most 255
                #[allow(clippy::cast_possible_truncation)]
                if n == 0 { 0 } else { (55 + n * 40) as u8 }
            };
            Color32::from_rgb(stufe(i / 36), stufe((i / 6) % 6), stufe(i % 6))
        }
        233..=256 => {
            // the grey ramp of xterm
            #[allow(clippy::cast_possible_truncation)]
            let g = (8 + (wert - 233) * 10) as u8;
            Color32::from_rgb(g, g, g)
        }
        _ => {
            let [_, r, g, b] = wert.to_be_bytes();
            rgb(r, g, b)
        }
    }
}

/// One contrast pair: name, ink, ground, the least ratio and whether OF3 point 5 binds it.
pub type Kontrastpaar = (String, Color32, Color32, f64, bool);

/// The contrast pairs of OF3 point 5 for one combination: text on every ground it stands on (4.5:1), state dots and the
/// focus frame on every ground (3:1), and the terminal. The palette of the terminal is bound too (4.5:1); colour 0 is a
/// ground colour of programs and is listed only.
#[must_use]
pub fn kontrastpaare(r: &Rollen, t: &Terminalfarben) -> Vec<Kontrastpaar> {
    let gruende = [
        ("grund", r.grund),
        ("leiste", r.leiste),
        ("erhoben", r.erhoben),
        ("wahl", r.wahl),
        ("inspektor", r.inspektor),
        ("inspektor_kopf", r.inspektor_kopf),
        ("reiter_wahl", r.reiter_wahl),
        ("kopf", r.kopf),
        ("kopf_aktiv", r.kopf_aktiv),
    ];
    let mut v: Vec<Kontrastpaar> = Vec::new();
    let name = |a: &str, b: &str| format!("{a}/{b}");
    for (g, c) in gruende {
        v.push((name("schrift", g), r.schrift, c, 4.5, true));
        // the chosen tab and the chosen segment carry only text
        if g == "reiter_wahl" {
            continue;
        }
        v.push((name("gedaempft", g), r.gedaempft, c, 4.5, true));
        for (z, f) in [
            ("laeuft", r.laeuft),
            ("wartet", r.wartet),
            ("fertig", r.fertig),
            ("tot", r.tot),
        ] {
            v.push((name(z, g), f, c, 3.0, true));
        }
    }
    for (g, c) in [
        ("grund", r.grund),
        ("leiste", r.leiste),
        ("erhoben", r.erhoben),
        ("kopf_aktiv", r.kopf_aktiv),
    ] {
        v.push((name("akzent_text", g), r.akzent_text, c, 4.5, true));
    }
    for (g, c) in [
        ("grund", r.grund),
        ("leiste", r.leiste),
        ("erhoben", r.erhoben),
        ("inspektor", r.inspektor),
    ] {
        v.push((name("akzent", g), r.akzent, c, 3.0, true));
    }
    v.push((
        "akzent_tinte/akzent_flaeche".to_owned(),
        r.akzent_tinte,
        r.akzent_flaeche,
        4.5,
        true,
    ));
    v.push((
        "wartet_tinte/wartet_flaeche".to_owned(),
        r.wartet_tinte,
        r.wartet_flaeche,
        4.5,
        true,
    ));
    v.push((
        "terminal schrift/grund".to_owned(),
        t.schrift,
        t.grund,
        4.5,
        true,
    ));
    v.push((
        "terminal palette 0/grund".to_owned(),
        t.palette[0],
        t.grund,
        1.0,
        false,
    ));
    v.extend(
        t.palette
            .iter()
            .skip(1)
            .map(|c| ("terminal palette/grund".to_owned(), *c, t.grund, 4.5, true)),
    );
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_combination_keeps_the_contrast_of_of3() {
        let mut rot = Vec::new();
        let mut zeilen = vec![
            "| Kombination | Paar | Kontrast | Mindestens |".to_owned(),
            "|---|---|---|---|".to_owned(),
        ];
        for thema in [Thema::Hell, Thema::Dunkel] {
            for farbsatz in [Farbsatz::Grau, Farbsatz::Blauschwarz] {
                for terminal in [
                    Terminalwahl::Dunkel,
                    Terminalwahl::Hell,
                    Terminalwahl::WieThema,
                ] {
                    let w = Wahl {
                        thema,
                        farbsatz,
                        terminal,
                    };
                    for (n, a, b, min, pflicht) in kontrastpaare(w.rollen(true), w.terminal(true)) {
                        let k = kontrast(a, b);
                        let marke = if pflicht { "" } else { " (Hinweis)" };
                        zeilen.push(format!(
                            "| {thema:?}/{farbsatz:?}/{terminal:?} | {n} {:02x}{:02x}{:02x} | {k:.2} | {min}{marke} |",
                            a.r(),
                            a.g(),
                            a.b()
                        ));
                        if pflicht && k < min {
                            rot.push(format!(
                                "{thema:?}/{farbsatz:?}/{terminal:?} {n}: {k:.2} < {min}"
                            ));
                        }
                    }
                }
            }
        }
        if let Some(p) = std::env::var_os("FENSTER_KONTRAST") {
            let _ = std::fs::write(p, zeilen.join("\n"));
        }
        assert!(rot.is_empty(), "{rot:#?}");
    }
}
