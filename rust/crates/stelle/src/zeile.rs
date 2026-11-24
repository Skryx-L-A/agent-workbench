//! The four kinds of line of the point (contract 6, section 7). A line here carries its fields without
//! `fassung`, `vorher` and `zeit`; the shell adds them through [`verketten`] when it writes the lines of one event.

use formate::kette::{Fehler, kanonisch, sha256_hex, zeit_text};
use serde_json::{Value, json};

use crate::start::Sicht;
use crate::{Aufrufer, Code};

/// The end of a request in a line `mcp.schreibaufruf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ende {
    /// Redeemed by the hub after a release.
    Eingeloest,
    /// Refused by a human.
    Abgelehnt,
    /// The deadline passed.
    Abgelaufen,
    /// The option "release per write call" is off for the server.
    OhneFreigabe,
}

impl Ende {
    /// The text of the field `ergebnis`.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Eingeloest => "eingeloest",
            Self::Abgelehnt => "abgelehnt",
            Self::Abgelaufen => "abgelaufen",
            Self::OhneFreigabe => "ohne_freigabe",
        }
    }
}

/// The fields of a line `mcp.schreibaufruf` (section 7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schreibaufruf {
    /// Request identifier.
    pub anfrage: String,
    /// `digest` of the call.
    pub digest: String,
    /// Pseudonym of the deciding human, or `None`.
    pub entscheidend: Option<String>,
    /// End of the request.
    pub ergebnis: Ende,
    /// Pseudonym of the person of the run.
    pub person: String,
    /// Server key.
    pub server: String,
    /// Session identifier.
    pub sitzung: String,
    /// Tool name.
    pub werkzeug: String,
    /// Time of the request in milliseconds.
    pub zeit_anfrage_ms: i64,
    /// Time of the decision in milliseconds, or `None`.
    pub zeit_entscheidung_ms: Option<i64>,
}

/// One line of the point, without `fassung`, `vorher` and `zeit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protokollzeile {
    /// `mcp.schreibaufruf`: one line per request at its end.
    Schreibaufruf(Schreibaufruf),
    /// `sitzung.gestartet`.
    SitzungGestartet(Sicht),
    /// `sitzung.abgelehnt` with the code.
    SitzungAbgelehnt(Code, Sicht),
    /// `vorgang.abgelehnt`: the request identifier of the input (when it holds the form), the caller, the code
    /// and the name of the operation.
    VorgangAbgelehnt {
        /// Request identifier of the input, or `None`.
        anfrage: Option<String>,
        /// Kind of the caller.
        aufrufer: Aufrufer,
        /// Code of the refusal.
        grund: Code,
        /// Name of the operation.
        vorgang: &'static str,
    },
}

/// The fields of a start line.
fn sitzung_zeile(aktionsname: &str, ergebnis: &str, grund: Option<&str>, s: &Sicht) -> Value {
    json!({
        "aktionsname": aktionsname,
        "ergebnis": ergebnis,
        "grund": grund,
        "harness": s.harness,
        "kunde": s.kunde,
        "route": s.route,
        "sitzung": s.sitzung,
    })
}

impl Protokollzeile {
    /// The value of the field `aktionsname`.
    #[must_use]
    pub fn aktionsname(&self) -> &'static str {
        match self {
            Self::Schreibaufruf(_) => "mcp.schreibaufruf",
            Self::SitzungGestartet(_) => "sitzung.gestartet",
            Self::SitzungAbgelehnt(..) => "sitzung.abgelehnt",
            Self::VorgangAbgelehnt { .. } => "vorgang.abgelehnt",
        }
    }

    /// The fields of the line without `fassung`, `vorher` and `zeit`.
    ///
    /// # Errors
    /// `zeit_ungueltig` when a time of the line lies outside of 1970 up to 9999.
    pub fn felder(&self) -> Result<Value, Fehler> {
        Ok(match self {
            Self::Schreibaufruf(s) => json!({
                "aktionsname": self.aktionsname(),
                "anfrage": s.anfrage,
                "digest": s.digest,
                "entscheidend": s.entscheidend,
                "ergebnis": s.ergebnis.text(),
                "person": s.person,
                "server": s.server,
                "sitzung": s.sitzung,
                "werkzeug": s.werkzeug,
                "zeit_anfrage": zeit_text(s.zeit_anfrage_ms)?,
                "zeit_entscheidung": s.zeit_entscheidung_ms.map(zeit_text).transpose()?,
            }),
            Self::SitzungGestartet(sicht) => {
                sitzung_zeile(self.aktionsname(), "gestartet", None, sicht)
            }
            Self::SitzungAbgelehnt(code, sicht) => {
                sitzung_zeile(self.aktionsname(), "abgelehnt", Some(code.code()), sicht)
            }
            Self::VorgangAbgelehnt {
                anfrage,
                aufrufer,
                grund,
                vorgang,
            } => json!({
                "aktionsname": self.aktionsname(),
                "anfrage": anfrage,
                "aufrufer": aufrufer.text(),
                "ergebnis": "abgelehnt",
                "grund": grund.code(),
                "vorgang": vorgang,
            }),
        })
    }

    /// The bytes of the line (RFC 8785, without the line feed) with `fassung` 2, `vorher` and `zeit`.
    ///
    /// # Errors
    /// `zeit_ungueltig` when a time lies outside of 1970 up to 9999.
    pub fn text(&self, vorher: &str, zeit_ms: i64) -> Result<String, Fehler> {
        let mut wert = self.felder()?;
        if let Value::Object(felder) = &mut wert {
            felder.insert("fassung".to_owned(), json!(2));
            felder.insert("vorher".to_owned(), json!(vorher));
            felder.insert("zeit".to_owned(), json!(zeit_text(zeit_ms)?));
        }
        kanonisch(&wert)
    }
}

/// The lines of one event as one sequence of texts: the first line follows `vorher`, every further line the
/// sha256 of the line before it. The shell writes them in one write call; the sha256 of the last text is the
/// `vorher` of the next event.
///
/// # Errors
/// `zeit_ungueltig` when a time lies outside of 1970 up to 9999.
pub fn verketten(
    zeilen: &[Protokollzeile],
    vorher: &str,
    zeit_ms: i64,
) -> Result<Vec<String>, Fehler> {
    let mut letzte = vorher.to_owned();
    let mut texte = Vec::with_capacity(zeilen.len());
    for zeile in zeilen {
        let text = zeile.text(&letzte, zeit_ms)?;
        letzte = sha256_hex(text.as_bytes());
        texte.push(text);
    }
    Ok(texte)
}
