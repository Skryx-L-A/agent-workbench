//! The form of a request (BR3, check 1): exactly this shape, otherwise 400, or 414 and 431 for sizes.

use crate::fehler::Abgelehnt;

/// The request line and the heads together, with the closing empty line, have at most this many bytes.
pub(crate) const MAX_KOPF: usize = 8192;
/// At most this many head lines (without the request line).
pub(crate) const MAX_KOPFZEILEN: usize = 64;
/// The target has at most this many bytes.
pub(crate) const MAX_ZIEL: usize = 2048;

const FORM: Abgelehnt = Abgelehnt {
    status: 400,
    grund: "anfrage_form",
};

/// The heads that may occur at most once.
const EINFACH: [&str; 7] = [
    "host",
    "authorization",
    "origin",
    "content-length",
    "content-type",
    "x-verbindung",
    "transfer-encoding",
];

/// A request that has the right form: method, target and the heads (names in lower case, values as bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Anfrage {
    pub(crate) methode: String,
    pub(crate) ziel: String,
    koepfe: Vec<(String, Vec<u8>)>,
}

impl Anfrage {
    /// The value of a head by its lower-case name.
    pub(crate) fn kopf(&self, name: &str) -> Option<&[u8]> {
        self.koepfe
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, w)| w.as_slice())
    }
}

/// Splits at every `\r\n`, from the left, without overlap. `None` when a line ends in a bare `\n` (no `\r` before it): such a
/// line holds a `\n` and the form check refuses it. The split is made at each `\n`, so no arithmetic on positions is needed.
fn zerlegen(kopf: &[u8]) -> Option<Vec<&[u8]>> {
    let stuecke: Vec<&[u8]> = kopf.split(|&b| b == b'\n').collect();
    let (letztes, vorne) = stuecke.split_last()?;
    let mut zeilen = Vec::with_capacity(stuecke.len());
    for z in vorne {
        zeilen.push(z.strip_suffix(b"\r")?);
    }
    zeilen.push(letztes);
    Some(zeilen)
}

