//! The line `modellweg.aufruf` (contract 8, section 8) in the chain of contract 5, source `gateway`.
//!
//! This is the only module that calls the crate `formate`: the line is built with `kanonisch` and `zeit_text`, and it is
//! appended with the general append over months (`anhaengen_zeile`, contract 5, section 5.3), which sets `fassung`,
//! `vorher` and `zeit` under the lock of the source. A change of that interface touches only this file.

use std::path::Path;

use formate::kette::{self, Fehler};
use serde_json::{Map, Value};

use crate::Betriebsart;
use crate::aufruf::{kennung_ok, modell_ok};
use crate::pfad::pfad_feld_ok;

/// The name of the line type.
pub const AKTIONSNAME: &str = "modellweg.aufruf";
/// The source of the chain: files `gateway-<JJJJ-MM>.jsonl` in the protocol folder.
pub const QUELLE: &str = "gateway";
const U32: u64 = 4_294_967_295;

/// The fields of one line before the chain (`vorher`) and the time text are added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modellweg {
    /// Time of the line in milliseconds since 1970.
    pub zeit_ms: i64,
    /// The mode of the process (`openai-lokal`).
    pub betriebsart: String,
    /// The session of the call.
    pub sitzung: String,
    /// The route of the call.
    pub route: String,
    /// Always the model of the call, never the requested one.
    pub modell: String,
    /// `weitergereicht`, `abgelehnt`, `ziel_fehler` or `leck_gesperrt`.
    pub ergebnis: String,
    /// `None` for `weitergereicht`, else the code that fits the result.
    pub grund: Option<String>,
    /// The status the harness got (100 to 599).
    pub status: u32,
    /// `usage.prompt_tokens` of the target, only for `weitergereicht`.
    pub token_ein: Option<u64>,
    /// `usage.completion_tokens` of the target, only for `weitergereicht`.
    pub token_aus: Option<u64>,
    /// The limited path of the request (section 8), only for a rejection and for a `GET`; else `None`.
    pub pfad: Option<String>,
    /// The host of the bound target; in this mode always `None`, so both forms carry the same fields.
    pub host: Option<String>,
}

/// The codes that fit a result.
fn gruende(ergebnis: &str) -> Option<&'static [Option<&'static str>]> {
    match ergebnis {
        "weitergereicht" => Some(&[None]),
        "abgelehnt" => Some(&[
            Some("mw.anfrage_unlesbar"),
            Some("mw.ausgelastet"),
            Some("mw.pfad_unbekannt"),
            Some("mw.methode_falsch"),
            Some("mw.chunked_nicht_unterstuetzt"),
            Some("mw.laenge_fehlt"),
            Some("mw.koerper_zu_gross"),
            Some("mw.json_ungueltig"),
            Some("mw.modell_nicht_erlaubt"),
            Some("mw.zaehlung_fehler"),
        ]),
        "ziel_fehler" => Some(&[Some("mw.ziel_nicht_erreichbar"), Some("mw.ziel_frist")]),
        "leck_gesperrt" => Some(&[Some("mw.leck_gesperrt")]),
        _ => None,
    }
}

/// The checks of the fields in the order of the contract, without `vorher` and the time.
fn pruefe(m: &Modellweg) -> Result<(), Fehler> {
    let feld = |gut: bool, name: &'static str| {
        if gut {
            Ok(())
        } else {
            Err(Fehler::FeldUngueltig(name))
        }
    };
    feld(
        m.betriebsart == Betriebsart::OpenaiLokal.name(),
        "betriebsart",
    )?;
    feld(kennung_ok(&m.route), "route")?;
    feld(kennung_ok(&m.sitzung), "sitzung")?;
    feld(modell_ok(&m.modell), "modell")?;
    let gruende = gruende(&m.ergebnis).ok_or(Fehler::FeldUngueltig("ergebnis"))?;
    feld(gruende.contains(&m.grund.as_deref()), "grund")?;
    feld((100..=599).contains(&m.status), "status")?;
    let weiter = m.ergebnis == "weitergereicht";
    let token_gut = |t: Option<u64>| t.is_none_or(|n| weiter && n <= U32);
    feld(token_gut(m.token_ein), "token_ein")?;
    feld(
        token_gut(m.token_aus) && m.token_ein.is_some() == m.token_aus.is_some(),
        "token_aus",
    )?;
    // Only a rejection carries a path here: a `GET` never passes stage 4 or 5 of this mode (section 10).
    feld(
        m.pfad
            .as_deref()
            .is_none_or(|p| pfad_feld_ok(p) && m.ergebnis == "abgelehnt"),
        "pfad",
    )?;
    feld(m.host.is_none(), "host")
}

