//! The small HTTP/1.1 of the model path: the head of a request of the harness (stage 2), the head of an answer of the target
//! and the body of that answer by its framing (length, chunked, up to the end).

use std::io::{self, Read};

/// A head of 65536 bytes or more (with the empty line) is unreadable.
pub(crate) const KOPF_MAX: usize = 65_536;
/// Longest line of the chunked framing (size line, trailer line) of an answer.
const ZEILE_MAX: usize = 4096;
/// More reads than a line of the framing can need (see `zeile`).
const ZEILE_RUNDEN: usize = 4098;
const BLOCK: usize = 16_384;

/// Why a head could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lesefehler {
    /// The deadline of a read ran out.
    Frist,
    /// End of the stream, an error of the stream, or a head that is too long or has the wrong form.
    Sonst,
}

/// The head of a request: method, request target and fields with lower-case names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Anfragekopf {
    pub(crate) methode: String,
    pub(crate) ziel: String,
    pub(crate) felder: Vec<(String, String)>,
}

impl Anfragekopf {
    /// The values of the fields with this lower-case name, in order.
    pub(crate) fn werte<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.felder
            .iter()
            .filter(move |(n, _)| n == name)
            .map(|(_, w)| w.as_str())
    }

    /// `Content-Length`: none, or exactly one decimal value; two fields or another form is an error (stage 2).
    pub(crate) fn inhaltslaenge(&self) -> Result<Option<u64>, ()> {
        let mut werte = self.werte("content-length");
        let erster = werte.next();
        if werte.next().is_some() {
            return Err(());
        }
        erster.map(dezimal).transpose()
    }
}

/// A decimal number `0` or `[1-9][0-9]{0,15}`.
fn dezimal(text: &str) -> Result<u64, ()> {
    let b = text.as_bytes();
    let gut = text == "0"
        || ((1..=16).contains(&b.len()) && b[0] != b'0' && b.iter().all(u8::is_ascii_digit));
    if gut {
        text.parse().map_err(|_| ())
    } else {
        Err(())
    }
}

/// Reads the head of a request and returns it with the bytes read after it.
pub(crate) fn lies_anfragekopf<R: Read>(r: &mut R) -> Option<(Anfragekopf, Vec<u8>)> {
    let (kopf, rest) = lies_bis_leerzeile(r, Vec::new()).ok()?;
    Some((zerlege_anfragekopf(&kopf)?, rest))
}

/// Reads up to the empty line; the head ends within the first 65536 bytes or is unreadable.
/// Returns the head without the empty line and the bytes after it.
pub(crate) fn lies_bis_leerzeile<R: Read>(
    r: &mut R,
    mut puf: Vec<u8>,
) -> Result<(Vec<u8>, Vec<u8>), Lesefehler> {
    let mut block = vec![0u8; BLOCK];
    loop {
        if let Some(ende) = finde(&puf, b"\r\n\r\n") {
            if ende + 4 > KOPF_MAX {
                return Err(Lesefehler::Sonst);
            }
            let rest = puf.split_off(ende + 4);
            puf.truncate(ende);
            return Ok((puf, rest));
        }
        if puf.len() >= KOPF_MAX {
            return Err(Lesefehler::Sonst);
        }
        let n = r.read(&mut block).map_err(|e| lesefehler(&e))?;
        if n == 0 {
            return Err(Lesefehler::Sonst);
        }
        puf.extend_from_slice(&block[..n]);
    }
}

fn lesefehler(e: &io::Error) -> Lesefehler {
    if matches!(
        e.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        Lesefehler::Frist
    } else {
        Lesefehler::Sonst
    }
}

/// The first position of `nadel` in `heu`.
pub(crate) fn finde(heu: &[u8], nadel: &[u8]) -> Option<usize> {
    heu.windows(nadel.len()).position(|w| w == nadel)
}

