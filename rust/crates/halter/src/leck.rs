//! The leak lock (section 8): before a byte goes out, the holder looks for every secret it holds, in each form a
//! target may give it back in: the bytes themselves, Base64 and Base64url (with or without padding, at each of the three
//! offsets in the text) and percent encoding (upper or lower case hex, also mixed with plain bytes). In a stream it keeps
//! the last [`Sperre::rueckhalt`] bytes back until the next part covers them; at the end of the stream the kept bytes go
//! out, because they cannot hold a whole form of a secret.

use crate::geheim::Geheim;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `true` if `nadel` stands in `heu`.
#[must_use]
pub fn enthaelt(heu: &[u8], nadel: &[u8]) -> bool {
    !nadel.is_empty() && nadel.len() <= heu.len() && heu.windows(nadel.len()).any(|f| f == nadel)
}

/// The Base64 characters of `bytes` in `alphabet`, without padding; the last character may hold bits of zero.
fn zeichen(bytes: &[u8], alphabet: &[u8; 64]) -> Vec<u8> {
    let bits = bytes.len() * 8;
    (0..bits.div_ceil(6))
        .map(|i| {
            let mut n = 0usize;
            for b in i * 6..i * 6 + 6 {
                let bit = bytes.get(b / 8).map_or(0, |x| (x >> (7 - b % 8)) & 1);
                n = n * 2 + usize::from(bit);
            }
            alphabet[n]
        })
        .collect()
}

/// The forms of `wert` the lock looks for: the bytes themselves and, for each offset (0, 1 or 2 bytes before it in the
/// encoded text) and both alphabets, the Base64 characters that only bytes of `wert` determine. Padding and the bytes
/// around the secret change only the characters at both ends, so the form holds with and without padding.
#[must_use]
pub fn formen(wert: &[u8]) -> Vec<Vec<u8>> {
    let mut f = vec![wert.to_vec()];
    for versatz in 0..3 {
        let mut roh = vec![0u8; versatz];
        roh.extend_from_slice(wert);
        let erstes = (8 * versatz).div_ceil(6);
        let ende = 8 * (versatz + wert.len()) / 6;
        if ende <= erstes {
            continue;
        }
        for alphabet in [B64, B64URL] {
            let form = zeichen(&roh, alphabet)[erstes..ende].to_vec();
            if !f.contains(&form) {
                f.push(form);
            }
        }
    }
    f
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// `daten` with every `%XX` (upper or lower case hex) replaced by its byte; `None` if there is no `%`.
#[must_use]
pub fn prozent_dekodiert(daten: &[u8]) -> Option<Vec<u8>> {
    if !daten.contains(&b'%') {
        return None;
    }
    let mut aus = Vec::with_capacity(daten.len());
    // walk along the rest instead of an index: a changed step of an index would only hang the loop
    let mut rest = daten;
    while let Some((&c, nach)) = rest.split_first() {
        if c == b'%'
            && let [h, l, ..] = nach
            && let (Some(h), Some(l)) = (hex(*h), hex(*l))
        {
            aus.push(h * 16 + l);
            rest = &nach[2..];
        } else {
            aus.push(c);
            rest = nach;
        }
    }
    Some(aus)
}

/// `true` if one of `formen` stands in `daten` or in `daten` with its percent encoding undone.
fn trifft_formen(formen: &[Vec<u8>], daten: &[u8]) -> bool {
    let dekodiert = prozent_dekodiert(daten);
    let sichten = std::iter::once(daten).chain(dekodiert.as_deref());
    sichten
        .into_iter()
        .any(|s| formen.iter().any(|f| enthaelt(s, f)))
}

/// `true` if any of `werte` stands in `daten`, in any form of [`formen`], also percent encoded.
#[must_use]
pub fn trifft(werte: &[Geheim], daten: &[u8]) -> bool {
    werte
        .iter()
        .any(|w| trifft_formen(&formen(w.bytes()), daten))
}

/// A secret was found in the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Treffer;

/// The lock over one stream of bytes, for the secrets it was made for.
pub struct Sperre<'a> {
    _werte: &'a [Geheim],
    formen: Vec<Vec<u8>>,
    rueckhalt: usize,
    puffer: Vec<u8>,
    gesehen: u64,
    frei: u64,
}

impl<'a> Sperre<'a> {
    /// A lock for the secrets `werte`.
    #[must_use]
    pub fn neu(werte: &'a [Geheim]) -> Self {
        let formen: Vec<Vec<u8>> = werte.iter().flat_map(|w| formen(w.bytes())).collect();
        let laengste = formen.iter().map(Vec::len).max().unwrap_or(0);
        Sperre {
            _werte: werte,
            formen,
            // a form with each byte percent encoded takes three bytes per byte
            rueckhalt: (3 * laengste).saturating_sub(1),
            puffer: Vec::new(),
            gesehen: 0,
            frei: 0,
        }
    }

