//! The line form: text with SGR sequences, read into runs and written back in the canonical way
//! (contract rules RA1 to RA4).

use crate::fehler::Fehler;

/// Bit in [`Stil::fl`]: bold.
pub const FETT: u8 = 1;
/// Bit in [`Stil::fl`]: dim.
pub const DIMM: u8 = 2;
/// Bit in [`Stil::fl`]: italic.
pub const KURSIV: u8 = 4;
/// Bit in [`Stil::fl`]: underlined.
pub const UNTERSTRICHEN: u8 = 8;
/// Bit in [`Stil::fl`]: inverse.
pub const INVERS: u8 = 16;
/// Bit in [`Stil::fl`]: hidden.
pub const VERDECKT: u8 = 32;
/// Bit in [`Stil::fl`]: struck through.
pub const DURCHGESTRICHEN: u8 = 64;

/// The flag bits with their SGR numbers, in the order the canonical form writes them.
const ATTRIBUTE: [(u8, u8); 7] = [
    (FETT, 1),
    (DIMM, 2),
    (KURSIV, 3),
    (UNTERSTRICHEN, 4),
    (INVERS, 7),
    (VERDECKT, 8),
    (DURCHGESTRICHEN, 9),
];

/// The first Truecolor value (`0x0100_0000 | rgb`).
const TRUECOLOR: u32 = 0x0100_0000;

/// A style: foreground, background and attribute bits. A colour is 0 (default), 1 to 256 (palette: 1 plus index
/// 0 to 255; 1 to 16 are the named colours, bright from 9) or `0x0100_0000 | rgb` (Truecolor).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stil {
    /// Foreground colour.
    pub fg: u32,
    /// Background colour.
    pub bg: u32,
    /// Attribute bits (1 bold, 2 dim, 4 italic, 8 underlined, 16 inverse, 32 hidden, 64 struck through).
    pub fl: u8,
}

impl Stil {
    /// The default style: no colour, no attribute.
    pub const VORGABE: Stil = Stil {
        fg: 0,
        bg: 0,
        fl: 0,
    };

    /// Whether the three values are a valid style (section 4): each colour 0 to 256 or `0x0100_0000` to
    /// `0x01ff_ffff`, the flags 0 to 127. The values are wide so that a caller may pass any whole number.
    #[must_use]
    pub fn gueltig(fg: i128, bg: i128, fl: i128) -> bool {
        let farbe = |c: i128| (0..=256).contains(&c) || (0x0100_0000..=0x01ff_ffff).contains(&c);
        farbe(fg) && farbe(bg) && (0..128).contains(&fl)
    }

    /// The style from three values already known to be valid.
    fn aus_gueltig(fg: i128, bg: i128, fl: i128) -> Stil {
        Stil {
            fg: u32::try_from(fg).unwrap_or(0),
            bg: u32::try_from(bg).unwrap_or(0),
            fl: u8::try_from(fl).unwrap_or(0),
        }
    }

    /// The style from three values, or [`Fehler::StilUngueltig`].
    ///
    /// # Errors
    ///
    /// [`Fehler::StilUngueltig`] when [`Stil::gueltig`] is false.
    pub fn neu(fg: i128, bg: i128, fl: i128) -> Result<Stil, Fehler> {
        if Stil::gueltig(fg, bg, fl) {
            Ok(Stil::aus_gueltig(fg, bg, fl))
        } else {
            Err(Fehler::StilUngueltig)
        }
    }
}

/// A run: text in one style.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lauf {
    /// The style of the whole text.
    pub stil: Stil,
    /// The text: Unicode scalars without C0, DEL and C1 characters.
    pub text: String,
}

/// A line: runs in canonical form (no empty run, no two neighbours of one style, and the last run is not the
/// default style with spaces at its end). The empty line has no runs.
pub type Zeile = Vec<Lauf>;

/// Whether a character is allowed in the text of a run: not below U+0020, not U+007F, not U+0080 to U+009F.
#[must_use]
pub fn zeichen_ok(c: char) -> bool {
    let o = u32::from(c);
    !(o < 0x20 || (0x7f..=0x9f).contains(&o))
}

/// Whether a line is in canonical form (RA2).
#[must_use]
pub fn kanonisch(z: &[Lauf]) -> bool {
    let nachbarn_gleich = z.windows(2).any(|w| w[0].stil == w[1].stil);
    let leer = z.iter().any(|l| l.text.is_empty());
    let ende = z
        .last()
        .is_some_and(|l| l.stil == Stil::VORGABE && l.text.ends_with(' '));
    !(nachbarn_gleich || leer || ende)
}

