//! Mode 2 `kopf-einsetzen` (contract 8, part C): its options (section 13.5), the fixed answers, the stages of a request
//! in the tunnel (section 13.2), the head towards the target and the heads of the answer (section 13.3). No I/O here;
//! the tunnel itself is `crate::tunnel` (Linux).

#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::zielliste::{METHODEN, Ziel};
#[cfg(target_os = "linux")]
use crate::aufruf::{PFAD_MAX, pfad_ok};
use crate::halter;
use crate::http::name_ok;

/// The options of this mode.
pub const OPTIONEN: [&str; 3] = ["--zielliste", "--ca-schluessel", "--ca-zertifikat"];
/// Largest `Content-Length` (and largest chunked body) of a request to an **unbound** target.
pub const KOERPER_MAX: u64 = 33_554_432;
/// The value of the field `kopf` of the line for a **bound** target (contract 8, section 13.4). The register of a
/// line of version 2 takes `halter` since 25.11.2026 (`rust/crates/formate/src/kette/pruefen.rs:425`); a line
/// outside the register is not appended at all, so this one constant is the only place that names the value.
pub const KOPF_GEBUNDEN: &str = "halter";
/// Largest body of a request to a **bound** target (H6): it goes as `koerper_b64` in one line of the holder, so
/// contract 8 lowers the limit instead of splitting the body (section 13.2, stage 7).
pub const KOERPER_MAX_GEBUNDEN: u64 = halter::KOERPER_MAX;
/// Headers of one connection (hop by hop).
const HOP: [&str; 6] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "te",
    "trailer",
    "upgrade",
];
/// Every name the holder forbids (holder contract, section 4: 31 names). This is the list of the **holder path**
/// (`modell.aufruf` and `modell.strom`, from step I): there the holder sets host, the framing and the header with the
/// token itself, so none of the names goes to it (contract 8, section 13.2, point 1). From step I a bound target takes
/// no other way, so the direct list of twenty names fell away with the direct way to a bound target.
pub const HALTER_VERBOTEN: [&str; 31] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "cookie",
    "set-cookie",
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "upgrade",
    "te",
    "trailer",
    "keep-alive",
    "proxy-connection",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "accept-encoding",
    "sec-websocket-extensions",
    "x-http-method-override",
    "x-http-method",
    "x-method-override",
    "x-original-method",
    "x-original-url",
    "x-original-uri",
    "x-rewrite-url",
    "x-forwarded-uri",
    "x-forwarded-path",
    "x-forwarded-prefix",
    "x-envoy-original-path",
];
/// The checked options of this mode: three session files of "Lauf starten".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Optionen {
    /// The target list.
    pub zielliste: PathBuf,
    /// The key of the throw-away CA.
    pub ca_schluessel: PathBuf,
    /// The certificate of the throw-away CA.
    pub ca_zertifikat: PathBuf,
}

impl Optionen {
    /// Takes the three options out of `werte`; `None` is `mw.aufruf`.
    #[cfg(target_os = "linux")]
    pub(crate) fn lies(werte: &mut BTreeMap<String, String>) -> Option<Self> {
        let [zielliste, schluessel, zertifikat] = OPTIONEN.map(|name| werte.remove(name));
        let (zielliste, schluessel, zertifikat) = (zielliste?, schluessel?, zertifikat?);
        let gut = [&zielliste, &schluessel, &zertifikat]
            .iter()
            .all(|p| pfad_ok(p, PFAD_MAX));
        gut.then(|| Self {
            zielliste: PathBuf::from(zielliste),
            ca_schluessel: PathBuf::from(schluessel),
            ca_zertifikat: PathBuf::from(zertifikat),
        })
    }
}

