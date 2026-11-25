//! The outside tests of section 12 that run without the enclosure: M3 to M7, M9 and M10 against a stand-in of the target on
//! the loopback, with the program as a process (empty environment) or the library; and the steps of the start and the end.
//! M1, M2 (inside the enclosure), M8 (TLS, `tests/tls.rs`) and M11 (real harnesses) are not here.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![allow(
    clippy::many_single_char_names,
    reason = "short names for case, answer, line and process keep the tables of the tests readable"
)]

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gemeinsam::{
    Antwort, Attrappe, FRIST, MODELL, Prozess, SCHLUESSEL, Skript, Wegwerf, anfrage, argv, dienst,
    enthaelt, koerper, lauf_bis_ende, post, stueckweise, ueber_socket, zeilen,
};
use modellweg::dienst::{Dienst, FAEDEN_MAX};
use modellweg::start::{Startfehler, vorbereiten};
use modellweg::ziel::{FRISTEN, Fristen};
use serde_json::Value;

/// A running program against the stand-in: folder, socket, protocol folder.
struct Lauf {
    w: Wegwerf,
    p: Prozess,
    /// The arguments of the start, for the check of the visible command line.
    #[cfg(target_os = "linux")]
    a: Vec<OsString>,
}

impl Lauf {
    fn start(ziel: &str) -> Self {
        let w = Wegwerf::neu("lauf");
        let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
        let a = argv(&w.pfad("m.sock"), ziel, &schluessel, &w.protokoll());
        let (p, erste) = Prozess::start(&a, &[]);
        assert_eq!(erste, "{\"bereit\":true}");
        Self {
            w,
            p,
            #[cfg(target_os = "linux")]
            a,
        }
    }

    fn socket(&self) -> std::path::PathBuf {
        self.w.pfad("m.sock")
    }

    fn sende(&self, roh: &[u8]) -> Vec<u8> {
        ueber_socket(&self.socket(), roh)
    }

    fn zeilen(&self) -> Vec<Value> {
        zeilen(&self.w.protokoll())
    }
}

fn antwort(status: u16, kopf: &str, koerper: &[u8]) -> Vec<u8> {
    let mut v = format!(
        "HTTP/1.1 {status} X\r\n{kopf}Content-Length: {}\r\n\r\n",
        koerper.len()
    )
    .into_bytes();
    v.extend_from_slice(koerper);
    v
}

#[test]
fn m3_another_model_path_or_method_never_reaches_the_target() {
    let a = Attrappe::neu(Box::new(|_| vec![antwort(200, "", b"{}")]));
    let l = Lauf::start(&a.ziel());
    let fremd = r#"{"model":"lokal-beispiel/modell-b","messages":[]}"#;
    assert_eq!(Antwort::lies(&l.sende(&post(fremd, ""))).status, 403);
    assert_eq!(
        Antwort::lies(&l.sende(b"POST /v1/models HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}")).status,
        404
    );
    assert_eq!(
        Antwort::lies(
            &l.sende(b"PUT /v1/chat/completions HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}")
        )
        .status,
        405
    );
    assert_eq!(a.verbunden.load(Ordering::SeqCst), 0);
    assert_eq!(
        Antwort::lies(&l.sende(&post(&koerper("x"), ""))).status,
        200
    );
    assert_eq!(a.verbunden.load(Ordering::SeqCst), 1);
    let z = l.zeilen();
    let gruende: Vec<Value> = z.iter().map(|z| z["grund"].clone()).collect();
    let soll = [
        "mw.modell_nicht_erlaubt",
        "mw.pfad_unbekannt",
        "mw.methode_falsch",
    ];
    let mut soll: Vec<Value> = soll.into_iter().map(Value::from).collect();
    soll.push(Value::Null);
    assert_eq!(gruende, soll);
}

#[test]
fn m4_the_bearer_of_the_harness_is_replaced() {
    let a = Attrappe::neu(Box::new(|_| {
        vec![antwort(200, "Content-Type: application/json\r\n", b"{}")]
    }));
    let l = Lauf::start(&a.ziel());
    let roh = post(
        &koerper("x"),
        "Authorization: Bearer platzhalter\r\nX-Api-Key: platzhalter\r\n",
    );
    assert_eq!(Antwort::lies(&l.sende(&roh)).status, 200);
    let e = a.erhalten();
    assert_eq!(e.len(), 1);
    assert!(
        e[0].kopf
            .contains(&format!("\r\nAuthorization: Bearer {SCHLUESSEL}\r\n")),
        "{}",
        e[0].kopf
    );
    assert!(!e[0].alles().contains("platzhalter"), "{}", e[0].alles());
    assert_eq!(e[0].koerper, koerper("x").as_bytes());
}

/// The value of `Authorization` the stand-in got.
fn bearer(e: &gemeinsam::Erhalten) -> String {
    e.kopf
        .lines()
        .find_map(|z| z.strip_prefix("Authorization: "))
        .unwrap_or("")
        .to_owned()
}

#[test]
fn m5_a_mirrored_key_never_reaches_the_harness() {
    type Spiegel = fn(&str) -> Vec<Vec<u8>>;
    let faelle: [(&str, Spiegel, u16, &str); 5] = [
        (
            "koerper",
            |b| {
                vec![antwort(
                    200,
                    "Content-Type: application/json\r\n",
                    format!("{{\"echo\":\"{b}\"}}").as_bytes(),
                )]
            },
            502,
            "leck_gesperrt",
        ),
        (
            "fehler",
            |b| {
                vec![antwort(
                    401,
                    "Content-Type: application/json\r\n",
                    format!("{{\"error\":\"bad {b}\"}}").as_bytes(),
                )]
            },
            502,
            "leck_gesperrt",
        ),
        (
            "kopf",
            |b| {
                vec![antwort(
                    200,
                    &format!("Content-Type: text/plain; k={}\r\n", b.replace(' ', "")),
                    b"ok",
                )]
            },
            502,
            "leck_gesperrt",
        ),
        (
            "anderer_kopf",
            |b| {
                vec![antwort(
                    200,
                    &format!("X-Echo: {b}\r\nSet-Cookie: {b}\r\nContent-Type: text/plain\r\n"),
                    b"ok",
                )]
            },
            200,
            "weitergereicht",
        ),
        (
            "teilgrenze",
            |b| {
                let k = format!("data: {{\"echo\":\"{b}\"}}\n\n").into_bytes();
                let mitte = 30;
                vec![
                    b"HTTP/1.1 200 X\r\nContent-Type: text/event-stream\r\n\r\n".to_vec(),
                    k[..mitte].to_vec(),
                    k[mitte..].to_vec(),
                ]
            },
            502,
            "leck_gesperrt",
        ),
    ];
    for (name, spiegel, status, ergebnis) in faelle {
        let a = Attrappe::neu(Box::new(move |e| spiegel(&bearer(e))));
        let l = Lauf::start(&a.ziel());
        let roh = l.sende(&post(&koerper("x"), ""));
        assert!(
            !enthaelt(&roh, SCHLUESSEL.as_bytes()),
            "{name}: {}",
            String::from_utf8_lossy(&roh)
        );
        let ant = Antwort::lies(&roh);
        assert_eq!(ant.status, status, "{name}");
        let z = l.zeilen();
        assert_eq!(z.len(), 1, "{name}");
        assert_eq!(z[0]["ergebnis"], ergebnis, "{name}");
        if ergebnis == "weitergereicht" {
            assert_eq!(
                ant.ohne_rahmung(),
                vec![("content-type".to_owned(), "text/plain".to_owned())],
                "{name}"
            );
            assert_eq!(ant.koerper, b"ok", "{name}");
        }
    }
}

