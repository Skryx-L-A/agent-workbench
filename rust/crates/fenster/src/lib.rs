//! App window of the pilot (contract `seite`, part surface, OF14; decision of the operator E5, 26.11.2026): the
//! terminal view with the operating paths of scope pilot, in egui.
//!
//! - [`fenster`]: draws, reads the view core and sends the events of the view.
//! - [`weg`]: the narrow interface to the service; [`attrappe`] stands behind it until the local path (gap L11) exists.
//! - [`bedienwege`]: the table of section 2 and the rule which operating path shows (OF7, OF8).
//! - [`gestalt`]: every measure and role colour (OF2, OF3). [`merken`]: what the window keeps per device (L8).
//! - [`tasten`], [`terminal`]: keyboard and terminal picture. [`ziel`]: the fields of the gaps, read only for the mock-up.
//! - [`bild`]: pictures without a window (OF13).
//!
//! The logic of the sessions lives in the view core of the crate `ansicht` ([`ansicht::kern::Kern`]), the same the
//! browser view uses; the window has no second copy of it.

pub mod attrappe;
pub mod bedienwege;
pub mod bild;
pub mod fenster;
pub mod gestalt;
pub mod menue;
pub mod merken;
pub mod symbole;
pub mod tasten;
pub mod terminal;
pub mod weg;
pub mod ziel;

use std::sync::Arc;

use egui::{FontFamily, FontId, RichText, TextStyle};

use crate::attrappe::{Attrappe, Form, Griff};
use crate::fenster::{Fenster, Reiter, Seite, Szene};
use crate::gestalt::MASSE;
use crate::merken::Merken;

/// The notice of the files taken into this crate (README.md, section "Code taken from other projects");
/// `werkbank-fenster --licenses` prints it.
pub const NOTICE: &str = include_str!("../NOTICE");

/// The family of the bold cut: Ubuntu at weight 700, then the fonts of egui for the characters it lacks.
pub const FETT: &str = "fett";

/// The font file of the bold cut, `Ubuntu[wdth,wght].ttf` of the Ubuntu package `fonts-ubuntu` (Ubuntu-font-1.0,
/// origin and checksums in `NOTICE`).
///
/// OF4 point 1: this is a fallback until the window loads the font of the system. The fonts of egui have no bold cut,
/// so without it every bold text of the window would be regular. Once the window takes the font of the system, this
/// file and its row in README.md go.
const UBUNTU: &[u8] = include_bytes!("../schriften/Ubuntu-wdth-wght.ttf");