/// A head `METHOD SP target SP HTTP/1.1` and fields `name: value` (the form of the visible cases).
fn zerlege_anfragekopf(kopf: &[u8]) -> Option<Anfragekopf> {
    let text = std::str::from_utf8(kopf).ok()?;
    let mut zeilen = text.split("\r\n");
    let start = zeilen.next()?;
    let (methode, rest) = start.split_once(' ')?;
    let (ziel, version) = rest.split_once(' ')?;
    let methode_gut =
        (1..=16).contains(&methode.len()) && methode.bytes().all(|c| c.is_ascii_uppercase());
    let ziel_gut = ziel.starts_with('/') && ziel.bytes().all(|c| (0x21..=0x7e).contains(&c));
    if !(methode_gut && ziel_gut && version == "HTTP/1.1") {
        return None;
    }
    let mut felder = Vec::new();
    for zeile in zeilen {
        let (name, wert) = zeile.split_once(':')?;
        let wert = wert.trim_matches([' ', '\t']);
        if !(name_ok(name.as_bytes()) && wert.bytes().all(|c| (0x20..=0x7e).contains(&c))) {
            return None;
        }
        felder.push((name.to_ascii_lowercase(), wert.to_owned()));
    }
    Some(Anfragekopf {
        methode: methode.to_owned(),
        ziel: ziel.to_owned(),
        felder,
    })
}

