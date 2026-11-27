//! The operating paths of section 2 of the surface part of contract `seite` (B01 to B80) and the rule which of them
//! a view shows (OF7, OF8), outside of Dioxus, so that the window can take it over.
//!
//! The table and the rule [`sichtbar`] have the meaning of the rule of the app window (OF14): scope P, N or E, the gaps
//! L1 to L11 of section 4, carriers. The browser view adds the base forms that the contract `seite` asks for
//! with the messages of today (column "Grundform", [`im_browser`]). A test reads section 2 from the contract file and compares it
//! with this table, so the two cannot drift.

/// Scope of an operating path (section 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Umfang {
    /// Pilot.
    P,
    /// After the pilot.
    N,
    /// Dropped.
    E,
}

/// One operating path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bedienweg {
    /// The identifier `Bnn`, also the element id in the accessibility tree.
    pub nr: &'static str,
    /// Its name in section 2.
    pub name: &'static str,
    /// Its scope.
    pub umfang: Umfang,
    /// The gaps of section 4 it needs (`L1` to `L11` as 1 to 11).
    pub luecken: &'static [u8],
    /// Column "Grundform" of section 2: it shows before its gaps in the form the contract `seite` asks for today
    /// (SE2, SE3, SE6, SE7); only the parts that need the gap stay away until it is closed (OF8 point 3).
    pub grundform: bool,
}

const fn w(
    nr: &'static str,
    name: &'static str,
    umfang: Umfang,
    luecken: &'static [u8],
) -> Bedienweg {
    Bedienweg {
        nr,
        name,
        umfang,
        luecken,
        grundform: false,
    }
}

/// An operating path with a base form (column "Grundform" of section 2).
const fn g(
    nr: &'static str,
    name: &'static str,
    umfang: Umfang,
    luecken: &'static [u8],
) -> Bedienweg {
    Bedienweg {
        grundform: true,
        ..w(nr, name, umfang, luecken)
    }
}