/// Status and fixed text of an own answer of this mode.
#[must_use]
pub fn ablehnung(code: &str) -> Option<(u16, &'static str)> {
    Some(match code {
        "mw.protokoll_fehlt" => (503, "Protokoll nicht schreibbar"),
        "mw.anfrage_unlesbar" => (400, "Anfrage nicht lesbar"),
        "mw.ausgelastet" => (429, "zu viele gleichzeitige Anfragen"),
        "mw.nur_connect" => (405, "nur CONNECT"),
        "mw.ziel_nicht_erlaubt" => (403, "Ziel nicht erlaubt"),
        "mw.host_falsch" => (400, "Host passt nicht zum Tunnel"),
        "mw.pfad_nicht_erlaubt" => (403, "Pfad nicht erlaubt"),
        "mw.methode_falsch" => (405, "Methode nicht erlaubt"),
        "mw.upgrade_abgelehnt" => (403, "Upgrade nicht erlaubt"),
        "mw.anmeldung_an_ungebundenes_ziel" => (403, "Anmeldung an diesem Ziel nicht erlaubt"),
        "mw.koerper_zu_gross" => (413, "Koerper zu gross"),
        "mw.halter_kein_schluessel" => (503, "kein Schluessel beim Halter"),
        "mw.halter_fehler" => (502, "Halter lehnte ab"),
        "mw.halter_frist" => (504, "Halter antwortete nicht"),
        "mw.ziel_nicht_erreichbar" => (502, "Ziel nicht erreichbar"),
        "mw.ziel_frist" => (504, "Ziel antwortet nicht"),
        "mw.leck_gesperrt" => (502, "Antwort des Ziels gesperrt"),
        "mw.antwort_komprimiert" => (502, "Antwort komprimiert"),
        "mw.zaehlung_fehler" => (503, "Zaehlung nicht schreibbar"),
        _ => return None,
    })
}

/// The whole own answer: status, `content-type`, `content-length` and the body in RFC 8785.
#[must_use]
pub fn antwort(code: &str) -> Vec<u8> {
    let (status, text) = ablehnung(code).unwrap_or((500, "interner Fehler"));
    let koerper = format!("{{\"error\":{{\"code\":\"{code}\",\"message\":\"{text}\"}}}}");
    format!(
        "HTTP/1.1 {status} \r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{koerper}",
        koerper.len()
    )
    .into_bytes()
}

/// The fields of the line that a request has reached so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meldung {
    /// The limited path of the request (section 8), as soon as the head is read.
    pub pfad: Option<String>,
    /// The method, if it is one of the seven known ones.
    pub methode: Option<&'static str>,
    /// The path of the rule that fitted.
    pub pfadregel: Option<String>,
    /// [`KOPF_GEBUNDEN`], `keiner` or `abgewiesen`.
    pub kopf: Option<&'static str>,
}

/// The head of a request in the tunnel: the request line and the fields with their names as sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnenKopf {
    /// The request line as sent.
    pub zeile: String,
    /// The method.
    pub methode: String,
    /// The request target (`/…`).
    pub ziel: String,
    /// The fields: name as sent, value without the white space around it.
    pub felder: Vec<(String, String)>,
}

impl InnenKopf {
    fn werte<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.felder
            .iter()
            .filter(move |(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, w)| w.as_str())
    }
}

/// A head `METHOD SP /target SP HTTP/1.1` with fields `name: value`; `None` is `mw.anfrage_unlesbar`.
#[must_use]
pub fn zerlege(kopf: &[u8]) -> Option<InnenKopf> {
    let text = std::str::from_utf8(kopf).ok()?;
    let mut zeilen = text.split("\r\n");
    let zeile = zeilen.next()?;
    let (methode, rest) = zeile.split_once(' ')?;
    let (ziel, version) = rest.split_once(' ')?;
    let gut = (1..=16).contains(&methode.len())
        && methode.bytes().all(|c| c.is_ascii_uppercase())
        && ziel.starts_with('/')
        && ziel.bytes().all(|c| (0x21..=0x7e).contains(&c))
        && version == "HTTP/1.1";
    if !gut {
        return None;
    }
    let mut felder = Vec::new();
    for z in zeilen {
        let (name, wert) = z.split_once(':')?;
        let wert = wert.trim_matches([' ', '\t']);
        if !(name_ok(name.as_bytes()) && wert.bytes().all(|c| (0x20..=0x7e).contains(&c))) {
            return None;
        }
        felder.push((name.to_owned(), wert.to_owned()));
    }
    Some(InnenKopf {
        zeile: zeile.to_owned(),
        methode: methode.to_owned(),
        ziel: ziel.to_owned(),
        felder,
    })
}

/// How the body of a request ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rahmen {
    /// No body.
    Ohne,
    /// `Content-Length`.
    Laenge(u64),
    /// `Transfer-Encoding: chunked` (the framing goes out unchanged).
    Stueckweise,
}

/// A request that passed the stages up to 7; the length check and the key follow with the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geprueft {
    /// The fields of the line so far.
    pub meldung: Meldung,
    /// The request asks for an upgrade that is handed through.
    pub umstellen: bool,
    /// The framing of the body.
    pub rahmen: Rahmen,
}

