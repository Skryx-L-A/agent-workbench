//! The screen of one session: grid, scanner and the events of TR2 to TR11.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Line;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::Parser;
use alacritty_terminal::vte::ansi::{Processor, Timeout};

use crate::ereignis::Ereignis;
use crate::scan::{Gesehen, Scan};
use crate::vorspann::Vorspann;
use crate::zeile::{HAUTTON_ERSATZ, klartext, zeilenform};

/// The default scrollback in lines (decision of 18.11.2026, not 10 000).
pub const RUECKLAUF_VORGABE: usize = 2000;

/// The fewest columns of a screen. `alacritty_terminal` 0.26.0 has two faults that occur only in a grid of one column (a
/// character of width 2 panics, `Term::resize` never ends); the library names `term::MIN_COLUMNS = 2` but does not check it
/// (alacritty/alacritty issue 9112). So the carrier never lets one column reach the grid.
pub const SPALTEN_MIN: usize = 2;
/// The most columns of a screen.
pub const SPALTEN_MAX: usize = 1000;
/// The fewest rows of a screen.
pub const ZEILEN_MIN: usize = 1;
/// The most rows of a screen.
pub const ZEILEN_MAX: usize = 1000;

/// A size outside 2 to 1000 columns and 1 to 1000 rows (TR13). It is always a refusal: nothing changes and nothing is clamped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroesseUngueltig;

impl fmt::Display for GroesseUngueltig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("groesse_ungueltig")
    }
}

impl std::error::Error for GroesseUngueltig {}

/// `true` if `spalten` x `zeilen` is a size the carrier takes: 2 to 1000 columns, 1 to 1000 rows.
#[must_use]
pub fn groesse_gueltig(spalten: usize, zeilen: usize) -> bool {
    (SPALTEN_MIN..=SPALTEN_MAX).contains(&spalten) && (ZEILEN_MIN..=ZEILEN_MAX).contains(&zeilen)
}

/// Bytes the grid wants to send back to the child (answers to device queries).
type Antworten = Arc<Mutex<Vec<u8>>>;

/// The listener of the grid. It keeps exactly one kind of event, the answers to device queries.
#[derive(Clone)]
struct Zuhoerer {
    antworten: Antworten,
}

impl EventListener for Zuhoerer {
    fn send_event(&self, ereignis: Event) {
        // SECURITY (TR10, TR7): only `PtyWrite` (the fixed answers to DA1, DSR and the like) goes back to the child. Every
        // other event is dropped here on purpose, none is ever answered: the colour, text area size and clipboard requests
        // (`ColorRequest`, `TextAreaSizeRequest`, `ClipboardLoad`) would bring values the carrier does not know or the
        // content of the clipboard, and a clipboard write is not used either. The OSC 52 data come from the own scanner
        // (`scan.rs`), not from here.
        if let Event::PtyWrite(text) = ereignis {
            // A109: the carrier answers the cursor query itself (`Raster::cursor_antworten`); the answer of the grid to the
            // same query is dropped here, so that each query gets exactly one answer.
            if ist_cursorantwort(&text) {
                return;
            }
            self.antworten
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(text.as_bytes());
        }
    }
}