/// The 69 operating paths of section 2, in its order.
pub const BEDIENWEGE: [Bedienweg; 69] = [
    w("B01", "Umschalter Code / Agents", Umfang::N, &[]),
    g(
        "B02",
        "Titelleiste: Projektname mit Herkunft",
        Umfang::P,
        &[5],
    ),
    w("B03", "Freigabe-Abzeichen", Umfang::P, &[]),
    g("B04", "Inspektor ein- und ausklappen", Umfang::P, &[8]),
    w(
        "B05",
        "Warnleiste „Verbindung zum Kern fehlt“",
        Umfang::P,
        &[],
    ),
    w("B06", "Freigabeleiste", Umfang::E, &[]),
    w(
        "B07",
        "Meldung „Freigabe erteilt/abgelehnt“",
        Umfang::P,
        &[6],
    ),
    g("B08", "Kopf der Leiste mit „+“", Umfang::P, &[]),
    w("B09", "Projektzeile", Umfang::P, &[5]),
    g("B10", "Sitzungszeile", Umfang::P, &[1, 5]),
    w("B11", "„Fortsetzen“", Umfang::P, &[3]),
    w("B12", "„Namen ändern …“", Umfang::P, &[3]),
    w("B14", "„Ordner im Finder zeigen“", Umfang::N, &[]),
    w("B15", "„Sitzung schließen“", Umfang::P, &[]),
    w("B16", "„Endgültig löschen …“", Umfang::P, &[3]),
    w("B17", "„Wiederbeleben …“", Umfang::P, &[3]),
    w("B18", "Gesprächszeile in der Leiste", Umfang::E, &[]),
    w("B19", "Umsortieren durch Ziehen", Umfang::P, &[8]),
    w("B20", "Maschinenkarte", Umfang::N, &[]),
    w("B21", "„heute“ im Fuß der Leiste", Umfang::N, &[]),
    w("B22", "Zahl laufender Worker", Umfang::P, &[1]),
    g("B23", "Zahnrad im Fuß der Leiste", Umfang::P, &[]),
    w("B24", "Breite der Leiste ziehen", Umfang::P, &[8]),
    w("B25", "Inhaltskopf", Umfang::E, &[]),
    w("B26", "Statuspille „N laufen“", Umfang::P, &[1]),
    w("B27", "Orchestrator / Worker", Umfang::P, &[1, 2]),
    w("B28", "Zahnrad in der Titelleiste", Umfang::P, &[4]),
    w("B29", "Worker-Liste", Umfang::P, &[1]),
    w("B30", "Worker-Tabs", Umfang::P, &[2]),
    w("B31", "Übersicht / Gekachelt / Gezoomt", Umfang::P, &[2]),
    w("B32", "Ordnen", Umfang::P, &[2]),
    w("B33", "Hinweis zur Kachelform", Umfang::P, &[2]),
    w("B34", "Leerzustand", Umfang::P, &[]),
    w("B35", "Terminalbild", Umfang::P, &[]),
    w("B36", "Tastatur im Terminal", Umfang::P, &[]),
    w("B37", "Eingabezeile mit „Senden“", Umfang::N, &[]),
    w("B38", "Kopieren und Einfügen", Umfang::P, &[]),
    g("B39", "Größe an die Fläche anpassen", Umfang::P, &[2]),
    w("B40", "Rückblick", Umfang::N, &[]),
    g("B41", "Pane-Kopf", Umfang::P, &[1]),
    w("B42", "Dialoge der Harnesses", Umfang::P, &[]),
    g("B51", "Reiter des Inspektors", Umfang::P, &[]),
    w("B52", "Ordner-Blatt", Umfang::P, &[9]),
    w("B53", "Inhaltssuche", Umfang::P, &[9]),
    w("B54", "Aktivitäts-Blatt", Umfang::N, &[]),
    w("B55", "Protokolle-Blatt", Umfang::N, &[]),
    g("B56", "Freigaben-Blatt", Umfang::P, &[6]),
    w("B57", "Breite des Inspektors ziehen", Umfang::P, &[8]),
    w("B58", "Editor-Tab-Zeile", Umfang::N, &[]),
    w("B59", "Datei öffnen", Umfang::N, &[]),
    w("B60", "Datei speichern", Umfang::N, &[]),
    w("B61", "An Orchestrator senden", Umfang::N, &[]),
    w("B62", "Schnellöffner Cmd+P", Umfang::N, &[]),
    w("B63", "Diff-Tab und Auftrag-Tab", Umfang::N, &[]),
    w("B64", "Editor ein- und ausklappen", Umfang::N, &[]),
    w("B66", "Meldung „Ergebnis da“", Umfang::N, &[]),
    w("B67", "Kurzmeldung", Umfang::P, &[]),
    g("B68", "Fenster „Sitzungen“", Umfang::P, &[3, 5, 10]),
    g("B69", "Modell und Effort beim Start", Umfang::P, &[10]),
    w("B70", "Frühere Sitzungen", Umfang::E, &[]),
    g("B71", "Meldung bei abgelehntem Start", Umfang::P, &[]),
    g("B72", "Einstellungen des Nutzers", Umfang::P, &[4]),
    w("B73", "Verbrauchsansicht", Umfang::N, &[]),
    w("B75", "Anwendungsmenü", Umfang::P, &[]),
    w("B76", "Escape schließt Fenster und Blase", Umfang::P, &[]),
    w("B77", "Farbwahl", Umfang::P, &[]),
    w("B78", "Admin-Einstellungen", Umfang::P, &[4]),
    w("B79", "Umschalter Chat / Terminal", Umfang::N, &[]),
    w("B80", "Leiste ein- und ausblenden", Umfang::P, &[8]),
];

/// Gap L8 (widths, inspector, order of the sessions) needs no message: the window keeps it in a file of the user.
pub const LUECKE_MERKEN: u8 = 8;

/// The set of gaps of section 4 that a path to the service has closed (bit `n` for `Ln`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Luecken(u16);

impl Luecken {
    /// No gap closed: the messages and events of today's contract `dienst`.
    pub const KEINE: Luecken = Luecken(0);
    /// Every gap L1 to L11 closed: the target state the mock-up shows.
    pub const ALLE: Luecken = Luecken(0b1111_1111_1110);

