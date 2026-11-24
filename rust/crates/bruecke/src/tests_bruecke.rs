//! Tests of the running bridge in the process of the test: real sockets on 127.0.0.1, a stand-in for the service.

use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::unix::fs::MetadataExt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::dienst::rahmen;
use crate::pruefstand::{FakeDienst, Gelesen, Ordner, Stand, anfrage, kopf_zerlegen};
use crate::server;
use crate::verbindung::Protokoll;

fn kurz(ms: u64) -> impl FnOnce(&mut crate::startdatei::Config) {
    move |c| c.lese_frist = Duration::from_millis(ms)
}

fn ohne_cors(g: &Gelesen) {
    assert!(
        g.koepfe
            .iter()
            .all(|(n, _)| !n.to_ascii_lowercase().starts_with("access-control-")),
        "{g:?}"
    );
    assert!(g.kopf("set-cookie").is_none());
}

fn leer(g: &Gelesen, status: u16) {
    assert_eq!(g.status, status, "{g:?}");
    assert!(g.koerper.is_empty(), "{g:?}");
    assert_eq!(g.kopf("content-length"), Some("0"));
    ohne_cors(g);
}

fn nichts_weitergereicht(d: &FakeDienst) {
    assert_eq!(d.verbindungen(), 0);
}

/// The stand-in that does nothing (no connection is expected).
fn stumm(p: &std::path::Path) -> FakeDienst {
    FakeDienst::viele(p, |_, _| {})
}

// ---------------------------------------------------------------------------------------------- start and end

#[test]
fn start_writes_the_files_and_binds_only_127_0_0_1() {
    let o = Ordner::neu();
    let t = o.text("t");
    let p = o.text("p");
    let st = Stand::neu(o, |c| c.port_datei = Some(p.clone()));
    let meta = fs::metadata(&t).expect("token file");
    assert_eq!(meta.mode() & 0o7777, 0o600);
    assert_eq!(meta.len(), 65);
    let inhalt = fs::read_to_string(&t).expect("read");
    assert!(
        inhalt.ends_with('\n')
            && inhalt.len() == 65
            && inhalt[..64]
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    );
    assert_eq!(st.token, &inhalt[..64]);
    assert_eq!(fs::metadata(&p).expect("port file").mode() & 0o7777, 0o600);
    assert_eq!(
        fs::read_to_string(&p).expect("read"),
        format!("{}\n", st.port)
    );
    assert_ne!(st.port, 0);
    assert!(TcpStream::connect(("127.0.0.1", st.port)).is_ok());
}

#[test]
fn the_address_is_fixed() {
    let l = server::binden(0).expect("bind");
    let a = l.local_addr().expect("addr");
    assert!(
        a.is_ipv4() && a.ip().is_loopback() && a.ip().to_string() == "127.0.0.1",
        "{a}"
    );
    // a second bind on the busy port fails with exit code 1
    let e = server::binden(a.port()).expect_err("busy");
    assert_eq!(e.code, 1);
}

#[test]
fn a_busy_port_leaves_no_file() {
    let l = server::binden(0).expect("bind");
    let busy = l.local_addr().expect("addr").port();
    let o = Ordner::neu();
    let cfg = crate::startdatei::Config {
        dienst: o.text("d"),
        port: busy,
        token_datei: o.text("t"),
        port_datei: Some(o.text("p")),
        deflate: true,
        lese_frist: Duration::from_secs(10),
        verbindungen_max: 64,
    };
    let sink: Protokoll = Arc::new(|_| {});
    let e = server::starten(cfg, sink).err().expect("must fail");
    assert_eq!(e.code, 1);
    assert!(!o.pfad("t").exists() && !o.pfad("p").exists());
}

#[test]
fn a_bad_folder_stops_before_any_file_is_made() {
    let o = Ordner::neu();
    let p = o.text("sub/t");
    let cfg = crate::startdatei::Config {
        dienst: o.text("d"),
        port: 0,
        token_datei: o.text("t"),
        port_datei: Some(p),
        deflate: true,
        lese_frist: Duration::from_secs(10),
        verbindungen_max: 64,
    };
    let sink: Protokoll = Arc::new(|_| {});
    let e = server::starten(cfg, sink).err().expect("must fail");
    assert_eq!(e.code, 3);
    assert!(!o.pfad("t").exists());
}

#[test]
fn an_existing_token_file_stays_as_it_is() {
    let o = Ordner::neu();
    fs::write(o.pfad("t"), b"alt").expect("write");
    let cfg = crate::startdatei::Config {
        dienst: o.text("d"),
        port: 0,
        token_datei: o.text("t"),
        port_datei: None,
        deflate: true,
        lese_frist: Duration::from_secs(10),
        verbindungen_max: 64,
    };
    let e = server::starten(cfg, Arc::new(|_| {}))
        .err()
        .expect("must fail");
    assert_eq!(e.code, 3);
    assert_eq!(fs::read(o.pfad("t")).expect("read"), b"alt");
}