/// A field name of `tchar` (RFC 9110).
pub(crate) fn name_ok(name: &[u8]) -> bool {
    !name.is_empty()
        && name
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+.^_`|~-".contains(c))
}

/// The head of an answer of the target: status and fields with lower-case names and raw values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Antwortkopf {
    pub(crate) status: u16,
    pub(crate) felder: Vec<(String, Vec<u8>)>,
    /// The field names as sent, in the order of `felder`.
    pub(crate) namen: Vec<String>,
}

impl Antwortkopf {
    fn werte<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a [u8]> + 'a {
        self.felder
            .iter()
            .filter(move |(n, _)| n == name)
            .map(|(_, w)| w.as_slice())
    }

    /// `content-type` for the harness: exactly one field of printable ASCII, 1 to 256 characters (section 7).
    pub(crate) fn inhaltstyp(&self) -> Option<&[u8]> {
        let mut werte = self.werte("content-type");
        let erster = werte.next()?;
        let gut = werte.next().is_none()
            && (1..=256).contains(&erster.len())
            && erster.iter().all(|c| (0x20..=0x7e).contains(c));
        gut.then_some(erster)
    }

    /// The framing of the body: chunked when the last coding of `Transfer-Encoding` is `chunked`, up to the end for
    /// another coding or without a length, else the length. Two different or wrong lengths: `None`.
    pub(crate) fn rahmung(&self) -> Option<Rahmung> {
        if let Some(letzte) = self.werte("transfer-encoding").last() {
            let kodierung = letzte.rsplit(|c| *c == b',').next().unwrap_or(letzte);
            let chunked = kodierung.trim_ascii().eq_ignore_ascii_case(b"chunked");
            return Some(if chunked {
                Rahmung::Stueckweise
            } else {
                Rahmung::BisEnde
            });
        }
        let mut laengen = self.werte("content-length");
        let Some(erste) = laengen.next() else {
            return Some(Rahmung::BisEnde);
        };
        if laengen.any(|w| w != erste) {
            return None;
        }
        let n = std::str::from_utf8(erste).ok().map(dezimal)?.ok()?;
        Some(Rahmung::Laenge(n))
    }
}

/// How the body of an answer ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rahmung {
    /// After this many bytes.
    Laenge(u64),
    /// Chunked transfer coding (the chunks are decoded, the body goes on unchanged).
    Stueckweise,
    /// With the end of the connection.
    BisEnde,
}

/// Reads the head of the answer; interim answers 1xx are skipped. Returns the head and the bytes read after it.
pub(crate) fn lies_antwortkopf<R: Read>(r: &mut R) -> Result<(Antwortkopf, Vec<u8>), Lesefehler> {
    let mut vorrat = Vec::new();
    loop {
        let (kopf, rest) = lies_bis_leerzeile(r, vorrat)?;
        let k = zerlege_antwortkopf(&kopf).ok_or(Lesefehler::Sonst)?;
        // Interim answers are skipped; 101 (switching protocols) is a final answer.
        if k.status >= 200 || k.status == 101 {
            return Ok((k, rest));
        }
        vorrat = rest;
    }
}

/// `HTTP/1.x SSS[ reason]` and fields; the status lies in 100 to 599.
fn zerlege_antwortkopf(kopf: &[u8]) -> Option<Antwortkopf> {
    let mut zeilen = kopf.split(|c| *c == b'\n');
    let start = zeilen.next()?;
    let start = start.strip_suffix(b"\r").unwrap_or(start);
    let rest = start
        .strip_prefix(b"HTTP/1.1 ")
        .or_else(|| start.strip_prefix(b"HTTP/1.0 "))?;
    let (ziffern, grund) = rest.split_at_checked(3)?;
    if !(grund.is_empty() || grund.starts_with(b" ")) || !ziffern.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let status: u16 = std::str::from_utf8(ziffern).ok()?.parse().ok()?;
    if !(100..=599).contains(&status) {
        return None;
    }
    let mut felder = Vec::new();
    let mut namen = Vec::new();
    for zeile in zeilen {
        let zeile = zeile.strip_suffix(b"\r").unwrap_or(zeile);
        let doppelpunkt = zeile.iter().position(|c| *c == b':')?;
        let (name, wert) = zeile.split_at(doppelpunkt);
        if !name_ok(name) {
            return None;
        }
        let name = String::from_utf8(name.to_vec()).ok()?;
        felder.push((name.to_ascii_lowercase(), wert[1..].trim_ascii().to_vec()));
        namen.push(name);
    }
    Some(Antwortkopf {
        status,
        felder,
        namen,
    })
}

/// The body of an answer, part by part as it arrives.
pub(crate) struct KoerperLeser<R> {
    r: R,
    puf: Vec<u8>,
    zustand: Zustand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zustand {
    Laenge(u64),
    BisEnde,
    Groesse,
    Daten(u64),
    DatenEnde,
    Anhang,
    Fertig,
}

impl<R: Read> KoerperLeser<R> {
    /// A reader of the body after the head; `rest` are the bytes already read after the head.
    pub(crate) fn neu(r: R, rest: Vec<u8>, rahmung: Rahmung) -> Self {
        let zustand = match rahmung {
            Rahmung::Laenge(n) => Zustand::Laenge(n),
            Rahmung::Stueckweise => Zustand::Groesse,
            Rahmung::BisEnde => Zustand::BisEnde,
        };
        Self {
            r,
            puf: rest,
            zustand,
        }
    }

    /// The next part of the body; `Ok(None)` at its end.
    ///
    /// # Errors
    /// The stream breaks, ends too early or the chunked framing is wrong.
    pub(crate) fn teil(&mut self) -> io::Result<Option<Vec<u8>>> {
        loop {
            match self.zustand {
                Zustand::Fertig | Zustand::Laenge(0) => return Ok(None),
                Zustand::Laenge(n) => {
                    let t = self.nimm(n)?;
                    self.zustand = Zustand::Laenge(n - laenge(&t));
                    return Ok(Some(t));
                }
                Zustand::Daten(n) => {
                    let t = self.nimm(n)?;
                    let rest = n - laenge(&t);
                    self.zustand = if rest == 0 {
                        Zustand::DatenEnde
                    } else {
                        Zustand::Daten(rest)
                    };
                    return Ok(Some(t));
                }
                Zustand::BisEnde => {
                    if self.puf.is_empty() && !self.gelesen()? {
                        self.zustand = Zustand::Fertig;
                    } else {
                        return Ok(Some(std::mem::take(&mut self.puf)));
                    }
                }
                Zustand::Groesse => {
                    let zeile = self.zeile()?;
                    let n = groesse(&zeile).ok_or_else(rahmenfehler)?;
                    self.zustand = if n == 0 {
                        Zustand::Anhang
                    } else {
                        Zustand::Daten(n)
                    };
                }
                Zustand::DatenEnde => {
                    if !self.zeile()?.is_empty() {
                        return Err(rahmenfehler());
                    }
                    self.zustand = Zustand::Groesse;
                }
                Zustand::Anhang => {
                    if self.zeile()?.is_empty() {
                        self.zustand = Zustand::Fertig;
                    }
                }
            }
        }
    }

    /// One read into the buffer. The callers see the end of the stream by the length of the buffer, so a changed
    /// result of this function cannot make them loop.
    fn fuelle(&mut self) -> io::Result<()> {
        let mut block = vec![0u8; BLOCK];
        let n = self.r.read(&mut block)?;
        self.puf.extend_from_slice(&block[..n]);
        Ok(())
    }

    /// Whether one read brought new bytes.
    fn gelesen(&mut self) -> io::Result<bool> {
        let vorher = self.puf.len();
        self.fuelle()?;
        Ok(self.puf.len() != vorher)
    }

    /// Up to `n` bytes: what is buffered, or one read.
    fn nimm(&mut self, n: u64) -> io::Result<Vec<u8>> {
        if self.puf.is_empty() && !self.gelesen()? {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let k = usize::try_from(n).unwrap_or(usize::MAX).min(self.puf.len());
        let rest = self.puf.split_off(k);
        Ok(std::mem::replace(&mut self.puf, rest))
    }

    /// One line of the framing without CRLF.
    fn zeile(&mut self) -> io::Result<Vec<u8>> {
        // Each round adds at least one byte or ends, so a line of at most ZEILE_MAX bytes needs fewer rounds.
        for _ in 0..ZEILE_RUNDEN {
            if let Some(i) = finde(&self.puf, b"\r\n") {
                let rest = self.puf.split_off(i + 2);
                let mut zeile = std::mem::replace(&mut self.puf, rest);
                zeile.truncate(i);
                return Ok(zeile);
            }
            if self.puf.len() > ZEILE_MAX {
                return Err(rahmenfehler());
            }
            if !self.gelesen()? {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
        }
        Err(rahmenfehler())
    }
}

fn laenge(t: &[u8]) -> u64 {
    u64::try_from(t.len()).unwrap_or(u64::MAX)
}

fn rahmenfehler() -> io::Error {
    io::ErrorKind::InvalidData.into()
}

/// The size of a chunk: 1 to 16 hex digits before an optional `;extension`.
fn groesse(zeile: &[u8]) -> Option<u64> {
    let ziffern = zeile.split(|c| *c == b';').next()?.trim_ascii();
    if !(1..=16).contains(&ziffern.len()) {
        return None;
    }
    u64::from_str_radix(std::str::from_utf8(ziffern).ok()?, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        Anfragekopf, KOPF_MAX, KoerperLeser, Lesefehler, Rahmung, dezimal, finde, groesse,
        lies_anfragekopf, lies_antwortkopf, lies_bis_leerzeile, zerlege_anfragekopf,
        zerlege_antwortkopf,
    };
    use std::collections::VecDeque;
    use std::io::{self, Read};

    /// A reader that hands out one prepared part per read; an empty queue is the end, `Err` parts are errors.
    struct Teile(VecDeque<io::Result<Vec<u8>>>);

    impl Teile {
        fn neu(teile: &[&[u8]]) -> Self {
            Self(teile.iter().map(|t| Ok(t.to_vec())).collect())
        }
    }

    impl Read for Teile {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.pop_front() {
                None => Ok(0),
                Some(Err(e)) => Err(e),
                Some(Ok(mut t)) => {
                    let n = t.len().min(buf.len());
                    buf[..n].copy_from_slice(&t[..n]);
                    if n < t.len() {
                        self.0.push_front(Ok(t.split_off(n)));
                    }
                    Ok(n)
                }
            }
        }
    }

    /// All parts; a reader that never ends fails the test instead of hanging it.
    fn alle<R: Read>(mut l: KoerperLeser<R>) -> io::Result<Vec<Vec<u8>>> {
        let mut v = Vec::new();
        while let Some(t) = l.teil()? {
            assert!(!t.is_empty() || v.len() < 64, "endless empty parts");
            v.push(t);
            assert!(v.len() < 10_000, "endless parts");
        }
        Ok(v)
    }

    #[test]
    fn the_head_ends_within_65536_bytes() {
        let mut genau = b"GET / HTTP/1.1\r\nX: ".to_vec();
        genau.resize(KOPF_MAX - 4, b'a');
        genau.extend_from_slice(b"\r\n\r\nrest");
        let (kopf, rest) =
            lies_bis_leerzeile(&mut Teile::neu(&[&genau]), Vec::new()).expect("fits");
        assert_eq!(kopf.len(), KOPF_MAX - 4);
        assert!(b"rest".starts_with(&rest));
        let mut lang = b"GET / HTTP/1.1\r\nX: ".to_vec();
        lang.resize(KOPF_MAX - 3, b'a');
        lang.extend_from_slice(b"\r\n\r\n");
        assert_eq!(
            lies_bis_leerzeile(&mut Teile::neu(&[&lang]), Vec::new()),
            Err(Lesefehler::Sonst)
        );
    }

    #[test]
    fn a_full_head_without_end_is_not_read_further() {
        let voll = vec![b'a'; KOPF_MAX];
        let mut r = Teile::neu(&[&voll]);
        r.0.push_back(Err(io::ErrorKind::TimedOut.into()));
        assert_eq!(
            lies_bis_leerzeile(&mut r, Vec::new()),
            Err(Lesefehler::Sonst)
        );
        let mut r = Teile::neu(&[&voll[1..]]);
        r.0.push_back(Ok(b"\r\n\r\n".to_vec()));
        assert_eq!(
            lies_bis_leerzeile(&mut r, Vec::new()),
            Err(Lesefehler::Sonst)
        );
    }

    #[test]
    fn the_head_may_come_in_parts_and_a_deadline_is_its_own_error() {
        let mut r = Teile::neu(&[b"GET / HT", b"TP/1.1\r", b"\n\r\nab"]);
        let (k, rest) = lies_anfragekopf(&mut r).expect("head");
        assert_eq!(
            (k.methode.as_str(), k.ziel.as_str(), rest.as_slice()),
            ("GET", "/", &b"ab"[..])
        );
        let mut r = Teile::neu(&[b"GET"]);
        r.0.push_back(Err(io::ErrorKind::TimedOut.into()));
        assert_eq!(
            lies_bis_leerzeile(&mut r, Vec::new()),
            Err(Lesefehler::Frist)
        );
        let mut r = Teile::neu(&[]);
        r.0.push_back(Err(io::ErrorKind::WouldBlock.into()));
        assert_eq!(
            lies_bis_leerzeile(&mut r, Vec::new()),
            Err(Lesefehler::Frist)
        );
        let mut r = Teile::neu(&[]);
        r.0.push_back(Err(io::ErrorKind::BrokenPipe.into()));
        assert_eq!(
            lies_bis_leerzeile(&mut r, Vec::new()),
            Err(Lesefehler::Sonst)
        );
        assert_eq!(
            lies_bis_leerzeile(&mut Teile::neu(&[b"GET"]), Vec::new()),
            Err(Lesefehler::Sonst)
        );
        assert!(lies_anfragekopf(&mut Teile::neu(&[b"GET / HTTP/1.1\r\n\r\n"])).is_some());
        assert!(lies_anfragekopf(&mut Teile::neu(&[b"get / HTTP/1.1\r\n\r\n"])).is_none());
    }

    #[test]
    fn the_form_of_a_request_head() {
        let k = zerlege_anfragekopf(b"POST /a HTTP/1.1\r\nHost: x\r\nX-A:\t b c \t\r\nE:")
            .expect("valid");
        assert_eq!(
            k.felder,
            vec![
                ("host".to_owned(), "x".to_owned()),
                ("x-a".to_owned(), "b c".to_owned()),
                ("e".to_owned(), String::new())
            ]
        );
        for falsch in [
            &b"POST /a HTTP/1.0"[..],
            b"POST /a HTTP/1.1 ",
            b"POST a HTTP/1.1",
            b"POST /a\x7f HTTP/1.1",
            b"POST  /a HTTP/1.1",
            b"PoST /a HTTP/1.1",
            b" /a HTTP/1.1",
            b"ABCDEFGHIJKLMNOPQ /a HTTP/1.1",
            b"POST /a HTTP/1.1\r\nHost x",
            b"POST /a HTTP/1.1\r\nHo st: x",
            b"POST /a HTTP/1.1\r\n: x",
            b"POST /a HTTP/1.1\r\nA: b\tc",
            b"POST /a HTTP/1.1\r\nA: b\x7f",
            b"POST /a HTTP/1.1\r\nA: \xc3\xa4",
            b"POST /a HTTP/1.1\r\nA: \x01",
            b"POST /a HTTP/1.1\nA: b",
        ] {
            assert_eq!(
                zerlege_anfragekopf(falsch),
                None,
                "{}",
                String::from_utf8_lossy(falsch)
            );
        }
        assert!(zerlege_anfragekopf(b"ABCDEFGHIJKLMNOP /a HTTP/1.1").is_some());
        assert!(zerlege_anfragekopf(b"POST /!~ HTTP/1.1\r\n!#$%&'*+.^_`|~-a0: \x20~").is_some());
    }

    fn mit_laenge(werte: &[&str]) -> Anfragekopf {
        Anfragekopf {
            methode: "POST".to_owned(),
            ziel: "/".to_owned(),
            felder: werte
                .iter()
                .map(|w| ("content-length".to_owned(), (*w).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn the_length_is_one_decimal_number() {
        assert_eq!(mit_laenge(&[]).inhaltslaenge(), Ok(None));
        assert_eq!(mit_laenge(&["0"]).inhaltslaenge(), Ok(Some(0)));
        assert_eq!(mit_laenge(&["12"]).inhaltslaenge(), Ok(Some(12)));
        assert_eq!(mit_laenge(&["2", "2"]).inhaltslaenge(), Err(()));
        assert_eq!(mit_laenge(&["02"]).inhaltslaenge(), Err(()));
        assert_eq!(dezimal("9999999999999999"), Ok(9_999_999_999_999_999));
        assert_eq!(dezimal("10000000000000000"), Err(()));
        assert_eq!(dezimal(""), Err(()));
        assert_eq!(dezimal("+1"), Err(()));
        assert_eq!(dezimal("1a"), Err(()));
    }

    #[test]
    fn the_head_of_an_answer() {
        let k = zerlege_antwortkopf(b"HTTP/1.1 200 OK\r\nContent-Type:  a/b \r\nX-Y: \xff")
            .expect("valid");
        assert_eq!(k.status, 200);
        assert_eq!(k.namen, vec!["Content-Type".to_owned(), "X-Y".to_owned()]);
        assert_eq!(
            k.felder,
            vec![
                ("content-type".to_owned(), b"a/b".to_vec()),
                ("x-y".to_owned(), vec![0xff])
            ]
        );
        assert_eq!(
            zerlege_antwortkopf(b"HTTP/1.0 599").map(|k| k.status),
            Some(599)
        );
        assert_eq!(
            zerlege_antwortkopf(b"HTTP/1.1 100 x").map(|k| k.status),
            Some(100)
        );
        for falsch in [
            &b"HTTP/1.1 600 x"[..],
            b"HTTP/1.1 099 x",
            b"HTTP/2 200 x",
            b"HTTP/1.1 20",
            b"HTTP/1.1 2000",
            b"HTTP/1.1 2a0",
            b"HTTP/1.1 200 x\r\nkein feld",
            b"HTTP/1.1 200 x\r\na b: c",
        ] {
            assert_eq!(
                zerlege_antwortkopf(falsch),
                None,
                "{}",
                String::from_utf8_lossy(falsch)
            );
        }
    }

    #[test]
    fn interim_answers_are_skipped() {
        let mut r = Teile::neu(&[b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 201 x\r\n\r\nab"]);
        let (k, rest) = lies_antwortkopf(&mut r).expect("head");
        assert_eq!((k.status, rest.as_slice()), (201, &b"ab"[..]));
        let mut r = Teile::neu(&[b"HTTP/1.1 101 x\r\n\r\n"]);
        assert_eq!(lies_antwortkopf(&mut r).map(|(k, _)| k.status), Ok(101));
        let mut r = Teile::neu(&[
            b"HTTP/1.1 102 x\r\n\r\n",
            b"HTTP/1.1 199 x\r\n\r\n",
            b"HTTP/1.1 200 x\r\n\r\n",
        ]);
        assert_eq!(lies_antwortkopf(&mut r).map(|(k, _)| k.status), Ok(200));
        assert_eq!(
            lies_antwortkopf(&mut Teile::neu(&[b"HTTP/1.1 2 x\r\n\r\n"])),
            Err(Lesefehler::Sonst)
        );
    }

    fn kopf(felder: &[(&str, &[u8])]) -> super::Antwortkopf {
        super::Antwortkopf {
            status: 200,
            felder: felder
                .iter()
                .map(|(n, w)| ((*n).to_owned(), w.to_vec()))
                .collect(),
            namen: felder.iter().map(|(n, _)| (*n).to_owned()).collect(),
        }
    }

    #[test]
    fn the_content_type_for_the_harness() {
        assert_eq!(
            kopf(&[("content-type", b"a")]).inhaltstyp(),
            Some(&b"a"[..])
        );
        assert_eq!(kopf(&[]).inhaltstyp(), None);
        assert_eq!(
            kopf(&[("content-type", b"a"), ("content-type", b"a")]).inhaltstyp(),
            None
        );
        assert_eq!(kopf(&[("content-type", b"")]).inhaltstyp(), None);
        assert_eq!(
            kopf(&[("content-type", &[b'a'; 256])]).inhaltstyp(),
            Some(&[b'a'; 256][..])
        );
        assert_eq!(kopf(&[("content-type", &[b'a'; 257])]).inhaltstyp(), None);
        assert_eq!(kopf(&[("content-type", b"a\x7f")]).inhaltstyp(), None);
        assert_eq!(kopf(&[("content-type", b"a\x1f")]).inhaltstyp(), None);
        assert_eq!(
            kopf(&[("content-type", b" ~")]).inhaltstyp(),
            Some(&b" ~"[..])
        );
    }

    #[test]
    fn the_framing_of_an_answer() {
        assert_eq!(kopf(&[]).rahmung(), Some(Rahmung::BisEnde));
        assert_eq!(
            kopf(&[("content-length", b"5")]).rahmung(),
            Some(Rahmung::Laenge(5))
        );
        assert_eq!(
            kopf(&[("content-length", b"5"), ("content-length", b"5")]).rahmung(),
            Some(Rahmung::Laenge(5))
        );
        assert_eq!(
            kopf(&[("content-length", b"5"), ("content-length", b"6")]).rahmung(),
            None
        );
        assert_eq!(kopf(&[("content-length", b"x")]).rahmung(), None);
        assert_eq!(kopf(&[("content-length", b"\xff")]).rahmung(), None);
        assert_eq!(
            kopf(&[
                ("transfer-encoding", b"gzip, Chunked "),
                ("content-length", b"5")
            ])
            .rahmung(),
            Some(Rahmung::Stueckweise)
        );
        assert_eq!(
            kopf(&[
                ("transfer-encoding", b"chunked"),
                ("transfer-encoding", b"gzip")
            ])
            .rahmung(),
            Some(Rahmung::BisEnde)
        );
    }

    #[test]
    fn a_body_by_length() {
        let l = KoerperLeser::new_for_test(&[b"cd", b"efg"], b"ab", Rahmung::Laenge(6));
        assert_eq!(
            alle(l).expect("body"),
            vec![b"ab".to_vec(), b"cd".to_vec(), b"ef".to_vec()]
        );
        let l = KoerperLeser::new_for_test(&[b"x"], b"", Rahmung::Laenge(0));
        assert_eq!(alle(l).expect("body"), Vec::<Vec<u8>>::new());
        let l = KoerperLeser::new_for_test(&[b"ab"], b"", Rahmung::Laenge(3));
        assert_eq!(
            alle(l).map_err(|e| e.kind()),
            Err(io::ErrorKind::UnexpectedEof)
        );
    }

    #[test]
    fn a_body_up_to_the_end() {
        let l = KoerperLeser::new_for_test(&[b"cd", b"e"], b"ab", Rahmung::BisEnde);
        assert_eq!(
            alle(l).expect("body"),
            vec![b"ab".to_vec(), b"cd".to_vec(), b"e".to_vec()]
        );
    }

    #[test]
    fn a_chunked_body_keeps_its_bytes() {
        let l = KoerperLeser::new_for_test(
            &[
                b"3;x=1\r\nabc\r",
                b"\n10\r\n0123456789",
                b"abcdef\r\n0\r\nT: 1\r\n\r\nnie",
            ],
            b"",
            Rahmung::Stueckweise,
        );
        assert_eq!(
            alle(l).expect("body"),
            vec![b"abc".to_vec(), b"0123456789".to_vec(), b"abcdef".to_vec()]
        );
        let l = KoerperLeser::new_for_test(&[b"2\r\nabX\r\n0\r\n\r\n"], b"", Rahmung::Stueckweise);
        assert_eq!(
            alle(l).map_err(|e| e.kind()),
            Err(io::ErrorKind::InvalidData)
        );
        let l = KoerperLeser::new_for_test(&[b"z\r\n"], b"", Rahmung::Stueckweise);
        assert_eq!(
            alle(l).map_err(|e| e.kind()),
            Err(io::ErrorKind::InvalidData)
        );
        let l = KoerperLeser::new_for_test(&[b"2\r\nab"], b"", Rahmung::Stueckweise);
        assert_eq!(
            alle(l).map_err(|e| e.kind()),
            Err(io::ErrorKind::UnexpectedEof)
        );
        let l = KoerperLeser::new_for_test(&[b"2\r\na"], b"", Rahmung::Stueckweise);
        assert_eq!(
            alle(l).map_err(|e| e.kind()),
            Err(io::ErrorKind::UnexpectedEof)
        );
        let lang = vec![b'1'; 4097];
        let l = KoerperLeser::new_for_test(&[&lang], b"", Rahmung::Stueckweise);
        assert_eq!(
            alle(l).map_err(|e| e.kind()),
            Err(io::ErrorKind::InvalidData)
        );
        let mut genau = b"0;".to_vec();
        genau.resize(4096, b'x');
        genau.extend_from_slice(b"\r\n\r\n");
        let l = KoerperLeser::new_for_test(
            &[&genau[..4096], &genau[4096..]],
            b"",
            Rahmung::Stueckweise,
        );
        assert_eq!(
            alle(l).expect("a line of 4096 bytes"),
            Vec::<Vec<u8>>::new()
        );
    }

    #[test]
    fn the_size_of_a_chunk() {
        assert_eq!(groesse(b"ff"), Some(255));
        assert_eq!(groesse(b" A ;x"), Some(10));
        assert_eq!(groesse(b"ffffffffffffffff"), Some(u64::MAX));
        assert_eq!(groesse(b"10000000000000000"), None);
        assert_eq!(groesse(b""), None);
        assert_eq!(groesse(b"g"), None);
    }

    #[test]
    fn finding_bytes() {
        assert_eq!(finde(b"abcabc", b"ca"), Some(2));
        assert_eq!(finde(b"ab", b"abc"), None);
    }

    impl KoerperLeser<Teile> {
        fn new_for_test(teile: &[&[u8]], rest: &[u8], rahmung: Rahmung) -> Self {
            Self::neu(Teile::neu(teile), rest.to_vec(), rahmung)
        }
    }
}
