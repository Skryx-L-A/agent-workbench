//! M8 of section 12 without touching the truststore of the system: a throw-away CA exists only in this test. The TLS of
//! the model path trusts it only when the OpenSSL default path points to it (`SSL_CERT_FILE`, the same mechanism as the
//! system store on Linux); with the empty environment of production it fails with `mw.ziel_nicht_erreichbar`. A wrong
//! host name fails too, and a silent handshake ends within the deadline.
//!
//! Since stage 0 (section 3, F1) the program refuses to start with `SSL_CERT_FILE` in its environment. The trusting half
//! therefore runs the library in a child process of this test binary that has the variable (`im_kind`); the program
//! itself is started only with the empty environment, and with the variable it must end with exit 78.
#![cfg(target_os = "linux")]

mod gemeinsam;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gemeinsam::{
    Antwort, Prozess, SCHLUESSEL, Wegwerf, anfrage, argv, koerper, lies_anfrage, post,
    ueber_socket, zeilen,
};
use modellweg::start::vorbereiten;
use modellweg::ziel::Fristen;
use openssl::asn1::Asn1Time;
use openssl::bn::BigNum;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::ssl::{SslAcceptor, SslMethod};
use openssl::x509::extension::{BasicConstraints, SubjectAlternativeName};
use openssl::x509::{X509, X509NameBuilder};

fn schluessel() -> PKey<Private> {
    let gruppe = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("curve");
    PKey::from_ec_key(EcKey::generate(&gruppe).expect("key")).expect("pkey")
}

fn zertifikat(
    name: &str,
    pk: &PKey<Private>,
    aussteller: Option<(&X509, &PKey<Private>)>,
    ca: bool,
) -> X509 {
    let mut n = X509NameBuilder::new().expect("name");
    n.append_entry_by_text("CN", name).expect("cn");
    let n = n.build();
    let mut b = X509::builder().expect("builder");
    b.set_version(2).expect("version");
    let serial = if ca { 1 } else { 2 };
    b.set_serial_number(
        &BigNum::from_u32(serial)
            .expect("bn")
            .to_asn1_integer()
            .expect("serial"),
    )
    .expect("serial");
    b.set_subject_name(&n).expect("subject");
    match aussteller {
        Some((c, _)) => b.set_issuer_name(c.subject_name()).expect("issuer"),
        None => b.set_issuer_name(&n).expect("issuer"),
    }
    b.set_pubkey(pk).expect("pubkey");
    b.set_not_before(&Asn1Time::days_from_now(0).expect("time"))
        .expect("before");
    b.set_not_after(&Asn1Time::days_from_now(2).expect("time"))
        .expect("after");
    if ca {
        b.append_extension(BasicConstraints::new().critical().ca().build().expect("bc"))
            .expect("ext");
    } else {
        let san = SubjectAlternativeName::new()
            .dns(name)
            .build(&b.x509v3_context(aussteller.map(|(c, _)| &**c), None))
            .expect("san");
        b.append_extension(san).expect("ext");
    }
    b.sign(aussteller.map_or(pk, |(_, k)| k), MessageDigest::sha256())
        .expect("sign");
    b.build()
}

/// A TLS stand-in on the loopback with a certificate for `name` from a throw-away CA; it answers 200 `{}`.
/// Returns its address and the CA file.
fn tls_attrappe(w: &Wegwerf, name: &str) -> (SocketAddr, std::path::PathBuf) {
    let ca_key = schluessel();
    let ca = zertifikat("mw-test-ca", &ca_key, None, true);
    let key = schluessel();
    let cert = zertifikat(name, &key, Some((&ca, &ca_key)), false);
    let ca_datei = w.pfad("ca.pem");
    std::fs::write(&ca_datei, ca.to_pem().expect("pem")).expect("write the CA");
    let mut b = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).expect("acceptor");
    b.set_private_key(&key).expect("key");
    b.set_certificate(&cert).expect("cert");
    let acceptor = Arc::new(b.build());
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let adresse = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for strom in listener.incoming() {
            let Ok(strom) = strom else { continue };
            let acceptor = Arc::clone(&acceptor);
            std::thread::spawn(move || {
                let Ok(mut s) = acceptor.accept(strom) else {
                    return;
                };
                if lies_anfrage(&mut s).is_some() {
                    let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}");
                    let _ = s.flush();
                }
                let mut rest = Vec::new();
                let _ = s.read_to_end(&mut rest);
            });
        }
    });
    (adresse, ca_datei)
}

/// The variable that makes a run of this binary the child of `im_kind`: the port of the stand-in.
const KIND_PORT: &str = "MW_TLS_KIND_PORT";

/// Runs the test `kind_bis_zum_ziel` of this binary as a child with `SSL_CERT_FILE=ca` and the stand-in at `port`;
/// returns the status the model path answered (from the line `status=<n>` of the child).
fn im_kind(ca: &std::path::Path, port: u16) -> u16 {
    let aus = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "kind_bis_zum_ziel",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("SSL_CERT_FILE", ca)
        .env(KIND_PORT, port.to_string())
        .stdin(std::process::Stdio::null())
        .output()
        .expect("start the child");
    assert!(aus.status.success(), "{aus:?}");
    let text = String::from_utf8_lossy(&aus.stdout);
    text.lines()
        .find_map(|z| z.split_once("status=").map(|(_, n)| n))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or_else(|| panic!("no status in {text}"))
}