#[test]
fn m5_a_key_late_in_a_stream_tears_the_connection_down() {
    let a = Attrappe::neu(Box::new(|e| {
        vec![
            b"HTTP/1.1 200 X\r\nContent-Type: text/event-stream\r\n\r\n".to_vec(),
            b"data: {\"choices\":[{\"delta\":{\"content\":\"Hallo, das ist ein langer Teil\"}}]}\n\n".to_vec(),
            format!("data: {{\"k\":\"{}\"}}\n\n", bearer(e)).into_bytes(),
            b"data: [DONE]\n\n".to_vec(),
        ]
    }));
    let l = Lauf::start(&a.ziel());
    let roh = l.sende(&post(&koerper("x"), "Accept: text/event-stream\r\n"));
    assert!(!enthaelt(&roh, SCHLUESSEL.as_bytes()));
    let ant = Antwort::lies(&roh);
    assert_eq!(ant.status, 200);
    assert!(
        ant.koerper.starts_with(b"data: {\"choices\""),
        "{}",
        String::from_utf8_lossy(&ant.koerper)
    );
    assert!(!enthaelt(&ant.koerper, b"[DONE]"));
    let z = l.zeilen();
    assert_eq!(
        (z[0]["ergebnis"].as_str(), z[0]["status"].as_u64()),
        (Some("leck_gesperrt"), Some(200))
    );
}

