//! The application menu (B75): one list of entries for both forms. Under Windows and Linux the window draws it as the
//! menu in the title row; under macOS it is the menu bar of the system, built with `muda` from the same list
//! ([`Menueleiste`]). The list holds only entries whose operating path shows (OF8); [`crate::fenster::Fenster`] builds it
//! and carries out the chosen entry.

/// What an entry of the menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tat {
    /// Open the settings (B72).
    Einstellungen,
    /// Close the window.
    Beenden,
    /// Open the window "Sitzungen" (B68).
    NeueSitzung,
    /// Copy the marked text of the terminal (B38).
    Kopieren,
    /// Paste into the terminal (B38).
    Einfuegen,
    /// Continue the chosen session (B11).
    Fortsetzen,
    /// Rename the chosen session (B12).
    Umbenennen,
    /// Close the chosen session (B15).
    Schliessen,
    /// Delete the chosen session (B16).
    Loeschen,
    /// Show or hide the sidebar (B80).
    Leiste,
    /// Show or hide the inspector (B04).
    Inspektor,
    /// The view "Orchestrator" (B27).
    Orchestrator,
    /// The view "Worker" (B27).
    Worker,
    /// Show or hide the worker list (B29).
    WorkerListe,
}

impl Tat {
    /// Every entry, for the ids of the menu bar.
    pub const ALLE: [Tat; 14] = [
        Tat::Einstellungen,
        Tat::Beenden,
        Tat::NeueSitzung,
        Tat::Kopieren,
        Tat::Einfuegen,
        Tat::Fortsetzen,
        Tat::Umbenennen,
        Tat::Schliessen,
        Tat::Loeschen,
        Tat::Leiste,
        Tat::Inspektor,
        Tat::Orchestrator,
        Tat::Worker,
        Tat::WorkerListe,
    ];

    /// The id of the entry in the menu bar.
    #[must_use]
    pub fn kennung(self) -> String {
        format!("werkbank-{self:?}")
    }

    /// The entry with this id of the menu bar.
    #[must_use]
    pub fn aus_kennung(k: &str) -> Option<Tat> {
        Tat::ALLE.into_iter().find(|t| t.kennung() == k)
    }
}

/// One entry of a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    /// What it does.
    pub tat: Tat,
    /// Its text.
    pub text: &'static str,
    /// Whether it can be chosen now.
    pub an: bool,
    /// The operating path it belongs to; the entry is in the list only while that path shows (OF8).
    pub weg: &'static str,
}

/// One menu of the menu bar with its entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Untermenue {
    /// The title of the menu.
    pub titel: &'static str,
    /// Its entries, never empty.
    pub eintraege: Vec<Eintrag>,
}

/// The menu bar of macOS (B75), built with `muda` from the list of the window. It is rebuilt when the list changes;
/// the chosen entries come back through a channel, and each choice wakes the window.
#[cfg(target_os = "macos")]
pub struct Menueleiste {
    menu: Option<muda::Menu>,
    stand: Vec<Untermenue>,
    gewaehlt: std::sync::mpsc::Receiver<String>,
}

#[cfg(target_os = "macos")]
impl Menueleiste {
    /// A menu bar for the window of `ctx`. Only call it once the application exists (in the creator of eframe).
    #[must_use]
    pub fn neu(ctx: &egui::Context) -> Menueleiste {
        let (tx, rx) = std::sync::mpsc::channel();
        let c = ctx.clone();
        // a sender in a mutex, so that the handler is Sync on every toolchain
        let tx = std::sync::Mutex::new(tx);
        muda::MenuEvent::set_event_handler(Some(std::sync::Arc::new(move |e: muda::MenuEvent| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(e.id().0.clone());
            }
            c.request_repaint();
        })));
        Menueleiste {
            menu: None,
            stand: Vec::new(),
            gewaehlt: rx,
        }
    }

    /// Brings the menu bar to the list `soll` and returns the entries chosen since the last call.
    pub fn abgleichen(&mut self, soll: &[Untermenue]) -> Vec<Tat> {
        if self.menu.is_none() || self.stand != soll {
            match bauen(soll) {
                Ok(m) => {
                    m.init_for_nsapp();
                    self.menu = Some(m);
                    self.stand = soll.to_vec();
                }
                Err(e) => {
                    use std::io::Write;
                    let _ = writeln!(
                        std::io::stderr(),
                        "werkbank-fenster: Menüleiste nicht gebaut: {e}"
                    );
                    // the same list does not try again every frame
                    self.stand = soll.to_vec();
                }
            }
        }
        self.gewaehlt
            .try_iter()
            .filter_map(|k| Tat::aus_kennung(&k))
            .collect()
    }
}

/// The title of the application menu, as in [`crate::fenster::Fenster::anwendungsmenue`].
#[cfg(target_os = "macos")]
const WERKBANK: &str = "Werkbank";