    /// Whether gap `n` is closed.
    #[must_use]
    pub fn hat(self, n: u8) -> bool {
        n == LUECKE_MERKEN || (n < 16 && self.0 & (1 << n) != 0)
    }

    /// This set with gap `n` closed.
    #[must_use]
    pub fn mit(self, n: u8) -> Luecken {
        if n < 16 {
            Luecken(self.0 | (1 << n))
        } else {
            self
        }
    }
}

/// Operating paths of scope P that the browser view does not build, with the reason.
pub const FEHLT: [(&str, &str); 1] = [(
    "B75",
    "in der Browser-Ansicht kein eigener Knopf (OBERFLAECHE.md Abschnitt 2, B75)",
)];

/// Operating paths that live inside another one: they show only when one of their carriers shows (the menu of a
/// session row, the window "Sitzungen", the settings, the worker view, the inspector). The same list as in the window.
const TRAEGER: [(&str, &[&str]); 21] = [
    ("B03", &["B56"]),
    ("B04", &["B51"]),
    ("B07", &["B56"]),
    ("B08", &["B68"]),
    ("B11", &["B10"]),
    ("B12", &["B10"]),
    ("B15", &["B10"]),
    ("B16", &["B10"]),
    ("B17", &["B10"]),
    ("B19", &["B10"]),
    ("B23", &["B72"]),
    ("B28", &["B72"]),
    ("B29", &["B26"]),
    ("B41", &["B27"]),
    ("B51", &["B52", "B56"]),
    ("B53", &["B52"]),
    ("B57", &["B51"]),
    ("B69", &["B68"]),
    ("B71", &["B68"]),
    ("B76", &["B68", "B29", "B10", "B72"]),
    ("B77", &["B72"]),
];

/// The carriers of operating path `nr` (empty when it lives on its own).
#[must_use]
pub fn traeger(nr: &str) -> &'static [&'static str] {
    TRAEGER
        .iter()
        .find(|(n, _)| *n == nr)
        .map_or(&[], |(_, t)| t)
}

/// The operating path `nr`, if section 2 has it.
#[must_use]
pub fn finde(nr: &str) -> Option<&'static Bedienweg> {
    BEDIENWEGE.iter().find(|b| b.nr == nr)
}

/// Whether an operating path shows with the gaps `geschlossen` closed (OF7, OF8), with the meaning of the window: scope P,
/// every gap it needs closed, built, and at least one of its carriers shown. Paths of scope N or E never show.
#[must_use]
pub fn sichtbar(nr: &str, geschlossen: Luecken) -> bool {
    tief(nr, geschlossen, 0, false)
}

/// Every operating path that shows with these gaps closed, in the order of section 2.
#[must_use]
pub fn alle_sichtbaren(geschlossen: Luecken) -> Vec<&'static str> {
    BEDIENWEGE
        .iter()
        .filter(|b| sichtbar(b.nr, geschlossen))
        .map(|b| b.nr)
        .collect()
}

/// Whether the browser view shows the operating path `nr` with the gaps `geschlossen` closed: the rule of [`sichtbar`], and
/// besides it the base form of the paths that have one ([`Bedienweg::grundform`]).
#[must_use]
pub fn im_browser(nr: &str, geschlossen: Luecken) -> bool {
    tief(nr, geschlossen, 0, true)
}

/// Every operating path the browser view shows with these gaps closed, in the order of section 2.
#[must_use]
pub fn alle_im_browser(geschlossen: Luecken) -> Vec<&'static str> {
    BEDIENWEGE
        .iter()
        .filter(|b| im_browser(b.nr, geschlossen))
        .map(|b| b.nr)
        .collect()
}