fn ist_token_zeichen(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+.^_`|~-".contains(&b)
}

fn ohne_rand(w: &[u8]) -> &[u8] {
    let ist = |b: &u8| matches!(b, b' ' | b'\t');
    let von = w.iter().position(|b| !ist(b)).unwrap_or(w.len());
    let bis = w.iter().rposition(|b| !ist(b)).map_or(von, |p| p + 1);
    &w[von..bis]
}

/// Checks the head (everything before the closing empty line) and returns the request.
pub(crate) fn lesen(kopf: &[u8]) -> Result<Anfrage, Abgelehnt> {
    let zeilen = zerlegen(kopf).ok_or(FORM)?;
    if zeilen
        .iter()
        .any(|z| z.iter().any(|b| matches!(b, b'\r' | 0)))
    {
        return Err(FORM);
    }
    let erste = zeilen.first().copied().unwrap_or_default();
    let teile: Vec<&str> = std::str::from_utf8(erste)
        .ok()
        .filter(|s| s.is_ascii())
        .ok_or(FORM)?
        .split(' ')
        .collect();
    let [methode, ziel, version] = teile[..] else {
        return Err(FORM);
    };
    if ziel.len() > MAX_ZIEL {
        return Err(Abgelehnt {
            status: 414,
            grund: "ziel_zu_lang",
        });
    }
    if version != "HTTP/1.1" {
        return Err(FORM);
    }
    if !(1..=16).contains(&methode.len()) || !methode.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(FORM);
    }
    if !ziel.starts_with('/') || !ziel.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(FORM);
    }
    if zeilen.len() - 1 > MAX_KOPFZEILEN {
        return Err(Abgelehnt {
            status: 431,
            grund: "kopf_zu_viele",
        });
    }
    let mut koepfe: Vec<(String, Vec<u8>)> = Vec::new();
    for z in &zeilen[1..] {
        if matches!(z.first(), Some(b' ' | b'\t')) {
            return Err(FORM);
        }
        let p = z.iter().position(|&b| b == b':').ok_or(FORM)?;
        let name = &z[..p];
        let wert = ohne_rand(&z[p + 1..]);
        if name.is_empty() || !name.iter().all(|&b| ist_token_zeichen(b)) {
            return Err(FORM);
        }
        if wert.iter().any(|&b| (b < 0x20 && b != b'\t') || b == 0x7f) {
            return Err(FORM);
        }
        let name = String::from_utf8_lossy(name).to_ascii_lowercase();
        if EINFACH.contains(&name.as_str()) && koepfe.iter().any(|(n, _)| *n == name) {
            return Err(FORM);
        }
        koepfe.push((name, wert.to_vec()));
    }
    let anfrage = Anfrage {
        methode: methode.to_string(),
        ziel: ziel.to_string(),
        koepfe,
    };
    if anfrage.kopf("transfer-encoding").is_some() {
        return Err(FORM);
    }
    let laenge = anfrage.kopf("content-length");
    if let Some(l) = laenge
        && !((1..=10).contains(&l.len()) && l.iter().all(u8::is_ascii_digit))
    {
        return Err(FORM);
    }
    if anfrage.methode != "POST" && laenge.is_some_and(|l| l != b"0") {
        return Err(FORM);
    }
    Ok(anfrage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kopf(zeilen: &[&str]) -> Vec<u8> {
        zeilen.join("\r\n").into_bytes()
    }

    fn form(z: &[&str]) -> Result<Anfrage, Abgelehnt> {
        lesen(&kopf(z))
    }

    fn status(z: &[&str]) -> u16 {
        form(z).expect_err("must be rejected").status
    }

    #[test]
    fn a_good_request_is_read() {
        let a = form(&[
            "GET /strom HTTP/1.1",
            "Host: 127.0.0.1:1",
            "X-Y:  a b \t",
            "x-y2:",
        ])
        .expect("ok");
        assert_eq!((a.methode.as_str(), a.ziel.as_str()), ("GET", "/strom"));
        assert_eq!(a.kopf("host"), Some(&b"127.0.0.1:1"[..]));
        assert_eq!(a.kopf("x-y"), Some(&b"a b"[..]));
        assert_eq!(a.kopf("x-y2"), Some(&b""[..]));
        assert_eq!(a.kopf("fehlt"), None);
        let b = form(&["POST /a HTTP/1.1", "Content-Length: 0000000012"]).expect("ok");
        assert_eq!(b.kopf("content-length"), Some(&b"0000000012"[..]));
        assert!(form(&["GET /a HTTP/1.1", "Content-Length: 0"]).is_ok());
        assert!(form(&["GET /a HTTP/1.1"]).is_ok());
    }

    #[test]
    fn the_request_line_has_one_space_between_three_parts() {
        for z in [
            "GET  / HTTP/1.1",
            "GET / HTTP/1.1 ",
            " GET / HTTP/1.1",
            "GET /",
            "GET",
            "",
            "GET /\tx HTTP/1.1",
            "GET / HTTP/1.0",
            "GET / HTTP/2",
            "GET / http/1.1",
            "GET / HTTP/1.1x",
            "get / HTTP/1.1",
            "Get / HTTP/1.1",
            "G3T / HTTP/1.1",
            " / HTTP/1.1",
            "ABCDEFGHIJKLMNOPQ / HTTP/1.1",
            "GET x HTTP/1.1",
            "GET http://a/ HTTP/1.1",
            "GET * HTTP/1.1",
            "GET /\u{e4} HTTP/1.1",
            "GET /a\u{7f} HTTP/1.1",
        ] {
            assert_eq!(status(&[z]), 400, "{z:?}");
        }
        assert!(form(&["ABCDEFGHIJKLMNOP / HTTP/1.1"]).is_ok());
        assert!(form(&["A / HTTP/1.1"]).is_ok());
        assert!(form(&["GET /!~?a=1&b=%20#x HTTP/1.1"]).is_ok());
        assert_eq!(
            lesen(b"GET /\xc3\xa4 HTTP/1.1")
                .expect_err("high byte")
                .status,
            400
        );
    }

    #[test]
    fn the_target_limit_is_2048_and_comes_before_the_version() {
        let lang = format!("GET /{} HTTP/1.1", "a".repeat(2047));
        assert!(form(&[&lang]).is_ok());
        let zu_lang = format!("GET /{} HTTP/1.1", "a".repeat(2048));
        assert_eq!(
            form(&[&zu_lang]).expect_err("long"),
            Abgelehnt {
                status: 414,
                grund: "ziel_zu_lang"
            }
        );
        let alt = format!("GET /{} HTTP/1.0", "a".repeat(2048));
        assert_eq!(status(&[&alt]), 414);
    }

    #[test]
    fn the_head_line_limit_is_64() {
        let mut z = vec!["GET / HTTP/1.1".to_string()];
        z.extend((0..64).map(|i| format!("X-{i}: v")));
        let r: Vec<&str> = z.iter().map(String::as_str).collect();
        assert!(form(&r).is_ok());
        z.push("X-64: v".to_string());
        let r: Vec<&str> = z.iter().map(String::as_str).collect();
        assert_eq!(
            form(&r).expect_err("many"),
            Abgelehnt {
                status: 431,
                grund: "kopf_zu_viele"
            }
        );
        // the limit is checked before the lines are read: a broken line past the limit is still 431
        z.push("kaputt".to_string());
        let r: Vec<&str> = z.iter().map(String::as_str).collect();
        assert_eq!(status(&r), 431);
    }

    #[test]
    fn broken_head_lines_are_400() {
        for z in [
            "X: a\u{0}b",
            "X: a\u{1}b",
            "X: a\u{1f}b",
            "X: a\u{7f}b",
            "X: a\nb",
            "X: a\rb",
            "X : a",
            " X: a",
            "\tX: a",
            "X a",
            ": a",
            "X Y: a",
            "X\u{e4}: a",
            "X(: a",
            "X\"y: a",
            "X/y: a",
            "X;y: a",
            "X@y: a",
            "X[y: a",
            "X{y: a",
            "X=y: a",
            "X<y: a",
            "X,y: a",
            "X?y: a",
            "X\\y: a",
        ] {
            assert_eq!(status(&["GET / HTTP/1.1", z]), 400, "{z:?}");
        }
        assert_eq!(
            lesen(b"GET / HTTP/1.1\r\nX: a\xc3\xa4b").map(|a| a.koepfe.len()),
            Ok(1)
        );
        assert_eq!(
            lesen(b"GET / HTTP/1.1\r\nX: a\r\r\nb: c")
                .expect_err("lone CR")
                .status,
            400
        );
        for name in ["!#$%&'*+.^_`|~-", "A9z"] {
            assert!(
                form(&["GET / HTTP/1.1", &format!("{name}: v")]).is_ok(),
                "{name}"
            );
        }
        assert!(form(&["GET / HTTP/1.1", "X: a\tb"]).is_ok());
    }

    #[test]
    fn single_heads_must_not_repeat_but_others_may() {
        for n in [
            "Host",
            "authorization",
            "ORIGIN",
            "Content-Length",
            "Content-Type",
            "X-Verbindung",
        ] {
            assert_eq!(
                status(&[
                    "POST / HTTP/1.1",
                    &format!("{n}: 1"),
                    &format!("{}: 1", n.to_lowercase())
                ]),
                400,
                "{n}"
            );
        }
        let a = form(&["GET / HTTP/1.1", "X-A: 1", "x-a: 2"]).expect("repeatable");
        assert_eq!(a.kopf("x-a"), Some(&b"2"[..]));
    }

    #[test]
    fn transfer_encoding_in_any_form_is_400() {
        for v in ["chunked", "", "identity", "gzip, chunked"] {
            assert_eq!(
                status(&["POST / HTTP/1.1", &format!("Transfer-Encoding: {v}")]),
                400
            );
        }
    }

    #[test]
    fn content_length_is_one_to_ten_digits_and_zero_for_other_methods() {
        for v in ["", "a", "-1", "+1", "1 2", "12345678901", "1.0", "\u{e4}"] {
            assert_eq!(
                status(&["POST / HTTP/1.1", &format!("Content-Length: {v}")]),
                400,
                "{v:?}"
            );
        }
        assert!(form(&["POST / HTTP/1.1", "Content-Length: 1234567890"]).is_ok());
        assert!(form(&["POST / HTTP/1.1", "Content-Length: 0"]).is_ok());
        for v in ["1", "00", "5", "0 0"] {
            assert_eq!(
                status(&["GET / HTTP/1.1", &format!("Content-Length: {v}")]),
                400,
                "{v:?}"
            );
        }
        assert_eq!(status(&["PUT / HTTP/1.1", "Content-Length: 1"]), 400);
        assert!(form(&["PUT / HTTP/1.1", "Content-Length: 0"]).is_ok());
    }

    #[test]
    fn splitting_is_left_to_right_without_overlap() {
        assert_eq!(zerlegen(b"a\r\nb"), Some(vec![&b"a"[..], b"b"]));
        assert_eq!(zerlegen(b"a\r\r\nb"), Some(vec![&b"a\r"[..], b"b"]));
        assert_eq!(zerlegen(b""), Some(vec![&b""[..]]));
        assert_eq!(zerlegen(b"\r\n"), Some(vec![&b""[..], b""]));
        assert_eq!(zerlegen(b"a\r\nb\r\nc"), Some(vec![&b"a"[..], b"b", b"c"]));
        assert_eq!(zerlegen(b"a\nb"), None);
        assert_eq!(zerlegen(b"a\r\nb\nc"), None);
        assert_eq!(zerlegen(b"\n"), None);
        assert_eq!(zerlegen(b"a\r"), Some(vec![&b"a\r"[..]]));
        assert_eq!(ohne_rand(b" \t a b\t "), b"a b");
        assert_eq!(ohne_rand(b" \t "), b"");
        assert_eq!(ohne_rand(b""), b"");
        assert_eq!(ohne_rand(b"a"), b"a");
    }

    /// A fixed stream of pseudo-random bytes (xorshift, fixed seed) against the checker: never a panic, always a 400 or a
    /// size answer, or a good request.
    #[test]
    fn a_fixed_random_stream_never_panics() {
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut weiter = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let stuecke: [&[u8]; 8] = [
            b"GET ",
            b" HTTP/1.1",
            b"\r\n",
            b"Host: ",
            b": ",
            b"/",
            b"\xff",
            b"\0",
        ];
        for runde in 0..3000 {
            let n = usize::try_from(weiter() % 200).expect("small");
            let mut kopf = Vec::new();
            for _ in 0..n {
                let w = weiter();
                if w % 3 == 0 {
                    kopf.extend_from_slice(stuecke[usize::try_from((w >> 8) % 8).expect("small")]);
                } else {
                    kopf.push(w.to_le_bytes()[3]);
                }
            }
            match lesen(&kopf) {
                Ok(_) => {}
                Err(a) => assert!(
                    [400, 414, 431].contains(&a.status),
                    "round {runde}: {}",
                    a.status
                ),
            }
        }
    }
}
