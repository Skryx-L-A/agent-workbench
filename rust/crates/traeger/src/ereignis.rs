//! The events the carrier hands to the state core (contract section 4), without `sitzung` and `t`.

use serde_json::{Map, Value, json};

/// One event of the carrier. The service adds the session and the time before the core sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ereignis {
    /// The lines and the cursor that differ from what the core was told last: row index and line in the canonical SGR form.
    Ausgabe {
        /// Counter shared with [`Ereignis::Invers`]; starts at 1 and rises by 1 with each event that carries it.
        stand: u64,
        /// Row index (from 0) and the new line, in row order.
        zeilen: Vec<(usize, String)>,
        /// The cursor in every `Ausgabe`: row, column (both on the visible screen, from 0) and visibility (TR2).
        cursor: (usize, usize, bool),
    },
    /// DEC mode 5 (reverse video) was set or reset.
    Invers {
        /// `true` for `ESC [ ? 5 h`, `false` for `ESC [ ? 5 l`.
        an: bool,
        /// The shared counter, see [`Ereignis::Ausgabe`].
        stand: u64,
    },
    /// DEC mode 2004 (bracketed paste) was set or reset.
    KlammerPaste {
        /// `true` for `h`, `false` for `l`.
        an: bool,
    },
    /// `ESC [ ? 2026 h`: a synchronized update begins.
    SyncBeginn,
    /// `ESC [ ? 2026 l`: a synchronized update ends.
    SyncEnde,
    /// OSC 52 with data: the text exactly as it came, never decoded.
    Zwischenablage {
        /// The third part of the sequence, unchanged.
        daten_b64: String,
    },
    /// OSC 52 with the data `?`: a read request. The carrier never answers it.
    ZwischenablageLesen,
    /// OSC 0 or OSC 2.
    Titel {
        /// The title text; invalid UTF-8 is replaced by U+FFFD.
        text: String,
    },
    /// `ESC [ 8 ; rows ; cols t`: reported, never executed.
    GroessenFolge {
        /// Requested columns (at most 65535).
        spalten: u32,
        /// Requested rows (at most 65535).
        zeilen: u32,
    },
    /// The child ended, after its last output.
    Kindende {
        /// The exit code of the child.
        code: u32,
    },
}

impl Ereignis {
    /// The event as the JSON object of the contract (section 4).
    #[must_use]
    pub fn zu_json(&self) -> Value {
        match self {
            Self::Ausgabe {
                stand,
                zeilen,
                cursor: (zeile, spalte, sichtbar),
            } => {
                let mut karte = Map::new();
                for (index, zeile) in zeilen {
                    karte.insert(index.to_string(), json!(zeile));
                }
                json!({"art": "Ausgabe", "stand": stand, "zeilen": Value::Object(karte),
                       "cursor": [zeile, spalte, sichtbar]})
            }
            Self::Invers { an, stand } => json!({"art": "Modus", "invers": an, "stand": stand}),
            Self::KlammerPaste { an } => json!({"art": "Modus", "bracketed_paste": an}),
            Self::SyncBeginn => json!({"art": "SyncBeginn"}),
            Self::SyncEnde => json!({"art": "SyncEnde"}),
            Self::Zwischenablage { daten_b64 } => {
                json!({"art": "Zwischenablage", "daten_b64": daten_b64})
            }
            Self::ZwischenablageLesen => json!({"art": "Zwischenablage", "lesen": true}),
            Self::Titel { text } => json!({"art": "Titel", "text": text}),
            Self::GroessenFolge { spalten, zeilen } => {
                json!({"art": "GroessenFolge", "spalten": spalten, "zeilen": zeilen})
            }
            Self::Kindende { code } => json!({"art": "Kindende", "code": code}),
        }
    }

    /// `true` for the event kind `Ausgabe`.
    #[must_use]
    pub fn ist_ausgabe(&self) -> bool {
        matches!(self, Self::Ausgabe { .. })
    }
}