fn tief(nr: &str, geschlossen: Luecken, tiefe: u8, browser: bool) -> bool {
    let Some(b) = finde(nr) else {
        return false;
    };
    if b.umfang != Umfang::P || FEHLT.iter().any(|(f, _)| *f == nr) || tiefe > 8 {
        return false;
    }
    if browser && b.grundform {
        return true;
    }
    if !b.luecken.iter().all(|l| geschlossen.hat(*l)) {
        return false;
    }
    match TRAEGER.iter().find(|(n, _)| *n == nr) {
        Some((_, traeger)) => traeger
            .iter()
            .any(|t| tief(t, geschlossen, tiefe + 1, browser)),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_counts_like_section_2() {
        let zahl = |u| BEDIENWEGE.iter().filter(|b| b.umfang == u).count();
        assert_eq!(
            (zahl(Umfang::P), zahl(Umfang::N), zahl(Umfang::E)),
            (47, 18, 4)
        );
        assert_eq!(BEDIENWEGE.iter().filter(|b| b.grundform).count(), 13);
    }

    #[test]
    fn the_rule_has_the_meaning_of_the_window() {
        // the same expectations as the tests of the window
        assert_eq!(
            alle_sichtbaren(Luecken::KEINE),
            [
                "B05", "B24", "B34", "B35", "B36", "B38", "B42", "B67", "B80"
            ]
        );
        let alle = alle_sichtbaren(Luecken::ALLE);
        assert_eq!(alle.len(), 47 - FEHLT.len());
        assert!(!alle.contains(&"B75"));
    }

    #[test]
    fn the_browser_shows_the_base_forms_today_and_everything_in_the_target_state() {
        assert_eq!(
            alle_im_browser(Luecken::KEINE),
            [
                "B02", "B03", "B04", "B05", "B08", "B10", "B15", "B19", "B23", "B24", "B34", "B35",
                "B36", "B38", "B39", "B41", "B42", "B51", "B56", "B57", "B67", "B68", "B69", "B71",
                "B72", "B76", "B77", "B80"
            ]
        );
        assert_eq!(
            alle_im_browser(Luecken::ALLE),
            alle_sichtbaren(Luecken::ALLE)
        );
        // B28 needs L4 (section 2); the menu of a session row (L3) and the pill (L1) follow their gaps
        assert!(!im_browser("B28", Luecken::KEINE.mit(1)));
        assert!(im_browser("B28", Luecken::KEINE.mit(4)));
        assert!(im_browser("B11", Luecken::KEINE.mit(3)));
        assert!(im_browser("B29", Luecken::KEINE.mit(1)));
        assert!(!im_browser("B30", Luecken::KEINE.mit(1)));
    }

    /// Section 2 of the contract file, as the runner of the window reads it: number, scope, gaps, base form.
    fn abschnitt_2() -> Vec<(String, String, Vec<u8>, bool)> {
        let pfad = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../spec/seite/OBERFLAECHE.md"
        );
        let text = std::fs::read_to_string(pfad).expect("OBERFLAECHE.md lesbar");
        let start = text.find("## 2. Bedienwege").expect("Abschnitt 2");
        let ende = text[start..].find("Zählung").expect("Zählung") + start;
        text[start..ende]
            .lines()
            .filter(|z| z.starts_with("| B"))
            .map(|z| {
                let teile: Vec<&str> = z.split('|').map(str::trim).collect();
                let luecken = teile[5]
                    .split(',')
                    .filter_map(|l| l.trim().strip_prefix('L').and_then(|n| n.parse().ok()))
                    .collect();
                (
                    teile[1].to_owned(),
                    teile[3].to_owned(),
                    luecken,
                    teile[6] == "ja",
                )
            })
            .collect()
    }

    #[test]
    fn the_table_of_the_view_equals_section_2() {
        let spec = abschnitt_2();
        assert_eq!(spec.len(), 69, "Abschnitt 2 hat 69 Bedienwege");
        let tabelle: Vec<(String, String, Vec<u8>, bool)> = BEDIENWEGE
            .iter()
            .map(|b| {
                let u = match b.umfang {
                    Umfang::P => "P",
                    Umfang::N => "N",
                    Umfang::E => "E",
                };
                (
                    b.nr.to_owned(),
                    u.to_owned(),
                    b.luecken.to_vec(),
                    b.grundform,
                )
            })
            .collect();
        assert_eq!(tabelle, spec);
    }
}
