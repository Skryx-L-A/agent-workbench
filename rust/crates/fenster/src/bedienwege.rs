//! The rule which operating paths of section 2 the window shows (OF7, OF8), over the table that the window shares with the
//! browser view (`ansicht::bedienwege`). Only the rule is the window's own: the application menu B75 stands in the title
//! row under Windows and Linux, and a base form shows before its gaps (OF8 point 3).
//!
//! The runner of the window (`tests/laeufer.rs`) reads section 2 from the contract file and compares it with the shared
//! table; a test below compares this rule with the rule of the browser view, so the two cannot drift.

pub use ansicht::bedienwege::{
    BEDIENWEGE, Bedienweg, LUECKE_MERKEN, Luecken, Umfang, finde, traeger,
};

/// Operating paths of scope P that the window does not build, with the reason. Under Windows and Linux the application
/// menu stands in the title row; under macOS it would be the menu bar of the system.
#[cfg(target_os = "macos")]
pub const FEHLT: [(&str, &str); 1] = [(
    "B75",
    "die Menüleiste von macOS braucht muda; muda 0.22 bringt die Doppelversion keyboard-types 0.7 und 0.8",
)];
/// Operating paths of scope P that the window does not build, with the reason. Under Windows and Linux the application
/// menu stands in the title row (B75).
#[cfg(not(target_os = "macos"))]
pub const FEHLT: [(&str, &str); 0] = [];

/// Whether an operating path shows with the gaps `geschlossen` closed (OF7, OF8): scope P, built, every gap it needs
/// closed and at least one of its carriers shown. A base form shows before its gaps and without its carriers; only its
/// parts from the gaps stay away (OF8 point 3). Paths of scope N or E never show.
#[must_use]
pub fn sichtbar(nr: &str, geschlossen: Luecken) -> bool {
    sichtbar_tief(nr, geschlossen, 0)
}

fn sichtbar_tief(nr: &str, geschlossen: Luecken, tiefe: u8) -> bool {
    let Some(b) = finde(nr) else {
        return false;
    };
    if b.umfang != Umfang::P || FEHLT.iter().any(|(f, _)| *f == nr) || tiefe > 8 {
        return false;
    }
    // the base form (OF8 point 3): the window shows it before its gaps, like the browser view
    if b.grundform {
        return true;
    }
    if !b.luecken.iter().all(|l| geschlossen.hat(*l)) {
        return false;
    }
    match traeger(nr) {
        [] => true,
        t => t.iter().any(|t| sichtbar_tief(t, geschlossen, tiefe + 1)),
    }
}

/// Every operating path the window shows with these gaps closed, in the order of section 2.
#[must_use]
pub fn alle_sichtbaren(geschlossen: Luecken) -> Vec<&'static str> {
    BEDIENWEGE
        .iter()
        .filter(|b| sichtbar(b.nr, geschlossen))
        .map(|b| b.nr)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_table_counts_like_section_2() {
        let zahl = |u| BEDIENWEGE.iter().filter(|b| b.umfang == u).count();
        assert_eq!(
            (zahl(Umfang::P), zahl(Umfang::N), zahl(Umfang::E)),
            (47, 18, 4)
        );
        assert_eq!(BEDIENWEGE.iter().filter(|b| b.grundform).count(), 13);
    }

    #[test]
    fn the_target_state_shows_every_pilot_path_that_is_built() {
        let alle = alle_sichtbaren(Luecken::ALLE);
        assert_eq!(alle.len(), 47 - FEHLT.len());
        assert_eq!(alle.contains(&"B75"), !cfg!(target_os = "macos"));
    }

    #[test]
    fn today_the_paths_without_a_gap_and_the_base_forms_show() {
        let heute = alle_sichtbaren(Luecken::KEINE);
        // the paths without a gap, the base forms and what they carry (OF8 point 3)
        let mut erwartet = vec![
            "B02", "B03", "B04", "B05", "B08", "B10", "B15", "B19", "B23", "B24", "B34", "B35",
            "B36", "B38", "B39", "B41", "B42", "B51", "B56", "B57", "B67", "B68", "B69", "B71",
            "B72", "B75", "B76", "B77", "B80",
        ];
        if cfg!(target_os = "macos") {
            erwartet.retain(|n| *n != "B75");
        }
        assert_eq!(heute, erwartet);
    }

    #[test]
    fn a_base_form_shows_before_its_gap() {
        // B10 needs L1 and L5 and has a base form: choosing a session works today
        assert!(sichtbar("B10", Luecken::KEINE));
        // B09 needs L5 and has none: it stays away until L5 is closed
        assert!(!sichtbar("B09", Luecken::KEINE));
        assert!(sichtbar("B09", Luecken::KEINE.mit(5)));
        // B68 has a base form, so "+" (B08, carried by B68) and the rejection (B71) show: starting works today
        assert!(
            sichtbar("B68", Luecken::KEINE)
                && sichtbar("B08", Luecken::KEINE)
                && sichtbar("B71", Luecken::KEINE)
        );
        // B41 shows over the orchestrator although its carrier (the worker view B27, L1 and L2) does not
        assert!(sichtbar("B41", Luecken::KEINE) && !sichtbar("B27", Luecken::KEINE));
        // B11 sits in the menu of B10 but needs L3 and has no base form
        assert!(!sichtbar("B11", Luecken::KEINE));
        // B28 needs L4 now; B80 needs only L8, which the window keeps itself
        assert!(!sichtbar("B28", Luecken::KEINE) && sichtbar("B80", Luecken::KEINE));
    }

    #[test]
    fn the_rule_equals_the_rule_of_the_browser_view_except_the_menu() {
        // the browser view has no own button for B75; everything else shows in both views alike
        for g in [Luecken::KEINE, Luecken::ALLE, Luecken::KEINE.mit(1).mit(5)] {
            for b in BEDIENWEGE.iter().filter(|b| b.nr != "B75") {
                assert_eq!(
                    sichtbar(b.nr, g),
                    ansicht::bedienwege::im_browser(b.nr, g),
                    "{} mit {g:?}",
                    b.nr
                );
            }
        }
    }
}
