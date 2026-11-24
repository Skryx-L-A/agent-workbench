//! The frame (byte form A-B, contract rules RA9 and RA10): the encoder of the sender and the strict decoder of the
//! receiver.
//!
//! ```text
//! [Art=5] [Sitzung u8] [Flags u8] [Stand vu]
//!         [Flags & 4: Spalten vu, Zeilen vu]
//!         [Flags & 1: oben vu, unten vu, n vu]
//!         [Flags & 2: zeile vu, spalte vu, sichtbar vu (0 or 1)]
//!         [number of styles vu] [each style: fg vu, bg vu, fl vu]
//!         [number of lines vu] [each line: index vu, number of runs vu, each run: style number vu, length vu, UTF-8]
//! ```

use std::collections::HashMap;
use std::hash::BuildHasher;

use crate::bild::{Abbild, Auszug, Cursor, GROESSE_MAX, SPALTEN_MIN, Schub, ZEILEN_MIN};
use crate::fehler::Fehler;
use crate::zeile::{Lauf, Stil, Zeile, kanonisch, zeichen_ok};

/// The first byte of every frame.
const ART: u8 = 5;
/// Flag: the frame holds a scroll.
const FLAG_SCHUB: u8 = 1;
/// Flag: the frame holds a cursor.
const FLAG_CURSOR: u8 = 2;
/// Flag: the frame is a full picture and holds its size.
const FLAG_GROESSE: u8 = 4;
/// Flag: the picture is inverse.
const FLAG_INVERS: u8 = 8;
/// The flag bits that must be 0.
const FLAGS_FREI: u8 = 0xf0;

/// The most bytes of a frame (RA11): longer is [`Fehler::RahmenZuGross`].
pub const RAHMEN_MAX: usize = 16_777_216;

// ---------------------------------------------------------------------------------------------------- encoder

/// Writes a varint: 7 bits per byte, lowest group first, bit 7 set while a byte follows.
fn vu_schreiben(aus: &mut Vec<u8>, n: u64) {
    // The number of 7-bit groups; the value 0 takes one.
    let gruppen = n.bit_width().div_ceil(7).max(1);
    for k in 0..gruppen {
        let gruppe = u8::try_from((n >> (7 * k)) & 0x7f).unwrap_or(0);
        aus.push(if k + 1 < gruppen {
            gruppe + 0x80
        } else {
            gruppe
        });
    }
}

/// A decoded number as an index. A number beyond `usize` becomes `usize::MAX`, which no check accepts.
fn als_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// Writes a count or an index.
fn zahl_schreiben(aus: &mut Vec<u8>, n: usize) {
    vu_schreiben(aus, u64::try_from(n).unwrap_or(u64::MAX));
}

/// The frame of an extract.
#[must_use]
pub fn rahmen_kodieren(a: &Auszug) -> Vec<u8> {
    let mut flags = 0;
    if a.schub.is_some() {
        flags |= FLAG_SCHUB;
    }
    if a.cursor.is_some() {
        flags |= FLAG_CURSOR;
    }
    if a.groesse.is_some() {
        flags |= FLAG_GROESSE;
    }
    if a.invers {
        flags |= FLAG_INVERS;
    }
    let mut aus = vec![ART, a.nr, flags];
    vu_schreiben(&mut aus, a.stand);
    if let Some((spalten, zeilen)) = a.groesse {
        zahl_schreiben(&mut aus, spalten);
        zahl_schreiben(&mut aus, zeilen);
    }
    if let Some(s) = a.schub {
        zahl_schreiben(&mut aus, s.oben);
        zahl_schreiben(&mut aus, s.unten);
        zahl_schreiben(&mut aus, s.n);
    }
    if let Some(c) = a.cursor {
        zahl_schreiben(&mut aus, c.zeile);
        zahl_schreiben(&mut aus, c.spalte);
        vu_schreiben(&mut aus, u64::from(c.sichtbar));
    }
    // The style table holds the styles in the order of their first appearance.
    let mut stile: Vec<Stil> = Vec::new();
    let mut nummer: HashMap<Stil, usize> = HashMap::new();
    for (_, z) in &a.zeilen {
        for l in z {
            nummer.entry(l.stil).or_insert_with(|| {
                stile.push(l.stil);
                stile.len() - 1
            });
        }
    }
    zahl_schreiben(&mut aus, stile.len());
    for s in &stile {
        vu_schreiben(&mut aus, u64::from(s.fg));
        vu_schreiben(&mut aus, u64::from(s.bg));
        vu_schreiben(&mut aus, u64::from(s.fl));
    }
    zahl_schreiben(&mut aus, a.zeilen.len());
    for (i, z) in &a.zeilen {
        zahl_schreiben(&mut aus, *i);
        zahl_schreiben(&mut aus, z.len());
        for l in z {
            zahl_schreiben(&mut aus, nummer[&l.stil]);
            zahl_schreiben(&mut aus, l.text.len());
            aus.extend_from_slice(l.text.as_bytes());
        }
    }
    aus
}