/// The object of the line without `fassung`, `vorher` and `zeit`: the closed form of `openai-lokal`.
fn objekt(m: &Modellweg) -> Map<String, Value> {
    let mut o = Map::new();
    o.insert("aktionsname".to_owned(), AKTIONSNAME.into());
    o.insert("betriebsart".to_owned(), m.betriebsart.clone().into());
    o.insert("ergebnis".to_owned(), m.ergebnis.clone().into());
    o.insert("grund".to_owned(), m.grund.clone().into());
    o.insert("host".to_owned(), m.host.clone().into());
    o.insert("modell".to_owned(), m.modell.clone().into());
    o.insert("pfad".to_owned(), m.pfad.clone().into());
    o.insert("route".to_owned(), m.route.clone().into());
    o.insert("sitzung".to_owned(), m.sitzung.clone().into());
    o.insert("status".to_owned(), m.status.into());
    o.insert("token_aus".to_owned(), m.token_aus.into());
    o.insert("token_ein".to_owned(), m.token_ein.into());
    o
}

/// The bytes of the line (RFC 8785, without line feed) after the line `vorher`.
///
/// # Errors
/// `feld_ungueltig:<feld>` in the order `betriebsart`, `route`, `sitzung`, `modell`, `ergebnis`, `grund`, `status`,
/// `token_ein`, `token_aus`, `pfad`, `host`, `vorher`; then `zeit_ungueltig`.
pub fn zeile_modellweg(m: &Modellweg, vorher: &str) -> Result<String, Fehler> {
    pruefe(m)?;
    if !hex64(vorher) {
        return Err(Fehler::FeldUngueltig("vorher"));
    }
    let zeit = kette::zeit_text(m.zeit_ms)?;
    let mut o = objekt(m);
    o.insert("fassung".to_owned(), 2.into());
    o.insert("vorher".to_owned(), vorher.into());
    o.insert("zeit".to_owned(), zeit.into());
    kette::kanonisch(&Value::Object(o))
}

/// Appends the line to the chain of the source `gateway` in `ordner` (the month file follows from the time).
/// Returns the sha256 of the line.
///
/// # Errors
/// The field errors of [`zeile_modellweg`] (without `vorher`), then every error of `anhaengen_zeile` (contract 5).
pub fn anhaengen_modellweg(ordner: &Path, m: &Modellweg) -> Result<String, Fehler> {
    pruefe(m)?;
    kette::anhaengen_zeile(ordner, QUELLE, m.zeit_ms, &Value::Object(objekt(m)))
}

/// The start line `probelauf.zaehlung` (contract 8, section 9) is the shared line kind of `formate`.
pub use formate::kette::{Probelauf, zeile_probelauf};

/// Appends the start line to the chain of the source `gateway` in `ordner` and returns the sha256 of the line.
///
/// # Errors
/// Every error of `anhaengen_probelauf` of `formate` (contract 5, section 6).
pub fn anhaengen_probelauf(ordner: &Path, p: &Probelauf) -> Result<String, Fehler> {
    kette::anhaengen_probelauf(ordner, QUELLE, p)
}

/// 64 lower-case hex digits.
fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// The fields of one line of mode 2 `kopf-einsetzen` before the chain and the time text are added (contract 8,
/// section 13.4): the common fields and `ziel`, `methode`, `pfadregel`, `kopf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KopfZeile {
    /// Time of the line in milliseconds since 1970.
    pub zeit_ms: i64,
    /// The mode of the process (`kopf-einsetzen`).
    pub betriebsart: String,
    /// The route of the call.
    pub route: String,
    /// The session of the call.
    pub sitzung: String,
    /// `name` of the target, or `None` if the tunnel fits no target.
    pub ziel: Option<String>,
    /// The method of the request if it is one of the seven known ones.
    pub methode: Option<String>,
    /// `pfad` of the rule that fitted, never the path of the request.
    pub pfadregel: Option<String>,
    /// `ersetzt`, `keiner`, `abgewiesen`, or `None` if the request did not reach the binding.
    pub kopf: Option<String>,
    /// `weitergereicht`, `abgelehnt`, `ziel_fehler` or `leck_gesperrt`.
    pub ergebnis: String,
    /// `None` for `weitergereicht`, else the code that fits the result.
    pub grund: Option<String>,
    /// The status the harness got; `None` only for `mw.sni_falsch`.
    pub status: Option<u32>,
    /// The limited path of the request (section 8), only for a rejection and for a `GET`; else `None`.
    pub pfad: Option<String>,
    /// The host of the target the tunnel is bound to, lower case and without port; `None` before the binding.
    pub host: Option<String>,
}

