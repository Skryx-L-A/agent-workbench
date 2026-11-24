//! Announcements for the screen reader (contract `seite` SE3 points 3 and 4): the output of the terminal is
//! collected; at most one output announcement per 1000 ms of the page clock and at most the last 5 lines each.

use std::collections::BTreeMap;

/// The quiet time after an output announcement, in milliseconds.
pub const SPERRZEIT_MS: f64 = 1000.0;
/// The most lines in one output announcement.
pub const HOECHSTENS_ZEILEN: usize = 5;

/// What the announcer wants after new lines.
#[derive(Clone, Debug, PartialEq)]
pub enum Ausgabe {
    /// Nothing now.
    Nichts,
    /// Announce this text now.
    Sofort(String),
    /// Call [`Ansager::wecken`] after this many milliseconds.
    Wecker(f64),
}

/// The collector of output lines.
#[derive(Debug)]
pub struct Ansager {
    sperre_bis: f64,
    sammlung: BTreeMap<usize, String>,
    wecker: bool,
}

impl Default for Ansager {
    fn default() -> Self {
        Ansager {
            sperre_bis: f64::NEG_INFINITY,
            sammlung: BTreeMap::new(),
            wecker: false,
        }
    }
}

impl Ansager {
    /// New lines of the output (index and text) at the time `jetzt` (milliseconds of the page clock).
    pub fn zeilen(&mut self, neu: Vec<(usize, String)>, jetzt: f64) -> Ausgabe {
        self.sammlung.extend(neu);
        if self.sammlung.is_empty() {
            return Ausgabe::Nichts;
        }
        if jetzt >= self.sperre_bis {
            return self.ausgabe(jetzt).map_or(Ausgabe::Nichts, Ausgabe::Sofort);
        }
        if self.wecker {
            return Ausgabe::Nichts;
        }
        self.wecker = true;
        Ausgabe::Wecker(self.sperre_bis - jetzt)
    }

    /// The alarm set by [`Ausgabe::Wecker`] went off.
    pub fn wecken(&mut self, jetzt: f64) -> Option<String> {
        self.wecker = false;
        self.ausgabe(jetzt)
    }

    fn ausgabe(&mut self, jetzt: f64) -> Option<String> {
        if self.sammlung.is_empty() {
            return None;
        }
        let alle: Vec<String> = std::mem::take(&mut self.sammlung).into_values().collect();
        let von = alle.len().saturating_sub(HOECHSTENS_ZEILEN);
        self.sperre_bis = jetzt + SPERRZEIT_MS;
        Some(alle[von..].join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_lines_go_at_once_then_the_quiet_time_collects() {
        let mut a = Ansager::default();
        assert_eq!(
            a.zeilen(vec![(1, "eins".into())], 0.0),
            Ausgabe::Sofort("eins".into())
        );
        assert_eq!(
            a.zeilen(vec![(2, "zwei".into())], 100.0),
            Ausgabe::Wecker(900.0)
        );
        assert_eq!(a.zeilen(vec![(3, "drei".into())], 200.0), Ausgabe::Nichts);
        assert_eq!(a.wecken(1000.0), Some("zwei\ndrei".into()));
        assert_eq!(a.zeilen(vec![], 3000.0), Ausgabe::Nichts);
    }

    #[test]
    fn at_most_five_lines_by_index() {
        let mut a = Ansager::default();
        let neu = (0..8).rev().map(|i| (i, format!("z{i}"))).collect();
        assert_eq!(
            a.zeilen(neu, 0.0),
            Ausgabe::Sofort("z3\nz4\nz5\nz6\nz7".into())
        );
    }
}
