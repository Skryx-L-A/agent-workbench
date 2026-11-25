//! Mode 2 `kopf-einsetzen` (contract 8, part C): every case of `faelle/kopf-*.json` through a real tunnel. The harness
//! side is a TLS client over the socket pair; the throw-away CA exists only in this test; each target is a scripted
//! stand-in (no network). The search in the enclosure of `kopf-gegenprobe.json` (E5) runs in the VM; here only its
//! part of the forwarder: `GET /user/repos`.
#![cfg(target_os = "linux")]
#![allow(
    clippy::many_single_char_names,
    reason = "short names for case, answer, line and stream keep the tests readable"
)]

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use formate::kette::sha256_hex;
use gemeinsam::{Halterattrappe, Skript, Wegwerf, fall_datei, latin1, lauf_bis_ende, text, zeilen};
use modellweg::aufruf::{self, Aufruffehler};
use modellweg::betrieb::{Betrieb, Server};
use modellweg::betriebsart::zielliste;
use modellweg::ca::Ca;
use modellweg::probelauf::Zaehlung;
use modellweg::protokoll::{KopfZeile, zeile_kopf};
use modellweg::tunnel::Tunnel;
use modellweg::ziel::Verbinder;
use openssl::asn1::Asn1Time;
use openssl::bn::BigNum;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::ssl::{SslConnector, SslMethod, SslStream};
use openssl::x509::extension::BasicConstraints;
use openssl::x509::{X509, X509Extension, X509NameBuilder};
use serde_json::Value;

const MARKE: &str = "wb-marke-echtes-token-7d1c0e";
/// Read deadline of the harness in these tests: every answer comes at once, so a wrong program fails fast.
const LESEFRIST: std::time::Duration = std::time::Duration::from_secs(3);
const ZEIT: i64 = 1_795_190_400_000;
/// The lines a stand-in holder answers when a test does not care: a plain answer 200 without a body.
fn halter_ok() -> Vec<String> {
    halter_antwort(200, "{}", None)
}

fn schluessel_ec() -> PKey<Private> {
    let gruppe = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("curve");
    PKey::from_ec_key(EcKey::generate(&gruppe).expect("key")).expect("pkey")
}

/// A throw-away CA with the properties of section 12 for `hosts` (or broken in one way); returns key and certificate.
#[allow(
    deprecated,
    reason = "X509Extension::new_nid is the way the openssl crate writes name constraints"
)]
fn ca_bauen(hosts: &[&str], pfadlaenge: u32, stunden: u32, grenzen: bool) -> (PKey<Private>, X509) {
    let k = schluessel_ec();
    let mut n = X509NameBuilder::new().expect("name");
    n.append_entry_by_text("CN", "mw-test-ca").expect("cn");
    let n = n.build();
    let mut b = X509::builder().expect("builder");
    b.set_version(2).expect("version");
    b.set_serial_number(
        &BigNum::from_u32(1)
            .expect("bn")
            .to_asn1_integer()
            .expect("serial"),
    )
    .expect("serial");
    b.set_subject_name(&n).expect("subject");
    b.set_issuer_name(&n).expect("issuer");
    b.set_pubkey(&k).expect("pubkey");
    // One reading of the clock for both ends: two readings can lie in two seconds, and then a lifetime of 24 h is
    // 24 h and one second, which the start check rightly refuses (ca.rs, a_lifetime_that_ends_now_has_ended).
    let beginn = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_secs(),
    )
    .expect("secs")
        - 60;
    let jetzt = Asn1Time::from_unix(beginn).expect("time");
    let ende = Asn1Time::from_unix(beginn + i64::from(stunden) * 3600).expect("time");
    b.set_not_before(&jetzt).expect("before");
    b.set_not_after(&ende).expect("after");
    b.append_extension(
        BasicConstraints::new()
            .critical()
            .ca()
            .pathlen(pfadlaenge)
            .build()
            .expect("bc"),
    )
    .expect("ext");
    if grenzen {
        let mut wert = String::from("critical");
        for h in hosts {
            wert.push_str(",permitted;DNS:");
            wert.push_str(h);
        }
        wert.push_str(",excluded;IP:0.0.0.0/0.0.0.0,excluded;IP:::/::");
        let ext = X509Extension::new_nid(
            None,
            Some(&b.x509v3_context(None, None)),
            Nid::NAME_CONSTRAINTS,
            &wert,
        )
        .expect("nc");
        b.append_extension(ext).expect("ext");
    }
    b.sign(&k, MessageDigest::sha256()).expect("sign");
    (k, b.build())
}

fn datei_0600(w: &Wegwerf, name: &str, inhalt: &[u8]) -> PathBuf {
    let p = w.pfad(name);
    fs::write(&p, inhalt).expect("write");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).expect("chmod");
    p
}

/// The target list of the cases. From step I it names no path and no key: a bound target carries only `halter_ziel`,
/// and the token lies with the holder.
fn liste_lokal(_w: &Wegwerf, text: &str) -> String {
    text.to_owned()
}

fn vorfuehrung() -> String {
    let f = fall_datei("kopf-ziele.json");
    text(&f["faelle"][0]["text"]).to_owned()
}

/// A tunnel service for a target list, with one scripted stand-in per target.
struct Aufbau {
    w: Wegwerf,
    dienst: Arc<dyn Server>,
    skripte: Vec<Skript>,
    ca: X509,
    liste: zielliste::Zielliste,
    halter: Halterattrappe,
}

fn aufbau(liste_text: &str, antwort: &[Vec<u8>]) -> Aufbau {
    aufbau_ohne(liste_text, antwort, &[])
}

/// As [`aufbau`]; the targets named in `unaufgeloest` got no address at the start.
fn aufbau_ohne(liste_text: &str, antwort: &[Vec<u8>], unaufgeloest: &[&str]) -> Aufbau {
    aufbau_mit(
        Wegwerf::neu("kopf"),
        liste_text,
        antwort,
        unaufgeloest,
        None,
    )
}

/// As [`aufbau_ohne`] in the folder `w`, with the count of a trial run.
fn aufbau_mit(
    w: Wegwerf,
    liste_text: &str,
    antwort: &[Vec<u8>],
    unaufgeloest: &[&str],
    probelauf: Option<Zaehlung>,
) -> Aufbau {
    aufbau_voll(w, liste_text, antwort, unaufgeloest, probelauf, false)
}

/// As [`aufbau_mit`]; `pfad_voll` is the admin switch `--protokoll-pfad-voll` (section 8).
fn aufbau_voll(
    w: Wegwerf,
    liste_text: &str,
    antwort: &[Vec<u8>],
    unaufgeloest: &[&str],
    probelauf: Option<Zaehlung>,
    pfad_voll: bool,
) -> Aufbau {
    aufbau_kern(
        w,
        liste_text,
        antwort,
        unaufgeloest,
        probelauf,
        pfad_voll,
        halter_ok(),
    )
}

/// As [`aufbau`], with the lines the stand-in holder answers (the way of every bound target).
fn aufbau_mit_halter(liste_text: &str, halter: Vec<String>) -> Aufbau {
    aufbau_kern(
        Wegwerf::neu("kopf"),
        liste_text,
        &[],
        &[],
        None,
        false,
        halter,
    )
}

/// As [`aufbau_mit_halter`] in the folder `w`, with the count of a trial run.
fn aufbau_halter_probelauf(
    w: Wegwerf,
    liste_text: &str,
    probelauf: Option<Zaehlung>,
    halter: Vec<String>,
) -> Aufbau {
    aufbau_kern(w, liste_text, &[], &[], probelauf, false, halter)
}

/// The line `beginn` with status 101 of a stream, the only line a stand-in sends before the bridge.
fn halter_101() -> Vec<String> {
    vec![
        r#"{"art":"beginn","fassung":1,"id":<ID>,"kopf":{"sec-websocket-accept":"s3pPLMBiTxaQ9kYGzzhZRbK+xOo="},"status":101}"#
            .to_owned(),
    ]
}

/// The bytes the model path sent into an open stream as `strom.teil` (the way from the harness to the target, W6).
fn strom_bytes(a: &Aufbau) -> Vec<u8> {
    let mut aus = Vec::new();
    for z in a.halter.empfangen() {
        let v: Value = serde_json::from_str(&z).expect("line");
        if v["op"] == "strom.teil" {
            aus.extend_from_slice(
                &modellweg::b64::dekodieren(text(&v["daten_b64"])).expect("base64"),
            );
        }
    }
    aus
}

#[allow(
    clippy::too_many_arguments,
    reason = "one value per part of the start, each named"
)]
fn aufbau_kern(
    w: Wegwerf,
    liste_text: &str,
    antwort: &[Vec<u8>],
    unaufgeloest: &[&str],
    probelauf: Option<Zaehlung>,
    pfad_voll: bool,
    halter_zeilen: Vec<String>,
) -> Aufbau {
    let text = liste_lokal(&w, liste_text);
    let liste = zielliste::lies(text.as_bytes()).expect("valid list");
    let hosts: Vec<&str> = liste.ziele.iter().map(|z| z.host.as_str()).collect();
    let (k, z) = ca_bauen(&hosts, 0, 24, true);
    let kp = datei_0600(&w, "ca.key", &k.private_key_to_pem_pkcs8().expect("pem"));
    let zp = datei_0600(&w, "ca.crt", &z.to_pem().expect("pem"));
    let uid = rustix::process::getuid().as_raw();
    let ca = Ca::pruefe(&kp, &zp, uid, &hosts).expect("the CA passes the start check");
    let skripte: Vec<Skript> = liste
        .ziele
        .iter()
        .map(|_| Skript::antwortet(antwort.to_vec()))
        .collect();
    let verbinder: Vec<Option<Box<dyn Verbinder>>> = liste
        .ziele
        .iter()
        .zip(&skripte)
        .map(|(z, s)| {
            (!unaufgeloest.contains(&z.name.as_str()))
                .then(|| Box::new(s.clone()) as Box<dyn Verbinder>)
        })
        .collect();
    let halter = Halterattrappe::neu(&w, halter_zeilen);
    let tunnel = Tunnel::neu(
        liste.clone(),
        ca,
        verbinder,
        "s-1".to_owned(),
        "cloud-beispiel".to_owned(),
        w.protokoll(),
        probelauf,
        pfad_voll,
        uid,
        halter.pfad(),
        Betrieb::neu(Box::new(|| ZEIT)),
    );
    Aufbau {
        dienst: Arc::new(tunnel),
        skripte,
        ca: z,
        liste,
        halter,
        w,
    }
}