/// Bits 1 and 2 together: the SGR number 22 clears bold and dim at once.
const FETT_UND_DIMM: u8 = 3;

/// Applies the parameters of one SGR sequence to a style, left to right (RA1).
fn sgr_anwenden(ps: &[u16], start: Stil) -> Result<Stil, Fehler> {
    let mut st = start;
    let mut rest = ps;
    while let Some((&v, danach)) = rest.split_first() {
        rest = danach;
        match v {
            0 => st = Stil::VORGABE,
            1 => st.fl |= FETT,
            2 => st.fl |= DIMM,
            3 => st.fl |= KURSIV,
            4 => st.fl |= UNTERSTRICHEN,
            7 => st.fl |= INVERS,
            8 => st.fl |= VERDECKT,
            9 => st.fl |= DURCHGESTRICHEN,
            22 => st.fl &= !FETT_UND_DIMM,
            23 => st.fl &= !KURSIV,
            24 => st.fl &= !UNTERSTRICHEN,
            27 => st.fl &= !INVERS,
            28 => st.fl &= !VERDECKT,
            29 => st.fl &= !DURCHGESTRICHEN,
            30..=37 => st.fg = 1 + u32::from(v - 30),
            40..=47 => st.bg = 1 + u32::from(v - 40),
            90..=97 => st.fg = 9 + u32::from(v - 90),
            100..=107 => st.bg = 9 + u32::from(v - 100),
            39 => st.fg = 0,
            49 => st.bg = 0,
            38 | 48 => {
                let (farbe, nach_farbe) = erweiterte_farbe(rest)?;
                if v == 38 {
                    st.fg = farbe;
                } else {
                    st.bg = farbe;
                }
                rest = nach_farbe;
            }
            _ => return Err(Fehler::SgrParameter),
        }
    }
    Ok(st)
}

/// The colour after `38` or `48`: `5;n` or `2;r;g;b`. Returns the colour and the parameters after it.
fn erweiterte_farbe(rest: &[u16]) -> Result<(u32, &[u16]), Fehler> {
    match rest {
        [5, n, danach @ ..] if *n <= 255 => Ok((1 + u32::from(*n), danach)),
        [2, r, g, b, danach @ ..] if *r <= 255 && *g <= 255 && *b <= 255 => Ok((
            TRUECOLOR + (u32::from(*r) << 16) + (u32::from(*g) << 8) + u32::from(*b),
            danach,
        )),
        _ => Err(Fehler::SgrParameter),
    }
}

/// The parameters of a sequence: split at `;`, an empty one is 0, at most three digits each.
fn parameter(roh: &[u8]) -> Result<Vec<u16>, Fehler> {
    roh.split(|b| *b == b';')
        .map(|tok| {
            if tok.len() > 3 {
                return Err(Fehler::SgrParameter);
            }
            Ok(tok.iter().fold(0_u16, |n, d| n * 10 + u16::from(d - b'0')))
        })
        .collect()
}

/// The end of the sequence that starts at `ESC` at `i`: the index of its `m`, or the error (RA4).
fn sgr_ende(b: &[u8], i: usize) -> Result<usize, Fehler> {
    match b.get(i + 1) {
        None => return Err(Fehler::SgrUnvollstaendig),
        Some(b'[') => {}
        Some(_) => return Err(Fehler::EscUngueltig),
    }
    let j = i
        + 2
        + b[i + 2..]
            .iter()
            .take_while(|x| x.is_ascii_digit() || **x == b';')
            .count();
    match b.get(j) {
        None => Err(Fehler::SgrUnvollstaendig),
        Some(b'm') => Ok(j),
        Some(b'@'..=b'~') => Err(Fehler::SgrUnbekannt),
        Some(_) => Err(Fehler::SgrParameter),
    }
}

