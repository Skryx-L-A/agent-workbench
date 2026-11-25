//! The way to a target without the server: the bytes of a request and the reading of an answer (head, length, chunked,
//! up to the close) against a stand-in on 127.0.0.1.
#![allow(
    clippy::many_single_char_names,
    reason = "s, l, k, w, a, b: stream, line, head, heads in every step"
)]

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use halter::code::Code;
use halter::konfig::Adresse;
use halter::ziel::{self, Koerper, Leitung};
use serde_json::{Map, Value};

fn adresse(host: &str, port: u16, tls: bool) -> Adresse {
    Adresse {
        host: host.to_owned(),
        port,
        tls,
        kopf: "authorization".to_owned(),
        vorsatz: "Bearer ".to_owned(),
    }
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).expect("utf-8")
}

#[test]
fn anfrage_bytes() {
    let mut kopf = Map::new();
    kopf.insert("accept".into(), Value::String("application/json".into()));
    let a = adresse("api.example.invalid", 443, true);
    let get = text(&ziel::anfrage(
        "GET", "/x?a=1", &a, &kopf, b"W", None, false,
    ));
    assert_eq!(
        get,
        "GET /x?a=1 HTTP/1.1\r\nhost: api.example.invalid\r\naccept: application/json\r\naccept-encoding: identity\r\nauthorization: Bearer W\r\nconnection: close\r\n\r\n"
    );
    let post = text(&ziel::anfrage(
        "POST",
        "/c",
        &a,
        &Map::new(),
        b"W",
        Some(b""),
        false,
    ));
    assert!(post.contains("content-length: 0\r\n"), "{post}");
    let post = text(&ziel::anfrage(
        "POST",
        "/c",
        &a,
        &Map::new(),
        b"W",
        Some(b"abc"),
        false,
    ));
    assert!(
        post.ends_with("content-length: 3\r\nconnection: close\r\n\r\nabc"),
        "{post}"
    );
    let get_mit = text(&ziel::anfrage(
        "GET",
        "/c",
        &a,
        &Map::new(),
        b"W",
        Some(b"ab"),
        false,
    ));
    assert!(get_mit.contains("content-length: 2\r\n"));
    let b = adresse("::1", 8443, false);
    let v6 = text(&ziel::anfrage(
        "GET",
        "/",
        &b,
        &Map::new(),
        b"W",
        None,
        true,
    ));
    assert!(v6.contains("host: [::1]:8443\r\n"), "{v6}");
    assert!(
        v6.ends_with("upgrade: websocket\r\nconnection: upgrade\r\n\r\n"),
        "{v6}"
    );
    assert!(!v6.contains("content-length"));
    let c = adresse("127.0.0.1", 80, false);
    assert!(
        text(&ziel::anfrage(
            "GET",
            "/",
            &c,
            &Map::new(),
            b"W",
            None,
            false
        ))
        .contains("host: 127.0.0.1\r\n")
    );
    let d = adresse("127.0.0.1", 443, false);
    assert!(
        text(&ziel::anfrage(
            "GET",
            "/",
            &d,
            &Map::new(),
            b"W",
            None,
            false
        ))
        .contains("host: 127.0.0.1:443\r\n")
    );
}

/// Connects to a stand-in that sends `antwort` and closes; returns the client end.
fn mit_antwort(antwort: Vec<u8>) -> TcpStream {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let _ = s.write_all(&antwort);
        }
    });
    let s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    s
}

fn lesen(antwort: &[u8], methode: &str) -> Result<(u16, Vec<u8>), Code> {
    let mut s = mit_antwort(antwort.to_vec());
    let l: &mut dyn Leitung = &mut s;
    let (k, vorrat) = ziel::kopf_lesen(l, Duration::from_secs(10))?;
    let mut koerper = Koerper::neu(l, &k, vorrat, methode)?;
    let mut alles = Vec::new();
    while let Some(b) = koerper.naechstes()? {
        alles.extend_from_slice(&b);
    }
    Ok((k.status, alles))
}