#[test]
fn when_the_port_file_fails_the_token_file_made_here_goes_again() {
    let o = Ordner::neu();
    let sperre = o.pfad("zu");
    fs::create_dir(&sperre).expect("mkdir");
    fs::set_permissions(&sperre, std::os::unix::fs::PermissionsExt::from_mode(0o500))
        .expect("chmod");
    let cfg = crate::startdatei::Config {
        dienst: o.text("d"),
        port: 0,
        token_datei: o.text("t"),
        port_datei: Some(o.text("zu/p")),
        deflate: true,
        lese_frist: Duration::from_secs(10),
        verbindungen_max: 64,
    };
    let e = server::dateien_anlegen(&cfg, b"tok", 1).expect_err("port file cannot be made");
    assert_eq!(e.code, 3);
    assert!(!o.pfad("t").exists());
    // without a port file the token file stays
    let cfg2 = crate::startdatei::Config {
        port_datei: None,
        ..cfg
    };
    server::dateien_anlegen(&cfg2, b"tok", 1).expect("ok");
    assert_eq!(fs::read(o.pfad("t")).expect("read"), b"tok\n");
    fs::set_permissions(&sperre, std::os::unix::fs::PermissionsExt::from_mode(0o700))
        .expect("chmod");
}

#[test]
fn the_end_removes_both_files_and_closes_connections() {
    let o = Ordner::neu();
    let (t, p) = (o.text("t"), o.text("p"));
    let st = Stand::neu(o, |c| c.port_datei = Some(p.clone()));
    let mut offen = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    offen
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    thread::sleep(Duration::from_millis(100));
    let port = st.port;
    drop(st);
    assert!(!std::path::Path::new(&t).exists() && !std::path::Path::new(&p).exists());
    let mut b = [0u8; 8];
    assert_eq!(
        offen.read(&mut b).unwrap_or(0),
        0,
        "the held connection is closed"
    );
    assert!(
        TcpStream::connect(("127.0.0.1", port)).is_err(),
        "the listener is closed"
    );
}

// ---------------------------------------------------------------------------------------------- pages

#[test]
fn the_pages_are_public_with_the_policy_and_the_fixed_heads() {
    let st = Stand::ohne_dienst(|_| {});
    for (ziel, typ, inhalt) in [
        ("/", "text/html; charset=utf-8", crate::seite::HTML),
        (
            "/ansicht.js",
            "text/javascript; charset=utf-8",
            crate::seite::SKRIPT,
        ),
        ("/ansicht_bg.wasm", "application/wasm", crate::seite::WASM),
    ] {
        let roh = format!("GET {ziel} HTTP/1.1\r\nHost: {}\r\n\r\n", st.host());
        let g = st.senden(roh.as_bytes());
        assert_eq!(g.status, 200);
        assert_eq!(g.kopf("content-type"), Some(typ));
        assert_eq!(g.kopf("content-security-policy"), Some(crate::seite::CSP));
        assert_eq!(g.kopf("cache-control"), Some("no-store"));
        assert_eq!(g.kopf("x-content-type-options"), Some("nosniff"));
        assert_eq!(g.kopf("connection"), Some("close"));
        assert_eq!(
            g.kopf("content-length"),
            Some(inhalt.len().to_string().as_str())
        );
        assert_eq!(g.koerper, inhalt);
        ohne_cors(&g);
    }
    assert!(st.zeilen().is_empty(), "successful requests write nothing");
    let g = st.senden(
        format!(
            "GET / HTTP/1.1\r\nHost: localhost:{}\r\nOrigin: {}\r\n\r\n",
            st.port,
            st.origin()
        )
        .as_bytes(),
    );
    assert_eq!(g.status, 200);
}

#[test]
fn rejections_are_empty_and_logged_with_one_word() {
    let st = Stand::ohne_dienst(|_| {});
    let h = st.host();
    let o = st.origin();
    let faelle: Vec<(String, u16, &str)> = vec![
        (
            format!("GET / HTTP/1.0\r\nHost: {h}\r\n\r\n"),
            400,
            "anfrage_form",
        ),
        (format!("GET / HTTP/1.1\r\n\r\n"), 400, "anfrage_form"),
        (format!("GET / HTTP/1.1\r\nHost: evil\r\n\r\n"), 403, "host"),
        (
            format!("GET / HTTP/1.1\r\nHost: {h}\r\nOrigin: evil\r\n\r\n"),
            403,
            "origin",
        ),
        (
            format!("GET /strom HTTP/1.1\r\nHost: {h}\r\n\r\n"),
            401,
            "token",
        ),
        (
            format!(
                "GET /x HTTP/1.1\r\nHost: {h}\r\nAuthorization: Bearer {}\r\n\r\n",
                st.token
            ),
            404,
            "route",
        ),
        (
            format!(
                "PUT /eingabe HTTP/1.1\r\nHost: {h}\r\nOrigin: {o}\r\nAuthorization: Bearer {}\r\n\r\n",
                st.token
            ),
            405,
            "methode",
        ),
        (
            format!("GET /{} HTTP/1.1\r\nHost: {h}\r\n\r\n", "a".repeat(2048)),
            414,
            "ziel_zu_lang",
        ),
    ];
    for (roh, status, _) in &faelle {
        let g = st.senden(roh.as_bytes());
        leer(&g, *status);
        assert_eq!(g.kopf("cache-control"), Some("no-store"));
        assert_eq!(g.kopf("x-content-type-options"), Some("nosniff"));
        assert_eq!(g.kopf("connection"), Some("close"));
    }
    let soll: Vec<String> = faelle.iter().map(|(_, _, g)| (*g).to_string()).collect();
    assert_eq!(st.zeilen(), soll);
    let alles = st.zeilen().join("|");
    assert!(!alles.contains(&st.token) && !alles.contains("evil") && !alles.contains("/x"));
}