/// Reads a line form into runs in canonical form (RA1, RA2, RA4).
///
/// # Errors
///
/// [`Fehler::EscUngueltig`], [`Fehler::SgrUnvollstaendig`], [`Fehler::SgrUnbekannt`], [`Fehler::SgrParameter`]
/// for a bad sequence and [`Fehler::ZeichenUngueltig`] for a bad character; the first one in reading order.
pub fn zeile_lesen(text: &str) -> Result<Zeile, Fehler> {
    let bytes = text.as_bytes();
    let mut stil = Stil::VORGABE;
    let mut laeufe: Zeile = Vec::new();
    let mut zeichen = text.char_indices();
    while let Some((pos, z)) = zeichen.next() {
        if z == '\u{1b}' {
            let ende = sgr_ende(bytes, pos)?;
            stil = sgr_anwenden(&parameter(&bytes[pos + 2..ende])?, stil)?;
            // The rest of the sequence is ASCII, one character per byte; ESC itself is taken already.
            for _ in pos..ende {
                zeichen.next();
            }
            continue;
        }
        if !zeichen_ok(z) {
            return Err(Fehler::ZeichenUngueltig);
        }
        match laeufe.last_mut() {
            Some(letzter) if letzter.stil == stil => letzter.text.push(z),
            _ => laeufe.push(Lauf {
                stil,
                text: z.to_string(),
            }),
        }
    }
    ende_beschneiden(&mut laeufe);
    Ok(laeufe)
}

/// Cuts the spaces at the end of the line in the default style; a run that becomes empty goes, and so on with
/// the run before it while the last run has the default style (RA2).
fn ende_beschneiden(laeufe: &mut Zeile) {
    while let Some(letzter) = laeufe.last_mut() {
        if letzter.stil != Stil::VORGABE {
            return;
        }
        let neu_laenge = letzter.text.trim_end_matches(' ').len();
        if neu_laenge == 0 {
            laeufe.pop();
        } else {
            letzter.text.truncate(neu_laenge);
            return;
        }
    }
}

/// The SGR parameter text of a colour (RA3).
fn farbe_schreiben(c: u32, hintergrund: bool) -> String {
    if c >= TRUECOLOR {
        return format!(
            "{};2;{};{};{}",
            if hintergrund { 48 } else { 38 },
            (c >> 16) & 255,
            (c >> 8) & 255,
            c & 255
        );
    }
    let n = c - 1;
    if n < 8 {
        (n + if hintergrund { 40 } else { 30 }).to_string()
    } else if n < 16 {
        (n - 8 + if hintergrund { 100 } else { 90 }).to_string()
    } else {
        format!("{};5;{n}", if hintergrund { 48 } else { 38 })
    }
}

/// Writes a canonical line in its canonical form (RA3): a sequence before each run whose style differs from the
/// one before (the first run is compared with the default style), none at the end.
#[must_use]
pub fn zeile_schreiben(z: &[Lauf]) -> String {
    let mut aus = String::new();
    let mut vorher = Stil::VORGABE;
    for lauf in z {
        let st = lauf.stil;
        if st != vorher {
            if st == Stil::VORGABE {
                aus.push_str("\u{1b}[0m");
            } else {
                let mut ps = vec!["0".to_owned()];
                for (bit, code) in ATTRIBUTE {
                    if st.fl & bit != 0 {
                        ps.push(code.to_string());
                    }
                }
                if st.fg != 0 {
                    ps.push(farbe_schreiben(st.fg, false));
                }
                if st.bg != 0 {
                    ps.push(farbe_schreiben(st.bg, true));
                }
                aus.push_str("\u{1b}[");
                aus.push_str(&ps.join(";"));
                aus.push('m');
            }
        }
        aus.push_str(&lauf.text);
        vorher = st;
    }
    aus
}

/// Checks runs given as whole numbers and text, as the step `Laeufe` has them, and returns the line (RA4):
/// first every style, then every character, then the canonical form.
///
/// # Errors
///
/// [`Fehler::StilUngueltig`], else [`Fehler::ZeichenUngueltig`], else [`Fehler::ZeileNichtKanonisch`].
pub fn zeile_pruefen(roh: &[(i64, i64, i64, String)]) -> Result<Zeile, Fehler> {
    let gueltig = roh
        .iter()
        .all(|(fg, bg, fl, _)| Stil::gueltig(i128::from(*fg), i128::from(*bg), i128::from(*fl)));
    if !gueltig {
        return Err(Fehler::StilUngueltig);
    }
    if roh.iter().any(|(_, _, _, t)| !t.chars().all(zeichen_ok)) {
        return Err(Fehler::ZeichenUngueltig);
    }
    let zeile: Zeile = roh
        .iter()
        .map(|(fg, bg, fl, text)| Lauf {
            stil: Stil::aus_gueltig(i128::from(*fg), i128::from(*bg), i128::from(*fl)),
            text: text.clone(),
        })
        .collect();
    if kanonisch(&zeile) {
        Ok(zeile)
    } else {
        Err(Fehler::ZeileNichtKanonisch)
    }
}