/// The codes that fit a result of mode 2.
fn gruende_kopf(ergebnis: &str) -> Option<&'static [Option<&'static str>]> {
    match ergebnis {
        "weitergereicht" => Some(&[None]),
        "abgelehnt" => Some(&[
            Some("mw.anfrage_unlesbar"),
            Some("mw.ausgelastet"),
            Some("mw.nur_connect"),
            Some("mw.ziel_nicht_erlaubt"),
            Some("mw.sni_falsch"),
            Some("mw.host_falsch"),
            Some("mw.pfad_nicht_erlaubt"),
            Some("mw.methode_falsch"),
            Some("mw.upgrade_abgelehnt"),
            Some("mw.anmeldung_an_ungebundenes_ziel"),
            Some("mw.koerper_zu_gross"),
            Some("mw.zaehlung_fehler"),
        ]),
        "ziel_fehler" => Some(&[
            Some("mw.schluessel_fehlt"),
            Some("mw.ziel_nicht_erreichbar"),
            Some("mw.ziel_frist"),
            Some("mw.antwort_komprimiert"),
        ]),
        "leck_gesperrt" => Some(&[Some("mw.leck_gesperrt")]),
        _ => None,
    }
}

/// The checks of section 13.4 in their order, without `vorher` and the time.
fn pruefe_kopf(z: &KopfZeile) -> Result<(), Fehler> {
    let feld = |gut: bool, name: &'static str| {
        if gut {
            Ok(())
        } else {
            Err(Fehler::FeldUngueltig(name))
        }
    };
    feld(z.betriebsart == "kopf-einsetzen", "betriebsart")?;
    feld(kennung_ok(&z.route), "route")?;
    feld(kennung_ok(&z.sitzung), "sitzung")?;
    feld(z.ziel.as_deref().is_none_or(kennung_ok), "ziel")?;
    feld(
        z.methode
            .as_deref()
            .is_none_or(|m| crate::betriebsart::zielliste::METHODEN.contains(&m)),
        "methode",
    )?;
    feld(
        z.pfadregel
            .as_deref()
            .is_none_or(|p| crate::betriebsart::zielliste::regel_pfad_ok(p) && z.ziel.is_some()),
        "pfadregel",
    )?;
    let kopf = z.kopf.as_deref();
    let kopf_gut = matches!(kopf, None | Some("ersetzt" | "keiner" | "abgewiesen"))
        && (z.ergebnis != "weitergereicht" || matches!(kopf, Some("ersetzt" | "keiner")))
        && (kopf != Some("abgewiesen")
            || z.grund.as_deref() == Some("mw.anmeldung_an_ungebundenes_ziel"));
    feld(kopf_gut, "kopf")?;
    let gruende = gruende_kopf(&z.ergebnis).ok_or(Fehler::FeldUngueltig("ergebnis"))?;
    feld(gruende.contains(&z.grund.as_deref()), "grund")?;
    let status_gut = match z.status {
        None => z.grund.as_deref() == Some("mw.sni_falsch"),
        Some(s) => (100..=599).contains(&s),
    };
    feld(status_gut, "status")?;
    feld(
        z.pfad.as_deref().is_none_or(|p| {
            pfad_feld_ok(p) && (z.ergebnis == "abgelehnt" || z.methode.as_deref() == Some("GET"))
        }),
        "pfad",
    )?;
    // The host says which target of the list the tunnel is bound to, so it stands exactly where `ziel` stands.
    feld(
        z.host.is_some() == z.ziel.is_some()
            && z.host
                .as_deref()
                .is_none_or(crate::betriebsart::zielliste::host_ok),
        "host",
    )
}

fn objekt_kopf(z: &KopfZeile) -> Map<String, Value> {
    let mut o = Map::new();
    o.insert("aktionsname".to_owned(), AKTIONSNAME.into());
    o.insert("betriebsart".to_owned(), z.betriebsart.clone().into());
    o.insert("ergebnis".to_owned(), z.ergebnis.clone().into());
    o.insert("grund".to_owned(), z.grund.clone().into());
    o.insert("host".to_owned(), z.host.clone().into());
    o.insert("kopf".to_owned(), z.kopf.clone().into());
    o.insert("methode".to_owned(), z.methode.clone().into());
    o.insert("pfad".to_owned(), z.pfad.clone().into());
    o.insert("pfadregel".to_owned(), z.pfadregel.clone().into());
    o.insert("route".to_owned(), z.route.clone().into());
    o.insert("sitzung".to_owned(), z.sitzung.clone().into());
    o.insert("status".to_owned(), z.status.into());
    o.insert("ziel".to_owned(), z.ziel.clone().into());
    o
}