#[test]
fn the_protocol_line_has_only_the_word() {
    assert_eq!(
        server::protokollzeile("host"),
        "wb-bruecke: abgelehnt host\n"
    );
}

// ---------------------------------------------------------------------------------------------- limits

fn kopf_mit_laenge(st: &Stand, gesamt: usize) -> Vec<u8> {
    let anfang = format!("GET / HTTP/1.1\r\nHost: {}\r\nX: ", st.host());
    let fuell = gesamt - anfang.len() - 4;
    let mut v = anfang.into_bytes();
    v.extend(std::iter::repeat_n(b'a', fuell));
    v.extend_from_slice(b"\r\n\r\n");
    assert_eq!(v.len(), gesamt);
    v
}

#[test]
fn the_head_limit_is_8192_bytes_with_the_closing_line() {
    let st = Stand::ohne_dienst(kurz(300));
    assert_eq!(st.senden(&kopf_mit_laenge(&st, 8192)).status, 200);
    let g = st.senden(&kopf_mit_laenge(&st, 8193));
    leer(&g, 431);
    assert_eq!(st.zeilen(), vec!["kopf_zu_gross"]);
}

#[test]
fn a_head_without_end_is_431_above_8192_and_408_at_8192() {
    let st = Stand::ohne_dienst(kurz(300));
    let mut v = kopf_mit_laenge(&st, 8192);
    v.truncate(8192 - 4);
    v.extend_from_slice(b"aaaa");
    let t = Instant::now();
    let g = st.senden(&v);
    leer(&g, 408);
    assert!(t.elapsed() >= Duration::from_millis(250));
    v.push(b'a');
    let g = st.senden(&v);
    leer(&g, 431);
    assert_eq!(st.zeilen(), vec!["zeit", "kopf_zu_gross"]);
}

#[test]
fn a_head_that_never_ends_is_408_within_the_time() {
    /// The most bytes the drip of the second part sends, about one every 100 ms.
    const TROPFEN: usize = 30;
    let st = Stand::ohne_dienst(kurz(300));
    let mut s = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    s.write_all(b"GET / HTTP/1.1\r\nHo").expect("write");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let t = Instant::now();
    let mut alles = Vec::new();
    let _ = s.read_to_end(&mut alles);
    let dauer = t.elapsed();
    let (g, _) = kopf_zerlegen(&alles).expect("an answer");
    assert_eq!(g.status, 408);
    assert!(
        dauer >= Duration::from_millis(250) && dauer < Duration::from_millis(1500),
        "{dauer:?}"
    );
    // the time counts from the start, not from the last byte: a drip does not keep it open. The client drips one byte about
    // every 100 ms for up to 30 bytes (3 s) and stops when the answer comes; a read with a short timeout paces the drip, so the
    // answer is seen at once. The 408 must come while the drip still goes on and within the bound of the first part; a time
    // that restarted with each byte would answer only 300 ms after the last byte, after 3 s. The bound does not depend on how
    // long the sleeps of a slow machine take (macos-latest, 23.11.2026: eight fixed sleeps of 100 ms passed 1200 ms).
    let mut s = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    s.set_read_timeout(Some(Duration::from_millis(100)))
        .expect("timeout");
    let t = Instant::now();
    let mut alles = Vec::new();
    let mut stueck = [0u8; 1024];
    let mut getropft = 0;
    while getropft < TROPFEN {
        if s.write_all(b"x").is_err() {
            break;
        }
        getropft += 1;
        match s.read(&mut stueck) {
            Ok(0) => break,
            Ok(n) => {
                alles.extend_from_slice(&stueck[..n]);
                break;
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let _ = s.read_to_end(&mut alles);
    let dauer = t.elapsed();
    assert!(
        getropft < TROPFEN,
        "the answer came only after the drip: {dauer:?}"
    );
    assert!(
        dauer < Duration::from_millis(1500),
        "{dauer:?} after {getropft} bytes"
    );
    assert_eq!(kopf_zerlegen(&alles).expect("an answer").0.status, 408);
}

#[test]
fn a_connection_that_ends_early_gets_no_answer_and_no_line() {
    let st = Stand::ohne_dienst(|_| {});
    let mut s = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    s.write_all(b"GET / HTTP/1.1\r\nHo").expect("write");
    s.shutdown(Shutdown::Write).expect("shutdown");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut alles = Vec::new();
    let _ = s.read_to_end(&mut alles);
    assert!(alles.is_empty());
    assert_eq!(
        st.senden(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", st.host()).as_bytes())
            .status,
        200
    );
    assert!(st.zeilen().is_empty());
}

fn post_text(st: &Stand, ziel: &str, laenge: usize, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut kopf = format!(
        "POST {ziel} HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {laenge}\r\n",
        st.host(),
        st.origin(),
        st.token
    );
    for (n, w) in extra {
        kopf.push_str(&format!("{n}: {w}\r\n"));
    }
    kopf.push_str("\r\n");
    kopf.into_bytes()
}

fn json_koerper(n: usize) -> Vec<u8> {
    // a JSON value of exactly n bytes: spaces, then `{}`
    let mut v = vec![b' '; n - 2];
    v.extend_from_slice(b"{}");
    v
}

#[test]
fn the_start_body_limit_is_16384_and_a_larger_one_is_not_read() {
    let (st, d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                let mut buf = vec![0u8; 65_536];
                let _ = s.read(&mut buf);
                let _ = s.write_all(&rahmen(7, br#"{"grund":"g","code":"c","text":"t"}"#));
            })
        },
        |_| {},
    );
    let mut ok = post_text(&st, "/starten", 16_384, &[]);
    ok.extend(json_koerper(16_384));
    assert_eq!(st.senden(&ok).status, 422);
    // 16385: refused with 413 before the body is read: send only the head
    let g = st.senden(&post_text(&st, "/starten", 16_385, &[]));
    leer(&g, 413);
    assert_eq!(d.verbindungen(), 1);
    assert_eq!(st.zeilen(), vec!["koerper_zu_gross"]);
}

#[test]
fn the_input_body_limit_is_1048575() {
    let (st, d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                let mut buf = vec![0u8; 65_536];
                while s.read(&mut buf).unwrap_or(0) > 0 {}
            })
        },
        |_| {},
    );
    let kennung = "0".repeat(32);
    let kopf = |n: usize| post_text(&st, "/eingabe", n, &[("X-Verbindung", &kennung)]);
    let g = st.senden(&kopf(1_048_576));
    leer(&g, 413);
    // within the limit the body is read, the identifier is unknown: 404 comes after the body was read
    let mut ok = kopf(1_048_575);
    ok.extend(json_koerper(1_048_575));
    leer(&st.senden(&ok), 404);
    assert_eq!(d.verbindungen(), 0);
}

