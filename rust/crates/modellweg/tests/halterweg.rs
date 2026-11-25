//! The holder path of mode 2 (contract 8, section 5.1) against the real holder (`halter`, a dev-dependency of this
//! crate) and against a stand-in of him. The holder runs with a test `halter.json` and a test secret; no test uses a
//! real token, and no test opens a connection to anything but the loopback.
//!
//! The holder itself is Linux only, like mode 2.
#![cfg(target_os = "linux")]

mod gemeinsam;

use std::time::Duration;

use gemeinsam::{Halterattrappe, Wegwerf};
use modellweg::halter::{self, Klient};

fn uid() -> u32 {
    rustix::process::getuid().as_raw()
}

#[test]
fn the_endpoint_is_the_fixed_path_of_the_own_uid() {
    assert_eq!(
        halter::fester_pfad(uid()),
        std::path::PathBuf::from(format!("/run/user/{}/wb-halter/halter.sock", uid()))
    );
}

/// The check of the counterparty (section 5.1): the folder is 0700 and the socket 0600 of the own uid, and the uid of
/// the server out of `SO_PEERCRED` is the own one. A folder or a socket with wider rights is `mw.halter_unecht`, a
/// missing endpoint is `mw.halter_fehlt`.
#[test]
fn the_counterparty_is_checked_before_the_first_line() {
    use std::os::unix::fs::PermissionsExt;
    let w = Wegwerf::neu("hw-gegen");
    let a = Halterattrappe::stumm(&w);
    assert!(Klient::verbinde(&a.pfad(), uid()).is_ok());
    // A socket with group rights.
    std::fs::set_permissions(a.pfad(), std::fs::Permissions::from_mode(0o660)).expect("chmod");
    assert_eq!(
        Klient::verbinde(&a.pfad(), uid()).err(),
        Some("mw.halter_unecht")
    );
    std::fs::set_permissions(a.pfad(), std::fs::Permissions::from_mode(0o600)).expect("chmod");
    // A folder others may enter.
    let ordner = a.pfad().parent().expect("folder").to_path_buf();
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    assert_eq!(
        Klient::verbinde(&a.pfad(), uid()).err(),
        Some("mw.halter_unecht")
    );
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    // Another uid as the expected one: the peer check fails, never the connection.
    assert_eq!(
        Klient::verbinde(&a.pfad(), uid() + 1).err(),
        Some("mw.halter_unecht")
    );
    // Nothing at the path at all.
    assert_eq!(
        Klient::verbinde(&w.pfad("gibt-es-nicht"), uid()).err(),
        Some("mw.halter_fehlt")
    );
    // A file instead of a socket: the connection fails.
    let datei = w.pfad("datei");
    std::fs::write(&datei, b"x").expect("write");
    std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    assert_eq!(
        Klient::verbinde(&datei, uid()).err(),
        Some("mw.halter_fehlt")
    );
}

/// A holder that says nothing: the wait ends with the deadline, and the model path answers `mw.halter_frist`. The
/// deadline of the contract is 600 s; the test sets a short one on the same code.
#[test]
fn a_silent_holder_ends_with_the_deadline() {
    let w = Wegwerf::neu("hw-frist");
    let a = Halterattrappe::stumm(&w);
    let mut k = Klient::verbinde(&a.pfad(), uid()).expect("holder");
    k.frist(Some(Duration::from_millis(200))).expect("deadline");
    let id = k.naechste_id();
    k.senden(&halter::modell_aufruf(id, "a.b", "GET", "/x", None, &[], b"").expect("line"))
        .expect("send");
    assert_eq!(k.naechste_zeile().err(), Some("mw.halter_frist"));
}