/// The bytes of a line of mode 2 (RFC 8785, without line feed) after the line `vorher`.
///
/// # Errors
/// `feld_ungueltig:<feld>` in the order `betriebsart`, `route`, `sitzung`, `ziel`, `methode`, `pfadregel`, `kopf`,
/// `ergebnis`, `grund`, `status`, `pfad`, `host`, `vorher`; then `zeit_ungueltig`.
pub fn zeile_kopf(z: &KopfZeile, vorher: &str) -> Result<String, Fehler> {
    pruefe_kopf(z)?;
    if !hex64(vorher) {
        return Err(Fehler::FeldUngueltig("vorher"));
    }
    let zeit = kette::zeit_text(z.zeit_ms)?;
    let mut o = objekt_kopf(z);
    o.insert("fassung".to_owned(), 2.into());
    o.insert("vorher".to_owned(), vorher.into());
    o.insert("zeit".to_owned(), zeit.into());
    kette::kanonisch(&Value::Object(o))
}

/// Appends a line of mode 2 to the chain of the source `gateway` in `ordner`. Returns the sha256 of the line.
///
/// # Errors
/// The field errors of [`zeile_kopf`] (without `vorher`), then every error of `anhaengen_zeile` (contract 5).
pub fn anhaengen_kopf(ordner: &Path, z: &KopfZeile) -> Result<String, Fehler> {
    pruefe_kopf(z)?;
    kette::anhaengen_zeile(ordner, QUELLE, z.zeit_ms, &Value::Object(objekt_kopf(z)))
}

#[cfg(test)]
mod tests {
    use super::{KopfZeile, Modellweg, pruefe, pruefe_kopf};

    fn zeile_1(ergebnis: &str, grund: Option<&str>) -> Modellweg {
        Modellweg {
            zeit_ms: 0,
            betriebsart: "openai-lokal".to_owned(),
            sitzung: "s".to_owned(),
            route: "r".to_owned(),
            modell: "m".to_owned(),
            ergebnis: ergebnis.to_owned(),
            grund: grund.map(str::to_owned),
            status: 404,
            token_ein: None,
            token_aus: None,
            pfad: None,
            host: None,
        }
    }

    fn code_1(m: &Modellweg) -> Option<String> {
        pruefe(m).err().map(|f| f.code())
    }

    #[test]
    fn mode_1_carries_a_path_only_at_a_rejection_and_never_a_host() {
        let mut m = zeile_1("abgelehnt", Some("mw.pfad_unbekannt"));
        assert_eq!(code_1(&m), None);
        m.pfad = Some("/v1/models".to_owned());
        assert_eq!(code_1(&m), None);
        m.pfad = Some("*".to_owned());
        assert_eq!(code_1(&m), None);
        m.pfad = Some("/v1/models?a=b".to_owned());
        assert_eq!(code_1(&m).as_deref(), Some("feld_ungueltig:pfad"));
        let mut weiter = zeile_1("weitergereicht", None);
        weiter.status = 200;
        assert_eq!(code_1(&weiter), None);
        weiter.pfad = Some("/v1/chat".to_owned());
        assert_eq!(code_1(&weiter).as_deref(), Some("feld_ungueltig:pfad"));
        let mut host = zeile_1("abgelehnt", Some("mw.pfad_unbekannt"));
        host.host = Some("api.beispiel.invalid".to_owned());
        assert_eq!(code_1(&host).as_deref(), Some("feld_ungueltig:host"));
    }

    fn zeile(ergebnis: &str, grund: Option<&str>, kopf: Option<&str>) -> KopfZeile {
        KopfZeile {
            zeit_ms: 0,
            betriebsart: "kopf-einsetzen".to_owned(),
            route: "r".to_owned(),
            sitzung: "s".to_owned(),
            ziel: Some("z".to_owned()),
            methode: Some("GET".to_owned()),
            pfadregel: Some("/*".to_owned()),
            kopf: kopf.map(str::to_owned),
            ergebnis: ergebnis.to_owned(),
            grund: grund.map(str::to_owned),
            status: Some(502),
            pfad: None,
            host: Some("a.beispiel.invalid".to_owned()),
        }
    }

    fn code(z: &KopfZeile) -> Option<String> {
        pruefe_kopf(z).err().map(|f| f.code())
    }