impl Aufbau {
    fn verbunden(&self) -> usize {
        self.skripte
            .iter()
            .map(|s| s.verbunden.load(Ordering::SeqCst))
            .sum()
    }

    /// Starts one connection of the harness; returns its end and the thread of the service.
    fn verbinden(&self) -> (UnixStream, std::thread::JoinHandle<()>) {
        let (harness, modellweg) = UnixStream::pair().expect("pair");
        harness.set_read_timeout(Some(LESEFRIST)).expect("timeout");
        let d = Arc::clone(&self.dienst);
        let faden = std::thread::spawn(move || d.bediene(modellweg));
        (harness, faden)
    }

    /// CONNECT and its answer; the answer is read up to the empty line, or to the end for a refusal.
    fn connect(h: &mut UnixStream, roh: &[u8]) -> Vec<u8> {
        h.write_all(roh).expect("send");
        let mut antwort = Vec::new();
        let mut b = [0u8; 1];
        while !antwort.ends_with(b"\r\n\r\n") {
            match h.read(&mut b) {
                Ok(1) => antwort.push(b[0]),
                _ => break,
            }
        }
        if !antwort.starts_with(b"HTTP/1.1 200 ") {
            let _ = h.read_to_end(&mut antwort);
        }
        antwort
    }

    /// The TLS client of the harness: it trusts the CA and checks chain, name constraints and host name.
    fn tls(&self, h: UnixStream, sni: Option<&str>) -> Result<SslStream<UnixStream>, String> {
        let mut b = SslConnector::builder(SslMethod::tls_client()).expect("connector");
        b.cert_store_mut()
            .add_cert(self.ca.clone())
            .expect("trust the CA");
        b.set_alpn_protos(b"\x08http/1.1").expect("alpn");
        let c = b.build();
        let mut cfg = c.configure().expect("configure");
        cfg.set_use_server_name_indication(sni.is_some());
        cfg.set_verify_hostname(sni.is_some());
        cfg.connect(sni.unwrap_or("api.github.com"), h)
            .map_err(|e| e.to_string())
    }
}

/// An answer to the harness: status, fields as sent, the body decoded, and whether it ended by its framing.
#[derive(Debug)]
struct Antwort {
    status: u16,
    kopf: Vec<(String, String)>,
    koerper: Vec<u8>,
    ganz: bool,
}

fn lies_antwort<R: Read>(r: &mut R) -> Antwort {
    let mut kopf = Vec::new();
    let mut b = [0u8; 1];
    while !kopf.ends_with(b"\r\n\r\n") {
        match r.read(&mut b) {
            Ok(1) => kopf.push(b[0]),
            _ => panic!("no head: {:?}", String::from_utf8_lossy(&kopf)),
        }
    }
    let text = String::from_utf8_lossy(&kopf[..kopf.len() - 4]).into_owned();
    let mut zeilen = text.split("\r\n");
    let status = zeilen.next().expect("status")[9..12]
        .parse()
        .expect("status");
    let felder: Vec<(String, String)> = zeilen
        .map(|z| {
            let (n, w) = z.split_once(": ").expect("field");
            (n.to_owned(), w.to_owned())
        })
        .collect();
    let wert = |n: &str| {
        felder
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(n))
            .map(|(_, w)| w.clone())
    };
    let mut koerper = Vec::new();
    let mut ganz = true;
    if let Some(n) = wert("content-length") {
        let n: usize = n.parse().expect("length");
        let _ = r
            .take(u64::try_from(n).expect("length"))
            .read_to_end(&mut koerper);
        ganz = koerper.len() == n;
    } else if wert("transfer-encoding").is_some() {
        let mut roh = Vec::new();
        let _ = r.read_to_end(&mut roh);
        let mut rest = roh.as_slice();
        ganz = false;
        while let Some(i) = rest.windows(2).position(|w| w == b"\r\n") {
            let n = usize::from_str_radix(std::str::from_utf8(&rest[..i]).expect("hex"), 16)
                .expect("hex");
            if n == 0 {
                ganz = true;
                break;
            }
            if rest.len() < i + 2 + n {
                break;
            }
            koerper.extend_from_slice(&rest[i + 2..i + 2 + n]);
            rest = &rest[(i + 4 + n).min(rest.len())..];
        }
    } else {
        let _ = r.read_to_end(&mut koerper);
    }
    Antwort {
        status,
        kopf: felder,
        koerper,
        ganz,
    }
}