#[test]
fn koerper_nach_laenge_stuecken_und_ende() {
    assert_eq!(
        lesen(b"HTTP/1.1 200 OK\r\ncontent-length: 3\r\n\r\nabcdef", "GET"),
        Ok((200, b"abc".to_vec()))
    );
    assert_eq!(
        lesen(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n", "GET"),
        Ok((200, Vec::new()))
    );
    assert_eq!(
        lesen(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nab", "GET"),
        Err(Code::ZielFehler)
    );
    assert_eq!(
        lesen(b"HTTP/1.1 200 OK\r\ncontent-length: x\r\n\r\n", "GET"),
        Err(Code::ZielFehler)
    );
    assert_eq!(
        lesen(b"HTTP/1.1 200 OK\r\n\r\nbis zum ende", "GET"),
        Ok((200, b"bis zum ende".to_vec()))
    );
    assert_eq!(
        lesen(b"HTTP/1.1 204 No Content\r\n\r\nxyz", "GET"),
        Ok((204, Vec::new()))
    );
    assert_eq!(
        lesen(b"HTTP/1.1 304 Not Modified\r\n\r\nxyz", "GET"),
        Ok((304, Vec::new()))
    );
    assert_eq!(
        lesen(b"HTTP/1.1 200 OK\r\ncontent-length: 3\r\n\r\nabc", "HEAD"),
        Ok((200, Vec::new()))
    );
    let stueck = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3;ext=1\r\nabc\r\n0A\r\n0123456789\r\n0\r\nx-trailer: 1\r\n\r\nREST";
    assert_eq!(lesen(stueck, "GET"), Ok((200, b"abc0123456789".to_vec())));
    let ohne_anhang = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n1\r\nz\r\n0\r\n\r\n";
    assert_eq!(lesen(ohne_anhang, "GET"), Ok((200, b"z".to_vec())));
    for kaputt in [
        &b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\nzz\r\nab\r\n0\r\n\r\n"[..],
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\nabc\r\n0\r\n\r\n",
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nabcX\n0\r\n\r\n",
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nabc\rX0\r\n\r\n",
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n\r\n",
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n1000000000000000\r\n",
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nab",
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: gzip\r\n\r\nab",
    ] {
        assert_eq!(
            lesen(kaputt, "GET"),
            Err(Code::ZielFehler),
            "{}",
            String::from_utf8_lossy(kaputt)
        );
    }
    let lange_zeile = [
        b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n".to_vec(),
        vec![b'1'; 1100],
    ]
    .concat();
    assert_eq!(lesen(&lange_zeile, "GET"), Err(Code::ZielFehler));
}

#[test]
fn kopf_grenzen_und_form() {
    let mut riesig = b"HTTP/1.1 200 OK\r\nx: ".to_vec();
    riesig.extend(vec![b'a'; ziel::KOPF_BYTES_MAX + 10]);
    assert!(matches!(lesen(&riesig, "GET"), Err(Code::ZielFehler)));
    for kaputt in [
        &b"HTTP/1.1 200 OK\r\nx y: 1\r\n\r\n"[..],
        b"HTTP/1.1 200 OK\r\n: 1\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nohne-doppelpunkt\r\n\r\n",
        b"HTTP/1.1 20 OK\r\n\r\n",
        b"HTTP/1.1 2000 OK\r\n\r\n",
        b"HTTP/1.1 abc OK\r\n\r\n",
        b"HTTP/1.1 099 OK\r\n\r\n",
        b"HTTP/1.1 600 OK\r\n\r\n",
        b"SSH-2.0 x\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nx\x7f: 1\r\n\r\n",
        b"HTTP/1.1 200",
    ] {
        assert_eq!(
            lesen(kaputt, "GET"),
            Err(Code::ZielFehler),
            "{}",
            String::from_utf8_lossy(kaputt)
        );
    }
    assert_eq!(
        lesen(b"HTTP/1.0 599 X\r\nx:\t1\t\r\n\r\n", "HEAD"),
        Ok((599, Vec::new()))
    );
    assert_eq!(
        lesen(b"HTTP/1.1 100 Continue\r\n\r\n", "GET"),
        Ok((100, Vec::new()))
    );
}

#[test]
fn weitergegebene_koepfe() {
    let mut s = mit_antwort(
        b"HTTP/1.1 200 OK\r\nX-A: 1\r\nContent-Length: 0\r\nKeep-Alive: x\r\nx-leer:\r\nx-wert: a\x01b\r\n\r\n".to_vec(),
    );
    let l: &mut dyn Leitung = &mut s;
    let (k, _) = ziel::kopf_lesen(l, Duration::from_secs(10)).expect("head");
    assert_eq!(
        k.weiter(),
        Err(Code::ZielFehler),
        "a control character in a value"
    );
    let mut s = mit_antwort(
        b"HTTP/1.1 200 OK\r\nX-A: 1\r\nContent-Length: 0\r\nKeep-Alive: x\r\nx-leer:\r\n\r\n"
            .to_vec(),
    );
    let l: &mut dyn Leitung = &mut s;
    let (k, _) = ziel::kopf_lesen(l, Duration::from_secs(10)).expect("head");
    let w = k.weiter().expect("heads");
    assert_eq!(w.len(), 2, "{w:?}");
    assert_eq!(w["x-a"], "1");
    assert_eq!(w["x-leer"], "");
    // two lines of 4096 characters join to 8194: too long
    let a = "a".repeat(4096);
    let mut s = mit_antwort(format!("HTTP/1.1 200 OK\r\nx: {a}\r\nx: {a}\r\n\r\n").into_bytes());
    let l: &mut dyn Leitung = &mut s;
    let (k, _) = ziel::kopf_lesen(l, Duration::from_secs(10)).expect("head");
    assert_eq!(k.weiter(), Err(Code::ZielFehler));
    let b = "a".repeat(4095);
    let mut s = mit_antwort(format!("HTTP/1.1 200 OK\r\nx: {b}\r\nx: {b}\r\n\r\n").into_bytes());
    let l: &mut dyn Leitung = &mut s;
    let (k, _) = ziel::kopf_lesen(l, Duration::from_secs(10)).expect("head");
    assert_eq!(
        k.weiter().expect("heads")["x"].as_str().map(str::len),
        Some(8192)
    );
    let viele: String = (0..64).fold(String::new(), |mut t, i| {
        use std::fmt::Write as _;
        let _ = write!(t, "k{i}: v\r\n");
        t
    });
    let mut s =
        mit_antwort(format!("HTTP/1.1 200 OK\r\n{viele}set-cookie: a\r\n\r\n").into_bytes());
    let l: &mut dyn Leitung = &mut s;
    let (k, _) = ziel::kopf_lesen(l, Duration::from_secs(10)).expect("head");
    assert_eq!(
        k.weiter().map(|w| w.len()),
        Ok(64),
        "64 heads after the forbidden ones are dropped"
    );
    let k2 = k.clone();
    assert!(k2.hat("set-cookie") && !k2.hat("cookie"));
    assert_eq!(k2.wert("k1"), Some("v"));
    for (ce, soll) in [
        ("gzip", true),
        ("identity", false),
        ("Identity", false),
        ("identity, br", true),
        ("", false),
    ] {
        let k = ziel::Antwortkopf {
            status: 200,
            zeilen: vec![("content-encoding".into(), ce.into())],
        };
        assert_eq!(k.komprimiert(), soll, "{ce}");
    }
}

#[test]
fn verbinden_scheitert_sauber() {
    // a port nobody listens on (bound and closed again)
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    drop(l);
    assert!(matches!(
        ziel::verbinden(&adresse("127.0.0.1", port, false)),
        Err(Code::ZielFehler)
    ));
    assert!(matches!(
        ziel::verbinden(&adresse("name.invalid", 443, true)),
        Err(Code::ZielFehler)
    ));
}