// ---------------------------------------------------------------------------------------------------- decoder

/// Reads bytes with the checks of RA10.
struct Leser<'a> {
    b: &'a [u8],
    p: usize,
}

impl Leser<'_> {
    /// How many bytes are not read yet.
    fn rest(&self) -> usize {
        self.b.len() - self.p
    }

    fn byte(&mut self) -> Result<u8, Fehler> {
        let x = *self.b.get(self.p).ok_or(Fehler::RahmenAbgeschnitten)?;
        self.p += 1;
        Ok(x)
    }

    /// A varint of at most 10 bytes that fits 64 bits, in its shortest form.
    fn vu(&mut self) -> Result<u64, Fehler> {
        let mut n: u128 = 0;
        for k in 0..10_u32 {
            let x = self.byte()?;
            n |= u128::from(x & 0x7f) << (7 * k);
            if x & 0x80 == 0 {
                let nicht_kuerzeste = k > 0 && x == 0;
                return match u64::try_from(n) {
                    Ok(wert) if !nicht_kuerzeste => Ok(wert),
                    _ => Err(Fehler::VuUngueltig),
                };
            }
        }
        Err(Fehler::VuUngueltig)
    }

    /// A count: it cannot exceed the bytes not read yet, because each element needs at least one byte. Nothing is
    /// reserved for it.
    fn zahl(&mut self) -> Result<usize, Fehler> {
        let n = self.vu()?;
        match usize::try_from(n) {
            Ok(n) if n <= self.rest() => Ok(n),
            _ => Err(Fehler::RahmenAbgeschnitten),
        }
    }

    fn bytes(&mut self, n: usize) -> Result<&[u8], Fehler> {
        if n > self.rest() {
            return Err(Fehler::RahmenAbgeschnitten);
        }
        let aus = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(aus)
    }
}

/// Checks a cursor against the size of the picture.
pub(crate) fn cursor_pruefen(
    zeile: u64,
    spalte: u64,
    spalten: usize,
    zeilen: usize,
) -> Result<(), Fehler> {
    if als_usize(zeile) < zeilen && als_usize(spalte) < spalten {
        Ok(())
    } else {
        Err(Fehler::CursorUngueltig)
    }
}

/// Reads the size of a full picture.
fn groesse_lesen(p: &mut Leser<'_>) -> Result<(usize, usize), Fehler> {
    let (sp, ze) = (als_usize(p.vu()?), als_usize(p.vu()?));
    if (SPALTEN_MIN..=GROESSE_MAX).contains(&sp) && (ZEILEN_MIN..=GROESSE_MAX).contains(&ze) {
        Ok((sp, ze))
    } else {
        Err(Fehler::GroesseUngueltig)
    }
}

/// Reads the scroll and checks it against the number of rows.
fn schub_lesen(p: &mut Leser<'_>, zeilen: usize) -> Result<Schub, Fehler> {
    let (oben, unten, n) = (als_usize(p.vu()?), als_usize(p.vu()?), als_usize(p.vu()?));
    let passt = n >= 1 && oben.checked_add(n).is_some_and(|s| s <= unten) && unten <= zeilen;
    if passt {
        Ok(Schub { oben, unten, n })
    } else {
        Err(Fehler::SchubUngueltig)
    }
}

/// Reads the cursor and checks it against the size of the picture.
fn cursor_lesen(p: &mut Leser<'_>, spalten: usize, zeilen: usize) -> Result<Cursor, Fehler> {
    let (z, s, v) = (p.vu()?, p.vu()?, p.vu()?);
    if v > 1 {
        return Err(Fehler::CursorUngueltig);
    }
    cursor_pruefen(z, s, spalten, zeilen)?;
    Ok(Cursor {
        zeile: als_usize(z),
        spalte: als_usize(s),
        sichtbar: v == 1,
    })
}

/// Reads the style table.
fn stile_lesen(p: &mut Leser<'_>) -> Result<Vec<Stil>, Fehler> {
    let ns = p.zahl()?;
    let mut stile = Vec::new();
    for _ in 0..ns {
        let (fg, bg, fl) = (p.vu()?, p.vu()?, p.vu()?);
        stile.push(Stil::neu(i128::from(fg), i128::from(bg), i128::from(fl))?);
    }
    Ok(stile)
}