/// Stages 1 to 7 of section 13.2 for a request to `ziel` (the head is already read). The error carries the code and the
/// fields of the line reached so far.
///
/// # Errors
/// The code of the first failing stage with its fields.
pub fn pruefe(
    ziel: &Ziel,
    k: &InnenKopf,
    pfad_voll: bool,
) -> Result<Geprueft, (&'static str, Meldung)> {
    // The path of the line is known with the head, before any stage, and stays limited (section 8).
    let mut meldung = Meldung {
        pfad: Some(crate::pfad::pfad_feld(&k.ziel, pfad_voll)),
        ..Meldung::default()
    };
    let rahmen = rahmen(k)
        .filter(|_| anfrageziel_ok(ziel, k))
        .ok_or_else(|| ("mw.anfrage_unlesbar", meldung.clone()))?;
    let mut host = k.werte("host");
    let host_gut = match (host.next(), host.next()) {
        (Some(h), None) => h == ziel.host || h == format!("{}:{}", ziel.host, ziel.port),
        _ => false,
    };
    if !host_gut {
        return Err(("mw.host_falsch", meldung));
    }
    meldung.methode = METHODEN.iter().copied().find(|m| *m == k.methode);
    let pfad = k.ziel.split('?').next().unwrap_or_default();
    let form_gut = pfad_sicher(pfad) && (!ziel.gebunden() || halter_pfad_ok(pfad));
    let passend: Vec<_> = if form_gut {
        ziel.regeln.iter().filter(|r| r.passt(pfad)).collect()
    } else {
        Vec::new()
    };
    let Some(erste) = passend.first() else {
        return Err(("mw.pfad_nicht_erlaubt", meldung));
    };
    let Some(regel) = passend.iter().find(|r| r.erlaubt(&k.methode)) else {
        meldung.pfadregel = Some(erste.pfad.clone());
        return Err(("mw.methode_falsch", meldung));
    };
    meldung.pfadregel = Some(regel.pfad.clone());
    let upgrade = k.werte("upgrade").next().is_some()
        || k.werte("connection").any(|v| {
            v.split(',')
                .any(|t| t.trim().eq_ignore_ascii_case("upgrade"))
        });
    if upgrade && !ziel.upgrade_durchreichen {
        return Err(("mw.upgrade_abgelehnt", meldung));
    }
    let anmeldung = k.werte("authorization").next().is_some();
    if !ziel.gebunden() && anmeldung {
        meldung.kopf = Some("abgewiesen");
        return Err(("mw.anmeldung_an_ungebundenes_ziel", meldung));
    }
    if matches!(rahmen, Rahmen::Laenge(n) if n > koerper_grenze(ziel)) {
        return Err(("mw.koerper_zu_gross", meldung));
    }
    Ok(Geprueft {
        meldung,
        umstellen: upgrade,
        rahmen,
    })
}

/// The largest body of a request to `ziel` (stage 7): 23 MiB at a bound target, 32 MiB at an unbound one.
#[must_use]
pub fn koerper_grenze(ziel: &Ziel) -> u64 {
    if ziel.gebunden() {
        KOERPER_MAX_GEBUNDEN
    } else {
        KOERPER_MAX
    }
}

/// Stage 1 for the request target: never a `#`, and at a **bound** target a query that keeps the form of the field
/// `abfrage` of the holder (at most 1024 characters of its set; contract 8, section 5.1, H4). The model path checks it
/// itself, so a request outside the form never becomes a 502 at the holder.
#[must_use]
pub fn anfrageziel_ok(ziel: &Ziel, k: &InnenKopf) -> bool {
    if k.ziel.contains('#') {
        return false;
    }
    match (ziel.gebunden(), k.ziel.split_once('?')) {
        (true, Some((_, abfrage))) => halter::abfrage_ok(abfrage),
        _ => true,
    }
}

/// Stage 3 at a bound target: the form `^/[A-Za-z0-9._~/-]{0,511}$` of the field `pfad` of the holder. A path with
/// another character, also a percent-encoded one, or above 512 characters is `mw.pfad_nicht_erlaubt` (403), fail
/// closed: the holder refused it with `halter.wert_form` anyway.
#[must_use]
pub fn halter_pfad_ok(pfad: &str) -> bool {
    pfad.len() <= 512
        && pfad.starts_with('/')
        && pfad
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._~/-".contains(&c))
}