fn letzte_zeile(a: &Aufbau, name: &str) -> Value {
    zeilen(&a.w.protokoll()).pop().unwrap_or_else(|| {
        let inhalt: Vec<String> = fs::read_dir(a.w.protokoll())
            .map(|d| {
                d.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        panic!("{name}: no line; folder {inhalt:?}")
    })
}

#[test]
fn kopf_aufruf_cases() {
    let f = fall_datei("kopf-aufruf.json");
    for fall in f["aufruf"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let argv: Vec<OsString> = fall["argv"]
            .as_array()
            .expect("argv")
            .iter()
            .map(|a| OsString::from(text(a)))
            .collect();
        let e = &fall["erwartet"];
        let ist = aufruf::lies(&argv);
        if e["bis_schritt"] == "aufruf_gut" {
            assert!(ist.is_ok(), "{name}: {ist:?}");
            continue;
        }
        assert_eq!(ist.err(), Some(Aufruffehler::Aufruf), "{name}");
        let (exit, out, err) = lauf_bis_ende(&argv);
        assert_eq!(i64::from(exit), e["exit"].as_i64().expect("exit"), "{name}");
        assert_eq!(out, latin1(text(&e["stdout"])), "{name}");
        assert_eq!(err, latin1(text(&e["stderr"])), "{name}");
    }
}

/// The text of a target list case; `text_muster` builds a list from the demo list (a top-level key `x` with 65536
/// times `a`).
fn ziele_text(fall: &Value) -> String {
    if let Some(t) = fall["text"].as_str() {
        return t.to_owned();
    }
    let muster = &fall["text_muster"];
    assert_eq!(muster["basis"], "vorfuehrung");
    assert_eq!(
        muster["zusatz"],
        "Schluessel x mit 65536 mal a auf oberster Ebene"
    );
    let basis = vorfuehrung();
    format!("{{\"x\":\"{}\",{}", "a".repeat(65_536), &basis[1..])
}

#[test]
fn kopf_ziele_cases() {
    let f = fall_datei("kopf-ziele.json");
    for fall in f["faelle"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let e = &fall["erwartet"];
        let roh = ziele_text(fall);
        let ist = zielliste::lies(roh.as_bytes());
        if let Some(code) = e["code"].as_str() {
            assert_eq!(ist.err(), Some(code), "{name}");
        } else {
            {
                let liste = ist.unwrap_or_else(|f| panic!("{name}: {f}"));
                let mut namen: Vec<&str> = liste.ziele.iter().map(|z| z.host.as_str()).collect();
                namen.sort_unstable();
                let soll: Vec<&str> = e["ca_erlaubte_namen"]
                    .as_array()
                    .expect("names")
                    .iter()
                    .map(text)
                    .collect();
                assert_eq!(namen, soll, "{name}");
            }
        }
        start_mit_liste(name, &roh, &e["start"]);
    }
}

/// The start of mode 2 with this target list and a valid CA: up to the target list, or the exit of its step.
fn start_mit_liste(name: &str, liste_text: &str, e: &Value) {
    let w = Wegwerf::neu("zstart");
    let lokal = liste_lokal(&w, liste_text);
    let liste = datei_0600(&w, "zielliste.json", lokal.as_bytes());
    let hosts: Vec<String> = zielliste::lies(lokal.as_bytes())
        .map(|l| l.ziele.iter().map(|z| z.host.clone()).collect())
        .unwrap_or_default();
    let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
    let (k, z) = ca_bauen(&hosts, 0, 24, true);
    let kp = datei_0600(&w, "ca.key", &k.private_key_to_pem_pkcs8().expect("pem"));
    let zp = datei_0600(&w, "ca.crt", &z.to_pem().expect("pem"));
    let args: Vec<OsString> = [
        "--betriebsart",
        "kopf-einsetzen",
        "--socket",
        w.pfad("m.sock").to_str().expect("utf8"),
        "--socket-rechte",
        "0600",
        "--sitzung",
        "s-1",
        "--route",
        "cloud-beispiel",
        "--protokoll",
        w.protokoll().to_str().expect("utf8"),
        "--zielliste",
        liste.to_str().expect("utf8"),
        "--ca-schluessel",
        kp.to_str().expect("utf8"),
        "--ca-zertifikat",
        zp.to_str().expect("utf8"),
    ]
    .iter()
    .map(OsString::from)
    .collect();
    let aufloeser = |_: &str, p: u16| -> std::io::Result<Vec<std::net::SocketAddr>> {
        Ok(vec![std::net::SocketAddr::from(([127, 0, 0, 1], p))])
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let ergebnis = modellweg::start::starten(
        &args,
        rustix::process::getuid().as_raw(),
        &aufloeser,
        modellweg::ziel::FRISTEN,
        Box::new(|| ZEIT),
        &mut out,
        &mut err,
    );
    match (ergebnis, e["exit"].as_i64()) {
        // A list that passes and names a bound target leads into step 3: the model path reaches the holder at his
        // fixed path. In a test there is none there, so the start ends with exit 78 `mw.halter_fehlt` — and exactly
        // that is the proof that step 3 runs after step 2 and that there is no fall back on a file.
        (Err(exit), None) if e["bis_schritt"] == "halter" => {
            assert_eq!(i64::from(exit), 78, "{name}");
            assert_eq!(
                err,
                latin1("mw.halter_fehlt: Halter nicht erreichbar\n"),
                "{name}"
            );
            assert!(out.is_empty(), "{name}");
        }
        (Ok(_), None) => assert_eq!(e["bis_schritt"], "zielliste_gut", "{name}"),
        (Err(exit), Some(soll)) => {
            assert_eq!(i64::from(exit), soll, "{name}");
            assert_eq!(err, latin1(text(&e["stderr"])), "{name}");
            assert!(out.is_empty(), "{name}");
        }
        (Ok(_), Some(soll)) => panic!("{name}: started, expected exit {soll}"),
        (Err(exit), None) => panic!("{name}: exit {exit}, {}", String::from_utf8_lossy(&err)),
    }
}

/// Step 6 of the start (section 13.5): an unbound target whose resolution answers with an empty list has no address,
/// the same as a resolution that fails. The start goes on, and a CONNECT to that target is refused at once with 502
/// `mw.ziel_nicht_erreichbar` (stage 6), before any `200 Connection Established` and before TLS. A connector over an
/// empty list of addresses would instead accept the CONNECT and fail only at the request inside the tunnel.
#[test]
fn an_empty_resolution_at_the_start_is_no_address() {
    let w = Wegwerf::neu("leer-aufgeloest");
    let host = "telemetry.beispiel.invalid";
    let text = format!(
        r#"{{"fassung":1,"ca_zulassung":"zugelassen","ziele":[{{"name":"tele","host":"{host}","port":443,"halter_ziel":null,"regeln":[{{"methode":"POST","pfad":"/telemetry"}}],"upgrade":"ablehnen"}}]}}"#
    );
    let liste = datei_0600(&w, "zielliste.json", text.as_bytes());
    let (k, z) = ca_bauen(&[host], 0, 24, true);
    let kp = datei_0600(&w, "ca.key", &k.private_key_to_pem_pkcs8().expect("pem"));
    let zp = datei_0600(&w, "ca.crt", &z.to_pem().expect("pem"));
    let args: Vec<OsString> = [
        "--betriebsart",
        "kopf-einsetzen",
        "--socket",
        w.pfad("m.sock").to_str().expect("utf8"),
        "--socket-rechte",
        "0600",
        "--sitzung",
        "s-1",
        "--route",
        "cloud-beispiel",
        "--protokoll",
        w.protokoll().to_str().expect("utf8"),
        "--zielliste",
        liste.to_str().expect("utf8"),
        "--ca-schluessel",
        kp.to_str().expect("utf8"),
        "--ca-zertifikat",
        zp.to_str().expect("utf8"),
    ]
    .iter()
    .map(OsString::from)
    .collect();
    let gefragt = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let zaehler = Arc::clone(&gefragt);
    let aufloeser = move |h: &str, _: u16| -> std::io::Result<Vec<std::net::SocketAddr>> {
        assert_eq!(h, host);
        zaehler.fetch_add(1, Ordering::SeqCst);
        Ok(Vec::new())
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let bereit = modellweg::start::starten(
        &args,
        rustix::process::getuid().as_raw(),
        &aufloeser,
        modellweg::ziel::FRISTEN,
        Box::new(|| ZEIT),
        &mut out,
        &mut err,
    )
    .unwrap_or_else(|e| panic!("exit {e}: {}", String::from_utf8_lossy(&err)));
    assert_eq!(gefragt.load(Ordering::SeqCst), 1);
    assert_eq!(out, b"{\"bereit\":true}\n");
    let (mut h, seite) = UnixStream::pair().expect("pair");
    h.set_read_timeout(Some(LESEFRIST)).expect("timeout");
    let d = Arc::clone(&bereit.dienst);
    let faden = std::thread::spawn(move || d.bediene(seite));
    let antwort = Aufbau::connect(
        &mut h,
        format!("CONNECT {host}:443 HTTP/1.1\r\n\r\n").as_bytes(),
    );
    assert_eq!(
        antwort,
        modellweg::betriebsart::kopf_einsetzen::antwort("mw.ziel_nicht_erreichbar")
    );
    faden.join().expect("thread");
    let z = zeilen(&w.protokoll());
    assert_eq!(z.len(), 1, "{z:?}");
    assert_eq!(z[0]["grund"], "mw.ziel_nicht_erreichbar");
    drop(bereit);
}

#[test]
fn the_ca_check() {
    let w = Wegwerf::neu("ca");
    let uid = rustix::process::getuid().as_raw();
    let hosts = ["api.github.com", "api.githubcopilot.com"];
    let pruefe = |k: &PKey<Private>, z: &X509, uid_pruef: u32, hosts_pruef: &[&str]| {
        let kp = datei_0600(&w, "k.pem", &k.private_key_to_pem_pkcs8().expect("pem"));
        let zp = datei_0600(&w, "z.pem", &z.to_pem().expect("pem"));
        Ca::pruefe(&kp, &zp, uid_pruef, hosts_pruef).is_some()
    };
    let (k, z) = ca_bauen(&hosts, 0, 24, true);
    assert!(pruefe(&k, &z, uid, &hosts));
    assert!(!pruefe(&k, &z, uid + 1, &hosts), "not own");
    assert!(
        !pruefe(&k, &z, uid, &hosts[..1]),
        "constraints for another set of hosts"
    );
    assert!(
        !pruefe(
            &k,
            &z,
            uid,
            &["api.github.com", "api.githubcopilot.com", "x.y"]
        ),
        "a host without constraint"
    );
    let (k2, _) = ca_bauen(&hosts, 0, 24, true);
    assert!(
        !pruefe(&k2, &z, uid, &hosts),
        "key and certificate do not belong together"
    );
    let (k, z) = ca_bauen(&hosts, 1, 24, true);
    assert!(!pruefe(&k, &z, uid, &hosts), "path length 1");
    let (k, z) = ca_bauen(&hosts, 0, 25, true);
    assert!(!pruefe(&k, &z, uid, &hosts), "lifetime over 24 h");
    let (k, z) = ca_bauen(&hosts, 0, 24, false);
    assert!(!pruefe(&k, &z, uid, &hosts), "without name constraints");
    let (k, z) = ca_bauen(&hosts, 0, 24, true);
    let kp = datei_0600(&w, "k.pem", &k.private_key_to_pem_pkcs8().expect("pem"));
    let mut zwei = z.to_pem().expect("pem");
    zwei.extend(z.to_pem().expect("pem"));
    let zp = datei_0600(&w, "z.pem", &zwei);
    assert!(
        Ca::pruefe(&kp, &zp, uid, &hosts).is_none(),
        "two certificates"
    );
    let zp = datei_0600(&w, "z.pem", &z.to_pem().expect("pem"));
    fs::set_permissions(&zp, fs::Permissions::from_mode(0o644)).expect("chmod");
    assert!(Ca::pruefe(&kp, &zp, uid, &hosts).is_none(), "mode 0644");
    let link = w.pfad("link.pem");
    let zp = datei_0600(&w, "z.pem", &z.to_pem().expect("pem"));
    std::os::unix::fs::symlink(&zp, &link).expect("symlink");
    assert!(Ca::pruefe(&kp, &link, uid, &hosts).is_none(), "a link");
    assert!(Ca::pruefe(&kp, &zp, uid, &hosts).is_some());
}

#[test]
fn kopf_zeile_cases() {
    let f = fall_datei("kopf-zeile.json");
    for fall in f["zeilen"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let a = &fall["aufruf"];
        let s = |k: &str| a[k].as_str().map(str::to_owned);
        let z = KopfZeile {
            zeit_ms: a["zeit_ms"].as_i64().expect("zeit"),
            betriebsart: text(&a["betriebsart"]).to_owned(),
            route: text(&a["route"]).to_owned(),
            sitzung: text(&a["sitzung"]).to_owned(),
            ziel: s("ziel"),
            methode: s("methode"),
            pfadregel: s("pfadregel"),
            kopf: s("kopf"),
            ergebnis: text(&a["ergebnis"]).to_owned(),
            grund: s("grund"),
            status: a["status"].as_u64().map(|v| u32::try_from(v).expect("u32")),
            pfad: s("pfad"),
            host: s("host"),
        };
        let ist = zeile_kopf(&z, text(&fall["vorher"]));
        let e = &fall["erwartet"];
        if let Some(code) = e["code"].as_str() {
            assert_eq!(ist.map_err(|f| f.code()), Err(code.to_owned()), "{name}");
        } else {
            let zeile = ist.unwrap_or_else(|f| panic!("{name}: {}", f.code()));
            assert_eq!(zeile, text(&e["zeile"]), "{name}");
            assert_eq!(sha256_hex(zeile.as_bytes()), text(&e["sha256"]), "{name}");
        }
    }
}

/// The line of a request in the tunnel carries the limited path and the host of the target the tunnel is bound
/// to; with the admin switch it carries the whole path (contract 8, sections 8 and 13.4).
#[test]
fn die_zeile_im_tunnel_traegt_pfad_und_host() {
    let antwort = vec![b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}".to_vec()];
    // The demonstration list carries only exact paths since N1, and none of its `GET` paths has three segments (a
    // `POST` that goes through carries the path `null`). This test gives the plan target one exact rule more,
    // `GET /agents/sessions/eins`, so that the limited path in the line is shorter than the whole path.
    let mut liste: Value = serde_json::from_str(&vorfuehrung()).expect("list");
    let plan = liste["ziele"]
        .as_array_mut()
        .expect("targets")
        .iter_mut()
        .find(|z| z["name"] == "copilot-plan")
        .expect("plan target");
    plan["regeln"]
        .as_array_mut()
        .expect("rules")
        .push(serde_json::json!({"methode": "GET", "pfad": "/agents/sessions/eins"}));
    let text = serde_json::to_string(&liste).expect("text");
    let a = aufbau(&text, &antwort);
    let host = a
        .liste
        .ziele
        .iter()
        .find(|z| z.name == "copilot-plan")
        .expect("plan target")
        .host
        .clone();
    let anfrage = format!("GET /agents/sessions/eins?x=1 HTTP/1.1\r\nHost: {host}\r\n\r\n");
    im_tunnel(&a, "copilot-plan", anfrage.as_bytes());
    let z = letzte_zeile(&a, "pfad_und_host");
    assert_eq!(z["ergebnis"], "weitergereicht");
    assert_eq!(z["methode"], "GET");
    assert_eq!(z["pfad"], "/agents/sessions");
    assert_eq!(z["host"], host.as_str());

    let b = aufbau_voll(Wegwerf::neu("kopfvoll"), &text, &antwort, &[], None, true);
    im_tunnel(&b, "copilot-plan", anfrage.as_bytes());
    let zb = letzte_zeile(&b, "pfad_voll");
    assert_eq!(zb["pfad"], "/agents/sessions/eins");
    assert_eq!(zb["host"], host.as_str());
}

/// A subtree rule in the tunnel (section 13.2): the demonstration list carries only exact paths since N1, so this test
/// gives the unbound exp target the rule `POST /api/v1/*`. A path below it goes through and names the rule in the line;
/// the prefix itself without its `/` and a neighbour path with the same letters do not.
#[test]
fn a_subtree_rule_covers_the_paths_below_it() {
    let mut liste: Value = serde_json::from_str(&vorfuehrung()).expect("list");
    let exp = liste["ziele"]
        .as_array_mut()
        .expect("targets")
        .iter_mut()
        .find(|z| z["name"] == "copilot-exp")
        .expect("exp target");
    exp["regeln"] = serde_json::json!([{"methode": "POST", "pfad": "/api/v1/*"}]);
    let text = serde_json::to_string(&liste).expect("text");
    let antwort = vec![b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}".to_vec()];
    let host = "exp.individual.githubcopilot.com";
    for (pfad, status) in [
        ("/api/v1/assignments", 200_u16),
        ("/api/v1/assignments/x", 200),
        ("/api/v1", 403),
        ("/api/v10/assignments", 403),
    ] {
        let a = aufbau(&text, &antwort);
        let anfrage =
            format!("POST {pfad} HTTP/1.1\r\nHost: {host}\r\nContent-Length: 2\r\n\r\n{{}}");
        let r = im_tunnel(&a, "copilot-exp", anfrage.as_bytes());
        assert_eq!(r.status, status, "{pfad}");
        let z = letzte_zeile(&a, pfad);
        if status == 200 {
            assert_eq!(z["ergebnis"], "weitergereicht", "{pfad}");
            assert_eq!(z["pfadregel"], "/api/v1/*", "{pfad}");
        } else {
            assert_eq!(z["grund"], "mw.pfad_nicht_erlaubt", "{pfad}");
        }
    }
}

fn pruefe_zeile(name: &str, z: &Value, soll: &Value) {
    for (k, v) in soll.as_object().expect("line") {
        assert_eq!(&z[k], v, "{name}: {k}");
    }
}

#[test]
fn kopf_tunnel_cases() {
    let f = fall_datei("kopf-tunnel.json");
    for fall in f["faelle"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let lage = &fall["lage"];
        let e = &fall["erwartet"];
        let unaufgeloest: Vec<&str> = lage["unaufgeloest"]
            .as_array()
            .map(|v| v.iter().map(text).collect())
            .unwrap_or_default();
        let a = aufbau_ohne(
            &vorfuehrung(),
            &[b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec()],
            &unaufgeloest,
        );
        let mut gehalten = Vec::new();
        for _ in 0..lage["laufend"].as_u64().expect("laufend") {
            let (mut h, f) = a.verbinden();
            assert!(
                Aufbau::connect(&mut h, b"CONNECT api.github.com:443 HTTP/1.1\r\n\r\n")
                    .starts_with(b"HTTP/1.1 200 ")
            );
            gehalten.push((h, f));
        }
        if lage["protokoll_ok"] == false {
            fs::remove_dir_all(a.w.protokoll()).expect("remove");
            let (mut h, f) = a.verbinden();
            let _ = Aufbau::connect(&mut h, b"GET / HTTP/1.1\r\n\r\n");
            f.join().expect("thread");
            a.w.protokoll();
        }
        let vorher = zeilen(&a.w.protokoll()).len();
        let (mut h, faden) = a.verbinden();
        let antwort = Aufbau::connect(&mut h, &latin1(text(&fall["connect"])));
        if let Some(soll) = e["antwort"].as_str() {
            if soll == "tls_abbruch" {
                assert!(antwort.starts_with(b"HTTP/1.1 200 "), "{name}");
                assert!(a.tls(h, fall["sni"].as_str()).is_err(), "{name}");
            } else {
                assert_eq!(antwort, latin1(soll), "{name}");
                let mut t = a
                    .tls(h, fall["sni"].as_str())
                    .expect("TLS with a leaf of the CA");
                let leaf = t.ssl().peer_certificate().expect("leaf");
                let host = leaf
                    .subject_alt_names()
                    .expect("san")
                    .iter()
                    .filter_map(|n| n.dnsname().map(str::to_owned))
                    .collect::<Vec<_>>();
                assert_eq!(host, vec![text(&fall["sni"]).to_owned()], "{name}");
                assert_eq!(
                    t.ssl().selected_alpn_protocol(),
                    Some(&b"http/1.1"[..]),
                    "{name}"
                );
                let _ = t.shutdown();
            }
        } else {
            let r = gemeinsam::Antwort::lies(&antwort);
            assert_eq!(
                i64::from(r.status),
                e["antwort"]["status"].as_i64().expect("status"),
                "{name}"
            );
            assert_eq!(r.koerper, latin1(text(&e["antwort"]["koerper"])), "{name}");
        }
        faden.join().expect("thread");
        assert_eq!(a.verbunden(), 0, "{name}: the target was touched");
        let alle = zeilen(&a.w.protokoll());
        if e["zeile"].is_null() {
            assert_eq!(alle.len(), vorher, "{name}");
        } else if e.get("zeile").is_some() {
            assert_eq!(alle.len(), vorher + 1, "{name}");
            pruefe_zeile(name, &alle[alle.len() - 1], &e["zeile"]);
        }
        drop(gehalten);
    }
}

/// One request in a tunnel to the target `name`; the answer of the harness.
fn im_tunnel(a: &Aufbau, ziel_name: &str, roh: &[u8]) -> Antwort {
    let z = a
        .liste
        .ziele
        .iter()
        .find(|z| z.name == ziel_name)
        .expect("target");
    let (mut h, faden) = a.verbinden();
    let connect = format!("CONNECT {}:{} HTTP/1.1\r\n\r\n", z.host, z.port);
    assert!(Aufbau::connect(&mut h, connect.as_bytes()).starts_with(b"HTTP/1.1 200 "));
    let mut t = a.tls(h, Some(&z.host)).expect("tls");
    t.write_all(roh).expect("send");
    let antwort = lies_antwort(&mut t);
    drop(t);
    faden.join().expect("thread");
    antwort
}

/// One case of `kopf-anfragen.json` or of the requests of `kopf-websocket.json`: the bytes at the target or the refusal,
/// and the line.
fn anfrage_fall(fall: &Value) {
    let name = text(&fall["name"]);
    let e = &fall["erwartet"];
    let a = aufbau(
        &vorfuehrung(),
        &[b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}".to_vec()],
    );
    let antwort = im_tunnel(&a, text(&fall["ziel"]), &latin1(text(&fall["anfrage"])));
    let gesehen: Vec<Vec<u8>> = a.skripte.iter().flat_map(Skript::gesehen).collect();
    // The line of the holder for a bound target; its `id` is not part of the expectation (it counts per connection).
    let an_halter: Vec<String> = a.halter.empfangen().iter().map(|z| ohne_id(z)).collect();
    if e["ziel_beruehrt"] == true && e.get("an_halter").is_some() {
        // A bound target: the request goes to the holder as one line, and the model path opens no connection of its own
        // to the target.
        assert!(gesehen.is_empty(), "{name}: the target was touched");
        assert_eq!(an_halter, vec![text(&e["an_halter"]).to_owned()], "{name}");
        assert!(
            !an_halter[0].contains("wb-platzhalter") && !an_halter[0].contains(MARKE),
            "{name}: a value went to the holder"
        );
    } else if e["ziel_beruehrt"] == true {
        assert_eq!(gesehen, vec![latin1(text(&e["an_ziel"]))], "{name}");
        assert!(
            !gemeinsam::enthaelt(&gesehen[0], b"wb-platzhalter"),
            "{name}"
        );
    } else {
        assert!(gesehen.is_empty(), "{name}");
        assert!(an_halter.is_empty(), "{name}: the holder was asked");
        assert_eq!(
            i64::from(antwort.status),
            e["status"].as_i64().expect("status"),
            "{name}"
        );
        assert_eq!(antwort.koerper, latin1(text(&e["koerper"])), "{name}");
    }
    pruefe_zeile(name, &letzte_zeile(&a, name), &e["zeile"]);
}

/// A line of the holder with its `id` replaced by `1`, so a case names the fields and not the count of a connection.
fn ohne_id(zeile: &str) -> String {
    let mut v: Value = serde_json::from_str(zeile).expect("line of the holder");
    if let Some(o) = v.as_object_mut() {
        o.insert("id".to_owned(), Value::from(1));
    }
    formate::kette::kanonisch(&v).expect("canonical")
}

/// The lines a stand-in holder answers for a case of `halter` (`beginn`, `teile`, `ende`).
fn halter_zeilen(h: &Value) -> Vec<String> {
    let mut zeilen = Vec::new();
    if let Some(b) = h["beginn"].as_object() {
        let kopf: Vec<String> = b["kopf"]
            .as_array()
            .expect("head")
            .iter()
            .map(|k| format!("{:?}:{:?}", text(&k[0]), text(&k[1])))
            .collect();
        zeilen.push(format!(
            r#"{{"art":"beginn","fassung":1,"id":<ID>,"kopf":{{{}}},"status":{}}}"#,
            kopf.join(","),
            b["status"].as_i64().expect("status")
        ));
    }
    for t in h["teile"].as_array().into_iter().flatten() {
        zeilen.push(format!(
            r#"{{"art":"teil","daten_b64":"{}","fassung":1,"id":<ID>}}"#,
            modellweg::b64::kodieren(&latin1(text(t)))
        ));
    }
    match h["ende"].as_str() {
        None => zeilen.push(r#"{"art":"ende","fassung":1,"id":<ID>,"ok":true}"#.to_owned()),
        Some(code) => zeilen.push(format!(
            r#"{{"art":"ende","code":"{code}","fassung":1,"grund":"Grund","id":<ID>,"ok":false}}"#
        )),
    }
    zeilen
}

#[test]
fn kopf_anfragen_cases() {
    let f = fall_datei("kopf-anfragen.json");
    for fall in f["faelle"].as_array().expect("cases") {
        anfrage_fall(fall);
    }
}

/// The head of a scripted answer of the target and its parts (chunked if the head says so).
fn ziel_teile(z: &Value) -> Vec<Vec<u8>> {
    let mut kopf = format!("HTTP/1.1 {} X\r\n", z["status"]);
    let mut chunked = false;
    for k in z["kopf"].as_array().expect("head") {
        chunked |= text(&k[0]).eq_ignore_ascii_case("transfer-encoding");
        kopf.push_str(text(&k[0]));
        kopf.push_str(": ");
        kopf.push_str(text(&k[1]));
        kopf.push_str("\r\n");
    }
    kopf.push_str("\r\n");
    let mut teile = vec![latin1(&kopf)];
    for t in z["teile"].as_array().expect("parts") {
        let t = latin1(text(t));
        teile.push(if chunked {
            [
                format!("{:x}\r\n", t.len()).into_bytes(),
                t,
                b"\r\n".to_vec(),
            ]
            .concat()
        } else {
            t
        });
    }
    if chunked {
        teile.push(b"0\r\n\r\n".to_vec());
    }
    teile
}

fn plan_host(a: &Aufbau) -> String {
    a.liste
        .ziele
        .iter()
        .find(|z| z.name == "copilot-plan")
        .expect("plan")
        .host
        .clone()
}

/// The upgrade request of the harness to the plan target.
fn upgrade_anfrage(plan: &str) -> String {
    format!(
        "GET /responses HTTP/1.1\r\nHost: {plan}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n"
    )
}

/// One case of `kopf-antworten.json` or of the answers of `kopf-websocket.json`: what reaches the harness, and the line.
fn antwort_fall(fall: &Value) {
    let name = text(&fall["name"]);
    let gebunden = fall.get("halter").is_some();
    let h_fall = &fall["halter"];
    let z = &fall["ziel_antwort"];
    let a = if gebunden {
        aufbau_mit_halter(&vorfuehrung(), halter_zeilen(h_fall))
    } else {
        aufbau(&vorfuehrung(), &ziel_teile(z))
    };
    let umstellen = gebunden && h_fall["upgrade"] == "durchreichen";
    let plan = plan_host(&a);
    let (ziel_name, anfrage) = if !gebunden {
        (
            "copilot-exp",
            "POST /api/v1/assignments HTTP/1.1\r\nHost: exp.individual.githubcopilot.com\r\nContent-Length: 2\r\n\r\n{}"
                .to_owned(),
        )
    } else if umstellen {
        ("copilot-plan", upgrade_anfrage(&plan))
    } else {
        (
            "copilot-plan",
            format!("GET /models HTTP/1.1\r\nHost: {plan}\r\n\r\n"),
        )
    };
    let antwort = im_tunnel(&a, ziel_name, anfrage.as_bytes());
    let h = &fall["erwartet"]["an_harness"];
    assert_eq!(
        i64::from(antwort.status),
        h["status"].as_i64().expect("status"),
        "{name}"
    );
    let kopf_soll: Vec<(String, String)> = h["kopf"]
        .as_array()
        .expect("head")
        .iter()
        .map(|k| (text(&k[0]).to_owned(), text(&k[1]).to_owned()))
        .collect();
    assert_eq!(antwort.kopf, kopf_soll, "{name}");
    if text(&h["art"]) == "abgerissen" {
        assert_eq!(antwort.koerper, latin1(text(&h["gesendet"])), "{name}");
        assert!(!antwort.ganz || umstellen, "{name}");
    } else {
        assert_eq!(antwort.koerper, latin1(text(&h["koerper"])), "{name}");
        // a whole answer ends by its framing: a length that fits, or the last chunk `0`
        assert!(
            antwort.ganz,
            "{name}: the answer does not end by its framing"
        );
    }
    assert!(
        !gemeinsam::enthaelt(&antwort.koerper, MARKE.as_bytes()),
        "{name}"
    );
    pruefe_zeile(name, &letzte_zeile(&a, name), &fall["erwartet"]["zeile"]);
}

#[test]
fn kopf_antworten_cases() {
    let f = fall_datei("kopf-antworten.json");
    for fall in f["faelle"].as_array().expect("cases") {
        antwort_fall(fall);
    }
}

#[test]
fn kopf_websocket_anfragen_cases() {
    let f = fall_datei("kopf-websocket.json");
    let faelle = f["anfragen"].as_array().expect("cases");
    assert_eq!(faelle.len(), 3);
    for fall in faelle {
        anfrage_fall(fall);
    }
}

#[test]
fn kopf_websocket_antworten_cases() {
    let f = fall_datei("kopf-websocket.json");
    let faelle = f["antworten"].as_array().expect("cases");
    assert_eq!(faelle.len(), 9);
    for fall in faelle {
        antwort_fall(fall);
    }
}

/// Reads a head up to the empty line, byte for byte (the switched stream behind it stays unread).
fn lies_kopf<R: Read>(r: &mut R) -> Vec<u8> {
    let mut kopf = Vec::new();
    let mut b = [0u8; 1];
    while !kopf.ends_with(b"\r\n\r\n") {
        match r.read(&mut b) {
            Ok(1) => kopf.push(b[0]),
            _ => panic!("no head: {:?}", String::from_utf8_lossy(&kopf)),
        }
    }
    kopf
}

/// W6: after a 101 of the bound plan target the bytes of the harness go into the stream of the holder unchanged,
/// without a check and without a line of their own. From step I they go as lines `strom.teil` with the `id` of the
/// stream; the way out to the target belongs to the holder.
#[test]
fn kopf_websocket_von_harness_cases() {
    let f = fall_datei("kopf-websocket.json");
    let faelle = f["von_harness"].as_array().expect("cases");
    assert_eq!(faelle.len(), 2);
    for fall in faelle {
        let name = text(&fall["name"]);
        let a = aufbau_mit_halter(&vorfuehrung(), halter_101());
        let plan = plan_host(&a);
        let (mut h, faden) = a.verbinden();
        let connect = format!("CONNECT {plan}:443 HTTP/1.1\r\n\r\n");
        assert!(Aufbau::connect(&mut h, connect.as_bytes()).starts_with(b"HTTP/1.1 200 "));
        let mut t = a.tls(h, Some(&plan)).expect("tls");
        t.write_all(upgrade_anfrage(&plan).as_bytes())
            .expect("send");
        assert!(lies_kopf(&mut t).starts_with(b"HTTP/1.1 101 "), "{name}");
        for teil in fall["von_harness"].as_array().expect("parts") {
            t.write_all(&latin1(text(teil))).expect("send");
            t.flush().expect("flush");
        }
        let soll = latin1(text(&fall["erwartet"]["an_dienst"]));
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let gesehen = loop {
            let g = strom_bytes(&a);
            if g.len() >= soll.len() {
                break g;
            }
            assert!(std::time::Instant::now() < ende, "{name}: {g:?}");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(gesehen, soll, "{name}");
        assert!(fall["erwartet"]["zeile"].is_null(), "{name}");
        assert!(
            zeilen(&a.w.protokoll()).is_empty(),
            "{name}: no line per message"
        );
        drop(t);
        faden.join().expect("thread");
        let z = zeilen(&a.w.protokoll());
        assert_eq!(z.len(), 1, "{name}: one line at the end of the connection");
        assert_eq!(
            (&z[0]["ergebnis"], &z[0]["status"]),
            (&Value::from("weitergereicht"), &Value::from(101)),
            "{name}"
        );
        // The stream ended on the side of the harness, so the model path sent `strom.ende` with its `id`.
        let letzte = a.halter.empfangen();
        assert!(
            letzte.last().expect("line").contains("strom.ende"),
            "{name}: {letzte:?}"
        );
    }
}

/// The whole frames in `b`: their length in bytes and whether each is a data frame (opcode under 0x8).
fn ganze_rahmen(b: &[u8]) -> Vec<(usize, bool)> {
    let mut v = Vec::new();
    let mut i = 0;
    while let (Some(&b0), Some(&b1)) = (b.get(i), b.get(i + 1)) {
        let (mut kopf, laenge) = match b1 & 0x7f {
            126 => match b.get(i + 2..i + 4) {
                Some(l) => (4, u64::from(u16::from_be_bytes([l[0], l[1]]))),
                None => break,
            },
            127 => match b.get(i + 2..i + 10) {
                Some(l) => (10, u64::from_be_bytes(l.try_into().expect("8 bytes"))),
                None => break,
            },
            n => (2, u64::from(n)),
        };
        if b1 & 0x80 != 0 {
            kopf += 4;
        }
        let Ok(ganz) = usize::try_from(laenge).map(|l| l + kopf) else {
            break;
        };
        if b.len() < i + ganz {
            break;
        }
        v.push((ganz, b0 & 0x08 == 0));
        i += ganz;
    }
    v
}

/// The bytes that went towards the target after the upgrade request: from step I they are the payload of the lines
/// `strom.teil` that the model path sent to the holder (the way out belongs to him).
fn nach_kopf(a: &Aufbau) -> Vec<u8> {
    strom_bytes(a)
}

/// Waits until that many bytes went into the stream after the upgrade request.
fn warte_an_ziel(a: &Aufbau, n: usize, name: &str) -> Vec<u8> {
    let ende = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let g = nach_kopf(a);
        if g.len() >= n {
            return g;
        }
        assert!(std::time::Instant::now() < ende, "{name}: {g:?}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// One case of `faelle/probelauf-websocket.json` (section 9, Z1 to Z3) through the tunnel: the upgrade request to the
/// bound plan target, a 101, then the frames of the harness part by part. After each part the target has the whole
/// frames so far (unchanged, masked) and the count file the state after the last whole data frame; at the end the
/// bytes at the target, the count file and the line are exactly as expected.
#[allow(
    clippy::too_many_lines,
    reason = "one case is one sequence on one stream; its steps share the stream, the gate and the folder"
)]
fn probelauf_ws_fall(fall: &Value) {
    let name = text(&fall["name"]);
    let e = &fall["erwartet"];
    let w = Wegwerf::neu("pws");
    let ordner = w.pfad("zaehlung");
    fs::create_dir(&ordner).expect("folder");
    fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("chmod");
    let datei = ordner.join("marker.json");
    let probelauf = fall["probelauf"] == true;
    let zaehlung = probelauf.then(|| {
        let marker: Vec<String> = fall["marker"]
            .as_array()
            .expect("markers")
            .iter()
            .map(|m| text(m).to_owned())
            .collect();
        let z = Zaehlung::neu(marker, datei.clone(), "s-1".to_owned());
        z.anfang().expect("first state");
        z
    });
    let a = aufbau_halter_probelauf(w, &vorfuehrung(), zaehlung, halter_101());
    let plan = plan_host(&a);
    let (mut h, faden) = a.verbinden();
    let connect = format!("CONNECT {plan}:443 HTTP/1.1\r\n\r\n");
    assert!(Aufbau::connect(&mut h, connect.as_bytes()).starts_with(b"HTTP/1.1 200 "));
    let mut t = a.tls(h, Some(&plan)).expect("tls");
    t.write_all(upgrade_anfrage(&plan).as_bytes())
        .expect("send");
    assert!(lies_kopf(&mut t).starts_with(b"HTTP/1.1 101 "), "{name}");
    let lies_datei = || {
        fs::read(&datei)
            .ok()
            .map(|d| String::from_utf8(d).expect("utf-8"))
    };
    assert_eq!(
        lies_datei().as_deref(),
        e["datei_anfang"].as_str(),
        "{name}: the upgrade request is counted"
    );
    let an_ziel = latin1(text(&e["an_ziel"]));
    let folge: Vec<&str> = e["datei_folge"]
        .as_array()
        .expect("states")
        .iter()
        .map(text)
        .collect();
    let hinaus = e["rahmen_hinaus"]
        .as_u64()
        .map(|n| usize::try_from(n).expect("count"));
    let ab = fall["zaehldatei_nicht_schreibbar_ab_datenrahmen"]
        .as_u64()
        .map(|n| usize::try_from(n).expect("count"));
    let mut gesendet = Vec::new();
    for teil in fall["von_harness"].as_array().expect("parts") {
        let daten_vorher = ganze_rahmen(&gesendet).iter().filter(|(_, d)| *d).count();
        if ab.is_some_and(|ab| daten_vorher + 1 >= ab) {
            fs::set_permissions(&ordner, fs::Permissions::from_mode(0o500)).expect("chmod");
        }
        gesendet.extend(latin1(text(teil)));
        let rahmen = ganze_rahmen(&gesendet);
        if t.write_all(&latin1(text(teil)))
            .and_then(|()| t.flush())
            .is_err()
        {
            break;
        }
        let Some(hinaus) = hinaus else { continue };
        let fertig = &rahmen[..rahmen.len().min(hinaus)];
        let bytes: usize = fertig.iter().map(|(n, _)| n).sum();
        let datenrahmen = fertig.iter().filter(|(_, d)| *d).count();
        let g = warte_an_ziel(&a, bytes, name);
        assert!(
            an_ziel.starts_with(&g),
            "{name}: unchanged and in order: {g:?}"
        );
        let soll = if datenrahmen == 0 {
            text(&e["datei_anfang"])
        } else {
            folge[datenrahmen - 1]
        };
        assert_eq!(
            lies_datei().as_deref(),
            Some(soll),
            "{name}: after {datenrahmen} data frames"
        );
    }
    if hinaus.is_none() {
        warte_an_ziel(&a, an_ziel.len(), name);
    }
    if e["abriss"].is_null() {
        std::thread::sleep(std::time::Duration::from_millis(60));
        drop(t);
    } else {
        let mut rest = Vec::new();
        let _ = t.read_to_end(&mut rest);
        assert!(rest.is_empty(), "{name}: nothing more to the harness");
        drop(t);
    }
    faden.join().expect("thread");
    fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("chmod");
    assert_eq!(nach_kopf(&a), an_ziel, "{name}: the bytes at the target");
    assert_eq!(
        lies_datei().as_deref(),
        e["datei_ende"].as_str(),
        "{name}: the count file at the end"
    );
    if probelauf {
        assert_eq!(
            fs::metadata(&datei).expect("meta").permissions().mode() & 0o777,
            0o600,
            "{name}"
        );
        assert_eq!(
            fs::read_dir(&ordner).expect("dir").count(),
            1,
            "{name}: a temporary file was left"
        );
    }
    let z = zeilen(&a.w.protokoll());
    assert_eq!(z.len(), 1, "{name}: one line at the end of the connection");
    pruefe_zeile(name, &z[0], &e["zeile"]);
    assert_eq!(z[0]["status"], 101, "{name}");
}

/// Section 9, Z1 to Z3 (amendment 17): every case of `faelle/probelauf-websocket.json` through the tunnel.
#[test]
fn probelauf_websocket_cases() {
    let f = fall_datei("probelauf-websocket.json");
    let faelle = f["faelle"].as_array().expect("cases");
    assert_eq!(faelle.len(), 27);
    let namen: Vec<&str> = faelle.iter().map(|f| text(&f["name"])).collect();
    assert!(namen.contains(&"ws_marker_ein_und_geteilt"));
    assert!(namen.contains(&"ws_ohne_probelauf"));
    for fall in faelle {
        probelauf_ws_fall(fall);
    }
}

/// The required case: one marker in one frame and one spread over two frames, both counted (Z1).
#[test]
fn probelauf_websocket_one_marker_whole_and_one_split() {
    let f = fall_datei("probelauf-websocket.json");
    let fall = f["faelle"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|f| f["name"] == "ws_marker_ein_und_geteilt")
        .expect("the case");
    let ende = text(&fall["erwartet"]["datei_ende"]);
    assert!(ende.contains(r#""MK-INTERN":1"#), "{ende}");
    assert!(ende.contains(r#""MK-KUNDE":1"#), "{ende}");
    probelauf_ws_fall(fall);
}

/// W6 stays without the trial run: every byte of the harness reaches the target as sent, even frames that the trial
/// run would refuse (no mask, RSV1, opcode 0x3), and nothing is written.
#[test]
fn without_trial_run_the_harness_bytes_arrive_unchanged() {
    let f = fall_datei("probelauf-websocket.json");
    let fall = f["faelle"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|f| f["name"] == "ws_ohne_probelauf")
        .expect("the case");
    let teile: Vec<u8> = fall["von_harness"]
        .as_array()
        .expect("parts")
        .iter()
        .flat_map(|t| latin1(text(t)))
        .collect();
    assert_eq!(teile, latin1(text(&fall["erwartet"]["an_ziel"])));
    probelauf_ws_fall(fall);
}

#[test]
fn kopf_gegenprobe_user_repos() {
    let f = fall_datei("kopf-gegenprobe.json");
    for fall in f["faelle"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let Some(soll) = fall["erwartet"]["user_repos"].as_str() else {
            continue;
        };
        // Das Ziel ist gebunden: der Weg nach aussen gehoert dem Halter, und gelungen heisst, dass die Anfrage ihn
        // als eine Zeile erreicht hat.
        let a = aufbau_mit_halter(
            text(&fall["aufbau"]["zielliste"]),
            halter_antwort(200, "[]", None),
        );
        let antwort = im_tunnel(
            &a,
            "github-api",
            b"GET /user/repos HTTP/1.1\r\nHost: api.github.com\r\n\r\n",
        );
        let gelungen = antwort.status == 200 && a.halter.empfangen().len() == 1;
        assert_eq!(
            if gelungen { "gelingt" } else { "abgelehnt" },
            soll,
            "{name}"
        );
    }
}

#[test]
fn a_tunnel_takes_one_request_after_another() {
    let a = aufbau_mit_halter(&vorfuehrung(), halter_antwort(200, "{}", None));
    let plan = a
        .liste
        .ziele
        .iter()
        .find(|z| z.name == "copilot-plan")
        .expect("plan")
        .host
        .clone();
    let (mut h, faden) = a.verbinden();
    let connect = format!("CONNECT {plan}:443 HTTP/1.1\r\n\r\n");
    assert!(Aufbau::connect(&mut h, connect.as_bytes()).starts_with(b"HTTP/1.1 200 "));
    let mut t = a.tls(h, Some(&plan)).expect("tls");
    for _ in 0..2 {
        t.write_all(format!("GET /models HTTP/1.1\r\nHost: {plan}\r\n\r\n").as_bytes())
            .expect("send");
        let antwort = lies_antwort(&mut t);
        assert_eq!(
            (antwort.status, antwort.koerper.as_slice()),
            (200, &b"{}"[..])
        );
    }
    drop(t);
    faden.join().expect("thread");
    // The way out of a bound target belongs to the holder: the model path opened no connection of its own, and each of
    // the two requests went to him as one line of its own.
    assert_eq!(a.verbunden(), 0);
    assert_eq!(a.halter.empfangen().len(), 2);
    assert_eq!(zeilen(&a.w.protokoll()).len(), 2);
}

/// One answer of the bound plan target to `anfrage` (`PLAN` stands for its host), the holder answering with `zeilen`.
fn plan_antwort(zeilen: Vec<String>, anfrage: &str) -> (Antwort, Aufbau) {
    let a = aufbau_mit_halter(&vorfuehrung(), zeilen);
    let plan = plan_host(&a);
    let roh = anfrage.replace("PLAN", &plan);
    let antwort = im_tunnel(&a, "copilot-plan", roh.as_bytes());
    (antwort, a)
}

/// The lines of a plain answer of the holder: `beginn` with `status`, one `teil` with `koerper`, then `ende`.
fn halter_antwort(status: u16, koerper: &str, ende: Option<&str>) -> Vec<String> {
    let mut z = vec![format!(
        r#"{{"art":"beginn","fassung":1,"id":<ID>,"kopf":{{"content-type":"application/json"}},"status":{status}}}"#
    )];
    if !koerper.is_empty() {
        z.push(format!(
            r#"{{"art":"teil","daten_b64":"{}","fassung":1,"id":<ID>}}"#,
            modellweg::b64::kodieren(koerper.as_bytes())
        ));
    }
    z.push(match ende {
        None => r#"{"art":"ende","fassung":1,"id":<ID>,"ok":true}"#.to_owned(),
        Some(code) => format!(
            r#"{{"art":"ende","code":"{code}","fassung":1,"grund":"Grund","id":<ID>,"ok":false}}"#
        ),
    });
    z
}

/// A stream of the holder that ends with a code after the head went out: the answer to the harness keeps the head and
/// the bytes that came, and it never ends by its framing. Nothing is held back: the model path has no key for a bound
/// target, so its leak lock does not run here (that lock is the holder's).
#[test]
fn a_broken_answer_keeps_one_head() {
    let anfrage = "GET /models HTTP/1.1\r\nHost: PLAN\r\n\r\n";
    let (kurz, _a) = plan_antwort(halter_antwort(200, "", Some("halter.ziel_fehler")), anfrage);
    assert_eq!(
        (kurz.status, kurz.koerper.as_slice(), kurz.ganz),
        (
            502,
            &b"{\"error\":{\"code\":\"mw.halter_fehler\",\"message\":\"Halter lehnte ab\"}}"[..],
            true
        ),
        "no part went out yet, so the own answer comes"
    );
    let lang = "x".repeat(100);
    let (b, _a) = plan_antwort(
        halter_antwort(200, &lang, Some("halter.ziel_fehler")),
        anfrage,
    );
    assert_eq!(b.status, 200);
    assert!(!b.ganz);
    assert_eq!(b.koerper, lang.as_bytes());
}

/// An upgrade that the holder answers with a plain status runs like a call; a 101 without a requested upgrade is no
/// stream either. The body always goes chunked to the harness: the holder sends no `content-length` (it stands on his
/// block list).
#[test]
fn only_a_requested_upgrade_with_101_is_bridged() {
    let (a, aufbau_a) = plan_antwort(
        halter_antwort(200, "{}", None),
        "GET /responses HTTP/1.1\r\nHost: PLAN\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
    );
    assert_eq!((a.status, a.koerper.as_slice()), (200, &b"{}"[..]));
    assert!(
        a.kopf
            .contains(&("transfer-encoding".to_owned(), "chunked".to_owned())),
        "{:?}",
        a.kopf
    );
    // The upgrade went out as `modell.strom`, and the answer was no 101.
    let erste = aufbau_a.halter.empfangen();
    assert!(erste[0].contains(r#""op":"modell.strom""#), "{erste:?}");
    // A 101 of the holder without a requested upgrade: no stream, the status goes out as it is.
    let (b, _aufbau_b) = plan_antwort(
        {
            let mut z = halter_101();
            z.push(format!(
                r#"{{"art":"teil","daten_b64":"{}","fassung":1,"id":<ID>}}"#,
                modellweg::b64::kodieren(b"daten")
            ));
            z.push(r#"{"art":"ende","fassung":1,"id":<ID>,"ok":true}"#.to_owned());
            z
        },
        "GET /responses HTTP/1.1\r\nHost: PLAN\r\n\r\n",
    );
    assert_eq!(b.status, 101);
    assert!(
        b.kopf
            .contains(&("transfer-encoding".to_owned(), "chunked".to_owned())),
        "{:?}",
        b.kopf
    );
    assert!(
        !b.kopf
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case("upgrade")),
        "{:?}",
        b.kopf
    );
}

/// The request of the harness to the unbound target `copilot-exp` that the answer cases use.
const EXP_ANFRAGE: &[u8] = b"POST /api/v1/assignments HTTP/1.1\r\nHost: exp.individual.githubcopilot.com\r\nContent-Length: 2\r\n\r\n{}";

/// Section 13.3: the status and the head of an unbound target go to the harness as they came, also when its body breaks
/// before the first byte. The harness gets the head and then the end, never an answer of its own and never nothing; the
/// line says `weitergereicht` with the status that went out.
#[test]
fn a_body_that_breaks_before_its_first_byte_keeps_the_head() {
    let a = aufbau(
        &vorfuehrung(),
        &[
            b"HTTP/1.1 200 X\r\nContent-Type: application/json\r\nContent-Length: 10\r\n\r\n"
                .to_vec(),
        ],
    );
    let antwort = im_tunnel(&a, "copilot-exp", EXP_ANFRAGE);
    assert_eq!(antwort.status, 200);
    assert_eq!(
        antwort.kopf,
        vec![
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("content-length".to_owned(), "10".to_owned()),
        ]
    );
    assert_eq!(
        (antwort.koerper.as_slice(), antwort.ganz),
        (&b""[..], false)
    );
    let z = letzte_zeile(&a, "broken body");
    assert_eq!(
        (&z["ergebnis"], &z["grund"], &z["status"]),
        (
            &Value::from("weitergereicht"),
            &Value::Null,
            &Value::from(200)
        )
    );
}

/// Opens the switched stream to the bound plan target: CONNECT, TLS, the upgrade request and the head of the 101.
fn strom_oeffnen(a: &Aufbau) -> (SslStream<UnixStream>, std::thread::JoinHandle<()>) {
    let plan = plan_host(a);
    let (mut h, faden) = a.verbinden();
    let connect = format!("CONNECT {plan}:443 HTTP/1.1\r\n\r\n");
    assert!(Aufbau::connect(&mut h, connect.as_bytes()).starts_with(b"HTTP/1.1 200 "));
    let mut t = a.tls(h, Some(&plan)).expect("tls");
    t.write_all(upgrade_anfrage(&plan).as_bytes())
        .expect("send");
    assert!(lies_kopf(&mut t).starts_with(b"HTTP/1.1 101 "));
    (t, faden)
}

/// A line `teil` of the holder with `daten`.
fn teil_zeile(daten: &[u8]) -> String {
    format!(
        r#"{{"art":"teil","daten_b64":"{}","fassung":1,"id":<ID>}}"#,
        modellweg::b64::kodieren(daten)
    )
}

/// Waits up to 5 s for the end of the thread of the service, so that a stream that never ends fails the test.
fn faden_endet(faden: std::thread::JoinHandle<()>, name: &str) {
    let ende = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !faden.is_finished() {
        assert!(
            std::time::Instant::now() < ende,
            "{name}: the stream still runs"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    faden.join().expect("thread");
}

/// The line of the stream: result, reason and status.
fn strom_zeile(a: &Aufbau, name: &str) -> (Value, Value, Value) {
    let z = letzte_zeile(a, name);
    (
        z["ergebnis"].clone(),
        z["grund"].clone(),
        z["status"].clone(),
    )
}

/// Section 5.1: after the harness closed its way, `strom.ende` goes to the holder, and every part he still sends before
/// his `ende` goes to the harness, in order. The stream then ends as `weitergereicht` with status 101.
#[test]
fn after_the_harness_closed_the_last_parts_of_the_holder_still_reach_it() {
    let name = "last parts";
    let a = aufbau_mit_halter(&vorfuehrung(), halter_101());
    a.halter.schluss_setzen(vec![
        teil_zeile(b"eins"),
        teil_zeile(b"zwei"),
        r#"{"art":"ende","fassung":1,"id":<ID>,"ok":true}"#.to_owned(),
    ]);
    let (mut t, faden) = strom_oeffnen(&a);
    t.shutdown().expect("close_notify");
    let mut rest = Vec::new();
    let _ = t.read_to_end(&mut rest);
    faden_endet(faden, name);
    assert_eq!(rest, b"einszwei", "{name}");
    assert!(
        a.halter
            .empfangen()
            .last()
            .is_some_and(|z| z.contains(r#""op":"strom.ende""#)),
        "{name}"
    );
    assert_eq!(
        strom_zeile(&a, name),
        (Value::from("weitergereicht"), Value::Null, Value::from(101))
    );
}

/// Section 5.1: a code of the holder after the harness closed its way is mapped like every other code of his:
/// `halter.ziel_komprimiert` becomes `mw.antwort_komprimiert`, with status 101 in the line.
#[test]
fn a_code_of_the_holder_after_the_harness_closed_is_mapped_too() {
    let name = "late code";
    let a = aufbau_mit_halter(&vorfuehrung(), halter_101());
    a.halter.schluss_setzen(vec![
        teil_zeile(b"eins"),
        r#"{"art":"ende","code":"halter.ziel_komprimiert","fassung":1,"grund":"Grund","id":<ID>,"ok":false}"#
            .to_owned(),
    ]);
    let (mut t, faden) = strom_oeffnen(&a);
    t.shutdown().expect("close_notify");
    let mut rest = Vec::new();
    let _ = t.read_to_end(&mut rest);
    faden_endet(faden, name);
    assert_eq!(rest, b"eins", "{name}");
    assert_eq!(
        strom_zeile(&a, name),
        (
            Value::from("ziel_fehler"),
            Value::from("mw.antwort_komprimiert"),
            Value::from(101)
        )
    );
}

/// A harness that is quiet for a while keeps its stream: a passed read deadline of one round is no end, and the bytes
/// that come later still go to the holder as `strom.teil`, without a `strom.ende` before them.
#[test]
fn a_quiet_harness_keeps_its_stream_open() {
    let name = "quiet harness";
    let a = aufbau_mit_halter(&vorfuehrung(), halter_101());
    let (mut t, faden) = strom_oeffnen(&a);
    // many rounds of polling pass without a byte of the harness
    std::thread::sleep(std::time::Duration::from_millis(300));
    let rahmen = b"\x81\x82abcdXY";
    t.write_all(rahmen).expect("send");
    t.flush().expect("flush");
    assert_eq!(warte_an_ziel(&a, rahmen.len(), name), rahmen, "{name}");
    assert!(
        !a.halter
            .empfangen()
            .iter()
            .any(|z| z.contains(r#""op":"strom.ende""#)),
        "{name}: the stream ended before the harness did"
    );
    drop(t);
    faden_endet(faden, name);
}

/// A way of the harness that breaks (here a TLS record that is longer than the protocol allows) is an end of that way,
/// not a passed deadline: `strom.ende` goes to the holder, and the stream ends with his `ende`.
#[test]
fn a_broken_way_of_the_harness_ends_the_stream() {
    let name = "broken way";
    let a = aufbau_mit_halter(&vorfuehrung(), halter_101());
    let (mut t, faden) = strom_oeffnen(&a);
    t.get_mut()
        .write_all(b"\x17\x03\x03\xff\xff")
        .expect("send");
    faden_endet(faden, name);
    endet_mit_strom_ende(&a, name);
}

/// The end of the service tears every connection down: a stream over the holder ends too. From then on every read of the
/// way of the harness fails; that is an end of that way, never a passed deadline, so `strom.ende` goes to the holder and
/// the thread of the stream ends.
#[test]
fn a_tear_down_ends_a_stream_over_the_holder() {
    let name = "tear-down";
    let a = aufbau_mit_halter(&vorfuehrung(), halter_101());
    let (t, faden) = strom_oeffnen(&a);
    a.dienst.betrieb().abreissen_alle();
    faden_endet(faden, name);
    endet_mit_strom_ende(&a, name);
    drop(t);
}

/// The holder of these tests answers `strom.ende` with nothing at all: a model path that still waits for his `ende`
/// after its way to the harness failed holds the thread, and `faden_endet` fails.
fn halter_ohne_schluss(zeilen: Vec<String>) -> Aufbau {
    let a = aufbau_mit_halter(&vorfuehrung(), zeilen);
    a.halter.schluss_setzen(Vec::new());
    a
}

/// The end of a failed way (section 5.1): `strom.ende` is the last line at the holder, and the stream ends as
/// `weitergereicht` with status 101. The stand-in reads in a thread of its own, so the line may reach its list a little
/// after the thread of the stream ended; the test waits up to 5 s for it.
fn endet_mit_strom_ende(a: &Aufbau, name: &str) {
    let ist_ende = |e: &[String]| e.last().is_some_and(|z| z.contains(r#""op":"strom.ende""#));
    let ende = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !ist_ende(&a.halter.empfangen()) && std::time::Instant::now() < ende {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        ist_ende(&a.halter.empfangen()),
        "{name}: {:?}",
        a.halter.empfangen().last()
    );
    assert_eq!(
        strom_zeile(a, name),
        (Value::from("weitergereicht"), Value::Null, Value::from(101)),
        "{name}"
    );
}

/// Section 5.1: a way of the harness that failed (a TLS record longer than the protocol allows) ends the stream with
/// `strom.ende` at once. Nothing reaches the harness any more, so the model path does not wait for the `ende` of the
/// holder; a model path that took the failure for a passed deadline reads on and then waits for that `ende`.
#[test]
fn a_failed_way_of_the_harness_does_not_wait_for_the_holder() {
    let name = "failed way, silent holder";
    let a = halter_ohne_schluss(halter_101());
    let (mut t, faden) = strom_oeffnen(&a);
    t.get_mut()
        .write_all(b"\x17\x03\x03\xff\xff")
        .expect("send");
    faden_endet(faden, name);
    endet_mit_strom_ende(&a, name);
}

/// Section 5.1: after the tear-down every read of the way of the harness fails, also one that the TLS layer reports
/// as a passed deadline. `strom.ende` goes out at once, and the thread does not wait for the holder.
#[test]
fn a_tear_down_does_not_wait_for_the_holder() {
    let name = "tear-down, silent holder";
    let a = halter_ohne_schluss(halter_101());
    let (t, faden) = strom_oeffnen(&a);
    a.dienst.betrieb().abreissen_alle();
    faden_endet(faden, name);
    endet_mit_strom_ende(&a, name);
    drop(t);
}

/// The holder of these tests has 500 lines `teil` waiting behind his 101.
fn halter_mit_500_teilen() -> Aufbau {
    let mut zeilen = halter_101();
    zeilen.extend((0..500).map(|_| teil_zeile(b"x")));
    halter_ohne_schluss(zeilen)
}

/// Section 5.1, with 500 lines `teil` of the holder waiting: the tear-down may first show as a failed write to the
/// harness. That is a failed way too, so `strom.ende` goes out before the model path closes its connection to the
/// holder.
#[test]
fn a_tear_down_with_waiting_parts_ends_with_strom_ende() {
    let name = "tear-down, 500 parts";
    let a = halter_mit_500_teilen();
    let (t, faden) = strom_oeffnen(&a);
    a.dienst.betrieb().abreissen_alle();
    faden_endet(faden, name);
    endet_mit_strom_ende(&a, name);
    drop(t);
}

/// Section 5.1, with 500 lines `teil` of the holder waiting: a harness that takes nothing more (it shut its reading
/// half) fails the first write of a part, while every read of its way still only passes its deadline. That failed
/// write ends the stream with `strom.ende` too.
#[test]
fn a_failed_write_to_the_harness_ends_with_strom_ende() {
    let name = "failed write, 500 parts";
    let a = halter_mit_500_teilen();
    let (t, faden) = strom_oeffnen(&a);
    t.get_ref()
        .shutdown(std::net::Shutdown::Read)
        .expect("half close");
    faden_endet(faden, name);
    endet_mit_strom_ende(&a, name);
    drop(t);
}

/// Section 5.1, with 500 lines `teil` of the holder waiting: a harness that goes away without a word fails the way
/// as well, by a read or by a write; either way `strom.ende` goes out.
#[test]
fn a_harness_gone_with_waiting_parts_ends_with_strom_ende() {
    let name = "harness gone, 500 parts";
    let a = halter_mit_500_teilen();
    let (t, faden) = strom_oeffnen(&a);
    drop(t);
    faden_endet(faden, name);
    endet_mit_strom_ende(&a, name);
}