/// Reads one line of a frame (after its index).
fn zeile_lesen_rahmen(p: &mut Leser<'_>, stile: &[Stil]) -> Result<Zeile, Fehler> {
    let nl = p.zahl()?;
    let mut z: Zeile = Vec::new();
    for _ in 0..nl {
        let sn = p.vu()?;
        let stil = usize::try_from(sn)
            .ok()
            .and_then(|i| stile.get(i))
            .ok_or(Fehler::StilnrUngueltig)?;
        let laenge = p.zahl()?;
        let roh = p.bytes(laenge)?;
        let text = std::str::from_utf8(roh).map_err(|_| Fehler::Utf8Ungueltig)?;
        if !text.chars().all(zeichen_ok) {
            return Err(Fehler::ZeichenUngueltig);
        }
        z.push(Lauf {
            stil: *stil,
            text: text.to_owned(),
        });
    }
    if kanonisch(&z) {
        Ok(z)
    } else {
        Err(Fehler::ZeileNichtKanonisch)
    }
}

/// Decodes and checks a frame against the pictures of the receiver and returns the extract and the new picture.
/// The map is not changed; the caller stores the new picture when it wants it.
///
/// # Errors
///
/// Every error of rule RA10, in the order the rule lists the checks.
pub fn rahmen_dekodieren<S: BuildHasher>(
    b: &[u8],
    abbilder: &HashMap<u8, Abbild, S>,
) -> Result<(Auszug, Abbild), Fehler> {
    let mut p = Leser { b, p: 0 };
    if p.byte()? != ART {
        return Err(Fehler::ArtUngueltig);
    }
    let nr = p.byte()?;
    let flags = p.byte()?;
    if flags & FLAGS_FREI != 0 {
        return Err(Fehler::FlagsUnbekannt);
    }
    let stand = p.vu()?;
    let voll = flags & FLAG_GROESSE != 0;
    let mut a = Auszug {
        nr,
        stand,
        groesse: None,
        invers: flags & FLAG_INVERS != 0,
        schub: None,
        cursor: None,
        zeilen: Vec::new(),
    };
    let alt = if voll {
        a.groesse = Some(groesse_lesen(&mut p)?);
        if flags & FLAG_SCHUB != 0 {
            return Err(Fehler::RahmenUngueltig);
        }
        None
    } else {
        let alt = abbilder.get(&nr).ok_or(Fehler::KeinAbzug)?;
        if stand <= alt.stand {
            return Err(Fehler::StandAlt);
        }
        Some(alt)
    };
    let (spalten, zeilen) = a
        .groesse
        .or_else(|| alt.map(|x| (x.spalten, x.zeilen)))
        .unwrap_or_default();
    if flags & FLAG_SCHUB != 0 {
        a.schub = Some(schub_lesen(&mut p, zeilen)?);
    }
    if flags & FLAG_CURSOR != 0 {
        a.cursor = Some(cursor_lesen(&mut p, spalten, zeilen)?);
    }
    let stile = stile_lesen(&mut p)?;
    let nz = p.zahl()?;
    let mut letzte: Option<usize> = None;
    for _ in 0..nz {
        let i = als_usize(p.vu()?);
        if i >= zeilen {
            return Err(Fehler::ZeileAusserhalb);
        }
        if letzte.is_some_and(|l| i <= l) {
            return Err(Fehler::ZeileReihenfolge);
        }
        letzte = Some(i);
        a.zeilen.push((i, zeile_lesen_rahmen(&mut p, &stile)?));
    }
    if p.rest() != 0 {
        return Err(Fehler::RahmenRest);
    }
    let neu = anwenden(&a, alt, spalten, zeilen);
    Ok((a, neu))
}

/// The picture after a checked frame: an empty picture of the size for a full picture, else a copy of the old
/// one; then the scroll, the lines, the cursor, `invers` and the stand.
fn anwenden(a: &Auszug, alt: Option<&Abbild>, spalten: usize, zeilen: usize) -> Abbild {
    let mut neu = alt.map_or_else(|| Abbild::leer(spalten, zeilen), Clone::clone);
    if let Some(s) = a.schub {
        neu.lines[s.oben..s.unten].rotate_left(s.n);
        for z in &mut neu.lines[s.unten - s.n..s.unten] {
            z.clear();
        }
    }
    for (i, z) in &a.zeilen {
        neu.lines[*i].clone_from(z);
    }
    if let Some(c) = a.cursor {
        neu.cursor = c;
    }
    neu.invers = a.invers;
    neu.stand = a.stand;
    neu
}