#[test]
fn a_body_that_does_not_come_is_408() {
    let (st, d) = Stand::mit_dienst(stumm, kurz(300));
    let mut s = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    let mut t = post_text(&st, "/starten", 50, &[]);
    t.extend_from_slice(b"{\"a\":");
    s.write_all(&t).expect("write");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let zeit = Instant::now();
    let mut alles = Vec::new();
    let _ = s.read_to_end(&mut alles);
    let (g, _) = kopf_zerlegen(&alles).expect("an answer");
    assert_eq!(g.status, 408);
    assert!(
        zeit.elapsed() >= Duration::from_millis(250)
            && zeit.elapsed() < Duration::from_millis(1500)
    );
    assert_eq!(st.zeilen(), vec!["zeit"]);
    nichts_weitergereicht(&d);
}

#[test]
fn a_body_given_in_pieces_is_read_whole() {
    let (st, _d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                let mut buf = vec![0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(&rahmen(6, br#"{"art":"Gestartet","sitzung":"s-1"}"#));
            })
        },
        |_| {},
    );
    let mut s = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    let koerper = br#"{"modus": "x", "n": 1}"#;
    s.write_all(&post_text(&st, "/starten", koerper.len(), &[]))
        .expect("write");
    for stueck in koerper.chunks(5) {
        thread::sleep(Duration::from_millis(30));
        s.write_all(stueck).expect("write");
    }
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut alles = Vec::new();
    let _ = s.read_to_end(&mut alles);
    let (g, n) = kopf_zerlegen(&alles).expect("an answer");
    assert_eq!(g.status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&alles[n..]).expect("json"),
        json!({"sitzung": "s-1"})
    );
}

