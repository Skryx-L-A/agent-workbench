//! Token form and the reading of `config.json` of the Copilot CLI. A token is never printed: `Token` has no
//! `Debug`, no `Display` and no `Clone`; the only way out is `as_str`, used once to write the key file.

use serde_json::Value;

/// The fixed placeholder of contract 1 (`gho_` and 36 zeros). It is never a key.
#[must_use]
pub fn platzhalter() -> String {
    format!("gho_{}", "0".repeat(36))
}

/// Top-level field of `config.json` that holds the token (measurement point M1 of the contract, measured with CLI 1.0.88 on
/// 21.11.2026): an object whose values are objects whose values are the token texts (depth object, object, text).
/// The names below it are neither needed nor read.
pub const FELD: &str = "authTokens";

/// Largest `config.json` that is read.
pub const CONFIG_GRENZE: u64 = 1 << 20;

const PRAEFIXE: [&str; 3] = ["gho_", "ghu_", "github_pat_"];

/// A token. No `Debug`, no `Clone`.
pub struct Token(String);

impl Token {
    /// The text, for the one write into the key file.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a read of `config.json` found.
pub enum Fund {
    /// No token in the field.
    Keins,
    /// Exactly one token.
    Eins(Token),
    /// More than one token: the command does not choose.
    Mehrere,
    /// The text is not valid JSON.
    KeinJson,
}

/// Is `text` shaped like a token of the CLI (measurement point M2): prefix `gho_`, `ghu_` or `github_pat_`, then only
/// `[A-Za-z0-9_]`, 20 to 255 characters in all, and not the placeholder.
#[must_use]
pub fn token_form_ok(text: &str) -> bool {
    let Some(praefix) = PRAEFIXE.iter().find(|p| text.starts_with(**p)) else {
        return false;
    };
    let rest = &text[praefix.len()..];
    (20..=255).contains(&text.len())
        && !rest.is_empty()
        && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && text != platzhalter()
}

/// The offset of the JSON text: the leading whole lines that start with `//` (and end with LF) are skipped. Every
/// other text before the first `{` stays in the JSON text, so a parse refuses it.
fn json_ab(text: &[u8]) -> usize {
    let mut ab = 0;
    while text[ab..].starts_with(b"//") {
        match text[ab..].iter().position(|b| *b == b'\n') {
            Some(n) => ab += n + 1,
            None => break,
        }
    }
    ab
}

/// The parsed JSON text after the leading comment lines; `None` for anything that is not valid JSON.
fn lesen(text: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(&text[json_ab(text)..]).ok()
}

/// The non-empty texts of the field in exactly the depth object, object, text.
fn blaetter(wert: &Value) -> Vec<&str> {
    let Some(Value::Object(stufe1)) = wert.get(FELD) else {
        return Vec::new();
    };
    stufe1
        .values()
        .filter_map(|v| match v {
            Value::Object(stufe2) => Some(stufe2.values()),
            _ => None,
        })
        .flatten()
        .filter_map(|v| match v {
            Value::String(s) if !s.is_empty() => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

fn tokenartig(s: &str) -> bool {
    PRAEFIXE.iter().any(|p| s.starts_with(p)) && s.len() >= 20
}

/// How many texts anywhere in the value have the prefix of a token.
fn tokenartige_texte(wert: &Value) -> usize {
    match wert {
        Value::String(s) => usize::from(tokenartig(s)),
        Value::Array(liste) => liste.iter().map(tokenartige_texte).sum(),
        Value::Object(karte) => karte.values().map(tokenartige_texte).sum(),
        _ => 0,
    }
}

/// Reads the token of the measured form (CLI 1.0.88): leading `//` lines, then JSON. The token is the one non-empty
/// text under the field in the depth object, object, text, and no other text of the file has a token prefix.
/// Any other form is `Keins`, `Mehrere` or `KeinJson`, never a guess.
#[must_use]
pub fn token_aus_config(text: &[u8]) -> Fund {
    let Some(wert) = lesen(text) else {
        return Fund::KeinJson;
    };
    let blatt = blaetter(&wert);
    let davon_tokenartig = blatt.iter().filter(|s| tokenartig(s)).count();
    let fremd = tokenartige_texte(&wert) - davon_tokenartig;
    match (blatt.as_slice(), fremd) {
        ([], _) => Fund::Keins,
        ([eins], 0) => Fund::Eins(Token((*eins).to_owned())),
        _ => Fund::Mehrere,
    }
}

/// Does a `config.json` of the real home hold a login? Fail closed: an unreadable text counts as a login (so does
/// any other form than the measured one that is not valid JSON), and so does any string anywhere that has the prefix
/// of a token, even in a field other than the assumed one.
#[must_use]
pub fn config_hat_anmeldung(text: &[u8]) -> bool {
    let Some(wert) = lesen(text) else {
        return true;
    };
    !blaetter(&wert).is_empty() || tokenartige_texte(&wert) > 0
}

impl Token {
    /// Is `inhalt` exactly the token and one LF (the form of the key file)? Compares, shows nothing.
    #[must_use]
    pub fn ist_schluessel(&self, inhalt: &[u8]) -> bool {
        inhalt.strip_suffix(b"\n") == Some(self.0.as_bytes())
    }
}

/// The bytes of `text` without the one leaf of the field (M1) whose value is `token`; every other byte stays as it
/// was, the leading comment lines and the order of all keys included. Removed are the key, the colon, the value and
/// one comma next to them with the blank between; a parent object that becomes empty stays. `None` when the leaf
/// is not exactly one, the field is not exactly once at the top, or the rest still holds a login after
/// `config_hat_anmeldung`, a token read by `token_aus_config` or the bytes of the token: then nothing may be written.
#[must_use]
pub fn ohne_token(text: &[u8], token: &Token) -> Option<Vec<u8>> {
    let (von, bis) = eintrag_stelle(text, token.as_str())?;
    let mut neu = Vec::with_capacity(text.len());
    neu.extend_from_slice(&text[..von]);
    neu.extend_from_slice(&text[bis..]);
    let roh = token.as_str().as_bytes();
    let sauber = matches!(token_aus_config(&neu), Fund::Keins)
        && !config_hat_anmeldung(&neu)
        && !neu.windows(roh.len()).any(|w| w == roh);
    sauber.then_some(neu)
}

/// A span of bytes, `(start, end)`.
type Stelle = (usize, usize);

/// The span to remove for the leaf with `token` in the field (see `ohne_token`).
fn eintrag_stelle(text: &[u8], token: &str) -> Option<Stelle> {
    let mut oben = Leser::neu(text, json_ab(text));
    oben.leer();
    let felder = oben.objekt()?;
    oben.leer();
    if !oben.rest.is_empty() {
        return None;
    }
    let treffer: Vec<&(Stelle, Stelle)> = felder
        .iter()
        .filter(|(k, _)| dekodiert(text, *k).as_deref() == Some(FELD))
        .collect();
    let [(_, wert)] = treffer.as_slice() else {
        return None;
    };
    let mut passend: Vec<(Vec<(Stelle, Stelle)>, usize)> = Vec::new();
    for (_, mitte) in Leser::neu(text, wert.0).objekt()? {
        // A value that is not an object holds no leaf of this depth.
        let Some(blaetter) = Leser::neu(text, mitte.0).objekt() else {
            continue;
        };
        for (n, (_, v)) in blaetter.iter().enumerate() {
            if dekodiert(text, *v).as_deref() == Some(token) {
                passend.push((blaetter.clone(), n));
            }
        }
    }
    let [(eintraege, n)] = passend.as_slice() else {
        return None;
    };
    let n = *n;
    Some(if n + 1 < eintraege.len() {
        (eintraege[n].0.0, eintraege[n + 1].0.0)
    } else if n > 0 {
        (eintraege[n - 1].1.1, eintraege[n].1.1)
    } else {
        (eintraege[n].0.0, eintraege[n].1.1)
    })
}

/// The text of a JSON string at `stelle`, decoded; `None` for anything else.
fn dekodiert(text: &[u8], stelle: Stelle) -> Option<String> {
    serde_json::from_slice::<String>(text.get(stelle.0..stelle.1)?).ok()
}

/// A reader of the spans of a JSON text that `serde_json` has already accepted (so the depth is bounded by its
/// limit). It keeps the byte positions; it decodes nothing itself. It moves only by taking bytes off the front of
/// `rest`, so every loop ends at the end of the text.
struct Leser<'a> {
    t: &'a [u8],
    rest: &'a [u8],
}

impl<'a> Leser<'a> {
    fn neu(t: &'a [u8], ab: usize) -> Leser<'a> {
        Leser {
            t,
            rest: t.get(ab..).unwrap_or_default(),
        }
    }

    fn pos(&self) -> usize {
        self.t.len() - self.rest.len()
    }

    fn naechstes(&self) -> Option<u8> {
        self.rest.first().copied()
    }

    fn leer(&mut self) {
        while let [b' ' | b'\t' | b'\n' | b'\r', danach @ ..] = self.rest {
            self.rest = danach;
        }
    }

    fn zeichen(&mut self, c: u8) -> Option<()> {
        let (erstes, danach) = self.rest.split_first()?;
        if *erstes != c {
            return None;
        }
        self.rest = danach;
        Some(())
    }

    fn text(&mut self) -> Option<Stelle> {
        let start = self.pos();
        self.zeichen(b'"')?;
        loop {
            match self.rest {
                [b'"', danach @ ..] => {
                    self.rest = danach;
                    return Some((start, self.pos()));
                }
                [b'\\', _, danach @ ..] | [_, danach @ ..] => self.rest = danach,
                [] => return None,
            }
        }
    }

    fn wert(&mut self) -> Option<Stelle> {
        let start = self.pos();
        match self.naechstes()? {
            b'"' => {
                self.text()?;
            }
            b'{' => {
                self.objekt()?;
            }
            b'[' => {
                self.zeichen(b'[')?;
                self.leer();
                if self.zeichen(b']').is_none() {
                    loop {
                        self.leer();
                        self.wert()?;
                        self.leer();
                        if self.zeichen(b',').is_none() {
                            self.zeichen(b']')?;
                            break;
                        }
                    }
                }
            }
            _ => {
                while let [c, danach @ ..] = self.rest
                    && !matches!(c, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
                {
                    self.rest = danach;
                }
                if self.pos() == start {
                    return None;
                }
            }
        }
        Some((start, self.pos()))
    }

    /// The entries of an object: span of the key, span of the value.
    fn objekt(&mut self) -> Option<Vec<(Stelle, Stelle)>> {
        self.zeichen(b'{')?;
        self.leer();
        let mut eintraege = Vec::new();
        if self.zeichen(b'}').is_some() {
            return Some(eintraege);
        }
        loop {
            self.leer();
            let k = self.text()?;
            self.leer();
            self.zeichen(b':')?;
            self.leer();
            let v = self.wert()?;
            eintraege.extend([(k, v)]);
            self.leer();
            if self.zeichen(b',').is_none() {
                self.zeichen(b'}')?;
                return Some(eintraege);
            }
        }
    }
}
