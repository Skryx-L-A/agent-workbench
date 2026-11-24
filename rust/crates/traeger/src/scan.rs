//! TR5 to TR9: the own scanner of the control sequences (`vte::Parser`, not a text search).
//!
//! The grid of `alacritty_terminal` does not report these things to us, so the bytes are read a second time here. The scanner
//! only collects what it saw; the screen part decides the order against the output lines.

use alacritty_terminal::vte::{Params, Perform};

/// What the scanner saw in the bytes.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Gesehen {
    /// DEC mode 5 set (`true`) or reset.
    Invers(bool),
    /// DEC mode 2004 set (`true`) or reset.
    Klammer(bool),
    /// DEC mode 2026 set.
    SyncBeginn,
    /// DEC mode 2026 reset.
    SyncEnde,
    /// OSC 0 or OSC 2.
    Titel(String),
    /// OSC 52 with data; `None` is the read request `?`.
    Zwischen(Option<String>),
    /// `ESC [ 8 ; rows ; cols t` as (columns, rows).
    GroessenFolge(u32, u32),
    /// The cursor query `ESC [ 6 n` (A109). It is no event: the screen part answers it and reports nothing for it.
    Cursorabfrage,
}

/// Collects the sequences the contract names, in the order of the bytes.
#[derive(Default)]
pub(crate) struct Scan {
    /// What was seen since the last `nehmen`.
    gesehen: Vec<Gesehen>,
}

impl Scan {
    /// Hands over what was seen so far and starts empty again.
    pub(crate) fn nehmen(&mut self) -> Vec<Gesehen> {
        std::mem::take(&mut self.gesehen)
    }
}

impl Perform for Scan {
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore {
            return;
        }
        // A109: exactly the queries `alacritty_terminal` takes for a cursor query (`device_status(6)`: final `n`, no
        // intermediates, first parameter 6, so also `ESC[06n`, `ESC[6;5n` and `ESC[6:1n`; not `ESC[?6n`). The same set,
        // so that the answer the grid drops (`raster.rs`, `Zuhoerer`) and the answer the carrier gives match one to one.
        if action == 'n'
            && intermediates.is_empty()
            && params.iter().next().and_then(|p| p.first().copied()) == Some(6)
        {
            self.gesehen.push(Gesehen::Cursorabfrage);
            return;
        }
        if (action == 'h' || action == 'l') && intermediates == *b"?" {
            let an = action == 'h';
            for parameter in params {
                match parameter.first().copied() {
                    Some(5) => self.gesehen.push(Gesehen::Invers(an)),
                    Some(2004) => self.gesehen.push(Gesehen::Klammer(an)),
                    Some(2026) => self.gesehen.push(if an {
                        Gesehen::SyncBeginn
                    } else {
                        Gesehen::SyncEnde
                    }),
                    _ => {}
                }
            }
        } else if action == 't' && intermediates.is_empty() {
            // SECURITY (TR9): a size sequence is only reported here. Nothing in this crate resizes the grid or the PTY
            // because of it; only the explicit `Raster::groesse` and `Kind::groesse` calls do, and the core refuses the
            // sequence (`groesse_nur_kern`).
            let teile: Vec<&[u16]> = params.iter().collect();
            if let [[8], [zeilen], [spalten]] = teile[..] {
                self.gesehen.push(Gesehen::GroessenFolge(
                    u32::from(*spalten),
                    u32::from(*zeilen),
                ));
            }
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let [nummer, rest @ ..] = params else {
            return;
        };
        if rest.is_empty() {
            return;
        }
        match *nummer {
            b"0" | b"2" => {
                let teile: Vec<String> = rest
                    .iter()
                    .map(|teil| String::from_utf8_lossy(teil).into_owned())
                    .collect();
                self.gesehen.push(Gesehen::Titel(teile.join(";")));
            }
            b"52" => {
                // SECURITY (TR7): the data go out exactly as they came (never decoded), and a read request (`?`) is only
                // reported. It is never answered: nothing is written to the PTY and the real clipboard is not touched.
                if let [_auswahl, daten] = rest {
                    self.gesehen.push(Gesehen::Zwischen(if *daten == b"?" {
                        None
                    } else {
                        Some(String::from_utf8_lossy(daten).into_owned())
                    }));
                }
            }
            _ => {}
        }
    }
}