/// Stage 1 for the framing: `Transfer-Encoding` only as exactly `chunked` and not together with `Content-Length`;
/// `Content-Length` at most once and decimal.
fn rahmen(k: &InnenKopf) -> Option<Rahmen> {
    let te: Vec<&str> = k.werte("transfer-encoding").collect();
    let cl: Vec<&str> = k.werte("content-length").collect();
    match (te.as_slice(), cl.as_slice()) {
        ([], []) => Some(Rahmen::Ohne),
        ([t], []) if t.eq_ignore_ascii_case("chunked") => Some(Rahmen::Stueckweise),
        ([], [n]) => dezimal(n).map(Rahmen::Laenge),
        _ => None,
    }
}

fn dezimal(text: &str) -> Option<u64> {
    let b = text.as_bytes();
    let gut = text == "0"
        || ((1..=16).contains(&b.len()) && b[0] != b'0' && b.iter().all(u8::is_ascii_digit));
    if gut { text.parse().ok() } else { None }
}

/// A path without `//`, `\`, a segment `.` or `..`, and without `%2e`, `%2f`, `%5c` in any case.
#[must_use]
pub fn pfad_sicher(pfad: &str) -> bool {
    let klein = pfad.to_ascii_lowercase();
    pfad.starts_with('/')
        && !pfad.contains("//")
        && !pfad.contains('\\')
        && !["%2e", "%2f", "%5c"].iter().any(|k| klein.contains(k))
        && !pfad.split('/').any(|s| s == "." || s == "..")
}

/// The head to an **unbound** target (section 13.2): the request line, the fields of the harness in their order and
/// spelling — `cookie`, `x-api-key` and `accept-encoding` expressly among them — without `authorization` (stage 6
/// refused it before) and without the headers of one connection, then `connection: close`. An unbound target never
/// gets a key, and it never hands an upgrade through (section 12), so `upgrade` and `connection` always fall here.
/// A **bound** target takes no byte of this way: it goes to the holder ([`an_halter`]).
#[must_use]
pub fn an_ziel(k: &InnenKopf) -> Vec<u8> {
    let weg = |name: &str| {
        let name = name.to_ascii_lowercase();
        name == "authorization" || HOP.contains(&name.as_str())
    };
    let mut v = format!("{}\r\n", k.zeile).into_bytes();
    for (name, wert) in k.felder.iter().filter(|(n, _)| !weg(n)) {
        v.extend_from_slice(format!("{name}: {wert}\r\n").as_bytes());
    }
    v.extend_from_slice(b"connection: close\r\n");
    v.extend_from_slice(b"\r\n");
    v
}

/// Path, query and headers of a request to a **bound** target, for `modell.aufruf` and `modell.strom` (section 5.1).
/// The model path splits the request target at the first `?`; a `?` never stands in the field `pfad`. Of the headers
/// it drops **every** one of the 31 names the holder forbids and writes the rest small: the holder sets `host`, the
/// framing, `accept-encoding: identity`, `upgrade`, `connection` and the header with the token itself.
/// `connection: close` does not go to the holder either; the way out is his.
/// The order of the headers is **not** kept and a doubled name keeps only its last value: the field `kopf` of the
/// holder is an object and the line is canonical (contract 8, section 5.1, and its question F2).
/// `None` if the headers do not keep the form the holder takes (`mw.anfrage_unlesbar`, 400).
#[must_use]
pub fn an_halter(k: &InnenKopf) -> Option<(String, Option<String>, Vec<halter::Kopf>)> {
    let (pfad, abfrage) = match k.ziel.split_once('?') {
        Some((p, a)) => (p.to_owned(), Some(a.to_owned())),
        None => (k.ziel.clone(), None),
    };
    let kopf: Vec<halter::Kopf> = k
        .felder
        .iter()
        .map(|(n, w)| (n.to_ascii_lowercase(), w.clone()))
        .filter(|(n, _)| !HALTER_VERBOTEN.contains(&n.as_str()))
        .collect();
    halter::kopf_ok(&kopf).then_some((pfad, abfrage, kopf))
}