/// A holder that closes without an answer, and one that sends a line outside the closed form: both are
/// `mw.halter_fehler`, never a fall back on anything.
#[test]
fn a_torn_connection_and_an_unreadable_line_are_a_failure_of_the_holder() {
    let w = Wegwerf::neu("hw-abriss");
    let zu = Halterattrappe::neu(&w, vec!["ZU".to_owned()]);
    let mut k = Klient::verbinde(&zu.pfad(), uid()).expect("holder");
    let id = k.naechste_id();
    k.senden(&halter::modell_suche(id, "a.b", &["/x".to_owned()]).expect("line"))
        .expect("send");
    assert_eq!(k.naechste_zeile().err(), Some("mw.halter_fehler"));
    let w2 = Wegwerf::neu("hw-abriss2");
    let leer = Halterattrappe::neu(
        &w2,
        vec![r#"{"art":"erfunden","fassung":1,"id":1}"#.to_owned()],
    );
    let mut k = Klient::verbinde(&leer.pfad(), uid()).expect("holder");
    let id = k.naechste_id();
    k.senden(&halter::modell_suche(id, "a.b", &["/x".to_owned()]).expect("line"))
        .expect("send");
    assert_eq!(k.naechste_zeile().err(), Some("mw.halter_fehler"));
}

/// The ops of the role `bruecke` against the **real** holder (`halter`, a dev-dependency): the holder runs with a test
/// `halter.json`, a test secret and a stand-in of the target on the loopback, and the client of the model path speaks
/// to him over a socket pair. So the line protocol, the order of checks and the codes of section 5.1 are measured
/// against the real counterpart, not only against a stand-in of him.
mod echt {
    use std::collections::BTreeMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ::halter::ablage::{Ablage, EBENEN};
    use ::halter::dienst::Dienst;
    use ::halter::konfig;
    use ::halter::pruefung::Rolle;
    use modellweg::b64;
    use modellweg::halter::{self, Klient, Zeile};
    use serde_json::json;

    /// The invented test secret of these tests; it is never a real token and never tokenlike.
    const GEHEIM: &str = "hgx1-testgeheimnis-0123";
    const JETZT: i64 = 1_795_000_000_000;

    static ZAEHLER: AtomicUsize = AtomicUsize::new(0);

    fn uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    fn uhr() -> i64 {
        JETZT
    }

    fn ordner(name: &str) -> PathBuf {
        let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("hw-{}-{n}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("folder");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        p
    }

    /// A home with the store of the holder; with `mit_geheimnis` the test secret `anmeldung.copilot` lies in it.
    fn heim(name: &str, mit_geheimnis: bool) -> PathBuf {
        let heim = ordner(name);
        let mut g = heim.clone();
        for e in EBENEN {
            g = g.join(e);
            std::fs::create_dir(&g).expect("level");
            std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        }
        if mit_geheimnis {
            let p = g.join("anmeldung.copilot");
            std::fs::write(&p, format!("{GEHEIM}\n")).expect("secret");
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        }
        heim
    }

    /// A stand-in of a model target on the loopback: it answers every request with `antwort` and keeps what it got.
    /// `tls` is `false`, which `halter.json` allows for `127.0.0.1` only.
    fn ziel_attrappe(antwort: &'static str) -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
        let horcher = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = horcher.local_addr().expect("addr").port();
        let gesehen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = Arc::clone(&gesehen);
        std::thread::spawn(move || {
            for strom in horcher.incoming() {
                let Ok(mut strom) = strom else { break };
                let g = Arc::clone(&g);
                std::thread::spawn(move || {
                    let mut leser = BufReader::new(strom.try_clone().expect("clone"));
                    let mut kopf = String::new();
                    loop {
                        let mut zeile = String::new();
                        match leser.read_line(&mut zeile) {
                            Ok(0) | Err(_) => return,
                            Ok(_) => {}
                        }
                        let fertig = zeile == "\r\n";
                        kopf.push_str(&zeile);
                        if fertig {
                            break;
                        }
                    }
                    g.lock().expect("lock").push(kopf);
                    let _ = strom.write_all(antwort.as_bytes());
                    let _ = strom.flush();
                });
            }
        });
        (port, gesehen)
    }

    /// The real holder for the target `127.0.0.1:<port>` with the paths `pfade`, and a client of the model path on the
    /// other end of a socket pair. The role comes from the effective gid of this process, as in the tests of the
    /// holder himself.
    fn halter_und_klient(
        name: &str,
        port: u16,
        pfade: &[&str],
        mit_geheimnis: bool,
    ) -> (Klient, Arc<Dienst>) {
        let k = json!({
            "fassung": 1,
            "konten": {uid().to_string(): {"person": null}},
            "rollen": {"bruecke": "wb-test"},
            "ziele": {"127.0.0.1": {"host": "127.0.0.1", "port": port, "tls": false, "praefix": "",
                "kopf": "authorization", "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot",
                "pfade": pfade}},
            "server": {},
            "kids": {},
        });
        let konfig = konfig::lesen(k.to_string().as_bytes()).expect("configuration");
        let heim = heim(name, mit_geheimnis);
        let rollen = BTreeMap::from([(
            rustix::process::getegid().as_raw(),
            Rolle::aus_text("bruecke").expect("role"),
        )]);
        let d = Arc::new(Dienst::neu(
            konfig,
            Ablage::neu(&heim, uid()),
            uid(),
            JETZT - 3_600_000,
            rollen,
            None,
            uhr,
        ));
        let (a, b) = UnixStream::pair().expect("pair");
        let dienst = Arc::clone(&d);
        std::thread::spawn(move || dienst.verbindung(b));
        (Klient::auf(a), d)
    }

    /// A good way of `modell.aufruf`: the holder sets the header with the token himself, the answer comes as
    /// `beginn`, `teil` and `ende`, and the value never stands in a line that goes back.
    #[test]
    fn a_call_goes_out_with_the_token_of_the_holder_and_comes_back_as_a_stream() {
        let (port, gesehen) = ziel_attrappe(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"login\":1}",
        );
        let (mut k, _d) = halter_und_klient("ruf", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "GET",
                "/copilot_internal/user",
                None,
                &[("accept".to_owned(), "application/json".to_owned())],
                b"",
            )
            .expect("line"),
        )
        .expect("send");
        let mut koerper = Vec::new();
        let mut status = 0;
        let mut kopf = Vec::new();
        loop {
            match k.naechste_zeile().expect("line of the holder") {
                Zeile::Beginn { status: s, kopf: h } => {
                    status = s;
                    kopf = h;
                }
                Zeile::Teil(d) => koerper.extend_from_slice(&d),
                Zeile::Ende(code) => {
                    assert_eq!(code, None);
                    break;
                }
                Zeile::Antwort { .. } => panic!("a call answers as a stream"),
            }
        }
        assert_eq!(status, 200);
        assert_eq!(koerper, br#"{"login":1}"#);
        // The holder passes the headers of the target after his own block list; the framing is not among them.
        assert!(
            kopf.iter()
                .any(|(n, w)| n == "content-type" && w == "application/json"),
            "{kopf:?}"
        );
        assert!(
            !kopf
                .iter()
                .any(|(n, _)| n == "content-length" || n == "transfer-encoding"),
            "{kopf:?}"
        );
        // The target got the token of the holder and the header `accept` of the request; the model path never had it.
        let an_ziel = gesehen.lock().expect("lock").clone();
        assert_eq!(an_ziel.len(), 1, "{an_ziel:?}");
        assert!(
            an_ziel[0].contains(&format!("authorization: Bearer {GEHEIM}")),
            "{an_ziel:?}"
        );
        assert!(
            an_ziel[0]
                .to_lowercase()
                .contains("accept: application/json"),
            "{an_ziel:?}"
        );
        assert!(
            an_ziel[0]
                .to_lowercase()
                .contains("accept-encoding: identity"),
            "{an_ziel:?}"
        );
    }

    /// A body goes as `koerper_b64` in one piece, and the holder frames it towards the target.
    #[test]
    fn a_body_goes_as_base64_in_one_piece() {
        let (port, gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let (mut k, _d) = halter_und_klient("koerper", port, &["POST /chat/completions"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "POST",
                "/chat/completions",
                Some("x=1"),
                &[("content-type".to_owned(), "application/json".to_owned())],
                br#"{"a":1}"#,
            )
            .expect("line"),
        )
        .expect("send");
        assert!(matches!(
            k.naechste_zeile(),
            Ok(Zeile::Beginn { status: 200, .. })
        ));
        let an_ziel = gesehen.lock().expect("lock").clone();
        assert!(
            an_ziel[0].starts_with("POST /chat/completions?x=1 HTTP/1.1"),
            "{an_ziel:?}"
        );
        assert!(
            an_ziel[0].to_lowercase().contains("content-length: 7"),
            "{an_ziel:?}"
        );
    }

    /// Every code of his order of checks becomes an own code of the model path (section 5.1), and the holder is the
    /// last lock for the path: a path outside `pfade` never reaches the target.
    #[test]
    fn the_codes_of_the_holder_become_own_codes() {
        let (port, gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let (mut k, _d) = halter_und_klient("codes", port, &["GET /copilot_internal/user"], true);
        // A path no rule covers: stage 15 of the holder, before anything reaches the target.
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(id, "127.0.0.1", "GET", "/user/repos", None, &[], b"")
                .expect("line"),
        )
        .expect("send");
        let code = match k.naechste_zeile().expect("line") {
            Zeile::Antwort { code, .. } | Zeile::Ende(code) => code.expect("code"),
            _ => panic!("a refusal is one line"),
        };
        assert_eq!(code, "halter.pfad_verboten");
        assert_eq!(halter::auf_mw(&code), "mw.halter_fehler");
        assert!(
            gesehen.lock().expect("lock").is_empty(),
            "the target was touched"
        );
        // An unknown target: stage 14.
        let (mut k, _d) = halter_und_klient("codes2", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(id, "a.b", "GET", "/copilot_internal/user", None, &[], b"")
                .expect("line"),
        )
        .expect("send");
        let code = match k.naechste_zeile().expect("line") {
            Zeile::Antwort { code, .. } | Zeile::Ende(code) => code.expect("code"),
            _ => panic!("a refusal is one line"),
        };
        assert_eq!(code, "halter.ziel_unbekannt");
        assert_eq!(halter::auf_mw(&code), "mw.halter_fehler");
    }

    /// Without a secret for the target the holder answers `halter.kein_schluessel`, which becomes
    /// `mw.halter_kein_schluessel` (503 in the tunnel, section 13.2, stage 9).
    #[test]
    fn a_target_without_a_secret_names_its_own_code() {
        let (port, gesehen) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let (mut k, _d) = halter_und_klient("ohne", port, &["GET /copilot_internal/user"], false);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "GET",
                "/copilot_internal/user",
                None,
                &[],
                b"",
            )
            .expect("line"),
        )
        .expect("send");
        let code = match k.naechste_zeile().expect("line") {
            Zeile::Antwort { code, .. } | Zeile::Ende(code) => code.expect("code"),
            _ => panic!("a refusal is one line"),
        };
        assert_eq!(code, "halter.kein_schluessel");
        assert_eq!(halter::auf_mw(&code), "mw.halter_kein_schluessel");
        assert!(
            gesehen.lock().expect("lock").is_empty(),
            "the target was touched"
        );
    }

    /// The leak lock of the holder: a target that mirrors the token back ends the stream with
    /// `halter.leck_gesperrt`, and the model path turns it into `mw.halter_fehler`. No byte of the value comes back.
    #[test]
    fn a_target_that_mirrors_the_token_is_locked_by_the_holder() {
        let antwort: &'static str = Box::leak(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\nToken: {GEHEIM}",
                7 + GEHEIM.len()
            )
            .into_boxed_str(),
        );
        let (port, _gesehen) = ziel_attrappe(antwort);
        let (mut k, _d) = halter_und_klient("leck", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_aufruf(
                id,
                "127.0.0.1",
                "GET",
                "/copilot_internal/user",
                None,
                &[],
                b"",
            )
            .expect("line"),
        )
        .expect("send");
        let mut gesehen_bytes = Vec::new();
        let code = loop {
            match k.naechste_zeile().expect("line") {
                Zeile::Beginn { .. } => {}
                Zeile::Teil(d) => gesehen_bytes.extend_from_slice(&d),
                Zeile::Ende(code) => break code.expect("the lock gives a code"),
                Zeile::Antwort { code, .. } => break code.expect("code"),
            }
        };
        assert_eq!(code, "halter.leck_gesperrt");
        assert_eq!(halter::auf_mw(&code), "mw.halter_fehler");
        let text = String::from_utf8_lossy(&gesehen_bytes).into_owned();
        assert!(!text.contains(GEHEIM), "the value came back: {text:?}");
    }

    /// `modell.suche` of the search (section 9a, step 3a): the holder searches the given paths for his own secret and
    /// answers only with numbers.
    #[test]
    fn the_search_of_the_holder_answers_only_numbers() {
        let (port, _g) = ziel_attrappe("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let arbeit = ordner("suche-in");
        std::fs::write(arbeit.join("harmlos.txt"), b"ohne Geheimnis\n").expect("write");
        std::fs::write(arbeit.join("leck.txt"), format!("x {GEHEIM} y")).expect("write");
        let (mut k, _d) = halter_und_klient("suche", port, &["GET /copilot_internal/user"], true);
        let id = k.naechste_id();
        let pfade = vec![arbeit.to_string_lossy().into_owned()];
        k.senden(&halter::modell_suche(id, "127.0.0.1", &pfade).expect("line"))
            .expect("send");
        match k.naechste_zeile().expect("line") {
            Zeile::Antwort {
                code: None,
                suche: Some(s),
                ..
            } => {
                assert_eq!(s.treffer, 1, "{s:?}");
                assert_eq!(s.dateien, 2, "{s:?}");
                assert_eq!(s.nicht_lesbar, 0, "{s:?}");
            }
            z => panic!("the search answers with numbers: {z:?}"),
        }
    }

    /// A stream over `modell.strom`: the holder answers the 101 without `upgrade` and `connection` (both stand on his
    /// block list), the bytes of the target come as `teil`, and the bytes of the harness go as `strom.teil`.
    #[test]
    fn a_stream_carries_the_bytes_of_both_ways() {
        let horcher = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = horcher.local_addr().expect("addr").port();
        let gesehen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = Arc::clone(&gesehen);
        std::thread::spawn(move || {
            let (mut strom, _) = horcher.accept().expect("accept");
            let mut leser = BufReader::new(strom.try_clone().expect("clone"));
            loop {
                let mut zeile = String::new();
                match leser.read_line(&mut zeile) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
                if zeile == "\r\n" {
                    break;
                }
            }
            let _ = strom.write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            );
            let _ = strom.write_all(b"\x81\x05Hallo");
            let _ = strom.flush();
            // Everything the harness sent into the stream.
            let mut rest = [0_u8; 64];
            if let Ok(n) = std::io::Read::read(&mut strom, &mut rest) {
                g.lock().expect("lock").extend_from_slice(&rest[..n]);
            }
        });
        let (mut k, _d) = halter_und_klient("strom", port, &["GET /responses"], true);
        let id = k.naechste_id();
        k.senden(
            &halter::modell_strom(
                id,
                "127.0.0.1",
                "/responses",
                None,
                &[("sec-websocket-version".to_owned(), "13".to_owned())],
            )
            .expect("line"),
        )
        .expect("send");
        let (status, kopf) = match k.naechste_zeile().expect("line") {
            Zeile::Beginn { status, kopf } => (status, kopf),
            z => panic!("the holder answers the 101 as beginn: {z:?}"),
        };
        assert_eq!(status, 101);
        assert!(
            !kopf
                .iter()
                .any(|(n, _)| n == "upgrade" || n == "connection"),
            "both stand on his block list: {kopf:?}"
        );
        // The way from the harness: the bytes go unchanged into the stream.
        k.senden(&halter::strom_teil(id, b"\x8a\x00").expect("line"))
            .expect("send");
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if gesehen.lock().expect("lock").as_slice() == b"\x8a\x00" {
                break;
            }
            assert!(
                std::time::Instant::now() < frist,
                "the bytes of the harness"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // The bytes of the target go as `teil`. The leak lock of the holder holds back the last bytes of the payload
        // until a later part covers them (holder contract, section 8); at the end of the stream they go out as the
        // last `teil` before `ende`. The sum of the parts is therefore the whole frame, measured after `strom.ende`.
        k.frist(Some(std::time::Duration::from_secs(5)))
            .expect("deadline");
        k.senden(&halter::strom_ende(id).expect("line"))
            .expect("send");
        let mut bytes = Vec::new();
        loop {
            match k.naechste_zeile().expect("line") {
                Zeile::Teil(d) => bytes.extend_from_slice(&d),
                Zeile::Ende(code) => {
                    assert_eq!(code, None);
                    break;
                }
                z => panic!("a stream carries parts and one end: {z:?}"),
            }
        }
        assert_eq!(bytes, b"\x81\x05Hallo");
        assert_eq!(b64::kodieren(&bytes), "gQVIYWxsbw==");
    }
}
