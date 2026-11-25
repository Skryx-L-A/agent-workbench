//! Mode 2 `kopf-einsetzen` (contract 8, part C): its options (section 13.5), the fixed answers, the stages of a request
//! in the tunnel (section 13.2), the head towards the target and the heads of the answer (section 13.3). No I/O here;
//! the tunnel itself is `crate::tunnel` (Linux).

#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::zielliste::{METHODEN, Ziel};
#[cfg(target_os = "linux")]
use crate::aufruf::{PFAD_MAX, pfad_ok};
use crate::http::name_ok;

/// The options of this mode.
pub const OPTIONEN: [&str; 3] = ["--zielliste", "--ca-schluessel", "--ca-zertifikat"];
/// Largest `Content-Length` (and largest chunked body) of a request in the tunnel.
pub const KOERPER_MAX: u64 = 33_554_432;
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
/// (`modell.aufruf` and `modell.strom`, from step I): there the holder sets host and framing itself, so none of the
/// names goes to it (contract 8, section 13.2, point 1). The holder path is not built yet; this constant is its named
/// place, and `the_two_lists_of_forbidden_names` holds its length and its relation to the direct one.
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
/// What the **direct** way to a bound target drops in addition (before step I, with the key from the file; decision of
/// 25.11.2026): the names of `HALTER_VERBOTEN` without `host`, `content-length` and `transfer-encoding`, which the
/// request needs there, and without the eight this module already drops above (`authorization`, `accept-encoding` and
/// the six headers of one connection, whose exception for a handed-through upgrade stays).
const DIREKT_VERBOTEN: [&str; 20] = [
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "cookie",
    "set-cookie",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
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
        "mw.schluessel_fehlt" => (503, "Schluessel nicht lesbar"),
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
    /// `ersetzt`, `keiner` or `abgewiesen`.
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
    let rahmen = rahmen(k).ok_or_else(|| ("mw.anfrage_unlesbar", meldung.clone()))?;
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
    let passend: Vec<_> = if pfad_sicher(pfad) {
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
    if ziel.bindung.is_none() && anmeldung {
        meldung.kopf = Some("abgewiesen");
        return Err(("mw.anmeldung_an_ungebundenes_ziel", meldung));
    }
    if matches!(rahmen, Rahmen::Laenge(n) if n > KOERPER_MAX) {
        return Err(("mw.koerper_zu_gross", meldung));
    }
    Ok(Geprueft {
        meldung,
        umstellen: upgrade,
        rahmen,
    })
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

/// The head to the target (section 13.2): the request line, the fields of the harness without `authorization`, the key
/// header of the target, the headers of one connection (an upgrade handed through keeps `upgrade` and `connection`)
/// and, at a bound target, `accept-encoding` and every name of `DIREKT_VERBOTEN` (among them
/// `sec-websocket-extensions`; W1: a negotiated compression would blind the leak lock); then `accept-encoding: identity`
/// at a bound target, `connection: close` unless upgrading, and the key header of a bound target.
/// This is the direct way: `host`, `content-length` and `transfer-encoding` stay, because the request needs them here.
/// The holder path drops those three as well (`HALTER_VERBOTEN`), and the holder sets them itself.
#[must_use]
pub fn an_ziel(ziel: &Ziel, k: &InnenKopf, umstellen: bool, schluessel: Option<&[u8]>) -> Vec<u8> {
    let weg = |name: &str| {
        let name = name.to_ascii_lowercase();
        name == "authorization"
            || ziel.bindung.as_ref().is_some_and(|b| {
                b.kopf == name
                    || name == "accept-encoding"
                    || DIREKT_VERBOTEN.contains(&name.as_str())
            })
            || (HOP.contains(&name.as_str())
                && !(umstellen && (name == "upgrade" || name == "connection")))
    };
    let mut v = format!("{}\r\n", k.zeile).into_bytes();
    for (name, wert) in k.felder.iter().filter(|(n, _)| !weg(n)) {
        v.extend_from_slice(format!("{name}: {wert}\r\n").as_bytes());
    }
    if ziel.bindung.is_some() {
        v.extend_from_slice(b"accept-encoding: identity\r\n");
    }
    if !umstellen {
        v.extend_from_slice(b"connection: close\r\n");
    }
    if let (Some(b), Some(s)) = (&ziel.bindung, schluessel) {
        v.extend_from_slice(format!("{}: {}", b.kopf, b.wert_vorsatz).as_bytes());
        v.extend_from_slice(s);
        v.extend_from_slice(b"\r\n");
    }
    v.extend_from_slice(b"\r\n");
    v
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
        DIREKT_VERBOTEN, HALTER_VERBOTEN, HOP, Rahmen, ablehnung, an_ziel, antwort,
        antwortkopf_bleibt, dezimal, pfad_sicher, pruefe, rahmen, zerlege,
    };
    use crate::betriebsart::zielliste::{Bindung, Regel, Ziel};

    fn ziel(gebunden: bool, upgrade: bool) -> Ziel {
        Ziel {
            name: "z".to_owned(),
            host: "a.b".to_owned(),
            port: 443,
            bindung: gebunden.then(|| Bindung {
                schluessel_datei: "/k".to_owned(),
                kopf: "x-key".to_owned(),
                wert_vorsatz: "V ".to_owned(),
            }),
            regeln: vec![Regel {
                methode: "*".to_owned(),
                pfad: "/*".to_owned(),
            }],
            upgrade_durchreichen: upgrade,
        }
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
            ("mw.schluessel_fehlt", 503, "Schluessel nicht lesbar"),
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
        assert!(
            String::from_utf8(antwort("mw.x"))
                .expect("ascii")
                .starts_with("HTTP/1.1 500 ")
        );
    }

    /// The list of the holder path carries all 31 names; the direct one is it without the three the request needs
    /// there and without the eight dropped elsewhere (decision of 25.11.2026).
    #[test]
    fn the_two_lists_of_forbidden_names() {
        assert_eq!(HALTER_VERBOTEN.len(), 31);
        assert_eq!(DIREKT_VERBOTEN.len(), 20);
        for name in DIREKT_VERBOTEN {
            assert!(HALTER_VERBOTEN.contains(&name), "{name}");
        }
        let fehlt: Vec<&str> = HALTER_VERBOTEN
            .iter()
            .filter(|n| !DIREKT_VERBOTEN.contains(n))
            .copied()
            .collect();
        assert_eq!(
            fehlt,
            [
                "authorization",
                "host",
                "content-length",
                "transfer-encoding",
                "connection",
                "upgrade",
                "te",
                "trailer",
                "keep-alive",
                "proxy-connection",
                "accept-encoding",
            ]
        );
        // The eight dropped elsewhere; only the three in between belong to the request of the direct way.
        for name in ["authorization", "accept-encoding"] {
            assert!(HALTER_VERBOTEN.contains(&name), "{name}");
        }
        for name in HOP {
            assert!(HALTER_VERBOTEN.contains(&name), "{name}");
        }
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
    fn the_head_to_the_target_drops_hop_headers() {
        let k = kopf(
            "\r\nConnection: keep-alive\r\nKeep-Alive: 5\r\nAccept-Encoding: gzip\r\nX-A: 1\r\nX-Key: alt\r\nAuthorization: Bearer p",
        );
        let gebunden =
            String::from_utf8(an_ziel(&ziel(true, true), &k, false, Some(b"S"))).expect("ascii");
        assert_eq!(
            gebunden,
            "GET /a HTTP/1.1\r\nHost: a.b\r\nX-A: 1\r\naccept-encoding: identity\r\nconnection: close\r\nx-key: V S\r\n\r\n"
        );
        let frei = String::from_utf8(an_ziel(&ziel(false, false), &k, false, None)).expect("ascii");
        assert_eq!(
            frei,
            "GET /a HTTP/1.1\r\nHost: a.b\r\nAccept-Encoding: gzip\r\nX-A: 1\r\nX-Key: alt\r\nconnection: close\r\n\r\n"
        );
        let u = kopf("\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nKeep-Alive: 5");
        let um =
            String::from_utf8(an_ziel(&ziel(true, true), &u, true, Some(b"S"))).expect("ascii");
        assert_eq!(
            um,
            "GET /a HTTP/1.1\r\nHost: a.b\r\nConnection: Upgrade\r\nUpgrade: websocket\r\naccept-encoding: identity\r\nx-key: V S\r\n\r\n"
        );
    }

    #[test]
    fn an_upgrade_to_a_bound_target_loses_every_extension_header() {
        let mit = "\r\nSec-WebSocket-Extensions: permessage-deflate\r\nSEC-WEBSOCKET-EXTENSIONS: x\r\nsec-websocket-extensions: y";
        let u = kopf(&format!(
            "\r\nConnection: Upgrade\r\nUpgrade: websocket{mit}"
        ));
        let um =
            String::from_utf8(an_ziel(&ziel(true, true), &u, true, Some(b"S"))).expect("ascii");
        assert_eq!(
            um,
            "GET /a HTTP/1.1\r\nHost: a.b\r\nConnection: Upgrade\r\nUpgrade: websocket\r\naccept-encoding: identity\r\nx-key: V S\r\n\r\n"
        );
        // An unbound target never gets a key: its upgrade keeps the extension (W1 is for bound targets).
        let frei = String::from_utf8(an_ziel(&ziel(false, true), &u, true, None)).expect("ascii");
        assert_eq!(
            frei,
            format!(
                "GET /a HTTP/1.1\r\nHost: a.b\r\nConnection: Upgrade\r\nUpgrade: websocket{mit}\r\n\r\n"
            )
        );
        // Without an upgrade the header is no negotiation, but the holder forbids it at a bound target all the same.
        let n = kopf("\r\nSec-WebSocket-Extensions: permessage-deflate");
        let ohne =
            String::from_utf8(an_ziel(&ziel(true, true), &n, false, Some(b"S"))).expect("ascii");
        assert_eq!(
            ohne,
            "GET /a HTTP/1.1\r\nHost: a.b\r\naccept-encoding: identity\r\nconnection: close\r\nx-key: V S\r\n\r\n"
        );
        // At a bound target every name of the direct list goes too, but `Host` and `Content-Length` stay: the
        // request needs them on the direct way (decision of 25.11.2026, contract 8, section 13.2, point 1).
        let v = kopf(
            "\r\nContent-Length: 0\r\nCookie: a=b\r\nX-Original-Url: /anders\r\nX-Envoy-Original-Path: /anders\r\nX-A: 1",
        );
        let gebunden =
            String::from_utf8(an_ziel(&ziel(true, true), &v, false, Some(b"S"))).expect("ascii");
        assert_eq!(
            gebunden,
            "GET /a HTTP/1.1\r\nHost: a.b\r\nContent-Length: 0\r\nX-A: 1\r\naccept-encoding: identity\r\nconnection: close\r\nx-key: V S\r\n\r\n"
        );
        // An unbound target keeps them: there it never gets a key (contract 8, section 13.2, point 1).
        let frei2 = String::from_utf8(an_ziel(&ziel(false, true), &v, false, None)).expect("ascii");
        assert!(
            frei2.contains("\r\nCookie: a=b\r\n")
                && frei2.contains("\r\nX-Original-Url: /anders\r\n")
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