/// Whether a header of the answer goes to the harness: not the headers of one connection (a switch 101 keeps
/// `upgrade` and `connection`), not the framing, not `set-cookie`, not `proxy-authenticate`.
#[must_use]
pub fn antwortkopf_bleibt(name: &str, umgestellt: bool) -> bool {
    let name = name.to_ascii_lowercase();
    let hop = HOP.contains(&name.as_str())
        && !(umgestellt && (name == "upgrade" || name == "connection"));
    !hop && ![
        "transfer-encoding",
        "content-length",
        "set-cookie",
        "proxy-authenticate",
    ]
    .contains(&name.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        HALTER_VERBOTEN, HOP, KOERPER_MAX, KOERPER_MAX_GEBUNDEN, Rahmen, ablehnung, an_halter,
        an_ziel, antwort, antwortkopf_bleibt, dezimal, halter_pfad_ok, koerper_grenze, pfad_sicher,
        pruefe, rahmen, zerlege,
    };
    use crate::betriebsart::zielliste::{Regel, Ziel};

    fn ziel(gebunden: bool, upgrade: bool) -> Ziel {
        Ziel {
            name: "z".to_owned(),
            host: "a.b".to_owned(),
            port: 443,
            halter_ziel: gebunden.then(|| "a.b".to_owned()),
            regeln: vec![Regel {
                methode: "*".to_owned(),
                pfad: "/*".to_owned(),
            }],
            upgrade_durchreichen: upgrade,
        }
    }

    /// The headers of a request to the holder as one text, so a test reads them like a head.
    fn halterkopf(k: &super::InnenKopf) -> String {
        let (pfad, abfrage, kopf) = an_halter(k).expect("head");
        let felder: Vec<String> = kopf.iter().map(|(n, w)| format!("{n}: {w}")).collect();
        format!(
            "{pfad}|{}|{}",
            abfrage.unwrap_or_else(|| "-".to_owned()),
            felder.join(" ")
        )
    }

    fn kopf(felder: &str) -> super::InnenKopf {
        zerlege(format!("GET /a HTTP/1.1\r\nHost: a.b{felder}").as_bytes()).expect("head")
    }

    #[test]
    fn every_own_answer_has_its_status_and_text() {
        let tabelle = [
            ("mw.protokoll_fehlt", 503, "Protokoll nicht schreibbar"),
            ("mw.anfrage_unlesbar", 400, "Anfrage nicht lesbar"),
            ("mw.ausgelastet", 429, "zu viele gleichzeitige Anfragen"),
            ("mw.nur_connect", 405, "nur CONNECT"),
            ("mw.ziel_nicht_erlaubt", 403, "Ziel nicht erlaubt"),
            ("mw.host_falsch", 400, "Host passt nicht zum Tunnel"),
            ("mw.pfad_nicht_erlaubt", 403, "Pfad nicht erlaubt"),
            ("mw.methode_falsch", 405, "Methode nicht erlaubt"),
            ("mw.upgrade_abgelehnt", 403, "Upgrade nicht erlaubt"),
            (
                "mw.anmeldung_an_ungebundenes_ziel",
                403,
                "Anmeldung an diesem Ziel nicht erlaubt",
            ),
            ("mw.koerper_zu_gross", 413, "Koerper zu gross"),
            (
                "mw.halter_kein_schluessel",
                503,
                "kein Schluessel beim Halter",
            ),
            ("mw.halter_fehler", 502, "Halter lehnte ab"),
            ("mw.halter_frist", 504, "Halter antwortete nicht"),
            ("mw.ziel_nicht_erreichbar", 502, "Ziel nicht erreichbar"),
            ("mw.ziel_frist", 504, "Ziel antwortet nicht"),
            ("mw.leck_gesperrt", 502, "Antwort des Ziels gesperrt"),
            ("mw.antwort_komprimiert", 502, "Antwort komprimiert"),
            ("mw.zaehlung_fehler", 503, "Zaehlung nicht schreibbar"),
        ];
        for (code, status, text) in tabelle {
            assert_eq!(ablehnung(code), Some((status, text)), "{code}");
        }
        assert_eq!(ablehnung("mw.sni_falsch"), None);
        // The key file of a bound target fell away with the direct way; its code is no answer of the tunnel any more.
        assert_eq!(ablehnung("mw.schluessel_fehlt"), None);
        assert!(
            String::from_utf8(antwort("mw.x"))
                .expect("ascii")
                .starts_with("HTTP/1.1 500 ")
        );
    }

    /// From step I a bound target takes only the holder path, and there every one of the 31 names falls.
    #[test]
    fn the_list_of_forbidden_names_holds_all_thirty_one() {
        assert_eq!(HALTER_VERBOTEN.len(), 31);
        for name in [
            "authorization",
            "host",
            "content-length",
            "transfer-encoding",
        ] {
            assert!(HALTER_VERBOTEN.contains(&name), "{name}");
        }
        for name in HOP {
            assert!(HALTER_VERBOTEN.contains(&name), "{name}");
        }
        let doppelt = HALTER_VERBOTEN
            .iter()
            .enumerate()
            .any(|(i, n)| HALTER_VERBOTEN[..i].contains(n));
        assert!(!doppelt);
    }

    #[test]
    fn the_host_may_carry_the_port() {
        let z = ziel(true, true);
        assert!(pruefe(&z, &kopf(""), false).is_ok());
        let mit_port = zerlege(b"GET /a HTTP/1.1\r\nHost: a.b:443").expect("head");
        assert!(pruefe(&z, &mit_port, false).is_ok());
        let falsch = zerlege(b"GET /a HTTP/1.1\r\nHost: a.b:80").expect("head");
        assert_eq!(
            pruefe(&z, &falsch, false).map_err(|(c, _)| c),
            Err("mw.host_falsch")
        );
        let doppelt = zerlege(b"GET /a HTTP/1.1\r\nHost: a.b\r\nHost: a.b").expect("head");
        assert_eq!(
            pruefe(&z, &doppelt, false).map_err(|(c, _)| c),
            Err("mw.host_falsch")
        );
    }

    #[test]
    fn an_upgrade_is_seen_in_either_header() {
        let z = ziel(true, false);
        for felder in [
            "\r\nUpgrade: websocket",
            "\r\nConnection: keep-alive, Upgrade",
        ] {
            assert_eq!(
                pruefe(&z, &kopf(felder), false).map_err(|(c, _)| c),
                Err("mw.upgrade_abgelehnt"),
                "{felder}"
            );
        }
        assert!(pruefe(&z, &kopf("\r\nConnection: keep-alive"), false).is_ok());
        assert!(
            pruefe(&ziel(true, true), &kopf("\r\nUpgrade: websocket"), false)
                .is_ok_and(|g| g.umstellen)
        );
    }

    #[test]
    fn only_chunked_or_one_length() {
        assert_eq!(rahmen(&kopf("")), Some(Rahmen::Ohne));
        assert_eq!(
            rahmen(&kopf("\r\nTransfer-Encoding: Chunked")),
            Some(Rahmen::Stueckweise)
        );
        assert_eq!(rahmen(&kopf("\r\nTransfer-Encoding: gzip")), None);
        assert_eq!(
            rahmen(&kopf("\r\nContent-Length: 4")),
            Some(Rahmen::Laenge(4))
        );
        assert_eq!(
            rahmen(&kopf("\r\nContent-Length: 4\r\nContent-Length: 4")),
            None
        );
        assert_eq!(rahmen(&kopf("\r\nContent-Length: x")), None);
    }

    #[test]
    fn the_head_to_an_unbound_target_drops_hop_headers_and_keeps_the_rest() {
        let k = kopf(
            "\r\nConnection: keep-alive\r\nKeep-Alive: 5\r\nAccept-Encoding: gzip\r\nX-A: 1\r\nCookie: a=b\r\nX-Api-Key: p",
        );
        let frei = String::from_utf8(an_ziel(&k)).expect("ascii");
        assert_eq!(
            frei,
            "GET /a HTTP/1.1\r\nHost: a.b\r\nAccept-Encoding: gzip\r\nX-A: 1\r\nCookie: a=b\r\nX-Api-Key: p\r\nconnection: close\r\n\r\n"
        );
        // Stage 6 refused `Authorization` at an unbound target before; the head drops it all the same.
        let mit = kopf("\r\nAuthorization: Bearer p\r\nX-A: 1");
        let ohne = String::from_utf8(an_ziel(&mit)).expect("ascii");
        assert!(!ohne.contains("Authorization"), "{ohne}");
    }

    #[test]
    fn the_request_to_the_holder_drops_every_forbidden_name_and_writes_the_rest_small() {
        let mut felder = String::new();
        for n in HALTER_VERBOTEN {
            felder.push_str("\r\n");
            felder.push_str(n);
            felder.push_str(": x");
        }
        let k = zerlege(
            format!("GET /a HTTP/1.1\r\nHost: a.b{felder}\r\nX-A: 1\r\nAccept: */*").as_bytes(),
        )
        .expect("head");
        assert_eq!(halterkopf(&k), "/a|-|x-a: 1 accept: */*");
        // The holder sets `upgrade` and `connection` itself, so an upgrade request keeps neither.
        let u = kopf(
            "\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: k\r\nSec-WebSocket-Extensions: permessage-deflate",
        );
        assert_eq!(halterkopf(&u), "/a|-|sec-websocket-key: k");
    }

    #[test]
    fn path_and_query_go_apart_at_the_first_question_mark() {
        let mit = zerlege(b"GET /a/b?x=1&y=2 HTTP/1.1\r\nHost: a.b").expect("head");
        assert_eq!(halterkopf(&mit), "/a/b|x=1&y=2|");
        let zwei = zerlege(b"GET /a?x=1?y=2 HTTP/1.1\r\nHost: a.b").expect("head");
        assert_eq!(halterkopf(&zwei), "/a|x=1?y=2|");
        let leer = zerlege(b"GET /a? HTTP/1.1\r\nHost: a.b").expect("head");
        let (pfad, abfrage, _) = an_halter(&leer).expect("head");
        assert_eq!((pfad.as_str(), abfrage.as_deref()), ("/a", Some("")));
        // An empty query is stage 1 at a bound target: the form of the field takes 1 to 1024 characters.
        assert_eq!(
            pruefe(&ziel(true, false), &leer, false).map_err(|(c, _)| c),
            Err("mw.anfrage_unlesbar")
        );
        assert!(pruefe(&ziel(false, false), &leer, false).is_ok());
    }

    #[test]
    fn a_head_outside_the_form_of_the_holder_is_unreadable() {
        let mut viele = String::new();
        for i in 0..65 {
            viele.push_str("\r\nx-");
            viele.push_str(&i.to_string());
            viele.push_str(": v");
        }
        let k = zerlege(format!("GET /a HTTP/1.1\r\nHost: a.b{viele}").as_bytes()).expect("head");
        // `host` falls, so 65 fields of the harness leave exactly 65 for the holder.
        assert_eq!(an_halter(&k), None);
        let lang = "v".repeat(8193);
        let k = zerlege(format!("GET /a HTTP/1.1\r\nHost: a.b\r\nX-A: {lang}").as_bytes())
            .expect("head");
        assert_eq!(an_halter(&k), None);
        let grenze = "v".repeat(8192);
        let k = zerlege(format!("GET /a HTTP/1.1\r\nHost: a.b\r\nX-A: {grenze}").as_bytes())
            .expect("head");
        assert!(an_halter(&k).is_some());
    }

    #[test]
    fn a_bound_target_takes_only_the_path_form_of_the_holder() {
        assert!(halter_pfad_ok("/a/b.c~d-e_f"));
        assert!(halter_pfad_ok("/"));
        for p in ["/a%20b", "/a b", "/a+b", "/a:b", "/a,b", "/a@b"] {
            assert!(!halter_pfad_ok(p), "{p}");
        }
        assert!(halter_pfad_ok(&format!("/{}", "a".repeat(511))));
        assert!(!halter_pfad_ok(&format!("/{}", "a".repeat(512))));
        let lang = zerlege(format!("GET /{} HTTP/1.1\r\nHost: a.b", "a".repeat(512)).as_bytes())
            .expect("head");
        assert_eq!(
            pruefe(&ziel(true, false), &lang, false).map_err(|(c, _)| c),
            Err("mw.pfad_nicht_erlaubt")
        );
        // An unbound target keeps the wider form of the tunnel.
        assert!(pruefe(&ziel(false, false), &lang, false).is_ok());
        let prozent = zerlege(b"GET /a%41b HTTP/1.1\r\nHost: a.b").expect("head");
        assert_eq!(
            pruefe(&ziel(true, false), &prozent, false).map_err(|(c, _)| c),
            Err("mw.pfad_nicht_erlaubt")
        );
        assert!(pruefe(&ziel(false, false), &prozent, false).is_ok());
    }

    #[test]
    fn a_hash_in_the_request_target_is_unreadable_at_every_target() {
        let k = zerlege(b"GET /a#b HTTP/1.1\r\nHost: a.b").expect("head");
        for gebunden in [true, false] {
            assert_eq!(
                pruefe(&ziel(gebunden, false), &k, false).map_err(|(c, _)| c),
                Err("mw.anfrage_unlesbar"),
                "{gebunden}"
            );
        }
    }

    #[test]
    fn the_body_of_a_bound_target_stops_at_twenty_three_mebibytes() {
        assert_eq!(koerper_grenze(&ziel(true, false)), KOERPER_MAX_GEBUNDEN);
        assert_eq!(koerper_grenze(&ziel(false, false)), KOERPER_MAX);
        assert_eq!(KOERPER_MAX_GEBUNDEN, 24_117_248);
        let laenge = |n: u64| {
            zerlege(format!("POST /a HTTP/1.1\r\nHost: a.b\r\nContent-Length: {n}").as_bytes())
                .expect("head")
        };
        assert!(pruefe(&ziel(true, false), &laenge(KOERPER_MAX_GEBUNDEN), false).is_ok());
        assert_eq!(
            pruefe(&ziel(true, false), &laenge(KOERPER_MAX_GEBUNDEN + 1), false)
                .map_err(|(c, _)| c),
            Err("mw.koerper_zu_gross")
        );
        assert!(
            pruefe(
                &ziel(false, false),
                &laenge(KOERPER_MAX_GEBUNDEN + 1),
                false
            )
            .is_ok()
        );
        assert_eq!(
            pruefe(&ziel(false, false), &laenge(KOERPER_MAX + 1), false).map_err(|(c, _)| c),
            Err("mw.koerper_zu_gross")
        );
    }

    #[test]
    fn unsafe_paths() {
        assert!(pfad_sicher("/a/b"));
        assert!(pfad_sicher("/a.b/..c"));
        for p in [
            "a",
            "//a",
            "/a//b",
            "/a\\b",
            "/a/./b",
            "/a/..",
            "/%2E",
            "/%2f",
            "/a%5Cb",
            "/a/%2e%2e/b",
        ] {
            assert!(!pfad_sicher(p), "{p}");
        }
    }

    #[test]
    fn heads_of_the_answer() {
        assert!(antwortkopf_bleibt("Content-Type", false));
        for n in [
            "Connection",
            "Upgrade",
            "Keep-Alive",
            "TE",
            "Trailer",
            "Proxy-Connection",
            "Transfer-Encoding",
            "Content-Length",
            "Set-Cookie",
            "Proxy-Authenticate",
        ] {
            assert!(!antwortkopf_bleibt(n, false), "{n}");
        }
        assert!(antwortkopf_bleibt("Upgrade", true));
        assert!(antwortkopf_bleibt("Connection", true));
        assert!(!antwortkopf_bleibt("Keep-Alive", true));
        assert!(!antwortkopf_bleibt("Content-Length", true));
    }

    #[test]
    fn the_own_answer_and_the_head() {
        let a = String::from_utf8(antwort("mw.nur_connect")).expect("ascii");
        assert_eq!(
            a,
            "HTTP/1.1 405 \r\ncontent-type: application/json\r\ncontent-length: 59\r\n\r\n{\"error\":{\"code\":\"mw.nur_connect\",\"message\":\"nur CONNECT\"}}"
        );
        assert!(zerlege(b"GET /a HTTP/1.1\r\nA: b").is_some());
        assert!(zerlege(b"GET a HTTP/1.1").is_none());
        assert!(zerlege(b"GET /a HTTP/1.0").is_none());
        assert!(zerlege(b"get /a HTTP/1.1").is_none());
        assert!(zerlege(b"GET /a HTTP/1.1\r\nA b").is_none());
        assert!(zerlege(b"GET /a HTTP/1.1\r\nA b: c").is_none());
        assert!(zerlege(b"GET /a HTTP/1.1\r\nA: \x01").is_none());
        assert!(zerlege(b"GET /a\x7f HTTP/1.1").is_none());
        assert!(zerlege(b"GET  HTTP/1.1").is_none());
        assert!(zerlege(b"ABCDEFGHIJKLMNOPQ /a HTTP/1.1").is_none());
        assert!(zerlege(b"ABCDEFGHIJKLMNOP /a HTTP/1.1").is_some());
        assert_eq!(dezimal("0"), Some(0));
        assert_eq!(dezimal("01"), None);
        assert_eq!(dezimal("9999999999999999"), Some(9_999_999_999_999_999));
        assert_eq!(dezimal("10000000000000000"), None);
    }
}