    #[test]
    fn the_fields_of_mode_2_fit_together() {
        assert_eq!(code(&zeile("weitergereicht", None, Some("ersetzt"))), None);
        assert_eq!(code(&zeile("weitergereicht", None, Some("keiner"))), None);
        assert_eq!(
            code(&zeile("weitergereicht", None, None)).as_deref(),
            Some("feld_ungueltig:kopf")
        );
        assert_eq!(
            code(&zeile("weitergereicht", None, Some("x"))).as_deref(),
            Some("feld_ungueltig:kopf")
        );
        assert_eq!(
            code(&zeile(
                "ziel_fehler",
                Some("mw.ziel_frist"),
                Some("ersetzt")
            )),
            None
        );
        assert_eq!(
            code(&zeile("ziel_fehler", Some("mw.antwort_komprimiert"), None)),
            None
        );
        assert_eq!(
            code(&zeile(
                "ziel_fehler",
                Some("mw.schluessel_fehlt"),
                Some("ersetzt")
            )),
            None
        );
        assert_eq!(
            code(&zeile(
                "ziel_fehler",
                Some("mw.ziel_nicht_erreichbar"),
                None
            )),
            None
        );
        assert_eq!(
            code(&zeile("ziel_fehler", Some("mw.leck_gesperrt"), None)).as_deref(),
            Some("feld_ungueltig:grund")
        );
        assert_eq!(
            code(&zeile(
                "abgelehnt",
                Some("mw.anmeldung_an_ungebundenes_ziel"),
                Some("abgewiesen")
            )),
            None
        );
        assert_eq!(
            code(&zeile(
                "abgelehnt",
                Some("mw.pfad_nicht_erlaubt"),
                Some("abgewiesen")
            ))
            .as_deref(),
            Some("feld_ungueltig:kopf")
        );
        assert_eq!(
            code(&zeile(
                "leck_gesperrt",
                Some("mw.leck_gesperrt"),
                Some("ersetzt")
            )),
            None
        );
        let mut ohne_ziel = zeile("weitergereicht", None, Some("ersetzt"));
        ohne_ziel.ziel = None;
        ohne_ziel.host = None;
        assert_eq!(
            code(&ohne_ziel).as_deref(),
            Some("feld_ungueltig:pfadregel")
        );
        ohne_ziel.pfadregel = None;
        assert_eq!(code(&ohne_ziel), None);
        let mut ohne_status = zeile("abgelehnt", Some("mw.sni_falsch"), None);
        ohne_status.status = None;
        assert_eq!(code(&ohne_status), None);
        ohne_status.grund = Some("mw.nur_connect".to_owned());
        assert_eq!(code(&ohne_status).as_deref(), Some("feld_ungueltig:status"));
    }

    #[test]
    fn the_path_stands_only_at_a_rejection_or_a_get() {
        let mut get = zeile("weitergereicht", None, Some("ersetzt"));
        get.pfad = Some("/models/session".to_owned());
        assert_eq!(code(&get), None);
        let mut post = get.clone();
        post.methode = Some("POST".to_owned());
        assert_eq!(code(&post).as_deref(), Some("feld_ungueltig:pfad"));
        post.ergebnis = "abgelehnt".to_owned();
        post.grund = Some("mw.pfad_nicht_erlaubt".to_owned());
        post.kopf = None;
        assert_eq!(code(&post), None);
        post.pfad = None;
        assert_eq!(code(&post), None);
        let mut roh = get.clone();
        roh.pfad = Some("/models?a=b".to_owned());
        assert_eq!(code(&roh).as_deref(), Some("feld_ungueltig:pfad"));
        roh.pfad = Some("/*".to_owned());
        assert_eq!(code(&roh), None);
        roh.pfad = Some("*".to_owned());
        assert_eq!(code(&roh), None);
    }

    #[test]
    fn the_host_stands_exactly_where_the_target_stands() {
        let mut z = zeile("weitergereicht", None, Some("ersetzt"));
        assert_eq!(code(&z), None);
        z.host = None;
        assert_eq!(code(&z).as_deref(), Some("feld_ungueltig:host"));
        z.ziel = None;
        z.pfadregel = None;
        assert_eq!(code(&z), None);
        z.host = Some("a.beispiel.invalid".to_owned());
        assert_eq!(code(&z).as_deref(), Some("feld_ungueltig:host"));
        let mut mit_port = zeile("weitergereicht", None, Some("ersetzt"));
        mit_port.host = Some("a.beispiel.invalid:443".to_owned());
        assert_eq!(code(&mit_port).as_deref(), Some("feld_ungueltig:host"));
        mit_port.host = Some("A.Beispiel.Invalid".to_owned());
        assert_eq!(code(&mit_port).as_deref(), Some("feld_ungueltig:host"));
    }
}
