//! The order of the stages H1 to H12 for one connection (contract 15.12.3). Exactly one code, the one of the first
//! stage that fails. H1 to H5 run before one byte of the request is read: the request is read only through the closure
//! `lesen`, and only after H5.

use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};

use crate::code::Code;
use crate::ersetzen;
use crate::form;
use crate::json;
use crate::konten::Konten;
use crate::sys::{Art, Dateien, Meta};

/// Folder of the company default (contract 15.12.4).
pub const ORDNER: &str = "/etc/werkbank/vorgabe";
/// Name of the company default in its folder.
pub const NAME: &str = "firmenvorgabe.json";
/// Name of the system user of the helper.
pub const HELFERNUTZER: &str = "werkbank-vorgabe";
/// Most bytes of a request (H6).
pub const GRENZE_ANFRAGE: usize = 1_048_576;
/// Most bytes of the valid file; a longer file is a load error (H4).
pub const GRENZE_DATEI: usize = 1_048_576;
/// Write permission for group or others (H3).
pub const UNSICHER: u32 = 0o022;
/// uid of root, the owner of every parent folder.
pub const ROOT: u32 = 0;

/// Where the file lies and who owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orte {
    /// The folder of the helper.
    pub ordner: PathBuf,
    /// The name of the file in the folder.
    pub name: String,
    /// Every parent folder that must belong to root without `& 022`, from the nearest to `/`.
    pub eltern: Vec<PathBuf>,
    /// uid of `werkbank-vorgabe`, the owner of folder and file.
    pub helfer_uid: u32,
}

/// Every parent of `pfad` up to `/`, the nearest first.
#[must_use]
pub fn eltern_von(pfad: &Path) -> Vec<PathBuf> {
    pfad.ancestors().skip(1).map(Path::to_path_buf).collect()
}

impl Orte {
    /// The fixed places of the delivered program with the uid of `werkbank-vorgabe`.
    #[must_use]
    pub fn fest(helfer_uid: u32) -> Orte {
        Orte {
            ordner: PathBuf::from(ORDNER),
            name: NAME.to_owned(),
            eltern: eltern_von(Path::new(ORDNER)),
            helfer_uid,
        }
    }
}

/// The measured peer of one connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partner {
    /// uid from `SO_PEERCRED`, `None` if it cannot be determined.
    pub uid: Option<u32>,
    /// `true` if the peer runs in the unit of a run, or if that cannot be checked (fail closed).
    pub eingeschlossen: bool,
}

/// Why the request could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lesefehler {
    /// More than [`GRENZE_ANFRAGE`] bytes (H6).
    ZuGross,
    /// Not complete within the deadline (H7).
    Frist,
    /// The connection broke (counts as H8, the request has no form).
    Abbruch,
}

/// The outcome of one request: a code, or the sha256 of the new bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ergebnis {
    /// The code of the first failed stage, `None` when replaced.
    pub code: Option<Code>,
    /// sha256 (hex, lower case) of the new bytes, only without a code.
    pub sha256: Option<String>,
}

impl Ergebnis {
    /// The answer to the client: one JSON object with exactly `code` and `sha256`, and one line end.
    #[must_use]
    pub fn antwort(&self) -> Vec<u8> {
        let code = self
            .code
            .map_or_else(|| "null".to_owned(), |c| format!("\"{}\"", c.text()));
        let sha = self
            .sha256
            .as_ref()
            .map_or_else(|| "null".to_owned(), |s| format!("\"{s}\""));
        format!("{{\"code\": {code}, \"sha256\": {sha}}}\n").into_bytes()
    }

    /// The one line to the journal: no account, no uid, no content.
    #[must_use]
    pub fn zeile(&self) -> String {
        match self.code {
            None => "werkbank-vorgabe: ersetzt".to_owned(),
            Some(c) => format!("werkbank-vorgabe: abgelehnt: {}", c.text()),
        }
    }
}

fn sicher(m: &Meta, art: Art, eigner: u32) -> bool {
    m.art == art && m.uid == eigner && m.modus & UNSICHER == 0
}

/// H3: every parent, the folder and the file, the last two at their open descriptors. Returns the open folder and the
/// open file.
///
/// # Errors
/// [`Code::DateiUnsicher`].
pub fn sicher_oeffnen<D: Dateien>(orte: &Orte, d: &D) -> Result<(OwnedFd, OwnedFd), Code> {
    for e in &orte.eltern {
        let m = d.lstat(e).map_err(|_| Code::DateiUnsicher)?;
        if !sicher(&m, Art::Ordner, ROOT) {
            return Err(Code::DateiUnsicher);
        }
    }
    let ordner = d
        .ordner_oeffnen(&orte.ordner)
        .map_err(|_| Code::DateiUnsicher)?;
    let m = d.fstat(ordner.as_fd()).map_err(|_| Code::DateiUnsicher)?;
    if !sicher(&m, Art::Ordner, orte.helfer_uid) {
        return Err(Code::DateiUnsicher);
    }
    let datei = d
        .oeffnen(ordner.as_fd(), &orte.name)
        .map_err(|_| Code::DateiUnsicher)?;
    let m = d.fstat(datei.as_fd()).map_err(|_| Code::DateiUnsicher)?;
    if !sicher(&m, Art::Datei, orte.helfer_uid) {
        return Err(Code::DateiUnsicher);
    }
    Ok((ordner, datei))
}