#[test]
fn m6_errors_of_the_target_pass_unchanged() {
    let faelle: [(u16, &[u8]); 3] = [
        (403, br#"{"error":{"message":"model lokal-beispiel/modell-b is not allowed for this key","type":"policy_model_blocked","code":403}}"#),
        (401, b"unauthorized"),
        (500, b"{\"error\":\"intern\"}\n"),
    ];
    for (status, k) in faelle {
        let a = Attrappe::neu(Box::new(move |_| {
            vec![antwort(status, "Content-Type: application/json\r\n", k)]
        }));
        let l = Lauf::start(&a.ziel());
        let ant = Antwort::lies(&l.sende(&post(&koerper("x"), "")));
        assert_eq!((ant.status, ant.koerper.as_slice()), (status, k));
        let z = l.zeilen();
        assert_eq!(
            (z[0]["ergebnis"].as_str(), z[0]["status"].as_u64()),
            (Some("weitergereicht"), Some(u64::from(status)))
        );
    }
}

#[test]
fn m7_reasoning_and_a_stream_of_100_events_pass_byte_for_byte() {
    let json = br#"{"id":"c2","choices":[{"message":{"role":"assistant","content":"Hallo","reasoning":"Der Nutzer gruesst."}}],"usage":{"prompt_tokens":9,"completion_tokens":3}}"#;
    let a = Attrappe::neu(Box::new(|_| {
        vec![antwort(200, "Content-Type: application/json\r\n", json)]
    }));
    let l = Lauf::start(&a.ziel());
    let ant = Antwort::lies(&l.sende(&post(&koerper("x"), "")));
    assert_eq!(ant.koerper, json);
    let z = l.zeilen();
    assert_eq!(
        (z[0]["token_ein"].as_u64(), z[0]["token_aus"].as_u64()),
        (Some(9), Some(3))
    );

    let mut ereignisse: Vec<Vec<u8>> = (0..99)
        .map(|i| format!("data: {{\"id\":\"c1\",\"choices\":[{{\"delta\":{{\"reasoning\":\"Schritt {i}\"}}}}]}}\n\n").into_bytes())
        .collect();
    ereignisse.push(b"data: {\"id\":\"c1\",\"choices\":[],\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":99}}\n\n".to_vec());
    let teile: Vec<&[u8]> = ereignisse.iter().map(Vec::as_slice).collect();
    let rumpf = stueckweise(&teile);
    let erwartet: Vec<u8> = ereignisse.concat();
    let a = Attrappe::neu(Box::new(move |_| {
        let mut v = vec![b"HTTP/1.1 200 X\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec()];
        v.extend(rumpf.chunks(997).map(<[u8]>::to_vec));
        v
    }));
    let l = Lauf::start(&a.ziel());
    let ant = Antwort::lies(&l.sende(&post(&koerper("x"), "Accept: text/event-stream\r\n")));
    assert_eq!(
        ant.ohne_rahmung(),
        vec![("content-type".to_owned(), "text/event-stream".to_owned())]
    );
    assert_eq!(ant.koerper, erwartet);
    let z = l.zeilen();
    assert_eq!(
        (z[0]["token_ein"].as_u64(), z[0]["token_aus"].as_u64()),
        (Some(11), Some(99))
    );
}

#[test]
fn m9_a_redirect_is_not_followed() {
    let zweites = Attrappe::neu(Box::new(|_| vec![antwort(200, "", b"{}")]));
    let ort = format!(
        "http://127.0.0.1:{}/v1/chat/completions",
        zweites.adresse.port()
    );
    let a = Attrappe::neu(Box::new(move |_| {
        vec![antwort(302, &format!("Location: {ort}\r\n"), b"")]
    }));
    let l = Lauf::start(&a.ziel());
    let ant = Antwort::lies(&l.sende(&post(&koerper("x"), "")));
    assert_eq!(
        (ant.status, ant.ohne_rahmung(), ant.koerper),
        (302, Vec::new(), Vec::new())
    );
    assert_eq!(zweites.verbunden.load(Ordering::SeqCst), 0);
}

#[test]
fn m9_the_name_is_resolved_once_and_only_its_first_addresses_are_used() {
    let erstes = TcpListener::bind("127.0.0.1:0").expect("bind");
    let zweites = TcpListener::bind("127.0.0.1:0").expect("bind");
    let adressen = [
        erstes.local_addr().expect("addr"),
        zweites.local_addr().expect("addr"),
    ];
    let aufrufe = Arc::new(AtomicUsize::new(0));
    let a2 = Arc::clone(&aufrufe);
    let aufloeser = move |host: &str, port: u16| -> io::Result<Vec<SocketAddr>> {
        assert_eq!((host, port), ("ziel.beispiel.invalid", 8443));
        let n = a2.fetch_add(1, Ordering::SeqCst);
        Ok(vec![adressen[n.min(1)]])
    };
    let w = Wegwerf::neu("m9");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let args = argv(
        &w.pfad("m.sock"),
        "https://ziel.beispiel.invalid:8443/v1",
        &schluessel,
        &w.protokoll(),
    );
    let fristen = Fristen {
        verbinden: Duration::from_secs(1),
        kopf: Duration::from_secs(1),
    };
    let b = vorbereiten(
        &args,
        rustix::process::getuid().as_raw(),
        &aufloeser,
        fristen,
        Box::new(|| gemeinsam::ZEIT),
    )
    .expect("ready");
    erstes.set_nonblocking(true).expect("nonblocking");
    zweites.set_nonblocking(true).expect("nonblocking");
    for _ in 0..2 {
        let ant = Antwort::lies(&anfrage(&b.dienst, &post(&koerper("x"), "")));
        assert_eq!(ant.status, 502);
    }
    assert_eq!(aufrufe.load(Ordering::SeqCst), 1);
    let gezaehlt = |l: &TcpListener| std::iter::from_fn(|| l.accept().ok()).count();
    assert_eq!((gezaehlt(&erstes), gezaehlt(&zweites)), (2, 0));
    let z = zeilen(&w.protokoll());
    assert_eq!(z.len(), 2);
    assert_eq!(z[0]["grund"], "mw.ziel_nicht_erreichbar");
    let _ = fs::remove_file(w.pfad("m.sock"));
}

#[test]
fn m10_the_line_carries_no_content() {
    let a = Attrappe::neu(Box::new(|e| {
        let marke = String::from_utf8_lossy(&e.koerper).contains("MARKE-PROMPT");
        assert!(marke);
        vec![antwort(
            200,
            "Content-Type: application/json\r\n",
            br#"{"choices":[{"message":{"content":"MARKE-ANTWORT","reasoning":"MARKE-DENKEN"}}],"usage":{"prompt_tokens":1,"completion_tokens":2}}"#,
        )]
    }));
    let l = Lauf::start(&a.ziel());
    for i in 0..20 {
        let ant = Antwort::lies(&l.sende(&post(
            &koerper(&format!("MARKE-PROMPT-{i}")),
            "Authorization: Bearer MARKE-PLATZ\r\n",
        )));
        assert_eq!(ant.status, 200);
    }
    let mut kette = Vec::new();
    for e in fs::read_dir(l.w.protokoll()).expect("dir") {
        kette.extend(fs::read(e.expect("entry").path()).expect("read"));
    }
    for marke in ["MARKE", SCHLUESSEL, "127.0.0.1"] {
        assert!(!enthaelt(&kette, marke.as_bytes()), "{marke}");
    }
    let p =
        formate::kette::pruefe_quelle(&l.w.protokoll(), "gateway", formate::kette::NULL_HASH, None)
            .expect("check");
    assert!(p.ok(), "{}", p.ausgabe());
    assert_eq!(l.zeilen().len(), 20);
}

#[test]
fn the_socket_has_mode_0600_and_the_line_the_time_of_the_system() {
    let a = Attrappe::neu(Box::new(|_| vec![antwort(200, "", b"{}")]));
    let vorher = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis(),
    )
    .expect("ms");
    let l = Lauf::start(&a.ziel());
    let modus = fs::symlink_metadata(l.socket())
        .expect("socket")
        .permissions()
        .mode();
    assert_eq!(modus & 0o777, 0o600);
    assert_eq!(
        Antwort::lies(&l.sende(&post(&koerper("x"), ""))).status,
        200
    );
    let nachher = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis(),
    )
    .expect("ms");
    let z = l.zeilen();
    let zeit = z[0]["zeit"].as_str().expect("zeit");
    let ms = zeit_ms(zeit);
    assert!(vorher <= ms && ms <= nachher, "{vorher} {ms} {nachher}");
    assert_eq!(z[0]["sitzung"], "s-2");
    assert_eq!(z[0]["route"], "lokal-beispiel");
    assert_eq!(z[0]["modell"], MODELL);
}

/// Milliseconds of a time text `JJJJ-MM-TTThh:mm:ss.mmmZ` (days from the civil date, Hinnant).
fn zeit_ms(t: &str) -> i64 {
    let n = |a: usize, b: usize| t[a..b].parse::<i64>().expect("digits");
    let (j, m, d) = (n(0, 4), n(5, 7), n(8, 10));
    let j = if m <= 2 { j - 1 } else { j };
    let era = j.div_euclid(400);
    let yoe = j - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let tage = era * 146_097 + doe - 719_468;
    ((tage * 24 + n(11, 13)) * 60 + n(14, 16)) * 60_000 + n(17, 19) * 1000 + n(20, 23)
}

#[test]
fn the_end_tears_running_answers_down_writes_their_lines_and_deletes_the_socket() {
    let a = Attrappe::neu(Box::new(|_| {
        vec![
            b"HTTP/1.1 200 X\r\nContent-Type: text/event-stream\r\n\r\n".to_vec(),
            b"data: {\"choices\":[{\"delta\":{\"content\":\"ein erster Teil, lang genug\"}}]}\n\n"
                .to_vec(),
            Vec::new(),
            b"data: [DONE]\n\n".to_vec(),
        ]
    }));
    let mut l = Lauf::start(&a.ziel());
    let mut s = UnixStream::connect(l.socket()).expect("connect");
    s.set_read_timeout(Some(FRIST)).expect("timeout");
    s.write_all(&post(&koerper("x"), "")).expect("send");
    let mut erstes = [0u8; 64];
    let n = s.read(&mut erstes).expect("first bytes");
    assert!(n > 0);
    let start = Instant::now();
    l.p.signal(rustix::process::Signal::TERM);
    assert_eq!(l.p.warte(FRIST), Some(0));
    assert!(
        start.elapsed() < Duration::from_secs(4),
        "{:?}",
        start.elapsed()
    );
    assert!(!l.socket().exists());
    let z = l.zeilen();
    assert_eq!(z.len(), 1);
    assert_eq!(
        (z[0]["ergebnis"].as_str(), z[0]["status"].as_u64()),
        (Some("weitergereicht"), Some(200))
    );
}

