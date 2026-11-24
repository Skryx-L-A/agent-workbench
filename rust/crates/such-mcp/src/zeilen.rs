//! The loop of the program: lines in, lines out, until the input ends.

use crate::rpc;
use crate::suche::Befrager;
use std::io::{self, BufRead, Read, Write};

/// The longest input line accepted, without the LF. A longer line is dropped with an error answer.
pub const MAX_ZEILE: usize = 8 * 1024 * 1024;

/// One line of the input.
#[derive(Debug, PartialEq, Eq)]
pub enum Zeile {
    /// The line without its LF (the last line of the input may have none).
    Ganz(Vec<u8>),
    /// The line was longer than the limit; it was read to its end and dropped.
    ZuLang,
}

/// The next line of `eingabe`, or `None` at the end of the input. A line has at most `max` bytes without its LF.
///
/// # Errors
/// The read error of `eingabe`.
pub fn naechste<R: BufRead>(eingabe: &mut R, max: usize) -> io::Result<Option<Zeile>> {
    let grenze = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    let mut puffer = Vec::new();
    eingabe
        .by_ref()
        .take(grenze)
        .read_until(b'\n', &mut puffer)?;
    if puffer.is_empty() {
        return Ok(None);
    }
    if puffer.last() == Some(&b'\n') {
        puffer.pop();
        return Ok(Some(Zeile::Ganz(puffer)));
    }
    if puffer.len() <= max {
        return Ok(Some(Zeile::Ganz(puffer)));
    }
    // Over the limit and no LF yet: read on to the LF or the end without keeping anything.
    eingabe.skip_until(b'\n')?;
    Ok(Some(Zeile::ZuLang))
}

/// Reads lines from `eingabe` and writes the answers to `ausgabe`, one line and a flush each, until the input ends.
///
/// # Errors
/// A read error of `eingabe` or a write error of `ausgabe`.
pub fn lauf<R: BufRead, W: Write, B: Befrager>(
    eingabe: &mut R,
    ausgabe: &mut W,
    befrager: &B,
    max_zeile: usize,
) -> io::Result<()> {
    while let Some(zeile) = naechste(eingabe, max_zeile)? {
        let antwort = match zeile {
            Zeile::Ganz(bytes) => rpc::antworte(&bytes, befrager),
            Zeile::ZuLang => Some(rpc::zu_lang()),
        };
        if let Some(antwort) = antwort {
            ausgabe.write_all(antwort.as_bytes())?;
            ausgabe.write_all(b"\n")?;
            ausgabe.flush()?;
        }
    }
    Ok(())
}