#[test]
fn the_body_is_read_to_its_length_and_not_a_byte_more() {
    let seen: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let s2 = Arc::clone(&seen);
    let (st, _d) = Stand::mit_dienst(
        move |p| {
            FakeDienst::viele(p, move |_, s| {
                let mut buf = vec![0u8; 4096];
                let k = s.read(&mut buf).unwrap_or(0);
                *s2.lock().expect("lock") = buf[..k].to_vec();
                let _ = s.write_all(&rahmen(6, br#"{"art":"Gestartet","sitzung":"s-1"}"#));
            })
        },
        |_| {},
    );
    let koerper = br#"{"a":1}"#;
    let mut s = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    let mut kopf = post_text(&st, "/starten", koerper.len(), &[]);
    kopf.extend_from_slice(&koerper[..3]);
    s.write_all(&kopf).expect("write");
    thread::sleep(Duration::from_millis(150));
    let mut rest = koerper[3..].to_vec();
    rest.extend_from_slice(b" GARBAGE AFTER THE BODY");
    s.write_all(&rest).expect("write");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut alles = Vec::new();
    let _ = s.read_to_end(&mut alles);
    assert_eq!(kopf_zerlegen(&alles).expect("an answer").0.status, 200);
    let roh = seen.lock().expect("lock").clone();
    assert!(String::from_utf8_lossy(&roh[5..]).contains(r#""start":{"a":1}}"#));
}

#[test]
fn bytes_after_the_body_are_ignored() {
    let (st, _d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                let mut buf = vec![0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(&rahmen(6, br#"{"art":"Gestartet","sitzung":"s-1"}"#));
            })
        },
        |_| {},
    );
    let mut t = post_text(&st, "/starten", 2, &[]);
    t.extend_from_slice(b"{}GARBAGE");
    assert_eq!(st.senden(&t).status, 200);
}

#[test]
fn at_most_verbindungen_max_connections_at_once() {
    let st = Stand::ohne_dienst(|c| c.verbindungen_max = 3);
    let mut offen: Vec<TcpStream> = (0..3)
        .map(|_| TcpStream::connect(("127.0.0.1", st.port)).expect("connect"))
        .collect();
    thread::sleep(Duration::from_millis(150));
    let g = st.senden(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", st.host()).as_bytes());
    leer(&g, 503);
    assert_eq!(st.zeilen(), vec!["verbindungen"]);
    // free one: it goes again
    offen.pop();
    thread::sleep(Duration::from_millis(250));
    assert_eq!(
        st.senden(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", st.host()).as_bytes())
            .status,
        200
    );
    // with 2 of 3 held, a third still passes, the fourth does not
    let dritte = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    thread::sleep(Duration::from_millis(150));
    leer(
        &st.senden(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", st.host()).as_bytes()),
        503,
    );
    drop((dritte, offen));
}

#[test]
fn a_broken_request_ends_only_its_connection() {
    let st = Stand::ohne_dienst(|_| {});
    let mut gehalten = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    gehalten
        .write_all(b"GET / HTTP/1.1\r\nHost: ")
        .expect("write");
    for roh in [
        &b"\x00\xff\xfe\r\n\r\n"[..],
        b"GET\r\n\r\n",
        b"\r\n\r\n",
        &[0u8; 100],
        &vec![b'a'; 10_000],
    ] {
        let _ = anfrage(st.port, roh, Duration::from_millis(800));
    }
    gehalten
        .write_all(format!("{}\r\n\r\n", st.host()).as_bytes())
        .expect("write");
    gehalten
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut alles = Vec::new();
    let _ = gehalten.read_to_end(&mut alles);
    assert_eq!(kopf_zerlegen(&alles).expect("an answer").0.status, 200);
}

// ---------------------------------------------------------------------------------------------- token and the service

#[test]
fn without_a_valid_token_the_service_sees_nothing() {
    let (st, d) = Stand::mit_dienst(stumm, |_| {});
    let h = st.host();
    let o = st.origin();
    let letzter = if st.token.ends_with('0') { '1' } else { '0' };
    let falsch = format!("{}{letzter}", &st.token[..63]);
    for ziel in ["/strom", "/starten", "/eingabe"] {
        let methode = if ziel == "/strom" { "GET" } else { "POST" };
        for auth in [
            String::new(),
            format!("Authorization: Bearer {falsch}\r\n"),
            format!("Authorization: {}\r\n", st.token),
        ] {
            let koerper = if methode == "POST" {
                "Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
            } else {
                "\r\n"
            };
            let roh =
                format!("{methode} {ziel} HTTP/1.1\r\nHost: {h}\r\nOrigin: {o}\r\n{auth}{koerper}");
            let g = st.senden(roh.as_bytes());
            leer(&g, 401);
            assert!(g.kopf("www-authenticate").is_none());
        }
    }
    nichts_weitergereicht(&d);
    assert!(st.zeilen().iter().all(|z| z == "token"));
    assert_eq!(st.zeilen().len(), 9);
}

#[test]
fn start_answers_200_422_or_400_and_checks_the_post_in_order() {
    let (st, d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |n, s| {
                let mut buf = vec![0u8; 4096];
                let k = s.read(&mut buf).unwrap_or(0);
                // the event is not parsed: the deep test structure is beyond the parser's own depth limit
                let text = String::from_utf8_lossy(&buf[5..k]).into_owned();
                assert!(text.starts_with(r#"{"art":"Starten","start":"#), "{text}");
                let antwort = if text.contains(r#""nein":true"#) {
                    rahmen(7, br#"{"grund":"nein","code":"start.nein","text":"Nein."}"#)
                } else {
                    rahmen(
                        6,
                        format!(r#"{{"art":"Gestartet","sitzung":"s-{n}"}}"#).as_bytes(),
                    )
                };
                let _ = s.write_all(&antwort);
            })
        },
        |_| {},
    );
    let mit = |ziel: &str, extra: &[(&str, &str)], koerper: &[u8]| {
        let mut t = post_text(&st, ziel, koerper.len(), extra);
        t.extend_from_slice(koerper);
        st.senden(&t)
    };
    let g = mit("/starten", &[], br#"{"ja": 1}"#);
    assert_eq!(
        (g.status, g.kopf("content-type")),
        (200, Some("application/json"))
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&g.koerper).expect("json"),
        json!({"sitzung": "s-0"})
    );
    assert_eq!(
        g.kopf("content-length"),
        Some(g.koerper.len().to_string().as_str())
    );
    ohne_cors(&g);
    let g = mit("/starten", &[], br#"{"nein": true}"#);
    assert_eq!(
        (g.status, g.kopf("content-type")),
        (422, Some("application/json"))
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&g.koerper).expect("json"),
        json!({"grund": "nein", "code": "start.nein", "text": "Nein."})
    );
    assert_eq!(
        st.zeilen().len(),
        0,
        "422 is an answer of the service, no rejection of the bridge"
    );
    // not JSON: 400 without a connection to the service
    let vorher = d.verbindungen();
    for k in [&b""[..], b"nope", b"{", b"NaN", b"\xff", b"[1,", b"{} x"] {
        leer(&mit("/starten", &[], k), 400);
    }
    assert_eq!(d.verbindungen(), vorher);
    // any JSON value is sent on, also a list or a number
    assert_eq!(mit("/starten", &[], b"[1, 2]").status, 200);
    assert_eq!(mit("/starten", &[], b"7").status, 200);
    // depth: the depth limit of serde_json (128 counts the level that fails) is the limit; 127 levels pass, 128 are 400
    let tief = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
    assert_eq!(mit("/starten", &[], tief(127).as_bytes()).status, 200);
    leer(&mit("/starten", &[], tief(128).as_bytes()), 400);
    // content type, length
    let mut t = post_text(&st, "/starten", 2, &[]);
    t.extend_from_slice(b"{}");
    let falsch_typ = String::from_utf8(t.clone())
        .expect("utf8")
        .replace("application/json", "text/plain");
    leer(&st.senden(falsch_typ.as_bytes()), 415);
    let ohne_laenge = String::from_utf8(t)
        .expect("utf8")
        .replace("Content-Length: 2\r\n", "");
    leer(&st.senden(ohne_laenge.as_bytes()), 411);
    let z = st.zeilen();
    assert_eq!(&z[z.len() - 2..], ["inhalt_typ", "koerper_laenge"]);
}

#[test]
fn the_start_structure_is_written_anew_not_pasted_as_text() {
    let seen: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let s2 = Arc::clone(&seen);
    let (st, _d) = Stand::mit_dienst(
        move |p| {
            FakeDienst::viele(p, move |_, s| {
                let mut buf = vec![0u8; 4096];
                let k = s.read(&mut buf).unwrap_or(0);
                *s2.lock().expect("lock") = buf[..k].to_vec();
                let _ = s.write_all(&rahmen(6, br#"{"art":"Gestartet","sitzung":"s-1"}"#));
            })
        },
        |_| {},
    );
    let koerper = b"  {\"b\" : 1,\n \"a\":  [true , null]}  ";
    let mut t = post_text(&st, "/starten", koerper.len(), &[]);
    t.extend_from_slice(koerper);
    assert_eq!(st.senden(&t).status, 200);
    let roh = seen.lock().expect("lock").clone();
    let n = u32::from_be_bytes(roh[..4].try_into().expect("4")) as usize;
    assert_eq!((roh[4], roh.len()), (1, 4 + n));
    let nutz = std::str::from_utf8(&roh[5..]).expect("utf8");
    assert!(
        !nutz.contains(' ') && !nutz.contains('\n'),
        "compact: {nutz}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(nutz).expect("json"),
        json!({"art": "Starten", "start": {"a": [true, null], "b": 1}})
    );
}

#[test]
fn start_without_a_service_is_502_and_the_page_still_works() {
    let st = Stand::ohne_dienst(|_| {});
    for (methode, ziel, koerper) in [
        ("POST", "/starten", Some(&b"{}"[..])),
        ("GET", "/strom", None),
    ] {
        let g = st.senden(&st.text(methode, ziel, &[], koerper));
        leer(&g, 502);
    }
    assert_eq!(st.zeilen(), vec!["dienst", "dienst"]);
    assert_eq!(
        st.senden(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", st.host()).as_bytes())
            .status,
        200
    );
}

#[test]
fn start_with_a_service_that_ends_or_errs_is_502() {
    let (st, _d) = Stand::mit_dienst(|p| FakeDienst::viele(p, |_, _| {}), |_| {});
    leer(
        &st.senden(&st.text("POST", "/starten", &[], Some(b"{}"))),
        502,
    );
}

// ---------------------------------------------------------------------------------------------- stream and input

/// A stream of a test: the socket and the bytes read behind the head.
struct StromTest {
    s: TcpStream,
    kopf: Gelesen,
    rest: Vec<u8>,
}

fn strom_oeffnen(st: &Stand) -> StromTest {
    let mut s = TcpStream::connect(("127.0.0.1", st.port)).expect("connect");
    s.write_all(&st.text("GET", "/strom", &[], None))
        .expect("write");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut alles = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = s.read(&mut buf).expect("read the head");
        assert!(n > 0, "stream ended before the head");
        alles.extend_from_slice(&buf[..n]);
        if let Some((g, n)) = kopf_zerlegen(&alles) {
            return StromTest {
                s,
                kopf: g,
                rest: alles[n..].to_vec(),
            };
        }
    }
}

impl StromTest {
    fn lesen(&mut self, n: usize) -> Vec<u8> {
        let mut buf = [0u8; 4096];
        while self.rest.len() < n {
            let k = self.s.read(&mut buf).expect("read");
            assert!(k > 0, "stream ended");
            self.rest.extend_from_slice(&buf[..k]);
        }
        self.rest.drain(..n).collect()
    }

    fn kennung(&self) -> String {
        self.kopf
            .kopf("x-verbindung")
            .expect("X-Verbindung")
            .to_string()
    }

    fn ende(&mut self) -> bool {
        let mut buf = [0u8; 16];
        matches!(self.s.read(&mut buf), Ok(0) | Err(_))
    }
}

type Gesehen = Arc<Mutex<Vec<(usize, Vec<u8>)>>>;

/// A service that sends a frame to every stream at once and records what each connection reads.
fn aufzeichner(
    p: &std::path::Path,
    gesehen: &Gesehen,
    enden: &Arc<Mutex<Vec<usize>>>,
) -> FakeDienst {
    let (g, e) = (Arc::clone(gesehen), Arc::clone(enden));
    FakeDienst::viele(p, move |n, s| {
        let _ = s.write_all(&rahmen(6, format!("hallo {n}").as_bytes()));
        let mut buf = vec![0u8; 65_536];
        loop {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(k) => g.lock().expect("lock").push((n, buf[..k].to_vec())),
            }
        }
        e.lock().expect("lock").push(n);
    })
}

fn warte_bis(f: impl Fn() -> bool) -> bool {
    let ende = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ende {
        if f() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn a_stream_has_its_heads_hands_bytes_on_unchanged_and_ends_with_the_browser() {
    let gesehen: Gesehen = Arc::new(Mutex::new(Vec::new()));
    let enden = Arc::new(Mutex::new(Vec::new()));
    let (g2, e2) = (Arc::clone(&gesehen), Arc::clone(&enden));
    let (st, d) = Stand::mit_dienst(move |p| aufzeichner(p, &g2, &e2), |c| c.deflate = true);
    let mut a = strom_oeffnen(&st);
    assert_eq!(a.kopf.status, 200);
    assert_eq!(
        a.kopf.kopf("content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(a.kopf.kopf("cache-control"), Some("no-store"));
    assert_eq!(a.kopf.kopf("connection"), Some("close"));
    assert_eq!(a.kopf.kopf("x-content-type-options"), Some("nosniff"));
    assert_eq!(a.kopf.kopf("x-deflate"), Some("1"));
    assert!(a.kopf.kopf("content-length").is_none());
    ohne_cors(&a.kopf);
    let k = a.kennung();
    assert!(k.len() == 32 && k.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    assert_eq!(a.lesen(5 + 7), rahmen(6, b"hallo 0"));
    assert_eq!(d.verbindungen(), 1);
    // the browser closes: the service sees the end of its connection
    drop(a);
    assert!(
        warte_bis(|| enden.lock().expect("lock").contains(&0)),
        "the service saw no end"
    );
}

#[test]
fn x_deflate_is_zero_without_deflate_and_the_identifiers_differ() {
    let (st, _d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                let _ = s.write_all(&rahmen(6, b"x"));
                let mut b = [0u8; 8];
                let _ = s.read(&mut b);
            })
        },
        |_| {},
    );
    let a = strom_oeffnen(&st);
    let b = strom_oeffnen(&st);
    assert_eq!(a.kopf.kopf("x-deflate"), Some("0"));
    assert_ne!(a.kennung(), b.kennung());
}

#[test]
fn an_input_reaches_only_the_stream_of_its_identifier() {
    let gesehen: Gesehen = Arc::new(Mutex::new(Vec::new()));
    let enden = Arc::new(Mutex::new(Vec::new()));
    let (g2, e2) = (Arc::clone(&gesehen), Arc::clone(&enden));
    let (st, _d) = Stand::mit_dienst(move |p| aufzeichner(p, &g2, &e2), |_| {});
    let mut a = strom_oeffnen(&st);
    let mut b = strom_oeffnen(&st);
    a.lesen(5 + 7);
    b.lesen(5 + 7);
    let (ka, kb) = (a.kennung(), b.kennung());
    let eingabe = br#"{"art":"Text","text":"abc"}"#;
    let g = st.senden(&st.text("POST", "/eingabe", &[("X-Verbindung", &kb)], Some(eingabe)));
    assert_eq!((g.status, g.kopf("content-length")), (204, Some("0")));
    assert!(g.koerper.is_empty());
    assert!(warte_bis(|| !gesehen.lock().expect("lock").is_empty()));
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        *gesehen.lock().expect("lock"),
        vec![(1, rahmen(1, eingabe))]
    );
    let g = st.senden(&st.text("POST", "/eingabe", &[("X-Verbindung", &ka)], Some(b"{}")));
    assert_eq!(g.status, 204);
    assert!(warte_bis(|| gesehen.lock().expect("lock").len() == 2));
    assert_eq!(gesehen.lock().expect("lock")[1], (0, rahmen(1, b"{}")));
    // the body goes on byte for byte, also with spaces and a big size
    let gross = {
        let mut v = br#"{"a":""#.to_vec();
        v.extend(std::iter::repeat_n(b'x', 100_000));
        v.extend_from_slice(b"\" }  ");
        v
    };
    let g = st.senden(&st.text("POST", "/eingabe", &[("X-Verbindung", &ka)], Some(&gross)));
    assert_eq!(g.status, 204);
    let soll = rahmen(1, &gross);
    assert!(warte_bis(|| gesehen
        .lock()
        .expect("lock")
        .iter()
        .skip(2)
        .map(|(n, b)| (*n, b.len()))
        .fold((0, 0), |a, x| (x.0, a.1 + x.1))
        == (0, soll.len())));
    let alle: Vec<u8> = gesehen
        .lock()
        .expect("lock")
        .iter()
        .skip(2)
        .flat_map(|(_, b)| b.clone())
        .collect();
    assert_eq!(alle, soll);
}

#[test]
fn an_input_with_a_bad_identifier_or_body_is_refused() {
    let gesehen: Gesehen = Arc::new(Mutex::new(Vec::new()));
    let enden = Arc::new(Mutex::new(Vec::new()));
    let (g2, e2) = (Arc::clone(&gesehen), Arc::clone(&enden));
    let (st, _d) = Stand::mit_dienst(move |p| aufzeichner(p, &g2, &e2), |_| {});
    let mut a = strom_oeffnen(&st);
    a.lesen(12);
    let k = a.kennung();
    let senden = |extra: &[(&str, &str)], koerper: &[u8]| {
        st.senden(&st.text("POST", "/eingabe", extra, Some(koerper)))
    };
    leer(&senden(&[], b"{}"), 400);
    let grosse = k.to_uppercase();
    for falsch in [
        "",
        "abc",
        &k[..31],
        &format!("{k}0"),
        &grosse,
        &format!("{}g", &k[..31]),
    ] {
        leer(&senden(&[("X-Verbindung", falsch)], b"{}"), 400);
    }
    leer(&senden(&[("X-Verbindung", &"0".repeat(32))], b"{}"), 404);
    leer(&senden(&[("X-Verbindung", &"f".repeat(32))], b"{}"), 404);
    for k2 in [
        &b"[]"[..],
        b"1",
        b"\"x\"",
        b"null",
        b"true",
        b"",
        b"{",
        b"{} {}",
        b"\xff",
        b"NaN",
    ] {
        leer(&senden(&[("X-Verbindung", &k)], k2), 400);
    }
    let mut roh = st.text("POST", "/eingabe", &[("X-Verbindung", &k)], Some(b"{}"));
    let s = String::from_utf8(roh.clone())
        .expect("utf8")
        .replace("application/json", "text/plain");
    leer(&st.senden(s.as_bytes()), 415);
    roh.clear();
    thread::sleep(Duration::from_millis(100));
    assert!(
        gesehen.lock().expect("lock").is_empty(),
        "nothing reached the service"
    );
    let z = st.zeilen();
    assert_eq!(z[0], "kennung");
    assert!(
        z.iter().filter(|w| *w == "kennung").count() >= 9
            && z.iter().filter(|w| *w == "json").count() == 10
    );
}

#[test]
fn the_end_of_the_service_ends_the_stream_and_a_later_input_is_404() {
    let (st, _d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                let _ = s.write_all(&rahmen(6, b"x"));
                thread::sleep(Duration::from_millis(200));
            })
        },
        |_| {},
    );
    let mut a = strom_oeffnen(&st);
    a.lesen(6);
    assert!(a.ende(), "the stream ends with the service");
    thread::sleep(Duration::from_millis(100));
    leer(
        &st.senden(&st.text(
            "POST",
            "/eingabe",
            &[("X-Verbindung", &a.kennung())],
            Some(b"{}"),
        )),
        404,
    );
}

#[test]
fn an_input_to_a_service_that_has_ended_is_502() {
    let (st, _d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                let _ = s.write_all(&rahmen(6, b"x"));
                let _ = s.shutdown(Shutdown::Read);
                thread::sleep(Duration::from_millis(300));
            })
        },
        |_| {},
    );
    let mut a = strom_oeffnen(&st);
    a.lesen(6);
    thread::sleep(Duration::from_millis(100));
    let g = st.senden(&st.text(
        "POST",
        "/eingabe",
        &[("X-Verbindung", &a.kennung())],
        Some(b"{}"),
    ));
    assert!(g.status == 502 || g.status == 204, "{g:?}");
}

#[test]
fn a_stream_is_not_cut_by_the_read_limit() {
    let (st, _d) = Stand::mit_dienst(
        |p| {
            FakeDienst::viele(p, |_, s| {
                thread::sleep(Duration::from_millis(900));
                let _ = s.write_all(&rahmen(6, b"spaet"));
                let mut b = [0u8; 8];
                let _ = s.read(&mut b);
            })
        },
        kurz(300),
    );
    let mut a = strom_oeffnen(&st);
    assert_eq!(a.lesen(10), rahmen(6, b"spaet"));
}

#[test]
fn many_streams_and_inputs_at_once() {
    let gesehen: Gesehen = Arc::new(Mutex::new(Vec::new()));
    let enden = Arc::new(Mutex::new(Vec::new()));
    let (g2, e2) = (Arc::clone(&gesehen), Arc::clone(&enden));
    let (st, d) = Stand::mit_dienst(move |p| aufzeichner(p, &g2, &e2), |_| {});
    let mut strome: Vec<StromTest> = (0..8).map(|_| strom_oeffnen(&st)).collect();
    for (i, s) in strome.iter_mut().enumerate() {
        assert_eq!(s.lesen(5 + 7), rahmen(6, format!("hallo {i}").as_bytes()));
    }
    assert_eq!(d.verbindungen(), 8);
    let kennungen: Vec<String> = strome.iter().map(StromTest::kennung).collect();
    assert_eq!(
        kennungen
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
}

#[test]
fn a_panic_free_run_over_odd_requests() {
    let st = Stand::ohne_dienst(kurz(200));
    for roh in [
        &b"GET / HTTP/1.1\r\nHost: \xff\xfe\r\n\r\n"[..],
        b"POST /starten HTTP/1.1\r\nHost: x\r\nContent-Length: 99999999999999999999\r\n\r\n",
        b"\x00\x01\x02\r\n\r\n",
        b"GET / HTTP/1.1\r\n: x\r\n\r\n",
        b"\r\n\r\n\r\n\r\n",
    ] {
        let _ = anfrage(st.port, roh, Duration::from_secs(2));
    }
    assert_eq!(
        st.senden(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", st.host()).as_bytes())
            .status,
        200
    );
}