#[test]
fn sigint_ends_too() {
    let mut l = Lauf::start("http://127.0.0.1:9/v1");
    l.p.signal(rustix::process::Signal::INT);
    assert_eq!(l.p.warte(FRIST), Some(0));
    assert!(!l.socket().exists());
}

#[test]
fn a_target_that_cannot_be_reached_or_answers_late() {
    let frei = TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let l = Lauf::start(&format!("http://127.0.0.1:{frei}/v1"));
    let ant = Antwort::lies(&l.sende(&post(&koerper("x"), "")));
    assert_eq!(
        (ant.status, ant.koerper.as_slice()),
        (
            502,
            &br#"{"error":{"code":"mw.ziel_nicht_erreichbar","message":"Ziel nicht erreichbar"}}"#
                [..]
        )
    );
    assert_eq!(l.zeilen()[0]["ergebnis"], "ziel_fehler");

    let stumm = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = stumm.local_addr().expect("addr").port();
    let w = Wegwerf::neu("frist");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let args = argv(
        &w.pfad("m.sock"),
        &format!("http://127.0.0.1:{port}/v1"),
        &schluessel,
        &w.protokoll(),
    );
    let fristen = Fristen {
        verbinden: FRISTEN.verbinden,
        kopf: Duration::from_millis(500),
    };
    let b = vorbereiten(
        &args,
        rustix::process::getuid().as_raw(),
        &gemeinsam_aufloeser,
        fristen,
        Box::new(|| gemeinsam::ZEIT),
    )
    .expect("ready");
    let start = Instant::now();
    let ant = Antwort::lies(&anfrage(&b.dienst, &post(&koerper("x"), "")));
    assert_eq!(
        (ant.status, ant.koerper.as_slice()),
        (
            504,
            &br#"{"error":{"code":"mw.ziel_frist","message":"Ziel antwortet nicht"}}"#[..]
        )
    );
    assert!(start.elapsed() < Duration::from_secs(5));
    let z = zeilen(&w.protokoll());
    assert_eq!(
        (z[0]["ergebnis"].as_str(), z[0]["grund"].as_str()),
        (Some("ziel_fehler"), Some("mw.ziel_frist"))
    );
    drop(stumm);
}

fn gemeinsam_aufloeser(_: &str, _: u16) -> io::Result<Vec<SocketAddr>> {
    panic!("a loopback target is not resolved")
}