/// The fixed entries of the application menu of macOS (B75): the system entries of the menu bar. They need no operating
/// path of a page and work without one; that is in line with OF8. Their order follows the convention of macOS: the
/// entries of the window come first, then Services, then the hiding entries, then Quit.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fest {
    /// A separator line.
    Trenner,
    /// The submenu Services of the system.
    Dienste,
    /// Hide the window, Command+H.
    Ausblenden,
    /// Hide the other applications.
    AndereAusblenden,
    /// Show all windows of the application.
    AlleEinblenden,
    /// Quit, Command+Q. The system ends the program here, as the button of the title row does: the remembered values
    /// are written at exit (`Fenster::on_exit`).
    Beenden,
}

#[cfg(target_os = "macos")]
impl Fest {
    /// The text of the entry, `None` for a separator.
    #[must_use]
    pub fn text(self) -> Option<&'static str> {
        match self {
            Fest::Trenner => None,
            Fest::Dienste => Some("Dienste"),
            Fest::Ausblenden => Some("Werkbank ausblenden"),
            Fest::AndereAusblenden => Some("Andere ausblenden"),
            Fest::AlleEinblenden => Some("Alle einblenden"),
            Fest::Beenden => Some("Werkbank beenden"),
        }
    }
}

/// The fixed entries in their order, after the entries of the window. Without entries before them, the first separator
/// is left out.
#[cfg(target_os = "macos")]
#[must_use]
pub fn feste_reihe(vorher: bool) -> Vec<Fest> {
    use Fest::{AlleEinblenden, AndereAusblenden, Ausblenden, Beenden, Dienste, Trenner};
    let mut reihe = vec![
        Trenner,
        Dienste,
        Trenner,
        Ausblenden,
        AndereAusblenden,
        AlleEinblenden,
        Trenner,
        Beenden,
    ];
    if !vorher {
        reihe.remove(0);
    }
    reihe
}

/// The menu of `muda` for the list. The first menu, "Werkbank", is the application menu of macOS: it holds the entries of
/// the window and then the fixed entries of the system. The entry "Werkbank beenden" of the list is left out there, because
/// the fixed entry Quit replaces it (B75); Windows and Linux keep it in the title row. Every other key stays with the
/// window, and the system adds only the key equivalents of its own entries (Command+H, Command+Q; SE1 point 8).
#[cfg(target_os = "macos")]
fn bauen(liste: &[Untermenue]) -> muda::Result<muda::Menu> {
    use muda::{MenuItem, PredefinedMenuItem, Submenu};
    let menu = muda::Menu::new();
    let (eigene, weitere) = match liste.first() {
        Some(m) if m.titel == WERKBANK => (Some(m), &liste[1..]),
        _ => (None, liste),
    };
    let app = Submenu::new(WERKBANK, true);
    let mut vorher = false;
    for e in eigene
        .into_iter()
        .flat_map(|m| &m.eintraege)
        .filter(|e| e.tat != Tat::Beenden)
    {
        app.append(&MenuItem::with_id(e.tat.kennung(), e.text, e.an, None))?;
        vorher = true;
    }
    // Fixed entries (B75, OF8): the system carries out each of them itself. They need no page and no operating path,
    // so they work without one; the list above is the only part that depends on a page.
    for f in feste_reihe(vorher) {
        match f {
            Fest::Trenner => app.append(&PredefinedMenuItem::separator())?,
            Fest::Dienste => app.append(&PredefinedMenuItem::services(f.text()))?,
            Fest::Ausblenden => app.append(&PredefinedMenuItem::hide(f.text()))?,
            Fest::AndereAusblenden => app.append(&PredefinedMenuItem::hide_others(f.text()))?,
            Fest::AlleEinblenden => app.append(&PredefinedMenuItem::show_all(f.text()))?,
            Fest::Beenden => app.append(&PredefinedMenuItem::quit(f.text()))?,
        }
    }
    menu.append(&app)?;
    for m in weitere {
        let sub = Submenu::new(m.titel, true);
        for e in &m.eintraege {
            sub.append(&MenuItem::with_id(e.tat.kennung(), e.text, e.an, None))?;
        }
        menu.append(&sub)?;
    }
    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_has_its_own_id() {
        for t in Tat::ALLE {
            assert_eq!(Tat::aus_kennung(&t.kennung()), Some(t));
        }
        assert_eq!(Tat::aus_kennung("werkbank-Nichts"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_fixed_entries_keep_the_order_of_macos() {
        use Fest::{AlleEinblenden, AndereAusblenden, Ausblenden, Beenden, Dienste, Trenner};
        assert_eq!(
            feste_reihe(true),
            [
                Trenner,
                Dienste,
                Trenner,
                Ausblenden,
                AndereAusblenden,
                AlleEinblenden,
                Trenner,
                Beenden
            ]
        );
        assert_eq!(feste_reihe(false).first(), Some(&Dienste));
        assert_eq!(Trenner.text(), None);
        assert_eq!(Ausblenden.text(), Some("Werkbank ausblenden"));
        assert_eq!(AndereAusblenden.text(), Some("Andere ausblenden"));
        assert_eq!(AlleEinblenden.text(), Some("Alle einblenden"));
        assert_eq!(Dienste.text(), Some("Dienste"));
        assert_eq!(Beenden.text(), Some("Werkbank beenden"));
    }
}