/// `true` for the answer of `alacritty_terminal` to the cursor query: `ESC [ <digits> ; <digits> R` (`device_status(6)` in
/// `term/mod.rs`). No other answer of the library has this form; all of them end in `c`, `n`, `t` or `y`.
fn ist_cursorantwort(text: &str) -> bool {
    let Some(mitte) = text
        .strip_prefix("\x1b[")
        .and_then(|rest| rest.strip_suffix('R'))
    else {
        return false;
    };
    mitte.split_once(';').is_some_and(|(zeile, spalte)| {
        [zeile, spalte]
            .iter()
            .all(|zahl| !zahl.is_empty() && zahl.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The size of the grid for `alacritty_terminal`.
struct Groesse {
    spalten: usize,
    zeilen: usize,
}

impl Dimensions for Groesse {
    fn total_lines(&self) -> usize {
        self.zeilen
    }
    fn screen_lines(&self) -> usize {
        self.zeilen
    }
    fn columns(&self) -> usize {
        self.spalten
    }
}

/// Never buffers a synchronized update (DEC 2026): the core holds the images back, not the carrier (TR6).
#[derive(Default)]
struct KeinPuffer;

impl Timeout for KeinPuffer {
    fn set_timeout(&mut self, _dauer: Duration) {}
    fn clear_timeout(&mut self) {}
    fn pending_timeout(&self) -> bool {
        false
    }
}

/// The screen of one session.
pub struct Raster {
    term: Term<Zuhoerer>,
    verarbeiter: Processor<KeinPuffer>,
    parser: Parser,
    scan: Scan,
    spalten: usize,
    zeilen: usize,
    /// What the core was told last for each row.
    gemeldet: Vec<String>,
    /// The cursor the core was told last, or has since clamped to a new size (TR2, TR3). The service gives the core the
    /// cursor of an empty grid, top left and visible, as its start.
    gemeldet_cursor: (usize, usize, bool),
    stand: u64,
    antworten: Antworten,
    /// The events of the current call, in order.
    aus: Vec<Ereignis>,
    /// The start of a UTF-8 character (a skin tone modifier included) that a chunk ended in the middle of.
    text_rest: Vec<u8>,
    /// W7 (TR16): the prelude title of `ConPTY`, which is no title of the program.
    vorspann: Vorspann,
    /// `true` once a row with content or another event than `Ausgabe` went out; after it the prelude title is no longer possible. An
    /// `Ausgabe` that only moves or hides the cursor on an empty screen does not count: `ConPTY` repaints so before its prelude title
    /// when the size changes before the prelude (section 21c of the contract `dienst`, measured 25.11.2026).
    gemeldet_etwas: bool,
    /// The session title: the last title of the program (TR8). The prelude title of `ConPTY` is never in it.
    titel: Option<String>,
}

impl Raster {
    /// An empty screen of `spalten` x `zeilen` cells and a scrollback of `ruecklauf` lines. The core knows `""` for every
    /// row, the cursor is at the top left and `stand` is 0.
    ///
    /// # Errors
    /// The size is outside 2 to 1000 columns and 1 to 1000 rows. The check comes before `Term::new`, so no such size reaches
    /// `alacritty_terminal`.
    pub fn neu(spalten: usize, zeilen: usize, ruecklauf: usize) -> Result<Self, GroesseUngueltig> {
        // SECURITY (TR13): the size is checked before `Term::new`; a grid of one column would panic or hang in the library.
        if !groesse_gueltig(spalten, zeilen) {
            return Err(GroesseUngueltig);
        }
        let antworten = Antworten::default();
        let einstellung = Config {
            scrolling_history: ruecklauf,
            ..Config::default()
        };
        let term = Term::new(
            einstellung,
            &Groesse { spalten, zeilen },
            Zuhoerer {
                antworten: Arc::clone(&antworten),
            },
        );
        Ok(Self {
            term,
            verarbeiter: Processor::default(),
            parser: Parser::new(),
            scan: Scan::default(),
            spalten,
            zeilen,
            gemeldet: vec![String::new(); zeilen],
            gemeldet_cursor: (0, 0, true),
            stand: 0,
            antworten,
            aus: Vec::new(),
            text_rest: Vec::new(),
            vorspann: Vorspann::default(),
            gemeldet_etwas: false,
            titel: None,
        })
    }

    /// W7 (TR16 point 2): under `ConPTY` the first title before everything else, if it is the path of `programm`, is the prelude of
    /// `ConPTY`: it gives no `Titel` and does not change [`Raster::titel`]. Not called under Linux and macOS.
    pub fn vorspann_erwarten(&mut self, programm: &str) {
        self.vorspann = Vorspann::erwartet(programm);
    }

    /// The session title: the text of the last `Titel` event of the program, or `None` if it wrote none.
    #[must_use]
    pub fn titel(&self) -> Option<&str> {
        self.titel.as_deref()
    }

    /// Row `zeile` (negative: in the scrollback) in the canonical SGR form.
    fn zeilenform(&self, zeile: i32) -> String {
        zeilenform(&self.term.grid()[Line(zeile)], self.spalten)
    }

    /// Every row of the screen in the canonical SGR form (TR1).
    #[must_use]
    pub fn zeilen(&self) -> Vec<String> {
        (0..self.zeilen)
            .map(|zeile| self.zeilenform(zeile_als_i32(zeile)))
            .collect()
    }

    /// Every row of the screen as plain text without style.
    #[must_use]
    pub fn klartext(&self) -> Vec<String> {
        (0..self.zeilen)
            .map(|zeile| klartext(&self.term.grid()[Line(zeile_als_i32(zeile))], self.spalten))
            .collect()
    }

    /// Rewrites the bytes of a chunk before they reach the grid.
    ///
    /// Skin tone modifiers (U+1F3FB to U+1F3FF) are zero-width in the contract (TR11). `alacritty_terminal` would make them
    /// wide, so they are replaced by the zero-width stand-ins U+E0100 to U+E0104, and `zeile::sauber` writes the modifiers
    /// back when a row is read.
    ///
    /// An incomplete UTF-8 character at the end of the chunk is held back until the next chunk completes it, at every
    /// number of columns (TR2). Bytes that can never become a valid character go on unchanged. Without it `vte` counts a
    /// character that a chunk cuts differently from one that arrives whole (`65 cc | 81` gives another cursor than `65 cc 81`).
    fn umschreiben(&mut self, neu: &[u8]) -> Vec<u8> {
        let mut eingabe = std::mem::take(&mut self.text_rest);
        eingabe.extend_from_slice(neu);
        let halt = eingabe.len() - unvollstaendig_am_ende(&eingabe);
        self.text_rest = eingabe.split_off(halt);
        let mut aus = Vec::with_capacity(eingabe.len());
        let mut rest = &eingabe[..];
        loop {
            match rest {
                [0xF0, 0x9F, 0x8F, ton @ 0xBB..=0xBF, nach @ ..] => {
                    let ersatz = char::from_u32(HAUTTON_ERSATZ + u32::from(ton - 0xBB))
                        .unwrap_or('\u{fffd}');
                    let mut puffer = [0; 4];
                    aus.extend_from_slice(ersatz.encode_utf8(&mut puffer).as_bytes());
                    rest = nach;
                }
                [byte, nach @ ..] => {
                    aus.push(*byte);
                    rest = nach;
                }
                [] => break,
            }
        }
        aus
    }

    /// Gives bytes to the grid (after the rewrite of the skin tone modifiers).
    fn fuettern(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let umgeschrieben = self.umschreiben(bytes);
        self.verarbeiter.advance(&mut self.term, &umgeschrieben);
    }

    /// One `Ausgabe` with the rows that differ from the last report, if there are any or if the cursor differs (TR2). It
    /// always carries the cursor; a pure cursor step is an `Ausgabe` without rows.
    fn ausgabe_melden(&mut self) {
        let neu = self.zeilen();
        let cursor = self.cursor();
        let abweichend: Vec<(usize, String)> = self
            .gemeldet
            .iter()
            .zip(&neu)
            .enumerate()
            .filter(|(_, (alt, jetzt))| alt != jetzt)
            .map(|(index, (_, jetzt))| (index, jetzt.clone()))
            .collect();
        if abweichend.is_empty() && cursor == self.gemeldet_cursor {
            return;
        }
        self.stand += 1;
        // W7: only a row with content closes the prelude; a cursor step alone (the repaint of `ConPTY`) does not
        if !abweichend.is_empty() {
            self.gemeldet_etwas = true;
        }
        self.aus.push(Ereignis::Ausgabe {
            stand: self.stand,
            zeilen: abweichend,
            cursor,
        });
        self.gemeldet = neu;
        self.gemeldet_cursor = cursor;
    }

    /// A109: the carrier answers the cursor query `ESC[6n` itself, on every platform and whatever the screen store does with
    /// it. Under `ConPTY` (Windows) the child does not start before the answer (`portable-pty` 0.9.0 sets
    /// `PSEUDOCONSOLE_INHERIT_CURSOR`, and `ConPTY` then asks for the cursor first).
    ///
    /// The answer is `ESC [ <row> ; <column> R`, counted from 1, from the cursor of the own grid at the last byte of the query
    /// (the shown screen, the last column with a pending wrap). It goes into the answers right where the query came, so the
    /// answers keep the order of the queries; the answer of the grid to the same query is dropped in `Zuhoerer`. So there is
    /// exactly one answer per query, none twice and none lost. The query is no event: it reports no `Ausgabe` before it.
    fn cursor_antworten(&mut self) {
        let (zeile, spalte, _) = self.cursor();
        let antwort = format!("\x1b[{};{}R", zeile + 1, spalte + 1);
        self.antworten
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(antwort.as_bytes());
    }

    /// Reports what the scanner saw: the changed rows first, then the event (TR2 order). A cursor query is answered, not
    /// reported.
    fn gesehenes_melden(&mut self, gesehen: Vec<Gesehen>) {
        for ding in gesehen {
            if ding == Gesehen::Cursorabfrage {
                self.cursor_antworten();
                continue;
            }
            self.ausgabe_melden();
            // W7: the prelude title of `ConPTY` is dropped; it is no event and not the session title
            if let Gesehen::Titel(text) = &ding
                && self.vorspann.verwirft(text, self.gemeldet_etwas)
            {
                continue;
            }
            self.gemeldet_etwas = true;
            match ding {
                Gesehen::Cursorabfrage => {}
                Gesehen::Invers(an) => {
                    self.stand += 1;
                    self.aus.push(Ereignis::Invers {
                        an,
                        stand: self.stand,
                    });
                }
                Gesehen::Klammer(an) => self.aus.push(Ereignis::KlammerPaste { an }),
                Gesehen::SyncBeginn => self.aus.push(Ereignis::SyncBeginn),
                Gesehen::SyncEnde => self.aus.push(Ereignis::SyncEnde),
                Gesehen::Titel(text) => {
                    self.titel = Some(text.clone());
                    self.aus.push(Ereignis::Titel { text });
                }
                Gesehen::Zwischen(Some(daten_b64)) => {
                    self.aus.push(Ereignis::Zwischenablage { daten_b64 });
                }
                Gesehen::Zwischen(None) => self.aus.push(Ereignis::ZwischenablageLesen),
                Gesehen::GroessenFolge(spalten, zeilen) => {
                    self.aus.push(Ereignis::GroessenFolge { spalten, zeilen });
                }
            }
        }
    }

    /// One chunk of bytes of the child. The events come in the order of TR2: before each event that is not an output, the
    /// rows changed up to its last byte; after the last byte, the rows that still differ. A sequence that ends in a later
    /// chunk reports nothing now.
    pub fn bytes(&mut self, bytes: &[u8]) -> Vec<Ereignis> {
        let mut anfang = 0;
        for (i, byte) in bytes.iter().enumerate() {
            self.parser
                .advance(&mut self.scan, std::slice::from_ref(byte));
            let gesehen = self.scan.nehmen();
            if !gesehen.is_empty() {
                self.fuettern(&bytes[anfang..=i]);
                anfang = i + 1;
                self.gesehenes_melden(gesehen);
            }
        }
        self.fuettern(&bytes[anfang..]);
        self.ausgabe_melden();
        std::mem::take(&mut self.aus)
    }

    /// The effect `groessen` of the core (TR3): the grid changes as `Term::resize` does it, and the rows that differ from the
    /// rows the core cut or filled with `""`, and the cursor when it differs from the clamped cursor of the core, go out as
    /// one `Ausgabe`.
    ///
    /// # Errors
    /// The size is outside 2 to 1000 columns and 1 to 1000 rows. Nothing changes then: not the grid, the cursor, the
    /// scrollback, `stand` or the rows reported. The check comes before `Term::resize`.
    pub fn groesse(
        &mut self,
        spalten: usize,
        zeilen: usize,
    ) -> Result<Vec<Ereignis>, GroesseUngueltig> {
        // SECURITY (TR13): the size is checked before anything changes and before `Term::resize`, which never ends when a
        // wide character is in a grid that shrinks to one column.
        if !groesse_gueltig(spalten, zeilen) {
            return Err(GroesseUngueltig);
        }
        self.gemeldet.truncate(zeilen);
        self.gemeldet.resize(zeilen, String::new());
        // The core keeps its cursor inside the new size, last row and last column at most (contract of the core, R15, point 8);
        // the carrier does the same with the cursor it reported, and compares the cursor of the new grid with that one.
        let (zeile, spalte, sichtbar) = self.gemeldet_cursor;
        self.gemeldet_cursor = (
            zeile.min(zeilen.saturating_sub(1)),
            spalte.min(spalten.saturating_sub(1)),
            sichtbar,
        );
        self.term.resize(Groesse { spalten, zeilen });
        self.spalten = spalten;
        self.zeilen = zeilen;
        self.ausgabe_melden();
        Ok(std::mem::take(&mut self.aus))
    }

    /// The child ended: the rows still unreported go out first, then `Kindende` (TR12).
    pub fn kindende(&mut self, code: u32) -> Vec<Ereignis> {
        self.ausgabe_melden();
        self.aus.push(Ereignis::Kindende { code });
        std::mem::take(&mut self.aus)
    }

    /// The bytes the carrier must write back to the child (TR10): the answers of `alacritty_terminal` to device queries and
    /// the own answer to the cursor query (A109), in the order of the queries. A clipboard read is never among them.
    pub fn antworten(&mut self) -> Vec<u8> {
        std::mem::take(
            &mut *self
                .antworten
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// The cursor as (row, column, visible).
    #[must_use]
    pub fn cursor(&self) -> (usize, usize, bool) {
        let punkt = self.term.grid().cursor.point;
        (
            usize::try_from(punkt.line.0).unwrap_or(0),
            punkt.column.0,
            self.term.mode().contains(TermMode::SHOW_CURSOR),
        )
    }

    /// The size as (columns, rows).
    #[must_use]
    pub fn groesse_jetzt(&self) -> (usize, usize) {
        (self.spalten, self.zeilen)
    }

    /// The number of lines in the scrollback.
    #[must_use]
    pub fn ruecklauf_zahl(&self) -> usize {
        self.term.grid().history_size()
    }

    /// The oldest and the newest line of the scrollback, `None` if it is empty.
    #[must_use]
    pub fn ruecklauf_enden(&self) -> Option<(String, String)> {
        let zahl = self.ruecklauf_zahl();
        if zahl == 0 {
            return None;
        }
        Some((self.zeilenform(-zeile_als_i32(zahl)), self.zeilenform(-1)))
    }

    /// `true` if all `Ausgabe` events so far, applied to `""` and the cursor `[0, 0, true]`, give exactly the rows of the
    /// screen and the cursor.
    #[must_use]
    pub fn abbild_gleich(&self) -> bool {
        self.gemeldet == self.zeilen() && self.gemeldet_cursor == self.cursor()
    }

    /// The counter `stand` of the last event that carried it (0 before the first).
    #[must_use]
    pub fn stand(&self) -> u64 {
        self.stand
    }
}

/// The number of bytes at the end of `bytes` that are the start of a valid UTF-8 character that the bytes do not complete
/// (0 to 3). A prefix that can never become a valid character is not counted.
fn unvollstaendig_am_ende(bytes: &[u8]) -> usize {
    (1..=3.min(bytes.len()))
        .find(|&k| {
            std::str::from_utf8(&bytes[bytes.len() - k..])
                .is_err_and(|e| e.valid_up_to() == 0 && e.error_len().is_none())
        })
        .unwrap_or(0)
}

/// A row or line count as the signed index of the grid; sizes are at most 100 000 lines.
fn zeile_als_i32(zahl: usize) -> i32 {
    i32::try_from(zahl).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_size_for_the_grid_has_no_scrollback_of_its_own() {
        let groesse = Groesse {
            spalten: 7,
            zeilen: 3,
        };
        assert_eq!(groesse.total_lines(), 3);
        assert_eq!(groesse.screen_lines(), 3);
        assert_eq!(groesse.columns(), 7);
    }

    #[test]
    fn the_picture_differs_while_only_the_cursor_or_only_rows_are_not_reported() {
        // only the cursor moved: the rows are as reported
        let mut raster = Raster::neu(5, 3, 10).expect("valid size");
        raster.fuettern(b"\x1b[2;3H");
        assert!(!raster.abbild_gleich());
        raster.ausgabe_melden();
        assert!(raster.abbild_gleich());
        // only a row differs: the cursor is back where it was reported
        let mut raster = Raster::neu(5, 3, 10).expect("valid size");
        raster.fuettern(b"a\x1b[1;1H");
        assert_eq!(raster.cursor(), (0, 0, true));
        assert!(!raster.abbild_gleich());
    }

    #[test]
    fn only_the_answer_to_the_cursor_query_has_the_form_of_one() {
        for ja in ["\x1b[1;1R", "\x1b[12;345R", "\x1b[0;0R"] {
            assert!(ist_cursorantwort(ja), "{ja:?}");
        }
        for nein in [
            "\x1b[?6c",
            "\x1b[0n",
            "\x1b[8;24;80t",
            "\x1b[?2004;2$y",
            "\x1b[1;1",
            "[1;1R",
            "\x1b[;1R",
            "\x1b[1;R",
            "\x1b[1R",
            "\x1b[1;2;3R",
            "\x1b[a;1R",
            "\x1b[1;bR",
            "\x1b[?1;1R",
            "",
        ] {
            assert!(!ist_cursorantwort(nein), "{nein:?}");
        }
    }

    #[test]
    fn the_grid_drops_its_cursor_answer_and_keeps_the_others() {
        let mut raster = Raster::neu(10, 4, 10).expect("valid size");
        // straight into the grid, past the scanner: the answer of the grid to `ESC[6n` must not reach the child
        raster.fuettern(b"ab\x1b[6n\x1b[5n");
        assert_eq!(raster.antworten(), b"\x1b[0n");
    }

    #[test]
    fn each_cursor_query_gets_one_answer_from_the_cursor_at_the_query() {
        let mut raster = Raster::neu(10, 4, 10).expect("valid size");
        let ereignisse = raster.bytes(b"ab\x1b[6n\x1b[3;7Hc\x1b[6n\x1b[c\x1b[6n");
        assert_eq!(raster.antworten(), b"\x1b[1;3R\x1b[3;8R\x1b[?6c\x1b[3;8R");
        // no event of its own and no output of its own: one `Ausgabe` for the whole chunk
        assert_eq!(ereignisse.len(), 1);
        assert_eq!(raster.cursor(), (2, 7, true));
        // in the last column with a pending wrap, and on the shown screen of the alternate screen
        raster.bytes(b"\x1b[1;8Hxyz\x1b[6n\x1b[?1049h\x1b[2;2H\x1b[6n");
        assert_eq!(raster.antworten(), b"\x1b[1;10R\x1b[2;2R");
    }

    #[test]
    fn a_cursor_query_cut_by_a_chunk_is_answered_with_its_last_byte() {
        let mut raster = Raster::neu(10, 4, 10).expect("valid size");
        raster.bytes(b"abc\x1b[");
        assert_eq!(raster.antworten(), b"");
        raster.bytes(b"6");
        assert_eq!(raster.antworten(), b"");
        raster.bytes(b"nd\x1b[6");
        assert_eq!(raster.antworten(), b"\x1b[1;4R");
        // a query broken off by CAN gets no answer, a new query after a cut one gets exactly one
        raster.bytes(b"\x18x");
        assert_eq!(raster.antworten(), b"");
        raster.bytes(b"\x1b[6");
        raster.bytes(b"\x1b[6n");
        assert_eq!(raster.antworten(), b"\x1b[1;6R");
    }

    #[test]
    fn the_picture_differs_while_rows_are_not_reported() {
        let mut raster = Raster::neu(5, 2, 10).expect("valid size");
        assert!(raster.abbild_gleich());
        raster.fuettern(b"abc");
        assert!(!raster.abbild_gleich());
        raster.ausgabe_melden();
        assert!(raster.abbild_gleich());
    }

    const VORSPANN: &[u8] =
        b"\x1b[6n\x1b[?9001h\x1b[?1004h\x1b[m\x1b]0;C:\\Work\\tr.exe\x07\x1b[?25h";

    fn conpty() -> Raster {
        let mut raster = Raster::neu(20, 5, 10).expect("size is valid");
        raster.vorspann_erwarten(r"C:\Work\tr.exe");
        raster
    }

    fn titel_von(ereignisse: &[Ereignis]) -> Vec<&str> {
        ereignisse
            .iter()
            .filter_map(|e| match e {
                Ereignis::Titel { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn der_vorspann_von_conpty_gibt_kein_titelereignis_und_aendert_den_sitzungstitel_nicht() {
        let mut raster = conpty();
        let ereignisse = raster.bytes(VORSPANN);
        assert!(titel_von(&ereignisse).is_empty(), "{ereignisse:?}");
        assert_eq!(raster.titel(), None);
        // the cursor query of the prelude is still answered (TR14); the other sequences are no events
        assert_eq!(raster.antworten(), b"\x1b[1;1R");
        assert!(ereignisse.is_empty(), "{ereignisse:?}");
    }

    #[test]
    fn ein_titel_des_programms_nach_dem_vorspann_bleibt_ein_ereignis_und_setzt_den_sitzungstitel() {
        let mut raster = conpty();
        let mut ereignisse = raster.bytes(VORSPANN);
        ereignisse.extend(raster.bytes(b"\x1b]0;mein Titel\x07"));
        assert_eq!(titel_von(&ereignisse), ["mein Titel"]);
        assert_eq!(raster.titel(), Some("mein Titel"));
    }

    #[test]
    fn ein_titel_mit_dem_pfad_den_das_programm_selbst_schreibt_bleibt_ein_ereignis() {
        // other bytes of the child came before: this title is the program's own, also when it reads the same
        let mut raster = conpty();
        let ereignisse = raster.bytes(b"hallo\x1b]0;C:\\Work\\tr.exe\x07");
        assert_eq!(
            titel_von(&ereignisse),
            [r"C:\Work\tr.exe"],
            "{ereignisse:?}"
        );
        assert_eq!(raster.titel(), Some(r"C:\Work\tr.exe"));
        // also after another event than text
        let mut raster = conpty();
        let ereignisse = raster.bytes(b"\x1b[?2004h\x1b]2;C:\\Work\\tr.exe\x07");
        assert_eq!(
            titel_von(&ereignisse),
            [r"C:\Work\tr.exe"],
            "{ereignisse:?}"
        );
    }

    #[test]
    fn nach_dem_vorspann_bleibt_der_gleiche_titel_ein_ereignis() {
        let mut raster = conpty();
        raster.bytes(VORSPANN);
        let ereignisse = raster.bytes(b"\x1b]0;C:\\Work\\tr.exe\x07");
        assert_eq!(titel_von(&ereignisse), [r"C:\Work\tr.exe"]);
    }

    #[test]
    fn ein_anderer_pfad_im_ersten_titel_wird_nicht_verworfen() {
        let mut raster = conpty();
        let ereignisse = raster.bytes(b"\x1b]0;C:\\Other\\sh.exe\x07");
        assert_eq!(titel_von(&ereignisse), [r"C:\Other\sh.exe"]);
        assert_eq!(raster.titel(), Some(r"C:\Other\sh.exe"));
    }

    #[test]
    fn der_vorspann_in_kleinen_stuecken_wird_gleich_verworfen() {
        let mut raster = conpty();
        let mut ereignisse = Vec::new();
        for byte in VORSPANN {
            ereignisse.extend(raster.bytes(std::slice::from_ref(byte)));
        }
        assert!(titel_von(&ereignisse).is_empty(), "{ereignisse:?}");
        assert_eq!(raster.titel(), None);
    }

    #[test]
    fn unter_linux_und_macos_ohne_vorspann_bleibt_jeder_titel_ein_ereignis() {
        let mut raster = Raster::neu(20, 5, 10).expect("size is valid");
        let ereignisse = raster.bytes(VORSPANN);
        assert_eq!(titel_von(&ereignisse), [r"C:\Work\tr.exe"]);
        assert_eq!(raster.titel(), Some(r"C:\Work\tr.exe"));
    }

    /// The repaint of `ConPTY` before its prelude title, measured in the Windows VM on 25.11.2026 (size changed 14 to 33 ms after the
    /// start, before any byte; 8 of 8 rounds): it hides the cursor, clears every row and goes home. The `Ausgabe` of the hidden cursor
    /// does not close the prelude, so the title is still dropped.
    const NEUZEICHNEN: &[u8] = b"\x1b[?25l\x1b[m\x1b[K\r\n\x1b[K\r\n\x1b[K\r\n\x1b[K\r\n\x1b[K\x1b[H\x1b]0;C:\\Work\\tr.exe\x07\x1b[?25h";

    #[test]
    fn das_neuzeichnen_von_conpty_vor_dem_vorspann_laesst_den_titel_verwerfen() {
        let mut raster = conpty();
        let mut ereignisse = raster.bytes(b"\x1b[6n");
        ereignisse.extend(raster.bytes(b"\x1b[?9001h\x1b[?1004h"));
        ereignisse.extend(raster.bytes(NEUZEICHNEN));
        assert!(titel_von(&ereignisse).is_empty(), "{ereignisse:?}");
        assert_eq!(raster.titel(), None);
        // the cursor steps still go out as `Ausgabe` without rows
        assert_eq!(
            ereignisse,
            [
                Ereignis::Ausgabe {
                    stand: 1,
                    zeilen: Vec::new(),
                    cursor: (0, 0, false)
                },
                Ereignis::Ausgabe {
                    stand: 2,
                    zeilen: Vec::new(),
                    cursor: (0, 0, true)
                },
            ]
        );
    }

    #[test]
    fn nach_einem_cursorschritt_bleibt_ein_anderer_titel_und_nach_inhalt_der_pfad_ein_ereignis() {
        // a cursor step alone keeps the prelude open, but only the path of the program is dropped
        let mut raster = conpty();
        let ereignisse = raster.bytes(b"\x1b[?25l\x1b[3;4H\x1b]0;C:\\Other\\sh.exe\x07");
        assert_eq!(titel_von(&ereignisse), [r"C:\Other\sh.exe"]);
        // a row with content closes it, also when the cursor stepped first
        let mut raster = conpty();
        let ereignisse = raster.bytes(b"\x1b[?25l\x1b[2;1Hx\x1b]0;C:\\Work\\tr.exe\x07");
        assert_eq!(titel_von(&ereignisse), [r"C:\Work\tr.exe"]);
    }
}