fn ablauf<D, K, L>(orte: &Orte, d: &D, k: &K, partner: Partner, lesen: L) -> Result<String, Code>
where
    D: Dateien,
    K: Konten,
    L: FnOnce() -> Result<Vec<u8>, Lesefehler>,
{
    // H1
    let uid = partner.uid.ok_or(Code::PartnerUnbekannt)?;
    if k.konto(uid).is_none() {
        return Err(Code::PartnerUnbekannt);
    }
    // H2
    if partner.eingeschlossen {
        return Err(Code::PartnerEingeschlossen);
    }
    // H3
    let (ordner, datei) = sicher_oeffnen(orte, d)?;
    // H4
    let alt = d
        .lesen(datei.as_fd(), GRENZE_DATEI + 1)
        .map_err(|_| Code::Ladefehler)?;
    drop(datei);
    if alt.len() > GRENZE_DATEI {
        return Err(Code::Ladefehler);
    }
    let alt_wert = json::streng_lesen(&alt).ok_or(Code::Ladefehler)?;
    let admins = form::admins_der_vorgabe(&alt_wert).ok_or(Code::Ladefehler)?;
    // H5, against the valid file
    if !admins.iter().any(|a| k.uid(a) == Some(uid)) {
        return Err(Code::PartnerNichtAdmin);
    }
    // H6 to H8: only now the first byte of the request
    let roh = lesen().map_err(|f| match f {
        Lesefehler::ZuGross => Code::AnfrageZuGross,
        Lesefehler::Frist => Code::AnfrageFrist,
        Lesefehler::Abbruch => Code::AnfrageUngueltig,
    })?;
    if roh.len() > GRENZE_ANFRAGE {
        return Err(Code::AnfrageZuGross);
    }
    let anfrage = json::streng_lesen(&roh)
        .and_then(form::anfrage)
        .ok_or(Code::AnfrageUngueltig)?;
    // H9
    if anfrage.basis != formate::kette::sha256_hex(&alt) {
        return Err(Code::Veraltet);
    }
    // H10
    if !form::nur_leseklassen_anders(&alt_wert, &anfrage.inhalt) {
        return Err(Code::AusserLeseklassen);
    }
    // H11 and H12
    let neu = json::bytes_der(&anfrage.inhalt);
    ersetzen::ersetzen(d, ordner.as_fd(), &orte.name, &alt, &neu)?;
    Ok(formate::kette::sha256_hex(&neu))
}

/// One request from the measured `partner`. `lesen` reads the request; it is called only after H5, at most once.
pub fn bearbeiten<D, K, L>(orte: &Orte, d: &D, k: &K, partner: Partner, lesen: L) -> Ergebnis
where
    D: Dateien,
    K: Konten,
    L: FnOnce() -> Result<Vec<u8>, Lesefehler>,
{
    match ablauf(orte, d, k, partner, lesen) {
        Ok(sha) => Ergebnis {
            code: None,
            sha256: Some(sha),
        },
        Err(c) => Ergebnis {
            code: Some(c),
            sha256: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feste_orte_mit_allen_eltern() {
        let o = Orte::fest(990);
        assert_eq!(o.ordner, PathBuf::from("/etc/werkbank/vorgabe"));
        assert_eq!(o.name, "firmenvorgabe.json");
        assert_eq!(
            o.eltern,
            vec![
                PathBuf::from("/etc/werkbank"),
                PathBuf::from("/etc"),
                PathBuf::from("/")
            ]
        );
        assert_eq!(o.helfer_uid, 990);
        assert_eq!(
            (GRENZE_ANFRAGE, GRENZE_DATEI, UNSICHER, ROOT),
            (1_048_576, 1_048_576, 0o022, 0)
        );
        assert_eq!(HELFERNUTZER, "werkbank-vorgabe");
    }

    #[test]
    fn antwort_und_zeile() {
        let ok = Ergebnis {
            code: None,
            sha256: Some("ab".into()),
        };
        assert_eq!(ok.antwort(), b"{\"code\": null, \"sha256\": \"ab\"}\n");
        assert_eq!(ok.zeile(), "werkbank-vorgabe: ersetzt");
        let nein = Ergebnis {
            code: Some(Code::PartnerNichtAdmin),
            sha256: None,
        };
        assert_eq!(
            nein.antwort(),
            b"{\"code\": \"firmenvorgabe.partner_nicht_admin\", \"sha256\": null}\n"
        );
        assert_eq!(
            nein.zeile(),
            "werkbank-vorgabe: abgelehnt: firmenvorgabe.partner_nicht_admin"
        );
    }

    #[test]
    fn sicher_verlangt_art_eigner_und_modus() {
        let m = |art, uid, modus| Meta {
            art,
            uid,
            modus,
            ino: 1,
        };
        assert!(sicher(&m(Art::Datei, 5, 0o644), Art::Datei, 5));
        assert!(sicher(&m(Art::Datei, 5, 0o755), Art::Datei, 5));
        assert!(!sicher(&m(Art::Datei, 5, 0o664), Art::Datei, 5), "group");
        assert!(!sicher(&m(Art::Datei, 5, 0o646), Art::Datei, 5), "others");
        assert!(!sicher(&m(Art::Datei, 6, 0o644), Art::Datei, 5), "owner");
        assert!(!sicher(&m(Art::Verweis, 5, 0o644), Art::Datei, 5), "link");
        assert!(!sicher(&m(Art::Ordner, 5, 0o755), Art::Datei, 5), "kind");
    }
}