#[test]
fn the_steps_before_readiness() {
    let w = Wegwerf::neu("schritte");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let socket = w.pfad("m.sock");

    let ohne_protokoll = argv(
        &socket,
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.pfad("fehlt"),
    );
    let (exit, out, err) = lauf_bis_ende(&ohne_protokoll);
    assert_eq!(
        (exit, out.as_slice(), err.as_slice()),
        (
            78,
            &b""[..],
            &b"mw.protokoll_ordner: Protokollordner fehlt oder ist kein Ordner\n"[..]
        )
    );
    let datei_statt_ordner = argv(&socket, "http://127.0.0.1:9/v1", &schluessel, &schluessel);
    assert_eq!(lauf_bis_ende(&datei_statt_ordner).0, 78);

    fs::write(&socket, b"belegt").expect("occupy");
    let a = argv(
        &socket,
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    let (exit, out, err) = lauf_bis_ende(&a);
    assert_eq!(
        (exit, out.as_slice(), err.as_slice()),
        (
            74,
            &b""[..],
            &b"mw.socket_belegt: Socket belegt oder nicht anlegbar\n"[..]
        )
    );
    assert_eq!(fs::read(&socket).expect("untouched"), b"belegt");
    fs::remove_file(&socket).expect("remove");

    let a = argv(
        &socket,
        "https://ziel.beispiel.invalid/v1",
        &schluessel,
        &w.protokoll(),
    );
    let nie = |_: &str, _: u16| -> io::Result<Vec<SocketAddr>> { Err(io::Error::other("no")) };
    let f = vorbereiten(
        &a,
        rustix::process::getuid().as_raw(),
        &nie,
        FRISTEN,
        Box::new(|| 0),
    )
    .err();
    assert_eq!(f, Some(Startfehler::ZielUnaufloesbar));
    assert!(!socket.exists());

    let fehler = [
        (Startfehler::Aufruf, 64, "mw.aufruf: Aufruf ungueltig\n"),
        (
            Startfehler::BetriebsartUnbekannt,
            64,
            "mw.betriebsart_unbekannt: Betriebsart unbekannt oder nicht gebaut\n",
        ),
        (
            Startfehler::SchluesselDatei,
            78,
            "mw.schluessel_datei: Schluesseldatei nicht eigen, nicht 0600 oder keine Datei\n",
        ),
        (
            Startfehler::SchluesselForm,
            78,
            "mw.schluessel_form: Schluessel hat falsche Form\n",
        ),
        (
            Startfehler::ProtokollOrdner,
            78,
            "mw.protokoll_ordner: Protokollordner fehlt oder ist kein Ordner\n",
        ),
        (
            Startfehler::ZielUnaufloesbar,
            69,
            "mw.ziel_unaufloesbar: Ziel nicht aufloesbar\n",
        ),
        (
            Startfehler::SocketBelegt,
            74,
            "mw.socket_belegt: Socket belegt oder nicht anlegbar\n",
        ),
        (Startfehler::Intern, 70, "mw.intern: interner Fehler\n"),
    ];
    for (f, exit, zeile) in fehler {
        assert_eq!((f.exit(), f.zeile().as_str()), (exit, zeile));
        assert_ne!(exit, 5, "exit 5 belongs to mw.kernabbild only");
    }
}

#[test]
fn step_1_turns_the_shielding_into_a_note_or_exit_5() {
    use kernabbild::{Abbruch, Abgeschirmt, Hinweis};
    use modellweg::start::abschirmen;
    assert_eq!(
        (
            Startfehler::Kernabbild.exit(),
            Startfehler::Kernabbild.zeile().as_str()
        ),
        (5, "mw.kernabbild: Kernabbild nicht abschirmbar\n")
    );
    let mut err = Vec::new();
    assert_eq!(
        abschirmen(Ok(Abgeschirmt { hinweis: None }), &mut err),
        Ok(())
    );
    assert!(err.is_empty(), "no line without a note");
    for hinweis in [Hinweis::MusterDatei, Hinweis::MusterFehlt] {
        let mut err = Vec::new();
        assert_eq!(
            abschirmen(
                Ok(Abgeschirmt {
                    hinweis: Some(hinweis)
                }),
                &mut err
            ),
            Ok(())
        );
        assert_eq!(
            String::from_utf8(err).expect("utf-8"),
            format!("mw.kernabbild_hinweis: {}\n", hinweis.text())
        );
    }
    for grund in [
        Abbruch::AbzugSperren,
        Abbruch::GrenzeSetzen,
        Abbruch::MusterPipe,
        Abbruch::MusterNichtLesbar,
    ] {
        let mut err = Vec::new();
        assert_eq!(abschirmen(Err(grund), &mut err), Err(5), "{grund:?}");
        assert_eq!(
            String::from_utf8(err).expect("utf-8"),
            format!(
                "mw.kernabbild: Kernabbild nicht abschirmbar\nmw.kernabbild_grund: {}\n",
                grund.grund()
            )
        );
    }
}

#[test]
fn a_socket_folder_that_is_a_link_ends_the_start_with_74() {
    let w = Wegwerf::neu("verweis");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    // The folder of the socket is a link to a real folder: the start ends with 74 before the bind.
    let echt = w.pfad("echt");
    fs::create_dir(&echt).expect("folder");
    let verweis = w.pfad("lnk");
    std::os::unix::fs::symlink(&echt, &verweis).expect("link");
    let a = argv(
        &verweis.join("m.sock"),
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    let (exit, out, err) = lauf_bis_ende(&a);
    assert_eq!(
        (exit, out.as_slice(), err.as_slice()),
        (
            74,
            &b""[..],
            &b"mw.socket_belegt: Socket belegt oder nicht anlegbar\n"[..]
        )
    );
    assert_eq!(fs::read_dir(&echt).expect("dir").count(), 0, "no socket");
}

#[test]
fn a_link_above_the_folder_of_the_socket_ends_the_start_with_74() {
    let w = Wegwerf::neu("ueber");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    // `echt/unten` is the real folder of the socket, `lnk` a link to `echt`: in the socket path
    // `…/lnk/unten/m.sock` the link stands above the folder of the socket (contract 8, step 7, part a).
    let echt = w.pfad("echt");
    let unten = echt.join("unten");
    // Each folder gets its rights of its own: step 7 of another test sets the umask of the **whole** process to 0177
    // for the length of its bind, and a folder made in that moment would have no `x` (measured 24.11.2026: the
    // `create_dir_all` of this test ended with `EACCES` while another test was in step 7).
    for d in [&echt, &unten] {
        fs::create_dir(d).expect("folder");
        fs::set_permissions(d, fs::Permissions::from_mode(0o700)).expect("chmod");
    }
    std::os::unix::fs::symlink(&echt, w.pfad("lnk")).expect("link");
    let a = argv(
        &w.pfad("lnk").join("unten").join("m.sock"),
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    let (exit, out, err) = lauf_bis_ende(&a);
    assert_eq!(
        (exit, out.as_slice(), err.as_slice()),
        (
            74,
            &b""[..],
            &b"mw.socket_belegt: Socket belegt oder nicht anlegbar\n"[..]
        )
    );
    assert_eq!(fs::read_dir(&unten).expect("dir").count(), 0, "no socket");
}

#[test]
fn a_socket_folder_other_users_may_write_into_ends_the_start_with_74() {
    let w = Wegwerf::neu("offen");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    // The window between the bind and the check stays closed only while no other user may write into the folder
    // (contract 8, step 7, part b), so a folder with a `w` for others ends the start before the bind.
    let offen = w.pfad("o");
    fs::create_dir(&offen).expect("folder");
    fs::set_permissions(&offen, fs::Permissions::from_mode(0o777)).expect("chmod");
    let socket = offen.join("m.sock");
    let a = argv(
        &socket,
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    let (exit, out, err) = lauf_bis_ende(&a);
    assert_eq!(
        (exit, out.as_slice(), err.as_slice()),
        (
            74,
            &b""[..],
            &b"mw.socket_belegt: Socket belegt oder nicht anlegbar\n"[..]
        )
    );
    assert!(!socket.exists(), "nothing was bound");
}

#[test]
fn the_key_is_in_no_argument_and_no_environment() {
    let l = Lauf::start("http://127.0.0.1:9/v1");
    let pid = l.p.pid();
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        // PR_SET_DUMPABLE 0 makes the entries under /proc/<pid> belong to root (or to nobody in a user namespace).
        let meta = fs::metadata(format!("/proc/{pid}/stat")).expect("the entry");
        assert_ne!(
            meta.uid(),
            rustix::process::getuid().as_raw(),
            "the process is not dumpable"
        );
        // RLIMIT_CORE 1 soft and hard (crate kernabbild)
        let limits = fs::read_to_string(format!("/proc/{pid}/limits")).expect("limits");
        let zeile = limits
            .lines()
            .find(|z| z.starts_with("Max core file size"))
            .expect("the line of the core limit");
        let felder: Vec<&str> = zeile.split_whitespace().collect();
        assert_eq!(&felder[4..6], ["1", "1"], "{zeile}");
        // Visibility first: the command line is there in full (program and every argument, each ended by NUL), so the
        // absence of the key below is not the absence of an empty or cut line.
        let mut teile = vec![OsString::from(gemeinsam::programm())];
        teile.extend(l.a.iter().cloned());
        let mut soll = Vec::new();
        for teil in &teile {
            soll.extend_from_slice(teil.as_encoded_bytes());
            soll.push(0);
        }
        let cmdline = fs::read(format!("/proc/{pid}/cmdline")).expect("cmdline");
        assert_eq!(cmdline, soll, "the whole command line is visible");
        // Counter-check on the same bytes: the search finds an argument that is there.
        assert!(enthaelt(&cmdline, b"\0--schluessel-datei\0"));
        assert!(!enthaelt(&cmdline, SCHLUESSEL.as_bytes()));
        let environ = fs::read(format!("/proc/{pid}/environ"));
        assert_eq!(
            environ.map_err(|e| e.kind()),
            Err(io::ErrorKind::PermissionDenied),
            "not dumpable: the environment is closed to the same user"
        );
        let status = fs::read_to_string(format!("/proc/{pid}/status")).expect("status");
        assert!(status.lines().any(|z| z.starts_with("PPid:")), "{status}");
    }
    let _ = pid;
}

#[cfg(target_os = "linux")]
#[test]
fn the_end_of_the_parent_ends_the_model_path() {
    let w = Wegwerf::neu("eltern");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let socket = w.pfad("m.sock");
    let a = argv(
        &socket,
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    let fifo = w.pfad("bereit");
    let ok = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo");
    assert!(ok.success());
    let mut sh = std::process::Command::new("sh");
    sh.arg("-c")
        .arg("\"$0\" \"$@\" > \"$F\" & echo $!; head -n 1 < \"$F\" > /dev/null")
        .arg(gemeinsam::programm())
        .args(&a)
        .env_clear()
        .env("F", &fifo)
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped());
    let aus = sh.output().expect("sh");
    let pid: i32 = String::from_utf8_lossy(&aus.stdout)
        .trim()
        .parse()
        .expect("pid");
    warte_ende(pid, "the model path outlived its parent");
    assert!(!socket.exists());
}

/// Waits at most [`FRIST`] for the end of `pid` and fails with `grund` if it stays. A process this test does not wait
/// for becomes a zombie until its new parent reaps it, so a zombie counts as ended. A process that stayed is killed
/// before the failure: a broken program or a mutant must leave no orphan behind (incident 21.11.2026, nine test
/// children over seven hours).
#[cfg(target_os = "linux")]
fn warte_ende(pid: i32, grund: &str) {
    let ende = Instant::now() + FRIST;
    let mut geendet = false;
    while !geendet {
        geendet = !std::path::Path::new(&format!("/proc/{pid}")).exists() || zombie(pid);
        if Instant::now() > ende {
            break;
        }
        if !geendet {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    if !geendet && let Some(p) = rustix::process::Pid::from_raw(pid) {
        let _ = rustix::process::kill_process(p, rustix::process::Signal::KILL);
    }
    assert!(geendet, "{grund}");
}

/// Starts the program as the child of an `sh` that stays alive until this test kills it; gives back the helper and the
/// PID of the program. With `ignoriert` the helper sets SIGTERM to `SIG_IGN` (`trap '' TERM`). Such a disposition
/// survives `execve`, so the program inherits it like the orphans measured on 21.11.2026 (`SigIgn` 0x5000). `aus`
/// becomes the standard output of both.
#[cfg(target_os = "linux")]
fn unter_hilfsprozess(
    w: &Wegwerf,
    a: &[OsString],
    ignoriert: bool,
    aus: std::process::Stdio,
) -> (std::process::Child, i32) {
    let pid_datei = w.pfad("kind.pid");
    let _ = fs::remove_file(&pid_datei);
    // the helper replaces itself by `sleep`, so the PID of the parent stays the one of the child started here
    let skript = if ignoriert {
        "trap '' TERM; \"$0\" \"$@\" & echo $! >\"$P\"; exec sleep 300"
    } else {
        "\"$0\" \"$@\" & echo $! >\"$P\"; exec sleep 300"
    };
    let mut helfer = std::process::Command::new("sh")
        .arg("-c")
        .arg(skript)
        .arg(gemeinsam::programm())
        .args(a)
        .env_clear()
        .env("P", &pid_datei)
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .stdout(aus)
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("sh");
    let ende = Instant::now() + FRIST;
    // The helper goes to the caller with its pid, in one place at the end: every other way out of the loop ends it
    // and takes its line first, so no test leaves an orphan behind.
    let pid = loop {
        if let Some(pid) = fs::read_to_string(&pid_datei)
            .ok()
            .and_then(|t| t.trim().parse::<i32>().ok())
        {
            break pid;
        }
        if Instant::now() >= ende {
            let _ = helfer.kill();
            let _ = helfer.wait();
            panic!("the helper wrote no pid");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    (helfer, pid)
}

/// Waits at most [`FRIST`] for `pfad` to exist.
#[cfg(target_os = "linux")]
fn warte_datei(pfad: &std::path::Path, grund: &str) {
    let ende = Instant::now() + FRIST;
    while !pfad.exists() {
        assert!(Instant::now() < ende, "{grund}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// An inherited `SIG_IGN` for SIGTERM must not swallow the death signal of the parent (contract 8, section 3): the
/// handlers of SIGTERM and SIGINT stand in step 1, before `PR_SET_PDEATHSIG`, and the thread of the end runs from
/// there on. Here the program hangs in step 5 of a trial run on the lock of the anchor file, which this test holds, so
/// its parent dies long before readiness. Without the handlers of step 1 the program lives on: the kernel discarded
/// the signal (`SIG_IGN`), and the handler of signal-hook came only after step 7.
#[cfg(target_os = "linux")]
#[test]
fn an_inherited_sig_ign_does_not_swallow_the_death_signal_before_readiness() {
    for ignoriert in [true, false] {
        let w = Wegwerf::neu("sigign");
        let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
        let protokoll = w.protokoll();
        let zaehlordner = w.pfad("zaehlung");
        fs::create_dir(&zaehlordner).expect("folder");
        fs::set_permissions(&zaehlordner, fs::Permissions::from_mode(0o700)).expect("chmod");
        let zaehldatei = zaehlordner.join("marker.json");
        // the lock of the chain lies on the anchor file of the source; this test holds it, so step 5 waits for it
        let anker = fs::File::options()
            .read(true)
            .append(true)
            .create(true)
            .open(protokoll.join("gateway.anker.jsonl"))
            .expect("anchor");
        anker.lock().expect("lock");
        let mut a = argv(
            &w.pfad("m.sock"),
            "http://127.0.0.1:9/v1",
            &schluessel,
            &protokoll,
        );
        a.extend(
            [
                "--probelauf",
                "--marker",
                "MK-0",
                "--marker-zaehlung",
                zaehldatei.to_str().expect("utf8"),
            ]
            .iter()
            .map(OsString::from),
        );
        let (mut helfer, pid) = unter_hilfsprozess(&w, &a, ignoriert, std::process::Stdio::null());
        // step 5 writes the count file first and waits for the lock after it: from here on nothing becomes ready
        warte_datei(&zaehldatei, "step 5 of the trial run was not reached");
        assert!(
            std::path::Path::new(&format!("/proc/{pid}")).exists() && !zombie(pid),
            "ignoriert={ignoriert}: the program ended before its parent"
        );
        // SIGKILL: a SIGTERM would not reach a helper that ignores it
        helfer.kill().expect("kill the helper");
        helfer.wait().expect("wait for the helper");
        warte_ende(
            pid,
            &format!("ignoriert={ignoriert}: the model path outlived its parent before readiness"),
        );
        drop(anker);
    }
}

/// An end before readiness leaves no socket (contract 8, section 3): the program hangs in step 9 writing the ready
/// line into a full pipe, with its socket from step 7 already there. Its parent dies, and the thread of the end
/// deletes the socket of this process, which the bind remembered in the same step as it made it.
#[cfg(target_os = "linux")]
#[test]
fn an_end_between_the_socket_and_the_ready_line_deletes_the_socket() {
    let w = Wegwerf::neu("vorbereit");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let socket = w.pfad("m.sock");
    let a = argv(
        &socket,
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    // a pipe filled to its capacity: the 16 bytes of the ready line block. The reading end stays open and unread here,
    // so the write of the program waits instead of failing.
    let (_lese, schreibe) = io::pipe().expect("pipe");
    rustix::fs::fcntl_setfl(&schreibe, rustix::fs::OFlags::NONBLOCK).expect("non blocking");
    let mut schreiber = schreibe;
    let block = vec![b'x'; 4096];
    let mut gefuellt = 0usize;
    // the bound only keeps the loop finite; a full pipe answers with `WouldBlock` long before it
    for _ in 0..1024 {
        match schreiber.write(&block) {
            Ok(n) => gefuellt += n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => panic!("fill the pipe: {e}"),
        }
    }
    assert!(gefuellt >= block.len(), "the pipe took {gefuellt} bytes");
    // the flags belong to the open file description and the child inherits them: back to blocking before the start
    rustix::fs::fcntl_setfl(&schreiber, rustix::fs::OFlags::empty()).expect("blocking again");
    let (mut helfer, pid) = unter_hilfsprozess(&w, &a, true, std::process::Stdio::from(schreiber));
    warte_datei(&socket, "step 7 did not bind the socket");
    helfer.kill().expect("kill the helper");
    helfer.wait().expect("wait for the helper");
    warte_ende(pid, "the model path outlived its parent before readiness");
    assert!(
        !socket.exists(),
        "the socket of this process stays after an end before readiness"
    );
}

#[cfg(target_os = "linux")]
fn zombie(pid: i32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s.rsplit(") ").next().is_some_and(|r| r.starts_with('Z')),
        Err(_) => true,
    }
}

#[test]
fn at_most_64_connections_are_served_at_once() {
    let w = Wegwerf::neu("faeden");
    let d: Arc<Dienst> = dienst(
        &w,
        "http://127.0.0.1:9/v1",
        MODELL,
        SCHLUESSEL,
        Box::new(Skript::ok()),
    );
    let plaetze: Vec<_> = (0..FAEDEN_MAX).map(|_| d.faden()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    let d2 = Arc::clone(&d);
    std::thread::spawn(move || {
        let f = d2.faden();
        tx.send(()).expect("send");
        drop(f);
    });
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "a 65th place was given"
    );
    assert!(!d.warte_leer(Duration::from_millis(50)));
    drop(plaetze);
    rx.recv_timeout(FRIST).expect("a freed place is given");
    assert!(d.warte_leer(FRIST));
    let halter = d.faden();
    let d3 = Arc::clone(&d);
    let warter = std::thread::spawn(move || d3.warte_leer(FRIST));
    std::thread::sleep(Duration::from_millis(50));
    drop(halter);
    assert!(warter.join().expect("thread"));
}

#[test]
fn requests_in_work_end_with_a_line_when_torn_down() {
    let tor = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let skript = Skript {
        tor: Some(Arc::clone(&tor)),
        lesefehler: Some(io::ErrorKind::ConnectionReset),
        ..Skript::ok()
    };
    let w = Wegwerf::neu("abriss");
    let d = dienst(
        &w,
        "http://127.0.0.1:9/v1",
        MODELL,
        SCHLUESSEL,
        Box::new(skript.clone()),
    );
    let d2 = Arc::clone(&d);
    let faden = std::thread::spawn(move || anfrage(&d2, &post(&koerper("x"), "")));
    let ende = Instant::now() + FRIST;
    while skript.verbunden.load(Ordering::SeqCst) < 1 {
        assert!(Instant::now() < ende);
        std::thread::sleep(Duration::from_millis(2));
    }
    d.abreissen_alle();
    skript.oeffnen();
    let _ = faden.join().expect("thread");
    let z = zeilen(&w.protokoll());
    assert_eq!(z.len(), 1);
    assert_eq!(z[0]["grund"], "mw.ziel_nicht_erreichbar");
    let nach = anfrage(&d, &post(&koerper("x"), ""));
    assert!(nach.is_empty(), "{}", String::from_utf8_lossy(&nach));
    assert_eq!(zeilen(&w.protokoll()).len(), 2);
}

/// The harness has written its request and shut its writing half down before `bediene`; the end of the process then
/// comes before the service reads. The tear-down must not depend on `shutdown` of the system: on macOS it has no effect
/// on a stream whose other side shut down its writing half, and the request was served and answered (without the lock in
/// the sheath this test is red there; on Linux it was green before). Expected: an empty answer and exactly one new line.
#[test]
fn a_half_closed_request_is_not_served_after_the_tear_down() {
    let w = Wegwerf::neu("abriss-halb");
    let d = dienst(
        &w,
        "http://127.0.0.1:9/v1",
        MODELL,
        SCHLUESSEL,
        Box::new(Skript::ok()),
    );
    let (mut harness, modellweg) = UnixStream::pair().expect("pair");
    harness.set_read_timeout(Some(FRIST)).expect("timeout");
    harness
        .write_all(&post(&koerper("x"), ""))
        .expect("request");
    harness
        .shutdown(std::net::Shutdown::Write)
        .expect("half close");
    d.abreissen_alle();
    d.bediene(modellweg);
    let mut antwort = Vec::new();
    // The service closes with the request unread: the end may come as a reset, the bytes read before it are the answer.
    let _ = harness.read_to_end(&mut antwort);
    assert!(antwort.is_empty(), "{}", String::from_utf8_lossy(&antwort));
    assert_eq!(zeilen(&w.protokoll()).len(), 1);
}

#[test]
fn the_wrong_call_starts_nothing() {
    let (exit, out, err) = lauf_bis_ende(&[OsString::from("--socket")]);
    assert_eq!(
        (exit, out.as_slice(), err.as_slice()),
        (64, &b""[..], &b"mw.aufruf: Aufruf ungueltig\n"[..])
    );
}

#[test]
fn a_finished_request_does_not_take_others_out_of_the_tear_down() {
    let skript = Skript {
        haengen: true,
        ..Skript::antwortet(vec![
            b"HTTP/1.1 200 X\r\nContent-Type: text/plain\r\n\r\n".to_vec(),
        ])
    };
    let w = Wegwerf::neu("zwei");
    let d = dienst(
        &w,
        "http://127.0.0.1:9/v1",
        MODELL,
        SCHLUESSEL,
        Box::new(skript.clone()),
    );
    let (tx, rx) = std::sync::mpsc::channel();
    for _ in 0..2 {
        let d = Arc::clone(&d);
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(anfrage(&d, &post(&koerper("x"), "")));
        });
    }
    let ende = Instant::now() + FRIST;
    while skript.haenger.lock().expect("lock").len() < 2 {
        assert!(Instant::now() < ende);
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        Antwort::lies(&anfrage(&d, b"GET /x HTTP/1.1\r\n\r\n")).status,
        404
    );
    d.abreissen_alle();
    let beide: Vec<_> = (0..2)
        .map(|_| rx.recv_timeout(Duration::from_secs(2)))
        .collect();
    skript.alle_freigeben();
    for b in beide {
        let roh = b.expect("the tear-down reached the request");
        assert!(
            roh.is_empty() || Antwort::lies(&roh).status == 200,
            "{}",
            String::from_utf8_lossy(&roh)
        );
    }
    assert_eq!(zeilen(&w.protokoll()).len(), 3);
}

#[test]
fn a_body_of_exactly_33554432_bytes_is_accepted() {
    let w = Wegwerf::neu("gross");
    let skript = Skript::ok();
    let d = dienst(
        &w,
        "http://127.0.0.1:9/v1",
        MODELL,
        SCHLUESSEL,
        Box::new(skript.clone()),
    );
    let mut k = format!(r#"{{"model":"{MODELL}","x":""#).into_bytes();
    k.resize(33_554_432 - 2, b'a');
    k.extend_from_slice(b"\"}");
    let mut roh = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        k.len()
    )
    .into_bytes();
    roh.extend_from_slice(&k);
    assert_eq!(Antwort::lies(&anfrage(&d, &roh)).status, 200);
    assert_eq!(skript.verbunden.load(Ordering::SeqCst), 1);
}

#[test]
fn a_broken_answer_keeps_its_head_and_drops_the_held_bytes() {
    let w = Wegwerf::neu("bruch");
    let skript = Skript {
        abbruch_am_ende: true,
        ..Skript::antwortet(vec![
            b"HTTP/1.1 200 X\r\nContent-Type: text/plain\r\nContent-Length: 50\r\n\r\n".to_vec(),
            b"abc".to_vec(),
        ])
    };
    let d = dienst(
        &w,
        "http://127.0.0.1:9/v1",
        MODELL,
        SCHLUESSEL,
        Box::new(skript),
    );
    let a = Antwort::lies(&anfrage(&d, &post(&koerper("x"), "")));
    assert_eq!((a.status, a.koerper.as_slice()), (200, &b""[..]));
    let z = zeilen(&w.protokoll());
    assert_eq!(
        (z[0]["ergebnis"].as_str(), z[0]["status"].as_u64()),
        (Some("weitergereicht"), Some(200))
    );

    let w = Wegwerf::neu("bruch2");
    let mut lang = b"data: ".to_vec();
    lang.resize(100, b'x');
    let skript = Skript {
        abbruch_am_ende: true,
        ..Skript::antwortet(vec![
            b"HTTP/1.1 200 X\r\nContent-Type: text/plain\r\n\r\n".to_vec(),
            lang.clone(),
        ])
    };
    let d = dienst(
        &w,
        "http://127.0.0.1:9/v1",
        MODELL,
        SCHLUESSEL,
        Box::new(skript),
    );
    let roh = anfrage(&d, &post(&koerper("x"), ""));
    let a = Antwort::lies(&roh);
    assert_eq!(
        a.koerper,
        lang[..lang.len() - (SCHLUESSEL.len() - 1)].to_vec()
    );
    assert_eq!(roh.windows(8).filter(|w| *w == b"HTTP/1.1").count(), 1);
}

/// Duty 5 of section 3a: a child before the ready line ends the start with exit 70 `mw.kind_verboten`, writes nothing on
/// stdout and deletes the socket it made. The program reads its own threads for this (`tests/sicherer_modus.rs`,
/// MW-L5); here the answer of that reading comes from the test.
#[test]
fn a_child_before_the_ready_line_ends_the_start() {
    let w = Wegwerf::neu("kind");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let a = argv(
        &w.pfad("m.sock"),
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    let aufloeser = |_: &str, p: u16| -> io::Result<Vec<SocketAddr>> {
        Ok(vec![SocketAddr::from(([127, 0, 0, 1], p))])
    };
    let gefragt = std::cell::Cell::new(0);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let ergebnis = modellweg::start::starten_mit(
        &a,
        rustix::process::getuid().as_raw(),
        &aufloeser,
        FRISTEN,
        Box::new(|| gemeinsam::ZEIT),
        &mut out,
        &mut err,
        || {
            gefragt.set(gefragt.get() + 1);
            Err(Startfehler::KindVerboten)
        },
    );
    assert_eq!(ergebnis.err(), Some(70));
    assert_eq!(gefragt.get(), 1);
    assert_eq!(out, b"");
    assert_eq!(err, b"mw.kind_verboten: Kindprozess verboten\n");
    assert!(!w.pfad("m.sock").exists());

    // the same start with the answer "no child" is ready
    let w = Wegwerf::neu("kindlos");
    let schluessel = w.schluessel(format!("{SCHLUESSEL}\n").as_bytes());
    let a = argv(
        &w.pfad("m.sock"),
        "http://127.0.0.1:9/v1",
        &schluessel,
        &w.protokoll(),
    );
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let ergebnis = modellweg::start::starten_mit(
        &a,
        rustix::process::getuid().as_raw(),
        &aufloeser,
        FRISTEN,
        Box::new(|| gemeinsam::ZEIT),
        &mut out,
        &mut err,
        || Ok(()),
    );
    assert!(ergebnis.is_ok());
    assert_eq!(out, b"{\"bereit\":true}\n");
    assert_eq!(err, b"");
    assert!(w.pfad("m.sock").exists());
}