    /// How many bytes the lock keeps back: three times the longest form minus one.
    #[must_use]
    pub fn rueckhalt(&self) -> usize {
        self.rueckhalt
    }

    /// Takes the next bytes and returns the bytes that may go out now.
    ///
    /// # Errors
    /// [`Treffer`] if the kept bytes and `daten` together hold a secret; nothing more may go out then.
    pub fn schieben(&mut self, daten: &[u8]) -> Result<Vec<u8>, Treffer> {
        self.puffer.extend_from_slice(daten);
        self.gesehen += daten.len() as u64;
        if trifft_formen(&self.formen, &self.puffer) {
            self.puffer.clear();
            return Err(Treffer);
        }
        let raus = self.puffer.len().saturating_sub(self.rueckhalt);
        let frei: Vec<u8> = self.puffer.drain(..raus).collect();
        self.frei += frei.len() as u64;
        Ok(frei)
    }
    /// The end of the stream: the kept bytes, which were checked already.
    pub fn ende(&mut self) -> Vec<u8> {
        self.frei += self.puffer.len() as u64;
        std::mem::take(&mut self.puffer)
    }

    /// Number of bytes taken so far.
    #[must_use]
    pub fn gesehen(&self) -> u64 {
        self.gesehen
    }

    /// Number of bytes released so far.
    #[must_use]
    pub fn frei(&self) -> u64 {
        self.frei
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(t: &[u8]) -> Geheim {
        Geheim::neu(t.to_vec())
    }

    const WERT: &[u8] = b"hgx1-geheimwert";

    fn b64(bytes: &[u8]) -> String {
        crate::b64::kodieren(bytes)
    }

    fn url(t: &str) -> String {
        t.replace('+', "-").replace('/', "_")
    }

    fn prozent(bytes: &[u8], gross: bool) -> String {
        bytes
            .iter()
            .map(|b| {
                if gross {
                    format!("%{b:02X}")
                } else {
                    format!("%{b:02x}")
                }
            })
            .collect()
    }

    #[test]
    fn treffer_ueber_teilgrenze() {
        let w = [g(WERT)];
        let mut s = Sperre::neu(&w);
        assert_eq!(s.rueckhalt(), 59, "3 x 20 (Base64 of 15 bytes) - 1");
        let vorne = [b"0123456789".repeat(7), b"vorne hgx1-ge".to_vec()].concat();
        let a = s.schieben(&vorne).expect("no hit yet");
        // 83 bytes in, 59 kept back
        assert_eq!(a, &vorne[..24]);
        assert_eq!(s.schieben(b"heimwert hinten"), Err(Treffer));
    }

    #[test]
    fn zeichen_wie_rfc_4648() {
        assert_eq!(zeichen(b"Man", B64), b"TWFu");
        assert_eq!(zeichen(b"Ma", B64), b"TWE");
        assert_eq!(zeichen(b"M", B64), b"TQ");
        assert_eq!(zeichen(&[0xfb, 0xff], B64), b"+/8");
        assert_eq!(zeichen(&[0xfb, 0xff], B64URL), b"-_8");
        assert_eq!(zeichen(b"", B64), b"");
    }

    #[test]
    fn formen_eines_werts() {
        let f = formen(b"Man");
        assert_eq!(f[0], b"Man");
        assert!(f.contains(&b"TWFu".to_vec()), "offset 0");
        // offset 1: x M a n -> only the characters of bits 12..30
        assert!(
            f.contains(&zeichen(b"\0Man", B64)[2..5].to_vec()),
            "offset 1"
        );
        assert!(
            f.contains(&zeichen(b"\0\0Man", B64)[3..6].to_vec()),
            "offset 2"
        );
        assert_eq!(f.len(), 4, "the alphabets agree here, no form twice");
        assert_eq!(formen(b"").len(), 1, "an empty value has only itself");
    }

    #[test]
    fn base64_mit_und_ohne_auffuellung_in_jedem_versatz() {
        let w = [g(WERT)];
        for vorne in 0..3 {
            for hinten in 0..3 {
                let roh = [vec![b'v'; vorne], WERT.to_vec(), vec![b'h'; hinten]].concat();
                let mit = b64(&roh);
                let ohne = mit.trim_end_matches('=').to_owned();
                for text in [mit.clone(), ohne.clone(), url(&mit), url(&ohne)] {
                    let daten = format!("{{\"t\":\"{text}\"}}");
                    assert!(trifft(&w, daten.as_bytes()), "{vorne} {hinten} {text}");
                }
            }
        }
    }

    #[test]
    fn base64url_mit_eigenen_zeichen() {
        // bytes whose Base64 holds `+` and `/`: the url form differs
        let wert = [0xfb, 0xff, 0xbf, 0xfb, 0xff, 0xbf, 0xfb, 0xff, 0xbf];
        let w = [g(&wert)];
        let text = b64(&wert);
        assert!(text.contains('+') && text.contains('/'));
        assert!(trifft(&w, text.as_bytes()));
        assert!(trifft(&w, url(&text).as_bytes()));
    }

    #[test]
    fn prozentkodierung_gross_klein_und_gemischt() {
        let w = [g(WERT)];
        for gross in [true, false] {
            let t = format!("a={}&b=1", prozent(WERT, gross));
            assert!(trifft(&w, t.as_bytes()), "{t}");
        }
        let gemischt = format!("hgx1%2D{}", prozent(b"geheimwert", false));
        assert!(trifft(&w, gemischt.as_bytes()), "{gemischt}");
        let b64_in_url = format!("q={}", prozent(b64(WERT).as_bytes(), true));
        assert!(trifft(&w, b64_in_url.as_bytes()), "{b64_in_url}");
    }

    #[test]
    fn prozent_dekodiert_jede_schreibweise() {
        assert_eq!(prozent_dekodiert(b"abc"), None);
        assert_eq!(
            prozent_dekodiert(b"%4a%4A%Ff%09%af%AF%zz%4").expect("%"),
            b"JJ\xff\x09\xaf\xaf%zz%4"
        );
        assert_eq!(prozent_dekodiert(b"%g0%0g%").expect("%"), b"%g0%0g%");
        assert_eq!(
            prozent_dekodiert(b"%30%39%41%46%61%66").expect("%"),
            b"09AFaf"
        );
    }

    #[test]
    fn kodiert_ueber_teilgrenze() {
        let w = [g(WERT)];
        for form in [b64(WERT), url(&b64(WERT)), prozent(WERT, true)] {
            let mut s = Sperre::neu(&w);
            let (a, b) = form.as_bytes().split_at(form.len() / 2);
            let mut aus = s
                .schieben(&[b"x".repeat(100), a.to_vec()].concat())
                .expect("half");
            assert_eq!(s.schieben(b), Err(Treffer), "{form}");
            aus.extend(s.ende());
            assert!(!enthaelt(&aus, a), "the first half stayed back: {form}");
        }
    }

    #[test]
    fn gegenprobe_harmloser_text_geht_durch() {
        let w = [g(WERT)];
        let andere = b"hgx1-geheimwerT";
        for text in [
            b"hgx1-geheimwer und mehr Text".to_vec(),
            andere.to_vec(),
            b64(andere).into_bytes(),
            prozent(andere, true).into_bytes(),
            b"%zz%4 aGd4MS1n ordinary text 100% fine".to_vec(),
            b64(b"ein ganz anderer Inhalt").into_bytes(),
        ] {
            assert!(!trifft(&w, &text), "{}", String::from_utf8_lossy(&text));
            let mut s = Sperre::neu(&w);
            let mut aus = s.schieben(&text).expect("no hit");
            aus.extend(s.ende());
            assert_eq!(aus, text);
        }
    }

    #[test]
    fn ende_gibt_alles_frei() {
        let w = [g(b"0123456789abcdef")];
        let mut s = Sperre::neu(&w);
        let mut aus = s.schieben(&[b'x'; 40]).expect("no hit");
        aus.extend(s.ende());
        assert_eq!(aus.len(), 40);
        assert_eq!(s.frei(), 40);
        assert_eq!(s.gesehen(), 40);
    }

    /// The counters add up over several parts: what went out and what was taken.
    #[test]
    fn zaehler_ueber_mehrere_teile() {
        let werte = [g(b"zq")];
        let mut sperre = Sperre::neu(&werte);
        let halt = sperre.rueckhalt();
        assert!(halt < 100, "{halt}");
        let erste = sperre.schieben(&[b'x'; 100]).expect("no hit");
        assert_eq!(erste.len(), 100 - halt);
        assert_eq!(sperre.frei(), 100 - halt as u64);
        let zweite = sperre.schieben(&[b'y'; 50]).expect("no hit");
        assert_eq!(zweite.len(), 50);
        assert_eq!(sperre.frei(), 150 - halt as u64);
        assert_eq!(sperre.gesehen(), 150);
        assert_eq!(sperre.ende().len(), halt);
        assert_eq!(sperre.frei(), 150);
    }

    #[test]
    fn ohne_geheimnis_geht_alles_sofort() {
        let w: [Geheim; 0] = [];
        let mut s = Sperre::neu(&w);
        assert_eq!(s.schieben(b"abc").expect("no hit"), b"abc");
        assert_eq!(s.ende(), Vec::<u8>::new());
    }
}