/// The fonts of the window: those of egui and the bold family [`FETT`].
#[must_use]
pub fn schriften() -> egui::FontDefinitions {
    let mut d = egui::FontDefinitions::default();
    let mut fett = egui::FontData::from_static(UBUNTU);
    fett.tweak.coords = egui::epaint::text::VariationCoords::new([(b"wght", 700.0)]);
    d.font_data.insert("Ubuntu-Bold".to_owned(), Arc::new(fett));
    let mut familie = vec!["Ubuntu-Bold".to_owned()];
    familie.extend(
        d.families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    d.families.insert(FontFamily::Name(FETT.into()), familie);
    d
}

/// Bold text in the window: the bold cut and the strong colour of egui.
pub trait Fett {
    /// This text in the bold cut.
    #[must_use]
    fn fett(self) -> Self;
}

impl Fett for RichText {
    fn fett(self) -> Self {
        self.strong().family(FontFamily::Name(FETT.into()))
    }
}

/// Sets the fonts, text sizes and spacing of the window (OF4) on a context.
pub fn einrichten(ctx: &egui::Context) {
    ctx.set_fonts(schriften());
    ctx.all_styles_mut(|s| {
        s.text_styles
            .insert(TextStyle::Body, FontId::proportional(MASSE.schrift_grund));
        s.text_styles
            .insert(TextStyle::Button, FontId::proportional(MASSE.schrift_grund));
        s.text_styles
            .insert(TextStyle::Small, FontId::proportional(MASSE.schrift_klein));
        s.text_styles.insert(
            TextStyle::Heading,
            FontId::proportional(MASSE.schrift_titel + 3.0),
        );
        s.text_styles.insert(
            TextStyle::Monospace,
            FontId::monospace(MASSE.schrift_terminal),
        );
        s.spacing.item_spacing = egui::vec2(MASSE.abstand, MASSE.abstand);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.spacing.interact_size.y = 26.0;
    });
}

/// A window on a new mock-up of form `form`, with the defaults of [`Merken`] and no file.
#[must_use]
pub fn mit_attrappe(form: Form) -> (Fenster, Griff) {
    let (a, griff) = Attrappe::neu(form);
    (Fenster::neu(Box::new(a), Merken::default(), None), griff)
}

/// The pictures of OF13: name, situation, whether the viewer is an admin.
#[must_use]
pub fn szenen() -> Vec<(&'static str, Szene, bool)> {
    vec![
        ("orchestrator-inspektor-zu", Szene::OrchestratorZu, false),
        (
            "orchestrator-freigaben",
            Szene::OrchestratorOffen(Reiter::Freigaben),
            false,
        ),
        (
            "orchestrator-ordner",
            Szene::OrchestratorOffen(Reiter::Ordner),
            false,
        ),
        ("worker-kacheln", Szene::WorkerKacheln, false),
        ("worker-liste", Szene::WorkerListe, false),
        ("sitzungen-leer", Szene::SitzungenLeer, false),
        (
            "sitzungen-beendet",
            Szene::SitzungenGewaehlt("s-3".to_owned()),
            false,
        ),
        (
            "einstellungen-aussehen-nutzer",
            Szene::Einstellungen(Seite::Aussehen),
            false,
        ),
        (
            "einstellungen-sitzung-nutzer",
            Szene::Einstellungen(Seite::Sitzung),
            false,
        ),
        (
            "einstellungen-harnesses-nutzer",
            Szene::Einstellungen(Seite::Harnesses),
            false,
        ),
        (
            "einstellungen-aussehen-admin",
            Szene::Einstellungen(Seite::Aussehen),
            true,
        ),
        (
            "einstellungen-sitzung-admin",
            Szene::Einstellungen(Seite::Sitzung),
            true,
        ),
        (
            "einstellungen-harnesses-admin",
            Szene::Einstellungen(Seite::Harnesses),
            true,
        ),
    ]
}

/// Paints every picture of OF13 at 1280 by 800 points, light and dark, from the target state of the mock-up, and
/// writes them as `<name>-<hell|dunkel>.png` into `ordner`. Returns the files written.
///
/// # Errors
///
/// Every error of the file system.
pub fn bilder(ordner: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    std::fs::create_dir_all(ordner)?;
    let mut aus = Vec::new();
    for (name, szene, admin) in szenen() {
        for (thema, dunkel) in [("hell", false), ("dunkel", true)] {
            let ctx = egui::Context::default();
            einrichten(&ctx);
            let (mut f, griff) = mit_attrappe(Form::Ziel);
            griff.borrow_mut().admin_setzen(admin);
            // first frames take the stream and subscribe, then the situation
            let mut lw = bild::Leinwand::neu(1280, 800, egui::Color32::BLACK);
            bild::aufnehmen(&ctx, &mut f, &mut lw, 3, dunkel);
            f.szene(&szene);
            bild::aufnehmen(&ctx, &mut f, &mut lw, 16, dunkel);
            let pfad = ordner.join(format!("{name}-{thema}.png"));
            std::fs::write(&pfad, lw.png())?;
            aus.push(pfad);
        }
    }
    Ok(aus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bold_cut_is_wider_than_the_regular_one() {
        let ctx = egui::Context::default();
        einrichten(&ctx);
        let mut breite = [0.0_f32; 2];
        let mut aus = ctx.run_ui(egui::RawInput::default(), |ui| {
            for (i, familie) in [FontFamily::Proportional, FontFamily::Name(FETT.into())]
                .into_iter()
                .enumerate()
            {
                let g = ui.painter().layout_no_wrap(
                    "Werkbank Sitzung".to_owned(),
                    FontId::new(14.0, familie),
                    egui::Color32::WHITE,
                );
                breite[i] = g.size().x;
            }
        });
        aus.textures_delta.clear();
        // Ubuntu Light of egui against Ubuntu at weight 700: the bold cut is loaded and its axis takes effect
        assert!(breite[1] > breite[0] * 1.05, "{breite:?}");
    }

    #[test]
    fn the_notice_names_the_font_and_its_licence() {
        assert!(NOTICE.contains("schriften/Ubuntu-wdth-wght.ttf"));
        assert!(NOTICE.contains("Ubuntu-font-1.0"));
        assert!(NOTICE.contains("UBUNTU FONT LICENCE Version 1.0"));
        assert!(NOTICE.contains("Copyright: 2010–2022 Canonical Ltd"));
    }
}