/// Only as the child of `im_kind` (else nothing): the library of the model path with the TLS of the system, one request
/// to `https://localhost:<port>/v1`; writes `status=<n>`.
#[test]
fn kind_bis_zum_ziel() {
    let Some(port) = std::env::var_os(KIND_PORT) else {
        return;
    };
    let port: u16 = port.to_str().expect("port").parse().expect("port");
    let w = Wegwerf::neu("tlskind");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let args = argv(
        &w.pfad("m.sock"),
        &format!("https://localhost:{port}/v1"),
        &schluessel,
        &w.protokoll(),
    );
    let aufloeser = move |_: &str, p: u16| -> std::io::Result<Vec<SocketAddr>> {
        Ok(vec![SocketAddr::from(([127, 0, 0, 1], p))])
    };
    let b = vorbereiten(
        &args,
        rustix::process::getuid().as_raw(),
        &modellweg::ziel::geteilt(aufloeser),
        Fristen {
            verbinden: Duration::from_secs(5),
            kopf: Duration::from_secs(5),
        },
        Box::new(|| gemeinsam::ZEIT),
    )
    .expect("ready");
    let ant = Antwort::lies(&anfrage(&b.dienst, &post(&koerper("x"), "")));
    std::io::stdout()
        .write_all(format!("status={}\n", ant.status).as_bytes())
        .expect("stdout");
}

fn lauf(w: &Wegwerf, ziel: &str, env: &[(&str, &OsString)]) -> Prozess {
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let a = argv(&w.pfad("m.sock"), ziel, &schluessel, &w.protokoll());
    let (p, erste) = Prozess::start(&a, env);
    assert_eq!(erste, "{\"bereit\":true}");
    p
}

#[test]
fn m8_tls_trusts_the_default_path_of_openssl_only() {
    let w = Wegwerf::neu("tls");
    let (adresse, ca) = tls_attrappe(&w, "localhost");
    let ziel = format!("https://localhost:{}/v1", adresse.port());
    assert_eq!(im_kind(&ca, adresse.port()), 200);
    let ca = OsString::from(ca);

    // Stage 0: the program itself never starts with the variable (section 3, F1).
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let a = argv(&w.pfad("m.sock"), &ziel, &schluessel, &w.protokoll());
    let aus = std::process::Command::new(gemeinsam::programm())
        .args(&a)
        .env_clear()
        .env("SSL_CERT_FILE", &ca)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("start the program");
    assert_eq!(aus.status.code(), Some(78));
    assert_eq!(aus.stdout, b"");
    assert_eq!(aus.stderr, b"mw.umgebung_unsicher: Umgebung unsicher\n");
    assert!(!w.pfad("m.sock").exists());

    let mut p = lauf(&w, &ziel, &[]);
    let ant = Antwort::lies(&ueber_socket(&w.pfad("m.sock"), &post(&koerper("x"), "")));
    assert_eq!(ant.status, 502);
    assert_eq!(p.beende(), Some(0));
    // One line: the refused start of stage 0 wrote none; the trusting run of the child wrote into its own folder.
    let z = zeilen(&w.protokoll());
    assert_eq!(z.len(), 1);
    assert_eq!(z[0]["grund"], "mw.ziel_nicht_erreichbar");
}

#[test]
fn m8_the_host_name_is_checked() {
    let w = Wegwerf::neu("tlsname");
    let (adresse, ca) = tls_attrappe(&w, "anders.beispiel.invalid");
    assert_eq!(im_kind(&ca, adresse.port()), 502);
}

#[test]
fn a_silent_handshake_ends_within_the_deadline() {
    let stumm = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = stumm.local_addr().expect("addr").port();
    let w = Wegwerf::neu("tlsstumm");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let args = argv(
        &w.pfad("m.sock"),
        &format!("https://localhost:{port}/v1"),
        &schluessel,
        &w.protokoll(),
    );
    let aufloeser = move |_: &str, p: u16| -> std::io::Result<Vec<SocketAddr>> {
        assert_eq!(p, port);
        Ok(vec![SocketAddr::from(([127, 0, 0, 1], p))])
    };
    let fristen = Fristen {
        verbinden: Duration::from_millis(500),
        kopf: Duration::from_secs(600),
    };
    let b = vorbereiten(
        &args,
        rustix::process::getuid().as_raw(),
        &modellweg::ziel::geteilt(aufloeser),
        fristen,
        Box::new(|| gemeinsam::ZEIT),
    )
    .expect("ready");
    let start = Instant::now();
    let ant = Antwort::lies(&anfrage(&b.dienst, &post(&koerper("x"), "")));
    assert_eq!(ant.status, 502);
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "{:?}",
        start.elapsed()
    );
    drop(stumm);
}
